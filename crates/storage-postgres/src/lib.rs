//! PostgreSQL implementation of Comment fact admission V0.
//!
//! The public entry point always delegates to linggan-evidence, which invokes
//! the XHS runtime source contract before its transaction starts.

use core::fmt;
use std::collections::{BTreeMap, HashSet};

use linggan_contracts::{CapturePackageV0, ContextTextAvailabilityV0};
use linggan_domain::comment_research::{
    COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1, COMMENT_RESEARCH_EXECUTION_CONTRACT_V1, CleaningReasonCode,
    CommentResearchContextPackInputV1, CommentResearchContextPackV1, CommentResearchState,
    ContextPackReadinessV1, ContextPackRelatedDiscussionInputV1, ContextPackSourceTextV1,
    ResearchFingerprintInputV1, ResearchModelStrategyV1, build_comment_research_context_pack_v1,
    clean_comment_for_research, freeze_comment_research_context_pack_v1, research_fingerprint_v1,
};
use linggan_evidence::{
    CommentSourceIdentityV0, EvidencePreparationError, PreparedCommentContextEvidenceAdmissionV0,
    PreparedCommentEvidenceAdmissionV0, PreparedCommentRecordV0,
    PreparedContextDiscussionRecordKindV0, PreparedContextDiscussionRecordV0,
    PreparedContextWorkRecordV0, PreparedSourceEvidenceV0,
    prepare_xhs_comment_context_evidence_admission_v0, prepare_xhs_comment_evidence_admission_v0,
};
use linggan_observation::{
    CommentObservationDecisionV0, CurrentCommentFactV0, decide_comment_observation_v0,
};
use tokio::sync::Mutex;
use tokio_postgres::{Client, NoTls, Transaction};
use uuid::Uuid;

/// The one greenfield migration required by Comment fact storage V0.
pub const COMMENT_FACT_STORAGE_V0_MIGRATION: &str =
    include_str!("../../../database/migrations/0001_comment_fact_storage_v0.sql");

/// The follow-on migration for source-backed work and reply context. It
/// depends on `COMMENT_FACT_STORAGE_V0_MIGRATION` having already been applied.
pub const COMMENT_CONTEXT_STORAGE_V0_MIGRATION: &str =
    include_str!("../../../database/migrations/0002_comment_context_storage_v0.sql");

/// The immutable deterministic Cleaning Contract V1 storage extension.
/// It depends on Comment Fact Storage V0.
pub const COMMENT_DERIVATION_V1_MIGRATION: &str =
    include_str!("../../../database/migrations/0003_comment_derivation_v1.sql");

/// Append-only V1 run-input and conclusion foundations. This migration has no
/// worker, provider, queue, or HTTP entry point.
pub const COMMENT_RESEARCH_EXECUTION_FOUNDATION_V1_MIGRATION: &str =
    include_str!("../../../database/migrations/0004_comment_research_execution_foundation_v1.sql");

/// Adds an explicit initial `prepared` event for frozen inputs. This is a
/// lifecycle observation only; it does not imply a queue or an executor.
pub const COMMENT_RESEARCH_RUN_PREPARATION_V1_MIGRATION: &str =
    include_str!("../../../database/migrations/0005_comment_research_run_preparation_v1.sql");

/// A PostgreSQL adapter with no knowledge of HTTP, workers, models, or UI.
pub struct CommentFactStore {
    // One PostgreSQL connection is shared by read and write paths. Serializing
    // access prevents a transaction opened by the preparation endpoint from
    // accidentally capturing unrelated concurrent HTTP reads on this session.
    client: Mutex<Client>,
}

/// The largest page accepted by the current-comment read model.
///
/// This is a transport safety bound, not a statement about how many comments
/// should be researched together.
pub const CURRENT_COMMENT_VOICES_V0_MAX_LIMIT: i64 = 100;

/// The maximum number of current User Voices displayed by one strictly
/// read-only automatic-research scope preview. This is a display and planning
/// bound only: it never creates a run, reserves a comment, or freezes input.
pub const COMMENT_RESEARCH_PLAN_PREVIEW_V0_MAX_LIMIT: i64 = 100;

/// The default preview size keeps the first read small while still showing a
/// useful spread across source works.
pub const COMMENT_RESEARCH_PLAN_PREVIEW_V0_DEFAULT_LIMIT: i64 = 50;

/// Creation has the same explicit upper bound as the live scope preview. It
/// is a safety bound for one confirmation, never an automatic schedule.
pub const COMMENT_RESEARCH_RUN_PREPARATION_V1_MAX_LIMIT: i64 = 100;

/// Backfill is intentionally bounded and local. It has no model, network, or
/// worker dependency, and is never triggered by a User Voices read.
pub const COMMENT_DERIVATION_V1_MATERIALIZATION_MAX_LIMIT: i64 = 100;

/// A bounded, offset-based request for the User Voices V0 read model.
///
/// Pagination belongs here instead of the HTTP layer so other callers cannot
/// accidentally create an unbounded current-comment read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentVoicesPageRequestV0 {
    limit: i64,
    offset: i64,
    filter: CurrentCommentVoiceFilterV1,
}

impl CurrentCommentVoicesPageRequestV0 {
    pub fn new(limit: i64, offset: i64) -> Result<Self, StorageError> {
        Self::with_filter(limit, offset, CurrentCommentVoiceFilterV1::Available)
    }

    pub fn with_filter(
        limit: i64,
        offset: i64,
        filter: CurrentCommentVoiceFilterV1,
    ) -> Result<Self, StorageError> {
        if !(1..=CURRENT_COMMENT_VOICES_V0_MAX_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidCurrentCommentVoicesLimit);
        }
        if offset < 0 {
            return Err(StorageError::InvalidCurrentCommentVoicesOffset);
        }
        Ok(Self {
            limit,
            offset,
            filter,
        })
    }

    pub const fn limit(self) -> i64 {
        self.limit
    }

    pub const fn offset(self) -> i64 {
        self.offset
    }

    pub const fn filter(self) -> CurrentCommentVoiceFilterV1 {
        self.filter
    }
}

/// The only useful User Voices cleaning filters in V1. `Available` is the
/// default because both direct-ready expressions and short expressions that
/// need discussion context are valid research corpus; hard dropped/anomalous
/// observations are never a User Voices row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentCommentVoiceFilterV1 {
    Available,
    Ready,
    NeedsContext,
}

impl CurrentCommentVoiceFilterV1 {
    pub const fn as_storage_value(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Ready => "ready",
            Self::NeedsContext => "needs_context",
        }
    }
}

/// A bounded, read-only request for the comments which would be considered by
/// a future automatic-research action. There is deliberately no offset: this
/// is a fresh scope preview, not a selection cart or a persisted plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPlanPreviewRequestV0 {
    limit: i64,
    scope: CurrentCommentVoiceFilterV1,
}

impl CurrentCommentResearchPlanPreviewRequestV0 {
    pub fn new(limit: i64) -> Result<Self, StorageError> {
        Self::with_scope(limit, CurrentCommentVoiceFilterV1::Available)
    }

    pub fn with_scope(
        limit: i64,
        scope: CurrentCommentVoiceFilterV1,
    ) -> Result<Self, StorageError> {
        if !(1..=COMMENT_RESEARCH_PLAN_PREVIEW_V0_MAX_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidCommentResearchPlanPreviewLimit);
        }
        Ok(Self { limit, scope })
    }

    pub const fn limit(self) -> i64 {
        self.limit
    }

    pub const fn scope(self) -> CurrentCommentVoiceFilterV1 {
        self.scope
    }
}

/// A compact research-readiness state for an already cleaned User Voice. It is
/// preparation fact, not a model result or a task state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentCommentVoiceReadinessV1 {
    Ready,
    NeedsContext,
}

impl CurrentCommentVoiceReadinessV1 {
    fn from_storage_value(value: &str) -> Result<Self, StorageError> {
        match value {
            "analyzable" => Ok(Self::Ready),
            "needs_context" => Ok(Self::NeedsContext),
            _ => Err(StorageError::InvalidPersistedCommentDerivation),
        }
    }

    pub const fn as_api_value(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NeedsContext => "needs_context",
        }
    }

    const fn as_context_pack_readiness(self) -> ContextPackReadinessV1 {
        match self {
            Self::Ready => ContextPackReadinessV1::Ready,
            Self::NeedsContext => ContextPackReadinessV1::NeedsContext,
        }
    }
}

/// The immutable source record that supports the current comment text.
///
/// It is deliberately an Evidence relation, not a synthetic note URL, author,
/// engagement count, or inferred work context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentSourceEvidenceRelationV0 {
    pub evidence_id: Uuid,
    pub record_index: i32,
}

/// One current comment fact safely available to a User Voices V0 client.
///
/// `current_admitted_at` is Linggan's local admission time for the Current
/// materialization. It is not a platform publication or observation time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentVoiceV0 {
    pub source_note_id: String,
    /// The deterministic Cleaning Contract V1 expression shown in the table.
    /// Original source text remains in the detail drawer and Evidence only.
    pub research_text: String,
    pub readiness: CurrentCommentVoiceReadinessV1,
    pub current_admitted_at: String,
    pub source_evidence: CurrentCommentSourceEvidenceRelationV0,
}

/// One deterministically ordered page of current comment facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentVoicesPageV0 {
    pub total: i64,
    /// Current comments not yet carrying a V1 derivation. They are excluded
    /// from table rows until the explicit bounded local materializer runs.
    pub awaiting_cleaning_total: i64,
    pub voices: Vec<CurrentCommentVoiceV0>,
}

/// Counts which make a scope preview explainable without claiming that a
/// comment is already researched or that a task exists. All totals are over
/// the current comment projection under `comment-cleaning.v1`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPreparationTotalsV0 {
    pub current_total: i64,
    pub available_total: i64,
    pub ready_total: i64,
    pub needs_context_total: i64,
    pub awaiting_cleaning_total: i64,
    pub excluded_total: i64,
}

/// One source represented by candidates in the current bounded preview.
/// `eligible_total` describes the chosen scope, while `selected_total` only
/// describes this live preview. Neither is a research-quality score.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPreviewSourceV0 {
    pub source_note_id: String,
    pub eligible_total: i64,
    pub selected_total: i64,
}

/// One current, deterministically cleaned expression in a plan preview.
/// Source turn is the explained rotation position within its own source work;
/// it is not a platform ordering, priority score, or task identifier.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPreviewCandidateV0 {
    pub source_note_id: String,
    pub research_text: String,
    pub readiness: CurrentCommentVoiceReadinessV1,
    pub current_admitted_at: String,
    pub source_turn: i64,
    pub source_evidence: CurrentCommentSourceEvidenceRelationV0,
}

/// The complete, live, side-effect-free automatic-research scope preview.
/// It contains no execution object because previewing a scope must never
/// reserve comments, write a plan, or trigger a model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPlanPreviewV0 {
    pub scope: CurrentCommentVoiceFilterV1,
    pub limit: i64,
    pub totals: CurrentCommentResearchPreparationTotalsV0,
    pub sources: Vec<CurrentCommentResearchPreviewSourceV0>,
    pub candidates: Vec<CurrentCommentResearchPreviewCandidateV0>,
}

/// An explicit write request that creates a local immutable input snapshot.
/// It deliberately has no candidate identities: storage always refreshes the
/// current scope itself inside the transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchRunPreparationRequestV1 {
    limit: i64,
    scope: CurrentCommentVoiceFilterV1,
}

impl CurrentCommentResearchRunPreparationRequestV1 {
    pub fn with_scope(
        limit: i64,
        scope: CurrentCommentVoiceFilterV1,
    ) -> Result<Self, StorageError> {
        if !(1..=COMMENT_RESEARCH_RUN_PREPARATION_V1_MAX_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidCommentResearchRunPreparationLimit);
        }
        Ok(Self { limit, scope })
    }

    pub const fn limit(self) -> i64 {
        self.limit
    }

    pub const fn scope(self) -> CurrentCommentVoiceFilterV1 {
        self.scope
    }
}

