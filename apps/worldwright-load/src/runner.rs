use crate::{
    cli::RunOptions,
    fixture::{Credential, load_credentials},
    report::{FailureCounts, LoadReport, evaluate, latency_stats},
};
use futures_util::{SinkExt, StreamExt, future::join_all, stream::FuturesUnordered};
use reqwest::{
    Client, StatusCode, Url,
    header::{ORIGIN, SET_COOKIE},
};
use std::{
    fmt::Write as _,
    fs::OpenOptions,
    io::Write,
    process::ExitCode,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::TcpStream,
    sync::{Semaphore, mpsc, watch},
    time::{Instant, MissedTickBehavior, interval_at, sleep, sleep_until, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderValue, header::COOKIE},
    },
};
use worldwright_protocol::{
    ClientCommand, ClientMessage, PROTOCOL_VERSION, ServerEnvelope, ServerMessage, decode_server,
    encode_client,
};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;
const LOGIN_DELAYS_MS: [u64; 8] = [250, 500, 1_000, 2_000, 4_000, 4_000, 4_000, 4_000];

struct Endpoints {
    login: Url,
    ready: Url,
    websocket: String,
}

struct ConnectedClient {
    index: usize,
    socket: Socket,
    stream: StreamTracker,
}

#[derive(Default)]
struct StreamTracker {
    epoch: Option<String>,
    sequence: u64,
}

#[derive(Clone, Copy)]
enum ClientFailure {
    Login,
    Attach,
    Command,
    Protocol,
    Disconnect,
    Internal,
}

struct WorkResult {
    failure: Option<ClientFailure>,
}

pub(crate) async fn run(options: RunOptions) -> ExitCode {
    match run_inner(options).await {
        Ok(passed) if passed => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(message) => {
            eprintln!("worldwright-load: {message}");
            ExitCode::from(2)
        }
    }
}

async fn run_inner(options: RunOptions) -> Result<bool, String> {
    if options.output.exists() {
        return Err("report output must not exist".into());
    }
    validate_metadata(
        &options.server_revision,
        &options.artifact_digest,
        &options.environment_label,
    )?;
    let endpoints = endpoints(&options.base_url, &options.origin)?;
    let (target_clients, commands, period_ms) = options.profile.dimensions();
    let credentials = load_credentials(&options.credentials, target_clients)?;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| "HTTP client initialization failed")?;
    let run_nonce = random_nonce()?;
    let started_unix_seconds = unix_seconds()?;
    let slots = Arc::new(Semaphore::new(4));
    let connection_futures = credentials
        .into_iter()
        .enumerate()
        .map(|(index, credential)| {
            connect_client(
                index,
                credential,
                client.clone(),
                options.origin.clone(),
                endpoints.login.clone(),
                endpoints.websocket.clone(),
                slots.clone(),
                run_nonce.clone(),
            )
        });
    let connection_results = timeout(Duration::from_secs(120), join_all(connection_futures)).await;
    let mut failures = FailureCounts::default();
    let mut connected = Vec::with_capacity(target_clients);
    match connection_results {
        Ok(results) => {
            for result in results {
                match result {
                    Ok(client) => connected.push(client),
                    Err(failure) => record_failure(&mut failures, failure),
                }
            }
        }
        Err(_) => failures.attach = target_clients as u64,
    }
    connected.sort_by_key(|client| client.index);
    let connected_count = connected.len() as u64;
    let barrier = Instant::now() + Duration::from_millis(100);
    let mut latencies = Vec::new();
    let mut work = Vec::new();
    let (readiness_checks, readiness_failures, elapsed_ms) = if connected.len() == target_clients {
        let (readiness_stop, readiness_rx) = watch::channel(false);
        let readiness = tokio::spawn(poll_readiness(
            client.clone(),
            endpoints.ready,
            barrier,
            readiness_rx,
        ));
        let (latency_tx, mut latency_rx) = mpsc::unbounded_channel();
        let mut futures = connected
            .into_iter()
            .map(|client| {
                run_client(
                    client,
                    barrier,
                    target_clients,
                    commands,
                    period_ms,
                    run_nonce.clone(),
                    latency_tx.clone(),
                )
            })
            .collect::<FuturesUnordered<_>>();
        drop(latency_tx);
        sleep_until(barrier).await;
        let deadline = barrier + Duration::from_secs(90);
        while !futures.is_empty() {
            tokio::select! {
                result = futures.next() => {
                    if let Some(result) = result {
                        work.push(result);
                    }
                }
                latency = latency_rx.recv() => {
                    if let Some(latency) = latency {
                        latencies.push(latency);
                    }
                }
                _ = sleep_until(deadline) => {
                    failures.command = failures
                        .command
                        .saturating_add(futures.len() as u64);
                    break;
                }
            }
        }
        while let Ok(latency) = latency_rx.try_recv() {
            latencies.push(latency);
        }
        let elapsed_ms = u64::try_from(barrier.elapsed().as_millis()).unwrap_or(u64::MAX);
        let _ = readiness_stop.send(true);
        let (checks, readiness_failures) = readiness.await.unwrap_or((0, 1));
        let expected_checks = elapsed_ms.div_ceil(1_000).max(1);
        (
            checks,
            readiness_failures.saturating_add(expected_checks.saturating_sub(checks)),
            elapsed_ms,
        )
    } else {
        (0, 0, 0)
    };
    failures.readiness = readiness_failures;
    for result in work {
        if let Some(failure) = result.failure {
            record_failure(&mut failures, failure);
        }
    }
    let completed_commands = latencies.len() as u64;
    let throughput = if elapsed_ms == 0 {
        0
    } else {
        completed_commands.saturating_mul(1_000_000) / elapsed_ms
    };
    let mut report = LoadReport {
        schema_version: 1,
        profile: options.profile.name().to_owned(),
        profile_version: 1,
        storage_expectation: "operator-supplied-postgresql",
        server_revision: options.server_revision,
        artifact_digest: options.artifact_digest,
        environment_label: options.environment_label,
        run_nonce,
        operating_system: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        started_unix_seconds,
        ended_unix_seconds: unix_seconds()?,
        target_clients: target_clients as u64,
        connected_clients: connected_count,
        expected_commands: (target_clients * commands) as u64,
        completed_commands,
        failures,
        readiness_checks,
        elapsed_ms,
        throughput_milli_commands_per_second: throughput,
        latency: latency_stats(latencies),
        passed: false,
        failure_reasons: Vec::new(),
    };
    evaluate(&mut report, options.profile);
    let bytes = serde_json::to_vec(&report).map_err(|_| "report serialization failed")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&options.output)
        .map_err(|_| "report create failed")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "report write failed")?;
    println!(
        "profile={} connected={} completed={} passed={}",
        report.profile, report.connected_clients, report.completed_commands, report.passed
    );
    Ok(report.passed)
}

