//! Cooperative shutdown controller.
//!
//! `main.rs` combines this with OS signals to perform graceful shutdown.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::Notify;

/// Cooperative shutdown signal used for graceful exit.
///
/// This is primarily used by the self-update flow to request a restart without relying
/// solely on OS signals.
#[derive(Clone, Debug, Default)]
pub struct ShutdownController {
    inner: Arc<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    requested: AtomicBool,
    notify: Notify,
}

impl ShutdownController {
    /// Returns true if shutdown has been requested.
    pub fn is_shutdown_requested(&self) -> bool {
        self.inner.requested.load(Ordering::Relaxed)
    }

    /// Request shutdown and wake all waiters.
    pub fn request_shutdown(&self) {
        self.inner.requested.store(true, Ordering::SeqCst);
        self.inner.notify.notify_waiters();
    }

    /// Wait until shutdown is requested.
    pub async fn wait(&self) {
        if self.is_shutdown_requested() {
            return;
        }
        self.inner.notify.notified().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_state_not_requested() {
        let ctrl = ShutdownController::default();
        assert!(!ctrl.is_shutdown_requested());
    }

    #[test]
    fn after_request_shutdown_is_requested() {
        let ctrl = ShutdownController::default();
        ctrl.request_shutdown();
        assert!(ctrl.is_shutdown_requested());
    }

    #[tokio::test]
    async fn wait_resolves_on_request_shutdown() {
        let ctrl = ShutdownController::default();
        let ctrl2 = ctrl.clone();
        let handle = tokio::spawn(async move {
            ctrl2.wait().await;
        });
        ctrl.request_shutdown();
        // wait must complete without hanging
        tokio::time::timeout(std::time::Duration::from_secs(2), handle)
            .await
            .expect("timed out")
            .expect("task panicked");
    }

    #[test]
    fn double_request_shutdown_is_safe() {
        let ctrl = ShutdownController::default();
        ctrl.request_shutdown();
        ctrl.request_shutdown();
        assert!(ctrl.is_shutdown_requested());
    }

    #[test]
    fn clone_shares_state() {
        let ctrl = ShutdownController::default();
        let cloned = ctrl.clone();
        assert!(!cloned.is_shutdown_requested());
        ctrl.request_shutdown();
        assert!(cloned.is_shutdown_requested());
    }

    #[test]
    fn cloned_can_request_shutdown() {
        let ctrl = ShutdownController::default();
        let cloned = ctrl.clone();
        cloned.request_shutdown();
        assert!(ctrl.is_shutdown_requested());
    }

    #[tokio::test]
    async fn wait_returns_immediately_if_already_requested() {
        let ctrl = ShutdownController::default();
        ctrl.request_shutdown();
        // wait should return immediately since shutdown was already requested
        tokio::time::timeout(std::time::Duration::from_millis(100), ctrl.wait())
            .await
            .expect("should return immediately");
    }

    #[tokio::test]
    async fn multiple_waiters_all_notified() {
        let ctrl = ShutdownController::default();
        let c1 = ctrl.clone();
        let c2 = ctrl.clone();
        let h1 = tokio::spawn(async move {
            c1.wait().await;
        });
        let h2 = tokio::spawn(async move {
            c2.wait().await;
        });
        tokio::task::yield_now().await;
        ctrl.request_shutdown();
        tokio::time::timeout(std::time::Duration::from_secs(2), h1)
            .await
            .expect("timed out")
            .expect("h1 panicked");
        tokio::time::timeout(std::time::Duration::from_secs(2), h2)
            .await
            .expect("timed out")
            .expect("h2 panicked");
    }

    #[test]
    fn debug_format() {
        let ctrl = ShutdownController::default();
        let debug = format!("{:?}", ctrl);
        assert!(debug.contains("ShutdownController"));
    }

    #[test]
    fn default_creates_non_requested() {
        let ctrl = ShutdownController::default();
        assert!(!ctrl.is_shutdown_requested());
    }
}
