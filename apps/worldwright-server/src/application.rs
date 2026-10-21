use crate::{ServerConfig, ServerError, StorageConfig};
use std::sync::Arc;
use std::{future::Future, net::SocketAddr};
use tokio::net::TcpListener;

pub struct BoundServer {
    listener: TcpListener,
    router: axum::Router,
    world: crate::world::WorldHandle,
    allowed_origin: String,
}

impl BoundServer {
    pub async fn bind(config: ServerConfig) -> Result<Self, ServerError> {
        if !config.bind.ip().is_loopback() {
            return Err(ServerError::Configuration("bind address must be loopback"));
        }
        crate::cli::validate_origin(&config.allowed_origin, config.allow_insecure_http)
            .map_err(|_| ServerError::Configuration("origin is not valid for this mode"))?;
        let content = worldwright_content::load_file(&config.content_path, Default::default())
            .map_err(|_| ServerError::startup("world content failed validation"))?;
        let world_id = worldwright_core::ids::WorldId::from_u128(1)
            .map_err(|_| ServerError::startup("world identity failed"))?;
        let (auth, registrations, durable) = match config.storage {
            StorageConfig::Local { accounts_path } => {
                let auth = crate::auth::AuthService::load_local(
                    &accounts_path,
                    !config.allow_insecure_http,
                )?;
                let registrations = auth.registrations();
                (auth, registrations, None)
            }
            StorageConfig::Postgres { database_url } => {
                let store = worldwright_persistence::PostgresStore::connect(&database_url)
                    .await
                    .map_err(|_| ServerError::startup("durable storage connection failed"))?;
                store
                    .verify_migrations()
                    .await
                    .map_err(|_| ServerError::startup("durable migrations are not current"))?;
                store
                    .verify_world(world_id, content.namespace(), content.schema_version())
                    .await
                    .map_err(|_| ServerError::startup("durable world does not match content"))?;
                let writer = random_writer_id()
                    .ok_or_else(|| ServerError::startup("writer identity generation failed"))?;
                let lease = store
                    .acquire_writer(world_id, writer)
                    .await
                    .map_err(|_| ServerError::startup("writer acquisition failed"))?;
                let item_definition = format!("{}:items.welcome_token", content.namespace());
                let durable = crate::world::DurableContext {
                    store: Arc::new(store.clone()),
                    lease,
                    world: world_id,
                    item_definition,
                };
                let (auth, registrations) = crate::auth::AuthService::postgres(
                    store,
                    world_id,
                    !config.allow_insecure_http,
                )
                .await?;
                (auth, registrations, Some(durable))
            }
        };
        let world = crate::world::WorldHandle::new_with_durable(content, registrations, durable)?;
        let listener = match TcpListener::bind(config.bind).await {
            Ok(listener) => listener,
            Err(error) => {
                world.shutdown().await;
                return Err(ServerError::io(error));
            }
        };
        let router = crate::web::router(crate::web::AppState {
            allowed_origin: config.allowed_origin.clone(),
            auth,
            world: world.clone(),
        });
        Ok(Self {
            listener,
            router,
            world,
            allowed_origin: config.allowed_origin,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.listener
            .local_addr()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], 0)))
    }

    pub fn router(&self) -> axum::Router {
        self.router.clone()
    }

    pub fn allowed_origin(&self) -> &str {
        &self.allowed_origin
    }
}

pub(crate) async fn serve<F>(server: BoundServer, shutdown: F) -> Result<(), ServerError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let world_for_shutdown = server.world.clone();
    let graceful = async move {
        shutdown.await;
        world_for_shutdown.shutdown().await;
    };
    let result = axum::serve(server.listener, server.router)
        .with_graceful_shutdown(graceful)
        .await
        .map_err(ServerError::io);
    server.world.shutdown().await;
    result
}

