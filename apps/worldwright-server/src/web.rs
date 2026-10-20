use crate::{
    auth,
    world::{DisconnectReason, Outbound, SubmitError, WorldHandle, cancel_first},
};
use axum::{
    Router,
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use std::{collections::HashSet, fmt::Write as _, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    time::{Instant as TokioInstant, MissedTickBehavior, interval, timeout, timeout_at},
};
use worldwright_core::ids::ConnectionId;
use worldwright_protocol::{
    ClientMessage, ErrorCode, MAX_FRAME_BYTES, PROTOCOL_VERSION, ServerEnvelope, ServerMessage,
    decode_client, encode_server, negotiate_version,
};

const INDEX: &str = include_str!("../assets/index.html");
const APP_JS: &str = include_str!("../assets/app.js");
const APP_CSS: &str = include_str!("../assets/app.css");
const CSP: &str = "default-src 'self'; connect-src 'self'; script-src 'self'; style-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const QUEUE_TIMEOUT: Duration = Duration::from_secs(1);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) allowed_origin: String,
    pub(crate) auth: auth::AuthService,
    pub(crate) world: WorldHandle,
}

struct ConnectionContext<'a> {
    outbound: &'a mpsc::Sender<Outbound>,
    cancel_tx: &'a watch::Sender<Option<DisconnectReason>>,
    cancel_rx: watch::Receiver<Option<DisconnectReason>>,
    world: &'a WorldHandle,
    auth: &'a auth::AuthService,
    principal: &'a auth::Principal,
    session_token: &'a str,
    connection: ConnectionId,
}

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/app.css", get(app_css))
        .route("/api/v1/session", post(login).delete(logout))
        .route("/api/v1/play", get(play))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .layer(axum::extract::DefaultBodyLimit::max(4096))
        .layer(middleware::map_response(security_headers))
        .with_state(state)
}

async fn security_headers(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert("content-security-policy", HeaderValue::from_static(CSP));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

async fn index() -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], INDEX).into_response()
}

async fn app_js() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        APP_JS,
    )
        .into_response()
}

async fn app_css() -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], APP_CSS).into_response()
}

async fn live() -> Response {
    (StatusCode::OK, "live\n").into_response()
}

async fn ready(State(state): State<AppState>) -> Response {
    if state.world.is_ready() {
        (StatusCode::OK, "ready\n").into_response()
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, "not ready\n").into_response()
    }
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(payload): axum::Json<auth::LoginRequest>,
) -> Response {
    if !origin_matches(&headers, &state.allowed_origin) {
        return (StatusCode::FORBIDDEN, "forbidden\n").into_response();
    }
    match state.auth.login(payload.username, payload.password).await {
        Ok(token) => {
            let mut response = StatusCode::NO_CONTENT.into_response();
            if let Ok(value) =
                HeaderValue::from_str(&auth::session_cookie(&token, state.auth.cookie_secure()))
            {
                response.headers_mut().insert(header::SET_COOKIE, value);
            }
            response
        }
        Err(auth::AuthError::Saturated | auth::AuthError::SessionCapacity) => {
            (StatusCode::SERVICE_UNAVAILABLE, "temporarily unavailable\n").into_response()
        }
        Err(auth::AuthError::Internal) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "internal server error\n").into_response()
        }
        Err(auth::AuthError::InvalidInput | auth::AuthError::Unauthorized) => {
            auth::unauthorized_response()
        }
    }
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !origin_matches(&headers, &state.allowed_origin) {
        return (StatusCode::FORBIDDEN, "forbidden\n").into_response();
    }
    if let Some(token) = auth::token_from_cookie(&headers) {
        state.auth.revoke(&token);
        state.world.revoke_session(token).await;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Ok(value) = HeaderValue::from_str(&auth::expired_cookie(state.auth.cookie_secure())) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

async fn play(
    State(state): State<AppState>,
    headers: HeaderMap,
    websocket: WebSocketUpgrade,
) -> Response {
    if !origin_matches(&headers, &state.allowed_origin) {
        return (StatusCode::FORBIDDEN, "forbidden\n").into_response();
    }
    let Some(token) = auth::token_from_cookie(&headers) else {
        return auth::unauthorized_response();
    };
    let Some(principal) = state.auth.lookup(&token) else {
        return auth::unauthorized_response();
    };
    let connection = match random_connection_id() {
        Ok(connection) => connection,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error\n").into_response();
        }
    };
    let epoch = match random_epoch() {
        Ok(epoch) => epoch,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal server error\n").into_response();
        }
    };
    websocket
        .max_frame_size(MAX_FRAME_BYTES)
        .max_message_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| {
            connection_task(
                socket,
                state.world,
                state.auth,
                principal,
                token,
                connection,
                epoch,
            )
        })
        .into_response()
}

