mod coverage;
mod package;
mod record;
mod target;

pub use coverage::{Coverage, RemainingScope, Terminal, TerminalReason};
pub use package::{CapturePackage, ContractError, PackageRouting, parse_capture_package};
pub use record::CaptureRecord;
pub use target::{KnownSetTarget, KnownTargetOutcome, KnownTargetResult};

/// SCOPE-001 freezes a single capture unit; the enum exists so an unknown unit fails the
/// schema instead of silently degrading to a string comparison.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContentDetailUnit {
    ContentDetail,
}