/// One source's final frozen-item distribution. It describes the actual
/// transaction result, never the browser preview or research quality.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchPreparedSourceV1 {
    pub source_note_id: String,
    pub frozen_total: i64,
    pub prepared_total: i64,
    pub blocked_total: i64,
}

/// Public-preview facts used only to detect that a non-authoritative browser
/// summary has gone stale. They contain neither Evidence locators nor research
/// input hashes and cannot select a record for the write path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchScopeCandidateSummaryV1 {
    pub source_note_id: String,
    pub current_admitted_at: String,
    pub source_turn: i64,
}

/// The local result of a confirmed input-freezing action. A `prepared` item is
/// only ready for a future separately authorized executor; no model call is
/// made by this method.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentResearchRunPreparationOutcomeV1 {
    pub run_id: Uuid,
    pub execution_state: &'static str,
    pub scope: CurrentCommentVoiceFilterV1,
    pub limit: i64,
    pub totals: CurrentCommentResearchPreparationTotalsV0,
    /// The refreshed bounded scope before conclusion and same-run semantic
    /// exclusions. It is used only to determine whether a browser preview is
    /// stale; it is not a persisted authorization list.
    pub scoped_candidate_total: i64,
    pub scoped_source_distribution: Vec<(String, i64)>,
    pub scoped_candidates: Vec<CurrentCommentResearchScopeCandidateSummaryV1>,
    pub frozen_total: i64,
    pub prepared_total: i64,
    pub blocked_total: i64,
    pub concluded_excluded_total: i64,
    pub duplicate_input_excluded_total: i64,
    pub sources: Vec<CurrentCommentResearchPreparedSourceV1>,
}

#[derive(Clone, Debug)]
struct CurrentCommentResearchPreparationCandidateV1 {
    source_note_id: String,
    comment_id: String,
    source_evidence_id: Uuid,
    source_record_index: i32,
    current_admitted_at: String,
    source_turn: i64,
    observation_id: Uuid,
    derivation_id: Uuid,
    research_text: String,
    readiness: CurrentCommentVoiceReadinessV1,
}

#[derive(Clone, Debug)]
struct FrozenCommentResearchRunItemV1 {
    candidate: CurrentCommentResearchPreparationCandidateV1,
    frozen_context_text: String,
    context_integrity_sha256: String,
    research_fingerprint: String,
    context_sufficiency_state: &'static str,
    execution_state: &'static str,
    initial_failure_code: Option<&'static str>,
}

/// The outcome of one bounded local materialization pass. This is deliberately
/// internal/operator-facing and never appears in the User Voices DTO.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentDerivationMaterializationOutcomeV1 {
    pub materialized_current_observations: i64,
}

/// An immutable Evidence record reference for a bounded context field or
/// discussion record. It is deliberately a provenance pointer, not a URL or a
/// user-facing source claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextSourceEvidenceRelationV0 {
    pub evidence_id: Uuid,
    pub record_index: i32,
}

/// One source-backed work context available for a current comment. `title`
/// and `body_text` preserve unavailable vs supplied-blank vs observed text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentWorkContextV0 {
    pub source_evidence: ContextSourceEvidenceRelationV0,
    pub title: ContextTextAvailabilityV0,
    pub body_text: ContextTextAvailabilityV0,
}

/// One reply that is explicitly related to the current comment by an observed
/// root, parent, or reply-to pointer. The three pointer fields stay separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentRelatedReplyV0 {
    pub source_evidence: ContextSourceEvidenceRelationV0,
    pub text: String,
    pub root_comment_id: Option<String>,
    pub parent_comment_id: Option<String>,
    pub reply_to_comment_id: Option<String>,
}

/// A bounded read-only context detail for one current comment. It contains no
/// mutable current-context projection and no synthetic user-visible pack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentContextDetailV0 {
    pub work_context: CurrentCommentWorkContextV0,
    pub related_replies: Vec<CurrentCommentRelatedReplyV0>,
}

/// The result of resolving a browser-supplied Evidence locator against the
/// current comment projection.
///
/// `None` from the containing storage read means the locator no longer names a
/// current comment source for this workspace. `context: None` instead means
/// the locator is current, but no complete, text-matching context capture has
/// been admitted. Keeping those cases separate prevents an old source record
/// from being presented as a current voice, while avoiding a false claim that
/// the platform has no context.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentContextBySourceLocatorV0 {
    /// The immutable source text of the current Observation selected by this
    /// locator. It is not placed in the table, whose text is the cleaned
    /// research expression; callers use it only in the evidence drawer.
    pub original_source_text: String,
    pub context: Option<CurrentCommentContextDetailV0>,
}

/// Context reads return at most this many related reply facts. This bound is a
/// transport safety limit, not a claim that the complete discussion tree was
/// captured.
pub const CURRENT_COMMENT_CONTEXT_V0_MAX_REPLIES: i64 = 100;

impl CommentFactStore {
    /// Creates a store from a caller-managed PostgreSQL client.
    pub fn new(client: Client) -> Self {
        Self {
            client: Mutex::new(client),
        }
    }