fn origin_matches(headers: &HeaderMap, expected: &str) -> bool {
    let mut origins = headers.get_all(header::ORIGIN).iter();
    origins
        .next()
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| origins.next().is_none() && value.as_bytes() == expected.as_bytes())
}

async fn connection_task(
    socket: WebSocket,
    world: WorldHandle,
    auth: auth::AuthService,
    principal: auth::Principal,
    session_token: String,
    connection: ConnectionId,
    epoch: String,
) {
    let (sink, mut stream) = socket.split();
    let (outbound, receiver) = mpsc::channel(64);
    let (cancel_tx, cancel_rx) = watch::channel(None);
    let writer_cancel = cancel_rx.clone();
    let writer_cancel_tx = cancel_tx.clone();
    let writer = tokio::spawn(writer_task(
        sink,
        receiver,
        epoch,
        writer_cancel,
        writer_cancel_tx,
    ));
    let mut context = ConnectionContext {
        outbound: &outbound,
        cancel_tx: &cancel_tx,
        cancel_rx: cancel_rx.clone(),
        world: &world,
        auth: &auth,
        principal: &principal,
        session_token: &session_token,
        connection,
    };
    let (actor, mut request_ids) = handshake(&mut stream, &mut context).await;
    if let Some(actor) = actor {
        gameplay_loop(&mut stream, &mut context, actor, &mut request_ids).await;
        world.detach(actor).await;
    }
    cancel_first(&cancel_tx, DisconnectReason::Normal);
    drop(outbound);
    let _ = timeout(CLEANUP_TIMEOUT, writer).await;
}

async fn handshake(
    stream: &mut SplitStream<WebSocket>,
    context: &mut ConnectionContext<'_>,
) -> (
    Option<worldwright_core::command::ActorContext>,
    RecentRequestIds,
) {
    let outbound = context.outbound;
    let cancel_tx = context.cancel_tx;
    let cancel_rx = context.cancel_rx.clone();
    let world = context.world;
    let auth = context.auth;
    let principal = context.principal;
    let session_token = context.session_token;
    let connection = context.connection;
    let mut request_ids = RecentRequestIds::default();
    let hello_deadline = TokioInstant::now() + HANDSHAKE_TIMEOUT;
    let first = loop {
        match receive_until(stream, cancel_rx.clone(), hello_deadline).await {
            Receive::Message(message) => break message,
            Receive::Ping(payload) => {
                if !queue(outbound, cancel_tx, Outbound::Pong(payload)).await {
                    return (None, request_ids);
                }
            }
            Receive::Invalid => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::InvalidMessage,
                    "invalid protocol message",
                )
                .await;
                return (None, request_ids);
            }
            Receive::Ignore => {}
            Receive::Closed => return (None, request_ids),
        }
    };
    let ClientMessage::Hello { supported_versions } = first else {
        fatal(
            outbound,
            cancel_tx,
            ErrorCode::ProtocolState,
            "expected hello",
        )
        .await;
        return (None, request_ids);
    };
    let Some(version) = negotiate_version(&supported_versions) else {
        fatal(
            outbound,
            cancel_tx,
            ErrorCode::UnsupportedVersion,
            "unsupported protocol version",
        )
        .await;
        return (None, request_ids);
    };
    if version != PROTOCOL_VERSION {
        fatal(
            outbound,
            cancel_tx,
            ErrorCode::UnsupportedVersion,
            "unsupported protocol version",
        )
        .await;
        return (None, request_ids);
    }
    if !session_matches(auth, principal, session_token) {
        cancel_first(cancel_tx, DisconnectReason::SessionRevoked);
        return (None, request_ids);
    }
    if !queue(
        outbound,
        cancel_tx,
        Outbound::Message(ServerMessage::Welcome {
            heartbeat_interval_ms: 20_000,
        }),
    )
    .await
    {
        return (None, request_ids);
    }
    let attach_deadline = TokioInstant::now() + HANDSHAKE_TIMEOUT;
    loop {
        let message = match receive_until(stream, cancel_rx.clone(), attach_deadline).await {
            Receive::Message(message) => message,
            Receive::Ping(payload) => {
                if !queue(outbound, cancel_tx, Outbound::Pong(payload)).await {
                    return (None, request_ids);
                }
                continue;
            }
            Receive::Ignore => continue,
            Receive::Invalid => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::InvalidMessage,
                    "invalid protocol message",
                )
                .await;
                return (None, request_ids);
            }
            Receive::Closed => return (None, request_ids),
        };
        if !session_matches(auth, principal, session_token) {
            cancel_first(cancel_tx, DisconnectReason::SessionRevoked);
            return (None, request_ids);
        }
        match message {
            ClientMessage::Ping { nonce } => {
                if !queue(
                    outbound,
                    cancel_tx,
                    Outbound::Message(ServerMessage::Pong { nonce }),
                )
                .await
                {
                    return (None, request_ids);
                }
            }
            ClientMessage::Attach { request_id } => {
                if request_ids.contains(&request_id) {
                    let _ = queue(
                        outbound,
                        cancel_tx,
                        Outbound::Message(ServerMessage::Error {
                            request_id: Some(request_id),
                            code: ErrorCode::InvalidCommand,
                            message: "duplicate request ID".to_owned(),
                            fatal: false,
                        }),
                    )
                    .await;
                    continue;
                }
                let actor = match world
                    .attach(
                        principal.clone(),
                        session_token.to_owned(),
                        connection,
                        outbound.clone(),
                        cancel_tx.clone(),
                    )
                    .await
                {
                    Ok(actor) => actor,
                    Err(_) => {
                        fatal(
                            outbound,
                            cancel_tx,
                            ErrorCode::InternalError,
                            "internal server error",
                        )
                        .await;
                        return (None, request_ids);
                    }
                };
                request_ids.record(request_id.clone());
                if !queue(
                    outbound,
                    cancel_tx,
                    Outbound::Message(ServerMessage::Accepted {
                        request_id: request_id.clone(),
                    }),
                )
                .await
                {
                    world.detach(actor).await;
                    return (None, request_ids);
                }
                if world.initial(actor, request_id).await.is_err() {
                    fatal(
                        outbound,
                        cancel_tx,
                        ErrorCode::InternalError,
                        "internal server error",
                    )
                    .await;
                    world.detach(actor).await;
                    return (None, request_ids);
                }
                return (Some(actor), request_ids);
            }
            ClientMessage::Hello { .. } => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::ProtocolState,
                    "unexpected hello",
                )
                .await;
                return (None, request_ids);
            }
            ClientMessage::Command { .. } => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::ProtocolState,
                    "expected attach",
                )
                .await;
                return (None, request_ids);
            }
        }
    }
}