fn validate_metadata(revision: &str, digest: &str, label: &str) -> Result<(), String> {
    for value in [revision, label] {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
        {
            return Err("evidence metadata is invalid".into());
        }
    }
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("artifact digest is invalid".into());
    }
    Ok(())
}

fn endpoints(base: &str, origin: &str) -> Result<Endpoints, String> {
    let parsed = Url::parse(base).map_err(|_| "base URL is invalid")?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.host_str().is_none()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("base URL must be a canonical HTTPS origin".into());
    }
    let canonical = parsed.as_str().trim_end_matches('/');
    if origin != canonical {
        return Err("origin must exactly match base URL".into());
    }
    let mut login = parsed.clone();
    login.set_path("/api/v1/session");
    let mut ready = parsed.clone();
    ready.set_path("/health/ready");
    let authority = &canonical["https://".len()..];
    Ok(Endpoints {
        login,
        ready,
        websocket: format!("wss://{authority}/api/v1/play"),
    })
}

fn random_nonce() -> Result<String, String> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|_| "run nonce generation failed")?;
    let mut output = String::with_capacity(16);
    for byte in bytes {
        write!(output, "{byte:02x}").map_err(|_| "run nonce encoding failed")?;
    }
    Ok(output)
}

fn unix_seconds() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| "system clock is invalid".into())
}

