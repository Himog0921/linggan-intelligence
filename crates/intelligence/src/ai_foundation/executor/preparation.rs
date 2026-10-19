//! Expected pre-dispatch deferrals never leave a phantom running call or consume an attempt.
use super::{ExecutorError, queue::ClaimedItem};
use crate::{
    ai_foundation::store::FoundationError, content_workbench_ai::AiError,
    model_settings::ModelError,
};
use linggan_storage_postgres::Database;

pub(super) async fn handle(
    db: &Database,
    item: &ClaimedItem,
    error: ExecutorError,
) -> Result<(), ExecutorError> {
    match error {
        ExecutorError::Model(ModelError::SecretUnavailable) => {
            item.defer(db, "model_secret_unavailable").await
        }
        ExecutorError::Model(ModelError::InputLimit) => {
            item.skip_unattempted(db, "ai_input_limit_before_dispatch")
                .await
        }
        ExecutorError::Writing(AiError::Model)
        | ExecutorError::Configuration(
            FoundationError::ConfigurationIncomplete
            | FoundationError::NotConnected
            | FoundationError::NotFound,
        ) => item.defer(db, "configuration_unavailable").await,
        // The process may enter drain between a successful claim and prepare's running-only fence.
        // defer itself proves that the same current owner still holds an untouched item lease.
        ExecutorError::FenceLost => item.defer(db, "executor_draining").await,
        error => Err(error),
    }
}
