pub use worldwright_sdk::{
    GameModule,
    command::{
        ActorContext, CommandEnvelope, CommandIntent, CommandKind, ConnectionGeneration, PlayerText,
    },
    effect::{Audience, DomainEvent, Effect, EffectPlan, ItemLocation, ScheduledAction},
    ids::{AccountId, CharacterId, ConnectionId, ItemId, OperationId, RoomId, WorldId},
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    runtime::{ExecutionResult, ReferenceGame, WorldBuilder, WorldError, WorldRuntime},
    time::{RandomSource, Tick},
};
pub use worldwright_testkit::SequenceRandom;

pub fn room(n: u128) -> RoomId {
    RoomId::from_u128(n).unwrap()
}
pub fn character(n: u128) -> CharacterId {
    CharacterId::from_u128(n).unwrap()
}
pub fn account(n: u128) -> AccountId {
    AccountId::from_u128(n).unwrap()
}
pub fn connection(n: u128) -> ConnectionId {
    ConnectionId::from_u128(n).unwrap()
}
pub fn text(value: &str) -> PlayerText {
    PlayerText::new(value).unwrap()
}

pub struct Harness {
    pub runtime: WorldRuntime,
    pub alice: ActorContext,
    pub bob: ActorContext,
}
impl Harness {
    pub fn new(registry: RuleRegistry, reverse: bool) -> Self {
        let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
        for (n, title) in [(1, "Atrium"), (2, "Observatory"), (3, "Hidden vault")] {
            builder
                .add_room(room(n), text(title), text("A test room."))
                .unwrap();
        }
        builder.add_exit(room(1), room(2)).unwrap();
        if reverse {
            builder.add_exit(room(2), room(1)).unwrap();
        }
        builder
            .add_character(character(1), account(1), room(1))
            .unwrap();
        builder
            .add_character(character(2), account(2), room(2))
            .unwrap();
        let mut runtime = builder.build(registry).unwrap();
        let alice = runtime
            .attach(account(1), character(1), connection(1))
            .unwrap();
        let bob = runtime
            .attach(account(2), character(2), connection(2))
            .unwrap();
        Self {
            runtime,
            alice,
            bob,
        }
    }
    pub fn reference() -> Self {
        let mut registry = RuleRegistry::new();
        ReferenceGame.register_rules(&mut registry).unwrap();
        Self::new(registry, true)
    }
    pub fn scripted(kind: CommandKind, effects: Vec<Effect>) -> Self {
        let mut registry = RuleRegistry::new();
        registry.register(kind, Scripted(effects)).unwrap();
        Self::new(registry, true)
    }
    pub fn envelope(&self, intent: CommandIntent) -> CommandEnvelope {
        CommandEnvelope {
            operation: OperationId::from_u128(1).unwrap(),
            world: self.runtime.snapshot().world_id(),
            actor: self.alice,
            intent,
        }
    }
    pub fn execute(&mut self, intent: CommandIntent) -> Result<ExecutionResult, WorldError> {
        let command = self.envelope(intent);
        self.runtime
            .execute(&command, Tick::ZERO, &mut SequenceRandom::new([]))
    }
    pub fn reject_unchanged(&mut self, command: CommandEnvelope, expected: WorldError) {
        let before = self.runtime.snapshot();
        assert_eq!(
            self.runtime
                .execute(&command, Tick::ZERO, &mut SequenceRandom::new([])),
            Err(expected)
        );
        assert_eq!(self.runtime.snapshot(), before);
    }
}

pub struct Scripted(pub Vec<Effect>);
impl Rule for Scripted {
    fn evaluate(
        &self,
        _: &RuleContext<'_>,
        _: &CommandIntent,
        _: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        let mut plan = EffectPlan::new();
        for effect in &self.0 {
            plan.push(effect.clone())
                .map_err(|_| RuleError::PlanLimit)?;
        }
        Ok(plan)
    }
}
pub fn movement() -> Effect {
    Effect::MoveCharacter {
        character: character(1),
        destination: room(2),
    }
}
pub fn message(audience: Audience) -> Effect {
    Effect::EmitMessage {
        audience,
        text: text("Message must not leak on rejection."),
    }
}
