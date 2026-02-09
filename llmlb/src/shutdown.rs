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
