use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    sync::{mpsc, oneshot, watch},
    time::timeout,
};
use worldwright_content::{ValidatedWorldContent, WorldLookup, instantiate};
use worldwright_core::{
    command::{ActorContext, CommandEnvelope, CommandIntent, PlayerText},
    ids::{CharacterId, ConnectionId, OperationId, RoomId, WorldId},
    rule::RuleRegistry,
    runtime::{ExecutionResult, ReferenceGame, WorldError, WorldRuntime},
    time::{RandomExhausted, RandomSource, Tick},
};
use worldwright_protocol::{ClientCommand, ErrorCode, OutputKind, ServerMessage};

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

#[derive(Clone)]
pub(crate) struct WorldHandle {
    gameplay: mpsc::Sender<GameplayRequest>,
    control: mpsc::Sender<ControlRequest>,
    ready: Arc<AtomicBool>,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubmitError {
    Full,
    Closed,
}

struct GameplayRequest {
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
    next_operation: u128,
    started: Instant,
}

impl WorldHandle {
    pub(crate) fn new(
        content: ValidatedWorldContent,
        principals: Vec<crate::auth::Principal>,
    ) -> Result<Self, crate::ServerError> {
        let world = WorldId::from_u128(1)
            .map_err(|_| crate::ServerError::startup("world ID initialization failed"))?;
        let (mut builder, lookup) = instantiate(&content, world)
            .map_err(|_| crate::ServerError::startup("world content instantiation failed"))?
            .into_parts();
        let mut names = HashMap::with_capacity(principals.len());
        for principal in principals {
            builder
                .add_character(principal.character, principal.account, lookup.entry_room)
                .map_err(|_| {
                    crate::ServerError::startup("world character initialization failed")
                })?;
            names.insert(principal.character, principal.character_name);
        }
        let mut rules = RuleRegistry::new();
        worldwright_core::game::GameModule::register_rules(&ReferenceGame, &mut rules)
            .map_err(|_| crate::ServerError::startup("reference game registration failed"))?;
        let runtime = builder
            .build(rules)
            .map_err(|_| crate::ServerError::startup("world runtime initialization failed"))?;
        let (gameplay, gameplay_receiver) = mpsc::channel(GAMEPLAY_CAPACITY);
        let (control, control_receiver) = mpsc::channel(CONTROL_CAPACITY);
        let ready = Arc::new(AtomicBool::new(true));
        let owner = Owner {
            runtime,
            lookup,
            world,
            names,
            routes: HashMap::new(),
            gameplay: gameplay_receiver,
            control: control_receiver,
            _ready_guard: ReadyGuard(ready.clone()),
            next_operation: 1,
            started: Instant::now(),
        };
        tokio::spawn(owner.run());
        Ok(Self {
            gameplay,
            control,
            ready,
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
        self.gameplay
            .try_send(GameplayRequest {
                actor,
                request_id,
                command,
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => SubmitError::Full,
                mpsc::error::TrySendError::Closed(_) => SubmitError::Closed,
            })
    }

    pub(crate) fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
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
        loop {
            tokio::select! {
                biased;
                Some(request) = self.control.recv() => {
                    if self.handle_control(request).await { break; }
                }
                Some(request) = self.gameplay.recv() => self.handle_gameplay(request),
                else => break,
            }
        }
        self.shutdown_routes(DisconnectReason::Shutdown);
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
        Ok(ActorContextResult { actor })
    }

    fn initial(&mut self, actor: ActorContext, request_id: &str) -> Result<(), AttachError> {
        let attached = ServerMessage::Attached {
            request_id: request_id.to_owned(),
            character_name: self
                .names
                .get(&actor.character)
                .cloned()
                .unwrap_or_else(|| "Player".to_owned()),
        };
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

    fn detach(&mut self, actor: ActorContext) {
        self.remove_route(actor, DisconnectReason::Normal);
    }

    fn shutdown_routes(&mut self, reason: DisconnectReason) {
        let actors: Vec<ActorContext> = self.routes.values().map(|route| route.actor).collect();
        for actor in actors {
            self.remove_route(actor, reason);
        }
    }

    fn handle_gameplay(&mut self, request: GameplayRequest) {
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
        self.runtime.execute(
            &CommandEnvelope {
                operation,
                world: self.world,
                actor,
                intent,
            },
            Tick::from_millis(elapsed),
            &mut NoRandom,
        )
    }

    fn current_room(&self, actor: ActorContext) -> Result<RoomId, WorldError> {
        if !self.is_current(actor) {
            return Err(WorldError::StaleAttachment);
        }
        self.runtime
            .snapshot()
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
        let Some(route) = self.routes.get(&actor.character) else {
            return false;
        };
        if route.actor != actor
            || self.runtime.snapshot().attachment(actor.character) != Some(actor)
        {
            return false;
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
            && self.runtime.snapshot().attachment(actor.character) == Some(actor)
    }

    fn remove_route(&mut self, actor: ActorContext, reason: DisconnectReason) {
        if self
            .routes
            .get(&actor.character)
            .is_some_and(|route| route.actor == actor)
        {
            if let Some(route) = self.routes.remove(&actor.character) {
                cancel_first(&route.cancel, reason);
            }
            if self.runtime.snapshot().attachment(actor.character) == Some(actor) {
                let _ = self.runtime.detach(actor);
            }
        }
    }

    fn remove_route_without_detach(&mut self, actor: ActorContext, reason: DisconnectReason) {
        if let Some(route) = self.routes.remove(&actor.character) {
            cancel_first(&route.cancel, reason);
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

struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        Err(RandomExhausted)
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
        assert!(NoRandom.next_u64().is_err());
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
