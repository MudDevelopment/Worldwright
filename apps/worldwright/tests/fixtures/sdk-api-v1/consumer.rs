// Retained API-v1 consumer, captured from 0.1.0-dev.0, not a previous release.
// Do not rewrite this fixture just to make a newer SDK compile.
use worldwright_sdk::{
    GameModule,
    command::{CommandEnvelope, CommandIntent, PlayerText},
    content::load_bytes,
    ids::{AccountId, CharacterId, ConnectionId, OperationId, RoomId, WorldId},
    rule::RuleRegistry,
    runtime::{ReferenceGame, WorldBuilder},
    time::{RandomExhausted, RandomSource, Tick},
};

struct ConsumerGame;
impl GameModule for ConsumerGame {
    fn name(&self) -> &str { "Retained API-v1 consumer" }
}
struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> { Err(RandomExhausted) }
}

#[test]
fn retained_api_v1_builds_and_executes_real_reference_rules() {
    assert_eq!(worldwright_sdk::SDK_API_VERSION, 1);
    assert!(worldwright_sdk::SUPPORTED_SDK_API_VERSIONS.contains(&1));
    assert_eq!(ConsumerGame.name(), "Retained API-v1 consumer");
    let content = load_bytes("retained content", include_bytes!("../content/world.json"), Default::default()).unwrap();
    assert!(!content.rooms().is_empty());
    let world = WorldId::from_u128(1).unwrap();
    let room = RoomId::from_u128(1).unwrap();
    let account = AccountId::from_u128(1).unwrap();
    let character = CharacterId::from_u128(1).unwrap();
    let mut builder = WorldBuilder::new(world);
    builder.add_room(room, PlayerText::new("Retained room").unwrap(), PlayerText::new("API-v1 rules still execute.").unwrap()).unwrap();
    builder.add_character(character, account, room).unwrap();
    let mut rules = RuleRegistry::new();
    ReferenceGame.register_rules(&mut rules).unwrap();
    let mut runtime = builder.build(rules).unwrap();
    let actor = runtime.attach(account, character, ConnectionId::from_u128(1).unwrap()).unwrap();
    let result = runtime.execute(&CommandEnvelope {
        operation: OperationId::from_u128(1).unwrap(), world, actor, intent: CommandIntent::Look,
    }, Tick::from_millis(1), &mut NoRandom).unwrap();
    assert_eq!(result.messages.len(), 2);
    assert!(result.revision > 0);
}
