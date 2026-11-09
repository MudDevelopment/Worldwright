use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};
use worldwright_content::{ValidatedWorldContent, WorldLookup, instantiate};
use worldwright_core::{
    GameModule,
    command::{ActorContext, CommandEnvelope, CommandIntent, PlayerText},
    ids::{CharacterId, ConnectionId, ItemId, OperationId, RoomId, WorldId},
    rule::RuleRegistry,
    runtime::{ExecutionResult, ReferenceGame, WorldError, WorldRuntime},
    time::{RandomExhausted, RandomSource, Tick},
};
use worldwright_persistence::{
    DurableCharacterRoom, DurableGameSnapshot, InventorySnapshot, PersistenceError, PostgresStore,
    ReleaseReadGuard, RewardOutcome, RewardResult, TransferOutcome, TransferResult, WriterLease,
};
use worldwright_protocol::{ClientCommand, ErrorCode, OutputKind, ServerMessage};
use worldwright_release::{GameContract, Sha256Digest};

const GAMEPLAY_CAPACITY: usize = 256;
const CONTROL_CAPACITY: usize = 64;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisconnectReason {
    Normal,
    SessionReplaced,
    SessionRevoked,
    SlowConsumer,
    Shutdown,
    Internal,
    Protocol(ErrorCode),
}

pub(crate) fn cancel_first(
    sender: &watch::Sender<Option<DisconnectReason>>,
    reason: DisconnectReason,
) {
    sender.send_if_modified(|current| {
        if current.is_none() {
            *current = Some(reason);
            true
        } else {
            false
        }
    });
}

fn release_disconnect_reason(error: PersistenceError) -> DisconnectReason {
    match error {
        PersistenceError::StaleWriter => DisconnectReason::Protocol(ErrorCode::WriterFenced),
        PersistenceError::Database(_) => {
            DisconnectReason::Protocol(ErrorCode::TemporarilyUnavailable)
        }
        _ => DisconnectReason::Protocol(ErrorCode::InternalError),
    }
}

pub(crate) struct DurableRewardSpec {
    key: String,
    generation: u64,
    credits: u64,
    item_definition: String,
    item: ItemId,
}

pub(crate) struct DurableActor {
    world: WorldId,
    account: worldwright_core::ids::AccountId,
    character: CharacterId,
    token_hash: [u8; 32],
}

pub(crate) fn initial_durable_state(
    content: &ValidatedWorldContent,
    registrations: &[crate::auth::CharacterRegistration],
    game: crate::GameProfile,
) -> Result<
    (
        worldwright_core::runtime::DurableWorldState,
        Vec<DurableCharacterRoom>,
    ),
    crate::ServerError,
> {
    let world =
        WorldId::from_u128(1).map_err(|_| crate::ServerError::startup("world identity failed"))?;
    let (mut builder, lookup) = instantiate(content, world)
        .map_err(|_| crate::ServerError::startup("content instantiation failed"))?
        .into_parts();
    let mut rooms = Vec::new();
    let mut registrations = registrations.to_vec();
    registrations.sort_by_key(|r| r.principal.character);
    for registration in registrations {
        let p = registration.principal;
        let room = match registration.room_definition.as_deref() {
            None => lookup.entry_room,
            Some(raw) => {
                let definition = raw.parse().map_err(|_| {
                    crate::ServerError::startup(
                        "durable character room is not present in loaded content",
                    )
                })?;
                lookup.room_ids.get(&definition).copied().ok_or_else(|| {
                    crate::ServerError::startup(
                        "durable character room is not present in loaded content",
                    )
                })?
            }
        };
        builder
            .add_character(p.character, p.account, room)
            .map_err(|_| crate::ServerError::startup("character initialization failed"))?;
        if game == crate::GameProfile::Expedition {
            worldwright_expedition::initialize_character(&mut builder, p.character)
                .map_err(|_| crate::ServerError::startup("expedition initialization failed"))?;
        }
        rooms.push(DurableCharacterRoom {
            character: p.character,
            room_definition: lookup
                .room_ids
                .iter()
                .find_map(|(id, r)| (*r == room).then(|| id.to_string()))
                .unwrap_or_else(|| content.entry_room().to_string()),
        });
    }
    let mut rules = RuleRegistry::new();
    match game {
        crate::GameProfile::Reference => ReferenceGame.register_rules(&mut rules),
        crate::GameProfile::Expedition => {
            worldwright_expedition::ExpeditionGame.register_rules(&mut rules)
        }
    }
    .map_err(|_| crate::ServerError::startup("game registration failed"))?;
    let runtime = builder
        .build(rules)
        .map_err(|_| crate::ServerError::startup("runtime build failed"))?;
    Ok((
        runtime
            .snapshot()
            .durable_state()
            .map_err(|_| crate::ServerError::startup("durable state export failed"))?,
        rooms,
    ))
}

#[allow(dead_code)]
pub(crate) struct DurableGameplayMutation {
    pub(crate) actor: DurableActor,
    pub(crate) lease: WriterLease,
    pub(crate) expected_release: Sha256Digest,
    pub(crate) game: worldwright_release::GameContract,
    pub(crate) request_id: String,
    pub(crate) operation: OperationId,
    pub(crate) kind: worldwright_persistence::GameplayIntentKind,
    pub(crate) intent: Vec<u8>,
    pub(crate) expected_state_version: u64,
    pub(crate) runtime: worldwright_core::runtime::DurableWorldState,
    pub(crate) rooms: Vec<worldwright_persistence::DurableCharacterRoom>,
    pub(crate) now: Tick,
    pub(crate) result: serde_json::Value,
}

pub(crate) struct DurableTickMutation {
    lease: WriterLease,
    release: Sha256Digest,
    game: GameContract,
    version: u64,
    runtime: worldwright_core::runtime::DurableWorldState,
    rooms: Vec<DurableCharacterRoom>,
    now: Tick,
}

#[async_trait]
pub(crate) trait DurableStore: Send + Sync {
    async fn commit_tick(&self, input: DurableTickMutation) -> Result<u64, PersistenceError> {
        let _ = input;
        Err(PersistenceError::InvalidData)
    }
    #[allow(dead_code)]
    async fn commit_gameplay(
        &self,
        input: DurableGameplayMutation,
    ) -> Result<worldwright_persistence::GameplayCommit, PersistenceError>;
    async fn begin_release_read(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
    ) -> Result<ReleaseReadGuard, PersistenceError>;
    async fn inventory(
        &self,
        actor: &DurableActor,
        lease: WriterLease,
    ) -> Result<InventorySnapshot, PersistenceError>;
    async fn claim_reward(
        &self,
        actor: &DurableActor,
        lease: worldwright_persistence::WriterLease,
        request_id: String,
        operation: OperationId,
        spec: DurableRewardSpec,
    ) -> Result<RewardResult, PersistenceError>;
    async fn transfer_item(
        &self,
        actor: &DurableActor,
        lease: worldwright_persistence::WriterLease,
        request_id: String,
        operation: OperationId,
        item: ItemId,
        recipient: String,
    ) -> Result<TransferResult, PersistenceError>;
}

#[async_trait]
impl DurableStore for PostgresStore {
    async fn commit_tick(&self, input: DurableTickMutation) -> Result<u64, PersistenceError> {
        self.commit_gameplay_tick(input.lease, input.release, input.game, input.version,
            worldwright_persistence::GameplayState { runtime: &input.runtime, rooms: &input.rooms, now: input.now }).await
    }
    async fn commit_gameplay(
        &self,
        input: DurableGameplayMutation,
    ) -> Result<worldwright_persistence::GameplayCommit, PersistenceError> {
        self.commit_gameplay(worldwright_persistence::GameplayMutation {
            lease: input.lease,
            expected_release: input.expected_release,
            game: input.game,
            actor: worldwright_persistence::AuthenticatedActor {
                world: input.actor.world,
                account: input.actor.account,
                character: input.actor.character,
                token_hash: &input.actor.token_hash,
            },
            request_id: &input.request_id,
            operation: input.operation,
            kind: input.kind,
            intent: &input.intent,
            expected_state_version: input.expected_state_version,
            state: worldwright_persistence::GameplayState {
                runtime: &input.runtime,
                rooms: &input.rooms,
                now: input.now,
            },
            result: input.result,
        })
        .await
    }
    async fn begin_release_read(
        &self,
        lease: WriterLease,
        expected_release: Sha256Digest,
    ) -> Result<ReleaseReadGuard, PersistenceError> {
        PostgresStore::begin_release_read(self, lease, expected_release).await
    }
    async fn inventory(
        &self,
        actor: &DurableActor,
        lease: WriterLease,
    ) -> Result<InventorySnapshot, PersistenceError> {
        self.inventory_with_lease(
            lease,
            &worldwright_persistence::AuthenticatedActor {
                world: actor.world,
                account: actor.account,
                character: actor.character,
                token_hash: &actor.token_hash,
            },
        )
        .await
    }
    async fn claim_reward(
        &self,
        actor: &DurableActor,
        lease: worldwright_persistence::WriterLease,
        request_id: String,
        operation: OperationId,
        spec: DurableRewardSpec,
    ) -> Result<RewardResult, PersistenceError> {
        self.claim_reward(
            lease,
            &worldwright_persistence::AuthenticatedActor {
                world: actor.world,
                account: actor.account,
                character: actor.character,
                token_hash: &actor.token_hash,
            },
            &request_id,
            operation,
            worldwright_persistence::RewardSpec {
                key: &spec.key,
                generation: spec.generation,
                credits: spec.credits,
                item_definition: &spec.item_definition,
                item: spec.item,
            },
        )
        .await
    }
    async fn transfer_item(
        &self,
        actor: &DurableActor,
        lease: worldwright_persistence::WriterLease,
        request_id: String,
        operation: OperationId,
        item: ItemId,
        recipient: String,
    ) -> Result<TransferResult, PersistenceError> {
        self.transfer_item(
            lease,
            &worldwright_persistence::AuthenticatedActor {
                world: actor.world,
                account: actor.account,
                character: actor.character,
                token_hash: &actor.token_hash,
            },
            &request_id,
            operation,
            worldwright_persistence::TransferSpec {
                item,
                recipient_username: &recipient,
            },
        )
        .await
    }
}

