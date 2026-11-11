//! Reproducible in-process workload measurement; not a capacity promise.

use std::{
    error::Error,
    fmt,
    time::{Duration, Instant},
};
use worldwright_core::{
    GameModule,
    command::{CommandEnvelope, CommandIntent, PlayerText},
    ids::{AccountId, CharacterId, ConnectionId, OperationId, RoomId, WorldId},
    rule::RuleRegistry,
    runtime::{ReferenceGame, WorldBuilder},
    time::{RandomExhausted, RandomSource, Tick},
};

/// V2 changes only percentile selection from the historical v1 interpolation index.
pub const RUNTIME_WORKLOAD_RECIPE: &str = "runtime-look-nearest-rank-v2";
pub const RUNTIME_WORKLOAD_RECIPE_VERSION: u32 = 2;
pub use worldwright_core::measurement::NEAREST_RANK_RECIPE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Workload {
    pub clients: usize,
    pub commands_per_client: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadReport {
    pub clients: usize,
    pub commands: usize,
    pub elapsed: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub max: Duration,
    pub delivered_messages: usize,
}

#[derive(Debug)]
pub enum WorkloadError {
    InvalidBounds,
    Setup,
    Execution,
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidBounds => "workload requires 1..=100 clients and 1..=10000 commands",
            Self::Setup => "workload setup failed",
            Self::Execution => "workload execution failed",
        })
    }
}
impl Error for WorkloadError {}

pub fn run_runtime_workload(workload: Workload) -> Result<WorkloadReport, WorkloadError> {
    if !(1..=100).contains(&workload.clients)
        || workload.commands_per_client == 0
        || workload
            .clients
            .checked_mul(workload.commands_per_client)
            .is_none_or(|count| count > 10_000)
    {
        return Err(WorkloadError::InvalidBounds);
    }
    let world = WorldId::from_u128(1).map_err(|_| WorkloadError::Setup)?;
    let room = RoomId::from_u128(1).map_err(|_| WorkloadError::Setup)?;
    let mut builder = WorldBuilder::new(world);
    builder
        .add_room(
            room,
            PlayerText::new("Benchmark").map_err(|_| WorkloadError::Setup)?,
            PlayerText::new("Bounded workload room.").map_err(|_| WorkloadError::Setup)?,
        )
        .map_err(|_| WorkloadError::Setup)?;
    for index in 1..=workload.clients {
        builder
            .add_character(
                CharacterId::from_u128(index as u128).map_err(|_| WorkloadError::Setup)?,
                AccountId::from_u128(index as u128).map_err(|_| WorkloadError::Setup)?,
                room,
            )
            .map_err(|_| WorkloadError::Setup)?;
    }
    let mut rules = RuleRegistry::new();
    ReferenceGame
        .register_rules(&mut rules)
        .map_err(|_| WorkloadError::Setup)?;
    let mut runtime = builder.build(rules).map_err(|_| WorkloadError::Setup)?;
    let mut actors = Vec::with_capacity(workload.clients);
    for index in 1..=workload.clients {
        actors.push(
            runtime
                .attach(
                    AccountId::from_u128(index as u128).map_err(|_| WorkloadError::Setup)?,
                    CharacterId::from_u128(index as u128).map_err(|_| WorkloadError::Setup)?,
                    ConnectionId::from_u128(index as u128).map_err(|_| WorkloadError::Setup)?,
                )
                .map_err(|_| WorkloadError::Setup)?,
        );
    }

    let commands = workload.clients * workload.commands_per_client;
    let mut latencies = Vec::with_capacity(commands);
    let mut operation = 1u128;
    let mut delivered_messages = 0usize;
    let started = Instant::now();
    let mut random = NoRandom;
    for _ in 0..workload.commands_per_client {
        for actor in &actors {
            let command_started = Instant::now();
            let result = runtime
                .execute(
                    &CommandEnvelope {
                        operation: OperationId::from_u128(operation)
                            .map_err(|_| WorkloadError::Execution)?,
                        world,
                        actor: *actor,
                        intent: CommandIntent::Look,
                    },
                    Tick::from_millis(operation as u64),
                    &mut random,
                )
                .map_err(|_| WorkloadError::Execution)?;
            operation += 1;
            delivered_messages += result.messages.len();
            latencies.push(command_started.elapsed());
        }
    }
    let elapsed = started.elapsed();
    latencies.sort_unstable();
    Ok(WorkloadReport {
        clients: workload.clients,
        commands,
        elapsed,
        p50: percentile(&latencies, 50).ok_or(WorkloadError::Execution)?,
        p95: percentile(&latencies, 95).ok_or(WorkloadError::Execution)?,
        p99: percentile(&latencies, 99).ok_or(WorkloadError::Execution)?,
        max: *latencies.last().ok_or(WorkloadError::Execution)?,
        delivered_messages,
    })
}

fn percentile(values: &[Duration], percentile: usize) -> Option<Duration> {
    worldwright_core::measurement::nearest_rank_index(values.len(), percentile)
        .map(|index| values[index])
}

struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        Err(RandomExhausted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_percentiles_use_versioned_nearest_rank() {
        let values: Vec<_> = (1..=20).map(Duration::from_nanos).collect();
        assert_eq!(percentile(&values, 50), Some(Duration::from_nanos(10)));
        assert_eq!(percentile(&values, 95), Some(Duration::from_nanos(19)));
        assert_eq!(percentile(&values, 99), Some(Duration::from_nanos(20)));
        assert_eq!(percentile(&[], 50), None);
        assert_eq!(percentile(&values, 0), None);
        assert_eq!(RUNTIME_WORKLOAD_RECIPE_VERSION, 2);
        assert_eq!(
            NEAREST_RANK_RECIPE,
            worldwright_core::measurement::NEAREST_RANK_RECIPE
        );
    }
    #[test]
    fn hundred_client_objective_completes_bounded_work() {
        let report = run_runtime_workload(Workload {
            clients: 100,
            commands_per_client: 2,
        })
        .unwrap();
        assert_eq!(report.commands, 200);
        assert_eq!(report.delivered_messages, 400);
        assert!(report.p50 <= report.max);
        assert!(report.p95 <= report.max);
        assert!(report.p99 <= report.max);
    }
}
