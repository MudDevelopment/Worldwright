//! Deterministic fixtures and bounded scenarios over the real core runtime.

use std::{error::Error, fmt};
use worldwright_core::{
    command::{ActorContext, CommandEnvelope, CommandIntent},
    ids::OperationId,
    runtime::{ExecutionResult, WorldError, WorldRuntime},
    time::{RandomSource, Tick},
};

pub mod benchmark;
pub mod expedition;
pub mod fixtures;
pub mod two_room;
pub use benchmark::{Workload, WorkloadError, WorkloadReport, run_runtime_workload};
pub use expedition::{ExpeditionWorld, expedition_world};
pub use fixtures::{FakeClock, SequenceRandom, WorldFixture};
pub use two_room::{TwoRoomWorld, two_room_world};

pub use worldwright_content as content;
pub use worldwright_sdk::GameModule;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeStep {
    pub actor: ActorContext,
    pub intent: CommandIntent,
    pub at: Tick,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeScenario {
    steps: Vec<RuntimeStep>,
}

impl RuntimeScenario {
    pub const MAX_STEPS: usize = 1024;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, step: RuntimeStep) -> Result<(), ScenarioError> {
        if self.steps.len() >= Self::MAX_STEPS {
            return Err(ScenarioError::Full);
        }
        self.steps.push(step);
        Ok(())
    }

    pub fn steps(&self) -> &[RuntimeStep] {
        &self.steps
    }

    pub fn run(
        &self,
        runtime: &mut WorldRuntime,
        random: &mut dyn RandomSource,
    ) -> Result<ScenarioReport, ScenarioError> {
        let world = runtime.snapshot().world_id();
        let mut executions = Vec::with_capacity(self.steps.len());
        for (index, step) in self.steps.iter().enumerate() {
            let operation = OperationId::from_u128((index + 1) as u128)
                .map_err(|_| ScenarioError::OperationExhausted)?;
            let result = runtime
                .execute(
                    &CommandEnvelope {
                        operation,
                        world,
                        actor: step.actor,
                        intent: step.intent.clone(),
                    },
                    step.at,
                    random,
                )
                .map_err(|source| ScenarioError::Step { index, source })?;
            executions.push(result);
        }
        Ok(ScenarioReport { executions })
    }
}

#[deprecated(note = "use RuntimeScenario")]
pub type ScenarioStub = RuntimeScenario;

#[derive(Debug, Default)]
pub struct ScenarioReport {
    executions: Vec<ExecutionResult>,
}

impl ScenarioReport {
    pub fn executions(&self) -> &[ExecutionResult] {
        &self.executions
    }
}

#[derive(Debug)]
pub enum ScenarioError {
    Full,
    OperationExhausted,
    Step { index: usize, source: WorldError },
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => formatter.write_str("scenario reached its 1024-step limit"),
            Self::OperationExhausted => formatter.write_str("scenario operation ID exhausted"),
            Self::Step { index, .. } => write!(formatter, "scenario step {index} failed"),
        }
    }
}

impl Error for ScenarioError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Step { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scenario_is_a_real_success() {
        let mut world = two_room_world().unwrap();
        let report = RuntimeScenario::new()
            .run(&mut world.runtime, &mut SequenceRandom::default())
            .unwrap();
        assert!(report.executions().is_empty());
    }
}
