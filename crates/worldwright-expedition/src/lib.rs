//! Expedition rules and playable host built on public SDK seams.

pub mod terminal;

/// Friendly executable/browser commands map to the same authoritative actions.
pub fn action_id(command: &str) -> Option<&'static str> {
    match command {
        "forage" => Some(FORAGE_ACTION),
        "encounter" => Some(ENCOUNTER_ACTION),
        "strike" => Some(STRIKE_ACTION),
        "rest" => Some(REST_ACTION),
        _ => None,
    }
}

use std::fmt;
use worldwright_sdk::{
    GameModule, ModuleRegistrationError,
    command::{CommandIntent, CommandKind, PlayerText},
    effect::{Audience, Effect, EffectPlan, ScheduledAction},
    ids::{CharacterId, DefinitionId},
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    runtime::{WorldBuilder, WorldError},
    time::RandomSource,
};

pub const GAME_ID: &str = "worldwright-expedition";
pub const GAME_CONTRACT_VERSION: u32 = 1;
pub const SUPPLY_CAPACITY: u64 = 9;
pub const FORAGE_ACTION: &str = "expedition:actions.forage";
pub const ENCOUNTER_ACTION: &str = "expedition:actions.encounter";
pub const STRIKE_ACTION: &str = "expedition:actions.strike";
pub const REST_ACTION: &str = "expedition:actions.rest";
pub const SUPPLIES_KEY: &str = "expedition:state.supplies";
pub const THREAT_KEY: &str = "expedition:state.threat";
pub const VICTORIES_KEY: &str = "expedition:state.victories";
pub const RECOVERY_PENDING_KEY: &str = "expedition:state.recovery_pending";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rank {
    Scout,
    Pathfinder,
    Trailblazer,
}

impl fmt::Display for Rank {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Scout => "Scout",
            Self::Pathfinder => "Pathfinder",
            Self::Trailblazer => "Trailblazer",
        })
    }
}

pub const fn rank(victories: u64) -> Rank {
    match victories {
        0..=2 => Rank::Scout,
        3..=5 => Rank::Pathfinder,
        _ => Rank::Trailblazer,
    }
}

pub fn initialize_character(
    builder: &mut WorldBuilder,
    character: CharacterId,
) -> Result<(), WorldError> {
    builder.set_character_counter(character, key(SUPPLIES_KEY)?, 3)?;
    builder.set_character_counter(character, key(THREAT_KEY)?, 0)?;
    builder.set_character_counter(character, key(VICTORIES_KEY)?, 0)?;
    builder.set_character_counter(character, key(RECOVERY_PENDING_KEY)?, 0)?;
    Ok(())
}

fn key(value: &str) -> Result<DefinitionId, WorldError> {
    DefinitionId::parse(value).map_err(|_| WorldError::IntentViolation)
}

fn rule_key(value: &str) -> Result<DefinitionId, RuleError> {
    DefinitionId::parse(value).map_err(|_| RuleError::Rejected)
}

pub struct ExpeditionGame;

impl GameModule for ExpeditionGame {
    fn name(&self) -> &str {
        GAME_ID
    }

    fn register_rules(&self, registry: &mut RuleRegistry) -> Result<(), ModuleRegistrationError> {
        for kind in [
            CommandKind::Look,
            CommandKind::Move,
            CommandKind::Say,
            CommandKind::GameAction,
        ] {
            registry.register(kind, ExpeditionRule)?;
        }
        Ok(())
    }
}

struct ExpeditionRule;

