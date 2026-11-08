use worldwright_core::{
    GameModule,
    command::{
        ActorContext, CommandEnvelope, CommandIntent, CommandKind, ConnectionGeneration, PlayerText,
    },
    effect::{Effect, EffectPlan, ScheduledAction},
    ids::{AccountId, CharacterId, ConnectionId, DefinitionId, OperationId, RoomId, WorldId},
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    runtime::{MAX_COUNTER_VALUE, WorldBuilder, WorldError, WorldRuntime},
    time::{RandomExhausted, RandomSource, Tick},
};

struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        Err(RandomExhausted)
    }
}

struct ActionRule {
    other: CharacterId,
}

impl Rule for ActionRule {
    fn evaluate(
        &self,
        context: &RuleContext<'_>,
        command: &CommandIntent,
        _random: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        let CommandIntent::GameAction { action, .. } = command else {
            return Err(RuleError::Rejected);
        };
        let key = DefinitionId::parse("test:state.value").map_err(|_| RuleError::Rejected)?;
        let effect = match action.as_str() {
            "test:actions.schedule" => Effect::Schedule {
                at: Tick::from_millis(10),
                action: ScheduledAction::AdjustCounter {
                    character: context.actor.character,
                    key,
                    delta: 1,
                },
            },
            "test:actions.foreign" => Effect::AdjustCounter {
                character: self.other,
                key,
                delta: 1,
            },
            _ => return Err(RuleError::Rejected),
        };
        let mut plan = EffectPlan::new();
        plan.push(effect).map_err(|_| RuleError::PlanLimit)?;
        Ok(plan)
    }
}

struct Game;
impl GameModule for Game {
    fn name(&self) -> &str {
        "test"
    }
    fn register_rules(
        &self,
        registry: &mut RuleRegistry,
    ) -> Result<(), worldwright_core::ModuleRegistrationError> {
        registry.register(
            CommandKind::GameAction,
            ActionRule {
                other: CharacterId::from_u128(2)
                    .map_err(|_| worldwright_core::ModuleRegistrationError::NotImplemented)?,
            },
        )?;
        Ok(())
    }
}

fn fixture(value: u64) -> (WorldRuntime, ActorContext, CharacterId, DefinitionId) {
    let world = WorldId::from_u128(1).unwrap();
    let room = RoomId::from_u128(1).unwrap();
    let actor_id = CharacterId::from_u128(1).unwrap();
    let other = CharacterId::from_u128(2).unwrap();
    let account = AccountId::from_u128(1).unwrap();
    let key = DefinitionId::parse("test:state.value").unwrap();
    let mut builder = WorldBuilder::new(world);
    builder
        .add_room(
            room,
            PlayerText::new("Room").unwrap(),
            PlayerText::new("Room description.").unwrap(),
        )
        .unwrap();
    builder.add_character(actor_id, account, room).unwrap();
    builder
        .add_character(other, AccountId::from_u128(2).unwrap(), room)
        .unwrap();
    builder
        .set_character_counter(actor_id, key.clone(), value)
        .unwrap();
    let mut rules = RuleRegistry::new();
    Game.register_rules(&mut rules).unwrap();
    let mut runtime = builder.build(rules).unwrap();
    let actor = runtime
        .attach(account, actor_id, ConnectionId::from_u128(1).unwrap())
        .unwrap();
    (runtime, actor, actor_id, key)
}

fn envelope(actor: ActorContext, action: &str) -> CommandEnvelope {
    CommandEnvelope {
        operation: OperationId::from_u128(1).unwrap(),
        world: WorldId::from_u128(1).unwrap(),
        actor,
        intent: CommandIntent::GameAction {
            action: DefinitionId::parse(action).unwrap(),
            argument: None,
        },
    }
}

