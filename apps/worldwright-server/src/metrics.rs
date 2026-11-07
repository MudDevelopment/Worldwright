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
