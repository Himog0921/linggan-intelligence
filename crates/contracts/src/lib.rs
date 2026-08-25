//! Versioned boundary contracts. SCOPE-001 authorizes only the frozen
//! `content-detail.synthetic.v1` proof contract; real producer contracts require a later scope.

mod canonical;
mod capture;
mod discovery;

pub use capture::{
    CapturePackage, CaptureRecord, ContractError, Coverage, KnownSetTarget, KnownTargetOutcome,
    KnownTargetResult, PackageRouting, RemainingScope, Terminal, TerminalReason,
    parse_capture_package,
};
pub use discovery::{
    AcquisitionSpec, CoverPresentationState, DiscoveryCard, DiscoveryContractError,
    DiscoveryCoverage, DiscoveryOccurrence, DiscoveryPackage, DiscoveryStopReason, EvidenceQuery,
    EvidenceQueryScope, EvidenceQuerySort, PublishedWindow, is_rfc3339_timestamp,
    parse_discovery_package,
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
