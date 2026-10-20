use super::support::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[test]
fn forged_contexts_and_bad_destinations_fail_before_rule_evaluation() {
    struct CountRule(Arc<AtomicUsize>);
    impl Rule for CountRule {
        fn evaluate(
            &self,
            _: &RuleContext<'_>,
            _: &CommandIntent,
            _: &mut dyn RandomSource,
        ) -> Result<EffectPlan, RuleError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(EffectPlan::new())
        }
    }
    let calls = Arc::new(AtomicUsize::new(0));
    let mut registry = RuleRegistry::new();
    registry
        .register(CommandKind::Look, CountRule(calls.clone()))
        .unwrap();
    let mut h = Harness::new(registry, true);
    let base = h.envelope(CommandIntent::Look);
    let mut command = base.clone();
    command.world = WorldId::from_u128(99).unwrap();
    h.reject_unchanged(command, WorldError::WrongWorld);
    let mut command = base.clone();
    command.actor.character = character(99);
    h.reject_unchanged(command, WorldError::UnknownCharacter);
    let mut command = base.clone();
    command.actor.account = account(2);
    h.reject_unchanged(command, WorldError::OwnerMismatch);
    let mut command = base.clone();
    command.actor.connection = connection(99);
    h.reject_unchanged(command, WorldError::StaleAttachment);
    let mut command = base;
    command.actor.generation = ConnectionGeneration::new(99).unwrap();
    h.reject_unchanged(command, WorldError::StaleAttachment);
    h.reject_unchanged(
        h.envelope(CommandIntent::Move {
            destination: room(99),
        }),
        WorldError::UnknownRoom,
    );
    h.reject_unchanged(
        h.envelope(CommandIntent::Move {
            destination: room(3),
        }),
        WorldError::Unreachable,
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn replacement_attachment_invalidates_commands_and_stale_disconnects() {
    let mut h = Harness::reference();
    let old = h.alice;
    let replacement = h
        .runtime
        .attach(old.account, old.character, old.connection)
        .unwrap();
    assert!(replacement.generation.get() > old.generation.get());
    h.reject_unchanged(h.envelope(CommandIntent::Look), WorldError::StaleAttachment);
    let before = h.runtime.snapshot();
    assert_eq!(h.runtime.detach(old), Err(WorldError::StaleAttachment));
    assert_eq!(h.runtime.snapshot(), before);
    h.alice = replacement;
    h.execute(CommandIntent::Look).unwrap();
    h.runtime.detach(replacement).unwrap();
    h.reject_unchanged(h.envelope(CommandIntent::Look), WorldError::NotAttached);
    let next = h
        .runtime
        .attach(old.account, old.character, old.connection)
        .unwrap();
    assert!(next.generation.get() > replacement.generation.get());
}

#[test]
fn ownership_and_connection_collisions_cannot_change_attachments() {
    let mut h = Harness::reference();
    let before = h.runtime.snapshot();
    assert_eq!(
        h.runtime
            .attach(account(2), h.alice.character, connection(7)),
        Err(WorldError::OwnerMismatch)
    );
    assert_eq!(
        h.runtime
            .attach(h.alice.account, h.alice.character, h.bob.connection),
        Err(WorldError::ConnectionInUse)
    );
    assert_eq!(
        h.runtime.attach(account(1), character(99), connection(7)),
        Err(WorldError::UnknownCharacter)
    );
    assert_eq!(h.runtime.snapshot(), before);
}

#[test]
fn exits_are_directed_and_self_movement_is_not_teleportation() {
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry).unwrap();
    let mut h = Harness::new(registry, false);
    h.reject_unchanged(
        h.envelope(CommandIntent::Move {
            destination: room(1),
        }),
        WorldError::Unreachable,
    );
    h.execute(CommandIntent::Move {
        destination: room(2),
    })
    .unwrap();
    h.reject_unchanged(
        h.envelope(CommandIntent::Move {
            destination: room(1),
        }),
        WorldError::Unreachable,
    );
}

#[test]
fn rule_cannot_move_someone_else_change_destination_or_move_under_look() {
    for (kind, intent, effect) in [
        (
            CommandKind::Move,
            CommandIntent::Move {
                destination: room(2),
            },
            Effect::MoveCharacter {
                character: character(2),
                destination: room(2),
            },
        ),
        (
            CommandKind::Move,
            CommandIntent::Move {
                destination: room(2),
            },
            Effect::MoveCharacter {
                character: character(1),
                destination: room(3),
            },
        ),
        (CommandKind::Look, CommandIntent::Look, movement()),
        (
            CommandKind::Say,
            CommandIntent::Say {
                text: text("hello"),
            },
            movement(),
        ),
    ] {
        let mut h = Harness::scripted(kind, vec![effect]);
        h.reject_unchanged(h.envelope(intent), WorldError::IntentViolation);
    }
}

#[test]
fn move_requires_exactly_one_movement() {
    let mut missing = Harness::scripted(
        CommandKind::Move,
        vec![message(Audience::Character(character(1)))],
    );
    missing.reject_unchanged(
        missing.envelope(CommandIntent::Move {
            destination: room(2),
        }),
        WorldError::MissingMovement,
    );
    let mut repeated = Harness::scripted(CommandKind::Move, vec![movement(), movement()]);
    repeated.reject_unchanged(
        repeated.envelope(CommandIntent::Move {
            destination: room(2),
        }),
        WorldError::IntentViolation,
    );
}

#[test]
fn remote_direct_and_room_messages_and_look_broadcasts_are_denied() {
    for (kind, intent, audience) in [
        (
            CommandKind::Look,
            CommandIntent::Look,
            Audience::Character(character(2)),
        ),
        (
            CommandKind::Say,
            CommandIntent::Say {
                text: text("hello"),
            },
            Audience::Room(room(2)),
        ),
        (
            CommandKind::Look,
            CommandIntent::Look,
            Audience::Room(room(1)),
        ),
    ] {
        let mut h = Harness::scripted(kind, vec![message(audience)]);
        h.reject_unchanged(h.envelope(intent), WorldError::AudienceDenied);
    }
}

#[test]
fn rule_view_filters_other_characters_distant_rooms_and_descriptions() {
    struct Probe;
    impl Rule for Probe {
        fn evaluate(
            &self,
            c: &RuleContext<'_>,
            _: &CommandIntent,
            _: &mut dyn RandomSource,
        ) -> Result<EffectPlan, RuleError> {
            assert_eq!(c.world.character_room(character(1)), Some(room(1)));
            assert_eq!(c.world.character_room(character(2)), None);
            assert!(c.world.contains_room(room(2)));
            assert!(!c.world.contains_room(room(3)));
            assert!(c.world.room_description(room(1)).is_some());
            assert!(c.world.room_description(room(2)).is_none());
            assert!(c.world.room_description(room(3)).is_none());
            assert!(c.world.can_traverse(room(1), room(2)));
            assert!(!c.world.can_traverse(room(2), room(1)));
            assert_eq!(c.world.item_location(ItemId::from_u128(1).unwrap()), None);
            Ok(EffectPlan::new())
        }
    }
    let mut registry = RuleRegistry::new();
    registry.register(CommandKind::Look, Probe).unwrap();
    Harness::new(registry, true)
        .execute(CommandIntent::Look)
        .unwrap();
}