#[derive(Clone)]
pub(crate) struct DurableContext {
    pub(crate) store: Arc<dyn DurableStore>,
    pub(crate) lease: worldwright_persistence::WriterLease,
    pub(crate) world: WorldId,
    pub(crate) release_digest: Sha256Digest,
    pub(crate) item_definition: String,
    #[allow(dead_code)]
    pub(crate) game: GameContract,
    #[allow(dead_code)]
    pub(crate) game_state_version: u64,
}

#[derive(Clone)]
pub(crate) struct WorldHandle {
    gameplay: mpsc::Sender<GameplayRequest>,
    control: mpsc::Sender<ControlRequest>,
    ready: Arc<AtomicBool>,
    accepting: Arc<AtomicBool>,
    connected_clients: Arc<AtomicU64>,
    admission_rejections: Arc<AtomicU64>,
    timings: Arc<crate::metrics::GameplayTimings>,
}

pub(crate) enum Outbound {
    Message(ServerMessage),
    Pong(Vec<u8>),
}

pub(crate) struct ActorContextResult {
    pub(crate) actor: ActorContext,
}

#[derive(Debug)]
pub(crate) enum AttachError {
    OwnerMismatch,
    ConnectionInUse,
    UnknownCharacter,
    Internal,
    Draining,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubmitError {
    Full,
    Closed,
    Draining,
}

struct GameplayRequest {
    enqueued_at: Instant,
    actor: ActorContext,
    request_id: String,
    command: ClientCommand,
}

enum ControlRequest {
    Attach {
        principal: crate::auth::Principal,
        session_token: String,
        connection: ConnectionId,
        outbound: mpsc::Sender<Outbound>,
        cancel: watch::Sender<Option<DisconnectReason>>,
        response: oneshot::Sender<Result<ActorContextResult, AttachError>>,
    },
    Initial {
        actor: ActorContext,
        request_id: String,
        response: oneshot::Sender<Result<(), AttachError>>,
    },
    Detach {
        actor: ActorContext,
        response: oneshot::Sender<()>,
    },
    RevokeSession {
        session_token: String,
        response: oneshot::Sender<()>,
    },
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

struct Route {
    actor: ActorContext,
    session_token: String,
    outbound: mpsc::Sender<Outbound>,
    cancel: watch::Sender<Option<DisconnectReason>>,
}

struct ReadyGuard(Arc<AtomicBool>);
impl Drop for ReadyGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct Owner {
    runtime: WorldRuntime,
    lookup: WorldLookup,
    world: WorldId,
    names: HashMap<CharacterId, String>,
    routes: HashMap<CharacterId, Route>,
    gameplay: mpsc::Receiver<GameplayRequest>,
    control: mpsc::Receiver<ControlRequest>,
    _ready_guard: ReadyGuard,
    durable: Option<DurableContext>,
    next_operation: u128,
    started: Instant,
    connected_clients: Arc<AtomicU64>,
    random: OwnerRandom,
    staged_messages: Option<Vec<(ActorContext, ServerMessage)>>,
    timings: Arc<crate::metrics::GameplayTimings>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredGameplayResult {
    messages: Vec<ServerMessage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurableCommandStatus {
    Succeeded,
    Failed,
    Stop,
}

impl StoredGameplayResult {
    fn validate(&self, request_id: &str) -> bool {
        self.messages.len() <= 64
            && self.messages.iter().all(|message| match message {
                ServerMessage::Output {
                    request_id: Some(found),
                    ..
                }
                | ServerMessage::Exits {
                    request_id: Some(found),
                    ..
                }
                | ServerMessage::CommandResult {
                    request_id: found, ..
                } => found == request_id,
                _ => false,
            })
    }
}

impl WorldHandle {
    #[allow(dead_code)]
    pub(crate) fn new(
        content: ValidatedWorldContent,
        principals: Vec<crate::auth::Principal>,
    ) -> Result<Self, crate::ServerError> {
        Self::new_with_registrations(
            content,
            principals
                .into_iter()
                .map(|principal| crate::auth::CharacterRegistration {
                    principal,
                    room_definition: None,
                })
                .collect(),
        )
    }

    pub(crate) fn new_with_registrations(
        content: ValidatedWorldContent,
        registrations: Vec<crate::auth::CharacterRegistration>,
    ) -> Result<Self, crate::ServerError> {
        Self::new_with_durable(content, registrations, None)
    }

    pub(crate) fn new_with_durable(
        content: ValidatedWorldContent,
        registrations: Vec<crate::auth::CharacterRegistration>,
        durable: Option<DurableContext>,
    ) -> Result<Self, crate::ServerError> {
        Self::new_with_profile(
            content,
            registrations,
            durable,
            crate::GameProfile::Reference,
            None,
        )
    }

    pub(crate) fn new_with_profile(
        content: ValidatedWorldContent,
        registrations: Vec<crate::auth::CharacterRegistration>,
        durable: Option<DurableContext>,
        game: crate::GameProfile,
        initial_game: Option<DurableGameSnapshot>,
    ) -> Result<Self, crate::ServerError> {
        let world = WorldId::from_u128(1)
            .map_err(|_| crate::ServerError::startup("world ID initialization failed"))?;
        if durable
            .as_ref()
            .is_some_and(|context| context.world != world)
        {
            return Err(crate::ServerError::startup(
                "durable world identity does not match fixed world",
            ));
        }
        let (mut builder, lookup) = instantiate(&content, world)
            .map_err(|_| crate::ServerError::startup("world content instantiation failed"))?
            .into_parts();
        let mut names = HashMap::with_capacity(registrations.len());
        for registration in registrations {
            let principal = registration.principal;
            let room = match registration.room_definition.as_deref() {
                None => lookup.entry_room,
                Some(raw) => {
                    let definition = raw.parse().map_err(|_| {
                        crate::ServerError::startup(
                            "durable character room is not present in loaded content",
                        )
                    })?;
                    lookup.room_ids.get(&definition).copied().ok_or_else(|| {
                        crate::ServerError::startup(
                            "durable character room is not present in loaded content",
                        )
                    })?
                }
            };
            builder
                .add_character(principal.character, principal.account, room)
                .map_err(|_| {
                    crate::ServerError::startup("world character initialization failed")
                })?;
            if game == crate::GameProfile::Expedition && initial_game.is_none() {
                worldwright_expedition::initialize_character(&mut builder, principal.character)
                    .map_err(|_| {
                        crate::ServerError::startup("expedition character initialization failed")
                    })?;
            }
            names.insert(principal.character, principal.character_name);
        }
        let mut rules = RuleRegistry::new();
        match game {
            crate::GameProfile::Reference => {
                worldwright_core::game::GameModule::register_rules(&ReferenceGame, &mut rules)
                    .map_err(|_| {
                        crate::ServerError::startup("reference game registration failed")
                    })?;
            }
            crate::GameProfile::Expedition => {
                worldwright_core::game::GameModule::register_rules(
                    &worldwright_expedition::ExpeditionGame,
                    &mut rules,
                )
                .map_err(|_| crate::ServerError::startup("expedition game registration failed"))?;
            }
        }
        let runtime = if let Some(snapshot) = initial_game {
            builder
                .restore_durable_state(snapshot.runtime)
                .map_err(|_| crate::ServerError::startup("durable game state restore failed"))?;
            builder.build(rules)
        } else {
            builder.build(rules)
        }
        .map_err(|_| crate::ServerError::startup("world runtime initialization failed"))?;
        let (gameplay, gameplay_receiver) = mpsc::channel(GAMEPLAY_CAPACITY);
        let (control, control_receiver) = mpsc::channel(CONTROL_CAPACITY);
        let ready = Arc::new(AtomicBool::new(true));
        let accepting = Arc::new(AtomicBool::new(true));
        let connected_clients = Arc::new(AtomicU64::new(0));
        let admission_rejections = Arc::new(AtomicU64::new(0));
        let timings = Arc::new(crate::metrics::GameplayTimings::default());
        let random = match game {
            crate::GameProfile::Reference => OwnerRandom::Unavailable,
            crate::GameProfile::Expedition => OwnerRandom::seeded().ok_or_else(|| {
                crate::ServerError::startup("gameplay random initialization failed")
            })?,
        };
        let owner = Owner {
            runtime,
            lookup,
            world,
            names,
            routes: HashMap::new(),
            gameplay: gameplay_receiver,
            control: control_receiver,
            _ready_guard: ReadyGuard(ready.clone()),
            durable,
            next_operation: 1,
            started: Instant::now(),
            connected_clients: connected_clients.clone(),
            random,
            staged_messages: None,
            timings: Arc::clone(&timings),
        };
        tokio::spawn(owner.run());
        Ok(Self {
            gameplay,
            control,
            ready,
            accepting,
            connected_clients,
            admission_rejections,
            timings,
        })
    }

    pub(crate) async fn attach(
        &self,
        principal: crate::auth::Principal,
        session_token: String,
        connection: ConnectionId,
        outbound: mpsc::Sender<Outbound>,
        cancel: watch::Sender<Option<DisconnectReason>>,
    ) -> Result<ActorContext, AttachError> {
        if !self.accepting.load(Ordering::Acquire) {
            self.admission_rejections.fetch_add(1, Ordering::Relaxed);
            return Err(AttachError::Draining);
        }
        let (response, receiver) = oneshot::channel();
        let operation = self.control.send(ControlRequest::Attach {
            principal,
            session_token,
            connection,
            outbound,
            cancel,
            response,
        });
        let result = timeout(CONTROL_TIMEOUT, async {
            operation.await.map_err(|_| AttachError::Internal)?;
            receiver.await.map_err(|_| AttachError::Internal)?
        })
        .await
        .map_err(|_| AttachError::Internal)?;
        Ok(result?.actor)
    }

    pub(crate) async fn initial(
        &self,
        actor: ActorContext,
        request_id: String,
    ) -> Result<(), AttachError> {
        let (response, receiver) = oneshot::channel();
        let operation = self.control.send(ControlRequest::Initial {
            actor,
            request_id,
            response,
        });
        timeout(CONTROL_TIMEOUT, async {
            operation.await.map_err(|_| AttachError::Internal)?;
            receiver.await.map_err(|_| AttachError::Internal)?
        })
        .await
        .map_err(|_| AttachError::Internal)?
    }

    pub(crate) async fn detach(&self, actor: ActorContext) {
        let (response, receiver) = oneshot::channel();
        let operation = self
            .control
            .send(ControlRequest::Detach { actor, response });
        let _ = timeout(CONTROL_TIMEOUT, async {
            operation.await.ok()?;
            receiver.await.ok()
        })
        .await;
    }

    pub(crate) async fn revoke_session(&self, session_token: String) {
        let (response, receiver) = oneshot::channel();
        let operation = self.control.send(ControlRequest::RevokeSession {
            session_token,
            response,
        });
        let _ = timeout(CONTROL_TIMEOUT, async {
            operation.await.ok()?;
            receiver.await.ok()
        })
        .await;
    }

    pub(crate) fn submit(
        &self,
        actor: ActorContext,
        request_id: String,
        command: ClientCommand,
    ) -> Result<(), SubmitError> {
        if !self.accepting.load(Ordering::Acquire) {
            self.admission_rejections.fetch_add(1, Ordering::Relaxed);
            return Err(SubmitError::Draining);
        }
        self.gameplay
            .try_send(GameplayRequest {
                enqueued_at: Instant::now(),
                actor,
                request_id,
                command,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) if !self.accepting.load(Ordering::Acquire) => {
                    SubmitError::Draining
                }
                mpsc::error::TrySendError::Full(_) => SubmitError::Full,
                mpsc::error::TrySendError::Closed(_) => SubmitError::Closed,
            })
    }

    pub(crate) fn telemetry(&self) -> String {
        format!("worldwright_ready {}\nworldwright_connected_clients {}\nworldwright_admission_rejections_total {}\n{}",
            u8::from(self.is_ready()), self.connected_clients(), self.admission_rejections(), self.timings.render())
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire) && self.accepting.load(Ordering::Acquire)
    }
    pub(crate) fn begin_drain(&self) {
        self.accepting.store(false, Ordering::Release);
        self.ready.store(false, Ordering::Release);
    }

