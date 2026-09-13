//! PostgreSQL implementation of Comment fact admission V0.
//!
//! The public entry point always delegates to linggan-evidence, which invokes
//! the XHS runtime source contract before its transaction starts.

use core::fmt;

use linggan_contracts::{CapturePackageV0, ContextTextAvailabilityV0};
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
use tokio_postgres::{Client, NoTls, Transaction};
use uuid::Uuid;

/// The one greenfield migration required by Comment fact storage V0.
pub const COMMENT_FACT_STORAGE_V0_MIGRATION: &str =
    include_str!("../../../database/migrations/0001_comment_fact_storage_v0.sql");

/// The follow-on migration for source-backed work and reply context. It
/// depends on `COMMENT_FACT_STORAGE_V0_MIGRATION` having already been applied.
pub const COMMENT_CONTEXT_STORAGE_V0_MIGRATION: &str =
    include_str!("../../../database/migrations/0002_comment_context_storage_v0.sql");

/// A PostgreSQL adapter with no knowledge of HTTP, workers, models, or UI.
pub struct CommentFactStore {
    client: Client,
}

/// The largest page accepted by the current-comment read model.
///
/// This is a transport safety bound, not a statement about how many comments
/// should be researched together.
pub const CURRENT_COMMENT_VOICES_V0_MAX_LIMIT: i64 = 100;

/// A bounded, offset-based request for the User Voices V0 read model.
///
/// Pagination belongs here instead of the HTTP layer so other callers cannot
/// accidentally create an unbounded current-comment read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentCommentVoicesPageRequestV0 {
    limit: i64,
    offset: i64,
}

impl CurrentCommentVoicesPageRequestV0 {
    pub fn new(limit: i64, offset: i64) -> Result<Self, StorageError> {
        if !(1..=CURRENT_COMMENT_VOICES_V0_MAX_LIMIT).contains(&limit) {
            return Err(StorageError::InvalidCurrentCommentVoicesLimit);
        }
        if offset < 0 {
            return Err(StorageError::InvalidCurrentCommentVoicesOffset);
        }
        Ok(Self { limit, offset })
    }

    pub const fn limit(self) -> i64 {
        self.limit
    }

    pub const fn offset(self) -> i64 {
        self.offset
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
    pub text: String,
    pub current_admitted_at: String,
    pub source_evidence: CurrentCommentSourceEvidenceRelationV0,
}

/// One deterministically ordered page of current comment facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentVoicesPageV0 {
    pub total: i64,
    pub voices: Vec<CurrentCommentVoiceV0>,
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

/// Context reads return at most this many related reply facts. This bound is a
/// transport safety limit, not a claim that the complete discussion tree was
/// captured.
pub const CURRENT_COMMENT_CONTEXT_V0_MAX_REPLIES: i64 = 100;

impl CommentFactStore {
    /// Creates a store from a caller-managed PostgreSQL client.
    pub const fn new(client: Client) -> Self {
        Self { client }
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
            .batch_execute(COMMENT_CONTEXT_STORAGE_V0_MIGRATION)
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

        let rows = self
            .client
            .query(
                "WITH filtered AS ( \
                   SELECT current_projection.note_id, current_projection.comment_id, \
                          current_observation.source_text, current_observation.admitted_at, \
                          current_observation.source_evidence_id, \
                          current_observation.source_record_index \
                     FROM comment_current_v0 AS current_projection \
                     JOIN comment_observation_v0 AS current_observation \
                       ON current_observation.workspace_id = current_projection.workspace_id \
                      AND current_observation.platform = current_projection.platform \
                      AND current_observation.note_id = current_projection.note_id \
                      AND current_observation.comment_id = current_projection.comment_id \
                      AND current_observation.id = current_projection.current_observation_id \
                    WHERE current_projection.workspace_id = $1 \
                 ), page AS ( \
                   SELECT * FROM filtered \
                    ORDER BY admitted_at ASC, note_id ASC, comment_id ASC \
                    LIMIT $2 OFFSET $3 \
                 ), total AS ( \
                   SELECT count(*)::BIGINT AS total FROM filtered \
                 ) \
                 SELECT page.note_id, page.source_text, \
                        to_char(page.admitted_at AT TIME ZONE 'UTC', \
                          'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS current_admitted_at, \
                        page.source_evidence_id, page.source_record_index, total.total \
                   FROM total \
                   LEFT JOIN page ON TRUE \
                  ORDER BY page.admitted_at ASC NULLS LAST, page.note_id ASC NULLS LAST, \
                           page.comment_id ASC NULLS LAST",
                &[&workspace_id, &page.limit(), &page.offset()],
            )
            .await
            .map_err(StorageError::Database)?;

        let total = rows.first().map_or(0, |row| row.get(5));
        let voices = rows
            .into_iter()
            .filter_map(|row| {
                let source_note_id: Option<String> = row.get(0);
                source_note_id.map(|source_note_id| CurrentCommentVoiceV0 {
                    source_note_id,
                    text: row.get(1),
                    current_admitted_at: row.get(2),
                    source_evidence: CurrentCommentSourceEvidenceRelationV0 {
                        evidence_id: row.get(3),
                        record_index: row.get(4),
                    },
                })
            })
            .collect();

        Ok(CurrentCommentVoicesPageV0 { total, voices })
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

        let selected = self
            .client
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

        let rows = self
            .client
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
        let transaction = self
            .client
            .transaction()
            .await
            .map_err(StorageError::Database)?;

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
        let transaction = self
            .client
            .transaction()
            .await
            .map_err(StorageError::Database)?;

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
        }
    }
}

impl std::error::Error for StorageError {}

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
