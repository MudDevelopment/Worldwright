use worldwright_content::{ValidatedWorldContent, instantiate};
use worldwright_core::{
    command::{CommandEnvelope, CommandIntent},
    game::GameModule,
    ids::{AccountId, CharacterId, ConnectionId, DefinitionId, OperationId, WorldId},
    rule::RuleRegistry,
    runtime::{ReferenceGame, WorldError},
    time::{RandomExhausted, RandomSource, Tick},
};
use worldwright_expedition::ExpeditionGame;
use worldwright_persistence::{ActivationPlan, PersistenceError, PostgresStore};
use worldwright_release::{GameContract, verify_bytes};

#[derive(Debug)]
pub(crate) enum ReleaseRuntimeError {
    Persistence,
    Incompatible,
    Smoke,
}

struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        Err(RandomExhausted)
    }
}

pub(crate) async fn verify_and_smoke(
    store: &PostgresStore,
    plan: &ActivationPlan,
) -> Result<(), ReleaseRuntimeError> {
    let verified = verify_bytes(
        "activation artifact",
        plan.artifact().bytes(),
        Some(plan.artifact().digest()),
    )
    .map_err(|_| ReleaseRuntimeError::Incompatible)?;
    if verified.manifest().release_id() != plan.artifact().release_id() {
        return Err(ReleaseRuntimeError::Incompatible);
    }
    store
        .verify_world(
            plan.world(),
            verified.content().namespace(),
            verified.content().schema_version(),
        )
        .await
        .map_err(|_| ReleaseRuntimeError::Persistence)?;
    let references = store
        .durable_content_references(plan.world())
        .await
        .map_err(|_| ReleaseRuntimeError::Persistence)?;
    validate_references(
        verified.content(),
        references.room_definitions(),
        references.item_definitions(),
    )?;
    smoke_content(verified.content(), verified.manifest().game_contract())
}

#[allow(dead_code)]
pub(crate) async fn verify_restored_world(
    store: &PostgresStore,
    world: WorldId,
) -> Result<(), ReleaseRuntimeError> {
    let artifact = store
        .active_artifact(world)
        .await
        .map_err(|_| ReleaseRuntimeError::Persistence)?;
    let verified = verify_bytes(
        "restored active artifact",
        artifact.bytes(),
        Some(artifact.digest()),
    )
    .map_err(|_| ReleaseRuntimeError::Incompatible)?;
    if verified.manifest().release_id() != artifact.release_id() {
        return Err(ReleaseRuntimeError::Incompatible);
    }
    store
        .verify_world(
            world,
            verified.content().namespace(),
            verified.content().schema_version(),
        )
        .await
        .map_err(|_| ReleaseRuntimeError::Persistence)?;
    let references = store
        .durable_content_references(world)
        .await
        .map_err(|_| ReleaseRuntimeError::Persistence)?;
    validate_references(
        verified.content(),
        references.room_definitions(),
        references.item_definitions(),
    )?;
    smoke_content(verified.content(), verified.manifest().game_contract())
}

fn validate_references(
    content: &ValidatedWorldContent,
    rooms: &[String],
    items: &[String],
) -> Result<(), ReleaseRuntimeError> {
    for room in rooms {
        let definition = room
            .parse::<DefinitionId>()
            .map_err(|_| ReleaseRuntimeError::Incompatible)?;
        if !content.rooms().contains_key(&definition) {
            return Err(ReleaseRuntimeError::Incompatible);
        }
    }
    let expected_item = format!("{}:items.welcome_token", content.namespace());
    if items.iter().any(|definition| definition != &expected_item) {
        return Err(ReleaseRuntimeError::Incompatible);
    }
    Ok(())
}