    /// Opens PostgreSQL and drives the connection in a background task.
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .map_err(StorageError::Database)?;
        tokio::spawn(async move {
            let _ = connection.await;
        });
        Ok(Self::new(client))
    }

    /// Applies the V0 schema to an empty, isolated database.
    ///
    /// There is intentionally no migration journal or fallback. This is the
    /// clean baseline exercised by the V0 integration proof.
    pub async fn apply_comment_fact_storage_v0_migration(&self) -> Result<(), StorageError> {
        self.client
            .lock()
            .await
            .batch_execute(COMMENT_FACT_STORAGE_V0_MIGRATION)
            .await
            .map_err(StorageError::Database)
    }

    /// Applies Context Storage V0 after the Comment Fact V0 baseline.
    ///
    /// The separate method makes the migration dependency explicit. It never
    /// attempts a fallback, implicit baseline, or legacy schema import.
    pub async fn apply_comment_context_storage_v0_migration(&self) -> Result<(), StorageError> {
        self.client
            .lock()
            .await
            .batch_execute(COMMENT_CONTEXT_STORAGE_V0_MIGRATION)
            .await
            .map_err(StorageError::Database)
    }

    /// Applies the deterministic Comment Derivation V1 extension after the
    /// Comment Fact baseline. Existing current observations can then be
    /// materialized with the bounded explicit method below.
    pub async fn apply_comment_derivation_v1_migration(&self) -> Result<(), StorageError> {
        self.client
            .lock()
            .await
            .batch_execute(COMMENT_DERIVATION_V1_MIGRATION)
            .await
            .map_err(StorageError::Database)
    }

    /// Applies the V1 execution-foundation schema after Comment Fact and
    /// Derivation V1. It only defines immutable storage; it does not create a
    /// run, enqueue work, or contact a model.
    pub async fn apply_comment_research_execution_foundation_v1_migration(
        &self,
    ) -> Result<(), StorageError> {
        self.client
            .lock()
            .await
            .batch_execute(COMMENT_RESEARCH_EXECUTION_FOUNDATION_V1_MIGRATION)
            .await
            .map_err(StorageError::Database)
    }

    /// Applies the forward-only preparation extension after the execution
    /// foundation. It adds no worker, queue, provider, or model configuration.
    pub async fn apply_comment_research_run_preparation_v1_migration(
        &self,
    ) -> Result<(), StorageError> {
        self.client
            .lock()
            .await
            .batch_execute(COMMENT_RESEARCH_RUN_PREPARATION_V1_MIGRATION)
            .await
            .map_err(StorageError::Database)
    }

    /// Reads the current User Voices V0 projection without changing any table.
    ///
    /// The query deliberately joins only `comment_current_v0` and its backing
    /// immutable Observation. Its stable order is local current-admission time,
    /// then the source identity's note and comment IDs. The source comment ID
    /// is used solely as a deterministic tie-breaker and is never returned by
    /// this read model.
    ///
    /// `total` is calculated in the same PostgreSQL statement as the page, so
    /// it describes the same statement snapshot as returned rows. A sentinel
    /// row keeps that total available when an offset falls beyond the final
    /// page; it is not materialized as a voice.
    pub async fn list_current_comment_voices_v0(
        &self,
        workspace_id: &str,
        page: CurrentCommentVoicesPageRequestV0,
    ) -> Result<CurrentCommentVoicesPageV0, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }

        let client = self.client.lock().await;
        let rows = client
            .query(
                "WITH current_observations AS ( \
                   SELECT current_projection.note_id, current_projection.comment_id, \
                          current_observation.id AS observation_id, \
                          current_observation.admitted_at, \
                          current_observation.source_evidence_id, \
                          current_observation.source_record_index, \
                          derivation.research_state, derivation.research_text \
                     FROM comment_current_v0 AS current_projection \
                     JOIN comment_observation_v0 AS current_observation \
                       ON current_observation.workspace_id = current_projection.workspace_id \
                      AND current_observation.platform = current_projection.platform \
                      AND current_observation.note_id = current_projection.note_id \
                      AND current_observation.comment_id = current_projection.comment_id \
                      AND current_observation.id = current_projection.current_observation_id \
                     LEFT JOIN comment_derivation_v1 AS derivation \
                       ON derivation.comment_observation_id = current_observation.id \
                      AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                    WHERE current_projection.workspace_id = $1 \
                 ), filtered AS ( \
                   SELECT * FROM current_observations \
                    WHERE research_state IN ('analyzable', 'needs_context') \
                      AND ( \
                        $2 = 'available' \
                        OR ($2 = 'ready' AND research_state = 'analyzable') \
                        OR ($2 = 'needs_context' AND research_state = 'needs_context') \
                      ) \
                 ), page AS ( \
                   SELECT * FROM filtered \
                    ORDER BY admitted_at ASC, note_id ASC, comment_id ASC \
                    LIMIT $3 OFFSET $4 \
                 ), total AS ( \
                   SELECT count(*)::BIGINT AS total FROM filtered \
                 ), awaiting_cleaning AS ( \
                   SELECT count(*)::BIGINT AS total \
                     FROM current_observations \
                    WHERE observation_id IS NOT NULL AND research_state IS NULL \
                 ) \
                 SELECT page.note_id, page.research_text, page.research_state, \
                        to_char(page.admitted_at AT TIME ZONE 'UTC', \
                          'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS current_admitted_at, \
                        page.source_evidence_id, page.source_record_index, total.total, \
                        awaiting_cleaning.total \
                   FROM total CROSS JOIN awaiting_cleaning \
                   LEFT JOIN page ON TRUE \
                  ORDER BY page.admitted_at ASC NULLS LAST, page.note_id ASC NULLS LAST, \
                           page.comment_id ASC NULLS LAST",
                &[
                    &workspace_id,
                    &page.filter().as_storage_value(),
                    &page.limit(),
                    &page.offset(),
                ],
            )
            .await
            .map_err(StorageError::Database)?;

        let total = rows.first().map_or(0, |row| row.get(6));
        let awaiting_cleaning_total = rows.first().map_or(0, |row| row.get(7));
        let mut voices = Vec::with_capacity(rows.len());
        for row in rows {
            let Some(source_note_id) = row.get::<_, Option<String>>(0) else {
                continue;
            };
            let research_text = row
                .get::<_, Option<String>>(1)
                .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
            let research_state = row
                .get::<_, Option<String>>(2)
                .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
            voices.push(CurrentCommentVoiceV0 {
                source_note_id,
                research_text,
                readiness: CurrentCommentVoiceReadinessV1::from_storage_value(&research_state)?,
                current_admitted_at: row.get(3),
                source_evidence: CurrentCommentSourceEvidenceRelationV0 {
                    evidence_id: row.get(4),
                    record_index: row.get(5),
                },
            });
        }

        Ok(CurrentCommentVoicesPageV0 {
            total,
            awaiting_cleaning_total,
            voices,
        })
    }

    /// Previews a future automatic-research scope without doing any of the
    /// things that start research. It issues one SELECT statement against the
    /// current projection and V1 cleaning derivation only: no materialization,
    /// no plan persistence, no task reservation, and no model or vector work.
    ///
    /// Candidates rotate through source works. The first current expression of
    /// each source is considered before a second expression from any source;
    /// ties are broken only by Linggan-local admission facts and stable source
    /// identities. This does not claim platform chronology or research value.
    pub async fn preview_current_comment_research_plan_v0(
        &self,
        workspace_id: &str,
        request: CurrentCommentResearchPlanPreviewRequestV0,
    ) -> Result<CurrentCommentResearchPlanPreviewV0, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }

        let client = self.client.lock().await;
        let rows = client
            .query(
                "WITH current_observations AS ( \
                   SELECT current_projection.note_id, current_projection.comment_id, \
                          current_observation.admitted_at, \
                          current_observation.source_evidence_id, \
                          current_observation.source_record_index, \
                          derivation.research_state, derivation.research_text \
                     FROM comment_current_v0 AS current_projection \
                     JOIN comment_observation_v0 AS current_observation \
                       ON current_observation.workspace_id = current_projection.workspace_id \
                      AND current_observation.platform = current_projection.platform \
                      AND current_observation.note_id = current_projection.note_id \
                      AND current_observation.comment_id = current_projection.comment_id \
                      AND current_observation.id = current_projection.current_observation_id \
                     LEFT JOIN comment_derivation_v1 AS derivation \
                       ON derivation.comment_observation_id = current_observation.id \
                      AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                    WHERE current_projection.workspace_id = $1 \
                      AND current_projection.platform = 'xhs' \
                 ), totals AS ( \
                   SELECT count(*)::BIGINT AS current_total, \
                          count(*) FILTER (WHERE research_state IN ('analyzable', 'needs_context'))::BIGINT AS available_total, \
                          count(*) FILTER (WHERE research_state = 'analyzable')::BIGINT AS ready_total, \
                          count(*) FILTER (WHERE research_state = 'needs_context')::BIGINT AS needs_context_total, \
                          count(*) FILTER (WHERE research_state IS NULL)::BIGINT AS awaiting_cleaning_total, \
                          count(*) FILTER (WHERE research_state IN ('dropped', 'anomaly'))::BIGINT AS excluded_total \
                     FROM current_observations \
                 ), eligible AS ( \
                   SELECT * FROM current_observations \
                    WHERE research_state IN ('analyzable', 'needs_context') \
                      AND ( \
                        $2 = 'available' \
                        OR ($2 = 'ready' AND research_state = 'analyzable') \
                        OR ($2 = 'needs_context' AND research_state = 'needs_context') \
                      ) \
                 ), ranked AS ( \
                   SELECT eligible.*, \
                          row_number() OVER ( \
                            PARTITION BY note_id \
                            ORDER BY admitted_at ASC, comment_id ASC \
                          )::BIGINT AS source_turn, \
                          min(admitted_at) OVER (PARTITION BY note_id) AS source_first_admitted_at, \
                          count(*) OVER (PARTITION BY note_id)::BIGINT AS source_eligible_total \
                     FROM eligible \
                 ), selected AS ( \
                   SELECT * FROM ranked \
                    ORDER BY source_turn ASC, source_first_admitted_at ASC, note_id ASC, admitted_at ASC, comment_id ASC \
                    LIMIT $3 \
                 ), preview_sources AS ( \
                   SELECT note_id, \
                          max(source_eligible_total)::BIGINT AS eligible_total, \
                          count(*)::BIGINT AS selected_total, \
                          min(source_first_admitted_at) AS source_first_admitted_at \
                     FROM selected \
                    GROUP BY note_id \
                 ), rows AS ( \
                   SELECT 0::INTEGER AS output_group, totals.current_total, totals.available_total, \
                          totals.ready_total, totals.needs_context_total, totals.awaiting_cleaning_total, \
                          totals.excluded_total, \
                          NULL::TEXT AS source_note_id, NULL::TEXT AS research_text, \
                          NULL::TEXT AS research_state, NULL::TEXT AS current_admitted_at, \
                          NULL::UUID AS source_evidence_id, NULL::INTEGER AS source_record_index, \
                          NULL::BIGINT AS source_turn, NULL::BIGINT AS eligible_total, \
                          NULL::BIGINT AS selected_total, NULL::TIMESTAMPTZ AS source_first_admitted_at \
                     FROM totals \
                   UNION ALL \
                   SELECT 1::INTEGER AS output_group, totals.current_total, totals.available_total, \
                          totals.ready_total, totals.needs_context_total, totals.awaiting_cleaning_total, \
                          totals.excluded_total, \
                          selected.note_id, selected.research_text, selected.research_state, \
                          to_char(selected.admitted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'), \
                          selected.source_evidence_id, selected.source_record_index, selected.source_turn, \
                          NULL::BIGINT AS eligible_total, NULL::BIGINT AS selected_total, \
                          selected.source_first_admitted_at \
                     FROM totals CROSS JOIN selected \
                   UNION ALL \
                   SELECT 2::INTEGER AS output_group, totals.current_total, totals.available_total, \
                          totals.ready_total, totals.needs_context_total, totals.awaiting_cleaning_total, \
                          totals.excluded_total, \
                          preview_sources.note_id, NULL::TEXT AS research_text, NULL::TEXT AS research_state, \
                          NULL::TEXT AS current_admitted_at, NULL::UUID AS source_evidence_id, \
                          NULL::INTEGER AS source_record_index, NULL::BIGINT AS source_turn, \
                          preview_sources.eligible_total, preview_sources.selected_total, \
                          preview_sources.source_first_admitted_at \
                     FROM totals CROSS JOIN preview_sources \
                 ) \
                 SELECT output_group, current_total, available_total, ready_total, needs_context_total, \
                        awaiting_cleaning_total, excluded_total, source_note_id, research_text, \
                        research_state, current_admitted_at, source_evidence_id, source_record_index, \
                        source_turn, eligible_total, selected_total \
                   FROM rows \
                  ORDER BY output_group ASC, source_turn ASC NULLS LAST, \
                           source_first_admitted_at ASC NULLS LAST, source_note_id ASC NULLS LAST, \
                           current_admitted_at ASC NULLS LAST",
                &[
                    &workspace_id,
                    &request.scope().as_storage_value(),
                    &request.limit(),
                ],
            )
            .await
            .map_err(StorageError::Database)?;

        let totals_row = rows.first().ok_or(StorageError::InvalidPersistedCurrent)?;
        let totals = CurrentCommentResearchPreparationTotalsV0 {
            current_total: totals_row.get(1),
            available_total: totals_row.get(2),
            ready_total: totals_row.get(3),
            needs_context_total: totals_row.get(4),
            awaiting_cleaning_total: totals_row.get(5),
            excluded_total: totals_row.get(6),
        };
        let mut sources = Vec::new();
        let mut candidates = Vec::new();
        for row in rows {
            match row.get::<_, i32>(0) {
                0 => {}
                1 => {
                    let source_note_id = row
                        .get::<_, Option<String>>(7)
                        .ok_or(StorageError::InvalidPersistedCurrent)?;
                    let research_text = row
                        .get::<_, Option<String>>(8)
                        .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
                    let research_state = row
                        .get::<_, Option<String>>(9)
                        .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
                    candidates.push(CurrentCommentResearchPreviewCandidateV0 {
                        source_note_id,
                        research_text,
                        readiness: CurrentCommentVoiceReadinessV1::from_storage_value(
                            &research_state,
                        )?,
                        current_admitted_at: row
                            .get::<_, Option<String>>(10)
                            .ok_or(StorageError::InvalidPersistedCurrent)?,
                        source_evidence: CurrentCommentSourceEvidenceRelationV0 {
                            evidence_id: row
                                .get::<_, Option<Uuid>>(11)
                                .ok_or(StorageError::InvalidPersistedCurrent)?,
                            record_index: row
                                .get::<_, Option<i32>>(12)
                                .ok_or(StorageError::InvalidPersistedCurrent)?,
                        },
                        source_turn: row
                            .get::<_, Option<i64>>(13)
                            .ok_or(StorageError::InvalidPersistedCurrent)?,
                    });
                }
                2 => sources.push(CurrentCommentResearchPreviewSourceV0 {
                    source_note_id: row
                        .get::<_, Option<String>>(7)
                        .ok_or(StorageError::InvalidPersistedCurrent)?,
                    eligible_total: row
                        .get::<_, Option<i64>>(14)
                        .ok_or(StorageError::InvalidPersistedCurrent)?,
                    selected_total: row
                        .get::<_, Option<i64>>(15)
                        .ok_or(StorageError::InvalidPersistedCurrent)?,
                }),
                _ => return Err(StorageError::InvalidPersistedCurrent),
            }
        }

        Ok(CurrentCommentResearchPlanPreviewV0 {
            scope: request.scope(),
            limit: request.limit(),
            totals,
            sources,
            candidates,
        })
    }

    /// Recomputes the current automatic scope and freezes its selected V1
    /// inputs in one PostgreSQL transaction. The caller cannot select evidence
    /// records or carry a browser candidate list into this method.
    ///
    /// `needs_context` inputs are intentionally retained as blocked RunItems:
    /// their frozen source-backed Context Pack makes the block auditable, while
    /// their execution state and initial event make them ineligible for a
    /// future executor. A `prepared` event means exactly input preparation; it
    /// is not a queue reservation or a model invocation.
    pub async fn prepare_current_comment_research_run_v1(
        &self,
        workspace_id: &str,
        request: CurrentCommentResearchRunPreparationRequestV1,
    ) -> Result<CurrentCommentResearchRunPreparationOutcomeV1, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }

        let mut client = self.client.lock().await;
        let transaction = client.transaction().await.map_err(StorageError::Database)?;

        let totals_row = transaction
            .query_one(
                "WITH current_observations AS ( \
                   SELECT derivation.research_state \
                     FROM comment_current_v0 AS current_projection \
                     JOIN comment_observation_v0 AS current_observation \
                       ON current_observation.workspace_id = current_projection.workspace_id \
                      AND current_observation.platform = current_projection.platform \
                      AND current_observation.note_id = current_projection.note_id \
                      AND current_observation.comment_id = current_projection.comment_id \
                      AND current_observation.id = current_projection.current_observation_id \
                     LEFT JOIN comment_derivation_v1 AS derivation \
                       ON derivation.comment_observation_id = current_observation.id \
                      AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                    WHERE current_projection.workspace_id = $1 \
                      AND current_projection.platform = 'xhs' \
                 ) \
                 SELECT count(*)::BIGINT AS current_total, \
                        count(*) FILTER (WHERE research_state IN ('analyzable', 'needs_context'))::BIGINT AS available_total, \
                        count(*) FILTER (WHERE research_state = 'analyzable')::BIGINT AS ready_total, \
                        count(*) FILTER (WHERE research_state = 'needs_context')::BIGINT AS needs_context_total, \
                        count(*) FILTER (WHERE research_state IS NULL)::BIGINT AS awaiting_cleaning_total, \
                        count(*) FILTER (WHERE research_state IN ('dropped', 'anomaly'))::BIGINT AS excluded_total \
                   FROM current_observations",
                &[&workspace_id],
            )
            .await
            .map_err(StorageError::Database)?;
        let totals = CurrentCommentResearchPreparationTotalsV0 {
            current_total: totals_row.get(0),
            available_total: totals_row.get(1),
            ready_total: totals_row.get(2),
            needs_context_total: totals_row.get(3),
            awaiting_cleaning_total: totals_row.get(4),
            excluded_total: totals_row.get(5),
        };

        // This is deliberately a fresh database query, not a browser preview.
        // Source turns are recomputed before the bounded selection so every
        // source has its first current voice considered before a second one.
        let candidate_rows = transaction
            .query(
                "WITH current_observations AS ( \
                   SELECT current_projection.note_id, current_projection.comment_id, \
                          current_observation.id AS observation_id, \
                          current_observation.source_evidence_id, \
                          current_observation.source_record_index, \
                          current_observation.admitted_at, derivation.id AS derivation_id, \
                          derivation.research_state, derivation.research_text \
                     FROM comment_current_v0 AS current_projection \
                     JOIN comment_observation_v0 AS current_observation \
                       ON current_observation.workspace_id = current_projection.workspace_id \
                      AND current_observation.platform = current_projection.platform \
                      AND current_observation.note_id = current_projection.note_id \
                      AND current_observation.comment_id = current_projection.comment_id \
                      AND current_observation.id = current_projection.current_observation_id \
                     JOIN comment_derivation_v1 AS derivation \
                       ON derivation.comment_observation_id = current_observation.id \
                      AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                    WHERE current_projection.workspace_id = $1 \
                      AND current_projection.platform = 'xhs' \
                 ), eligible AS ( \
                   SELECT * FROM current_observations \
                    WHERE research_state IN ('analyzable', 'needs_context') \
                      AND ( \
                        $2 = 'available' \
                        OR ($2 = 'ready' AND research_state = 'analyzable') \
                        OR ($2 = 'needs_context' AND research_state = 'needs_context') \
                      ) \
                 ), ranked AS ( \
                   SELECT eligible.*, \
                          row_number() OVER ( \
                            PARTITION BY note_id \
                            ORDER BY admitted_at ASC, comment_id ASC \
                          )::BIGINT AS source_turn, \
                          min(admitted_at) OVER (PARTITION BY note_id) AS source_first_admitted_at \
                     FROM eligible \
                 ) \
                 SELECT note_id, comment_id, source_evidence_id, source_record_index, \
                        observation_id, derivation_id, research_text, research_state, \
                        to_char(admitted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS current_admitted_at, \
                        source_turn \
                   FROM ranked \
                  ORDER BY source_turn ASC, source_first_admitted_at ASC, note_id ASC, \
                           admitted_at ASC, comment_id ASC \
                  LIMIT $3",
                &[
                    &workspace_id,
                    &request.scope().as_storage_value(),
                    &request.limit(),
                ],
            )
            .await
            .map_err(StorageError::Database)?;

        let mut candidates = Vec::with_capacity(candidate_rows.len());
        for row in candidate_rows {
            let research_state: String = row.get(7);
            candidates.push(CurrentCommentResearchPreparationCandidateV1 {
                source_note_id: row.get(0),
                comment_id: row.get(1),
                source_evidence_id: row.get(2),
                source_record_index: row.get(3),
                current_admitted_at: row.get(8),
                source_turn: row.get(9),
                observation_id: row.get(4),
                derivation_id: row.get(5),
                research_text: row
                    .get::<_, Option<String>>(6)
                    .ok_or(StorageError::InvalidPersistedCommentDerivation)?,
                readiness: CurrentCommentVoiceReadinessV1::from_storage_value(&research_state)?,
            });
        }

        if candidates.is_empty() {
            return Err(StorageError::NoCurrentCommentResearchCandidates);
        }

        let scoped_candidate_total = candidates.len() as i64;
        let mut scoped_source_counts = BTreeMap::new();
        for candidate in &candidates {
            *scoped_source_counts
                .entry(candidate.source_note_id.clone())
                .or_insert(0_i64) += 1;
        }
        let scoped_candidates = candidates
            .iter()
            .map(|candidate| CurrentCommentResearchScopeCandidateSummaryV1 {
                source_note_id: candidate.source_note_id.clone(),
                current_admitted_at: candidate.current_admitted_at.clone(),
                source_turn: candidate.source_turn,
            })
            .collect();

        let mut frozen_items = Vec::with_capacity(candidates.len());
        let mut fingerprints = HashSet::new();
        let mut concluded_excluded_total = 0;
        let mut duplicate_input_excluded_total = 0;
        for candidate in candidates {
            let pack = build_current_comment_research_context_pack_in_transaction_v1(
                &transaction,
                workspace_id,
                &candidate,
            )
            .await?;
            let frozen_pack = freeze_comment_research_context_pack_v1(&pack);
            let research_fingerprint = research_fingerprint_v1(&ResearchFingerprintInputV1 {
                cleaned_research_text: candidate.research_text.clone(),
                frozen_context_pack: frozen_pack.clone(),
                cleaning_contract: "comment-cleaning.v1".to_owned(),
                research_contract: COMMENT_RESEARCH_EXECUTION_CONTRACT_V1.to_owned(),
                output_schema: COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1.to_owned(),
                // This is deliberately an execution-neutral strategy. An
                // actual provider/model strategy must create a new semantic
                // input rather than silently reuse this no-executor snapshot.
                model_strategy: ResearchModelStrategyV1 {
                    strategy_id: "comment-research-preparation".to_owned(),
                    strategy_version: "v1".to_owned(),
                },
            })
            .map_err(|_| StorageError::InvalidPersistedCommentDerivation)?;

            let already_concluded = transaction
                .query_one(
                    "SELECT EXISTS( \
                       SELECT 1 FROM comment_analysis_v1 \
                        WHERE comment_derivation_id = $1 \
                          AND research_fingerprint = $2 \
                          AND conclusion_state IN ('success', 'no_signal') \
                     )",
                    &[&candidate.derivation_id, &research_fingerprint],
                )
                .await
                .map_err(StorageError::Database)?
                .get::<_, bool>(0);
            if already_concluded {
                concluded_excluded_total += 1;
                continue;
            }
            if !fingerprints.insert(research_fingerprint.clone()) {
                // Only one identical semantic input is frozen in a newly
                // prepared run. This is never inferred from old incomplete
                // RunItems, so an interrupted/cancelled item remains eligible
                // on a later explicit confirmation.
                duplicate_input_excluded_total += 1;
                continue;
            }

            frozen_items.push(FrozenCommentResearchRunItemV1 {
                context_sufficiency_state: frozen_pack.context_sufficiency.as_storage_value(),
                execution_state: frozen_pack.context_sufficiency.initial_execution_state(),
                initial_failure_code: frozen_pack.context_sufficiency.initial_failure_code(),
                frozen_context_text: frozen_pack.text,
                context_integrity_sha256: frozen_pack.integrity_sha256,
                research_fingerprint,
                candidate,
            });
        }

        if frozen_items.is_empty() {
            return Err(StorageError::NoCurrentCommentResearchCandidates);
        }

        let prepared_total = frozen_items
            .iter()
            .filter(|item| item.execution_state == "prepared")
            .count() as i64;
        let blocked_total = frozen_items.len() as i64 - prepared_total;
        let run_execution_state = if prepared_total > 0 {
            "prepared"
        } else {
            "blocked"
        };
        let run_id = Uuid::new_v4();
        transaction
            .execute(
                "INSERT INTO comment_research_run_v1 (id, workspace_id, execution_state) \
                 VALUES ($1, $2, $3)",
                &[&run_id, &workspace_id, &run_execution_state],
            )
            .await
            .map_err(StorageError::Database)?;

        let mut source_counts: BTreeMap<String, (i64, i64, i64)> = BTreeMap::new();
        for item in &frozen_items {
            let run_item_id = Uuid::new_v4();
            transaction
                .execute(
                    "INSERT INTO comment_research_run_item_v1 \
                     (id, run_id, workspace_id, source_evidence_id, source_record_index, \
                      comment_observation_id, comment_derivation_id, cleaned_research_text, \
                      cleaning_contract, research_contract, output_schema, model_strategy_id, \
                      model_strategy_version, research_fingerprint, context_pack_version, \
                      context_pack_integrity_sha256, frozen_context_pack_text, \
                      context_sufficiency_state, execution_state, initial_failure_code) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'comment-cleaning.v1', \
                      'comment-research-execution.v1', 'comment-analysis-structured-output.v1', \
                      'comment-research-preparation', 'v1', $9, \
                      'comment-research-input-snapshot.v1', $10, $11, $12, $13, $14)",
                    &[
                        &run_item_id,
                        &run_id,
                        &workspace_id,
                        &item.candidate.source_evidence_id,
                        &item.candidate.source_record_index,
                        &item.candidate.observation_id,
                        &item.candidate.derivation_id,
                        &item.candidate.research_text,
                        &item.research_fingerprint,
                        &item.context_integrity_sha256,
                        &item.frozen_context_text,
                        &item.context_sufficiency_state,
                        &item.execution_state,
                        &item.initial_failure_code,
                    ],
                )
                .await
                .map_err(StorageError::Database)?;
            transaction
                .execute(
                    "INSERT INTO comment_research_run_item_event_v1 \
                     (id, run_item_id, execution_state, failure_code, output_validation_state) \
                     VALUES ($1, $2, $3, $4, 'not_submitted')",
                    &[
                        &Uuid::new_v4(),
                        &run_item_id,
                        &item.execution_state,
                        &item.initial_failure_code,
                    ],
                )
                .await
                .map_err(StorageError::Database)?;

            let entry = source_counts
                .entry(item.candidate.source_note_id.clone())
                .or_insert((0, 0, 0));
            entry.0 += 1;
            if item.execution_state == "prepared" {
                entry.1 += 1;
            } else {
                entry.2 += 1;
            }
        }

        transaction.commit().await.map_err(StorageError::Database)?;
        Ok(CurrentCommentResearchRunPreparationOutcomeV1 {
            run_id,
            execution_state: run_execution_state,
            scope: request.scope(),
            limit: request.limit(),
            totals,
            scoped_candidate_total,
            scoped_source_distribution: scoped_source_counts.into_iter().collect(),
            scoped_candidates,
            frozen_total: frozen_items.len() as i64,
            prepared_total,
            blocked_total,
            concluded_excluded_total,
            duplicate_input_excluded_total,
            sources: source_counts
                .into_iter()
                .map(
                    |(source_note_id, (frozen_total, prepared_total, blocked_total))| {
                        CurrentCommentResearchPreparedSourceV1 {
                            source_note_id,
                            frozen_total,
                            prepared_total,
                            blocked_total,
                        }
                    },
                )
                .collect(),
        })
    }

    /// Returns source-backed work and related reply context for one current
    /// comment, or `None` when no complete, text-matching context capture has
    /// been admitted. This method performs no writes.
    ///
    /// The SHA-256 equality is intentional: an old context capture may not be
    /// attached merely because a source comment ID stayed the same while its
    /// observed text changed.
    pub async fn get_current_comment_context_detail_v0(
        &self,
        workspace_id: &str,
        note_id: &str,
        comment_id: &str,
    ) -> Result<Option<CurrentCommentContextDetailV0>, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }
        if note_id.trim().is_empty() || comment_id.trim().is_empty() {
            return Err(StorageError::BlankCommentContextIdentity);
        }

        let client = self.client.lock().await;
        let selected = client
            .query_opt(
                "SELECT context_set.id, context_set.replies_evidence_id, \
                        work.source_evidence_id, work.source_record_index, \
                        work.title_availability, work.title_source_text, \
                        work.body_text_availability, work.body_text_source_text \
                   FROM comment_current_v0 AS current_projection \
                   JOIN context_discussion_record_v0 AS matching_comment \
                     ON matching_comment.workspace_id = current_projection.workspace_id \
                    AND matching_comment.platform = current_projection.platform \
                    AND matching_comment.note_id = current_projection.note_id \
                    AND matching_comment.comment_id = current_projection.comment_id \
                    AND matching_comment.record_kind = 'comment' \
                    AND matching_comment.text_sha256 = current_projection.text_sha256 \
                   JOIN source_context_capture_set_v0 AS context_set \
                     ON context_set.workspace_id = matching_comment.workspace_id \
                    AND context_set.platform = matching_comment.platform \
                    AND context_set.note_id = matching_comment.note_id \
                    AND context_set.comments_evidence_id = matching_comment.source_evidence_id \
                   JOIN context_work_record_v0 AS work \
                     ON work.source_evidence_id = context_set.detail_evidence_id \
                    AND work.source_record_index = 0 \
                  WHERE current_projection.workspace_id = $1 \
                    AND current_projection.platform = 'xhs' \
                    AND current_projection.note_id = $2 \
                    AND current_projection.comment_id = $3 \
                  ORDER BY context_set.admitted_at DESC, context_set.id ASC \
                  LIMIT 1",
                &[&workspace_id, &note_id, &comment_id],
            )
            .await
            .map_err(StorageError::Database)?;

        let Some(selected) = selected else {
            return Ok(None);
        };

        let replies_evidence_id: Uuid = selected.get(1);
        let work_context = CurrentCommentWorkContextV0 {
            source_evidence: ContextSourceEvidenceRelationV0 {
                evidence_id: selected.get(2),
                record_index: selected.get(3),
            },
            title: read_context_text_availability(
                selected.get(4),
                selected.get::<_, Option<String>>(5),
            )?,
            body_text: read_context_text_availability(
                selected.get(6),
                selected.get::<_, Option<String>>(7),
            )?,
        };

        let rows = client
            .query(
                "SELECT source_evidence_id, source_record_index, source_text, \
                        root_comment_id, parent_comment_id, reply_to_comment_id \
                   FROM context_discussion_record_v0 \
                  WHERE source_evidence_id = $1 \
                    AND record_kind = 'reply' \
                    AND workspace_id = $2 \
                    AND platform = 'xhs' \
                    AND note_id = $3 \
                    AND (root_comment_id = $4 OR parent_comment_id = $4 OR reply_to_comment_id = $4) \
                  ORDER BY source_record_index ASC \
                  LIMIT $5",
                &[
                    &replies_evidence_id,
                    &workspace_id,
                    &note_id,
                    &comment_id,
                    &CURRENT_COMMENT_CONTEXT_V0_MAX_REPLIES,
                ],
            )
            .await
            .map_err(StorageError::Database)?;

        let related_replies = rows
            .into_iter()
            .map(|row| CurrentCommentRelatedReplyV0 {
                source_evidence: ContextSourceEvidenceRelationV0 {
                    evidence_id: row.get(0),
                    record_index: row.get(1),
                },
                text: row.get(2),
                root_comment_id: row.get(3),
                parent_comment_id: row.get(4),
                reply_to_comment_id: row.get(5),
            })
            .collect();

        Ok(Some(CurrentCommentContextDetailV0 {
            work_context,
            related_replies,
        }))
    }

    /// Resolves a list-visible source Evidence locator only if it still backs
    /// the current comment projection for this workspace. The browser never
    /// supplies a comment ID: storage resolves it internally, then delegates to
    /// the bounded context read above. This method performs no writes.
    pub async fn get_current_comment_context_by_source_locator_v0(
        &self,
        workspace_id: &str,
        source_evidence_id: Uuid,
        source_record_index: i32,
    ) -> Result<Option<CurrentCommentContextBySourceLocatorV0>, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }
        if source_record_index < 0 {
            return Err(StorageError::InvalidCurrentCommentSourceRecordIndex);
        }

        let current_identity = {
            let client = self.client.lock().await;
            client
                .query_opt(
                    "SELECT current_projection.note_id, current_projection.comment_id, \
                        current_observation.source_text \
                   FROM comment_current_v0 AS current_projection \
                   JOIN comment_observation_v0 AS current_observation \
                     ON current_observation.workspace_id = current_projection.workspace_id \
                    AND current_observation.platform = current_projection.platform \
                    AND current_observation.note_id = current_projection.note_id \
                    AND current_observation.comment_id = current_projection.comment_id \
                    AND current_observation.id = current_projection.current_observation_id \
                  WHERE current_projection.workspace_id = $1 \
                    AND current_projection.platform = 'xhs' \
                    AND current_observation.source_evidence_id = $2 \
                    AND current_observation.source_record_index = $3 \
                  LIMIT 1",
                    &[&workspace_id, &source_evidence_id, &source_record_index],
                )
                .await
                .map_err(StorageError::Database)?
        };

        let Some(current_identity) = current_identity else {
            return Ok(None);
        };
        let note_id: String = current_identity.get(0);
        let comment_id: String = current_identity.get(1);
        let original_source_text: String = current_identity.get(2);
        let context = self
            .get_current_comment_context_detail_v0(workspace_id, &note_id, &comment_id)
            .await?;

        Ok(Some(CurrentCommentContextBySourceLocatorV0 {
            original_source_text,
            context,
        }))
    }

    /// Resolves a list-visible Evidence locator into a pure, bounded Context
    /// Pack V1 preview. This is only text assembly for inspection: it does not
    /// create an input snapshot, reserve a comment, call a model, or make a
    /// context-sufficiency decision for future execution.
    ///
    /// The locator must still back a current Observation with a current
    /// `comment-cleaning.v1` analyzable/needs-context derivation. Thus an old
    /// Evidence record cannot be used to view an input pack for a body that has
    /// since advanced, and a dropped/anomalous comment never becomes a pack.
    pub async fn get_current_comment_research_context_pack_by_source_locator_v1(
        &self,
        workspace_id: &str,
        source_evidence_id: Uuid,
        source_record_index: i32,
    ) -> Result<Option<CommentResearchContextPackV1>, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }
        if source_record_index < 0 {
            return Err(StorageError::InvalidCurrentCommentSourceRecordIndex);
        }

        let current_identity = {
            let client = self.client.lock().await;
            client
                .query_opt(
                    "SELECT current_projection.note_id, current_projection.comment_id, \
                        derivation.research_text, derivation.research_state \
                   FROM comment_current_v0 AS current_projection \
                   JOIN comment_observation_v0 AS current_observation \
                     ON current_observation.workspace_id = current_projection.workspace_id \
                    AND current_observation.platform = current_projection.platform \
                    AND current_observation.note_id = current_projection.note_id \
                    AND current_observation.comment_id = current_projection.comment_id \
                    AND current_observation.id = current_projection.current_observation_id \
                   JOIN comment_derivation_v1 AS derivation \
                     ON derivation.comment_observation_id = current_observation.id \
                    AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                    AND derivation.research_state IN ('analyzable', 'needs_context') \
                  WHERE current_projection.workspace_id = $1 \
                    AND current_projection.platform = 'xhs' \
                    AND current_observation.source_evidence_id = $2 \
                    AND current_observation.source_record_index = $3 \
                  LIMIT 1",
                    &[&workspace_id, &source_evidence_id, &source_record_index],
                )
                .await
                .map_err(StorageError::Database)?
        };

        let Some(current_identity) = current_identity else {
            return Ok(None);
        };
        let note_id: String = current_identity.get(0);
        let comment_id: String = current_identity.get(1);
        let research_expression = current_identity
            .get::<_, Option<String>>(2)
            .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
        let research_state = current_identity
            .get::<_, Option<String>>(3)
            .ok_or(StorageError::InvalidPersistedCommentDerivation)?;
        let readiness = CurrentCommentVoiceReadinessV1::from_storage_value(&research_state)?;
        let context = self
            .get_current_comment_context_detail_v0(workspace_id, &note_id, &comment_id)
            .await?;

        let pack = match context {
            Some(context) => {
                let CurrentCommentContextDetailV0 {
                    work_context,
                    related_replies,
                } = context;
                build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
                    research_expression,
                    readiness: readiness.as_context_pack_readiness(),
                    has_source_backed_context: true,
                    related_discussion: related_replies
                        .into_iter()
                        .map(|reply| ContextPackRelatedDiscussionInputV1 {
                            text: reply.text,
                            root_comment: reply.root_comment_id.is_some(),
                            parent_comment: reply.parent_comment_id.is_some(),
                            reply_to_comment: reply.reply_to_comment_id.is_some(),
                        })
                        .collect(),
                    work_title: context_pack_source_text_v1(work_context.title),
                    work_body: context_pack_source_text_v1(work_context.body_text),
                })
            }
            None => build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
                research_expression,
                readiness: readiness.as_context_pack_readiness(),
                has_source_backed_context: false,
                related_discussion: Vec::new(),
                work_title: ContextPackSourceTextV1::Unavailable,
                work_body: ContextPackSourceTextV1::Unavailable,
            }),
        };

        Ok(Some(pack))
    }

    /// Materializes at most `limit` legacy current observations that predate
    /// Comment Derivation V1. This is intentionally explicit and bounded: a
    /// read request never mutates corpus state. Re-running the method after a
    /// successful pass returns zero because the immutable observation anchor is
    /// unique.
    pub async fn materialize_missing_current_comment_derivations_v1(
        &mut self,
        workspace_id: &str,
        limit: i64,
    ) -> Result<CurrentCommentDerivationMaterializationOutcomeV1, StorageError> {
        if workspace_id.trim().is_empty() {
            return Err(StorageError::BlankWorkspaceId);
        }
        if !(1..=COMMENT_DERIVATION_V1_MATERIALIZATION_MAX_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidCommentDerivationMaterializationLimit);
        }

        let mut client = self.client.lock().await;
        let transaction = client.transaction().await.map_err(StorageError::Database)?;
        let rows = transaction
            .query(
                "SELECT current_observation.id, current_observation.source_text \
                   FROM comment_current_v0 AS current_projection \
                   JOIN comment_observation_v0 AS current_observation \
                     ON current_observation.workspace_id = current_projection.workspace_id \
                    AND current_observation.platform = current_projection.platform \
                    AND current_observation.note_id = current_projection.note_id \
                    AND current_observation.comment_id = current_projection.comment_id \
                    AND current_observation.id = current_projection.current_observation_id \
                   LEFT JOIN comment_derivation_v1 AS derivation \
                     ON derivation.comment_observation_id = current_observation.id \
                    AND derivation.cleaning_contract = 'comment-cleaning.v1' \
                  WHERE current_projection.workspace_id = $1 \
                    AND derivation.id IS NULL \
                  ORDER BY current_observation.admission_sequence ASC \
                  LIMIT $2 \
                  FOR UPDATE OF current_projection SKIP LOCKED",
                &[&workspace_id, &limit],
            )
            .await
            .map_err(StorageError::Database)?;

        let mut materialized = 0;
        for row in rows {
            let observation_id: Uuid = row.get(0);
            let source_text: String = row.get(1);
            materialize_comment_derivation_v1(&transaction, observation_id, &source_text).await?;
            materialized += 1;
        }
        transaction.commit().await.map_err(StorageError::Database)?;
        Ok(CurrentCommentDerivationMaterializationOutcomeV1 {
            materialized_current_observations: materialized,
        })
    }

    /// Validates then admits two producer packages atomically.
    ///
    /// A rejected source pair creates no transaction and no database write.
    pub async fn admit_xhs_comment_capture_pair_v0(
        &mut self,
        workspace_id: impl AsRef<str>,
        detail_package: CapturePackageV0,
        comments_package: CapturePackageV0,
    ) -> Result<CommentFactAdmissionOutcomeV0, StorageError> {
        let prepared = prepare_xhs_comment_evidence_admission_v0(
            workspace_id,
            detail_package,
            comments_package,
        )
        .map_err(StorageError::Preparation)?;
        self.admit_prepared_xhs_comment_evidence_v0(prepared).await
    }

    /// Validates and admits one independently captured content-detail,
    /// comments, and replies context set in a single transaction.
    ///
    /// Its comments also traverse the established immutable Comment admission
    /// path. Context source facts are stored separately so that context is
    /// never confused with the selected comment's own direct evidence.
    pub async fn admit_xhs_comment_context_capture_set_v0(
        &mut self,
        workspace_id: impl AsRef<str>,
        detail_package: CapturePackageV0,
        comments_package: CapturePackageV0,
        replies_package: CapturePackageV0,
    ) -> Result<CommentContextAdmissionOutcomeV0, StorageError> {
        let prepared = prepare_xhs_comment_context_evidence_admission_v0(
            workspace_id,
            detail_package,
            comments_package,
            replies_package,
        )
        .map_err(StorageError::Preparation)?;
        self.admit_prepared_xhs_comment_context_evidence_v0(prepared)
            .await
    }

    async fn admit_prepared_xhs_comment_evidence_v0(
        &mut self,
        prepared: PreparedCommentEvidenceAdmissionV0,
    ) -> Result<CommentFactAdmissionOutcomeV0, StorageError> {
        let mut client = self.client.lock().await;
        let transaction = client.transaction().await.map_err(StorageError::Database)?;

        let detail_evidence = admit_source_evidence(
            &transaction,
            &prepared.workspace_id,
            prepared.source_contract,
            &prepared.detail_evidence,
        )
        .await?;
        let comments_evidence = admit_source_evidence(
            &transaction,
            &prepared.workspace_id,
            prepared.source_contract,
            &prepared.comments_evidence,
        )
        .await?;
        let source_capture_pair = admit_source_capture_pair(
            &transaction,
            &prepared.workspace_id,
            detail_evidence.id,
            comments_evidence.id,
        )
        .await?;

        // Stable order prevents overlapping source packages from deadlocking
        // while they update multiple comment-current rows.
        let mut comments = prepared.comments;
        comments.sort_by(|left, right| left.identity.cmp(&right.identity));

        let mut comment_outcomes = Vec::with_capacity(comments.len());
        for comment in &comments {
            comment_outcomes
                .push(admit_comment_record(&transaction, comments_evidence.id, comment).await?);
        }

        transaction.commit().await.map_err(StorageError::Database)?;

        Ok(CommentFactAdmissionOutcomeV0 {
            detail_evidence,
            comments_evidence,
            source_capture_pair,
            comments: comment_outcomes,
        })
    }

    async fn admit_prepared_xhs_comment_context_evidence_v0(
        &mut self,
        prepared: PreparedCommentContextEvidenceAdmissionV0,
    ) -> Result<CommentContextAdmissionOutcomeV0, StorageError> {
        let mut client = self.client.lock().await;
        let transaction = client.transaction().await.map_err(StorageError::Database)?;

        let detail_evidence = admit_source_evidence(
            &transaction,
            &prepared.workspace_id,
            prepared.source_contract,
            &prepared.detail_evidence,
        )
        .await?;
        let comments_evidence = admit_source_evidence(
            &transaction,
            &prepared.workspace_id,
            prepared.source_contract,
            &prepared.comments_evidence,
        )
        .await?;
        let replies_evidence = admit_source_evidence(
            &transaction,
            &prepared.workspace_id,
            prepared.source_contract,
            &prepared.replies_evidence,
        )
        .await?;

        // Preserve the existing direct-comment current semantics. Stable sort
        // avoids deadlocks where overlapping package admissions advance more
        // than one identity concurrently.
        let mut comments = prepared.comments;
        comments.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut comment_outcomes = Vec::with_capacity(comments.len());
        for comment in &comments {
            comment_outcomes
                .push(admit_comment_record(&transaction, comments_evidence.id, comment).await?);
        }

        admit_context_work_record(
            &transaction,
            detail_evidence.id,
            &prepared.workspace_id,
            &prepared.work_context,
        )
        .await?;

        let mut discussion_records = prepared.discussion_records;
        discussion_records.sort_by(|left, right| {
            context_discussion_sort_key(left).cmp(&context_discussion_sort_key(right))
        });
        for record in &discussion_records {
            let source_evidence_id = match record.kind {
                PreparedContextDiscussionRecordKindV0::Comment => comments_evidence.id,
                PreparedContextDiscussionRecordKindV0::Reply => replies_evidence.id,
            };
            admit_context_discussion_record(&transaction, source_evidence_id, record).await?;
        }

        let comments_record_count = i32::try_from(
            discussion_records
                .iter()
                .filter(|record| record.kind == PreparedContextDiscussionRecordKindV0::Comment)
                .count(),
        )
        .map_err(|_| StorageError::SourceRecordIndexOutOfRange)?;
        let replies_record_count = i32::try_from(
            discussion_records
                .iter()
                .filter(|record| record.kind == PreparedContextDiscussionRecordKindV0::Reply)
                .count(),
        )
        .map_err(|_| StorageError::SourceRecordIndexOutOfRange)?;
        let source_context_capture_set = admit_source_context_capture_set(
            &transaction,
            &prepared.workspace_id,
            &prepared.work_context.note_id,
            detail_evidence.id,
            comments_evidence.id,
            replies_evidence.id,
            (comments_record_count, replies_record_count),
        )
        .await?;

        transaction.commit().await.map_err(StorageError::Database)?;

        Ok(CommentContextAdmissionOutcomeV0 {
            detail_evidence,
            comments_evidence,
            replies_evidence,
            source_context_capture_set,
            comments: comment_outcomes,
        })
    }
}

