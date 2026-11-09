use crate::GameProfile;
use crate::{ServerConfig, ServerError, StorageConfig};
use std::sync::Arc;
use std::{future::Future, net::SocketAddr};
use tokio::net::TcpListener;
use tokio::time::{Duration, timeout};
use worldwright_release::{GameContract, ReleaseManifest};

fn profile_contract(profile: GameProfile) -> GameContract {
    match profile {
        GameProfile::Reference => GameContract::Reference,
        GameProfile::Expedition => GameContract::Expedition,
    }
}
fn validate_active_game(
    profile: GameProfile,
    manifest: &ReleaseManifest,
) -> Result<(), ServerError> {
    if manifest.game_contract() != profile_contract(profile) {
        return Err(ServerError::startup(
            "active release game does not match configured profile",
        ));
    }
    Ok(())
}

pub struct BoundServer {
    listener: TcpListener,
    router: axum::Router,
    world: crate::world::WorldHandle,
    allowed_origin: String,
    lifecycle: crate::lifecycle::LifecycleController,
}

impl BoundServer {
    pub async fn bind(config: ServerConfig) -> Result<Self, ServerError> {
        if config.behind_tls_ingress && config.allow_insecure_http {
            return Err(ServerError::Configuration(
                "behind-tls-ingress cannot use insecure HTTP",
            ));
        }
        if config.behind_tls_ingress && !matches!(config.storage, StorageConfig::Postgres { .. }) {
            return Err(ServerError::Configuration(
                "behind-tls-ingress requires PostgreSQL storage",
            ));
        }
        if config.behind_tls_ingress && config.bind.ip().is_loopback() {
            return Err(ServerError::Configuration(
                "behind-tls-ingress requires non-loopback bind",
            ));
        }
        if config.behind_tls_ingress && config.operator.is_some() {
            return Err(ServerError::Configuration(
                "behind-tls-ingress cannot expose operator control",
            ));
        }
        if !config.behind_tls_ingress && !config.bind.ip().is_loopback() {
            return Err(ServerError::Configuration("bind address must be loopback"));
        }
        crate::cli::validate_origin(&config.allowed_origin, config.allow_insecure_http)
            .map_err(|_| ServerError::Configuration("origin is not valid for this mode"))?;
        let operator_config = config.operator;
        let game = config.game;
        let world_id = worldwright_core::ids::WorldId::from_u128(1)
            .map_err(|_| ServerError::startup("world identity failed"))?;
        let mut operator_store = None;
        let (content, auth, registrations, durable, initial_game, _startup_release_guard) =
            match config.storage {
                StorageConfig::Local {
                    accounts_path,
                    content_path,
                } => {
                    if config.oidc.is_some() {
                        return Err(ServerError::Configuration(
                            "OIDC requires PostgreSQL storage",
                        ));
                    }
                    if operator_config.is_some() {
                        return Err(ServerError::Configuration(
                            "operator control requires PostgreSQL storage",
                        ));
                    }
                    let content = worldwright_content::load_file(&content_path, Default::default())
                        .map_err(|_| ServerError::startup("world content failed validation"))?;
                    let auth = crate::auth::AuthService::load_local(
                        &accounts_path,
                        !config.allow_insecure_http,
                    )?;
                    let registrations = auth.registrations();
                    (content, auth, registrations, None, None, None)
                }
                StorageConfig::Postgres { database_url } => {
                    let store = worldwright_persistence::PostgresStore::connect(&database_url)
                        .await
                        .map_err(|_| ServerError::startup("durable storage connection failed"))?;
                    store
                        .verify_migrations()
                        .await
                        .map_err(|_| ServerError::startup("durable migrations are not current"))?;
                    let active = store
                        .active_artifact(world_id)
                        .await
                        .map_err(|_| ServerError::startup("durable active release is not ready"))?;
                    let verified = worldwright_release::verify_bytes(
                        "durable active release",
                        active.bytes(),
                        Some(active.digest()),
                    )
                    .map_err(|_| ServerError::startup("durable active release is invalid"))?;
                    if verified.manifest().release_id() != active.release_id() {
                        return Err(ServerError::startup(
                            "durable active release identity is invalid",
                        ));
                    }
                    validate_active_game(config.game, verified.manifest())?;
                    let content = verified.content().clone();
                    store
                        .verify_world(world_id, content.namespace(), content.schema_version())
                        .await
                        .map_err(|_| {
                            ServerError::startup("durable world does not match content")
                        })?;
                    let writer = random_writer_id()
                        .ok_or_else(|| ServerError::startup("writer identity generation failed"))?;
                    let lease = store
                        .acquire_writer(world_id, writer)
                        .await
                        .map_err(|_| ServerError::startup("writer acquisition failed"))?;
                    let mut game_snapshot = store
                        .load_game_state(
                            lease,
                            active.digest(),
                            verified.manifest().game_contract(),
                        )
                        .await
                        .map_err(|_| ServerError::startup("durable game state could not load"))?;
                    let state_store = store.clone();
                    operator_store = Some(store.clone());
                    let (auth, registrations) = crate::auth::AuthService::postgres(
                        store,
                        world_id,
                        !config.allow_insecure_http,
                    )
                    .await?;
                    if game_snapshot.state_version == 0 {
                        let (state, rooms) =
                            crate::world::initial_durable_state(&content, &registrations, game)?;
                        state_store
                            .initialize_game_state(
                                lease,
                                active.digest(),
                                verified.manifest().game_contract(),
                                worldwright_persistence::GameplayState {
                                    runtime: &state,
                                    rooms: &rooms,
                                    now: worldwright_core::time::Tick::ZERO,
                                },
                            )
                            .await
                            .map_err(|_| {
                                ServerError::startup("durable game state initialization failed")
                            })?;
                        game_snapshot.state_version = 1;
                        game_snapshot.runtime = state;
                        game_snapshot.rooms = rooms;
                    }
                    let item_definition = format!("{}:items.welcome_token", content.namespace());
                    let durable = crate::world::DurableContext {
                        store: Arc::new(state_store),
                        lease,
                        world: world_id,
                        release_digest: active.digest(),
                        item_definition,
                        game: verified.manifest().game_contract(),
                        game_state_version: game_snapshot.state_version,
                    };
                    let startup_release_guard = durable
                        .store
                        .begin_release_read(lease, active.digest())
                        .await
                        .map_err(|_| {
                            ServerError::startup("durable active release changed during startup")
                        })?;
                    (
                        content,
                        auth,
                        registrations,
                        Some(durable),
                        Some(game_snapshot),
                        Some(startup_release_guard),
                    )
                }
            };
        let world = crate::world::WorldHandle::new_with_profile(
            content,
            registrations,
            durable,
            game,
            initial_game,
        )?;
        let lifecycle = crate::lifecycle::LifecycleController::new();
        let operator = operator_config.map(|config| {
            Arc::new(crate::operator::OperatorService {
                store: operator_store.expect("operator requires PostgreSQL"),
                key: Arc::new(config.signing_key),
                issuer: config.issuer,
                audience: config.audience,
                environment: config.environment,
                world: world_id,
                lifecycle: lifecycle.clone(),
                metrics: crate::metrics::Metrics::new(),
                drain_gate: Arc::new(tokio::sync::Mutex::new(())),
            })
        });
        let listener = match TcpListener::bind(config.bind).await {
            Ok(listener) => listener,
            Err(error) => {
                world.shutdown().await;
                return Err(ServerError::io(error));
            }
        };
        let oidc = match config.oidc {
            Some(value) => Some(crate::oidc::OidcService::discover(value, auth.clone()).await?),
            None => None,
        };
        let router = crate::web::router(crate::web::AppState {
            allowed_origin: config.allowed_origin.clone(),
            auth,
            world: world.clone(),
            operator,
            oidc,
        });
        Ok(Self {
            listener,
            router,
            world,
            allowed_origin: config.allowed_origin,
            lifecycle,
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
    let lifecycle = server.lifecycle.clone();
    let mut lifecycle_rx = lifecycle.subscribe();
    let graceful = async move {
        let operator_initiated = tokio::select! {
            _ = shutdown => false,
            _ = lifecycle_rx.changed() => true,
        };
        world_for_shutdown.begin_drain();
        lifecycle.drain();
        let _ = timeout(Duration::from_secs(30), world_for_shutdown.drain()).await;
        complete_graceful_lifecycle(&lifecycle, operator_initiated);
    };
    let result = axum::serve(server.listener, server.router)
        .with_graceful_shutdown(graceful)
        .await
        .map_err(ServerError::io);
    server
        .lifecycle
        .transition(crate::lifecycle::LifecycleState::Stopping);
    server.world.shutdown().await;
    server
        .lifecycle
        .transition(crate::lifecycle::LifecycleState::Stopped);
    result
}

fn complete_graceful_lifecycle(
    lifecycle: &crate::lifecycle::LifecycleController,
    operator_initiated: bool,
) {
    if !operator_initiated {
        lifecycle.transition(crate::lifecycle::LifecycleState::Quiescent);
    }
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
    #[test]
    fn operator_drain_does_not_claim_quiescence_before_completion_audit() {
        let operator_lifecycle = crate::lifecycle::LifecycleController::new();
        assert!(operator_lifecycle.drain());
        complete_graceful_lifecycle(&operator_lifecycle, true);
        assert_eq!(
            operator_lifecycle.state(),
            crate::lifecycle::LifecycleState::Draining
        );

        let signal_lifecycle = crate::lifecycle::LifecycleController::new();
        assert!(signal_lifecycle.drain());
        complete_graceful_lifecycle(&signal_lifecycle, false);
        assert_eq!(
            signal_lifecycle.state(),
            crate::lifecycle::LifecycleState::Quiescent
        );
    }

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
            storage: crate::StorageConfig::Local {
                accounts_path,
                content_path,
            },
            operator: None,
            allow_insecure_http: true,
            behind_tls_ingress: false,
            game: crate::GameProfile::Reference,
            oidc: None,
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
            Self::start_with_game(accounts, crate::GameProfile::Reference).await
        }

        async fn start_with_game(
            accounts: &[(&str, &str, u128, u128, &str)],
            game: crate::GameProfile,
        ) -> Self {
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
            let content: &[u8] = match game {
                crate::GameProfile::Reference => include_bytes!("../../../examples/two-room/world.json"),
                crate::GameProfile::Expedition => include_bytes!("../../../examples/expedition/world.json"),
            };
            std::fs::write(&content_path, content).expect("world fixture");
            std::fs::write(
                &accounts_path,
                serde_json::to_vec(&registry).expect("registry"),
            )
            .expect("account fixture");
            let server = BoundServer::bind(ServerConfig {
                bind: "127.0.0.1:0".parse().expect("bind"),
                allowed_origin: "http://localhost".to_owned(),
                storage: StorageConfig::Local {
                    accounts_path: accounts_path.clone(),
                    content_path: content_path.clone(),
                },
                operator: None,
                allow_insecure_http: true,
                behind_tls_ingress: false,
                game,
                oidc: None,
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

    async fn expedition_completion(socket: &mut Socket, state: &mut StreamState, request: &str) -> Vec<String> {
        let mut output = Vec::new();
        for _ in 0..32 {
            match receive_envelope(socket, state).await {
                ServerMessage::Output { text, .. } => output.push(text),
                ServerMessage::CommandResult { request_id, .. } if request_id == request => return output,
                ServerMessage::Error { code, message, .. } => panic!("expedition command failed: {code:?}: {message}"),
                _ => {}
            }
        }
        panic!("expedition completion exceeded message bound");
    }

    async fn expedition_command(socket: &mut Socket, state: &mut StreamState, sequence: &mut u64, command: ClientCommand) -> Vec<String> {
        *sequence += 1;
        let request = format!("expedition-test-{sequence}");
        send_client(socket, ClientMessage::Command { request_id: request.clone(), command }).await;
        expedition_completion(socket, state, &request).await
    }

    fn supplies_from_output(output: &[String]) -> u64 {
        output.iter().find_map(|line| line.strip_prefix("Supplies: "))
            .and_then(|line| line.split(" | ").next())
            .and_then(|value| value.parse().ok()).expect("supplies status")
    }

    #[tokio::test]
    async fn multiplayer_expedition_is_playable_through_progression_and_recovery() {
        let fixture = NetworkFixture::start_with_game(
            &[("alice", "password", 1, 1, "Alice")], crate::GameProfile::Expedition,
        ).await;
        let cookie = login(&fixture.router, "alice", "password").await;
        let mut socket = connect(&fixture, &cookie).await;
        let mut state = StreamState::new();
        send_client(&mut socket, ClientMessage::Hello { supported_versions: vec![1] }).await;
        assert!(matches!(receive_envelope(&mut socket, &mut state).await, ServerMessage::Welcome { .. }));
        send_client(&mut socket, ClientMessage::Attach { request_id: "expedition-attach".into() }).await;
        let initial = expedition_completion(&mut socket, &mut state, "expedition-attach").await;
        assert_eq!(supplies_from_output(&initial), 3);
        let mut sequence = 0;
        for victories in 1..=6 {
            // Replenish before encountering, so every random threat (2..4)
            // can be resolved without changing the game's balance contract.
            for _ in 0..9 {
                let status = expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::Look).await;
                if supplies_from_output(&status) == worldwright_expedition::SUPPLY_CAPACITY { break; }
                expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::GameAction {
                    action: worldwright_expedition::action_id("forage").unwrap().into(), argument: None,
                }).await;
            }
            let supplied = expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::Look).await;
            assert_eq!(supplies_from_output(&supplied), worldwright_expedition::SUPPLY_CAPACITY);
            let encounter = expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::GameAction {
                action: worldwright_expedition::ENCOUNTER_ACTION.into(), argument: None,
            }).await;
            let threat: u64 = encounter.iter().find_map(|line| line.strip_prefix("An encounter begins with threat "))
                .and_then(|value| value.trim_end_matches('.').parse().ok()).expect("encounter threat");
            assert!((2..=4).contains(&threat));
            for _ in 0..threat {
                expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::GameAction {
                    action: worldwright_expedition::STRIKE_ACTION.into(), argument: None,
                }).await;
            }
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            let status = expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::Look).await;
            assert!(status.iter().any(|line| line.contains(&format!("Victories: {victories} | Rank: {}", worldwright_expedition::rank(victories)))));
            let before_rest = supplies_from_output(&status);
            assert_eq!(before_rest, 9 - threat + 1);
            expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::GameAction {
                action: worldwright_expedition::REST_ACTION.into(), argument: None,
            }).await;
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            let rested = expedition_command(&mut socket, &mut state, &mut sequence, ClientCommand::Look).await;
            assert_eq!(supplies_from_output(&rested), before_rest + 1);
        }
        let cookie = login(&fixture.router, "alice", "password").await;
        let mut replacement = connect(&fixture, &cookie).await;
        let mut replacement_state = StreamState::new();
        send_client(&mut replacement, ClientMessage::Hello { supported_versions: vec![1] }).await;
        assert!(matches!(receive_envelope(&mut replacement, &mut replacement_state).await, ServerMessage::Welcome { .. }));
        send_client(&mut replacement, ClientMessage::Attach { request_id: "expedition-reconnect".into() }).await;
        let resumed = expedition_completion(&mut replacement, &mut replacement_state, "expedition-reconnect").await;
        assert!(resumed.iter().any(|line| line.contains("Victories: 6 | Rank: Trailblazer")));
        replacement.close(None).await.expect("close replacement");
        drop(socket);
        fixture.shutdown().await;
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
