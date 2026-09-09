//! Cooperative shutdown boundary for the single comment-model execution loop.
//!
//! A drain is deliberately one-way for a worker process: after it is requested the loop may
//! finish its one in-flight invocation and receipt, but it must not reserve another invocation.

use std::time::Duration;
use tokio::sync::watch;

/// `linggan_model_config.timeout_seconds` is frozen per invocation and constrained to 1..=60.
pub const MAX_FROZEN_MODEL_TIMEOUT: Duration = Duration::from_secs(60);
/// PiAdapter owns a one-second transport/process allowance beyond the frozen provider timeout.
pub const PI_ADAPTER_DEADLINE_ALLOWANCE: Duration = Duration::from_secs(1);
/// Database receipt, usage checkpoint, and terminal-state writes get their own bounded margin.
pub const MODEL_RECEIPT_FINALIZATION_ALLOWANCE: Duration = Duration::from_secs(15);
/// A deployment must wait long enough for the largest valid frozen invocation and its receipt.
pub const MODEL_WORKER_DRAIN_GRACE: Duration = Duration::from_secs(60 + 1 + 15);

#[derive(Clone)]
pub struct ModelWorkerDrain {
    requested: watch::Sender<bool>,
}

impl ModelWorkerDrain {
    pub fn new() -> Self {
        let (requested, _) = watch::channel(false);
        Self { requested }
    }

    pub fn request(&self) {
        self.requested.send_replace(true);
    }

    pub fn is_requested(&self) -> bool {
        *self.requested.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.requested.subscribe()
    }
}

impl Default for ModelWorkerDrain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_grace_covers_every_valid_frozen_timeout_and_receipt_margin() {
        assert!(MODEL_WORKER_DRAIN_GRACE >= MAX_FROZEN_MODEL_TIMEOUT);
        assert_eq!(
            MODEL_WORKER_DRAIN_GRACE,
            MAX_FROZEN_MODEL_TIMEOUT
                + PI_ADAPTER_DEADLINE_ALLOWANCE
                + MODEL_RECEIPT_FINALIZATION_ALLOWANCE
        );
    }

    #[tokio::test]
    async fn request_is_visible_to_existing_and_new_subscribers() {
        let drain = ModelWorkerDrain::new();
        let mut existing = drain.subscribe();
        drain.request();

        existing.changed().await.expect("drain sender stays alive");
        assert!(*existing.borrow());
        assert!(drain.is_requested());
        assert!(*drain.subscribe().borrow());
    }
}
