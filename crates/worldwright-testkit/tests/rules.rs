use worldwright_sdk::{
    GameModule, WorldView,
    command::{
        ActorContext, CommandEnvelope, CommandIntent, CommandKind, ConnectionGeneration, PlayerText,
    },
    effect::{Audience, Effect, EffectPlan},
    ids::{AccountId, CharacterId, ConnectionId, OperationId, RoomId, WorldId},
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    time::{Clock, RandomSource, Tick, TimeError},
};
use worldwright_testkit::{FakeClock, SequenceRandom, WorldFixture};

struct LookRule;
impl Rule for LookRule {
    fn evaluate(
        &self,
        ctx: &RuleContext<'_>,
        command: &CommandIntent,
        _rng: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        if command != &CommandIntent::Look {
            return Err(RuleError::Rejected);
        }
        let room = ctx
            .world
            .character_room(ctx.actor.character)
            .ok_or(RuleError::Rejected)?;
        let mut plan = EffectPlan::new();
        plan.push(Effect::EmitMessage {
            audience: Audience::Character(ctx.actor.character),
            text: PlayerText::new(&format!("Room {room}")).unwrap(),
        })
        .map_err(|_| RuleError::PlanLimit)?;
        Ok(plan)
    }
}

fn fixture() -> (CommandEnvelope, WorldFixture) {
    let world = WorldId::from_u128(1).unwrap();
    let character = CharacterId::from_u128(1).unwrap();
    let room = RoomId::from_u128(1).unwrap();
    let command = CommandEnvelope {
        operation: OperationId::from_u128(1).unwrap(),
        world,
        actor: ActorContext {
            account: AccountId::from_u128(1).unwrap(),
            character,
            connection: ConnectionId::from_u128(1).unwrap(),
            generation: ConnectionGeneration::new(1).unwrap(),
        },
        intent: CommandIntent::Look,
    };
    let mut view = WorldFixture::new(world);
    view.add_room(room);
    assert!(view.place_character(character, room));
    (command, view)
}

#[test]
fn sdk_rule_registration_and_planning_do_not_mutate_world() {
    let (command, view) = fixture();
    let mut registry = RuleRegistry::new();
    registry.register(CommandKind::Look, LookRule).unwrap();
    assert!(registry.register(CommandKind::Look, LookRule).is_err());
    let before = view.character_room(command.actor.character);
    let plan = registry
        .plan(&command, &view, Tick::ZERO, &mut SequenceRandom::default())
        .unwrap();
    assert_eq!(plan.effects().len(), 1);
    assert_eq!(view.character_room(command.actor.character), before);
}

#[test]
fn wrong_world_and_missing_rule_fail_before_evaluation() {
    let (mut command, view) = fixture();
    let registry = RuleRegistry::new();
    let mut rng = SequenceRandom::default();
    assert_eq!(
        registry.plan(&command, &view, Tick::ZERO, &mut rng),
        Err(RuleError::MissingRule)
    );
    command.world = WorldId::from_u128(2).unwrap();
    assert_eq!(
        registry.plan(&command, &view, Tick::ZERO, &mut rng),
        Err(RuleError::WrongWorld)
    );
}

#[test]
fn fake_clock_rejects_regression_and_overflow_without_mutation() {
    let mut clock = FakeClock::new(Tick::from_millis(10));
    clock.advance(5).unwrap();
    assert_eq!(clock.now().as_millis(), 15);
    assert_eq!(clock.set(Tick::ZERO), Err(TimeError::WentBackwards));
    assert_eq!(clock.now().as_millis(), 15);
    assert_eq!(clock.advance(u64::MAX), Err(TimeError::Overflow));
    assert_eq!(clock.now().as_millis(), 15);
}

#[test]
fn sequence_rng_is_deterministic_and_exhaustion_is_visible() {
    let mut rng = SequenceRandom::new([7, 9]);
    assert_eq!(rng.next_u64().unwrap(), 7);
    assert_eq!(rng.next_u64().unwrap(), 9);
    assert!(rng.next_u64().is_err());
}

#[test]
fn metadata_only_game_does_not_claim_rule_registration() {
    struct MetadataOnly;
    impl GameModule for MetadataOnly {
        fn name(&self) -> &str {
            "metadata"
        }
    }
    assert!(
        MetadataOnly
            .register_rules(&mut RuleRegistry::new())
            .is_err()
    );
}