async fn gameplay_loop(
    stream: &mut SplitStream<WebSocket>,
    context: &mut ConnectionContext<'_>,
    actor: worldwright_core::command::ActorContext,
    request_ids: &mut RecentRequestIds,
) {
    let outbound = context.outbound;
    let cancel_tx = context.cancel_tx;
    let cancel_rx = context.cancel_rx.clone();
    let world = context.world;
    let auth = context.auth;
    let principal = context.principal;
    let session_token = context.session_token;
    let mut checks = interval(Duration::from_secs(5));
    checks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        let mut cancellation = cancel_rx.clone();
        let received = tokio::select! {
            biased;
            _ = cancellation.changed() => break,
            _ = checks.tick() => {
                if !session_matches(auth, principal, session_token) {
                    cancel_first(cancel_tx, DisconnectReason::SessionRevoked);
                    break;
                }
                continue;
            }
            received = receive_for(stream, cancel_rx.clone(), Duration::from_secs(60)) => received,
        };
        let message = match received {
            Receive::Message(message) => message,
            Receive::Ping(payload) => {
                if !queue(outbound, cancel_tx, Outbound::Pong(payload)).await {
                    break;
                }
                continue;
            }
            Receive::Ignore => continue,
            Receive::Invalid => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::InvalidMessage,
                    "invalid protocol message",
                )
                .await;
                break;
            }
            Receive::Closed => break,
        };
        match message {
            ClientMessage::Ping { nonce } => {
                if !queue(
                    outbound,
                    cancel_tx,
                    Outbound::Message(ServerMessage::Pong { nonce }),
                )
                .await
                {
                    break;
                }
            }
            ClientMessage::Command {
                request_id,
                command,
            } => {
                if request_ids.contains(&request_id) {
                    if !queue(
                        outbound,
                        cancel_tx,
                        Outbound::Message(ServerMessage::Error {
                            request_id: Some(request_id),
                            code: ErrorCode::InvalidCommand,
                            message: "duplicate request ID".to_owned(),
                            fatal: false,
                        }),
                    )
                    .await
                    {
                        break;
                    }
                    continue;
                }
                match world.submit(actor, request_id.clone(), command) {
                    Ok(()) => request_ids.record(request_id),
                    Err(SubmitError::Full) => {
                        if !queue(
                            outbound,
                            cancel_tx,
                            Outbound::Message(ServerMessage::Error {
                                request_id: Some(request_id),
                                code: ErrorCode::Overloaded,
                                message: "server is busy".to_owned(),
                                fatal: false,
                            }),
                        )
                        .await
                        {
                            break;
                        }
                    }
                    Err(SubmitError::Closed) => {
                        fatal(
                            outbound,
                            cancel_tx,
                            ErrorCode::InternalError,
                            "internal server error",
                        )
                        .await;
                        break;
                    }
                }
            }
            ClientMessage::Hello { .. } | ClientMessage::Attach { .. } => {
                fatal(
                    outbound,
                    cancel_tx,
                    ErrorCode::ProtocolState,
                    "unexpected handshake message",
                )
                .await;
                break;
            }
        }
    }
}