impl Rule for ExpeditionRule {
    fn evaluate(
        &self,
        context: &RuleContext<'_>,
        command: &CommandIntent,
        random: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        let actor = context.actor.character;
        let supplies_key = rule_key(SUPPLIES_KEY)?;
        let threat_key = rule_key(THREAT_KEY)?;
        let victories_key = rule_key(VICTORIES_KEY)?;
        let recovery_pending_key = rule_key(RECOVERY_PENDING_KEY)?;
        let supplies = counter(context, actor, supplies_key.clone())?;
        let threat = counter(context, actor, threat_key.clone())?;
        let victories = counter(context, actor, victories_key.clone())?;
        let recovery_pending = counter(context, actor, recovery_pending_key.clone())?;
        let mut plan = EffectPlan::new();
        let mut push = |effect| plan.push(effect).map_err(|_| RuleError::PlanLimit);

        match command {
            CommandIntent::Look => {
                let room = context
                    .world
                    .character_room(actor)
                    .ok_or(RuleError::Rejected)?;
                let description = context
                    .world
                    .room_description(room)
                    .ok_or(RuleError::Rejected)?;
                push(Effect::EmitMessage {
                    audience: Audience::Character(actor),
                    text: description.title,
                })?;
                push(Effect::EmitMessage {
                    audience: Audience::Character(actor),
                    text: description.description,
                })?;
                push(private_message(
                    actor,
                    &format!(
                        "Supplies: {supplies} | Threat: {threat} | Victories: {victories} | Rank: {}",
                        rank(victories)
                    ),
                )?)?;
            }
            CommandIntent::Move { destination } => {
                if supplies == 0 || threat != 0 {
                    return Err(RuleError::Rejected);
                }
                push(Effect::AdjustCounter {
                    character: actor,
                    key: supplies_key,
                    delta: -1,
                })?;
                push(Effect::MoveCharacter {
                    character: actor,
                    destination: *destination,
                })?;
                push(private_message(
                    actor,
                    "The expedition spends one supply to travel.",
                )?)?;
            }
            CommandIntent::Say { text } => {
                let room = context
                    .world
                    .character_room(actor)
                    .ok_or(RuleError::Rejected)?;
                push(Effect::EmitMessage {
                    audience: Audience::Room(room),
                    text: text.clone(),
                })?;
            }
            CommandIntent::GameAction { action, argument } => {
                if argument.is_some() {
                    return Err(RuleError::Rejected);
                }
                match action.as_str() {
                    FORAGE_ACTION
                        if threat == 0 && recovery_pending == 0 && supplies < SUPPLY_CAPACITY =>
                    {
                        let rolled = random
                            .next_u64()
                            .map_err(|_| RuleError::RandomUnavailable)?
                            % 3
                            + 1;
                        let gained = rolled.min(SUPPLY_CAPACITY - supplies);
                        push(Effect::AdjustCounter {
                            character: actor,
                            key: supplies_key,
                            delta: gained as i64,
                        })?;
                        push(private_message(
                            actor,
                            &format!("Foraging recovers {gained} supplies."),
                        )?)?;
                    }
                    ENCOUNTER_ACTION if threat == 0 => {
                        let danger = random
                            .next_u64()
                            .map_err(|_| RuleError::RandomUnavailable)?
                            % 3
                            + 2;
                        push(Effect::AdjustCounter {
                            character: actor,
                            key: threat_key,
                            delta: danger as i64,
                        })?;
                        push(private_message(
                            actor,
                            &format!("An encounter begins with threat {danger}."),
                        )?)?;
                    }
                    STRIKE_ACTION if threat > 0 && supplies > 0 => {
                        push(Effect::AdjustCounter {
                            character: actor,
                            key: supplies_key.clone(),
                            delta: -1,
                        })?;
                        push(Effect::AdjustCounter {
                            character: actor,
                            key: threat_key,
                            delta: -1,
                        })?;
                        if threat == 1 {
                            push(Effect::AdjustCounter {
                                character: actor,
                                key: victories_key,
                                delta: 1,
                            })?;
                            push(Effect::AdjustCounter {
                                character: actor,
                                key: recovery_pending_key.clone(),
                                delta: 1,
                            })?;
                            push(Effect::Schedule {
                                at: context
                                    .now
                                    .checked_add(1_000)
                                    .map_err(|_| RuleError::Rejected)?,
                                action: ScheduledAction::AdjustCounter {
                                    character: actor,
                                    key: supplies_key,
                                    delta: 1,
                                },
                            })?;
                            push(Effect::Schedule {
                                at: context
                                    .now
                                    .checked_add(1_000)
                                    .map_err(|_| RuleError::Rejected)?,
                                action: ScheduledAction::AdjustCounter {
                                    character: actor,
                                    key: recovery_pending_key,
                                    delta: -1,
                                },
                            })?;
                            push(private_message(
                                actor,
                                &format!(
                                    "Encounter resolved. Rank: {}.",
                                    rank(victories.saturating_add(1))
                                ),
                            )?)?;
                        } else {
                            push(private_message(
                                actor,
                                &format!("The encounter has {} threat remaining.", threat - 1),
                            )?)?;
                        }
                    }
                    REST_ACTION
                        if threat == 0 && recovery_pending == 0 && supplies < SUPPLY_CAPACITY =>
                    {
                        push(Effect::AdjustCounter {
                            character: actor,
                            key: recovery_pending_key.clone(),
                            delta: 1,
                        })?;
                        push(Effect::Schedule {
                            at: context
                                .now
                                .checked_add(1_000)
                                .map_err(|_| RuleError::Rejected)?,
                            action: ScheduledAction::AdjustCounter {
                                character: actor,
                                key: supplies_key,
                                delta: 1,
                            },
                        })?;
                        push(Effect::Schedule {
                            at: context
                                .now
                                .checked_add(1_000)
                                .map_err(|_| RuleError::Rejected)?,
                            action: ScheduledAction::AdjustCounter {
                                character: actor,
                                key: recovery_pending_key,
                                delta: -1,
                            },
                        })?;
                        push(private_message(
                            actor,
                            "Recovery is scheduled for the next turn.",
                        )?)?;
                    }
                    _ => return Err(RuleError::Rejected),
                }
            }
        }
        Ok(plan)
    }
}