fn random_writer_id() -> Option<worldwright_core::ids::WriterId> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).ok()?;
        if let Ok(id) = worldwright_core::ids::WriterId::from_u128(u128::from_be_bytes(bytes)) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{HeaderValue, Request, StatusCode},
    };
    use futures_util::{SinkExt, StreamExt};
    use http_body_util::BodyExt;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant as StdInstant},
    };
    use tokio::{
        net::TcpStream,
        sync::oneshot,
        task::JoinHandle,
        time::{interval, timeout},
    };
    use tokio_tungstenite::{
        WebSocketStream, connect_async,
        tungstenite::{
            Message,
            client::IntoClientRequest,
            error::Error as WsError,
            http::{Request as WsRequest, header},
        },
    };
    use tower::ServiceExt;
    use worldwright_protocol::{
        ClientCommand, ClientMessage, ErrorCode, OutputKind, ServerEnvelope, ServerMessage,
    };

    static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn fixture_paths() -> (PathBuf, PathBuf) {
        let prefix = format!(
            "worldwright-server-app-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir();
        (
            directory.join(format!("{prefix}-world.json")),
            directory.join(format!("{prefix}-accounts.json")),
        )
    }

    #[tokio::test]
    async fn bound_router_serves_static_security_headers_and_authenticates_login() {
        let (content_path, accounts_path) = fixture_paths();
        let cleanup_content = content_path.clone();
        let cleanup_accounts = accounts_path.clone();
        std::fs::write(
            &content_path,
            include_bytes!("../../../examples/two-room/world.json"),
        )
        .expect("world");
        let hash = crate::auth::make_hash(b"password").expect("hash");
        let accounts = serde_json::json!({
            "schema_version": 1,
            "accounts": [{
                "username": "alice",
                "password_hash": hash,
                "account_id": "00000000000000000000000000000001",
                "character_id": "00000000000000000000000000000001",
                "character_name": "Alice"
            }]
        });
        std::fs::write(
            &accounts_path,
            serde_json::to_vec(&accounts).expect("accounts"),
        )
        .expect("accounts");
        let server = BoundServer::bind(ServerConfig {
            bind: "127.0.0.1:0".parse().expect("bind"),
            allowed_origin: "http://localhost".to_owned(),
            content_path,
            storage: crate::StorageConfig::Local { accounts_path },
            allow_insecure_http: true,
        })
        .await
        .expect("bind");
        assert_ne!(server.local_addr().port(), 0);
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .uri("/")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        assert_eq!(response.headers()["cache-control"], "no-store");
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session")
                    .header("origin", "https://wrong.example")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"alice","password":"password"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = server
            .router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session")
                    .header("origin", "http://localhost")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"alice","password":"password"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response.headers()["set-cookie"]
                .to_str()
                .expect("cookie")
                .contains("HttpOnly")
        );
        server.world.shutdown().await;
        let _ = std::fs::remove_file(cleanup_content);
        let _ = std::fs::remove_file(cleanup_accounts);
    }

    struct NetworkFixture {
        router: Router,
        address: SocketAddr,
        shutdown: Option<oneshot::Sender<()>>,
        task: JoinHandle<Result<(), ServerError>>,
        content_path: PathBuf,
        accounts_path: PathBuf,
    }

    impl NetworkFixture {
        async fn start(accounts: &[(&str, &str, u128, u128, &str)]) -> Self {
            let (content_path, accounts_path) = fixture_paths();
            let records = accounts
                .iter()
                .map(|(username, password, account, character, name)| {
                    serde_json::json!({
                        "username": username,
                        "password_hash": crate::auth::make_hash(password.as_bytes()).expect("hash"),
                        "account_id": format!("{account:032x}"),
                        "character_id": format!("{character:032x}"),
                        "character_name": name,
                    })
                })
                .collect::<Vec<_>>();
            let registry = serde_json::json!({"schema_version": 1, "accounts": records});
            std::fs::write(
                &content_path,
                include_bytes!("../../../examples/two-room/world.json"),
            )
            .expect("world fixture");
            std::fs::write(
                &accounts_path,
                serde_json::to_vec(&registry).expect("registry"),
            )
            .expect("account fixture");
            let server = BoundServer::bind(ServerConfig {
                bind: "127.0.0.1:0".parse().expect("bind"),
                allowed_origin: "http://localhost".to_owned(),
                content_path: content_path.clone(),
                storage: StorageConfig::Local {
                    accounts_path: accounts_path.clone(),
                },
                allow_insecure_http: true,
            })
            .await
            .expect("bind");
            let address = server.local_addr();
            let router = server.router();
            let (shutdown, receiver) = oneshot::channel();
            let task = tokio::spawn(server.serve(async move {
                let _ = receiver.await;
            }));
            Self {
                router,
                address,
                shutdown: Some(shutdown),
                task,
                content_path,
                accounts_path,
            }
        }

        async fn shutdown(mut self) {
            if let Some(shutdown) = self.shutdown.take() {
                let _ = shutdown.send(());
            }
            self.task.await.expect("serve task").expect("serve");
            let _ = std::fs::remove_file(self.content_path);
            let _ = std::fs::remove_file(self.accounts_path);
        }
    }

    type Socket = WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

    struct StreamState {
        epoch: Option<String>,
        next_sequence: u64,
    }

    impl StreamState {
        fn new() -> Self {
            Self {
                epoch: None,
                next_sequence: 1,
            }
        }
    }

    async fn login(router: &Router, username: &str, password: &str) -> String {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/session")
                    .header("origin", "http://localhost")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"username": username, "password": password}).to_string(),
                    ))
                    .expect("login request"),
            )
            .await
            .expect("login response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let cookie_header = response.headers()[header::SET_COOKIE]
            .to_str()
            .expect("cookie")
            .to_owned();
        assert!(cookie_header.contains("HttpOnly"));
        assert!(cookie_header.contains("SameSite=Strict"));
        assert!(cookie_header.contains("Max-Age=43200"));
        let cookie = cookie_header
            .split(';')
            .next()
            .expect("cookie pair")
            .to_owned();
        let _ = response.into_body().collect().await.expect("body");
        cookie
    }

    fn websocket_request(address: SocketAddr, cookie: Option<&str>, origin: &str) -> WsRequest<()> {
        let mut request = format!("ws://{address}/api/v1/play")
            .into_client_request()
            .expect("WebSocket request");
        request.headers_mut().insert(
            header::ORIGIN,
            HeaderValue::from_str(origin).expect("origin"),
        );
        if let Some(cookie) = cookie {
            request.headers_mut().insert(
                header::COOKIE,
                HeaderValue::from_str(cookie).expect("cookie"),
            );
        }
        request
    }

    async fn connect(fixture: &NetworkFixture, cookie: &str) -> Socket {
        connect_async(websocket_request(
            fixture.address,
            Some(cookie),
            "http://localhost",
        ))
        .await
        .expect("WebSocket connect")
        .0
    }

    async fn send_client(socket: &mut Socket, message: ClientMessage) {
        let text = serde_json::to_string(&message).expect("client JSON");
        timeout(
            Duration::from_secs(2),
            socket.send(Message::Text(text.into())),
        )
        .await
        .expect("send timeout")
        .expect("send");
    }

    async fn receive_raw(socket: &mut Socket) -> Message {
        timeout(Duration::from_secs(3), socket.next())
            .await
            .expect("receive timeout")
            .expect("socket ended")
            .expect("socket error")
    }

    async fn receive_envelope(socket: &mut Socket, state: &mut StreamState) -> ServerMessage {
        let message = receive_raw(socket).await;
        let Message::Text(text) = message else {
            panic!("expected text envelope");
        };
        let envelope: ServerEnvelope =
            serde_json::from_str(text.as_str()).expect("server envelope");
        assert_eq!(envelope.protocol_version, 1);
        assert!(
            envelope.stream_epoch.len() == 32
                && envelope
                    .stream_epoch
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        if let Some(epoch) = &state.epoch {
            assert_eq!(epoch, &envelope.stream_epoch);
        } else {
            state.epoch = Some(envelope.stream_epoch.clone());
        }
        assert_eq!(envelope.sequence, state.next_sequence);
        state.next_sequence += 1;
        envelope.message
    }

    async fn expect_initial(socket: &mut Socket, state: &mut StreamState, name: &str) {
        send_client(
            socket,
            ClientMessage::Hello {
                supported_versions: vec![1],
            },
        )
        .await;
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Welcome {
                heartbeat_interval_ms: 20_000
            }
        ));
        send_client(
            socket,
            ClientMessage::Attach {
                request_id: format!("attach-{name}"),
            },
        )
        .await;
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Accepted { .. }
        ));
        assert!(
            matches!(receive_envelope(socket, state).await, ServerMessage::Attached { character_name, .. } if character_name == name)
        );
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Exits { .. }
        ));
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::CommandResult { .. }
        ));
    }

    async fn expect_move(socket: &mut Socket, state: &mut StreamState, request_id: &str) {
        assert!(
            matches!(receive_envelope(socket, state).await, ServerMessage::Accepted { request_id: received } if received == request_id)
        );
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Output {
                kind: OutputKind::Movement,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(socket, state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(
            matches!(receive_envelope(socket, state).await, ServerMessage::Exits { exits, .. } if exits == vec!["south"])
        );
        assert!(
            matches!(receive_envelope(socket, state).await, ServerMessage::CommandResult { request_id: received, .. } if received == request_id)
        );
    }

    async fn expect_upgrade_status(request: WsRequest<()>, status: StatusCode) {
        match connect_async(request).await {
            Err(WsError::Http(response)) => assert_eq!(response.status(), status),
            other => panic!("unexpected upgrade result: {other:?}"),
        }
    }

    #[tokio::test]
    async fn network_two_authenticated_clients_interact() {
        let fixture = NetworkFixture::start(&[
            ("alice", "alice-password", 1, 1, "Alice"),
            ("bob", "bob-password", 2, 2, "Bob"),
        ])
        .await;
        let alice_cookie = login(&fixture.router, "alice", "alice-password").await;
        let bob_cookie = login(&fixture.router, "bob", "bob-password").await;
        let mut alice = connect(&fixture, &alice_cookie).await;
        let mut bob = connect(&fixture, &bob_cookie).await;
        let mut alice_state = StreamState::new();
        let mut bob_state = StreamState::new();
        expect_initial(&mut alice, &mut alice_state, "Alice").await;
        expect_initial(&mut bob, &mut bob_state, "Bob").await;
        send_client(
            &mut bob,
            ClientMessage::Command {
                request_id: "bob-move".to_owned(),
                command: ClientCommand::Move {
                    exit: "north".to_owned(),
                },
            },
        )
        .await;
        expect_move(&mut bob, &mut bob_state, "bob-move").await;
        send_client(
            &mut alice,
            ClientMessage::Command {
                request_id: "alice-move".to_owned(),
                command: ClientCommand::Move {
                    exit: "north".to_owned(),
                },
            },
        )
        .await;
        expect_move(&mut alice, &mut alice_state, "alice-move").await;
        send_client(
            &mut alice,
            ClientMessage::Command {
                request_id: "alice-say".to_owned(),
                command: ClientCommand::Say {
                    text: "hello Bob".to_owned(),
                },
            },
        )
        .await;
        assert!(
            matches!(receive_envelope(&mut alice, &mut alice_state).await, ServerMessage::Accepted { request_id } if request_id == "alice-say")
        );
        assert!(
            matches!(receive_envelope(&mut alice, &mut alice_state).await, ServerMessage::Output { kind: OutputKind::Speech, speaker: Some(speaker), request_id: Some(request_id), text } if speaker == "Alice" && request_id == "alice-say" && text == "hello Bob")
        );
        assert!(
            matches!(receive_envelope(&mut alice, &mut alice_state).await, ServerMessage::CommandResult { request_id, .. } if request_id == "alice-say")
        );
        assert!(
            matches!(receive_envelope(&mut bob, &mut bob_state).await, ServerMessage::Output { kind: OutputKind::Speech, speaker: Some(speaker), request_id: None, text } if speaker == "Alice" && text == "hello Bob")
        );
        timeout(
            Duration::from_secs(2),
            alice.send(Message::Ping(vec![7, 8].into())),
        )
        .await
        .expect("ping timeout")
        .expect("ping");
        assert!(
            matches!(receive_raw(&mut alice).await, Message::Pong(payload) if payload.as_ref() == [7, 8])
        );
        timeout(Duration::from_secs(2), alice.close(None))
            .await
            .expect("close timeout")
            .expect("close");
        timeout(Duration::from_secs(2), bob.close(None))
            .await
            .expect("close timeout")
            .expect("close");
        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn network_auth_origin_and_logout_boundaries() {
        let fixture = NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let alice_cookie = login(&fixture.router, "alice", "alice-password").await;
        expect_upgrade_status(
            websocket_request(fixture.address, None, "http://localhost"),
            StatusCode::UNAUTHORIZED,
        )
        .await;
        expect_upgrade_status(
            websocket_request(fixture.address, Some(&alice_cookie), "http://wrong.example"),
            StatusCode::FORBIDDEN,
        )
        .await;
        let mut socket = connect(&fixture, &alice_cookie).await;
        let mut state = StreamState::new();
        expect_initial(&mut socket, &mut state, "Alice").await;
        let response = fixture
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/session")
                    .header("origin", "http://localhost")
                    .header("cookie", &alice_cookie)
                    .body(Body::empty())
                    .expect("logout request"),
            )
            .await
            .expect("logout response");
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(
            response.headers()[header::SET_COOKIE]
                .to_str()
                .expect("expired cookie")
                .contains("Max-Age=0")
        );
        let message = receive_envelope(&mut socket, &mut state).await;
        assert!(matches!(
            message,
            ServerMessage::Error {
                code: ErrorCode::Unauthenticated,
                fatal: true,
                ..
            }
        ));
        assert!(matches!(receive_raw(&mut socket).await, Message::Close(_)));
        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn network_takeover_replaces_old_connection_without_staling_new_one() {
        let fixture = NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let first_cookie = login(&fixture.router, "alice", "alice-password").await;
        let second_cookie = login(&fixture.router, "alice", "alice-password").await;
        assert_ne!(first_cookie, second_cookie);
        let mut first = connect(&fixture, &first_cookie).await;
        let mut second = connect(&fixture, &second_cookie).await;
        let mut first_state = StreamState::new();
        let mut second_state = StreamState::new();
        expect_initial(&mut first, &mut first_state, "Alice").await;
        expect_initial(&mut second, &mut second_state, "Alice").await;
        let message = receive_envelope(&mut first, &mut first_state).await;
        assert!(matches!(
            message,
            ServerMessage::Error {
                code: ErrorCode::SessionReplaced,
                fatal: true,
                ..
            }
        ));
        assert!(matches!(receive_raw(&mut first).await, Message::Close(_)));
        send_client(
            &mut second,
            ClientMessage::Command {
                request_id: "replacement-look".to_owned(),
                command: ClientCommand::Look,
            },
        )
        .await;
        assert!(
            matches!(receive_envelope(&mut second, &mut second_state).await, ServerMessage::Accepted { request_id } if request_id == "replacement-look")
        );
        assert!(matches!(
            receive_envelope(&mut second, &mut second_state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(&mut second, &mut second_state).await,
            ServerMessage::Output {
                kind: OutputKind::Room,
                ..
            }
        ));
        assert!(matches!(
            receive_envelope(&mut second, &mut second_state).await,
            ServerMessage::Exits { .. }
        ));
        assert!(
            matches!(receive_envelope(&mut second, &mut second_state).await, ServerMessage::CommandResult { request_id, .. } if request_id == "replacement-look")
        );
        timeout(Duration::from_secs(2), second.close(None))
            .await
            .expect("close timeout")
            .expect("close");
        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_closes_an_open_socket_and_finishes_serving() {
        let mut fixture =
            NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let cookie = login(&fixture.router, "alice", "alice-password").await;
        let mut socket = connect(&fixture, &cookie).await;
        let mut state = StreamState::new();
        expect_initial(&mut socket, &mut state, "Alice").await;

        let shutdown = fixture.shutdown.take().expect("shutdown sender");
        shutdown.send(()).expect("shutdown signal");
        assert!(matches!(receive_raw(&mut socket).await, Message::Close(_)));
        timeout(Duration::from_secs(3), fixture.task)
            .await
            .expect("serve completion timeout")
            .expect("serve task")
            .expect("serve");
        let _ = std::fs::remove_file(fixture.content_path);
        let _ = std::fs::remove_file(fixture.accounts_path);
    }

    #[tokio::test]
    async fn unsupported_versions_are_fatal_at_sequence_one() {
        let fixture = NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let cookie = login(&fixture.router, "alice", "alice-password").await;
        let mut socket = connect(&fixture, &cookie).await;
        let mut state = StreamState::new();
        send_client(
            &mut socket,
            ClientMessage::Hello {
                supported_versions: vec![99],
            },
        )
        .await;
        assert!(matches!(
            receive_envelope(&mut socket, &mut state).await,
            ServerMessage::Error {
                code: ErrorCode::UnsupportedVersion,
                fatal: true,
                ..
            }
        ));
        assert!(matches!(receive_raw(&mut socket).await, Message::Close(_)));
        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn binary_before_hello_is_fatal_at_sequence_one() {
        let fixture = NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let cookie = login(&fixture.router, "alice", "alice-password").await;
        let mut socket = connect(&fixture, &cookie).await;
        let mut state = StreamState::new();
        timeout(
            Duration::from_secs(2),
            socket.send(Message::Binary(vec![1, 2, 3].into())),
        )
        .await
        .expect("binary send timeout")
        .expect("binary send");
        assert!(matches!(
            receive_envelope(&mut socket, &mut state).await,
            ServerMessage::Error {
                code: ErrorCode::InvalidMessage,
                fatal: true,
                ..
            }
        ));
        assert!(matches!(receive_raw(&mut socket).await, Message::Close(_)));
        fixture.shutdown().await;
    }

    #[tokio::test]
    async fn control_pings_cannot_extend_hello_deadline() {
        let fixture = NetworkFixture::start(&[("alice", "alice-password", 1, 1, "Alice")]).await;
        let cookie = login(&fixture.router, "alice", "alice-password").await;
        let mut socket = connect(&fixture, &cookie).await;
        let started = StdInstant::now();
        let mut ticks = interval(Duration::from_millis(250));
        ticks.tick().await;
        let mut pongs = 0usize;
        let closed = timeout(Duration::from_secs(8), async {
            loop {
                tokio::select! {
                    _ = ticks.tick() => {
                        socket.send(Message::Ping(vec![7].into())).await.expect("ping");
                    }
                    message = socket.next() => {
                        match message {
                            Some(Ok(Message::Pong(payload))) => {
                                assert_eq!(payload.as_ref(), [7]);
                                pongs += 1;
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                assert!(started.elapsed() >= Duration::from_secs(4));
                                break;
                            }
                            Some(Err(error)) => {
                                assert!(started.elapsed() >= Duration::from_secs(4), "early socket error: {error}");
                                break;
                            }
                            Some(Ok(other)) => panic!("unexpected pre-hello frame: {other:?}"),
                        }
                    }
                }
            }
        })
        .await;
        assert!(closed.is_ok());
        assert!(pongs > 0);
        assert!(started.elapsed() < Duration::from_secs(7));
        fixture.shutdown().await;
    }
}
