use crate::{
    command::{CommandIntent, CommandKind, PlayerText},
    effect::{Audience, Effect, EffectPlan},
    game::{GameModule, ModuleRegistrationError},
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    time::RandomSource,
};

/// Minimal setting-neutral rules for the first in-process world.
/// Registration and planning do not themselves enforce execution authority.
pub struct ReferenceGame;
impl GameModule for ReferenceGame {
    fn name(&self) -> &str {
        "Worldwright two-room reference rules"
    }
    fn register_rules(&self, registry: &mut RuleRegistry) -> Result<(), ModuleRegistrationError> {
        for kind in [CommandKind::Look, CommandKind::Move, CommandKind::Say] {
            registry.register(kind, ReferenceRule)?;
        }
        Ok(())
    }
}

struct ReferenceRule;
impl Rule for ReferenceRule {
    fn evaluate(
        &self,
        context: &RuleContext<'_>,
        command: &CommandIntent,
        _random: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        let character = context.actor.character;
        let room = context
            .world
            .character_room(character)
            .ok_or(RuleError::Rejected)?;
        let mut plan = EffectPlan::new();
        let mut append = |effect| plan.push(effect).map_err(|_| RuleError::PlanLimit);
        match command {
            CommandIntent::Look => {
                let description = context
                    .world
                    .room_description(room)
                    .ok_or(RuleError::Rejected)?;
                append(Effect::EmitMessage {
                    audience: Audience::Character(character),
                    text: description.title,
                })?;
                append(Effect::EmitMessage {
                    audience: Audience::Character(character),
                    text: description.description,
                })?;
            }
            CommandIntent::Move { destination } => {
                append(Effect::MoveCharacter {
                    character,
                    destination: *destination,
                })?;
                append(Effect::EmitMessage {
                    audience: Audience::Character(character),
                    text: PlayerText::new("You move to the next room.")
                        .map_err(|_| RuleError::Rejected)?,
                })?;
            }
            CommandIntent::Say { text } => {
                append(Effect::EmitMessage {
                    audience: Audience::Room(room),
                    text: text.clone(),
                })?;
            }
        }
        Ok(plan)
    }
}
