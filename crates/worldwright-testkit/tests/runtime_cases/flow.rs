use super::support::*;
use worldwright_testkit::two_room_world;

#[test]
fn two_room_look_move_say_walkthrough() {
    let mut fixture = two_room_world().unwrap();
    let mut random = SequenceRandom::new([]);
    let command = fixture.command(
        OperationId::from_u128(1).unwrap(),
        fixture.alice,
        CommandIntent::Look,
    );
    let look = fixture
        .runtime
        .execute(&command, Tick::ZERO, &mut random)
        .unwrap();
    assert_eq!(look.messages.len(), 2);
    assert_eq!(look.messages[0].text.as_str(), "Atrium");
    assert!(look.messages.iter().all(|m| m.recipient == fixture.alice));
    assert!(look.events.is_empty());

    let command = fixture.command(
        OperationId::from_u128(2).unwrap(),
        fixture.alice,
        CommandIntent::Move {
            destination: fixture.observatory,
        },
    );
    let moved = fixture
        .runtime
        .execute(&command, Tick::from_millis(1), &mut random)
        .unwrap();
    assert_eq!(
        fixture.runtime.snapshot().room_of(fixture.alice.character),
        Some(fixture.observatory)
    );
    assert_eq!(
        moved.events,
        vec![DomainEvent::CharacterMoved {
            character: fixture.alice.character,
            from: fixture.atrium,
            to: fixture.observatory,
        }]
    );
    assert_eq!(moved.messages[0].recipient, fixture.alice);

    let command = fixture.command(
        OperationId::from_u128(3).unwrap(),
        fixture.alice,
        CommandIntent::Look,
    );
    let look = fixture
        .runtime
        .execute(&command, Tick::from_millis(2), &mut random)
        .unwrap();
    assert_eq!(look.messages[0].text.as_str(), "Observatory");
    let command = fixture.command(
        OperationId::from_u128(4).unwrap(),
        fixture.alice,
        CommandIntent::Say {
            text: text("Hello, Bob!"),
        },
    );
    let said = fixture
        .runtime
        .execute(&command, Tick::from_millis(3), &mut random)
        .unwrap();
    assert_eq!(
        said.messages
            .iter()
            .map(|m| m.recipient)
            .collect::<Vec<_>>(),
        vec![fixture.alice, fixture.bob]
    );
    assert!(
        said.messages
            .iter()
            .all(|m| m.source == fixture.alice.character && m.text.as_str() == "Hello, Bob!")
    );
    println!("Atrium -> Observatory; Look returned room text; Say addressed Alice and Bob.");
}

#[test]
fn say_does_not_reach_a_different_room() {
    let mut h = Harness::reference();
    let result = h
        .execute(CommandIntent::Say {
            text: text("hello"),
        })
        .unwrap();
    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].recipient, h.alice);
}

#[test]
fn detached_characters_are_not_message_recipients() {
    let mut h = Harness::reference();
    h.runtime.detach(h.bob).unwrap();
    h.execute(CommandIntent::Move {
        destination: room(2),
    })
    .unwrap();
    let said = h
        .execute(CommandIntent::Say {
            text: text("hello"),
        })
        .unwrap();
    assert_eq!(said.messages.len(), 1);
    assert_eq!(said.messages[0].recipient, h.alice);
}

#[test]
fn audiences_follow_each_staged_effect_not_original_locations() {
    let mut h = Harness::scripted(
        CommandKind::Move,
        vec![
            message(Audience::Room(room(1))),
            movement(),
            message(Audience::Room(room(2))),
        ],
    );
    let result = h
        .execute(CommandIntent::Move {
            destination: room(2),
        })
        .unwrap();
    assert_eq!(
        result
            .messages
            .iter()
            .map(|m| m.recipient)
            .collect::<Vec<_>>(),
        vec![h.alice, h.alice, h.bob]
    );
    assert_eq!(result.events.len(), 1);
}

#[test]
fn snapshots_are_detached_and_results_pin_recipient_generation() {
    let mut h = Harness::reference();
    let snapshot = h.runtime.snapshot();
    let result = h
        .execute(CommandIntent::Move {
            destination: room(2),
        })
        .unwrap();
    let replacement = h
        .runtime
        .attach(h.alice.account, h.alice.character, h.alice.connection)
        .unwrap();
    assert_eq!(snapshot.room_of(h.alice.character), Some(room(1)));
    assert_eq!(
        h.runtime.snapshot().room_of(h.alice.character),
        Some(room(2))
    );
    assert_eq!(result.messages[0].recipient, h.alice);
    assert_ne!(
        result.messages[0].recipient.generation,
        replacement.generation
    );
}

#[test]
fn identical_inputs_clock_and_fixture_produce_identical_results() {
    fn scenario() -> (
        Vec<ExecutionResult>,
        worldwright_sdk::runtime::WorldSnapshot,
    ) {
        let mut h = Harness::reference();
        let results = vec![
            h.execute(CommandIntent::Look).unwrap(),
            h.execute(CommandIntent::Move {
                destination: room(2),
            })
            .unwrap(),
            h.execute(CommandIntent::Say {
                text: text("deterministic"),
            })
            .unwrap(),
        ];
        (results, h.runtime.snapshot())
    }
    assert_eq!(scenario(), scenario());
}

#[test]
fn operation_ids_are_correlation_not_duplicate_suppression() {
    let mut h = Harness::reference();
    let first = h.execute(CommandIntent::Look).unwrap();
    let second = h.execute(CommandIntent::Look).unwrap();
    assert_eq!(first.operation, second.operation);
    assert_eq!(second.revision, first.revision + 1);
    assert_eq!(first.messages, second.messages);
}
