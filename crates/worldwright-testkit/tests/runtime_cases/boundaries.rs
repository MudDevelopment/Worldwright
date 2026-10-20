use super::support::*;
use worldwright_sdk::runtime::{MAX_CHARACTERS, MAX_DELIVERIES, MAX_EXITS_PER_ROOM, MAX_ROOMS};

#[test]
fn late_invalid_effect_rolls_back_movement_revision_time_and_messages() {
    let mut h = Harness::scripted(
        CommandKind::Move,
        vec![
            message(Audience::Room(room(1))),
            movement(),
            message(Audience::Character(character(1))),
            message(Audience::Room(room(1))),
        ],
    );
    h.reject_unchanged(
        h.envelope(CommandIntent::Move {
            destination: room(2),
        }),
        WorldError::AudienceDenied,
    );
    assert_eq!(
        h.runtime.snapshot().room_of(h.alice.character),
        Some(room(1))
    );
}

#[test]
fn item_and_schedule_effects_are_explicitly_unsupported_not_ignored() {
    for unsupported in [
        Effect::TransferItem {
            item: ItemId::from_u128(1).unwrap(),
            expected_from: ItemLocation::Room(room(1)),
            to: ItemLocation::Inventory(character(1)),
        },
        Effect::Schedule {
            at: Tick::ZERO,
            action: ScheduledAction::WakeCharacter(character(1)),
        },
    ] {
        let mut h = Harness::scripted(
            CommandKind::Move,
            vec![movement(), message(Audience::Room(room(2))), unsupported],
        );
        h.reject_unchanged(
            h.envelope(CommandIntent::Move {
                destination: room(2),
            }),
            WorldError::UnsupportedEffect,
        );
    }
}

#[test]
fn absent_rule_does_not_change_world() {
    let mut h = Harness::new(RuleRegistry::new(), true);
    h.reject_unchanged(
        h.envelope(CommandIntent::Look),
        WorldError::Rule(RuleError::MissingRule),
    );
}

#[test]
fn rejected_rule_rolls_back_world_but_not_caller_rng() {
    struct RejectAfterRandom;
    impl Rule for RejectAfterRandom {
        fn evaluate(
            &self,
            _: &RuleContext<'_>,
            _: &CommandIntent,
            rng: &mut dyn RandomSource,
        ) -> Result<EffectPlan, RuleError> {
            assert_eq!(rng.next_u64().unwrap(), 7);
            Err(RuleError::Rejected)
        }
    }
    let mut registry = RuleRegistry::new();
    registry
        .register(CommandKind::Look, RejectAfterRandom)
        .unwrap();
    let mut h = Harness::new(registry, true);
    let command = h.envelope(CommandIntent::Look);
    let before = h.runtime.snapshot();
    let mut rng = SequenceRandom::new([7, 9]);
    assert_eq!(
        h.runtime.execute(&command, Tick::ZERO, &mut rng),
        Err(WorldError::Rule(RuleError::Rejected))
    );
    assert_eq!(h.runtime.snapshot(), before);
    assert_eq!(rng.next_u64().unwrap(), 9);
}

#[test]
fn last_successful_time_is_monotonic_and_failures_do_not_advance_it() {
    let mut h = Harness::reference();
    let command = h.envelope(CommandIntent::Look);
    h.runtime
        .execute(
            &command,
            Tick::from_millis(10),
            &mut SequenceRandom::new([]),
        )
        .unwrap();
    let before = h.runtime.snapshot();
    assert_eq!(
        h.runtime
            .execute(&command, Tick::from_millis(9), &mut SequenceRandom::new([])),
        Err(WorldError::ClockRegression)
    );
    assert_eq!(h.runtime.snapshot(), before);
    let invalid = h.envelope(CommandIntent::Move {
        destination: room(99),
    });
    assert_eq!(
        h.runtime.execute(
            &invalid,
            Tick::from_millis(20),
            &mut SequenceRandom::new([])
        ),
        Err(WorldError::UnknownRoom)
    );
    assert_eq!(h.runtime.snapshot(), before);
    h.runtime
        .execute(
            &command,
            Tick::from_millis(10),
            &mut SequenceRandom::new([]),
        )
        .unwrap();
}