#[allow(clippy::too_many_arguments)]
async fn connect_client(
    index: usize,
    credential: Credential,
    http: Client,
    origin: String,
    login_url: Url,
    websocket_url: String,
    slots: Arc<Semaphore>,
    run_nonce: String,
) -> Result<ConnectedClient, ClientFailure> {
    let _permit = slots
        .acquire_owned()
        .await
        .map_err(|_| ClientFailure::Internal)?;
    let cookie = login(&http, login_url, &origin, &credential).await?;
    let mut request = websocket_url
        .into_client_request()
        .map_err(|_| ClientFailure::Internal)?;
    request.headers_mut().insert(
        tokio_tungstenite::tungstenite::http::header::ORIGIN,
        HeaderValue::from_str(&origin).map_err(|_| ClientFailure::Internal)?,
    );
    request.headers_mut().insert(
        COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| ClientFailure::Internal)?,
    );
    let (mut socket, response) = connect_async(request)
        .await
        .map_err(|_| ClientFailure::Attach)?;
    if response.status() != tokio_tungstenite::tungstenite::http::StatusCode::SWITCHING_PROTOCOLS {
        return Err(ClientFailure::Attach);
    }
    let mut stream = StreamTracker::default();
    send_client(
        &mut socket,
        &ClientMessage::Hello {
            supported_versions: vec![PROTOCOL_VERSION],
        },
    )
    .await?;
    match receive(&mut socket, &mut stream).await? {
        ServerMessage::Welcome { .. } => {}
        _ => return Err(ClientFailure::Protocol),
    }
    let request_id = format!("load-{run_nonce}-{index:03}-attach");
    send_client(
        &mut socket,
        &ClientMessage::Attach {
            request_id: request_id.clone(),
        },
    )
    .await?;
    let mut accepted = false;
    let mut attached = false;
    let mut result = false;
    while !(accepted && attached && result) {
        match receive(&mut socket, &mut stream).await? {
            ServerMessage::Accepted { request_id: id } if id == request_id => accepted = true,
            ServerMessage::Attached { request_id: id, .. } if id == request_id => attached = true,
            ServerMessage::CommandResult { request_id: id, .. } if id == request_id => {
                result = true
            }
            ServerMessage::Output {
                request_id: Some(id),
                ..
            }
            | ServerMessage::Exits {
                request_id: Some(id),
                ..
            } if id == request_id => {}
            ServerMessage::Error { .. } => return Err(ClientFailure::Attach),
            _ => return Err(ClientFailure::Protocol),
        }
    }
    Ok(ConnectedClient {
        index,
        socket,
        stream,
    })
}

async fn login(
    client: &Client,
    url: Url,
    origin: &str,
    credential: &Credential,
) -> Result<String, ClientFailure> {
    #[derive(serde::Serialize)]
    struct Login<'a> {
        username: &'a str,
        password: &'a str,
    }
    let mut delays = LOGIN_DELAYS_MS.into_iter();
    loop {
        let response = client
            .post(url.clone())
            .header(ORIGIN, origin)
            .json(&Login {
                username: &credential.username,
                password: &credential.password,
            })
            .send()
            .await
            .map_err(|_| ClientFailure::Login)?;
        if response.status() == StatusCode::NO_CONTENT {
            return session_cookie(response.headers());
        }
        if response.status() != StatusCode::SERVICE_UNAVAILABLE {
            return Err(ClientFailure::Login);
        }
        let delay = delays.next().ok_or(ClientFailure::Login)?;
        sleep(Duration::from_millis(delay)).await;
    }
}

fn session_cookie(headers: &reqwest::header::HeaderMap) -> Result<String, ClientFailure> {
    let mut token = None;
    for value in headers.get_all(SET_COOKIE) {
        let value = value.to_str().map_err(|_| ClientFailure::Login)?;
        let pair = value.split(';').next().ok_or(ClientFailure::Login)?;
        if let Some(candidate) = pair.strip_prefix("ww_session=") {
            if token.is_some()
                || candidate.len() != 64
                || !candidate
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
            {
                return Err(ClientFailure::Login);
            }
            token = Some(candidate.to_owned());
        }
    }
    token
        .map(|token| format!("ww_session={token}"))
        .ok_or(ClientFailure::Login)
}

async fn send_client(socket: &mut Socket, message: &ClientMessage) -> Result<(), ClientFailure> {
    let bytes = encode_client(message).map_err(|_| ClientFailure::Internal)?;
    let text = String::from_utf8(bytes).map_err(|_| ClientFailure::Internal)?;
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| ClientFailure::Disconnect)
}

async fn receive(
    socket: &mut Socket,
    stream: &mut StreamTracker,
) -> Result<ServerMessage, ClientFailure> {
    loop {
        let message = socket
            .next()
            .await
            .ok_or(ClientFailure::Disconnect)?
            .map_err(|_| ClientFailure::Disconnect)?;
        match message {
            Message::Text(text) => {
                let envelope =
                    decode_server(text.as_bytes()).map_err(|_| ClientFailure::Protocol)?;
                stream.accept(&envelope)?;
                return Ok(envelope.message);
            }
            Message::Ping(payload) => socket
                .send(Message::Pong(payload))
                .await
                .map_err(|_| ClientFailure::Disconnect)?,
            Message::Pong(_) => {}
            Message::Close(_) => return Err(ClientFailure::Disconnect),
            Message::Binary(_) | Message::Frame(_) => return Err(ClientFailure::Protocol),
        }
    }
}