fn session_matches(auth: &auth::AuthService, principal: &auth::Principal, token: &str) -> bool {
    auth.lookup(token)
        .is_some_and(|current| current == *principal)
}

enum Receive {
    Message(ClientMessage),
    Ping(Vec<u8>),
    Ignore,
    Invalid,
    Closed,
}

async fn receive_for(
    stream: &mut SplitStream<WebSocket>,
    cancel: watch::Receiver<Option<DisconnectReason>>,
    duration: Duration,
) -> Receive {
    receive_until(stream, cancel, TokioInstant::now() + duration).await
}

async fn receive_until(
    stream: &mut SplitStream<WebSocket>,
    mut cancel: watch::Receiver<Option<DisconnectReason>>,
    deadline: TokioInstant,
) -> Receive {
    tokio::select! {
        biased;
        changed = cancel.changed() => {
            let _ = changed;
            Receive::Closed
        }
        frame = timeout_at(deadline, stream.next()) => {
            match frame {
                Ok(Some(Ok(Message::Text(text)))) => match decode_client(text.as_str()) {
                    Ok(message) => Receive::Message(message),
                    Err(_) => Receive::Invalid,
                },
                Ok(Some(Ok(Message::Ping(payload)))) => Receive::Ping(payload.to_vec()),
                Ok(Some(Ok(Message::Pong(_)))) => Receive::Ignore,
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Err(_) => Receive::Closed,
                Ok(Some(Ok(Message::Binary(_)))) | Ok(Some(Err(_))) => Receive::Invalid,
            }
        }
    }
}

async fn queue(
    outbound: &mpsc::Sender<Outbound>,
    cancel: &watch::Sender<Option<DisconnectReason>>,
    message: Outbound,
) -> bool {
    match timeout(QUEUE_TIMEOUT, outbound.send(message)).await {
        Ok(Ok(())) => true,
        _ => {
            cancel_first(cancel, DisconnectReason::SlowConsumer);
            false
        }
    }
}

async fn fatal(
    _outbound: &mpsc::Sender<Outbound>,
    cancel: &watch::Sender<Option<DisconnectReason>>,
    code: ErrorCode,
    _message: &'static str,
) {
    cancel_first(cancel, DisconnectReason::Protocol(code));
}

async fn writer_task(
    mut sink: SplitSink<WebSocket, Message>,
    mut receiver: mpsc::Receiver<Outbound>,
    epoch: String,
    mut cancel: watch::Receiver<Option<DisconnectReason>>,
    cancel_tx: watch::Sender<Option<DisconnectReason>>,
) {
    let mut sequence = 1u64;
    loop {
        tokio::select! {
            biased;
            changed = cancel.changed() => {
                let _ = changed;
                let reason = *cancel.borrow();
                if let Some(reason) = reason {
                    write_cancellation(&mut sink, &epoch, &mut sequence, reason).await;
                }
                return;
            }
            message = receiver.recv() => {
                let Some(message) = message else { return; };
                match message {
                    Outbound::Pong(payload) => {
                        if !write_message(&mut sink, Message::Pong(payload.into())).await {
                            cancel_first(&cancel_tx, DisconnectReason::Internal);
                            return;
                        }
                    }
                    Outbound::Message(message) => {
                        let envelope = ServerEnvelope {
                            protocol_version: PROTOCOL_VERSION,
                            stream_epoch: epoch.clone(),
                            sequence,
                            message,
                        };
                        let Ok(frame) = encode_server(&envelope) else {
                            cancel_first(&cancel_tx, DisconnectReason::Internal);
                            return;
                        };
                        if !write_message(&mut sink, Message::Text(frame.into())).await {
                            cancel_first(&cancel_tx, DisconnectReason::Internal);
                            return;
                        }
                        let Some(next) = sequence.checked_add(1) else {
                            cancel_first(&cancel_tx, DisconnectReason::Internal);
                            return;
                        };
                        sequence = next;
                    }
                }
            }
        }
    }
}

