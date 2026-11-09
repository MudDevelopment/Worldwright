use crate::cli::Profile;
use serde::Serialize;

#[derive(Default, Serialize)]
pub(crate) struct FailureCounts {
    pub login: u64,
    pub attach: u64,
    pub command: u64,
    pub protocol: u64,
    pub disconnect: u64,
    pub readiness: u64,
    pub internal: u64,
}

impl FailureCounts {
    pub(crate) fn total(&self) -> u64 {
        self.login
            + self.attach
            + self.command
            + self.protocol
            + self.disconnect
            + self.readiness
            + self.internal
    }
}

#[derive(Default, Serialize)]
pub(crate) struct LatencyStats {
    pub count: u64,
    pub min_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
    pub max_us: u64,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FailureReason {
    ConnectedClientCount,
    CommandCount,
    LatencyCount,
    Failures,
    P95Latency,
    P99Latency,
    MaximumLatency,
    WallTime,
    SmokeProfile,
}

#[derive(Serialize)]
pub(crate) struct LoadReport {
    pub schema_version: u32,
    pub profile: String,
    pub profile_version: u32,
    pub storage_expectation: &'static str,
    pub server_revision: String,
    pub artifact_digest: String,
    pub environment_label: String,
    pub run_nonce: String,
    pub operating_system: &'static str,
    pub architecture: &'static str,
    pub started_unix_seconds: u64,
    pub ended_unix_seconds: u64,
    pub target_clients: u64,
    pub connected_clients: u64,
    pub expected_commands: u64,
    pub completed_commands: u64,
    pub failures: FailureCounts,
    pub readiness_checks: u64,
    pub elapsed_ms: u64,
    pub throughput_milli_commands_per_second: u64,
    pub latency: LatencyStats,
    pub passed: bool,
    pub failure_reasons: Vec<FailureReason>,
}

pub(crate) fn latency_stats(mut values: Vec<u64>) -> LatencyStats {
    if values.is_empty() {
        return LatencyStats::default();
    }
    values.sort_unstable();
    LatencyStats {
        count: values.len() as u64,
        min_us: values[0],
        p50_us: nearest_rank(&values, 50).unwrap_or_default(),
        p95_us: nearest_rank(&values, 95).unwrap_or_default(),
        p99_us: nearest_rank(&values, 99).unwrap_or_default(),
        max_us: values[values.len() - 1],
    }
}

fn nearest_rank(values: &[u64], percentile: usize) -> Option<u64> {
    worldwright_core::measurement::nearest_rank_index(values.len(), percentile)
        .map(|index| values[index])
}

pub(crate) fn evaluate(report: &mut LoadReport, profile: Profile) {
    let mut reasons = Vec::new();
    if profile != Profile::Beta250 {
        reasons.push(FailureReason::SmokeProfile);
    }
    if report.connected_clients != 250 {
        reasons.push(FailureReason::ConnectedClientCount);
    }
    if report.completed_commands != 7_500 {
        reasons.push(FailureReason::CommandCount);
    }
    if report.latency.count != report.completed_commands {
        reasons.push(FailureReason::LatencyCount);
    }
    if report.failures.total() != 0 {
        reasons.push(FailureReason::Failures);
    }
    if report.latency.p95_us > 750_000 {
        reasons.push(FailureReason::P95Latency);
    }
    if report.latency.p99_us > 1_500_000 {
        reasons.push(FailureReason::P99Latency);
    }
    if report.latency.max_us >= 10_000_000 {
        reasons.push(FailureReason::MaximumLatency);
    }
    if report.elapsed_ms > 90_000 {
        reasons.push(FailureReason::WallTime);
    }
    report.passed = reasons.is_empty();
    report.failure_reasons = reasons;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_uses_integer_contract() {
        let stats = latency_stats((1..=100).collect());
        assert_eq!((stats.p50_us, stats.p95_us, stats.p99_us), (50, 95, 99));
        assert_eq!(latency_stats(Vec::new()).count, 0);
    }

    #[test]
    fn beta_percentiles_match_shared_small_and_full_sample_recipe() {
        let stats = latency_stats((1..=20).collect());
        assert_eq!((stats.p50_us, stats.p95_us, stats.p99_us), (10, 19, 20));
        let stats = latency_stats((1..=7_500).rev().collect());
        assert_eq!((stats.p50_us, stats.p95_us, stats.p99_us), (3_750, 7_125, 7_425));
        assert_eq!(nearest_rank(&[], 50), None);
        assert_eq!(nearest_rank(&[1], 0), None);
        assert_eq!(nearest_rank(&[1], 101), None);
    }

    fn passing_report() -> LoadReport {
        LoadReport {
            schema_version: 1,
            profile: "beta-250-v1".into(),
            profile_version: 1,
            storage_expectation: "operator-supplied-postgresql",
            server_revision: "revision".into(),
            artifact_digest: "a".repeat(64),
            environment_label: "test".into(),
            run_nonce: "0".repeat(16),
            operating_system: "test",
            architecture: "test",
            started_unix_seconds: 1,
            ended_unix_seconds: 2,
            target_clients: 250,
            connected_clients: 250,
            expected_commands: 7_500,
            completed_commands: 7_500,
            failures: FailureCounts::default(),
            readiness_checks: 60,
            elapsed_ms: 90_000,
            throughput_milli_commands_per_second: 1,
            latency: LatencyStats {
                count: 7_500,
                min_us: 1,
                p50_us: 1,
                p95_us: 750_000,
                p99_us: 1_500_000,
                max_us: 9_999_999,
            },
            passed: false,
            failure_reasons: Vec::new(),
        }
    }

    #[test]
    fn evaluator_accepts_boundaries_and_rejects_each_overage() {
        let mut report = passing_report();
        evaluate(&mut report, Profile::Beta250);
        assert!(report.passed);

        let mut report = passing_report();
        report.latency.p95_us += 1;
        evaluate(&mut report, Profile::Beta250);
        assert_eq!(report.failure_reasons, [FailureReason::P95Latency]);

        let mut report = passing_report();
        report.latency.p99_us += 1;
        evaluate(&mut report, Profile::Beta250);
        assert_eq!(report.failure_reasons, [FailureReason::P99Latency]);

        let mut report = passing_report();
        report.latency.max_us += 1;
        evaluate(&mut report, Profile::Beta250);
        assert_eq!(report.failure_reasons, [FailureReason::MaximumLatency]);

        let mut report = passing_report();
        report.elapsed_ms += 1;
        evaluate(&mut report, Profile::Beta250);
        assert_eq!(report.failure_reasons, [FailureReason::WallTime]);
    }
}