    pub(crate) fn connected_clients(&self) -> u64 {
        self.connected_clients.load(Ordering::Relaxed)
    }

    pub(crate) fn admission_rejections(&self) -> u64 {
        self.admission_rejections.load(Ordering::Relaxed)
    }

    pub(crate) async fn drain(&self) {
        self.begin_drain();
        self.shutdown().await;
    }

    pub(crate) async fn shutdown(&self) {
        let (response, receiver) = oneshot::channel();
        let operation = self.control.send(ControlRequest::Shutdown { response });
        let _ = timeout(CONTROL_TIMEOUT, async {
            operation.await.ok()?;
            receiver.await.ok()
        })
        .await;
    }
}

impl Owner {
    async fn run(mut self) {
        let mut gameplay_timer = tokio::time::interval(Duration::from_millis(100));
        gameplay_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut release_monitor = self
            .durable
            .as_ref()
            .map(|_| tokio::time::interval(Duration::from_secs(1)));
        loop {
            if self.control.is_closed() && self.control.is_empty()
                && self.gameplay.is_closed() && self.gameplay.is_empty() { break; }
            tokio::select! {
                biased;
                _ = async {
                    match release_monitor.as_mut() {
                        Some(interval) => { interval.tick().await; }
                        None => std::future::pending::<()>().await,
                    }
                } => {
                    if self.monitor_release().await { break; }
                }
                Some(request) = self.control.recv() => {
                    if self.handle_control(request).await { break; }
                }
                _ = gameplay_timer.tick() => {
                    if self.advance_idle().await { break; }
                }
                Some(request) = self.gameplay.recv() => {
                    if self.handle_gameplay(request).await { break; }
                }
                else => break,
            }
        }
        self.shutdown_routes(DisconnectReason::Shutdown);
    }

    async fn advance_idle(&mut self) -> bool {
        let _timer_timing = self.timings.start(5);
        let Ok(elapsed) = u64::try_from(self.started.elapsed().as_millis()) else {
            return self.durable_failure(DisconnectReason::Internal);
        };
        let now = Tick::from_millis(elapsed);
        if !self.runtime.next_scheduled_at().is_some_and(|at| at <= now) { return false; }
        let checkpoint = self.runtime.snapshot();
        let result = match self.runtime.advance(now) {
            Ok(result) => result,
            Err(_) => return self.durable_failure(DisconnectReason::Internal),
        };
        if result.events.is_empty() { return false; }
        let Some(context) = self.durable.clone() else { return false; };
        let snapshot = self.runtime.snapshot();
        let runtime = match snapshot.durable_state() {
            Ok(runtime) => runtime,
            Err(_) => return self.durable_failure(DisconnectReason::Internal),
        };
        let mut rooms = Vec::new();
        for character in snapshot.characters() {
            let Some(room_definition) = snapshot.room_of(character).and_then(|room| {
                self.lookup.room_ids.iter().find_map(|(definition, id)|
                    (*id == room).then(|| definition.to_string()))
            }) else { return self.durable_failure(DisconnectReason::Internal); };
            rooms.push(DurableCharacterRoom { character, room_definition });
        }
        match context.store.commit_tick(DurableTickMutation {
            lease: context.lease, release: context.release_digest, game: context.game,
            version: context.game_state_version, runtime, rooms, now,
        }).await {
            Ok(version) => {
                if let Some(current) = self.durable.as_mut() { current.game_state_version = version; }
                false
            }
            Err(error) => {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                let reason = if matches!(error, PersistenceError::StaleWriter) {
                    DisconnectReason::Protocol(ErrorCode::WriterFenced)
                } else if matches!(error, PersistenceError::UnknownOutcome) {
                    DisconnectReason::Protocol(ErrorCode::OperationPending)
                } else { DisconnectReason::Protocol(ErrorCode::TemporarilyUnavailable) };
                self.durable_failure(reason)
            }
        }
    }

    async fn monitor_release(&mut self) -> bool {
        let Some(context) = self.durable.clone() else {
            return false;
        };
        match context
            .store
            .begin_release_read(context.lease, context.release_digest)
            .await
        {
            Ok(guard) => {
                drop(guard);
                false
            }
            Err(error) => self.durable_failure(release_disconnect_reason(error)),
        }
    }

