use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};
#[derive(Clone)]
pub struct Metrics {
    started: Instant,
    pub admission_rejections: Arc<AtomicU64>,
    pub operator_denied: Arc<AtomicU64>,
    pub drain: Arc<AtomicU64>,
}
impl Metrics {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            admission_rejections: Arc::new(AtomicU64::new(0)),
            operator_denied: Arc::new(AtomicU64::new(0)),
            drain: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn render(
        &self,
        lifecycle: u8,
        ready: bool,
        connected: u64,
        admissions: u64,
        backlog: u64,
        oldest: Option<u64>,
    ) -> String {
        format!(
            "worldwright_process_uptime_seconds {}\nworldwright_lifecycle {}\nworldwright_ready {}\nworldwright_connected_clients {}\nworldwright_admission_rejections_total {}\nworldwright_operator_denied_total {}\nworldwright_drain_total {}\nworldwright_outbox_backlog {}\nworldwright_outbox_oldest_seconds {}\n",
            self.started.elapsed().as_secs(),
            lifecycle,
            u8::from(ready),
            connected,
            admissions + self.admission_rejections.load(Ordering::Relaxed),
            self.operator_denied.load(Ordering::Relaxed),
            self.drain.load(Ordering::Relaxed),
            backlog,
            oldest.unwrap_or(0)
        )
    }
}

// Fixed dimensions only: no principal, request, artifact, text or provider labels.
const STAGES: [&str; 8] = [
    "queue",
    "owner",
    "planning",
    "sql",
    "delivery",
    "idle_tick",
    "snapshot",
    "timer_lag",
];
const BOUNDS_NS: [u64; 5] = [100_000, 1_000_000, 10_000_000, 100_000_000, 1_000_000_000];
#[derive(Default)]
struct Timing {
    count: AtomicU64,
    sum_ns: AtomicU64,
    buckets: [AtomicU64; 5],
}
#[derive(Default)]
pub(crate) struct GameplayTimings {
    stages: [Timing; 8],
}
pub(crate) struct TimingGuard {
    timings: Arc<GameplayTimings>,
    stage: usize,
    started: Instant,
}
fn saturating_add(value: &AtomicU64, increment: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(increment))
    });
}
impl GameplayTimings {
    pub(crate) fn start(self: &Arc<Self>, stage: usize) -> TimingGuard {
        TimingGuard {
            timings: Arc::clone(self),
            stage,
            started: Instant::now(),
        }
    }
    pub(crate) fn observe(&self, stage: usize, elapsed: std::time::Duration) {
        let Some(timing) = self.stages.get(stage) else {
            return;
        };
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        saturating_add(&timing.count, 1);
        saturating_add(&timing.sum_ns, ns);
        for (bound, bucket) in BOUNDS_NS.iter().zip(&timing.buckets) {
            if ns <= *bound {
                saturating_add(bucket, 1);
            }
        }
    }
    pub(crate) fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut output = String::with_capacity(4096);
        for (stage, timing) in STAGES.iter().zip(&self.stages) {
            let count = timing.count.load(Ordering::Relaxed);
            let _ = writeln!(
                output,
                "worldwright_gameplay_stage_seconds_count{{stage=\"{stage}\"}} {count}"
            );
            let _ = writeln!(
                output,
                "worldwright_gameplay_stage_seconds_sum{{stage=\"{stage}\"}} {}",
                timing.sum_ns.load(Ordering::Relaxed) as f64 / 1_000_000_000.0
            );
            for (bound, bucket) in BOUNDS_NS.iter().zip(&timing.buckets) {
                let _ = writeln!(
                    output,
                    "worldwright_gameplay_stage_seconds_bucket{{stage=\"{stage}\",le=\"{}\"}} {}",
                    *bound as f64 / 1_000_000_000.0,
                    bucket.load(Ordering::Relaxed)
                );
            }
            let _ = writeln!(
                output,
                "worldwright_gameplay_stage_seconds_bucket{{stage=\"{stage}\",le=\"+Inf\"}} {count}"
            );
        }
        output
    }
}
impl Drop for TimingGuard {
    fn drop(&mut self) {
        self.timings.observe(self.stage, self.started.elapsed());
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    #[test]
    fn bounded_histograms_have_fixed_dimensions_and_saturate() {
        let timings = GameplayTimings::default();
        timings.observe(0, std::time::Duration::from_micros(50));
        timings.observe(99, std::time::Duration::ZERO);
        let output = timings.render();
        assert_eq!(output.lines().count(), 64);
        assert!(output.contains("stage=\"queue\",le=\"0.0001\"} 1"));
        let value = AtomicU64::new(u64::MAX);
        saturating_add(&value, 1);
        assert_eq!(value.load(Ordering::Relaxed), u64::MAX);
    }
}