/// Immutable source-evidence admission result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceAdmissionOutcomeV0 {
    pub id: Uuid,
    /// False means the exact immutable payload already existed; no stored
    /// Evidence payload was overwritten.
    pub inserted: bool,
}

/// An immutable relation between the two independently captured packages used
/// as one validated comment-admission input. IDs intentionally remain internal;
/// callers only need to know whether the relation was new or replayed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceCapturePairAdmissionOutcomeV0 {
    pub inserted: bool,
}

/// A source record's effect on the current comment projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CommentProjectionDispositionV0 {
    Created {
        observation_id: Uuid,
    },
    ReplayUnchanged,
    Advanced {
        previous_observation_id: Uuid,
        observation_id: Uuid,
    },
    /// An already accepted source record never moves current backward.
    PreviouslyAdmittedSource {
        observation_id: Uuid,
    },
}

/// One stable comment identity and its projection result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentAdmissionOutcomeV0 {
    pub identity: CommentSourceIdentityV0,
    pub disposition: CommentProjectionDispositionV0,
}

/// The all-or-nothing result of one pair admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentFactAdmissionOutcomeV0 {
    pub detail_evidence: EvidenceAdmissionOutcomeV0,
    pub comments_evidence: EvidenceAdmissionOutcomeV0,
    pub source_capture_pair: SourceCapturePairAdmissionOutcomeV0,
    pub comments: Vec<CommentAdmissionOutcomeV0>,
}

