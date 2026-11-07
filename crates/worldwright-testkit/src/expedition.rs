//! Real-content fixture for the independent expedition game.

use std::error::Error;
use worldwright_expedition::{ExpeditionGame, initialize_character};
use worldwright_sdk::{
    GameModule,
    command::ActorContext,
    ids::{AccountId, CharacterId, ConnectionId, WorldId},
    rule::RuleRegistry,
    runtime::WorldRuntime,
};

pub struct ExpeditionWorld {
    pub runtime: WorldRuntime,
    pub alice: ActorContext,
    pub bob: ActorContext,
}

pub fn expedition_world() -> Result<ExpeditionWorld, Box<dyn Error>> {
    let content = worldwright_content::load_bytes(
        "examples/expedition/world.json",
        include_bytes!("../../../examples/expedition/world.json"),
        Default::default(),
    )?;
    let world = WorldId::from_u128(2)?;
    let (mut builder, lookup) = worldwright_content::instantiate(&content, world)?.into_parts();
    let alice_id = CharacterId::from_u128(101)?;
    let bob_id = CharacterId::from_u128(102)?;
    let alice_owner = AccountId::from_u128(101)?;
    let bob_owner = AccountId::from_u128(102)?;
    builder.add_character(alice_id, alice_owner, lookup.entry_room)?;
    builder.add_character(bob_id, bob_owner, lookup.entry_room)?;
    initialize_character(&mut builder, alice_id)?;
    initialize_character(&mut builder, bob_id)?;
    let mut rules = RuleRegistry::new();
    ExpeditionGame.register_rules(&mut rules)?;
    let mut runtime = builder.build(rules)?;
    let alice = runtime.attach(alice_owner, alice_id, ConnectionId::from_u128(101)?)?;
    let bob = runtime.attach(bob_owner, bob_id, ConnectionId::from_u128(102)?)?;
    Ok(ExpeditionWorld {
        runtime,
        alice,
        bob,
    })
}
