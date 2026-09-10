//! COMMENT-RESEARCH-RESET-001 input boundary.
//!
//! This module has one responsibility: turn a current, readable comment fact into a versioned
//! research derivation. It never mutates the raw comment, calls a model, or decides a Problem.

use crate::{
    comment_cleaning::{CLEANER_VERSION, CleanComment, clean},
    embedding_settings,
    research_text::content_hash,
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
    #[error("no eligible ordinary-user derivations are available")]
    NoEligibleDerivations,
    #[error("no enabled, qualified embedding configuration is available")]
    EmbeddingNotReady,
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

/// The worker may read this immutable execution input after it has claimed an Item.  Keeping the
/// source text here means the transport layer never needs to discover a second, legacy comment
/// projection just to construct a prompt.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimedResearchInput {
    pub run_ref: Uuid,
    pub derivation_ref: Uuid,
    pub attempt: i32,
    pub research_text: String,
    pub context_manifest: Value,
    pub config_ref: Option<Uuid>,
    pub token_limit: i64,
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

/// Derives up to `limit` changed current sources. Re-running is safe: the immutable identity
/// includes raw text, role attribution and reply context, so a late author-attribution fact
/// creates a new derivation rather than preserving stale research eligibility.
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
         WHERE NOT EXISTS ( \
           SELECT 1 FROM linggan_comment_research_derivation existing \
           WHERE existing.source_ref=source.material_ref \
             AND existing.derivation_version=$1 \
             AND existing.source_sha256=encode(sha256(convert_to(COALESCE(source.body_text,''),'UTF8')),'hex') \
             AND existing.author_role=CASE \
               WHEN NULLIF(btrim(source.author_external_id),'') IS NULL \
                 OR NULLIF(btrim(attribution.author_external_id),'') IS NULL THEN 'author_identity_unknown' \
               WHEN NULLIF(btrim(source.author_external_id),'')=NULLIF(btrim(attribution.author_external_id),'') THEN 'content_author_reply' \
               ELSE 'ordinary_user' END \
             AND existing.attribution_source IS NOT DISTINCT FROM attribution.attribution_source \
             AND existing.attribution_observed_at IS NOT DISTINCT FROM attribution.observed_at \
             AND existing.context_manifest=jsonb_build_object( \
               'workRef',source.content_public_ref, \
               'parentSourceRef',parent.material_ref, \
               'parentSourceSha256',CASE WHEN parent.body_text IS NULL THEN NULL ELSE encode(sha256(convert_to(parent.body_text,'UTF8')),'hex') END, \
               'parentRequested',source.parent_comment_external_id IS NOT NULL \
             ) \
         ) \
         ORDER BY source.created_at DESC,source.material_ref DESC LIMIT $2",
    )
    .bind(DERIVATION_VERSION)
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
    let extraction_rule_hash = content_hash(EXTRACTION_CONTRACT);
    let membership_policy_hash = content_hash(MEMBERSHIP_CONTRACT);
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
) -> Result<ResearchRunReceipt, CommentResearchKernelError> {
    derive_current_sources(database, MAX_DERIVATIONS_PER_PASS as usize).await?;
    let mut transaction = database.pool().begin().await?;
    if !embedding_settings::ready_in_transaction(&mut transaction, None).await? {
        return Err(CommentResearchKernelError::EmbeddingNotReady);
    }
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
    .bind(json!({
        "kind":"all_current_readable_ordinary_user_comments",
        "selection":"unprocessed_derivation_created_desc"
    }))
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
        content_hash(&format!(
            "{}\n{}\n{}",
            self.source_sha256, self.research_sha256, self.research_text
        ))
    }

    fn context_hash(&self) -> String {
        content_hash(&self.context_manifest.to_string())
    }
}