#[test]
fn duplicate_counter_setup_preserves_the_original_value() {
    let world = WorldId::from_u128(1).unwrap();
    let room = RoomId::from_u128(1).unwrap();
    let character = CharacterId::from_u128(1).unwrap();
    let key = DefinitionId::parse("test:state.value").unwrap();
    let mut builder = WorldBuilder::new(world);
    builder
        .add_room(
            room,
            PlayerText::new("Room").unwrap(),
            PlayerText::new("Description").unwrap(),
        )
        .unwrap();
    builder
        .add_character(character, AccountId::from_u128(1).unwrap(), room)
        .unwrap();
    builder
        .set_character_counter(character, key.clone(), 3)
        .unwrap();
    assert_eq!(
        builder.set_character_counter(character, key.clone(), 9),
        Err(WorldError::DuplicateCounter)
    );
    let runtime = builder.build(RuleRegistry::new()).unwrap();
    assert_eq!(runtime.snapshot().counter(character, key), Some(3));
}

#[test]
fn failed_due_adjustment_rolls_back_and_remains_retryable() {
    let (mut runtime, actor, character, key) = fixture(MAX_COUNTER_VALUE);
    runtime
        .execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(1),
            &mut NoRandom,
        )
        .unwrap();
    let revision = runtime.snapshot().revision();
    for now in [10, 11] {
        assert_eq!(
            runtime.advance(Tick::from_millis(now)),
            Err(WorldError::CounterLimit)
        );
        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.revision(), revision);
        assert_eq!(snapshot.scheduled_actions(), 1);
        assert_eq!(
            snapshot.counter(character, key.clone()),
            Some(MAX_COUNTER_VALUE)
        );
    }
}

#[test]
fn foreign_counter_effect_is_rejected_atomically() {
    let (mut runtime, actor, _, _) = fixture(1);
    let before = runtime.snapshot().revision();
    assert_eq!(
        runtime.execute(
            &envelope(actor, "test:actions.foreign"),
            Tick::from_millis(1),
            &mut NoRandom
        ),
        Err(WorldError::IntentViolation)
    );
    assert_eq!(runtime.snapshot().revision(), before);
}

#[test]
fn no_due_advance_still_enforces_monotonic_time() {
    let (mut runtime, actor, _, _) = fixture(1);
    assert!(
        runtime
            .advance(Tick::from_millis(20))
            .unwrap()
            .events
            .is_empty()
    );
    assert_eq!(
        runtime.execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(19),
            &mut NoRandom
        ),
        Err(WorldError::ClockRegression)
    );
}

#[test]
fn conservative_world_view_default_remains_source_compatible() {
    struct OldView;
    impl worldwright_core::WorldView for OldView {
        fn world_id(&self) -> WorldId {
            WorldId::from_u128(1).unwrap()
        }
        fn contains_room(&self, _room: RoomId) -> bool {
            false
        }
        fn character_room(&self, _character: CharacterId) -> Option<RoomId> {
            None
        }
        fn item_location(
            &self,
            _item: worldwright_core::ids::ItemId,
        ) -> Option<worldwright_core::effect::ItemLocation> {
            None
        }
    }
    assert_eq!(
        worldwright_core::WorldView::character_counter(
            &OldView,
            CharacterId::from_u128(1).unwrap(),
            DefinitionId::parse("test:state.value").unwrap()
        ),
        None
    );
}

#[test]
fn actor_context_generation_contract_is_unchanged() {
    assert!(ConnectionGeneration::new(0).is_err());
}

#[test]
fn durable_export_is_canonical_after_scheduling() {
    let (mut runtime, actor, character, key) = fixture(3);
    runtime
        .execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(1),
            &mut NoRandom,
        )
        .unwrap();
    let durable = runtime.snapshot().durable_state().unwrap();
    assert_eq!(
        durable.counters,
        vec![worldwright_core::runtime::CounterState {
            character,
            key: key.clone(),
            value: 3
        }]
    );
    assert_eq!(
        durable.scheduled,
        vec![worldwright_core::runtime::ScheduledCounterState {
            at: Tick::from_millis(10),
            character,
            key,
            delta: 1
        }]
    );
}