    async fn handle_control(&mut self, request: ControlRequest) -> bool {
        match request {
            ControlRequest::Attach {
                principal,
                session_token,
                connection,
                outbound,
                cancel,
                response,
            } => {
                if response.is_closed() {
                    return false;
                }
                match self.attach(principal, session_token, connection, outbound, cancel) {
                    Ok(result) => {
                        let actor = result.actor;
                        let response_closed = response.is_closed();
                        let response_failed =
                            response.send(Ok(ActorContextResult { actor })).is_err();
                        if response_closed || response_failed {
                            self.remove_route(actor, DisconnectReason::Normal);
                        }
                    }
                    Err(error) => {
                        let _ = response.send(Err(error));
                    }
                }
                false
            }
            ControlRequest::Initial {
                actor,
                request_id,
                response,
            } => {
                if response.is_closed() {
                    self.remove_route(actor, DisconnectReason::Normal);
                    return false;
                }
                if self.durable.is_some() {
                    let attached = self.attached_message(actor, &request_id);
                    let status = self
                        .commit_durable_command(
                            GameplayRequest {
                                enqueued_at: Instant::now(),
                                actor,
                                request_id,
                                command: ClientCommand::Look,
                            },
                            false,
                            vec![(actor, attached)],
                        )
                        .await;
                    let result =
                        if status == DurableCommandStatus::Succeeded && self.is_current(actor) {
                            Ok(())
                        } else {
                            if self.is_current(actor) {
                                self.remove_route(actor, DisconnectReason::Internal);
                            }
                            Err(AttachError::Internal)
                        };
                    let _ = response.send(result);
                    return status == DurableCommandStatus::Stop;
                }
                let result = self.initial(actor, &request_id);
                if response.send(result).is_err() && self.is_current(actor) {
                    self.remove_route(actor, DisconnectReason::Normal);
                }
                false
            }
            ControlRequest::Detach { actor, response } => {
                self.detach(actor);
                let _ = response.send(());
                false
            }
            ControlRequest::RevokeSession {
                session_token,
                response,
            } => {
                let routes: Vec<ActorContext> = self
                    .routes
                    .values()
                    .filter(|route| route.session_token == session_token)
                    .map(|route| route.actor)
                    .collect();
                for actor in routes {
                    self.remove_route(actor, DisconnectReason::SessionRevoked);
                }
                let _ = response.send(());
                false
            }
            ControlRequest::Shutdown { response } => {
                self.shutdown_routes(DisconnectReason::Shutdown);
                let _ = response.send(());
                true
            }
        }
    }

    fn attach(
        &mut self,
        principal: crate::auth::Principal,
        session_token: String,
        connection: ConnectionId,
        outbound: mpsc::Sender<Outbound>,
        cancel: watch::Sender<Option<DisconnectReason>>,
    ) -> Result<ActorContextResult, AttachError> {
        if !self.names.contains_key(&principal.character) {
            return Err(AttachError::UnknownCharacter);
        }
        let previous = self
            .routes
            .get(&principal.character)
            .map(|route| route.actor);
        let actor = self
            .runtime
            .attach(principal.account, principal.character, connection)
            .map_err(|error| match error {
                WorldError::OwnerMismatch => AttachError::OwnerMismatch,
                WorldError::ConnectionInUse => AttachError::ConnectionInUse,
                WorldError::UnknownCharacter => AttachError::UnknownCharacter,
                _ => AttachError::Internal,
            })?;
        if let Some(previous) = previous {
            self.remove_route_without_detach(previous, DisconnectReason::SessionReplaced);
        }
        self.routes.insert(
            actor.character,
            Route {
                actor,
                session_token,
                outbound,
                cancel,
            },
        );
        self.connected_clients.fetch_add(1, Ordering::Relaxed);
        Ok(ActorContextResult { actor })
    }

    fn initial(&mut self, actor: ActorContext, request_id: &str) -> Result<(), AttachError> {
        let attached = self.attached_message(actor, request_id);
        if !self.send_to_actor(actor, attached) {
            return Err(AttachError::Internal);
        }
        self.look(actor, request_id)
            .map_err(|_| AttachError::Internal)?;
        if self.is_current(actor) {
            Ok(())
        } else {
            Err(AttachError::Internal)
        }
    }

    fn attached_message(&self, actor: ActorContext, request_id: &str) -> ServerMessage {
        ServerMessage::Attached {
            request_id: request_id.to_owned(),
            character_name: self
                .names
                .get(&actor.character)
                .cloned()
                .unwrap_or_else(|| "Player".to_owned()),
        }
    }

    fn detach(&mut self, actor: ActorContext) {
        self.remove_route(actor, DisconnectReason::Normal);
    }

    fn shutdown_routes(&mut self, reason: DisconnectReason) {
        let actors: Vec<ActorContext> = self.routes.values().map(|route| route.actor).collect();
        for actor in actors {
            self.remove_route(actor, reason);
        }
    }

    async fn handle_gameplay(&mut self, request: GameplayRequest) -> bool {
        self.timings.observe(0, request.enqueued_at.elapsed());
        let _owner_timing = self.timings.start(1);
        let durable_command = matches!(
            request.command,
            ClientCommand::Inventory
                | ClientCommand::ClaimReward { .. }
                | ClientCommand::TransferItem { .. }
        );
        if self.durable.is_some() && !durable_command {
            return self.handle_durable_gameplay(request).await;
        }
        if !durable_command {
            if !self.is_current(request.actor) {
                return false;
            }
            let _release_guard = if let Some(context) = self.durable.clone() {
                match context
                    .store
                    .begin_release_read(context.lease, context.release_digest)
                    .await
                {
                    Ok(guard) => Some(guard),
                    Err(error) => return self.durable_failure(release_disconnect_reason(error)),
                }
            } else {
                None
            };
            self.handle_gameplay_sync(request);
            return false;
        }
        if !self.is_current(request.actor) {
            return false;
        }
        if !self.send_to_actor(
            request.actor,
            ServerMessage::Accepted {
                request_id: request.request_id.clone(),
            },
        ) {
            return false;
        }
        let Some(context) = self.durable.clone() else {
            let _ = self.send_to_actor(
                request.actor,
                ServerMessage::Error {
                    request_id: Some(request.request_id),
                    code: ErrorCode::DurabilityUnavailable,
                    message: "durability unavailable".to_owned(),
                    fatal: false,
                },
            );
            return false;
        };
        let token = self
            .routes
            .get(&request.actor.character)
            .map(|route| route.session_token.clone())
            .unwrap_or_default();
        let actor = DurableActor {
            world: context.world,
            account: request.actor.account,
            character: request.actor.character,
            token_hash: worldwright_persistence::session_token_hash(&token),
        };
        let result =
            match request.command {
                ClientCommand::Inventory => context
                    .store
                    .inventory(&actor, context.lease)
                    .await
                    .map(|snapshot| ServerMessage::Inventory {
                        request_id: request.request_id.clone(),
                        balance: snapshot.balance,
                        character_version: snapshot.character_version,
                        items: snapshot
                            .items
                            .into_iter()
                            .map(|item| worldwright_protocol::InventoryItem {
                                item_id: item.item.to_string(),
                                definition_id: item.definition,
                                version: item.version,
                            })
                            .collect(),
                    }),
                ClientCommand::ClaimReward { reward } => {
                    if reward != "welcome" {
                        let _ = self.send_to_actor(
                            request.actor,
                            ServerMessage::Error {
                                request_id: Some(request.request_id),
                                code: ErrorCode::NotFound,
                                message: "reward not found".to_owned(),
                                fatal: false,
                            },
                        );
                        return false;
                    }
                    let Ok((operation, item)) = durable_ids() else {
                        return self.durable_failure(DisconnectReason::Internal);
                    };
                    let spec = DurableRewardSpec {
                        key: "welcome".to_owned(),
                        generation: 1,
                        credits: 100,
                        item_definition: context.item_definition.clone(),
                        item,
                    };
                    context
                        .store
                        .claim_reward(
                            &actor,
                            context.lease,
                            request.request_id.clone(),
                            operation,
                            spec,
                        )
                        .await
                        .map(|result| ServerMessage::RewardResult {
                            request_id: request.request_id.clone(),
                            operation_id: result.operation.to_string(),
                            outcome: map_reward_outcome(result.outcome),
                            balance: result.balance,
                            item_id: result.item.map(|item| item.to_string()),
                            replayed: result.replayed,
                        })
                }
                ClientCommand::TransferItem { item_id, recipient } => {
                    let Ok(item) = item_id.parse::<worldwright_core::ids::ItemId>() else {
                        let _ = self.send_to_actor(
                            request.actor,
                            ServerMessage::Error {
                                request_id: Some(request.request_id),
                                code: ErrorCode::InvalidCommand,
                                message: "invalid item ID".to_owned(),
                                fatal: false,
                            },
                        );
                        return false;
                    };
                    let Ok(operation) = durable_operation_id() else {
                        return self.durable_failure(DisconnectReason::Internal);
                    };
                    context
                        .store
                        .transfer_item(
                            &actor,
                            context.lease,
                            request.request_id.clone(),
                            operation,
                            item,
                            recipient,
                        )
                        .await
                        .map(|result| ServerMessage::TransferResult {
                            request_id: request.request_id.clone(),
                            operation_id: result.operation.to_string(),
                            outcome: map_transfer_outcome(result.outcome),
                            item_id: result.item.to_string(),
                            owner_character_id: result.owner.map(|id| id.to_string()),
                            recipient_name: result.recipient_name,
                            replayed: result.replayed,
                        })
                }
                _ => unreachable!(),
            };
        match result {
            Ok(message) => {
                let _ = self.send_to_actor(request.actor, message);
                false
            }
            Err(error) => self.map_durable_failure(request.actor, Some(request.request_id), error),
        }
    }

