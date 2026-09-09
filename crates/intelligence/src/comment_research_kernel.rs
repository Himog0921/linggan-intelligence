//! COMMENT-RESEARCH-RESET-001 input boundary.
//!
//! This module has one responsibility: turn a current, readable comment fact into a versioned
//! research derivation. It never mutates the raw comment, calls a model, or decides a Problem.

use crate::{
    comment_cleaning::{CLEANER_VERSION, CleanComment, clean},
    comment_research::comment_source_hash,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub const DERIVATION_VERSION: &str = "comment-research.derivation.v1";
const MAX_DERIVATIONS_PER_PASS: i64 = 3000;
const EXTRACTION_CONTRACT: &str =
    "comment-research.semantic.v1/extract:problem,need,solution,experience";
const MEMBERSHIP_CONTRACT: &str =
    "comment-research.semantic.v1/membership:retrieval-only-before-decision";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentAuthorRole {
    OrdinaryUser,
    ContentAuthorReply,
    AuthorIdentityUnknown,
}

impl CommentAuthorRole {
    fn as_db(self) -> &'static str {
        match self {
            Self::OrdinaryUser => "ordinary_user",
            Self::ContentAuthorReply => "content_author_reply",
            Self::AuthorIdentityUnknown => "author_identity_unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchText {
    pub text: String,
    pub offsets: Vec<(usize, usize)>,
    pub clean_state: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DerivationEligibility {
    Eligible,
    ExcludedAuthorReply,
    AuthorIdentityUnknown,
    SourceBodyUnknown,
    DroppedOrAnomalous,
}

impl DerivationEligibility {
    fn as_db(self) -> &'static str {
        match self {
            Self::Eligible => "eligible",
            Self::ExcludedAuthorReply => "excluded_author_reply",
            Self::AuthorIdentityUnknown => "author_identity_unknown",
            Self::SourceBodyUnknown => "source_body_unknown",
            Self::DroppedOrAnomalous => "dropped_or_anomalous",
        }
    }

    fn reason(self) -> &'static str {
        match self {
            Self::Eligible => "ordinary_user_readable",
            Self::ExcludedAuthorReply => "content_author_reply",
            Self::AuthorIdentityUnknown => "content_author_identity_unknown",
            Self::SourceBodyUnknown => "comment_body_unknown",
            Self::DroppedOrAnomalous => "cleaner_excluded",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CommentResearchKernelError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("research derivation serialization failed")]
    Serialization,
    #[error("research policy values are invalid")]
    InvalidPolicy,
    #[error("no saved comment research policy exists")]
    PolicyMissing,
    #[error("the requested research scope is invalid")]
    InvalidScope,
    #[error("no eligible ordinary-user derivations are available")]
    NoEligibleDerivations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveResearchPolicy {
    pub config_ref: Option<Uuid>,
    pub source_limit: i32,
    pub token_limit: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchPolicyReceipt {
    pub policy_revision_ref: Uuid,
    pub active_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunReceipt {
    pub run_ref: Uuid,
    pub policy_revision_ref: Uuid,
    pub selected_sources: usize,
    pub external_calls_started: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunItemFailureClass {
    Retryable,
    Incompatible,
    Unrecoverable,
    ModelFailed,
}

impl RunItemFailureClass {
    fn terminal_state(self) -> Option<&'static str> {
        match self {
            Self::Retryable => None,
            Self::Incompatible => Some("incompatible"),
            Self::Unrecoverable => Some("unrecoverable"),
            Self::ModelFailed => Some("model_failed"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchRunItemClaim {
    pub run_ref: Uuid,
    pub derivation_ref: Uuid,
    pub attempt: i32,
}

/// A role is a comparison between two collected platform identities, never a name heuristic.
pub fn classify_author_role(
    comment_author_external_id: Option<&str>,
    content_author_external_id: Option<&str>,
) -> CommentAuthorRole {
    let comment_author = comment_author_external_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let content_author = content_author_external_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match (comment_author, content_author) {
        (Some(comment_author), Some(content_author)) if comment_author == content_author => {
            CommentAuthorRole::ContentAuthorReply
        }
        (Some(_), Some(_)) => CommentAuthorRole::OrdinaryUser,
        _ => CommentAuthorRole::AuthorIdentityUnknown,
    }
}

/// Removes the platform role badge only after author identity has independently established that
/// it is a content-author reply. An ordinary user may genuinely start a sentence with “作者”.
pub fn derive_research_text(raw: &str, author_role: CommentAuthorRole) -> ResearchText {
    let mut cleaned = clean(raw);
    if author_role == CommentAuthorRole::ContentAuthorReply {
        strip_confirmed_content_author_badge(&mut cleaned);
    }
    ResearchText {
        text: cleaned.text,
        offsets: cleaned.offsets,
        clean_state: cleaned.state,
        reasons: cleaned.reasons,
    }
}

fn strip_confirmed_content_author_badge(cleaned: &mut CleanComment) {
    let characters: Vec<char> = cleaned.text.chars().collect();
    if characters.get(0..3) != Some(&['作', '者', ' ']) {
        return;
    }
    let remove = characters
        .iter()
        .enumerate()
        .skip(2)
        .take_while(|(_, character)| character.is_whitespace())
        .last()
        .map_or(2, |(index, _)| index + 1);
    cleaned.text = characters[remove..].iter().collect();
    cleaned.offsets = cleaned.offsets[remove..].to_vec();
    cleaned.reasons.push("content_author_badge_removed".into());
}

/// Derives up to `limit` current sources. Re-running is safe: source + version + raw hash form
/// the immutable identity, and a concurrent worker can only win the same `ON CONFLICT` insert.
pub async fn derive_current_sources(
    database: &Database,
    limit: usize,
) -> Result<u64, CommentResearchKernelError> {
    let limit = i64::try_from(limit)
        .unwrap_or(MAX_DERIVATIONS_PER_PASS)
        .clamp(1, MAX_DERIVATIONS_PER_PASS);
    let rows = sqlx::query(
        "SELECT source.material_ref,source.content_public_ref,source.parent_comment_external_id, \
                source.body_text,source.author_external_id, \
                attribution.author_external_id AS content_author_external_id, \
                attribution.attribution_source,attribution.observed_at::text AS attribution_observed_at, \
                parent.material_ref AS parent_source_ref,parent.body_text AS parent_body_text \
         FROM linggan_comment_research_source_current source \
         LEFT JOIN linggan_material_content_author attribution \
           ON attribution.content_public_ref=source.content_public_ref \
         LEFT JOIN LATERAL ( \
           SELECT candidate.material_ref,candidate.body_text \
           FROM linggan_comment_research_source_current candidate \
           WHERE candidate.content_public_ref=source.content_public_ref \
             AND candidate.comment_external_id=source.parent_comment_external_id \
           ORDER BY candidate.observed_at::timestamptz DESC,candidate.created_at DESC,candidate.material_ref DESC \
           LIMIT 1 \
         ) parent ON true \
         ORDER BY source.created_at,source.material_ref LIMIT $1",
    )
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    let mut derived = 0;
    for row in rows {
        derived += persist_derivation(database, &row).await?;
    }
    Ok(derived)
}

/// Saving a policy is the only authorization boundary for a user-initiated run. Starting an
/// individual run below freezes a manifest and queues work; it intentionally starts no model call.
pub async fn save_active_policy(
    database: &Database,
    request: SaveResearchPolicy,
) -> Result<ResearchPolicyReceipt, CommentResearchKernelError> {
    if !(1..=MAX_DERIVATIONS_PER_PASS as i32).contains(&request.source_limit)
        || !(1024..=10_000_000).contains(&request.token_limit)
    {
        return Err(CommentResearchKernelError::InvalidPolicy);
    }
    let policy_revision_ref = Uuid::new_v4();
    let extraction_rule_hash = comment_source_hash(EXTRACTION_CONTRACT);
    let membership_policy_hash = comment_source_hash(MEMBERSHIP_CONTRACT);
    let mut transaction = database.pool().begin().await?;
    sqlx::query(
        "INSERT INTO linggan_comment_research_policy_revision( \
             policy_revision_ref,config_ref,contract_version,derivation_version,extraction_rule_hash, \
             membership_policy_hash,source_limit,token_limit \
         ) VALUES($1,$2,'comment-research.semantic.v1',$3,$4,$5,$6,$7)",
    )
    .bind(policy_revision_ref)
    .bind(request.config_ref)
    .bind(DERIVATION_VERSION)
    .bind(extraction_rule_hash)
    .bind(membership_policy_hash)
    .bind(request.source_limit)
    .bind(request.token_limit)
    .execute(&mut *transaction)
    .await?;
    let active_revision: i64 = sqlx::query_scalar(
        "UPDATE linggan_comment_research_policy_active \
         SET policy_revision_ref=$1,revision=revision+1,updated_at=scope_001_now() \
         WHERE singleton RETURNING revision",
    )
    .bind(policy_revision_ref)
    .fetch_one(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(ResearchPolicyReceipt {
        policy_revision_ref,
        active_revision,
    })
}

/// Creates a frozen eligible-source manifest using the previously saved policy. It neither asks
/// for a second authorization nor calls an external provider. The worker will advance this queue.
pub async fn start_run(
    database: &Database,
    scope: Value,
) -> Result<ResearchRunReceipt, CommentResearchKernelError> {
    if !scope.is_object() {
        return Err(CommentResearchKernelError::InvalidScope);
    }
    derive_current_sources(database, MAX_DERIVATIONS_PER_PASS as usize).await?;
    let mut transaction = database.pool().begin().await?;
    let policy = sqlx::query(
        "SELECT policy.policy_revision_ref,policy.derivation_version,policy.source_limit \
         FROM linggan_comment_research_policy_active active \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=active.policy_revision_ref \
         WHERE active.singleton FOR SHARE OF active",
    )
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(CommentResearchKernelError::PolicyMissing)?;
    let selected = select_eligible_derivations(&mut transaction, &policy).await?;
    if selected.is_empty() {
        return Err(CommentResearchKernelError::NoEligibleDerivations);
    }
    let run_ref = Uuid::new_v4();
    let manifest_hash = manifest_hash(&selected);
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *transaction)
        .await?;
    let policy_revision_ref: Uuid = policy.get("policy_revision_ref");
    let exclusion_counts = json!({"eligible":selected.len(),"selected":selected.len()});
    sqlx::query(
        "INSERT INTO linggan_comment_research_run( \
             run_ref,policy_revision_ref,state,as_of,scope,manifest_hash,exclusion_counts \
         ) VALUES($1,$2,'queued',$3::timestamptz,$4,$5,$6)",
    )
    .bind(run_ref)
    .bind(policy_revision_ref)
    .bind(&as_of)
    .bind(scope)
    .bind(&manifest_hash)
    .bind(exclusion_counts)
    .execute(&mut *transaction)
    .await?;
    for source in &selected {
        sqlx::query(
            "INSERT INTO linggan_comment_research_run_item( \
                 run_ref,derivation_ref,input_hash,context_hash \
             ) VALUES($1,$2,$3,$4)",
        )
        .bind(run_ref)
        .bind(source.derivation_ref)
        .bind(source.input_hash())
        .bind(source.context_hash())
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(ResearchRunReceipt {
        run_ref,
        policy_revision_ref,
        selected_sources: selected.len(),
        external_calls_started: 0,
    })
}

struct EligibleDerivation {
    derivation_ref: Uuid,
    source_sha256: String,
    research_sha256: String,
    research_text: String,
    context_manifest: Value,
}

impl EligibleDerivation {
    fn input_hash(&self) -> String {
        comment_source_hash(&format!(
            "{}\n{}\n{}",
            self.source_sha256, self.research_sha256, self.research_text
        ))
    }

    fn context_hash(&self) -> String {
        comment_source_hash(&self.context_manifest.to_string())
    }
}

async fn select_eligible_derivations(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy: &sqlx::postgres::PgRow,
) -> Result<Vec<EligibleDerivation>, CommentResearchKernelError> {
    let rows = sqlx::query(
        "SELECT derivation_ref,source_sha256,research_sha256,research_text,context_manifest \
         FROM linggan_comment_research_derivation_readable \
         WHERE derivation_version=$1 AND eligibility='eligible' \
         ORDER BY created_at,derivation_ref LIMIT $2",
    )
    .bind(policy.get::<String, _>("derivation_version"))
    .bind(policy.get::<i32, _>("source_limit"))
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| EligibleDerivation {
            derivation_ref: row.get("derivation_ref"),
            source_sha256: row.get("source_sha256"),
            research_sha256: row.get("research_sha256"),
            research_text: row.get("research_text"),
            context_manifest: row.get("context_manifest"),
        })
        .collect())
}

fn manifest_hash(sources: &[EligibleDerivation]) -> String {
    let manifest = sources
        .iter()
        .map(|source| {
            format!(
                "{}:{}:{}",
                source.derivation_ref, source.source_sha256, source.research_sha256
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    comment_source_hash(&manifest)
}

/// Claims a single new-kernel item. Legacy recovery tables are deliberately absent from this
/// query, so a historical incompatible record cannot head-of-line block a healthy V1 item.
pub async fn claim_next_run_item(
    database: &Database,
) -> Result<Option<ResearchRunItemClaim>, CommentResearchKernelError> {
    let mut transaction = database.pool().begin().await?;
    let row = sqlx::query(
        "WITH candidate AS ( \
             SELECT item.run_ref,item.derivation_ref \
             FROM linggan_comment_research_run_item item \
             JOIN linggan_comment_research_run run USING(run_ref) \
             JOIN linggan_comment_research_derivation_readable derivation \
               ON derivation.derivation_ref=item.derivation_ref \
             WHERE run.state IN ('queued','running') \
               AND (item.state='pending' OR (item.state='retryable' AND item.next_attempt_at<=scope_001_now())) \
             ORDER BY run.created_at,item.created_at,item.derivation_ref \
             LIMIT 1 FOR UPDATE OF item SKIP LOCKED \
         ), claimed AS ( \
             UPDATE linggan_comment_research_run_item item \
             SET state='running',attempts=attempts+1,next_attempt_at=NULL,updated_at=scope_001_now() \
             FROM candidate \
             WHERE item.run_ref=candidate.run_ref AND item.derivation_ref=candidate.derivation_ref \
             RETURNING item.run_ref,item.derivation_ref,item.attempts \
         ) \
         UPDATE linggan_comment_research_run run SET state='running',updated_at=scope_001_now() \
         FROM claimed WHERE run.run_ref=claimed.run_ref \
         RETURNING claimed.run_ref,claimed.derivation_ref,claimed.attempts",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(row.map(|row| ResearchRunItemClaim {
        run_ref: row.get("run_ref"),
        derivation_ref: row.get("derivation_ref"),
        attempt: row.get("attempts"),
    }))
}

/// Records one item outcome without changing any other queue member. A retry is bounded by the
/// schema attempt limit; permanent incompatibility is terminal and immediately lets the worker
/// claim the next healthy item.
pub async fn record_run_item_failure(
    database: &Database,
    claim: &ResearchRunItemClaim,
    class: RunItemFailureClass,
    failure_code: &str,
) -> Result<(), CommentResearchKernelError> {
    let mut transaction = database.pool().begin().await?;
    let terminal = class.terminal_state();
    let changed = if let Some(state) = terminal {
        sqlx::query(
            "UPDATE linggan_comment_research_run_item \
             SET state=$3,failure_code=$4,finished_at=scope_001_now(),updated_at=scope_001_now() \
             WHERE run_ref=$1 AND derivation_ref=$2 AND state='running'",
        )
        .bind(claim.run_ref)
        .bind(claim.derivation_ref)
        .bind(state)
        .bind(failure_code)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
    } else {
        sqlx::query(
            "UPDATE linggan_comment_research_run_item \
             SET state=CASE WHEN attempts>=3 THEN 'model_failed' ELSE 'retryable' END, \
                 failure_code=$3, \
                 next_attempt_at=CASE WHEN attempts>=3 THEN NULL ELSE scope_001_now()+interval '60 seconds' END, \
                 finished_at=CASE WHEN attempts>=3 THEN scope_001_now() ELSE NULL END, \
                 updated_at=scope_001_now() \
             WHERE run_ref=$1 AND derivation_ref=$2 AND state='running'",
        )
        .bind(claim.run_ref)
        .bind(claim.derivation_ref)
        .bind(failure_code)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
    };
    if changed == 1 {
        refresh_run_completion(&mut transaction, claim.run_ref).await?;
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) async fn refresh_run_completion(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
) -> Result<(), CommentResearchKernelError> {
    let outcome: (bool, bool) = sqlx::query_as(
        "SELECT bool_and(state IN ('succeeded','no_signal','incompatible','unrecoverable','model_failed','restricted','cancelled')), \
                bool_or(state IN ('incompatible','unrecoverable','model_failed','restricted')) \
         FROM linggan_comment_research_run_item WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(&mut **transaction)
    .await?;
    if outcome.0 {
        let state = if outcome.1 {
            "completed_with_failures"
        } else {
            "completed"
        };
        sqlx::query(
            "UPDATE linggan_comment_research_run \
             SET state=$2,finished_at=scope_001_now(),updated_at=scope_001_now() \
             WHERE run_ref=$1 AND state IN ('queued','running')",
        )
        .bind(run_ref)
        .bind(state)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn persist_derivation(
    database: &Database,
    row: &sqlx::postgres::PgRow,
) -> Result<u64, CommentResearchKernelError> {
    let source_ref: Uuid = row.get("material_ref");
    let content_ref: Uuid = row.get("content_public_ref");
    let raw: Option<String> = row.get("body_text");
    let author_role = classify_author_role(
        row.get::<Option<String>, _>("author_external_id")
            .as_deref(),
        row.get::<Option<String>, _>("content_author_external_id")
            .as_deref(),
    );
    let research = raw
        .as_deref()
        .map(|body| derive_research_text(body, author_role))
        .unwrap_or_else(empty_research_text);
    let eligibility = classify_eligibility(raw.is_some(), &research, author_role);
    let context_manifest = context_manifest(row, content_ref);
    let offsets = serde_json::to_value(&research.offsets)
        .map_err(|_| CommentResearchKernelError::Serialization)?;
    let reasons = serde_json::to_value(&research.reasons)
        .map_err(|_| CommentResearchKernelError::Serialization)?;
    let raw_hash = comment_source_hash(raw.as_deref().unwrap_or_default());
    let research_hash = comment_source_hash(&research.text);
    let inserted = sqlx::query(
        "INSERT INTO linggan_comment_research_derivation( \
             derivation_ref,source_ref,derivation_version,source_sha256,cleaner_version,clean_state, \
             research_text,research_sha256,research_offsets,normalization_reasons,author_role, \
             attribution_source,attribution_observed_at,eligibility,eligibility_reason,context_manifest \
         ) VALUES( \
             $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13::timestamptz,$14,$15,$16 \
         ) ON CONFLICT(source_ref,derivation_version,source_sha256) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(source_ref)
    .bind(DERIVATION_VERSION)
    .bind(raw_hash)
    .bind(CLEANER_VERSION)
    .bind(&research.clean_state)
    .bind(&research.text)
    .bind(research_hash)
    .bind(offsets)
    .bind(reasons)
    .bind(author_role.as_db())
    .bind(row.get::<Option<String>, _>("attribution_source"))
    .bind(row.get::<Option<String>, _>("attribution_observed_at"))
    .bind(eligibility.as_db())
    .bind(eligibility.reason())
    .bind(context_manifest)
    .execute(database.pool())
    .await?
    .rows_affected();
    Ok(inserted)
}

fn empty_research_text() -> ResearchText {
    ResearchText {
        text: String::new(),
        offsets: Vec::new(),
        clean_state: "anomaly".into(),
        reasons: vec!["missing_body".into()],
    }
}

fn classify_eligibility(
    body_known: bool,
    research: &ResearchText,
    author_role: CommentAuthorRole,
) -> DerivationEligibility {
    if !body_known {
        return DerivationEligibility::SourceBodyUnknown;
    }
    match author_role {
        CommentAuthorRole::ContentAuthorReply => DerivationEligibility::ExcludedAuthorReply,
        CommentAuthorRole::AuthorIdentityUnknown => DerivationEligibility::AuthorIdentityUnknown,
        CommentAuthorRole::OrdinaryUser
            if matches!(research.clean_state.as_str(), "direct" | "context") =>
        {
            DerivationEligibility::Eligible
        }
        CommentAuthorRole::OrdinaryUser => DerivationEligibility::DroppedOrAnomalous,
    }
}

fn context_manifest(row: &sqlx::postgres::PgRow, content_ref: Uuid) -> Value {
    let parent_ref: Option<Uuid> = row.get("parent_source_ref");
    let parent_hash = row
        .get::<Option<String>, _>("parent_body_text")
        .map(|body| comment_source_hash(&body));
    json!({
        "workRef":content_ref,
        "parentSourceRef":parent_ref,
        "parentSourceSha256":parent_hash,
        "parentRequested":row.get::<Option<String>, _>("parent_comment_external_id").is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_comparison_never_guesses_from_a_display_name() {
        assert_eq!(
            classify_author_role(Some("content-author"), Some("content-author")),
            CommentAuthorRole::ContentAuthorReply
        );
        assert_eq!(
            classify_author_role(Some("reader"), Some("content-author")),
            CommentAuthorRole::OrdinaryUser
        );
        assert_eq!(
            classify_author_role(Some("reader"), None),
            CommentAuthorRole::AuthorIdentityUnknown
        );
    }

    #[test]
    fn author_badge_is_removed_only_for_a_confirmed_content_author_reply() {
        let reply =
            derive_research_text("作者 别再瞎干预啦", CommentAuthorRole::ContentAuthorReply);
        assert_eq!(reply.text, "别再瞎干预啦");
        assert_eq!(reply.offsets[0], (3, 4));
        assert!(
            reply
                .reasons
                .contains(&"content_author_badge_removed".to_owned())
        );

        let reader = derive_research_text("作者 推荐的资料我看了", CommentAuthorRole::OrdinaryUser);
        assert_eq!(reader.text, "作者 推荐的资料我看了");
        assert!(
            !reader
                .reasons
                .contains(&"content_author_badge_removed".to_owned())
        );
    }

    #[test]
    fn only_readable_ordinary_user_text_can_be_eligible() {
        let direct = derive_research_text("孩子写作业时总是拖延", CommentAuthorRole::OrdinaryUser);
        assert_eq!(
            classify_eligibility(true, &direct, CommentAuthorRole::OrdinaryUser),
            DerivationEligibility::Eligible
        );
        assert_eq!(
            classify_eligibility(true, &direct, CommentAuthorRole::ContentAuthorReply),
            DerivationEligibility::ExcludedAuthorReply
        );
        assert_eq!(
            classify_eligibility(true, &direct, CommentAuthorRole::AuthorIdentityUnknown),
            DerivationEligibility::AuthorIdentityUnknown
        );
    }
}