fn counter(
    context: &RuleContext<'_>,
    actor: CharacterId,
    key: DefinitionId,
) -> Result<u64, RuleError> {
    context
        .world
        .character_counter(actor, key)
        .ok_or(RuleError::Rejected)
}

fn private_message(character: CharacterId, text: &str) -> Result<Effect, RuleError> {
    Ok(Effect::EmitMessage {
        audience: Audience::Character(character),
        text: PlayerText::new(text).map_err(|_| RuleError::Rejected)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldwright_core::{
        command::{ActorContext, CommandEnvelope},
        ids::{AccountId, ConnectionId, OperationId, RoomId, WorldId},
        rule::RuleRegistry,
        runtime::WorldRuntime,
        time::{RandomExhausted, Tick},
    };

    struct FixedRandom(u64);
    impl RandomSource for FixedRandom {
        fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
            Ok(self.0)
        }
    }

    fn fixture() -> (WorldRuntime, ActorContext, CharacterId, RoomId) {
        let world = WorldId::from_u128(1).unwrap();
        let camp = RoomId::from_u128(1).unwrap();
        let pass = RoomId::from_u128(2).unwrap();
        let character = CharacterId::from_u128(1).unwrap();
        let account = AccountId::from_u128(1).unwrap();
        let mut builder = WorldBuilder::new(world);
        builder
            .add_room(
                camp,
                PlayerText::new("Camp").unwrap(),
                PlayerText::new("A safe camp.").unwrap(),
            )
            .unwrap();
        builder
            .add_room(
                pass,
                PlayerText::new("Pass").unwrap(),
                PlayerText::new("A narrow pass.").unwrap(),
            )
            .unwrap();
        builder.add_exit(camp, pass).unwrap();
        builder.add_character(character, account, camp).unwrap();
        initialize_character(&mut builder, character).unwrap();
        let mut rules = RuleRegistry::new();
        ExpeditionGame.register_rules(&mut rules).unwrap();
        let mut runtime = builder.build(rules).unwrap();
        let actor = runtime
            .attach(account, character, ConnectionId::from_u128(1).unwrap())
            .unwrap();
        (runtime, actor, character, pass)
    }

    fn action(
        runtime: &mut WorldRuntime,
        actor: ActorContext,
        operation: u128,
        action: &str,
        at: u64,
        random: u64,
    ) {
        runtime
            .execute(
                &CommandEnvelope {
                    operation: OperationId::from_u128(operation).unwrap(),
                    world: WorldId::from_u128(1).unwrap(),
                    actor,
                    intent: CommandIntent::GameAction {
                        action: DefinitionId::parse(action).unwrap(),
                        argument: None,
                    },
                },
                Tick::from_millis(at),
                &mut FixedRandom(random),
            )
            .unwrap();
    }

    #[test]
    fn progression_thresholds_are_stable() {
        assert_eq!(rank(0), Rank::Scout);
        assert_eq!(rank(2), Rank::Scout);
        assert_eq!(rank(3), Rank::Pathfinder);
        assert_eq!(rank(5), Rank::Pathfinder);
        assert_eq!(rank(6), Rank::Trailblazer);
        assert_eq!(rank(u64::MAX), Rank::Trailblazer);
    }

    #[test]
    fn public_identifiers_are_valid_and_namespaced() {
        for value in [
            FORAGE_ACTION,
            ENCOUNTER_ACTION,
            STRIKE_ACTION,
            REST_ACTION,
            SUPPLIES_KEY,
            THREAT_KEY,
            VICTORIES_KEY,
            RECOVERY_PENDING_KEY,
        ] {
            assert!(DefinitionId::parse(value).is_ok(), "{value}");
        }
    }

    #[test]
    fn encounter_strikes_progress_and_scheduled_recovery_are_transactional() {
        let (mut runtime, actor, character, _) = fixture();
        action(&mut runtime, actor, 1, ENCOUNTER_ACTION, 1, 0);
        assert_eq!(
            runtime
                .snapshot()
                .counter(character, DefinitionId::parse(THREAT_KEY).unwrap()),
            Some(2)
        );
        action(&mut runtime, actor, 2, STRIKE_ACTION, 2, 0);
        action(&mut runtime, actor, 3, STRIKE_ACTION, 3, 0);
        let snapshot = runtime.snapshot();
        assert_eq!(
            snapshot.counter(character, DefinitionId::parse(THREAT_KEY).unwrap()),
            Some(0)
        );
        assert_eq!(
            snapshot.counter(character, DefinitionId::parse(VICTORIES_KEY).unwrap()),
            Some(1)
        );
        assert_eq!(snapshot.scheduled_actions(), 2);
        assert!(
            runtime
                .advance(Tick::from_millis(1_002))
                .unwrap()
                .events
                .is_empty()
        );
        let due = runtime.advance(Tick::from_millis(1_003)).unwrap();
        assert_eq!(due.events.len(), 2);
        assert_eq!(
            runtime
                .snapshot()
                .counter(character, DefinitionId::parse(SUPPLIES_KEY).unwrap()),
            Some(2)
        );
    }

    #[test]
    fn movement_spends_supply_and_takeover_preserves_progress() {
        let (mut runtime, actor, character, destination) = fixture();
        runtime
            .execute(
                &CommandEnvelope {
                    operation: OperationId::from_u128(1).unwrap(),
                    world: WorldId::from_u128(1).unwrap(),
                    actor,
                    intent: CommandIntent::Move { destination },
                },
                Tick::from_millis(1),
                &mut FixedRandom(0),
            )
            .unwrap();
        let replacement = runtime
            .attach(
                AccountId::from_u128(1).unwrap(),
                character,
                ConnectionId::from_u128(2).unwrap(),
            )
            .unwrap();
        assert_ne!(replacement.generation, actor.generation);
        assert_eq!(
            runtime
                .snapshot()
                .counter(character, DefinitionId::parse(SUPPLIES_KEY).unwrap()),
            Some(2)
        );
    }

    #[test]
    fn rejected_actions_do_not_mutate_revision_or_counters() {
        let (mut runtime, actor, character, _) = fixture();
        let before = runtime.snapshot();
        let error = runtime
            .execute(
                &CommandEnvelope {
                    operation: OperationId::from_u128(1).unwrap(),
                    world: WorldId::from_u128(1).unwrap(),
                    actor,
                    intent: CommandIntent::GameAction {
                        action: DefinitionId::parse(STRIKE_ACTION).unwrap(),
                        argument: None,
                    },
                },
                Tick::from_millis(1),
                &mut FixedRandom(0),
            )
            .unwrap_err();
        assert_eq!(error, WorldError::Rule(RuleError::Rejected));
        assert_eq!(runtime.snapshot().revision(), before.revision());
        assert_eq!(
            runtime
                .snapshot()
                .counter(character, DefinitionId::parse(SUPPLIES_KEY).unwrap()),
            Some(3)
        );
    }
}
