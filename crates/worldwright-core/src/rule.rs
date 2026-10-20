//! Object-safe, trusted native rule interfaces and duplicate-safe registration.

use crate::{
    command::{ActorContext, CommandEnvelope, CommandIntent, CommandKind},
    effect::EffectPlan,
    time::{RandomSource, Tick},
    world::WorldView,
};
use std::{
    collections::{BTreeMap, btree_map::Entry},
    error::Error,
    fmt,
};

pub struct RuleContext<'a> {
    pub actor: &'a ActorContext,
    pub world: &'a dyn WorldView,
    pub now: Tick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleError {
    WrongWorld,
    MissingRule,
    Rejected,
    RandomUnavailable,
    PlanLimit,
}
impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WrongWorld => "command targets a different world",
            Self::MissingRule => "no rule registered for this command",
            Self::Rejected => "rule rejected the command",
            Self::RandomUnavailable => "gameplay randomness unavailable",
            Self::PlanLimit => "effect plan is too large",
        })
    }
}
impl Error for RuleError {}

/// A Rust trait is not a sandbox: implementors are trusted reviewed code.
/// Do not block, read the wall clock, use ambient RNG, or mutate external state.
pub trait Rule: Send + Sync {
    fn evaluate(
        &self,
        context: &RuleContext<'_>,
        command: &CommandIntent,
        random: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateRule(pub CommandKind);
impl fmt::Display for DuplicateRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "a rule for {:?} is already registered", self.0)
    }
}
impl Error for DuplicateRule {}

#[derive(Default)]
pub struct RuleRegistry {
    rules: BTreeMap<CommandKind, Box<dyn Rule>>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(
        &mut self,
        kind: CommandKind,
        rule: impl Rule + 'static,
    ) -> Result<(), DuplicateRule> {
        match self.rules.entry(kind) {
            Entry::Vacant(entry) => {
                entry.insert(Box::new(rule));
                Ok(())
            }
            Entry::Occupied(_) => Err(DuplicateRule(kind)),
        }
    }
    pub fn get(&self, kind: CommandKind) -> Option<&dyn Rule> {
        self.rules.get(&kind).map(|rule| rule.as_ref())
    }

    /// Runs a rule to produce an UNVALIDATED plan, not a game command execution.
    /// Caller must establish and revalidate authority before this call, and the
    /// future executor must validate/commit the resulting plan afterwards.
    pub fn plan(
        &self,
        envelope: &CommandEnvelope,
        world: &dyn WorldView,
        now: Tick,
        random: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        if envelope.world != world.world_id() {
            return Err(RuleError::WrongWorld);
        }
        let rule = self
            .get(envelope.intent.kind())
            .ok_or(RuleError::MissingRule)?;
        let context = RuleContext {
            actor: &envelope.actor,
            world,
            now,
        };
        rule.evaluate(&context, &envelope.intent, random)
    }
}
