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

/// Register Unix termination/hangup before opening the serve loop. A platform
/// registration error is a startup failure, never an unhandled graceful-shutdown
/// claim. Windows retains Tokio's Ctrl+C handler.
pub(crate) fn shutdown_signal() -> std::io::Result<impl std::future::Future<Output = ()>> {
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let mut hangup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())?;
    Ok(async move {
        #[cfg(unix)]
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = terminate.recv() => {},
            _ = hangup.recv() => {},
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
    })
}

#[cfg(test)]
mod r3_signal_tests {
    #[tokio::test]
    async fn r3_shutdown_signals_register_before_waiting() {
        let signal = super::shutdown_signal().expect("shutdown handlers register");
        assert!(tokio::time::timeout(std::time::Duration::from_millis(10), signal)
            .await
            .is_err(), "registration is not itself a shutdown signal");
    }
}