async fn write_cancellation(
    sink: &mut SplitSink<WebSocket, Message>,
    epoch: &str,
    sequence: &mut u64,
    reason: DisconnectReason,
) {
    let Some((code, message)) = cancellation_error(reason) else {
        let _ = write_message(sink, Message::Close(None)).await;
        return;
    };
    let envelope = ServerEnvelope {
        protocol_version: PROTOCOL_VERSION,
        stream_epoch: epoch.to_owned(),
        sequence: *sequence,
        message: ServerMessage::Error {
            request_id: None,
            code,
            message: message.to_owned(),
            fatal: true,
        },
    };
    if let Ok(frame) = encode_server(&envelope) {
        if !write_message(sink, Message::Text(frame.into())).await {
            return;
        }
        if let Some(next) = sequence.checked_add(1) {
            *sequence = next;
        }
    }
    let _ = write_message(sink, Message::Close(None)).await;
}

fn cancellation_error(reason: DisconnectReason) -> Option<(ErrorCode, &'static str)> {
    match reason {
        DisconnectReason::SessionReplaced => Some((ErrorCode::SessionReplaced, "session replaced")),
        DisconnectReason::SessionRevoked => {
            Some((ErrorCode::Unauthenticated, "session expired or revoked"))
        }
        DisconnectReason::SlowConsumer => Some((ErrorCode::Overloaded, "server is busy")),
        DisconnectReason::Internal => Some((ErrorCode::InternalError, "internal server error")),
        DisconnectReason::Protocol(code) => Some((
            code,
            match code {
                ErrorCode::InvalidMessage => "invalid protocol message",
                ErrorCode::UnsupportedVersion => "unsupported protocol version",
                ErrorCode::ProtocolState => "protocol state violation",
                ErrorCode::StaleAttachment => "stale attachment",
                ErrorCode::Unauthenticated => "session expired or revoked",
                ErrorCode::Unauthorized => "not authorized",
                ErrorCode::InternalError => "internal server error",
                ErrorCode::SessionReplaced => "session replaced",
                ErrorCode::Overloaded => "server is busy",
                ErrorCode::InvalidCommand => "invalid command",
                ErrorCode::NotFound => "not found",
            },
        )),
        DisconnectReason::Normal | DisconnectReason::Shutdown => None,
    }
}

async fn write_message(sink: &mut SplitSink<WebSocket, Message>, message: Message) -> bool {
    matches!(timeout(WRITE_TIMEOUT, sink.send(message)).await, Ok(Ok(())))
}

#[derive(Default)]
struct RecentRequestIds {
    values: std::collections::VecDeque<String>,
    set: HashSet<String>,
}

impl RecentRequestIds {
    fn contains(&self, value: &str) -> bool {
        self.set.contains(value)
    }

    fn record(&mut self, value: String) {
        if !self.set.insert(value.clone()) {
            return;
        }
        self.values.push_back(value);
        if self.values.len() > 256 {
            if let Some(oldest) = self.values.pop_front() {
                self.set.remove(&oldest);
            }
        }
    }
}

fn random_connection_id() -> Result<ConnectionId, ()> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ())?;
        if let Ok(id) = ConnectionId::from_u128(u128::from_be_bytes(bytes)) {
            return Ok(id);
        }
    }
    Err(())
}

fn random_epoch() -> Result<String, ()> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| ())?;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_request_ids_are_bounded_and_retryable() {
        let mut values = RecentRequestIds::default();
        assert!(!values.contains("a"));
        values.record("a".to_owned());
        assert!(values.contains("a"));
        values.record("a".to_owned());
        for index in 0..256 {
            values.record(format!("r{index}"));
        }
        assert_eq!(values.values.len(), 256);
        assert!(!values.contains("a"));
    }

    #[test]
    fn browser_assets_are_csp_compatible_and_bounded() {
        assert!(!INDEX.contains("<script>"));
        assert!(!INDEX.contains("<style>"));
        assert!(!APP_JS.contains("innerHTML"));
        assert!(!APP_JS.contains("?token"));
        assert!(APP_JS.contains("childElementCount > 500"));
        assert!(APP_JS.contains("credentials: \"same-origin\""));
        assert!(APP_JS.contains("expectedSequence"));
        assert!(APP_JS.contains("stream_epoch"));
        assert!(APP_JS.contains("/^[0-9a-f]{32}$/"));
        assert!(APP_JS.contains("case \"accepted\":"));
        assert!(APP_JS.contains("heartbeatTimers"));
        assert!(INDEX.contains("id=\"command-submit\" type=\"submit\" disabled"));
    }
}