async fn select_eligible_derivations(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy: &sqlx::postgres::PgRow,
) -> Result<Vec<EligibleDerivation>, CommentResearchKernelError> {
    let rows = sqlx::query(
        "SELECT derivation_ref,source_sha256,research_sha256,research_text,context_manifest \
         FROM linggan_comment_research_derivation_current derivation \
         WHERE derivation_version=$1 AND eligibility='eligible' \
           AND NOT EXISTS ( \
             SELECT 1 FROM linggan_comment_research_run_item used \
             WHERE used.derivation_ref=derivation.derivation_ref \
           ) \
         ORDER BY created_at DESC,derivation_ref DESC LIMIT $2",
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
    content_hash(&manifest)
}

/// Claims a single new-kernel item. Legacy recovery tables are deliberately absent from this
/// query, so a historical incompatible record cannot head-of-line block a healthy V1 item.
pub async fn claim_next_run_item(
    database: &Database,
) -> Result<Option<ResearchRunItemClaim>, CommentResearchKernelError> {
    let mut transaction = database.pool().begin().await?;
    recover_expired_run_items_in(&mut transaction).await?;
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
             SET state='running',attempts=attempts+1,next_attempt_at=NULL, \
                 lease_until=scope_001_now()+interval '120 seconds',updated_at=scope_001_now() \
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

/// Returns only the frozen material belonging to a current V1 claim.  A missing row is a normal
/// lease-loss outcome: another recovery path may have settled the Item while the caller waited.
pub async fn load_claimed_research_input(
    database: &Database,
    claim: &ResearchRunItemClaim,
) -> Result<Option<ClaimedResearchInput>, CommentResearchKernelError> {
    let row = sqlx::query(
        "SELECT derivation.research_text,derivation.context_manifest,policy.config_ref,policy.token_limit \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_run run USING(run_ref) \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=item.derivation_ref \
         WHERE item.run_ref=$1 AND item.derivation_ref=$2 AND item.state='running'",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| ClaimedResearchInput {
        run_ref: claim.run_ref,
        derivation_ref: claim.derivation_ref,
        attempt: claim.attempt,
        research_text: row.get("research_text"),
        context_manifest: row.get("context_manifest"),
        config_ref: row.get("config_ref"),
        token_limit: row.get("token_limit"),
    }))
}

/// Reclaims only expired V1 execution leases.  Historical recovery records and legacy queues are
/// intentionally absent: a poisoned old task can never become a head-of-line blocker here.
pub async fn recover_expired_run_items(
    database: &Database,
) -> Result<u64, CommentResearchKernelError> {
    let mut transaction = database.pool().begin().await?;
    let recovered = recover_expired_run_items_in(&mut transaction).await?;
    transaction.commit().await?;
    Ok(recovered)
}

async fn recover_expired_run_items_in(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<u64, CommentResearchKernelError> {
    let rows = sqlx::query(
        "UPDATE linggan_comment_research_run_item \
         SET state=CASE WHEN attempts>=3 THEN 'model_failed' ELSE 'retryable' END, \
             failure_code='worker_interrupted', \
             next_attempt_at=CASE WHEN attempts>=3 THEN NULL ELSE scope_001_now()+interval '60 seconds' END, \
             finished_at=CASE WHEN attempts>=3 THEN scope_001_now() ELSE NULL END, \
             lease_until=NULL,updated_at=scope_001_now() \
         WHERE state='running' AND lease_until<=scope_001_now() \
         RETURNING run_ref,invocation_ref",
    )
    .fetch_all(&mut **transaction)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let invocations: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.get::<Option<Uuid>, _>("invocation_ref"))
        .collect();
    if !invocations.is_empty() {
        sqlx::query(
            "UPDATE linggan_model_invocation \
             SET state='failed',failure_code='worker_interrupted',finished_at=scope_001_now(), \
                 result=COALESCE(result,'{}'::jsonb)||jsonb_build_object( \
                     'callStarted',true,'usageUnknown',input_tokens IS NULL OR output_tokens IS NULL,'recovered',true \
                 ) \
             WHERE invocation_ref=ANY($1) AND state='running'",
        )
        .bind(&invocations)
        .execute(&mut **transaction)
        .await?;
    }
    let run_refs: std::collections::BTreeSet<Uuid> =
        rows.iter().map(|row| row.get("run_ref")).collect();
    for run_ref in run_refs {
        refresh_run_completion(transaction, run_ref).await?;
    }
    Ok(rows.len() as u64)
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
             SET state=$3,failure_code=$4,finished_at=scope_001_now(),lease_until=NULL,updated_at=scope_001_now() \
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
                 lease_until=NULL, \
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
    refresh_run_completion_with_embedding_state(transaction, run_ref, false).await
}

async fn refresh_run_completion_with_embedding_state(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    embedding_unavailable_is_terminal: bool,
) -> Result<(), CommentResearchKernelError> {
    let outcome = sqlx::query(
        "SELECT \
            NOT EXISTS( \
                SELECT 1 FROM linggan_comment_research_run_item \
                WHERE run_ref=$1 \
                  AND state NOT IN ('succeeded','no_signal','incompatible','unrecoverable','model_failed','restricted','cancelled') \
            ) AS items_terminal, \
            (SELECT count(*) FROM linggan_comment_research_run_item \
             WHERE run_ref=$1 \
               AND state IN ('incompatible','unrecoverable','model_failed','restricted')) AS item_failed_count, \
            (SELECT count(*) FROM linggan_comment_research_atom atom \
             WHERE atom.run_ref=$1 AND atom.kind IN ('problem','need') \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                              WHERE membership.atom_ref=atom.atom_ref AND membership.current)) AS unassigned_atoms, \
            (SELECT count(*) FROM linggan_comment_research_atom atom \
             WHERE atom.run_ref=$1 AND atom.kind IN ('problem','need') \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                              WHERE membership.atom_ref=atom.atom_ref AND membership.current) \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                              WHERE embedding.atom_ref=atom.atom_ref AND embedding.state='succeeded') \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                              WHERE embedding.atom_ref=atom.atom_ref AND embedding.state IN ('pending','running')) \
               AND (EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                           WHERE embedding.atom_ref=atom.atom_ref AND embedding.state IN ('failed','incompatible')) \
                    OR ($2 AND NOT EXISTS(SELECT 1 FROM linggan_embedding_settings settings \
                                  JOIN linggan_embedding_config config USING(config_ref) \
                                  JOIN linggan_model_entry model USING(model_ref) \
                                  JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
                                  JOIN linggan_model_connection connection USING(connection_ref) \
                                  WHERE settings.singleton AND config.enabled AND config.qualified AND connection.enabled))) \
            ) \
            + (SELECT count(*) FROM linggan_comment_research_atom atom \
               WHERE atom.run_ref=$1 AND atom.kind IN ('problem','need') \
                 AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                                WHERE membership.atom_ref=atom.atom_ref AND membership.current) \
                 AND $2 \
                 AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_problem_resolution resolution \
                                WHERE resolution.atom_ref=atom.atom_ref) \
                 AND EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                            WHERE embedding.atom_ref=atom.atom_ref AND embedding.state='succeeded') \
                 AND EXISTS( \
                     SELECT 1 FROM linggan_comment_research_problem_definition definition \
                     JOIN linggan_comment_research_problem problem USING(problem_ref) \
                     WHERE problem.state='active' \
                       AND EXISTS( \
                           SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                           JOIN linggan_comment_research_atom member_atom USING(atom_ref) \
                           JOIN linggan_comment_research_derivation_readable member_derivation \
                             ON member_derivation.derivation_ref=member_atom.derivation_ref \
                           WHERE membership.problem_ref=definition.problem_ref \
                             AND membership.definition_revision=definition.revision AND membership.current \
                       ) \
                       AND NOT EXISTS( \
                           SELECT 1 FROM linggan_comment_research_problem_definition_embedding definition_embedding \
                           JOIN linggan_comment_research_atom_embedding atom_embedding \
                             ON atom_embedding.atom_ref=atom.atom_ref AND atom_embedding.state='succeeded' \
                           WHERE definition_embedding.problem_ref=definition.problem_ref \
                             AND definition_embedding.definition_revision=definition.revision \
                             AND definition_embedding.space_ref=atom_embedding.space_ref \
                             AND definition_embedding.state='succeeded' \
                       ) \
                 ) \
            ) AS embedding_failed_atoms, \
            (SELECT count(*) FROM linggan_comment_research_atom atom \
             WHERE atom.run_ref=$1 AND atom.kind IN ('problem','need') \
               AND EXISTS(SELECT 1 FROM linggan_comment_research_problem_resolution resolution \
                          WHERE resolution.atom_ref=atom.atom_ref \
                            AND resolution.state IN ('model_failed','incompatible')) \
            ) AS resolution_failed_atoms, \
            (SELECT count(*) FROM linggan_comment_research_atom atom \
             WHERE atom.run_ref=$1 AND atom.kind IN ('problem','need') \
               AND EXISTS(SELECT 1 FROM linggan_comment_research_problem_resolution resolution \
                          WHERE resolution.atom_ref=atom.atom_ref \
                            AND resolution.state IN ('pending','running','retryable')) \
            ) AS unsettled_resolution_atoms",
    )
    .bind(run_ref)
    .bind(embedding_unavailable_is_terminal)
    .fetch_one(&mut **transaction)
    .await?;
    settle_run_completion(transaction, run_ref, &outcome).await
}

