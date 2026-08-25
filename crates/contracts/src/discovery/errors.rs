use thiserror::Error;

#[derive(Debug, Error)]
pub enum DiscoveryContractError {
    #[error("discovery contract is not valid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("discovery contract schema is invalid: {0}")]
    SchemaInvalid(String),
    #[error("unsupported discovery contract version: {0}")]
    UnsupportedContractVersion(String),
    #[error("LOCAL-001's first discovery canary must remain the fixed XHS ADHD first-20 plan")]
    FirstCanaryChanged,
    #[error("discovery coverage does not match the delivered visible cards")]
    CoverageDoesNotMatchCards,
    #[error("discovery coverage cannot make processed cards disagree with emitted or failed cards")]
    CoverageProcessingMismatch,
    #[error("quota_reached requires visible cards to equal the maximum quota")]
    QuotaReachedBeforeMaximumQuota,
    #[error("a visible discovery card is missing a stable platform content identity")]
    MissingPlatformContentIdentity,
    #[error("a package or occurrence observedAt is blank or not an RFC 3339 timestamp")]
    InvalidObservedAt,
    #[error("a discovery occurrence does not match the package observation context")]
    OccurrenceContextMismatch,
    #[error("two discovery cards claim the same result position")]
    DuplicateResultPosition,
}