#[test]
fn checkpoint_restore_recovers_attachment_and_schedule() {
    let (mut runtime, actor, _, _) = fixture(3);
    runtime
        .execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(1),
            &mut NoRandom,
        )
        .unwrap();
    let checkpoint = runtime.snapshot();
    runtime.detach(actor).unwrap();
    runtime.restore_checkpoint(checkpoint.clone()).unwrap();
    assert_eq!(runtime.snapshot(), checkpoint);
}

#[test]
fn wrong_world_checkpoint_is_rejected_atomically() {
    let (mut runtime, _, _, _) = fixture(3);
    let before = runtime.snapshot();
    let room = RoomId::from_u128(1).unwrap();
    let mut builder = WorldBuilder::new(WorldId::from_u128(2).unwrap());
    builder
        .add_room(
            room,
            PlayerText::new("Room").unwrap(),
            PlayerText::new("Description").unwrap(),
        )
        .unwrap();
    builder
        .add_character(
            CharacterId::from_u128(1).unwrap(),
            AccountId::from_u128(1).unwrap(),
            room,
        )
        .unwrap();
    let other = builder.build(RuleRegistry::new()).unwrap();
    assert_eq!(
        runtime.restore_checkpoint(other.snapshot()),
        Err(WorldError::WrongWorld)
    );
    assert_eq!(runtime.snapshot(), before);
}

#[test]
fn durable_state_restores_into_fresh_builder() {
    let (mut runtime, actor, character, key) = fixture(3);
    runtime
        .execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(1),
            &mut NoRandom,
        )
        .unwrap();
    let durable = runtime.snapshot().durable_state().unwrap();
    let world = WorldId::from_u128(1).unwrap();
    let room = RoomId::from_u128(1).unwrap();
    let mut builder = WorldBuilder::new(world);
    builder
        .add_room(
            room,
            PlayerText::new("Room").unwrap(),
            PlayerText::new("Description").unwrap(),
        )
        .unwrap();
    builder
        .add_character(character, AccountId::from_u128(1).unwrap(), room)
        .unwrap();
    builder.restore_durable_state(durable.clone()).unwrap();
    let fresh = builder.build(RuleRegistry::new()).unwrap();
    assert_eq!(fresh.snapshot().durable_state().unwrap(), durable);
    assert_eq!(key.to_string(), "test:state.value");
}

#[test]
fn malformed_durable_states_are_rejected_atomically() {
    let (mut runtime, actor, character, key) = fixture(3);
    runtime
        .execute(
            &envelope(actor, "test:actions.schedule"),
            Tick::from_millis(1),
            &mut NoRandom,
        )
        .unwrap();
    let base = runtime.snapshot().durable_state().unwrap();
    let make = || {
        let room = RoomId::from_u128(1).unwrap();
        let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
        builder
            .add_room(
                room,
                PlayerText::new("Room").unwrap(),
                PlayerText::new("Description").unwrap(),
            )
            .unwrap();
        builder
            .add_character(character, AccountId::from_u128(1).unwrap(), room)
            .unwrap();
        builder
    };
    let mut duplicate = base.clone();
    duplicate.counters.push(duplicate.counters[0].clone());
    assert!(make().restore_durable_state(duplicate).is_err());
    let mut unknown = base.clone();
    unknown.counters[0].character = CharacterId::from_u128(99).unwrap();
    assert!(make().restore_durable_state(unknown).is_err());
    let mut zero = base.clone();
    zero.scheduled[0].delta = 0;
    assert!(make().restore_durable_state(zero).is_err());
    let mut over = base.clone();
    over.counters[0].value = MAX_COUNTER_VALUE + 1;
    assert!(make().restore_durable_state(over).is_err());
    let mut full = base.clone();
    for i in 0..129 {
        full.scheduled
            .push(worldwright_core::runtime::ScheduledCounterState {
                at: Tick::from_millis(i),
                character,
                key: key.clone(),
                delta: 1,
            });
    }
    assert!(make().restore_durable_state(full).is_err());
}