async fn settle_run_completion(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    run_ref: Uuid,
    outcome: &sqlx::postgres::PgRow,
) -> Result<(), CommentResearchKernelError> {
    let items_terminal: bool = outcome.get("items_terminal");
    if !items_terminal {
        return Ok(());
    }
    let item_failed_count: i64 = outcome.get("item_failed_count");
    let unassigned_atoms: i64 = outcome.get("unassigned_atoms");
    let embedding_failed_atoms: i64 = outcome.get("embedding_failed_atoms");
    let resolution_failed_atoms: i64 = outcome.get("resolution_failed_atoms");
    let unsettled_resolution_atoms: i64 = outcome.get("unsettled_resolution_atoms");
    if unsettled_resolution_atoms > 0 {
        return Ok(());
    }
    let terminal_unassigned = embedding_failed_atoms + resolution_failed_atoms;
    if unassigned_atoms > 0 && terminal_unassigned < unassigned_atoms {
        return Ok(());
    }
    let state = if item_failed_count > 0 || terminal_unassigned > 0 {
        "completed_with_failures"
    } else {
        "completed"
    };
    sqlx::query(
        "UPDATE linggan_comment_research_run \
         SET state=$2,failure_counts=jsonb_strip_nulls(jsonb_build_object( \
               'runItems',NULLIF($3,0), \
               'embedding',NULLIF($4,0), \
               'problemResolution',NULLIF($5,0) \
             )),finished_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE run_ref=$1 AND state IN ('queued','running')",
    )
    .bind(run_ref)
    .bind(state)
    .bind(item_failed_count)
    .bind(embedding_failed_atoms)
    .bind(resolution_failed_atoms)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Re-evaluates the single Run whose problem-bearing Atom has just advanced.  Semantic item
/// completion alone never publishes a result: vector and problem-resolution terminal states
/// participate in this exact same completion decision.
pub async fn refresh_run_completion_for_atom(
    database: &Database,
    atom_ref: Uuid,
) -> Result<(), CommentResearchKernelError> {
    let run_ref: Option<Uuid> =
        sqlx::query_scalar("SELECT run_ref FROM linggan_comment_research_atom WHERE atom_ref=$1")
            .bind(atom_ref)
            .fetch_optional(database.pool())
            .await?;
    let Some(run_ref) = run_ref else {
        return Ok(());
    };
    let mut transaction = database.pool().begin().await?;
    refresh_run_completion(&mut transaction, run_ref).await?;
    transaction.commit().await?;
    Ok(())
}

/// A disabled embedding configuration terminalizes work that has not reserved a provider call.
/// Calls already in flight settle through their normal receipts; this function never creates a
/// new invocation merely to make an active Run finish.
pub async fn fail_active_runs_without_embedding_config(
    database: &Database,
) -> Result<u64, CommentResearchKernelError> {
    sqlx::query(
        "UPDATE linggan_comment_research_run_item item \
         SET state='incompatible',failure_code='embedding_configuration_unavailable', \
             finished_at=scope_001_now(),lease_until=NULL,next_attempt_at=NULL,updated_at=scope_001_now() \
         FROM linggan_comment_research_run run \
         WHERE item.run_ref=run.run_ref AND item.state IN ('pending','retryable') \
           AND run.state IN ('queued','running')",
    )
    .execute(database.pool())
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_research_atom_embedding embedding \
         SET state='failed',failure_code='embedding_configuration_unavailable',updated_at=scope_001_now() \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         WHERE embedding.atom_ref=atom.atom_ref AND embedding.state='pending' \
           AND run.state IN ('queued','running')",
    )
    .execute(database.pool())
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution resolution \
         SET state='incompatible',failure_code='embedding_configuration_unavailable', \
             finished_at=scope_001_now(),lease_until=NULL,next_attempt_at=NULL,updated_at=scope_001_now() \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         WHERE resolution.atom_ref=atom.atom_ref AND resolution.state IN ('pending','retryable') \
           AND run.state IN ('queued','running')",
    )
    .execute(database.pool())
    .await?;
    let run_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT run_ref FROM linggan_comment_research_run \
         WHERE state IN ('queued','running') ORDER BY created_at,run_ref",
    )
    .fetch_all(database.pool())
    .await?;
    let mut refreshed = 0;
    for run_ref in run_refs {
        let mut transaction = database.pool().begin().await?;
        refresh_run_completion_with_embedding_state(&mut transaction, run_ref, true).await?;
        transaction.commit().await?;
        refreshed += 1;
    }
    Ok(refreshed)
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
    let raw_hash = content_hash(raw.as_deref().unwrap_or_default());
    let research_hash = content_hash(&research.text);
    let derivation_input_hash = content_hash(
        &json!({
            "sourceRef":source_ref,
            "derivationVersion":DERIVATION_VERSION,
            "sourceSha256":raw_hash,
            "researchSha256":research_hash,
            "cleanerVersion":CLEANER_VERSION,
            "authorRole":author_role.as_db(),
            "attributionSource":row.get::<Option<String>, _>("attribution_source"),
            "attributionObservedAt":row.get::<Option<String>, _>("attribution_observed_at"),
            "contextManifest":context_manifest.clone(),
        })
        .to_string(),
    );
    let inserted = sqlx::query(
        "INSERT INTO linggan_comment_research_derivation( \
             derivation_ref,source_ref,derivation_version,source_sha256,cleaner_version,clean_state, \
             research_text,research_sha256,derivation_input_hash,research_offsets,normalization_reasons,author_role, \
             attribution_source,attribution_observed_at,eligibility,eligibility_reason,context_manifest \
         ) VALUES( \
             $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14::timestamptz,$15,$16,$17 \
         ) ON CONFLICT(source_ref,derivation_version,derivation_input_hash) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(source_ref)
    .bind(DERIVATION_VERSION)
    .bind(raw_hash)
    .bind(CLEANER_VERSION)
    .bind(&research.clean_state)
    .bind(&research.text)
    .bind(research_hash)
    .bind(derivation_input_hash)
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
        .map(|body| content_hash(&body));
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