/// The immutable relation that proves which three independently captured
/// Evidence packages supplied one bounded context set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceContextCaptureSetAdmissionOutcomeV0 {
    pub inserted: bool,
}

/// The all-or-nothing result of Context Storage V0 admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentContextAdmissionOutcomeV0 {
    pub detail_evidence: EvidenceAdmissionOutcomeV0,
    pub comments_evidence: EvidenceAdmissionOutcomeV0,
    pub replies_evidence: EvidenceAdmissionOutcomeV0,
    pub source_context_capture_set: SourceContextCaptureSetAdmissionOutcomeV0,
    pub comments: Vec<CommentAdmissionOutcomeV0>,
}

/// Storage failures never include captured comment content or identifiers.
#[derive(Debug)]
pub enum StorageError {
    BlankWorkspaceId,
    BlankCommentContextIdentity,
    InvalidCurrentCommentVoicesLimit,
    InvalidCurrentCommentVoicesOffset,
    InvalidCommentResearchPlanPreviewLimit,
    InvalidCommentResearchRunPreparationLimit,
    InvalidCurrentCommentSourceRecordIndex,
    InvalidCommentDerivationMaterializationLimit,
    Preparation(EvidencePreparationError),
    Database(tokio_postgres::Error),
    SourceRecordIndexOutOfRange,
    MissingPersistedEvidence,
    MissingPersistedSourceCapturePair,
    MissingPersistedSourceRecord,
    InvalidPersistedSourceRecord,
    MissingLockedIdentity,
    InvalidPersistedCurrent,
    MissingPersistedSourceContextCaptureSet,
    MissingPersistedContextRecord,
    InvalidPersistedContextRecord,
    InvalidPersistedContextTextAvailability,
    MissingPersistedCommentDerivation,
    InvalidPersistedCommentDerivation,
    NoCurrentCommentResearchCandidates,
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BlankWorkspaceId => write!(formatter, "workspace ID must not be blank"),
            Self::BlankCommentContextIdentity => write!(
                formatter,
                "comment context note and comment identifiers must not be blank"
            ),
            Self::InvalidCurrentCommentVoicesLimit => write!(
                formatter,
                "current comment voices limit must be between 1 and {CURRENT_COMMENT_VOICES_V0_MAX_LIMIT}"
            ),
            Self::InvalidCurrentCommentVoicesOffset => write!(
                formatter,
                "current comment voices offset must be zero or greater"
            ),
            Self::InvalidCommentResearchPlanPreviewLimit => write!(
                formatter,
                "comment research plan preview limit must be between 1 and {COMMENT_RESEARCH_PLAN_PREVIEW_V0_MAX_LIMIT}"
            ),
            Self::InvalidCommentResearchRunPreparationLimit => write!(
                formatter,
                "comment research run preparation limit must be between 1 and {COMMENT_RESEARCH_RUN_PREPARATION_V1_MAX_LIMIT}"
            ),
            Self::InvalidCurrentCommentSourceRecordIndex => write!(
                formatter,
                "current comment source record index must be zero or greater"
            ),
            Self::InvalidCommentDerivationMaterializationLimit => write!(
                formatter,
                "comment derivation materialization limit must be between 1 and {COMMENT_DERIVATION_V1_MATERIALIZATION_MAX_LIMIT}"
            ),
            Self::Preparation(error) => {
                write!(formatter, "comment evidence preparation failed: {error}")
            }
            Self::Database(error) => write!(
                formatter,
                "PostgreSQL comment fact operation failed: {error}"
            ),
            Self::SourceRecordIndexOutOfRange => write!(
                formatter,
                "source record index exceeds PostgreSQL integer range"
            ),
            Self::MissingPersistedEvidence => write!(
                formatter,
                "source evidence was not returned after idempotent admission"
            ),
            Self::MissingPersistedSourceCapturePair => write!(
                formatter,
                "source capture pair was not returned after idempotent admission"
            ),
            Self::MissingPersistedSourceRecord => write!(
                formatter,
                "evidence comment record was not returned after idempotent admission"
            ),
            Self::InvalidPersistedSourceRecord => write!(
                formatter,
                "existing evidence comment record does not match validated source facts"
            ),
            Self::MissingLockedIdentity => write!(
                formatter,
                "comment identity was not returned after idempotent admission"
            ),
            Self::InvalidPersistedCurrent => write!(
                formatter,
                "current projection contains an invalid observation reference"
            ),
            Self::MissingPersistedSourceContextCaptureSet => write!(
                formatter,
                "source context capture set was not returned after idempotent admission"
            ),
            Self::MissingPersistedContextRecord => write!(
                formatter,
                "context source record was not returned after idempotent admission"
            ),
            Self::InvalidPersistedContextRecord => write!(
                formatter,
                "existing context source record does not match validated source facts"
            ),
            Self::InvalidPersistedContextTextAvailability => write!(
                formatter,
                "context read contains an invalid text availability materialization"
            ),
            Self::MissingPersistedCommentDerivation => write!(
                formatter,
                "comment derivation was not returned after idempotent materialization"
            ),
            Self::InvalidPersistedCommentDerivation => write!(
                formatter,
                "persisted comment derivation does not match the deterministic cleaning contract"
            ),
            Self::NoCurrentCommentResearchCandidates => write!(
                formatter,
                "current scope has no comment research inputs to freeze"
            ),
        }
    }
}