impl StreamTracker {
    fn accept(&mut self, envelope: &ServerEnvelope) -> Result<(), ClientFailure> {
        match &self.epoch {
            None if envelope.sequence == 1 => self.epoch = Some(envelope.stream_epoch.clone()),
            Some(epoch)
                if epoch == &envelope.stream_epoch
                    && envelope.sequence == self.sequence.saturating_add(1) => {}
            _ => return Err(ClientFailure::Protocol),
        }
        self.sequence = envelope.sequence;
        Ok(())
    }
}

async fn run_client(
    mut client: ConnectedClient,
    barrier: Instant,
    total_clients: usize,
    commands: usize,
    period_ms: u64,
    run_nonce: String,
    latency_tx: mpsc::UnboundedSender<u64>,
) -> WorkResult {
    let offset_ms = (client.index as u64).saturating_mul(period_ms) / total_clients as u64;
    for command_index in 0..commands {
        let scheduled = barrier
            + Duration::from_millis(
                offset_ms.saturating_add((command_index as u64).saturating_mul(period_ms)),
            );
        sleep_until(scheduled).await;
        let request_id = format!("load-{run_nonce}-{:03}-{command_index:02}", client.index);
        let started = std::time::Instant::now();
        let outcome = timeout(Duration::from_secs(10), async {
            send_client(
                &mut client.socket,
                &ClientMessage::Command {
                    request_id: request_id.clone(),
                    command: ClientCommand::Look,
                },
            )
            .await?;
            let mut accepted = false;
            loop {
                match receive(&mut client.socket, &mut client.stream).await? {
                    ServerMessage::Accepted { request_id: id } if id == request_id => {
                        accepted = true
                    }
                    ServerMessage::CommandResult { request_id: id, .. }
                        if id == request_id && accepted =>
                    {
                        return Ok::<(), ClientFailure>(());
                    }
                    ServerMessage::Output {
                        request_id: Some(id),
                        ..
                    }
                    | ServerMessage::Exits {
                        request_id: Some(id),
                        ..
                    } if id == request_id => {}
                    ServerMessage::Error { .. } => return Err(ClientFailure::Command),
                    _ => return Err(ClientFailure::Protocol),
                }
            }
        })
        .await;
        match outcome {
            Ok(Ok(())) => {
                let latency = u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX);
                if latency_tx.send(latency).is_err() {
                    return WorkResult {
                        failure: Some(ClientFailure::Internal),
                    };
                }
            }
            Ok(Err(failure)) => {
                return WorkResult {
                    failure: Some(failure),
                };
            }
            Err(_) => {
                return WorkResult {
                    failure: Some(ClientFailure::Command),
                };
            }
        }
    }
    WorkResult { failure: None }
}

async fn poll_readiness(
    client: Client,
    url: Url,
    barrier: Instant,
    mut stop: watch::Receiver<bool>,
) -> (u64, u64) {
    let mut ticks = interval_at(barrier, Duration::from_secs(1));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut probes = FuturesUnordered::new();
    let mut checks = 0u64;
    let mut failures = 0u64;
    loop {
        tokio::select! {
            _ = ticks.tick() => {
                checks += 1;
                let client = client.clone();
                let url = url.clone();
                probes.push(async move {
                    matches!(
                        client.get(url).send().await,
                        Ok(response) if response.status() == StatusCode::OK
                    )
                });
            }
            result = probes.next(), if !probes.is_empty() => {
                if result == Some(false) {
                    failures += 1;
                }
            }
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() {
                    break;
                }
            }
        }
    }
    while let Some(success) = probes.next().await {
        if !success {
            failures += 1;
        }
    }
    (checks, failures)
}

fn record_failure(counts: &mut FailureCounts, failure: ClientFailure) {
    match failure {
        ClientFailure::Login => counts.login += 1,
        ClientFailure::Attach => counts.attach += 1,
        ClientFailure::Command => counts.command += 1,
        ClientFailure::Protocol => counts.protocol += 1,
        ClientFailure::Disconnect => counts.disconnect += 1,
        ClientFailure::Internal => counts.internal += 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn endpoints_require_one_exact_https_origin() {
        assert!(endpoints("https://world.example", "https://world.example").is_ok());
        assert!(endpoints("http://world.example", "http://world.example").is_err());
        assert!(endpoints("https://world.example/a", "https://world.example").is_err());
        assert!(endpoints("https://world.example", "https://other.example").is_err());
    }

    #[test]
    fn cookie_parser_rejects_missing_and_ambiguous_values() {
        let mut headers = HeaderMap::new();
        assert!(session_cookie(&headers).is_err());
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static(
                "ww_session=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa; HttpOnly",
            ),
        );
        assert!(session_cookie(&headers).is_ok());
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static(
                "ww_session=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            ),
        );
        assert!(session_cookie(&headers).is_err());
    }
}
