//! One-way stop intent for AI execution. Instance/item ownership remains in the database.
//! A stop closes new reservations while bounded in-flight calls and receipts finish.

use linggan_contracts::ai_limits::AI_LIMITS;
use std::{sync::LazyLock, time::Duration};
use tokio::sync::watch;

/// The shared resource contract bounds every valid frozen semantic invocation.
pub static MAX_FROZEN_MODEL_TIMEOUT: LazyLock<Duration> =
    LazyLock::new(|| Duration::from_secs(AI_LIMITS.max_timeout_seconds));
pub static PI_ADAPTER_DEADLINE_ALLOWANCE: LazyLock<Duration> =
    LazyLock::new(|| Duration::from_secs(AI_LIMITS.adapter_deadline_allowance_seconds));
pub static MODEL_RECEIPT_FINALIZATION_ALLOWANCE: LazyLock<Duration> =
    LazyLock::new(|| Duration::from_secs(AI_LIMITS.receipt_finalization_allowance_seconds));
pub static MODEL_WORKER_DRAIN_GRACE: LazyLock<Duration> =
    LazyLock::new(|| Duration::from_secs(AI_LIMITS.drain_grace_seconds()));

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
        assert!(*MODEL_WORKER_DRAIN_GRACE >= *MAX_FROZEN_MODEL_TIMEOUT);
        assert_eq!(
            *MODEL_WORKER_DRAIN_GRACE,
            *MAX_FROZEN_MODEL_TIMEOUT
                + *PI_ADAPTER_DEADLINE_ALLOWANCE
                + *MODEL_RECEIPT_FINALIZATION_ALLOWANCE
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