impl std::error::Error for StorageError {}

async fn build_current_comment_research_context_pack_in_transaction_v1(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    candidate: &CurrentCommentResearchPreparationCandidateV1,
) -> Result<CommentResearchContextPackV1, StorageError> {
    let selected = transaction
        .query_opt(
            "SELECT context_set.replies_evidence_id, work.source_evidence_id, \
                    work.source_record_index, work.title_availability, \
                    work.title_source_text, work.body_text_availability, \
                    work.body_text_source_text \
               FROM comment_current_v0 AS current_projection \
               JOIN context_discussion_record_v0 AS matching_comment \
                 ON matching_comment.workspace_id = current_projection.workspace_id \
                AND matching_comment.platform = current_projection.platform \
                AND matching_comment.note_id = current_projection.note_id \
                AND matching_comment.comment_id = current_projection.comment_id \
                AND matching_comment.record_kind = 'comment' \
                AND matching_comment.text_sha256 = current_projection.text_sha256 \
               JOIN source_context_capture_set_v0 AS context_set \
                 ON context_set.workspace_id = matching_comment.workspace_id \
                AND context_set.platform = matching_comment.platform \
                AND context_set.note_id = matching_comment.note_id \
                AND context_set.comments_evidence_id = matching_comment.source_evidence_id \
               JOIN context_work_record_v0 AS work \
                 ON work.source_evidence_id = context_set.detail_evidence_id \
                AND work.source_record_index = 0 \
              WHERE current_projection.workspace_id = $1 \
                AND current_projection.platform = 'xhs' \
                AND current_projection.note_id = $2 \
                AND current_projection.comment_id = $3 \
                AND current_projection.current_observation_id = $4 \
              ORDER BY context_set.admitted_at DESC, context_set.id ASC \
              LIMIT 1",
            &[
                &workspace_id,
                &candidate.source_note_id,
                &candidate.comment_id,
                &candidate.observation_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    let Some(selected) = selected else {
        return Ok(build_comment_research_context_pack_v1(
            CommentResearchContextPackInputV1 {
                research_expression: candidate.research_text.clone(),
                readiness: candidate.readiness.as_context_pack_readiness(),
                has_source_backed_context: false,
                related_discussion: Vec::new(),
                work_title: ContextPackSourceTextV1::Unavailable,
                work_body: ContextPackSourceTextV1::Unavailable,
            },
        ));
    };

    let replies_evidence_id: Uuid = selected.get(0);
    let title =
        read_context_text_availability(selected.get(3), selected.get::<_, Option<String>>(4))?;
    let body_text =
        read_context_text_availability(selected.get(5), selected.get::<_, Option<String>>(6))?;
    let reply_rows = transaction
        .query(
            "SELECT source_text, root_comment_id, parent_comment_id, reply_to_comment_id \
               FROM context_discussion_record_v0 \
              WHERE source_evidence_id = $1 \
                AND record_kind = 'reply' \
                AND workspace_id = $2 \
                AND platform = 'xhs' \
                AND note_id = $3 \
                AND (root_comment_id = $4 OR parent_comment_id = $4 OR reply_to_comment_id = $4) \
              ORDER BY source_record_index ASC \
              LIMIT $5",
            &[
                &replies_evidence_id,
                &workspace_id,
                &candidate.source_note_id,
                &candidate.comment_id,
                &CURRENT_COMMENT_CONTEXT_V0_MAX_REPLIES,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    Ok(build_comment_research_context_pack_v1(
        CommentResearchContextPackInputV1 {
            research_expression: candidate.research_text.clone(),
            readiness: candidate.readiness.as_context_pack_readiness(),
            has_source_backed_context: true,
            related_discussion: reply_rows
                .into_iter()
                .map(|row| ContextPackRelatedDiscussionInputV1 {
                    text: row.get(0),
                    root_comment: row.get::<_, Option<String>>(1).is_some(),
                    parent_comment: row.get::<_, Option<String>>(2).is_some(),
                    reply_to_comment: row.get::<_, Option<String>>(3).is_some(),
                })
                .collect(),
            work_title: context_pack_source_text_v1(title),
            work_body: context_pack_source_text_v1(body_text),
        },
    ))
}

fn context_discussion_sort_key(
    record: &PreparedContextDiscussionRecordV0,
) -> (u8, &CommentSourceIdentityV0, usize) {
    let kind = match record.kind {
        PreparedContextDiscussionRecordKindV0::Comment => 0,
        PreparedContextDiscussionRecordKindV0::Reply => 1,
    };
    (kind, &record.identity, record.source_record_index)
}

fn context_text_storage_values(
    availability: &ContextTextAvailabilityV0,
) -> (&'static str, Option<&str>) {
    match availability {
        ContextTextAvailabilityV0::Unavailable => ("unavailable", None),
        ContextTextAvailabilityV0::Blank => ("blank", None),
        ContextTextAvailabilityV0::Observed(value) => ("observed", Some(value)),
    }
}

fn context_pack_source_text_v1(value: ContextTextAvailabilityV0) -> ContextPackSourceTextV1 {
    match value {
        ContextTextAvailabilityV0::Unavailable => ContextPackSourceTextV1::Unavailable,
        ContextTextAvailabilityV0::Blank => ContextPackSourceTextV1::Blank,
        ContextTextAvailabilityV0::Observed(value) => ContextPackSourceTextV1::Observed(value),
    }
}

fn read_context_text_availability(
    availability: String,
    source_text: Option<String>,
) -> Result<ContextTextAvailabilityV0, StorageError> {
    match (availability.as_str(), source_text) {
        ("unavailable", None) => Ok(ContextTextAvailabilityV0::Unavailable),
        ("blank", None) => Ok(ContextTextAvailabilityV0::Blank),
        ("observed", Some(value)) if !value.trim().is_empty() => {
            Ok(ContextTextAvailabilityV0::Observed(value))
        }
        _ => Err(StorageError::InvalidPersistedContextTextAvailability),
    }
}

async fn admit_context_work_record(
    transaction: &Transaction<'_>,
    source_evidence_id: Uuid,
    workspace_id: &str,
    work_context: &PreparedContextWorkRecordV0,
) -> Result<(), StorageError> {
    let source_record_index = i32::try_from(work_context.source_record_index)
        .map_err(|_| StorageError::SourceRecordIndexOutOfRange)?;
    let (title_availability, title_source_text) = context_text_storage_values(&work_context.title);
    let (body_text_availability, body_text_source_text) =
        context_text_storage_values(&work_context.body_text);

    transaction
        .execute(
            "INSERT INTO context_work_record_v0 \
             (source_evidence_id, source_record_index, workspace_id, platform, note_id, \
              title_availability, title_source_text, body_text_availability, body_text_source_text, author_id) \
             VALUES ($1, $2, $3, 'xhs', $4, $5, $6, $7, $8, $9) \
             ON CONFLICT (source_evidence_id, source_record_index) DO NOTHING",
            &[
                &source_evidence_id,
                &source_record_index,
                &workspace_id,
                &work_context.note_id,
                &title_availability,
                &title_source_text,
                &body_text_availability,
                &body_text_source_text,
                &work_context.author_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    let record = transaction
        .query_opt(
            "SELECT workspace_id, platform, note_id, title_availability, title_source_text, \
                    body_text_availability, body_text_source_text, author_id \
               FROM context_work_record_v0 \
              WHERE source_evidence_id = $1 AND source_record_index = $2",
            &[&source_evidence_id, &source_record_index],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedContextRecord)?;

    if record.get::<_, String>(0) != workspace_id
        || record.get::<_, String>(1) != "xhs"
        || record.get::<_, String>(2) != work_context.note_id
        || record.get::<_, String>(3) != title_availability
        || record.get::<_, Option<String>>(4).as_deref() != title_source_text
        || record.get::<_, String>(5) != body_text_availability
        || record.get::<_, Option<String>>(6).as_deref() != body_text_source_text
        || record.get::<_, Option<String>>(7).as_deref() != work_context.author_id.as_deref()
    {
        return Err(StorageError::InvalidPersistedContextRecord);
    }

    Ok(())
}

async fn admit_context_discussion_record(
    transaction: &Transaction<'_>,
    source_evidence_id: Uuid,
    record: &PreparedContextDiscussionRecordV0,
) -> Result<(), StorageError> {
    let source_record_index = i32::try_from(record.source_record_index)
        .map_err(|_| StorageError::SourceRecordIndexOutOfRange)?;
    let kind = match record.kind {
        PreparedContextDiscussionRecordKindV0::Comment => "comment",
        PreparedContextDiscussionRecordKindV0::Reply => "reply",
    };

    transaction
        .execute(
            "INSERT INTO context_discussion_record_v0 \
             (source_evidence_id, source_record_index, record_kind, workspace_id, platform, note_id, \
              comment_id, text_sha256, source_text, author_id, root_comment_id, parent_comment_id, reply_to_comment_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13) \
             ON CONFLICT (source_evidence_id, source_record_index) DO NOTHING",
            &[
                &source_evidence_id,
                &source_record_index,
                &kind,
                &record.identity.workspace_id,
                &record.identity.platform,
                &record.identity.note_id,
                &record.identity.comment_id,
                &record.text_sha256,
                &record.text,
                &record.author_id,
                &record.root_comment_id,
                &record.parent_comment_id,
                &record.reply_to_comment_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    let stored = transaction
        .query_opt(
            "SELECT record_kind, workspace_id, platform, note_id, comment_id, text_sha256, source_text, \
                    author_id, root_comment_id, parent_comment_id, reply_to_comment_id \
               FROM context_discussion_record_v0 \
              WHERE source_evidence_id = $1 AND source_record_index = $2",
            &[&source_evidence_id, &source_record_index],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedContextRecord)?;

    if stored.get::<_, String>(0) != kind
        || stored.get::<_, String>(1) != record.identity.workspace_id
        || stored.get::<_, String>(2) != record.identity.platform
        || stored.get::<_, String>(3) != record.identity.note_id
        || stored.get::<_, String>(4) != record.identity.comment_id
        || stored.get::<_, String>(5) != record.text_sha256
        || stored.get::<_, String>(6) != record.text
        || stored.get::<_, Option<String>>(7).as_deref() != record.author_id.as_deref()
        || stored.get::<_, Option<String>>(8).as_deref() != record.root_comment_id.as_deref()
        || stored.get::<_, Option<String>>(9).as_deref() != record.parent_comment_id.as_deref()
        || stored.get::<_, Option<String>>(10).as_deref() != record.reply_to_comment_id.as_deref()
    {
        return Err(StorageError::InvalidPersistedContextRecord);
    }

    Ok(())
}

async fn admit_source_context_capture_set(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    note_id: &str,
    detail_evidence_id: Uuid,
    comments_evidence_id: Uuid,
    replies_evidence_id: Uuid,
    record_counts: (i32, i32),
) -> Result<SourceContextCaptureSetAdmissionOutcomeV0, StorageError> {
    let (comments_record_count, replies_record_count) = record_counts;
    let inserted = transaction
        .query_opt(
            "INSERT INTO source_context_capture_set_v0 \
             (id, workspace_id, platform, note_id, detail_evidence_id, comments_evidence_id, replies_evidence_id, comments_record_count, replies_record_count) \
             VALUES ($1, $2, 'xhs', $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (workspace_id, detail_evidence_id, comments_evidence_id, replies_evidence_id) \
             DO NOTHING RETURNING id",
            &[
                &Uuid::new_v4(),
                &workspace_id,
                &note_id,
                &detail_evidence_id,
                &comments_evidence_id,
                &replies_evidence_id,
                &comments_record_count,
                &replies_record_count,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    if inserted.is_some() {
        return Ok(SourceContextCaptureSetAdmissionOutcomeV0 { inserted: true });
    }

    let stored = transaction
        .query_opt(
            "SELECT note_id, comments_record_count, replies_record_count \
               FROM source_context_capture_set_v0 \
              WHERE workspace_id = $1 AND detail_evidence_id = $2 \
                AND comments_evidence_id = $3 AND replies_evidence_id = $4",
            &[
                &workspace_id,
                &detail_evidence_id,
                &comments_evidence_id,
                &replies_evidence_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedSourceContextCaptureSet)?;
    if stored.get::<_, String>(0) != note_id
        || stored.get::<_, i32>(1) != comments_record_count
        || stored.get::<_, i32>(2) != replies_record_count
    {
        return Err(StorageError::InvalidPersistedContextRecord);
    }

    Ok(SourceContextCaptureSetAdmissionOutcomeV0 { inserted: false })
}

async fn admit_source_evidence(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    source_contract: &str,
    source: &PreparedSourceEvidenceV0,
) -> Result<EvidenceAdmissionOutcomeV0, StorageError> {
    let candidate_id = Uuid::new_v4();
    let inserted = transaction
        .query_opt(
            "INSERT INTO source_evidence_v0 \
             (id, workspace_id, platform, source_contract, package_kind, payload_sha256, source_payload) \
             VALUES ($1, $2, 'xhs', $3, $4, $5, $6) \
             ON CONFLICT (workspace_id, platform, source_contract, package_kind, payload_sha256) \
             DO NOTHING RETURNING id",
            &[
                &candidate_id,
                &workspace_id,
                &source_contract,
                &source.package_kind,
                &source.payload_sha256,
                &source.payload,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    if let Some(row) = inserted {
        return Ok(EvidenceAdmissionOutcomeV0 {
            id: row.get(0),
            inserted: true,
        });
    }

    let row = transaction
        .query_opt(
            "SELECT id FROM source_evidence_v0 \
             WHERE workspace_id = $1 AND platform = 'xhs' AND source_contract = $2 \
               AND package_kind = $3 AND payload_sha256 = $4",
            &[
                &workspace_id,
                &source_contract,
                &source.package_kind,
                &source.payload_sha256,
            ],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedEvidence)?;

    Ok(EvidenceAdmissionOutcomeV0 {
        id: row.get(0),
        inserted: false,
    })
}

async fn admit_source_capture_pair(
    transaction: &Transaction<'_>,
    workspace_id: &str,
    detail_evidence_id: Uuid,
    comments_evidence_id: Uuid,
) -> Result<SourceCapturePairAdmissionOutcomeV0, StorageError> {
    let inserted = transaction
        .query_opt(
            "INSERT INTO source_capture_pair_v0 \
             (id, workspace_id, detail_evidence_id, comments_evidence_id) \
             VALUES ($1, $2, $3, $4) \
             ON CONFLICT (workspace_id, detail_evidence_id, comments_evidence_id) \
             DO NOTHING RETURNING id",
            &[
                &Uuid::new_v4(),
                &workspace_id,
                &detail_evidence_id,
                &comments_evidence_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    if inserted.is_some() {
        return Ok(SourceCapturePairAdmissionOutcomeV0 { inserted: true });
    }

    let exists = transaction
        .query_opt(
            "SELECT 1 FROM source_capture_pair_v0 \
             WHERE workspace_id = $1 AND detail_evidence_id = $2 AND comments_evidence_id = $3",
            &[&workspace_id, &detail_evidence_id, &comments_evidence_id],
        )
        .await
        .map_err(StorageError::Database)?;
    if exists.is_none() {
        return Err(StorageError::MissingPersistedSourceCapturePair);
    }

    Ok(SourceCapturePairAdmissionOutcomeV0 { inserted: false })
}

async fn materialize_comment_derivation_v1(
    transaction: &Transaction<'_>,
    comment_observation_id: Uuid,
    source_text: &str,
) -> Result<(), StorageError> {
    let cleaned = clean_comment_for_research(source_text);
    let research_state = comment_research_state_storage_value(cleaned.state);
    let reason_codes = cleaned
        .reason_codes
        .iter()
        .copied()
        .map(cleaning_reason_code_storage_value)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let research_text = cleaned.research_text.as_deref();

    let inserted = transaction
        .query_opt(
            "INSERT INTO comment_derivation_v1 \
             (id, comment_observation_id, cleaning_contract, research_state, research_text, reason_codes) \
             VALUES ($1, $2, 'comment-cleaning.v1', $3, $4, $5) \
             ON CONFLICT (comment_observation_id, cleaning_contract) DO NOTHING \
             RETURNING id",
            &[
                &Uuid::new_v4(),
                &comment_observation_id,
                &research_state,
                &research_text,
                &reason_codes,
            ],
        )
        .await
        .map_err(StorageError::Database)?;
    if inserted.is_some() {
        return Ok(());
    }

    // This path is only possible if an explicit materialization pass races a
    // successful admission. The immutable row must be exactly the same
    // deterministic contract result; otherwise silently accepting it would
    // hide a corrupted derivation.
    let stored = transaction
        .query_opt(
            "SELECT cleaning_contract, research_state, research_text, reason_codes \
              FROM comment_derivation_v1 \
              WHERE comment_observation_id = $1 \
                AND cleaning_contract = 'comment-cleaning.v1'",
            &[&comment_observation_id],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedCommentDerivation)?;
    if stored.get::<_, String>(0) != "comment-cleaning.v1"
        || stored.get::<_, String>(1) != research_state
        || stored.get::<_, Option<String>>(2).as_deref() != research_text
        || stored.get::<_, Vec<String>>(3) != reason_codes
    {
        return Err(StorageError::InvalidPersistedCommentDerivation);
    }
    Ok(())
}

fn comment_research_state_storage_value(state: CommentResearchState) -> &'static str {
    match state {
        CommentResearchState::Dropped => "dropped",
        CommentResearchState::Analyzable => "analyzable",
        CommentResearchState::NeedsContext => "needs_context",
        CommentResearchState::Anomaly => "anomaly",
    }
}

fn cleaning_reason_code_storage_value(reason: CleaningReasonCode) -> &'static str {
    match reason {
        CleaningReasonCode::Blank => "blank",
        CleaningReasonCode::MentionOnly => "mention_only",
        CleaningReasonCode::EmojiOnly => "emoji_only",
        CleaningReasonCode::MentionAndEmojiOnly => "mention_and_emoji_only",
        CleaningReasonCode::PunctuationOnly => "punctuation_only",
        CleaningReasonCode::NoEffectiveText => "no_effective_text",
        CleaningReasonCode::ControlCharacter => "control_character",
        CleaningReasonCode::ContextDependentReply => "context_dependent_reply",
    }
}

async fn admit_comment_record(
    transaction: &Transaction<'_>,
    source_evidence_id: Uuid,
    comment: &PreparedCommentRecordV0,
) -> Result<CommentAdmissionOutcomeV0, StorageError> {
    let source_record_index = i32::try_from(comment.source_record_index)
        .map_err(|_| StorageError::SourceRecordIndexOutOfRange)?;
    let identity = &comment.identity;

    transaction
        .execute(
            "INSERT INTO comment_identity_v0 (workspace_id, platform, note_id, comment_id) \
             VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
            &[
                &identity.workspace_id,
                &identity.platform,
                &identity.note_id,
                &identity.comment_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    // Serialize current decisions per stable external identity.
    let locked_identity = transaction
        .query_opt(
            "SELECT 1 FROM comment_identity_v0 \
             WHERE workspace_id = $1 AND platform = $2 AND note_id = $3 AND comment_id = $4 \
             FOR UPDATE",
            &[
                &identity.workspace_id,
                &identity.platform,
                &identity.note_id,
                &identity.comment_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;
    if locked_identity.is_none() {
        return Err(StorageError::MissingLockedIdentity);
    }

    transaction
        .execute(
            "INSERT INTO evidence_comment_record_v0 \
             (source_evidence_id, source_record_index, workspace_id, platform, note_id, comment_id, text_sha256) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (source_evidence_id, source_record_index) DO NOTHING",
            &[
                &source_evidence_id,
                &source_record_index,
                &identity.workspace_id,
                &identity.platform,
                &identity.note_id,
                &identity.comment_id,
                &comment.text_sha256,
            ],
        )
        .await
        .map_err(StorageError::Database)?;

    let source_record = transaction
        .query_opt(
            "SELECT workspace_id, platform, note_id, comment_id, text_sha256 \
             FROM evidence_comment_record_v0 \
             WHERE source_evidence_id = $1 AND source_record_index = $2",
            &[&source_evidence_id, &source_record_index],
        )
        .await
        .map_err(StorageError::Database)?
        .ok_or(StorageError::MissingPersistedSourceRecord)?;
    if source_record.get::<_, String>(0) != identity.workspace_id
        || source_record.get::<_, String>(1) != identity.platform
        || source_record.get::<_, String>(2) != identity.note_id
        || source_record.get::<_, String>(3) != identity.comment_id
        || source_record.get::<_, String>(4) != comment.text_sha256
    {
        return Err(StorageError::InvalidPersistedSourceRecord);
    }

    // Do not allow an exact old source record to reset a newer current value.
    if let Some(row) = transaction
        .query_opt(
            "SELECT id FROM comment_observation_v0 \
             WHERE source_evidence_id = $1 AND source_record_index = $2",
            &[&source_evidence_id, &source_record_index],
        )
        .await
        .map_err(StorageError::Database)?
    {
        return Ok(CommentAdmissionOutcomeV0 {
            identity: identity.clone(),
            disposition: CommentProjectionDispositionV0::PreviouslyAdmittedSource {
                observation_id: row.get(0),
            },
        });
    }

    let current = transaction
        .query_opt(
            "SELECT current_observation_id, text_sha256 FROM comment_current_v0 \
             WHERE workspace_id = $1 AND platform = $2 AND note_id = $3 AND comment_id = $4",
            &[
                &identity.workspace_id,
                &identity.platform,
                &identity.note_id,
                &identity.comment_id,
            ],
        )
        .await
        .map_err(StorageError::Database)?;
    let current_fact = current.as_ref().map(|row| CurrentCommentFactV0 {
        text_sha256: row.get(1),
    });

    match decide_comment_observation_v0(current_fact.as_ref(), comment) {
        CommentObservationDecisionV0::ReplayUnchanged => Ok(CommentAdmissionOutcomeV0 {
            identity: identity.clone(),
            disposition: CommentProjectionDispositionV0::ReplayUnchanged,
        }),
        CommentObservationDecisionV0::CreateInitial
        | CommentObservationDecisionV0::AppendAndAdvance => {
            let observation_id = Uuid::new_v4();
            transaction
                .execute(
                    "INSERT INTO comment_observation_v0 \
                     (id, workspace_id, platform, note_id, comment_id, source_evidence_id, source_record_index, text_sha256, source_text) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                    &[
                        &observation_id,
                        &identity.workspace_id,
                        &identity.platform,
                        &identity.note_id,
                        &identity.comment_id,
                        &source_evidence_id,
                        &source_record_index,
                        &comment.text_sha256,
                        &comment.text,
                    ],
                )
                .await
                .map_err(StorageError::Database)?;

            // The original source text remains in CommentObservation. This
            // immutable child stores only the deterministic research form and
            // is created in this same admission transaction. A source replay
            // reaches the earlier return and never creates another child.
            materialize_comment_derivation_v1(transaction, observation_id, &comment.text).await?;

            let disposition = match current {
                Some(current) => {
                    let previous_observation_id: Uuid = current.get(0);
                    let updated = transaction
                        .execute(
                            "UPDATE comment_current_v0 \
                             SET current_observation_id = $1, text_sha256 = $2, advanced_at = clock_timestamp() \
                             WHERE workspace_id = $3 AND platform = $4 AND note_id = $5 AND comment_id = $6",
                            &[
                                &observation_id,
                                &comment.text_sha256,
                                &identity.workspace_id,
                                &identity.platform,
                                &identity.note_id,
                                &identity.comment_id,
                            ],
                        )
                        .await
                        .map_err(StorageError::Database)?;
                    if updated != 1 {
                        return Err(StorageError::InvalidPersistedCurrent);
                    }
                    CommentProjectionDispositionV0::Advanced {
                        previous_observation_id,
                        observation_id,
                    }
                }
                None => {
                    transaction
                        .execute(
                            "INSERT INTO comment_current_v0 \
                             (workspace_id, platform, note_id, comment_id, current_observation_id, text_sha256) \
                             VALUES ($1, $2, $3, $4, $5, $6)",
                            &[
                                &identity.workspace_id,
                                &identity.platform,
                                &identity.note_id,
                                &identity.comment_id,
                                &observation_id,
                                &comment.text_sha256,
                            ],
                        )
                        .await
                        .map_err(StorageError::Database)?;
                    CommentProjectionDispositionV0::Created { observation_id }
                }
            };

            Ok(CommentAdmissionOutcomeV0 {
                identity: identity.clone(),
                disposition,
            })
        }
    }
}