fn smoke_content(
    content: &ValidatedWorldContent,
    contract: GameContract,
) -> Result<(), ReleaseRuntimeError> {
    let world = WorldId::from_u128(1).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let account = AccountId::from_u128(1).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let character = CharacterId::from_u128(1).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let connection = ConnectionId::from_u128(1).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let look_operation = OperationId::from_u128(1).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let move_operation = OperationId::from_u128(2).map_err(|_| ReleaseRuntimeError::Smoke)?;
    let (mut builder, lookup) = instantiate(content, world)
        .map_err(|_| ReleaseRuntimeError::Incompatible)?
        .into_parts();
    builder
        .add_character(character, account, lookup.entry_room)
        .map_err(|_| ReleaseRuntimeError::Smoke)?;
    if contract == GameContract::Expedition {
        worldwright_expedition::initialize_character(&mut builder, character)
            .map_err(|_| ReleaseRuntimeError::Smoke)?;
    }
    let mut rules = RuleRegistry::new();
    match contract {
        GameContract::Reference => ReferenceGame.register_rules(&mut rules),
        GameContract::Expedition => ExpeditionGame.register_rules(&mut rules),
    }
    .map_err(|_| ReleaseRuntimeError::Smoke)?;
    let mut runtime = builder
        .build(rules)
        .map_err(|_| ReleaseRuntimeError::Smoke)?;
    let actor = runtime
        .attach(account, character, connection)
        .map_err(|_| ReleaseRuntimeError::Smoke)?;
    let destination = lookup
        .named_exits
        .get(&lookup.entry_room)
        .and_then(|exits| exits.values().next())
        .copied()
        .ok_or(ReleaseRuntimeError::Smoke)?;
    let mut random = NoRandom;
    let execute = |runtime: &mut worldwright_core::runtime::WorldRuntime,
                   operation: OperationId,
                   intent: CommandIntent,
                   random: &mut NoRandom|
     -> Result<(), WorldError> {
        runtime.execute(
            &CommandEnvelope {
                operation,
                world,
                actor,
                intent,
            },
            Tick::from_millis(0),
            random,
        )?;
        Ok(())
    };
    let result = (|| {
        execute(
            &mut runtime,
            look_operation,
            CommandIntent::Look,
            &mut random,
        )?;
        execute(
            &mut runtime,
            move_operation,
            CommandIntent::Move { destination },
            &mut random,
        )?;
        execute(
            &mut runtime,
            OperationId::from_u128(3).map_err(|_| WorldError::CounterExhausted)?,
            CommandIntent::Look,
            &mut random,
        )?;
        Ok::<(), WorldError>(())
    })();
    let detached = runtime.detach(actor);
    if result.is_err() || detached.is_err() {
        return Err(ReleaseRuntimeError::Smoke);
    }
    Ok(())
}

#[allow(dead_code)]
fn _map_persistence(_: PersistenceError) -> ReleaseRuntimeError {
    ReleaseRuntimeError::Persistence
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_content_smoke_runs() {
        let content = worldwright_content::load_bytes(
            "test",
            include_bytes!("../../../examples/two-room/world.json"),
            Default::default(),
        )
        .unwrap();
        assert!(smoke_content(&content, GameContract::Reference).is_ok());
    }

    #[test]
    fn missing_room_reference_is_incompatible() {
        let content = worldwright_content::load_bytes(
            "test",
            include_bytes!("../../../examples/two-room/world.json"),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            validate_references(&content, &["reference:rooms.missing".to_owned()], &[]),
            Err(ReleaseRuntimeError::Incompatible)
        ));
    }

    #[test]
    fn unsupported_item_reference_is_incompatible() {
        let content = worldwright_content::load_bytes(
            "test",
            include_bytes!("../../../examples/two-room/world.json"),
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            validate_references(&content, &[], &["reference:items.other".to_owned()]),
            Err(ReleaseRuntimeError::Incompatible)
        ));
    }

    #[test]
    fn single_room_without_destination_fails_smoke() {
        let content = worldwright_content::load_bytes(
            "single-room",
            br#"{
                "schema_version": 1,
                "namespace": "reference",
                "entry_room": "reference:rooms.only",
                "rooms": [{
                    "id": "reference:rooms.only",
                    "title": "Only",
                    "description": "A single room.",
                    "exits": {},
                    "allow_unreachable": true
                }]
            }"#,
            Default::default(),
        )
        .unwrap();
        assert!(matches!(
            smoke_content(&content, GameContract::Reference),
            Err(ReleaseRuntimeError::Smoke)
        ));
    }
}