    async fn handle_durable_gameplay(&mut self, request: GameplayRequest) -> bool {
        self.commit_durable_command(request, true, Vec::new()).await == DurableCommandStatus::Stop
    }

    async fn commit_durable_command(
        &mut self,
        request: GameplayRequest,
        send_accepted: bool,
        prelude: Vec<(ActorContext, ServerMessage)>,
    ) -> DurableCommandStatus {
        if !self.is_current(request.actor) {
            return DurableCommandStatus::Failed;
        }
        let Some(context) = self.durable.clone() else {
            return DurableCommandStatus::Failed;
        };
        if send_accepted
            && !self.send_to_actor(
                request.actor,
                ServerMessage::Accepted {
                    request_id: request.request_id.clone(),
                },
            )
        {
            return DurableCommandStatus::Failed;
        }
        let token = match self.routes.get(&request.actor.character) {
            Some(route) if route.actor == request.actor => route.session_token.clone(),
            _ => return DurableCommandStatus::Failed,
        };
        let checkpoint = self.runtime.snapshot();
        let random_checkpoint = self.random;
        let kind = match &request.command {
            ClientCommand::Look => worldwright_persistence::GameplayIntentKind::Look,
            ClientCommand::Move { .. } => worldwright_persistence::GameplayIntentKind::Move,
            ClientCommand::Say { .. } => worldwright_persistence::GameplayIntentKind::Say,
            ClientCommand::GameAction { .. } => {
                worldwright_persistence::GameplayIntentKind::GameAction
            }
            _ => return DurableCommandStatus::Failed,
        };
        let intent = match serde_json::to_vec(&request.command) {
            Ok(intent) => intent,
            Err(_) => {
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
        };
        debug_assert!(self.staged_messages.is_none());
        self.staged_messages = Some(prelude.clone());
        let execution = {
            let _planning_timing = self.timings.start(2);
            self.execute_command(request.actor, &request.request_id, request.command.clone())
        };
        let staged = self.staged_messages.take().unwrap_or_default();
        if let Err(error) = execution {
            self.random = random_checkpoint;
            if self.runtime.restore_checkpoint(checkpoint).is_err() {
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
            self.send_command_error(request.actor, request.request_id, error);
            return DurableCommandStatus::Failed;
        }
        let runtime_state = match self.runtime.snapshot().durable_state() {
            Ok(v) => v,
            Err(_) => {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
        };
        let snapshot = self.runtime.snapshot();
        let mut rooms = Vec::new();
        for character in snapshot.characters() {
            let Some(room) = snapshot.room_of(character) else {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            };
            let Some(definition) = self
                .lookup
                .room_ids
                .iter()
                .find_map(|(definition, id)| (*id == room).then(|| definition.to_string()))
            else {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            };
            rooms.push(worldwright_persistence::DurableCharacterRoom {
                character,
                room_definition: definition,
            });
        }
        let operation = match durable_operation_id() {
            Ok(v) => v,
            Err(_) => {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
        };
        let stored = StoredGameplayResult {
            messages: staged
                .iter()
                .filter(|(recipient, message)| {
                    *recipient == request.actor
                        && !matches!(message, ServerMessage::Attached { .. })
                })
                .map(|(_, message)| message.clone())
                .collect(),
        };
        if !stored.validate(&request.request_id) {
            let _ = self.runtime.restore_checkpoint(checkpoint);
            self.durable_failure(DisconnectReason::Internal);
            return DurableCommandStatus::Stop;
        }
        let result = match serde_json::to_value(stored) {
            Ok(result) => result,
            Err(_) => {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
        };
        let elapsed = match u64::try_from(self.started.elapsed().as_millis()) {
            Ok(elapsed) => elapsed,
            Err(_) => {
                let _ = self.runtime.restore_checkpoint(checkpoint);
                self.durable_failure(DisconnectReason::Internal);
                return DurableCommandStatus::Stop;
            }
        };
        let actor = DurableActor {
            world: context.world,
            account: request.actor.account,
            character: request.actor.character,
            token_hash: worldwright_persistence::session_token_hash(&token),
        };
        let sql_timing = self.timings.start(3);
        let commit = context
            .store
            .commit_gameplay(DurableGameplayMutation {
                actor,
                lease: context.lease,
                expected_release: context.release_digest,
                game: context.game,
                request_id: request.request_id.clone(),
                operation,
                kind,
                intent,
                expected_state_version: context.game_state_version,
                runtime: runtime_state,
                rooms,
                now: Tick::from_millis(elapsed),
                result,
            })
            .await;
        drop(sql_timing);
        match commit {
            Ok(commit) => {
                if commit.replayed {
                    self.random = random_checkpoint;
                    if self.runtime.restore_checkpoint(checkpoint).is_err() {
                        self.durable_failure(DisconnectReason::Internal);
                        return DurableCommandStatus::Stop;
                    }
                    let stored: StoredGameplayResult =
                        match serde_json::from_value::<StoredGameplayResult>(commit.result) {
                            Ok(stored) if stored.validate(&request.request_id) => stored,
                            _ => {
                                self.durable_failure(DisconnectReason::Internal);
                                return DurableCommandStatus::Stop;
                            }
                        };
                    for (recipient, message) in prelude {
                        let _ = self.send_to_actor(recipient, message);
                    }
                    for message in stored.messages {
                        let _ = self.send_to_actor(request.actor, message);
                    }
                } else {
                    if let Some(current) = self.durable.as_mut() {
                        current.game_state_version = commit.state_version;
                    }
                    for (recipient, message) in staged {
                        let _ = self.send_to_actor(recipient, message);
                    }
                }
                DurableCommandStatus::Succeeded
            }
            Err(error) => {
                self.random = random_checkpoint;
                let _ = self.runtime.restore_checkpoint(checkpoint);
                if self.map_durable_failure(request.actor, Some(request.request_id), error) {
                    DurableCommandStatus::Stop
                } else {
                    DurableCommandStatus::Failed
                }
            }
        }
    }

    fn send_command_error(
        &mut self,
        actor: ActorContext,
        request_id: String,
        error: CommandError,
    ) -> bool {
        match error {
            CommandError::NotFound => {
                let _ = self.send_error(
                    actor,
                    Some(request_id),
                    ErrorCode::NotFound,
                    "exit not found",
                    false,
                );
                false
            }
            CommandError::Runtime(error) => {
                let (code, message, fatal) = map_world_error(error);
                if fatal {
                    self.remove_route(actor, DisconnectReason::Protocol(code));
                } else {
                    let _ = self.send_error(actor, Some(request_id), code, message, false);
                }
                false
            }
        }
    }

    fn durable_failure(&mut self, reason: DisconnectReason) -> bool {
        self.shutdown_routes(reason);
        true
    }

    fn map_durable_failure(
        &mut self,
        actor: ActorContext,
        request_id: Option<String>,
        error: PersistenceError,
    ) -> bool {
        let (code, message, fatal) = match error {
            PersistenceError::ReceiptConflict => (
                ErrorCode::Conflict,
                "request ID conflicts with an earlier intent",
                false,
            ),
            PersistenceError::Unauthorized => (
                ErrorCode::Unauthenticated,
                "session expired or revoked",
                true,
            ),
            PersistenceError::StaleWriter => {
                return self.durable_failure(DisconnectReason::Protocol(ErrorCode::WriterFenced));
            }
            PersistenceError::UnknownOutcome => {
                return self
                    .durable_failure(DisconnectReason::Protocol(ErrorCode::OperationPending));
            }
            PersistenceError::Database(_)
            | PersistenceError::StaleGameState
            | PersistenceError::ReleaseConflict => {
                return self.durable_failure(DisconnectReason::Protocol(
                    ErrorCode::TemporarilyUnavailable,
                ));
            }
            _ => return self.durable_failure(DisconnectReason::Protocol(ErrorCode::InternalError)),
        };
        if fatal {
            self.remove_route(actor, DisconnectReason::Protocol(code));
        } else {
            let _ = self.send_to_actor(
                actor,
                ServerMessage::Error {
                    request_id,
                    code,
                    message: message.to_owned(),
                    fatal,
                },
            );
        }
        false
    }

    fn handle_gameplay_sync(&mut self, request: GameplayRequest) {
        if !self.is_current(request.actor) {
            return;
        }
        if !self.send_to_actor(
            request.actor,
            ServerMessage::Accepted {
                request_id: request.request_id.clone(),
            },
        ) {
            return;
        }
        match self.execute_command(request.actor, &request.request_id, request.command) {
            Ok(()) => {}
            Err(CommandError::NotFound) => {
                let _ = self.send_error(
                    request.actor,
                    Some(request.request_id),
                    ErrorCode::NotFound,
                    "exit not found",
                    false,
                );
            }
            Err(CommandError::Runtime(error)) => {
                let (code, message, fatal) = map_world_error(error);
                if fatal {
                    self.remove_route(request.actor, DisconnectReason::Protocol(code));
                } else {
                    let _ = self.send_error(
                        request.actor,
                        Some(request.request_id),
                        code,
                        message,
                        false,
                    );
                }
            }
        }
    }

    fn execute_command(
        &mut self,
        actor: ActorContext,
        request_id: &str,
        command: ClientCommand,
    ) -> Result<(), CommandError> {
        match command {
            ClientCommand::Look => Ok(self.look(actor, request_id)?),
            ClientCommand::Move { exit } => {
                let room = self.current_room(actor)?;
                let destination = self
                    .lookup
                    .named_exits
                    .get(&room)
                    .and_then(|exits| exits.get(&exit))
                    .copied()
                    .ok_or(CommandError::NotFound)?;
                let result = self.execute(actor, CommandIntent::Move { destination })?;
                self.deliver_result(actor, &result, OutputKind::Movement, Some(request_id));
                if self.is_current(actor) {
                    self.look(actor, request_id)?;
                }
                Ok(())
            }
            ClientCommand::Say { text } => {
                let text = PlayerText::new(&text).map_err(|_| WorldError::IntentViolation)?;
                let result = self.execute(actor, CommandIntent::Say { text })?;
                self.deliver_result(actor, &result, OutputKind::Speech, Some(request_id));
                if self.is_current(actor) {
                    let _ = self.send_to_actor(
                        actor,
                        ServerMessage::CommandResult {
                            request_id: request_id.to_owned(),
                            revision: result.revision,
                        },
                    );
                }
                Ok(())
            }
            ClientCommand::GameAction { action, argument } => {
                let action = action.parse().map_err(|_| WorldError::IntentViolation)?;
                let argument = argument
                    .as_deref()
                    .map(PlayerText::new)
                    .transpose()
                    .map_err(|_| WorldError::IntentViolation)?;
                let result = self.execute(actor, CommandIntent::GameAction { action, argument })?;
                self.deliver_result(actor, &result, OutputKind::System, Some(request_id));
                if self.is_current(actor) {
                    let _ = self.send_to_actor(
                        actor,
                        ServerMessage::CommandResult {
                            request_id: request_id.to_owned(),
                            revision: result.revision,
                        },
                    );
                }
                Ok(())
            }
            ClientCommand::Inventory
            | ClientCommand::ClaimReward { .. }
            | ClientCommand::TransferItem { .. } => Err(WorldError::IntentViolation.into()),
        }
    }

    fn look(&mut self, actor: ActorContext, request_id: &str) -> Result<(), WorldError> {
        let result = self.execute(actor, CommandIntent::Look)?;
        self.deliver_result(actor, &result, OutputKind::Room, Some(request_id));
        if !self.is_current(actor) {
            return Ok(());
        }
        let room = self.current_room(actor)?;
        let exits = self
            .lookup
            .named_exits
            .get(&room)
            .ok_or(WorldError::UnknownRoom)?
            .keys()
            .cloned()
            .collect();
        if self.is_current(actor) {
            let _ = self.send_to_actor(
                actor,
                ServerMessage::Exits {
                    request_id: Some(request_id.to_owned()),
                    exits,
                },
            );
        }
        if self.is_current(actor) {
            let _ = self.send_to_actor(
                actor,
                ServerMessage::CommandResult {
                    request_id: request_id.to_owned(),
                    revision: result.revision,
                },
            );
        }
        Ok(())
    }

    fn execute(
        &mut self,
        actor: ActorContext,
        intent: CommandIntent,
    ) -> Result<ExecutionResult, WorldError> {
        let operation = OperationId::from_u128(self.next_operation)
            .map_err(|_| WorldError::CounterExhausted)?;
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or(WorldError::CounterExhausted)?;
        let elapsed = u64::try_from(self.started.elapsed().as_millis())
            .map_err(|_| WorldError::CounterExhausted)?;
        let now = Tick::from_millis(elapsed);
        self.runtime.advance(now)?;
        self.runtime.execute(
            &CommandEnvelope {
                operation,
                world: self.world,
                actor,
                intent,
            },
            now,
            &mut self.random,
        )
    }

    fn current_room(&self, actor: ActorContext) -> Result<RoomId, WorldError> {
        if !self.is_current(actor) {
            return Err(WorldError::StaleAttachment);
        }
        self.runtime
            .room_of(actor.character)
            .ok_or(WorldError::UnknownCharacter)
    }

    fn deliver_result(
        &mut self,
        actor: ActorContext,
        result: &ExecutionResult,
        kind: OutputKind,
        request_id: Option<&str>,
    ) {
        for message in &result.messages {
            let addressed_request_id = if message.recipient == actor {
                request_id.map(str::to_owned)
            } else {
                None
            };
            let speaker = if kind == OutputKind::Speech {
                self.names.get(&message.source).cloned()
            } else {
                None
            };
            let _ = self.send_to_actor(
                message.recipient,
                ServerMessage::Output {
                    request_id: addressed_request_id,
                    kind,
                    speaker,
                    text: message.text.as_str().to_owned(),
                },
            );
        }
    }

    fn send_error(
        &mut self,
        actor: ActorContext,
        request_id: Option<String>,
        code: ErrorCode,
        message: &str,
        fatal: bool,
    ) -> bool {
        self.send_to_actor(
            actor,
            ServerMessage::Error {
                request_id,
                code,
                message: message.to_owned(),
                fatal,
            },
        )
    }

    fn send_to_actor(&mut self, actor: ActorContext, message: ServerMessage) -> bool {
        let _delivery_timing = self.timings.start(4);
        let Some(route) = self.routes.get(&actor.character) else {
            return false;
        };
        if route.actor != actor
            || self.runtime.attachment(actor.character) != Some(actor)
        {
            return false;
        }
        if let Some(staged) = self.staged_messages.as_mut() {
            staged.push((actor, message));
            return true;
        }
        let sender = route.outbound.clone();
        match sender.try_send(Outbound::Message(message)) {
            Ok(()) => true,
            Err(_) => {
                self.remove_route(actor, DisconnectReason::SlowConsumer);
                false
            }
        }
    }

    fn is_current(&self, actor: ActorContext) -> bool {
        self.routes
            .get(&actor.character)
            .is_some_and(|route| route.actor == actor)
            && self.runtime.attachment(actor.character) == Some(actor)
    }

    fn remove_route(&mut self, actor: ActorContext, reason: DisconnectReason) {
        if self
            .routes
            .get(&actor.character)
            .is_some_and(|route| route.actor == actor)
        {
            if let Some(route) = self.routes.remove(&actor.character) {
                cancel_first(&route.cancel, reason);
                // The route map owns the one connected-client gauge decrement.
                self.connected_clients.fetch_sub(1, Ordering::Relaxed);
            }
            if self.runtime.attachment(actor.character) == Some(actor) {
                let _ = self.runtime.detach(actor);
            }
        }
    }

    fn remove_route_without_detach(&mut self, actor: ActorContext, reason: DisconnectReason) {
        if let Some(route) = self.routes.remove(&actor.character) {
            cancel_first(&route.cancel, reason);
            self.connected_clients.fetch_sub(1, Ordering::Relaxed);
        }
    }
}

enum CommandError {
    Runtime(WorldError),
    NotFound,
}

impl From<WorldError> for CommandError {
    fn from(error: WorldError) -> Self {
        Self::Runtime(error)
    }
}

fn durable_operation_id() -> Result<OperationId, ()> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ())?;
        if let Ok(id) = OperationId::from_u128(u128::from_be_bytes(bytes)) {
            return Ok(id);
        }
    }
    Err(())
}
fn durable_ids() -> Result<(OperationId, worldwright_core::ids::ItemId), ()> {
    Ok((durable_operation_id()?, durable_item_id()?))
}
fn durable_item_id() -> Result<worldwright_core::ids::ItemId, ()> {
    for _ in 0..8 {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| ())?;
        if let Ok(id) = worldwright_core::ids::ItemId::from_u128(u128::from_be_bytes(bytes)) {
            return Ok(id);
        }
    }
    Err(())
}
fn map_reward_outcome(value: RewardOutcome) -> worldwright_protocol::RewardOutcome {
    match value {
        RewardOutcome::RewardGranted => worldwright_protocol::RewardOutcome::RewardGranted,
        RewardOutcome::AlreadyClaimed => worldwright_protocol::RewardOutcome::AlreadyClaimed,
        RewardOutcome::InventoryFull => worldwright_protocol::RewardOutcome::InventoryFull,
    }
}
fn map_transfer_outcome(value: TransferOutcome) -> worldwright_protocol::TransferOutcome {
    match value {
        TransferOutcome::ItemTransferred => worldwright_protocol::TransferOutcome::ItemTransferred,
        TransferOutcome::NotOwned => worldwright_protocol::TransferOutcome::NotOwned,
        TransferOutcome::SameOwner => worldwright_protocol::TransferOutcome::SameOwner,
        TransferOutcome::TargetNotFound => worldwright_protocol::TransferOutcome::TargetNotFound,
        TransferOutcome::InventoryFull => worldwright_protocol::TransferOutcome::InventoryFull,
    }
}

fn map_world_error(error: WorldError) -> (ErrorCode, &'static str, bool) {
    match error {
        WorldError::NotAttached | WorldError::StaleAttachment | WorldError::OwnerMismatch => {
            (ErrorCode::StaleAttachment, "stale attachment", true)
        }
        WorldError::Unreachable
        | WorldError::IntentViolation
        | WorldError::MissingMovement
        | WorldError::Rule(_) => (ErrorCode::InvalidCommand, "invalid command", false),
        _ => (ErrorCode::InternalError, "internal server error", true),
    }
}

#[derive(Clone, Copy)]
enum OwnerRandom {
    Unavailable,
    Seeded(u64),
}

impl OwnerRandom {
    fn seeded() -> Option<Self> {
        let mut bytes = [0u8; 8];
        getrandom::fill(&mut bytes).ok()?;
        let seed = u64::from_be_bytes(bytes);
        Some(Self::Seeded(if seed == 0 { 1 } else { seed }))
    }
}

impl RandomSource for OwnerRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        match self {
            Self::Unavailable => Err(RandomExhausted),
            Self::Seeded(state) => {
                let mut value = *state;
                value ^= value >> 12;
                value ^= value << 25;
                value ^= value >> 27;
                *state = value;
                Ok(value.wrapping_mul(0x2545_f491_4f6c_dd1d))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::watch;
    use worldwright_content::load_bytes;

    fn principal(account: u128, character: u128, name: &str) -> crate::auth::Principal {
        crate::auth::Principal {
            account: worldwright_core::ids::AccountId::from_u128(account).expect("account"),
            character: worldwright_core::ids::CharacterId::from_u128(character).expect("character"),
            character_name: name.to_owned(),
        }
    }

    fn service() -> (WorldHandle, crate::auth::Principal, crate::auth::Principal) {
        let content = load_bytes(
            "test",
            include_bytes!("../../../examples/two-room/world.json"),
            Default::default(),
        )
        .expect("content");
        let alice = principal(1, 1, "Alice");
        let bob = principal(2, 2, "Bob");
        let service = WorldHandle::new(content, vec![alice.clone(), bob.clone()]).expect("world");
        (service, alice, bob)
    }

    fn expedition_service() -> (WorldHandle, crate::auth::Principal) {
        let content = load_bytes(
            "expedition-test",
            include_bytes!("../../../examples/expedition/world.json"),
            Default::default(),
        )
        .expect("content");
        let alice = principal(1, 1, "Alice");
        let registrations = vec![crate::auth::CharacterRegistration {
            principal: alice.clone(),
            room_definition: None,
        }];
        let service = WorldHandle::new_with_profile(
            content,
            registrations,
            None,
            crate::GameProfile::Expedition,
            None,
        )
        .expect("expedition world");
        (service, alice)
    }

    async fn initial(
        service: &WorldHandle,
        principal: crate::auth::Principal,
        connection: u128,
        token: &str,
    ) -> (
        ActorContext,
        mpsc::Receiver<Outbound>,
        watch::Receiver<Option<DisconnectReason>>,
    ) {
        let (sender, receiver) = mpsc::channel(64);
        let (cancel, cancel_receiver) = watch::channel(None);
        let actor = service
            .attach(
                principal,
                token.to_owned(),
                ConnectionId::from_u128(connection).expect("connection"),
                sender,
                cancel,
            )
            .await
            .expect("attach");
        service
            .initial(actor, "attach-1".to_owned())
            .await
            .expect("initial");
        (actor, receiver, cancel_receiver)
    }

    #[tokio::test]
    async fn closed_attach_response_does_not_leave_a_ghost_route() {
        let (service, alice, _) = service();
        let (outbound, _receiver) = mpsc::channel(8);
        let (cancel, cancel_receiver) = watch::channel(None);
        let (response, receiver) = oneshot::channel();
        drop(receiver);
        service
            .control
            .send(ControlRequest::Attach {
                principal: alice,
                session_token: "ghost-token".to_owned(),
                connection: ConnectionId::from_u128(61).expect("connection"),
                outbound,
                cancel,
                response,
            })
            .await
            .expect("control");
        service.revoke_session("ghost-token".to_owned()).await;
        assert_eq!(*cancel_receiver.borrow(), None);
        service.shutdown().await;
    }

    #[test]
    fn stored_gameplay_results_require_actor_request_ids_and_allowed_messages() {
        let valid = StoredGameplayResult {
            messages: vec![ServerMessage::CommandResult {
                request_id: "r".into(),
                revision: 1,
            }],
        };
        assert!(valid.validate("r"));
        assert!(
            !StoredGameplayResult {
                messages: vec![ServerMessage::Pong { nonce: 1 }]
            }
            .validate("r")
        );
        assert!(
            !StoredGameplayResult {
                messages: vec![ServerMessage::Output {
                    request_id: None,
                    kind: OutputKind::System,
                    speaker: None,
                    text: "x".into()
                }]
            }
            .validate("r")
        );
    }

    #[test]
    fn stored_gameplay_results_bound_message_count() {
        let messages = (0..65)
            .map(|_| ServerMessage::CommandResult {
                request_id: "r".into(),
                revision: 1,
            })
            .collect();
        assert!(!StoredGameplayResult { messages }.validate("r"));
    }

    #[tokio::test]
    async fn closed_initial_response_cancels_route_before_reattachment() {
        let (service, alice, _) = service();
        let (actor, _outbound, cancel_receiver) =
            initial(&service, alice.clone(), 62, "initial-token").await;
        let (response, receiver) = oneshot::channel();
        drop(receiver);
        service
            .control
            .send(ControlRequest::Initial {
                actor,
                request_id: "closed-initial".to_owned(),
                response,
            })
            .await
            .expect("control");
        service.revoke_session("initial-token".to_owned()).await;
        assert_eq!(*cancel_receiver.borrow(), Some(DisconnectReason::Normal));
        let (replacement_sender, _replacement_receiver) = mpsc::channel(8);
        let (replacement_cancel, _replacement_cancel_receiver) = watch::channel(None);
        assert!(
            service
                .attach(
                    alice,
                    "replacement-token".to_owned(),
                    ConnectionId::from_u128(63).expect("connection"),
                    replacement_sender,
                    replacement_cancel,
                )
                .await
                .is_ok()
        );
        service.shutdown().await;
    }

    #[test]
    fn cancellation_keeps_the_first_reason() {
        let (sender, receiver) = watch::channel(None);
        cancel_first(&sender, DisconnectReason::SessionReplaced);
        cancel_first(&sender, DisconnectReason::SlowConsumer);
        assert_eq!(*receiver.borrow(), Some(DisconnectReason::SessionReplaced));
    }

    #[test]
    fn random_source_is_exhausted() {
        assert!(OwnerRandom::Unavailable.next_u64().is_err());
    }

    #[tokio::test]
    async fn two_accounts_receive_initial_look_and_room_speech() {
        let (service, alice, bob) = service();
        let (alice_actor, mut alice_out, _) = initial(&service, alice, 11, "token-a").await;
        let (_bob_actor, mut bob_out, _) = initial(&service, bob, 12, "token-b").await;
        for _ in 0..5 {
            assert!(alice_out.recv().await.is_some());
            assert!(bob_out.recv().await.is_some());
        }
        service
            .submit(
                alice_actor,
                "say-1".to_owned(),
                ClientCommand::Say {
                    text: "hello".to_owned(),
                },
            )
            .expect("submit");
        assert!(matches!(
            alice_out.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { request_id })) if request_id == "say-1"
        ));
        assert!(matches!(
            alice_out.recv().await,
            Some(Outbound::Message(ServerMessage::Output { kind: OutputKind::Speech, speaker: Some(speaker), request_id: Some(_), .. })) if speaker == "Alice"
        ));
        assert!(matches!(
            bob_out.recv().await,
            Some(Outbound::Message(ServerMessage::Output { kind: OutputKind::Speech, speaker: Some(speaker), request_id: None, .. })) if speaker == "Alice"
        ));
        assert!(matches!(
            alice_out.recv().await,
            Some(Outbound::Message(ServerMessage::CommandResult { .. }))
        ));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn generic_game_action_runs_through_expedition_owner() {
        let (service, alice) = expedition_service();
        let (actor, mut output, _) = initial(&service, alice, 81, "expedition-token").await;
        while output.try_recv().is_ok() {}

        service
            .submit(
                actor,
                "rest-1".to_owned(),
                ClientCommand::GameAction {
                    action: worldwright_expedition::REST_ACTION.to_owned(),
                    argument: None,
                },
            )
            .expect("submit");
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { request_id }))
                if request_id == "rest-1"
        ));
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Output {
                request_id: Some(request_id),
                kind: OutputKind::System,
                ..
            })) if request_id == "rest-1"
        ));
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::CommandResult { request_id, .. }))
                if request_id == "rest-1"
        ));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn slow_bob_does_not_fail_alice_after_successful_say() {
        let (service, alice, bob) = service();
        let (alice_actor, mut alice_out, _) = initial(&service, alice, 41, "token-a").await;
        let (bob_sender, mut bob_out) = mpsc::channel(64);
        let (bob_cancel, mut bob_cancel_receiver) = watch::channel(None);
        let bob_actor = service
            .attach(
                bob,
                "token-b".to_owned(),
                ConnectionId::from_u128(42).expect("connection"),
                bob_sender.clone(),
                bob_cancel,
            )
            .await
            .expect("attach");
        service
            .initial(bob_actor, "attach-b".to_owned())
            .await
            .expect("initial");
        for _ in 0..5 {
            assert!(alice_out.recv().await.is_some());
            assert!(bob_out.recv().await.is_some());
        }
        for _ in 0..64 {
            bob_sender
                .try_send(Outbound::Pong(Vec::new()))
                .expect("fill bob");
        }
        service
            .submit(
                alice_actor,
                "say-slow".to_owned(),
                ClientCommand::Say {
                    text: "hello".to_owned(),
                },
            )
            .expect("submit");
        assert!(matches!(
            alice_out.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { .. }))
        ));
        assert!(matches!(
            alice_out.recv().await,
            Some(Outbound::Message(ServerMessage::Output {
                kind: OutputKind::Speech,
                ..
            }))
        ));
        assert!(
            matches!(alice_out.recv().await, Some(Outbound::Message(ServerMessage::CommandResult { request_id, .. })) if request_id == "say-slow")
        );
        assert_eq!(
            *bob_cancel_receiver.borrow_and_update(),
            Some(DisconnectReason::SlowConsumer)
        );
        service
            .submit(alice_actor, "look-after".to_owned(), ClientCommand::Look)
            .expect("alice remains attached");
        assert!(
            matches!(alice_out.recv().await, Some(Outbound::Message(ServerMessage::Accepted { request_id })) if request_id == "look-after")
        );
        service.shutdown().await;
    }

    #[tokio::test]
    async fn revoking_a_session_cancels_only_its_route() {
        let (service, alice, bob) = service();
        let (_alice_actor, _alice_out, mut alice_cancel) =
            initial(&service, alice, 51, "token-a").await;
        let (_bob_actor, _bob_out, bob_cancel) = initial(&service, bob, 52, "token-b").await;
        service.revoke_session("token-a".to_owned()).await;
        assert_eq!(
            *alice_cancel.borrow_and_update(),
            Some(DisconnectReason::SessionRevoked)
        );
        assert_eq!(*bob_cancel.borrow(), None);
        service.shutdown().await;
    }

    #[tokio::test]
    async fn connection_collision_preserves_alices_route() {
        let (service, alice, bob) = service();
        let (alice_actor, mut alice_out, alice_cancel) =
            initial(&service, alice, 71, "token-a").await;
        let (_bob_actor, mut bob_out, _bob_cancel) = initial(&service, bob, 72, "token-b").await;
        for _ in 0..5 {
            assert!(alice_out.recv().await.is_some());
            assert!(bob_out.recv().await.is_some());
        }
        let (replacement_sender, _replacement_receiver) = mpsc::channel(8);
        let (replacement_cancel, _replacement_cancel_receiver) = watch::channel(None);
        assert!(matches!(
            service
                .attach(
                    principal(1, 1, "Alice"),
                    "collision-token".to_owned(),
                    ConnectionId::from_u128(72).expect("connection"),
                    replacement_sender,
                    replacement_cancel,
                )
                .await,
            Err(AttachError::ConnectionInUse)
        ));
        assert_eq!(*alice_cancel.borrow(), None);
        service
            .submit(
                alice_actor,
                "look-collision".to_owned(),
                ClientCommand::Look,
            )
            .expect("alice remains attached");
        assert!(
            matches!(alice_out.recv().await, Some(Outbound::Message(ServerMessage::Accepted { request_id })) if request_id == "look-collision")
        );
        service.shutdown().await;
    }

    #[tokio::test]
    async fn move_resolves_named_exit_and_emits_a_follow_up_look() {
        let (service, alice, _) = service();
        let (actor, mut output, _) = initial(&service, alice, 16, "token-a").await;
        for _ in 0..5 {
            assert!(output.recv().await.is_some());
        }
        service
            .submit(
                actor,
                "move-1".to_owned(),
                ClientCommand::Move {
                    exit: "north".to_owned(),
                },
            )
            .expect("move");
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { .. }))
        ));
        let mut movement = false;
        let mut room = false;
        let mut exits = false;
        let mut result = false;
        for _ in 0..5 {
            match output.recv().await {
                Some(Outbound::Message(ServerMessage::Output {
                    kind: OutputKind::Movement,
                    ..
                })) => movement = true,
                Some(Outbound::Message(ServerMessage::Output {
                    kind: OutputKind::Room,
                    ..
                })) => room = true,
                Some(Outbound::Message(ServerMessage::Exits { .. })) => exits = true,
                Some(Outbound::Message(ServerMessage::CommandResult { request_id, .. }))
                    if request_id == "move-1" =>
                {
                    result = true
                }
                _ => {}
            }
        }
        assert!(movement && room && exits && result);
        service.shutdown().await;
    }

    #[tokio::test]
    async fn unknown_exit_is_not_found_without_detaching_actor() {
        let (service, alice, _) = service();
        let (actor, mut output, _) = initial(&service, alice, 81, "token-a").await;
        for _ in 0..5 {
            assert!(output.recv().await.is_some());
        }
        service
            .submit(
                actor,
                "go-nowhere".to_owned(),
                ClientCommand::Move {
                    exit: "west".to_owned(),
                },
            )
            .expect("submit");
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { .. }))
        ));
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Error { code: ErrorCode::NotFound, message, fatal: false, .. })) if message == "exit not found"
        ));
        service
            .submit(
                actor,
                "look-after-not-found".to_owned(),
                ClientCommand::Look,
            )
            .expect("actor remains attached");
        assert!(matches!(
            output.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { .. }))
        ));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn takeover_makes_old_actor_stale_without_detaching_new_actor() {
        let (service, alice, _) = service();
        let (old_actor, mut old_out, old_cancel) =
            initial(&service, alice.clone(), 21, "old-token").await;
        for _ in 0..5 {
            assert!(old_out.recv().await.is_some());
        }
        let (new_actor, mut new_out, _) = initial(&service, alice, 22, "new-token").await;
        for _ in 0..5 {
            assert!(new_out.recv().await.is_some());
        }
        assert_ne!(old_actor, new_actor);
        assert_eq!(
            *old_cancel.borrow(),
            Some(DisconnectReason::SessionReplaced)
        );
        service.detach(old_actor).await;
        service
            .submit(new_actor, "look-1".to_owned(), ClientCommand::Look)
            .expect("new actor remains attached");
        assert!(matches!(
            new_out.recv().await,
            Some(Outbound::Message(ServerMessage::Accepted { .. }))
        ));
        service.shutdown().await;
    }

    #[tokio::test]
    async fn full_outbound_route_is_removed_and_cancelled() {
        let (service, alice, _) = service();
        let (sender, _receiver) = mpsc::channel(1);
        let (cancel, mut cancel_receiver) = watch::channel(None);
        let actor = service
            .attach(
                alice,
                "token-a".to_owned(),
                ConnectionId::from_u128(31).expect("connection"),
                sender,
                cancel,
            )
            .await
            .expect("attach");
        assert!(service.initial(actor, "attach-1".to_owned()).await.is_err());
        assert_eq!(
            *cancel_receiver.borrow_and_update(),
            Some(DisconnectReason::SlowConsumer)
        );
        let (replacement_sender, _replacement_receiver) = mpsc::channel(8);
        let (replacement_cancel, _replacement_cancel_receiver) = watch::channel(None);
        assert!(
            service
                .attach(
                    principal(1, 1, "Alice"),
                    "replacement-token".to_owned(),
                    ConnectionId::from_u128(32).expect("connection"),
                    replacement_sender,
                    replacement_cancel,
                )
                .await
                .is_ok()
        );
        service.shutdown().await;
    }
}
