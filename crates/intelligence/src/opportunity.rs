//! Opportunity metric definitions and period helpers. Thresholds come from the database (R3);
//! this module never embeds a time zone or a period constant.

mod commenter_hash;
mod invariants;
mod metric_definition;
mod platform_time;
mod stats;

pub use commenter_hash::{COMMENTER_DIGEST_VERSION, CommenterDigest, CommenterDigestError};
pub use invariants::{InvariantCheck, InvariantReport, check_invariants};
pub use metric_definition::{
    OpportunityMetricDefinition, OpportunityMetricError, OpportunityMetricParams, Period,
    PeriodBasis, WeekStart, load_current_metric_definition,
};
pub use platform_time::{
    DisplayTextAgreement, DisplayTextMismatch, PlatformTimeCoverageRow, PlatformTimeProjection,
    PlatformTimeReport, current_as_of, project_comment_platform_times, read_platform_time_report,
};
pub use stats::{
    EvidenceTier, Interval, StatsError, TierInput, TierParams, evidence_tier, rate_per_1k,
    wilson_interval,
};
