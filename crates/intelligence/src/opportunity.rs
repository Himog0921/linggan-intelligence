//! Opportunity metric definitions and period helpers. Thresholds come from the database (R3);
//! this module never embeds a time zone or a period constant.

mod card;
mod commenter_hash;
mod invariants;
mod mentions;
mod metric_definition;
mod platform_time;
mod problem_metrics;
mod stats;

pub(crate) use card::enqueue_source_events;
pub use card::{CardError, CardRebuildMode, CardRebuildReport, rebuild_opportunity_cards};
pub use commenter_hash::{COMMENTER_DIGEST_VERSION, CommenterDigest, CommenterDigestError};
pub use invariants::{InvariantCheck, InvariantReport, check_invariants};
pub use mentions::{
    MentionEligibilityCount, MentionProblemReconciliation, MentionSyncError, MentionSyncReport,
    sync_comment_mentions, sync_comment_mentions_for_domain,
};
pub use metric_definition::{
    OpportunityMetricDefinition, OpportunityMetricError, OpportunityMetricParams, Period,
    PeriodBasis, WeekStart, load_current_metric_definition,
};
pub use platform_time::{
    DisplayTextAgreement, DisplayTextMismatch, PlatformTimeCoverageRow, PlatformTimeProjection,
    PlatformTimeReport, current_as_of, project_comment_platform_times, read_platform_time_report,
};
pub use problem_metrics::{
    CreatorDensity, CreatorDensityGroup, CreatorWindowExclusions, MetricCalculationError,
    MetricExclusion, MetricIdentityConflicts, MetricPeriod, MetricRebuildReport, RebuildMode,
    rebuild_problem_metrics, sort_creator_density_group,
};
pub use stats::{
    EvidenceTier, Interval, StatsError, TierInput, TierParams, evidence_tier, rate_per_1k,
    wilson_interval,
};
