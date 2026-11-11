//! Observations only; attempted replacement cardinality is not committed row count.
use std::{
    fmt::Write as _,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
const STAGES: [&str; 3] = ["pool_begin", "commit", "replacement"];
const EVENTS: [&str; 9] = [
    "look",
    "move",
    "say",
    "game_action",
    "tick",
    "replay",
    "commit_confirmed",
    "replacement_counters_attempted",
    "replacement_schedules_attempted",
];
const BOUNDS: [u64; 5] = [100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];
#[derive(Default)]
struct Timing {
    count: AtomicU64,
    sum: AtomicU64,
    buckets: [AtomicU64; 5],
}
#[derive(Default)]
pub(crate) struct Measurements {
    stages: [Timing; 3],
    events: [AtomicU64; 9],
}
pub(crate) struct Guard {
    measurements: Arc<Measurements>,
    stage: usize,
    started: Instant,
}
fn add(value: &AtomicU64, increment: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        Some(old.saturating_add(increment))
    });
}
impl Measurements {
    pub(crate) fn event(&self, index: usize, count: u64) {
        if let Some(value) = self.events.get(index) {
            add(value, count);
        }
    }
    pub(crate) fn start(self: &Arc<Self>, stage: usize) -> Guard {
        Guard {
            measurements: Arc::clone(self),
            stage,
            started: Instant::now(),
        }
    }
    fn observe(&self, stage: usize, ns: u64) {
        let Some(value) = self.stages.get(stage) else {
            return;
        };
        add(&value.count, 1);
        add(&value.sum, ns);
        for (bound, bucket) in BOUNDS.iter().zip(&value.buckets) {
            if ns <= *bound {
                add(bucket, 1);
            }
        }
    }
    pub(crate) fn render(&self) -> String {
        let mut output = String::with_capacity(4096);
        for (name, value) in EVENTS.iter().zip(&self.events) {
            let _ = writeln!(
                output,
                "worldwright_persistence_events_total{{event=\"{name}\"}} {}",
                value.load(Ordering::Relaxed)
            );
        }
        for (stage, value) in STAGES.iter().zip(&self.stages) {
            let count = value.count.load(Ordering::Relaxed);
            let _ = writeln!(
                output,
                "worldwright_persistence_stage_seconds_count{{stage=\"{stage}\"}} {count}"
            );
            let _ = writeln!(
                output,
                "worldwright_persistence_stage_seconds_sum{{stage=\"{stage}\"}} {}",
                value.sum.load(Ordering::Relaxed) as f64 / 1_000_000_000.0
            );
            for (bound, bucket) in BOUNDS.iter().zip(&value.buckets) {
                let _ = writeln!(
                    output,
                    "worldwright_persistence_stage_seconds_bucket{{stage=\"{stage}\",le=\"{}\"}} {}",
                    *bound as f64 / 1_000_000_000.0,
                    bucket.load(Ordering::Relaxed)
                );
            }
            let _ = writeln!(
                output,
                "worldwright_persistence_stage_seconds_bucket{{stage=\"{stage}\",le=\"+Inf\"}} {count}"
            );
        }
        output
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.measurements.observe(
            self.stage,
            u64::try_from(self.started.elapsed().as_nanos()).unwrap_or(u64::MAX),
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_dimensions_saturate_and_clone_shares_observations() {
        let measurements = Arc::new(Measurements::default());
        let cloned = Arc::clone(&measurements);
        cloned.event(0, u64::MAX);
        cloned.event(0, 1);
        cloned.event(99, 1);
        measurements.observe(0, 50_000);
        let output = measurements.render();
        assert_eq!(output.lines().count(), 33);
        assert!(output.contains("event=\"look\"} 18446744073709551615"));
        assert!(output.contains("stage=\"pool_begin\",le=\"0.0001\"} 1"));
    }
}
