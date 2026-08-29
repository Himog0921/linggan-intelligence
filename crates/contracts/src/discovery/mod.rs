//! LOCAL-001 / 001C-0's closed discovery and local retrieval boundary.
//!
//! This module intentionally models neither Evidence acceptance nor a plugin command. A
//! validated [`DiscoveryPackage`] only says what a discovery surface reported as visible. It is
//! not a detail capture, media acquisition, Observation, or a permission to perform a platform
//! request.
//!
//! The contract is split by cohesive responsibility: query instruction, visible-card facts,
//! package/coverage facts, validation, and errors. These re-exports preserve one stable wire
//! boundary while keeping validation helpers and wire-only fields private to this module.

mod content;
mod errors;
mod package;
mod query;
mod validation;

pub use content::{CoverPresentationState, DiscoveryCard, DiscoveryOccurrence};
pub use errors::DiscoveryContractError;
pub use package::{DiscoveryCoverage, DiscoveryPackage, DiscoveryStopReason};
pub use query::{
    AcquisitionSpec, EvidenceLaneState, EvidenceMaterialLane, EvidenceMediaKind, EvidenceQuery,
    EvidenceQueryScope, EvidenceQuerySort, EvidenceRestriction, EvidenceTimeView, PublishedWindow,
};
pub use validation::{is_rfc3339_timestamp, parse_discovery_package};
