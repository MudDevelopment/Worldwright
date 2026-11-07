use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tokio::sync::watch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LifecycleState {
    Running = 0,
    Draining = 1,
    Quiescent = 2,
    Stopping = 3,
    Stopped = 4,
}

#[derive(Clone)]
pub struct LifecycleController {
    state: Arc<AtomicU8>,
    notify: watch::Sender<LifecycleState>,
}
impl LifecycleController {
    pub fn new() -> Self {
        let (notify, _) = watch::channel(LifecycleState::Running);
        Self {
            state: Arc::new(AtomicU8::new(0)),
            notify,
        }
    }
    pub fn state(&self) -> LifecycleState {
        match self.state.load(Ordering::Acquire) {
            1 => LifecycleState::Draining,
            2 => LifecycleState::Quiescent,
            3 => LifecycleState::Stopping,
            4 => LifecycleState::Stopped,
            _ => LifecycleState::Running,
        }
    }
    pub fn transition(&self, next: LifecycleState) -> bool {
        let mut current = self.state.load(Ordering::Acquire);
        while (next as u8) > current {
            match self.state.compare_exchange(
                current,
                next as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    let _ = self.notify.send(next);
                    return true;
                }
                Err(value) => current = value,
            }
        }
        false
    }
    #[allow(dead_code)]
    pub fn subscribe(&self) -> watch::Receiver<LifecycleState> {
        self.notify.subscribe()
    }
    pub fn drain(&self) -> bool {
        self.transition(LifecycleState::Draining)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_are_strictly_monotonic_and_drain_is_once() {
        let lifecycle = LifecycleController::new();
        assert!(!lifecycle.transition(LifecycleState::Running));
        assert!(lifecycle.drain());
        assert!(!lifecycle.drain());
        assert!(!lifecycle.transition(LifecycleState::Running));
        assert!(lifecycle.transition(LifecycleState::Quiescent));
        assert!(!lifecycle.transition(LifecycleState::Quiescent));
    }
}