#[test]
fn fanout_limit_accepts_exact_boundary_and_rejects_overflow_atomically() {
    for count in [4, 5] {
        let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
        builder
            .add_room(room(1), text("Room"), text("Description"))
            .unwrap();
        for n in 1..=MAX_CHARACTERS as u128 {
            builder
                .add_character(character(n), account(1), room(1))
                .unwrap();
        }
        let mut registry = RuleRegistry::new();
        registry
            .register(
                CommandKind::Say,
                Scripted(vec![message(Audience::Room(room(1))); count]),
            )
            .unwrap();
        let mut runtime = builder.build(registry).unwrap();
        let mut first = None;
        for n in 1..=MAX_CHARACTERS as u128 {
            let actor = runtime
                .attach(account(1), character(n), connection(n))
                .unwrap();
            if n == 1 {
                first = Some(actor);
            }
        }
        let command = CommandEnvelope {
            world: runtime.snapshot().world_id(),
            actor: first.unwrap(),
            operation: OperationId::from_u128(1).unwrap(),
            intent: CommandIntent::Say {
                text: text("hello"),
            },
        };
        let before = runtime.snapshot();
        let result = runtime.execute(&command, Tick::ZERO, &mut SequenceRandom::new([]));
        if count == 4 {
            assert_eq!(result.unwrap().messages.len(), MAX_DELIVERIES);
        } else {
            assert_eq!(result, Err(WorldError::DeliveryLimit));
            assert_eq!(runtime.snapshot(), before);
        }
    }
}

#[test]
fn builder_rejects_duplicates_unknown_references_and_self_exits() {
    assert!(
        WorldBuilder::new(WorldId::from_u128(1).unwrap())
            .build(RuleRegistry::new())
            .is_err()
    );
    let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
    builder
        .add_room(room(1), text("Original"), text("Description"))
        .unwrap();
    assert_eq!(
        builder.add_room(room(1), text("Replacement"), text("Other")),
        Err(WorldError::DuplicateRoom)
    );
    assert_eq!(
        builder.add_exit(room(1), room(2)),
        Err(WorldError::UnknownRoom)
    );
    assert_eq!(
        builder.add_exit(room(1), room(1)),
        Err(WorldError::SelfExit)
    );
    assert_eq!(
        builder.add_character(character(1), account(1), room(2)),
        Err(WorldError::UnknownRoom)
    );
    builder
        .add_room(room(2), text("Next"), text("Description"))
        .unwrap();
    builder.add_exit(room(1), room(2)).unwrap();
    assert_eq!(
        builder.add_exit(room(1), room(2)),
        Err(WorldError::DuplicateExit)
    );
    builder
        .add_character(character(1), account(1), room(1))
        .unwrap();
    assert_eq!(
        builder.add_character(character(1), account(2), room(2)),
        Err(WorldError::DuplicateCharacter)
    );
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry).unwrap();
    let mut runtime = builder.build(registry).unwrap();
    let actor = runtime
        .attach(account(1), character(1), connection(1))
        .unwrap();
    let command = CommandEnvelope {
        world: runtime.snapshot().world_id(),
        actor,
        operation: OperationId::from_u128(1).unwrap(),
        intent: CommandIntent::Look,
    };
    let result = runtime
        .execute(&command, Tick::ZERO, &mut SequenceRandom::new([]))
        .unwrap();
    assert_eq!(result.messages[0].text.as_str(), "Original");
}

#[test]
fn builder_enforces_room_character_and_exit_capacity() {
    let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
    for n in 1..=MAX_ROOMS as u128 {
        builder
            .add_room(room(n), text("Room"), text("Description"))
            .unwrap();
    }
    assert_eq!(
        builder.add_room(
            room(MAX_ROOMS as u128 + 1),
            text("Room"),
            text("Description")
        ),
        Err(WorldError::WorldCapacity)
    );
    for n in 1..=MAX_CHARACTERS as u128 {
        builder
            .add_character(character(n), account(1), room(1))
            .unwrap();
    }
    assert_eq!(
        builder.add_character(character(MAX_CHARACTERS as u128 + 1), account(1), room(1)),
        Err(WorldError::WorldCapacity)
    );
    for n in 2..=MAX_EXITS_PER_ROOM as u128 + 1 {
        builder.add_exit(room(1), room(n)).unwrap();
    }
    assert_eq!(
        builder.add_exit(room(1), room(MAX_EXITS_PER_ROOM as u128 + 2)),
        Err(WorldError::ExitCapacity)
    );
}

#[test]
fn maximum_length_room_fields_do_not_require_unsafe_concatenation() {
    let mut builder = WorldBuilder::new(WorldId::from_u128(1).unwrap());
    let field = text(&"x".repeat(PlayerText::MAX_BYTES));
    builder.add_room(room(1), field.clone(), field).unwrap();
    builder
        .add_character(character(1), account(1), room(1))
        .unwrap();
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry).unwrap();
    let mut runtime = builder.build(registry).unwrap();
    let actor = runtime
        .attach(account(1), character(1), connection(1))
        .unwrap();
    let command = CommandEnvelope {
        world: runtime.snapshot().world_id(),
        actor,
        operation: OperationId::from_u128(1).unwrap(),
        intent: CommandIntent::Look,
    };
    let result = runtime
        .execute(&command, Tick::ZERO, &mut SequenceRandom::new([]))
        .unwrap();
    assert_eq!(result.messages.len(), 2);
    assert!(
        result
            .messages
            .iter()
            .all(|m| m.text.as_str().len() == PlayerText::MAX_BYTES)
    );
}
