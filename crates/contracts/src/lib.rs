//! Versioned boundary contracts. SCOPE-001 authorizes only the frozen
//! `content-detail.synthetic.v1` proof contract; real producer contracts require a later scope.

mod canonical;
mod capture;
mod discovery;
mod local_producer;
mod producer_runtime;

pub use capture::{
    CapturePackage, CaptureRecord, ContractError, Coverage, KnownSetTarget, KnownTargetOutcome,
    KnownTargetResult, PackageRouting, RemainingScope, Terminal, TerminalReason,
    parse_capture_package,
};
pub use discovery::{
    AcquisitionSpec, CoverPresentationState, DiscoveryCard, DiscoveryContractError,
    DiscoveryCoverage, DiscoveryOccurrence, DiscoveryPackage, DiscoveryStopReason, EvidenceQuery,
    EvidenceQueryScope, EvidenceQuerySort, EvidenceTimeView, PublishedWindow, is_rfc3339_timestamp,
    parse_discovery_package,
};
pub use local_producer::{
    LocalProducerAttempt, LocalProducerContractError, LocalProducerSubmission, LocalTaskSpec,
    parse_local_producer_attempt, parse_local_producer_submission, parse_local_task_spec,
};
pub use producer_runtime::{
    CAPTURE_PACKAGE_VERSION, PRODUCER_ATTEMPT_VERSION, PRODUCER_TASK_SPEC_VERSION, ProducerAttempt,
    ProducerCapturePackage, ProducerRuntimeContractError, ProducerSubmission, ProducerTaskSpec,
    parse_producer_attempt, parse_producer_capture_package, parse_producer_submission,
    parse_producer_task_spec,
};

pub const BOOTSTRAP_CONTRACT_VERSION: &str = "unimplemented";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_contract_version_is_not_a_real_contract() {
        assert_eq!(BOOTSTRAP_CONTRACT_VERSION, "unimplemented");
    }
}
