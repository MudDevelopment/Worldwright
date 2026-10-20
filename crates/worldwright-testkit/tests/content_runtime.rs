use worldwright_sdk::{
    GameModule,
    command::{CommandEnvelope, CommandIntent, PlayerText},
    content::{instantiate, load_bytes},
    effect::DomainEvent,
    ids::{AccountId, CharacterId, ConnectionId, DefinitionId, OperationId, WorldId},
    rule::RuleRegistry,
    runtime::{ReferenceGame, WorldError},
    time::Tick,
};
use worldwright_testkit::SequenceRandom;
const EXAMPLE: &str = include_str!("../../../examples/two-room/world.json");

#[test]
fn json_two_room_look_move_look_say_and_exact_recipients() {
    let content = load_bytes("example", EXAMPLE.as_bytes(), Default::default()).unwrap();
    let world_id = WorldId::from_u128(10).unwrap();
    let mut instance = instantiate(&content, world_id).unwrap();
    let atrium = instance.entry_room();
    let observatory =
        instance.room_ids()[&DefinitionId::parse("reference:rooms.observatory").unwrap()];
    let alice_id = CharacterId::from_u128(1).unwrap();
    let bob_id = CharacterId::from_u128(2).unwrap();
    let alice_owner = AccountId::from_u128(1).unwrap();
    let bob_owner = AccountId::from_u128(2).unwrap();
    instance
        .builder_mut()
        .add_character(alice_id, alice_owner, atrium)
        .unwrap();
    instance
        .builder_mut()
        .add_character(bob_id, bob_owner, observatory)
        .unwrap();
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry).unwrap();
    let (builder, lookup) = instance.into_parts();
    let mut runtime = builder.build(registry).unwrap();
    let alice = runtime
        .attach(alice_owner, alice_id, ConnectionId::from_u128(1).unwrap())
        .unwrap();
    let bob = runtime
        .attach(bob_owner, bob_id, ConnectionId::from_u128(2).unwrap())
        .unwrap();
    let mut random = SequenceRandom::new([]);
    let command = |n, intent| CommandEnvelope {
        operation: OperationId::from_u128(n).unwrap(),
        world: world_id,
        actor: alice,
        intent,
    };
    let result = runtime
        .execute(&command(1, CommandIntent::Look), Tick::ZERO, &mut random)
        .unwrap();
    assert_eq!(
        result
            .messages
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Atrium", "Sunlight fills a quiet entry hall."]
    );
    assert!(result.messages.iter().all(|m| m.recipient == alice));
    let said = runtime
        .execute(
            &command(
                2,
                CommandIntent::Say {
                    text: PlayerText::new("Before moving").unwrap(),
                },
            ),
            Tick::ZERO,
            &mut random,
        )
        .unwrap();
    assert_eq!(said.messages.len(), 1);
    assert_eq!(said.messages[0].recipient, alice);
    let destination = lookup.named_exits[&atrium]["north"];
    let moved = runtime
        .execute(
            &command(3, CommandIntent::Move { destination }),
            Tick::ZERO,
            &mut random,
        )
        .unwrap();
    assert_eq!(runtime.snapshot().room_of(alice_id), Some(observatory));
    assert_eq!(
        moved.events,
        vec![DomainEvent::CharacterMoved {
            character: alice_id,
            from: atrium,
            to: observatory
        }]
    );
    let result = runtime
        .execute(&command(4, CommandIntent::Look), Tick::ZERO, &mut random)
        .unwrap();
    assert_eq!(result.messages[0].text.as_str(), "Observatory");
    let said = runtime
        .execute(
            &command(
                5,
                CommandIntent::Say {
                    text: PlayerText::new("Hello, Bob!").unwrap(),
                },
            ),
            Tick::ZERO,
            &mut random,
        )
        .unwrap();
    assert_eq!(
        said.messages
            .iter()
            .map(|m| m.recipient)
            .collect::<Vec<_>>(),
        vec![alice, bob]
    );
    assert!(
        said.messages
            .iter()
            .all(|m| m.source == alice_id && m.text.as_str() == "Hello, Bob!")
    );
}

#[test]
fn loaded_one_way_world_rejects_reverse_movement_without_mutation() {
    let source = EXAMPLE.replace("\"south\": \"reference:rooms.atrium\"", "");
    let content = load_bytes("one way", source.as_bytes(), Default::default()).unwrap();
    let world_id = WorldId::from_u128(11).unwrap();
    let mut instance = instantiate(&content, world_id).unwrap();
    let atrium = instance.entry_room();
    let observatory = instance.named_exits()[&atrium]["north"];
    let character = CharacterId::from_u128(1).unwrap();
    let owner = AccountId::from_u128(1).unwrap();
    instance
        .builder_mut()
        .add_character(character, owner, observatory)
        .unwrap();
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry).unwrap();
    let mut runtime = instance.into_builder().build(registry).unwrap();
    let actor = runtime
        .attach(owner, character, ConnectionId::from_u128(1).unwrap())
        .unwrap();
    let before = runtime.snapshot();
    let command = CommandEnvelope {
        operation: OperationId::from_u128(1).unwrap(),
        world: world_id,
        actor,
        intent: CommandIntent::Move {
            destination: atrium,
        },
    };
    assert_eq!(
        runtime.execute(&command, Tick::ZERO, &mut SequenceRandom::new([])),
        Err(WorldError::Unreachable)
    );
    assert_eq!(runtime.snapshot(), before);
}
