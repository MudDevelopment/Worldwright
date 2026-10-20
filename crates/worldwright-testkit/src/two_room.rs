//! Programmatic, deterministic reference world. No login or external services.

use std::error::Error;
use worldwright_sdk::{
    GameModule,
    command::{ActorContext, CommandEnvelope, CommandIntent, PlayerText},
    ids::{AccountId, CharacterId, ConnectionId, OperationId, RoomId, WorldId},
    rule::RuleRegistry,
    runtime::{ReferenceGame, WorldBuilder, WorldRuntime},
};

pub struct TwoRoomWorld {
    pub runtime: WorldRuntime,
    pub world: WorldId,
    pub atrium: RoomId,
    pub observatory: RoomId,
    pub alice: ActorContext,
    pub bob: ActorContext,
}
impl TwoRoomWorld {
    pub fn command(
        &self,
        operation: OperationId,
        actor: ActorContext,
        intent: CommandIntent,
    ) -> CommandEnvelope {
        CommandEnvelope {
            operation,
            world: self.world,
            actor,
            intent,
        }
    }
}

pub fn two_room_world() -> Result<TwoRoomWorld, Box<dyn Error>> {
    let world = WorldId::from_u128(1)?;
    let atrium = RoomId::from_u128(1)?;
    let observatory = RoomId::from_u128(2)?;
    let alice_id = CharacterId::from_u128(1)?;
    let bob_id = CharacterId::from_u128(2)?;
    let alice_owner = AccountId::from_u128(1)?;
    let bob_owner = AccountId::from_u128(2)?;
    let mut builder = WorldBuilder::new(world);
    builder.add_room(
        atrium,
        PlayerText::new("Atrium")?,
        PlayerText::new("Sunlight fills a quiet entry hall.")?,
    )?;
    builder.add_room(
        observatory,
        PlayerText::new("Observatory")?,
        PlayerText::new("A telescope points toward the stars.")?,
    )?;
    builder.add_exit(atrium, observatory)?;
    builder.add_exit(observatory, atrium)?;
    builder.add_character(alice_id, alice_owner, atrium)?;
    builder.add_character(bob_id, bob_owner, observatory)?;
    let mut registry = RuleRegistry::new();
    ReferenceGame.register_rules(&mut registry)?;
    let mut runtime = builder.build(registry)?;
    let alice = runtime.attach(alice_owner, alice_id, ConnectionId::from_u128(1)?)?;
    let bob = runtime.attach(bob_owner, bob_id, ConnectionId::from_u128(2)?)?;
    Ok(TwoRoomWorld {
        runtime,
        world,
        atrium,
        observatory,
        alice,
        bob,
    })
}
