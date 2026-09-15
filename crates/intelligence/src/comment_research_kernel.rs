//! COMMENT-RESEARCH-RESET-001 input boundary.
//!
//! This module has one responsibility: turn a current, readable comment fact into a versioned
//! research derivation. It never mutates the raw comment, calls a model, or decides a Problem.

use crate::{
    comment_cleaning::{CLEANER_VERSION, CleanComment, clean},
    local_embedding_profile, model_settings,
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

/// V2 adds a semantic parent-comment context contract.  V1 derivations remain immutable and
/// readable as historical inputs; they must never be silently executed under the new contract.
pub const DERIVATION_VERSION: &str = "comment-research.derivation.v2";
const CONTEXT_MANIFEST_CONTRACT: &str = "comment-research.context.v2";
const MAX_DERIVATIONS_PER_PASS: i64 = 3000;
const MAX_DERIVATION_PREWARM_PASSES: usize = 100;
/// Backlog is deliberately bounded separately from newly frozen comments. A later Run may
/// continue eligible historical resolution work, but it cannot turn an old error into an
/// unbounded provider drain.
const MAX_BACKLOG_RESOLUTIONS_PER_RUN: i32 = 40;
/// V6 makes the provider JSON Schema an exact mirror of the Rust tagged variants.  The two
/// hashes below enter the saved policy and research fingerprint, so an older packet cannot be
/// reused after this branch contract changes.  `contract_version` remains the stable database
/// contract family; it is not an output-packet revision field.
const EXTRACTION_CONTRACT: &str = "comment-research.semantic.v7/extract:problem,need,belief,emotion,experience,solution,quote,context,question;problem-frame:evidence-grounded;scope:policy-frozen;evidence:exact-source-quote;output:exact-json-or-single-json-fence;examples:required;variants:exclusive-required";
const MEMBERSHIP_CONTRACT: &str = "comment-research.semantic.v7/membership:retrieval-only-before-decision;candidate-indices:closed-world;unknown:not-false;create:independent-pair-only;output:exact-json-or-single-json-fence;examples:required;variants:exclusive-required";
const PROBLEM_RESOLUTION_CONTRACT: &str = "comment-research.problem-resolution.v2";
const ADHD_DOMAIN_REF: &str = "00000000-0000-4000-8000-000000000001";

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
    #[error(
        "the saved comment research policy uses an older input contract and must be saved again"
    )]
    PolicyInputContractStale,
    #[error("no eligible ordinary-user derivations are available")]
    NoEligibleDerivations,
    #[error("current comment derivations did not settle within the bounded prewarm")]
    DerivationPrewarmIncomplete,
    #[error("no enabled, qualified embedding configuration is available")]
    EmbeddingNotReady,
    #[error("the selected research model has not passed the V1 semantic probe")]
    ModelNotReady,
    #[error(
        "comment research reset is blocked while {active_operations} V1 model operation(s) are active"
    )]
    DevelopmentResetBlocked { active_operations: i64 },
    #[error("the run item claim is no longer current")]
    ClaimLost,
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
    pub selected_backlog_atoms: usize,
    pub external_calls_started: usize,
}

/// A server-computed explanation of what a subsequent `start_run` may freeze.
///
/// This is deliberately a summary rather than a manifest: the browser never receives source
/// identifiers, source text, or a client-controlled selection. `start_run` still derives and
/// selects again in its own transaction, so a confirmation cannot turn an old preview into a
/// stale or hand-picked Run.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunPreview {
    pub policy_configured: bool,
    pub source_limit: Option<i32>,
    pub eligible_sources: usize,
    pub unprocessed_sources: usize,
    pub recoverable_sources: usize,
    pub selected_sources: usize,
    pub eligible_backlog_atoms: usize,
    pub selected_backlog_atoms: usize,
    pub selected_context_sources: usize,
    pub selected_missing_parent_context_sources: usize,
    pub succeeded_sources: usize,
    pub no_signal_sources: usize,
    pub active_sources: usize,
    pub retryable_sources: usize,
    pub terminal_item_states: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevelopmentResetReceipt {
    pub deleted_derivations: u64,
    pub deleted_runs: u64,
    pub deleted_run_items: u64,
    pub deleted_atoms: u64,
    pub deleted_problems: u64,
    pub deleted_results: u64,
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
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimedResearchInput {
    pub run_ref: Uuid,
    pub derivation_ref: Uuid,
    pub attempt: i32,
    pub research_text: String,
    pub clean_state: String,
    pub parent_context: ParentResearchContext,
    pub config_ref: Option<Uuid>,
    pub token_limit: i64,
    pub problem_scope_definition: Option<Value>,
}

/// The only reply context that a semantic model may receive.  Source references and hashes are
/// intentionally not part of this value: they are retained in the derivation audit manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentResearchContext {
    NotRequested,
    Available { research_text: String },
    Unavailable,
    Invalid,
}

impl ParentResearchContext {
    pub fn research_text(&self) -> Option<&str> {
        match self {
            Self::Available { research_text } => Some(research_text),
            Self::NotRequested | Self::Unavailable | Self::Invalid => None,
        }
    }
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
                parent.material_ref AS parent_source_ref,parent.body_text AS parent_body_text, \
                parent.author_external_id AS parent_author_external_id \
         FROM linggan_comment_research_source_current source \
         LEFT JOIN linggan_material_content_author attribution \
           ON attribution.content_public_ref=source.content_public_ref \
         LEFT JOIN LATERAL ( \
           SELECT candidate.material_ref,candidate.body_text,candidate.author_external_id \
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
             AND existing.context_manifest->>'contract'=$3 \
             AND existing.context_manifest->'audit'=jsonb_build_object( \
               'workRef',source.content_public_ref, \
               'parentSourceRef',parent.material_ref, \
               'parentSourceSha256',CASE WHEN parent.body_text IS NULL THEN NULL ELSE encode(sha256(convert_to(parent.body_text,'UTF8')),'hex') END, \
               'parentRequested',source.parent_comment_external_id IS NOT NULL, \
               'parentAuthorRole',CASE WHEN parent.material_ref IS NULL THEN NULL WHEN NULLIF(btrim(parent.author_external_id),'') IS NULL \
                 OR NULLIF(btrim(attribution.author_external_id),'') IS NULL THEN 'author_identity_unknown' \
                 WHEN NULLIF(btrim(parent.author_external_id),'')=NULLIF(btrim(attribution.author_external_id),'') THEN 'content_author_reply' \
                 ELSE 'ordinary_user' END \
             ) \
         ) \
         ORDER BY source.created_at DESC,source.material_ref DESC LIMIT $2",
    )
    .bind(DERIVATION_VERSION)
    .bind(limit)
    .bind(CONTEXT_MANIFEST_CONTRACT)
    .fetch_all(database.pool())
    .await?;
    let mut derived = 0;
    for row in rows {
        derived += persist_derivation(database, &row).await?;
    }
    Ok(derived)
}

/// Brings the current, readable source projection onto the active derivation contract before a
/// read surface is exposed. It is intentionally bounded so a deployment cannot spin forever if
/// new source material keeps arriving; failure leaves the caller to keep the old surface running.
/// This only persists deterministic derivations. It neither authorizes nor starts model work.
pub async fn prewarm_current_sources(
    database: &Database,
) -> Result<u64, CommentResearchKernelError> {
    let mut derived_total = 0;
    for _ in 0..MAX_DERIVATION_PREWARM_PASSES {
        let derived = derive_current_sources(database, usize::MAX).await?;
        derived_total += derived;
        if derived == 0 {
            return Ok(derived_total);
        }
    }
    Err(CommentResearchKernelError::DerivationPrewarmIncomplete)
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
    let Some(config_ref) = request.config_ref else {
        return Err(CommentResearchKernelError::ModelNotReady);
    };
    let policy_revision_ref = Uuid::new_v4();
    let extraction_rule_hash = content_hash(EXTRACTION_CONTRACT);
    let membership_policy_hash = content_hash(MEMBERSHIP_CONTRACT);
    let scope_definition = problem_scope_definition();
    let scope_hash = content_hash(&scope_definition.to_string());
    let scope_domain_ref =
        Uuid::parse_str(ADHD_DOMAIN_REF).map_err(|_| CommentResearchKernelError::InvalidPolicy)?;
    let mut transaction = database.pool().begin().await?;
    if !research_model_ready_in_transaction(&mut transaction, config_ref).await? {
        return Err(CommentResearchKernelError::ModelNotReady);
    }
    sqlx::query(
        "INSERT INTO linggan_comment_research_policy_revision( \
             policy_revision_ref,config_ref,contract_version,derivation_version,extraction_rule_hash, \
             membership_policy_hash,source_limit,token_limit,problem_resolution_contract, \
             problem_scope_domain_ref,problem_scope_definition,problem_scope_hash \
         ) VALUES($1,$2,'comment-research.semantic.v1',$3,$4,$5,$6,$7,$8,$9,$10,$11)",
    )
    .bind(policy_revision_ref)
    .bind(config_ref)
    .bind(DERIVATION_VERSION)
    .bind(extraction_rule_hash)
    .bind(membership_policy_hash)
    .bind(request.source_limit)
    .bind(request.token_limit)
    .bind(PROBLEM_RESOLUTION_CONTRACT)
    .bind(scope_domain_ref)
    .bind(scope_definition)
    .bind(scope_hash)
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

fn ensure_current_policy_contract(
    policy: &sqlx::postgres::PgRow,
) -> Result<(), CommentResearchKernelError> {
    (policy.get::<String, _>("derivation_version") == DERIVATION_VERSION
        && policy.get::<String, _>("extraction_rule_hash") == content_hash(EXTRACTION_CONTRACT)
        && policy.get::<String, _>("membership_policy_hash") == content_hash(MEMBERSHIP_CONTRACT)
        && policy
            .get::<Option<String>, _>("problem_resolution_contract")
            .as_deref()
            == Some(PROBLEM_RESOLUTION_CONTRACT)
        && policy.get::<Option<Uuid>, _>("problem_scope_domain_ref")
            == Uuid::parse_str(ADHD_DOMAIN_REF).ok()
        && policy
            .get::<Option<Value>, _>("problem_scope_definition")
            .is_some_and(|scope| scope == problem_scope_definition())
        && policy
            .get::<Option<String>, _>("problem_scope_hash")
            .as_deref()
            == Some(&content_hash(&problem_scope_definition().to_string())))
    .then_some(())
    .ok_or(CommentResearchKernelError::PolicyInputContractStale)
}

/// Creates a frozen eligible-source manifest using the previously saved policy. It neither asks
/// for a second authorization nor calls an external provider. The worker will advance this queue.
pub async fn start_run(
    database: &Database,
) -> Result<ResearchRunReceipt, CommentResearchKernelError> {
    derive_current_sources(database, MAX_DERIVATIONS_PER_PASS as usize).await?;
    let mut transaction = database.pool().begin().await?;
    if !local_embedding_profile::ready_in_transaction(&mut transaction).await? {
        return Err(CommentResearchKernelError::EmbeddingNotReady);
    }
    let policy = sqlx::query(
        "SELECT policy.policy_revision_ref,policy.derivation_version,policy.source_limit,policy.config_ref, \
                policy.extraction_rule_hash,policy.membership_policy_hash,policy.problem_resolution_contract, \
                policy.problem_scope_domain_ref,policy.problem_scope_definition,policy.problem_scope_hash \
         FROM linggan_comment_research_policy_active active \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=active.policy_revision_ref \
         WHERE active.singleton FOR SHARE OF active",
    )
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(CommentResearchKernelError::PolicyMissing)?;
    ensure_current_policy_contract(&policy)?;
    let config_ref: Option<Uuid> = policy.get("config_ref");
    let Some(config_ref) = config_ref else {
        return Err(CommentResearchKernelError::ModelNotReady);
    };
    if !research_model_ready_in_transaction(&mut transaction, config_ref).await? {
        return Err(CommentResearchKernelError::ModelNotReady);
    }
    let selected = select_eligible_derivations(&mut transaction, &policy).await?;
    let eligible_backlog = select_eligible_resolution_backlog(&mut transaction, &policy).await?;
    if selected.is_empty() && eligible_backlog.is_empty() {
        return Err(CommentResearchKernelError::NoEligibleDerivations);
    }
    let run_ref = Uuid::new_v4();
    let manifest_hash = manifest_hash(&selected);
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *transaction)
        .await?;
    let policy_revision_ref: Uuid = policy.get("policy_revision_ref");
    let exclusion_counts = json!({
        "eligible":selected.len(),
        "selected":selected.len(),
        "eligibleResolutionBacklog":eligible_backlog.len(),
        "selectedResolutionBacklog":eligible_backlog.len(),
    });
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
        "selection":"new_derivations_and_eligible_resolution_backlog"
    }))
    .bind(&manifest_hash)
    .bind(exclusion_counts)
    .execute(&mut *transaction)
    .await?;
    for source in &selected {
        sqlx::query(
            "INSERT INTO linggan_comment_research_run_item( \
                 run_ref,derivation_ref,input_hash,context_hash,research_fingerprint,attempts \
             ) SELECT $1,$2,$3,$4,$5,COALESCE(( \
                 SELECT max(prior.attempts) FROM linggan_comment_research_run_item prior \
                 WHERE prior.derivation_ref=$2 AND prior.research_fingerprint=$5 \
             ),0)",
        )
        .bind(run_ref)
        .bind(source.derivation_ref)
        .bind(source.input_hash())
        .bind(source.context_hash())
        .bind(source.research_fingerprint(
            policy.get::<String, _>("extraction_rule_hash").as_str(),
            policy.get::<String, _>("membership_policy_hash").as_str(),
            config_ref,
        ))
        .execute(&mut *transaction)
        .await?;
    }
    let selected_backlog_atoms =
        activate_eligible_resolution_backlog(&mut transaction, run_ref, &eligible_backlog).await?;
    // Two starts may observe the same terminal backlog before either one claims it. The UPDATE
    // above is the authority; do not persist a retry-only Run that lost that race and has no
    // frozen source Items to advance.
    if selected.is_empty() && selected_backlog_atoms == 0 {
        transaction.rollback().await?;
        return Err(CommentResearchKernelError::NoEligibleDerivations);
    }
    transaction.commit().await?;
    Ok(ResearchRunReceipt {
        run_ref,
        policy_revision_ref,
        selected_sources: selected.len(),
        selected_backlog_atoms,
        external_calls_started: 0,
    })
}

fn problem_scope_definition() -> Value {
    json!({
        "scopeId": "adhd",
        "scopeVersion": "comment-research.adhd.v1",
        "domainRef": ADHD_DOMAIN_REF,
        "definition": "ADHD 相关用户、照护者或儿童在注意力、冲动、多动、执行功能、学习、家庭支持、服务或信息需求中明确表达的困难或未满足需求。",
        "include": [
            {"id": "adhd_explicit", "rule": "当前评论或可引用上下文明确提及 ADHD、注意缺陷多动或诊断/支持语境。"},
            {"id": "adhd_functional", "rule": "当前评论或可引用上下文把困难明确关联到 ADHD 特征、执行功能或 ADHD 相关支持。"}
        ],
        "exclude": [
            {"id": "generic_without_link", "rule": "没有可引用 ADHD 关联的泛育儿、泛学习、泛睡眠、泛情绪或内容偏好。"},
            {"id": "inferred_identity_or_diagnosis", "rule": "不得仅凭作者、孩子、家长或困难表述推断 ADHD 身份、诊断或亲属关系。"}
        ],
        "uncertain": "缺少会改变 ADHD 范围或稳定问题身份的上下文时，保留为 deferred_context，不得改判为范围外。"
    })
}

/// Reads the current automatic-selection boundary without creating a Run or reserving a provider
/// call. Refreshing derivations is the same local, deterministic preparation that `start_run`
/// performs; it is needed so the preview and the confirmed command reason about the same current
/// evidence boundary.
pub async fn preview_run(
    database: &Database,
) -> Result<ResearchRunPreview, CommentResearchKernelError> {
    derive_current_sources(database, MAX_DERIVATIONS_PER_PASS as usize).await?;
    let mut transaction = database.pool().begin().await?;
    let policy = sqlx::query(
        "SELECT policy.derivation_version,policy.source_limit,policy.config_ref,policy.extraction_rule_hash,policy.membership_policy_hash, \
                policy.problem_resolution_contract,policy.problem_scope_domain_ref,policy.problem_scope_definition,policy.problem_scope_hash \
         FROM linggan_comment_research_policy_active active \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=active.policy_revision_ref \
         WHERE active.singleton FOR SHARE OF active",
    )
    .fetch_optional(&mut *transaction)
    .await?;
    if let Some(policy) = policy.as_ref() {
        ensure_current_policy_contract(policy)?;
    }
    let summary = sqlx::query(
        "WITH current AS ( \
             SELECT derivation.derivation_ref,latest_item.state AS latest_state \
             FROM linggan_comment_research_derivation_current derivation \
             LEFT JOIN LATERAL ( \
                 SELECT item.state \
                 FROM linggan_comment_research_run_item item \
                 WHERE item.derivation_ref=derivation.derivation_ref \
                 ORDER BY item.updated_at DESC,item.run_ref DESC LIMIT 1 \
             ) latest_item ON true \
             WHERE derivation.derivation_version=$1 AND derivation.eligibility='eligible' \
         ) \
         SELECT count(*) AS eligible_sources, \
                count(*) FILTER(WHERE latest_state IS NULL) AS unprocessed_sources, \
                count(*) FILTER(WHERE latest_state='cancelled') AS recoverable_sources, \
                count(*) FILTER(WHERE latest_state='succeeded') AS succeeded_sources, \
                count(*) FILTER(WHERE latest_state='no_signal') AS no_signal_sources, \
                count(*) FILTER(WHERE latest_state IN ('pending','running')) AS active_sources, \
                count(*) FILTER(WHERE latest_state='retryable') AS retryable_sources \
         FROM current",
    )
    .bind(
        policy
            .as_ref()
            .map(|row| row.get::<String, _>("derivation_version"))
            .unwrap_or_else(|| DERIVATION_VERSION.to_owned()),
    )
    .fetch_one(&mut *transaction)
    .await?;
    let terminal_item_states: Value = sqlx::query_scalar(
        "WITH current AS ( \
             SELECT latest_item.state AS latest_state \
             FROM linggan_comment_research_derivation_current derivation \
             LEFT JOIN LATERAL ( \
                 SELECT item.state \
                 FROM linggan_comment_research_run_item item \
                 WHERE item.derivation_ref=derivation.derivation_ref \
                 ORDER BY item.updated_at DESC,item.run_ref DESC LIMIT 1 \
             ) latest_item ON true \
             WHERE derivation.derivation_version=$1 AND derivation.eligibility='eligible' \
         ), grouped AS ( \
             SELECT latest_state,count(*) AS item_count FROM current \
             WHERE latest_state IN ('incompatible','unrecoverable','model_failed','restricted') \
             GROUP BY latest_state \
         ) \
         SELECT COALESCE(jsonb_object_agg(latest_state,item_count),'{}'::jsonb) FROM grouped",
    )
    .bind(
        policy
            .as_ref()
            .map(|row| row.get::<String, _>("derivation_version"))
            .unwrap_or_else(|| DERIVATION_VERSION.to_owned()),
    )
    .fetch_one(&mut *transaction)
    .await?;
    let selected = match policy.as_ref() {
        Some(policy) => select_eligible_derivations(&mut transaction, policy).await?,
        None => Vec::new(),
    };
    let eligible_backlog = match policy.as_ref() {
        Some(policy) => select_eligible_resolution_backlog(&mut transaction, policy).await?,
        None => Vec::new(),
    };
    let selected_context_sources = selected
        .iter()
        .filter(|source| source.clean_state == "context")
        .count();
    let selected_missing_parent_context_sources = selected
        .iter()
        .filter(|source| {
            source.clean_state == "context"
                && matches!(
                    parent_context_from_manifest(&source.context_manifest),
                    ParentResearchContext::Unavailable | ParentResearchContext::Invalid
                )
        })
        .count();
    let source_limit = policy.as_ref().map(|row| row.get::<i32, _>("source_limit"));
    transaction.commit().await?;
    Ok(ResearchRunPreview {
        policy_configured: source_limit.is_some(),
        source_limit,
        eligible_sources: summary.get::<i64, _>("eligible_sources") as usize,
        unprocessed_sources: summary.get::<i64, _>("unprocessed_sources") as usize,
        recoverable_sources: summary.get::<i64, _>("recoverable_sources") as usize,
        selected_sources: selected.len(),
        eligible_backlog_atoms: eligible_backlog.len(),
        selected_backlog_atoms: eligible_backlog.len(),
        selected_context_sources,
        selected_missing_parent_context_sources,
        succeeded_sources: summary.get::<i64, _>("succeeded_sources") as usize,
        no_signal_sources: summary.get::<i64, _>("no_signal_sources") as usize,
        active_sources: summary.get::<i64, _>("active_sources") as usize,
        retryable_sources: summary.get::<i64, _>("retryable_sources") as usize,
        terminal_item_states,
    })
}

/// Deletes only regenerable Comment Research V1 state for a local development reset.
///
/// Raw comments, content attribution, source restrictions, model configuration, embedding
/// profiles, active policy and the generic invocation ledger are deliberately outside this
/// operation.  It refuses to race a V1 model call; queued historical work is safe to remove.
pub async fn reset_development_derived(
    database: &Database,
) -> Result<DevelopmentResetReceipt, CommentResearchKernelError> {
    let mut transaction = database.pool().begin().await?;
    let active_operations: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM ( \
             SELECT 1 FROM linggan_comment_research_run_item WHERE state='running' \
             UNION ALL \
             SELECT 1 FROM linggan_comment_research_run_item item \
             JOIN linggan_model_invocation invocation ON invocation.invocation_ref=item.invocation_ref \
             WHERE invocation.state='running' \
             UNION ALL \
             SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
             JOIN linggan_model_invocation invocation ON invocation.invocation_ref=embedding.invocation_ref \
             WHERE invocation.state='running' \
             UNION ALL \
             SELECT 1 FROM linggan_comment_research_problem_resolution resolution \
             JOIN linggan_model_invocation invocation ON invocation.invocation_ref=resolution.invocation_ref \
             WHERE invocation.state='running' \
             UNION ALL \
             SELECT 1 FROM linggan_comment_research_problem_pair_evaluation evaluation \
             WHERE evaluation.state='running' \
             UNION ALL \
             SELECT 1 FROM linggan_comment_research_problem_pair_evaluation evaluation \
             JOIN linggan_model_invocation invocation ON invocation.invocation_ref=evaluation.invocation_ref \
             WHERE invocation.state='running' \
         ) active",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if active_operations > 0 {
        return Err(CommentResearchKernelError::DevelopmentResetBlocked { active_operations });
    }
    sqlx::query(
        "LOCK TABLE \
           linggan_comment_research_change_observation, \
           linggan_comment_research_problem_window_stat, \
           linggan_comment_research_result_revision, \
           linggan_comment_research_problem_resolution_execution, \
           linggan_comment_research_problem_pair_evaluation, \
           linggan_comment_research_problem_resolution, \
           linggan_comment_research_atom_problem_membership, \
           linggan_comment_research_atom_embedding, \
           linggan_comment_research_problem_definition, \
           linggan_comment_research_problem, \
           linggan_comment_research_atom, \
           linggan_comment_research_run_item, \
           linggan_comment_research_run, \
           linggan_comment_research_derivation, \
           linggan_comment_research_embedding_space \
         IN SHARE ROW EXCLUSIVE MODE",
    )
    .execute(&mut *transaction)
    .await?;
    for statement in [
        "ALTER TABLE linggan_comment_research_derivation DISABLE TRIGGER linggan_comment_research_derivation_immutable",
        "ALTER TABLE linggan_comment_research_atom DISABLE TRIGGER linggan_comment_research_atom_immutable",
        "ALTER TABLE linggan_comment_research_embedding_space DISABLE TRIGGER linggan_comment_research_embedding_space_immutable",
        "ALTER TABLE linggan_comment_research_problem_definition DISABLE TRIGGER linggan_comment_research_problem_definition_immutable",
    ] {
        sqlx::query(statement).execute(&mut *transaction).await?;
    }
    let deleted_results = sqlx::query("DELETE FROM linggan_comment_research_change_observation")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM linggan_comment_research_problem_window_stat")
        .execute(&mut *transaction)
        .await?;
    let deleted_result_revisions =
        sqlx::query("DELETE FROM linggan_comment_research_result_revision")
            .execute(&mut *transaction)
            .await?
            .rows_affected();
    sqlx::query("DELETE FROM linggan_comment_research_problem_resolution_execution")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM linggan_comment_research_problem_pair_evaluation")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM linggan_comment_research_problem_resolution")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM linggan_comment_research_atom_problem_membership")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM linggan_comment_research_atom_embedding")
        .execute(&mut *transaction)
        .await?;
    sqlx::query("DELETE FROM linggan_comment_research_problem_definition")
        .execute(&mut *transaction)
        .await?;
    let deleted_problems = sqlx::query("DELETE FROM linggan_comment_research_problem")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let deleted_atoms = sqlx::query("DELETE FROM linggan_comment_research_atom")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let deleted_run_items = sqlx::query("DELETE FROM linggan_comment_research_run_item")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let deleted_runs = sqlx::query("DELETE FROM linggan_comment_research_run")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    let deleted_derivations = sqlx::query("DELETE FROM linggan_comment_research_derivation")
        .execute(&mut *transaction)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM linggan_comment_research_embedding_space")
        .execute(&mut *transaction)
        .await?;
    for statement in [
        "ALTER TABLE linggan_comment_research_problem_definition ENABLE TRIGGER linggan_comment_research_problem_definition_immutable",
        "ALTER TABLE linggan_comment_research_embedding_space ENABLE TRIGGER linggan_comment_research_embedding_space_immutable",
        "ALTER TABLE linggan_comment_research_atom ENABLE TRIGGER linggan_comment_research_atom_immutable",
        "ALTER TABLE linggan_comment_research_derivation ENABLE TRIGGER linggan_comment_research_derivation_immutable",
    ] {
        sqlx::query(statement).execute(&mut *transaction).await?;
    }
    transaction.commit().await?;
    Ok(DevelopmentResetReceipt {
        deleted_derivations,
        deleted_runs,
        deleted_run_items,
        deleted_atoms,
        deleted_problems,
        deleted_results: deleted_results + deleted_result_revisions,
    })
}

async fn research_model_ready_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    config_ref: Uuid,
) -> Result<bool, CommentResearchKernelError> {
    model_settings::research_model_semantically_ready_in_transaction(transaction, config_ref)
        .await
        .map_err(|error| match error {
            model_settings::ModelError::Database(source) => {
                CommentResearchKernelError::Database(source)
            }
            _ => CommentResearchKernelError::ModelNotReady,
        })
}

struct EligibleDerivation {
    derivation_ref: Uuid,
    source_sha256: String,
    research_sha256: String,
    derivation_input_hash: String,
    research_text: String,
    clean_state: String,
    context_manifest: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EligibleResolutionBacklog {
    atom_ref: Uuid,
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

    fn research_fingerprint(
        &self,
        extraction_rule_hash: &str,
        membership_policy_hash: &str,
        config_ref: Uuid,
    ) -> String {
        content_hash(&format!(
            "comment-research.fingerprint.v1\n{}\n{}\n{}\n{}\n{}",
            self.derivation_input_hash,
            extraction_rule_hash,
            membership_policy_hash,
            config_ref,
            DERIVATION_VERSION,
        ))
    }
}

async fn select_eligible_derivations(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy: &sqlx::postgres::PgRow,
) -> Result<Vec<EligibleDerivation>, CommentResearchKernelError> {
    let config_ref: Option<Uuid> = policy.get("config_ref");
    let Some(config_ref) = config_ref else {
        return Ok(Vec::new());
    };
    let rows = sqlx::query(
        "SELECT derivation_ref,source_sha256,research_sha256,derivation_input_hash,research_text,clean_state,context_manifest \
         FROM linggan_comment_research_derivation_current derivation \
         WHERE derivation_version=$1 AND eligibility='eligible' \
           AND NOT EXISTS ( \
             SELECT 1 FROM linggan_comment_research_run_item used \
             WHERE used.derivation_ref=derivation.derivation_ref \
               AND used.research_fingerprint=encode(sha256(convert_to( \
                   concat_ws(E'\\n','comment-research.fingerprint.v1',derivation.derivation_input_hash,$3,$4,$5::text,$6), \
                   'UTF8')),'hex') \
               AND used.state IN ('succeeded','no_signal','pending','running','retryable','incompatible','unrecoverable','model_failed') \
           ) \
         ORDER BY created_at DESC,derivation_ref DESC LIMIT $2",
    )
    .bind(policy.get::<String, _>("derivation_version"))
    .bind(policy.get::<i32, _>("source_limit"))
    .bind(policy.get::<String, _>("extraction_rule_hash"))
    .bind(policy.get::<String, _>("membership_policy_hash"))
    .bind(config_ref)
    .bind(DERIVATION_VERSION)
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| EligibleDerivation {
            derivation_ref: row.get("derivation_ref"),
            source_sha256: row.get("source_sha256"),
            research_sha256: row.get("research_sha256"),
            derivation_input_hash: row.get("derivation_input_hash"),
            research_text: row.get("research_text"),
            clean_state: row.get("clean_state"),
            context_manifest: row.get("context_manifest"),
        })
        .collect())
}

/// A resolution remains attached to the Atom and its original source Run.  A subsequent Run can
/// only take over a terminal backlog row under one of the explicit retry policies below; this
/// query is shared by preview and start so the browser cannot nominate historical comments.
async fn select_eligible_resolution_backlog(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    policy: &sqlx::postgres::PgRow,
) -> Result<Vec<EligibleResolutionBacklog>, CommentResearchKernelError> {
    let config_ref: Option<Uuid> = policy.get("config_ref");
    let Some(config_ref) = config_ref else {
        return Ok(Vec::new());
    };
    let rows = sqlx::query(
        "SELECT resolution.atom_ref \
         FROM linggan_comment_research_problem_resolution resolution \
         JOIN linggan_comment_research_atom atom ON atom.atom_ref=resolution.atom_ref \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=atom.derivation_ref \
         JOIN linggan_comment_research_run source_run ON source_run.run_ref=atom.run_ref \
         JOIN linggan_comment_research_policy_revision source_policy \
           ON source_policy.policy_revision_ref=source_run.policy_revision_ref \
         JOIN linggan_comment_research_run prior_execution \
           ON prior_execution.run_ref=resolution.execution_run_ref \
         LEFT JOIN linggan_comment_research_problem_catalog_guard catalog_guard \
           ON catalog_guard.scope_domain_ref=source_policy.problem_scope_domain_ref \
         LEFT JOIN linggan_comment_research_atom_problem_membership membership \
           ON membership.atom_ref=atom.atom_ref AND membership.current \
         WHERE prior_execution.state NOT IN ('queued','running') \
           AND membership.atom_ref IS NULL \
           AND ( \
             (resolution.state='model_failed' AND ( \
               (resolution.failure_code='problem_resolution_admission_rejected' AND resolution.attempts<3) \
               OR resolution.failure_code IN ( \
                 'provider_timeout','provider_unavailable','provider_rate_limited', \
                 'provider_network_error','model_adapter_unavailable','model_database_unavailable','worker_interrupted' \
               ) \
               OR (resolution.failure_code IN ( \
                 'problem_resolution_json_unparseable','problem_resolution_json_schema_rejected' \
               ) AND (source_policy.membership_policy_hash<>$1 \
                       OR source_policy.config_ref IS DISTINCT FROM $2)) \
             )) \
             OR (resolution.state='succeeded' \
                 AND resolution.decision_kind IN ('deferred_novel','deferred_ambiguous','deferred_context') \
                 AND source_policy.membership_policy_hash=$1 \
                 AND source_policy.problem_scope_domain_ref=$5 \
                 AND catalog_guard.revision>resolution.catalog_revision_at_recall) \
           ) \
         ORDER BY resolution.last_attempt_at NULLS FIRST,resolution.created_at,resolution.atom_ref \
         LIMIT LEAST($3,$4)",
    )
    .bind(policy.get::<String, _>("membership_policy_hash"))
    .bind(config_ref)
    .bind(policy.get::<i32, _>("source_limit"))
    .bind(MAX_BACKLOG_RESOLUTIONS_PER_RUN)
    .bind(policy.get::<Option<Uuid>, _>("problem_scope_domain_ref"))
    .fetch_all(&mut **transaction)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| EligibleResolutionBacklog {
            atom_ref: row.get("atom_ref"),
        })
        .collect())
}

async fn activate_eligible_resolution_backlog(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    execution_run_ref: Uuid,
    backlog: &[EligibleResolutionBacklog],
) -> Result<usize, CommentResearchKernelError> {
    if backlog.is_empty() {
        return Ok(0);
    }
    let atom_refs = backlog
        .iter()
        .map(|candidate| candidate.atom_ref)
        .collect::<Vec<_>>();
    let reactivated: Vec<Uuid> = sqlx::query_scalar(
        "WITH activated AS ( \
             UPDATE linggan_comment_research_problem_resolution resolution \
             SET state='pending',execution_run_ref=$1, \
                 attempts=CASE WHEN failure_code='problem_resolution_admission_rejected' THEN attempts ELSE 0 END, \
                 next_attempt_at=NULL,lease_until=NULL,finished_at=NULL, \
                 failure_code=CASE WHEN decision_kind IN ('deferred_novel','deferred_ambiguous','deferred_context') \
                                   THEN 'candidate_catalog_changed' ELSE failure_code END, \
                 decision_kind=NULL,decision_payload=NULL,recheck_conditions=NULL,resolution_input_hash=NULL, \
                 catalog_revision_at_recall=NULL,updated_at=scope_001_now() \
             WHERE resolution.atom_ref=ANY($2) \
               AND (resolution.state='model_failed' \
                    OR (resolution.state='succeeded' AND resolution.decision_kind IN ( \
                        'deferred_novel','deferred_ambiguous','deferred_context'))) \
               AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_atom_problem_membership membership \
                              WHERE membership.atom_ref=resolution.atom_ref AND membership.current) \
             RETURNING resolution.atom_ref,resolution.attempts,resolution.last_attempt_at,resolution.failure_code \
         ) \
         INSERT INTO linggan_comment_research_problem_resolution_execution( \
             atom_ref,run_ref,state,attempts,last_attempt_at,failure_code \
         ) SELECT atom_ref,$1,'pending',attempts,last_attempt_at,failure_code FROM activated \
         RETURNING atom_ref",
    )
    .bind(execution_run_ref)
    .bind(&atom_refs)
    .fetch_all(&mut **transaction)
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_research_run \
         SET exclusion_counts=jsonb_set(exclusion_counts,'{selectedResolutionBacklog}',to_jsonb($2::bigint),true), \
             updated_at=scope_001_now() WHERE run_ref=$1",
    )
    .bind(execution_run_ref)
    .bind(i64::try_from(reactivated.len()).expect("backlog bound fits i64"))
    .execute(&mut **transaction)
    .await?;
    Ok(reactivated.len())
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
               AND item.attempts < 3 \
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

/// Returns only the frozen material belonging to a current V2 claim.  A missing row is a normal
/// lease-loss outcome: another recovery path may have settled the Item while the caller waited.
pub async fn load_claimed_research_input(
    database: &Database,
    claim: &ResearchRunItemClaim,
) -> Result<Option<ClaimedResearchInput>, CommentResearchKernelError> {
    let row = sqlx::query(
        "SELECT derivation.research_text,derivation.clean_state,derivation.context_manifest,policy.config_ref,policy.token_limit, \
                policy.problem_scope_definition \
         FROM linggan_comment_research_run_item item \
         JOIN linggan_comment_research_run run USING(run_ref) \
         JOIN linggan_comment_research_policy_revision policy \
           ON policy.policy_revision_ref=run.policy_revision_ref \
         JOIN linggan_comment_research_derivation_readable derivation \
           ON derivation.derivation_ref=item.derivation_ref \
         WHERE item.run_ref=$1 AND item.derivation_ref=$2 AND item.attempts=$3 AND item.state='running'",
    )
    .bind(claim.run_ref)
    .bind(claim.derivation_ref)
    .bind(claim.attempt)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| ClaimedResearchInput {
        run_ref: claim.run_ref,
        derivation_ref: claim.derivation_ref,
        attempt: claim.attempt,
        research_text: row.get("research_text"),
        clean_state: row.get("clean_state"),
        parent_context: parent_context_from_manifest(&row.get("context_manifest")),
        config_ref: row.get("config_ref"),
        token_limit: row.get("token_limit"),
        problem_scope_definition: row.get("problem_scope_definition"),
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
             WHERE run_ref=$1 AND derivation_ref=$2 AND attempts=$5 AND state='running'",
        )
        .bind(claim.run_ref)
        .bind(claim.derivation_ref)
        .bind(state)
        .bind(failure_code)
        .bind(claim.attempt)
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
             WHERE run_ref=$1 AND derivation_ref=$2 AND attempts=$4 AND state='running'",
        )
        .bind(claim.run_ref)
        .bind(claim.derivation_ref)
        .bind(failure_code)
        .bind(claim.attempt)
        .execute(&mut *transaction)
        .await?
        .rows_affected()
    };
    if changed != 1 {
        transaction.rollback().await?;
        return Err(CommentResearchKernelError::ClaimLost);
    }
    refresh_run_completion(&mut transaction, claim.run_ref).await?;
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
                              WHERE embedding.atom_ref=atom.atom_ref AND embedding.state IN ('pending','running','retryable')) \
               AND (EXISTS(SELECT 1 FROM linggan_comment_research_atom_embedding embedding \
                           WHERE embedding.atom_ref=atom.atom_ref AND embedding.state IN ('failed','incompatible')) \
                    OR ($2 AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_embedding_profile profile \
                                  JOIN linggan_model_entry model USING(model_ref) \
                                  JOIN linggan_model_connection_version version ON version.version_ref=model.connection_version_ref \
                                  JOIN linggan_model_connection connection USING(connection_ref) \
                                  WHERE profile.singleton AND profile.enabled AND connection.enabled))) \
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
               AND false \
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
            ) AS unsettled_resolution_atoms, \
            (SELECT count(*) FROM linggan_comment_research_problem_resolution resolution \
             JOIN linggan_comment_research_atom atom ON atom.atom_ref=resolution.atom_ref \
             WHERE resolution.execution_run_ref=$1 AND atom.run_ref<>resolution.execution_run_ref \
               AND resolution.state IN ('pending','running','retryable')) AS unsettled_backlog_resolutions, \
            (SELECT count(*) FROM linggan_comment_research_problem_resolution resolution \
             JOIN linggan_comment_research_atom atom ON atom.atom_ref=resolution.atom_ref \
             WHERE resolution.execution_run_ref=$1 AND atom.run_ref<>resolution.execution_run_ref \
               AND resolution.state IN ('model_failed','incompatible')) AS failed_backlog_resolutions, \
            (SELECT count(*) FROM linggan_comment_research_problem_pair_evaluation evaluation \
             WHERE evaluation.execution_run_ref=$1 AND evaluation.state IN ('pending','running','retryable')) \
              AS unsettled_pair_evaluations, \
            (SELECT count(*) FROM linggan_comment_research_problem_pair_evaluation evaluation \
             WHERE evaluation.execution_run_ref=$1 AND evaluation.state IN ('model_failed','incompatible')) \
              AS failed_pair_evaluations",
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
    let unsettled_backlog_resolutions: i64 = outcome.get("unsettled_backlog_resolutions");
    let failed_backlog_resolutions: i64 = outcome.get("failed_backlog_resolutions");
    let unsettled_pair_evaluations: i64 = outcome.get("unsettled_pair_evaluations");
    let failed_pair_evaluations: i64 = outcome.get("failed_pair_evaluations");
    if unsettled_resolution_atoms > 0
        || unsettled_backlog_resolutions > 0
        || unsettled_pair_evaluations > 0
    {
        return Ok(());
    }
    let terminal_unassigned = embedding_failed_atoms + resolution_failed_atoms;
    if unassigned_atoms > 0 && terminal_unassigned < unassigned_atoms {
        return Ok(());
    }
    let state = if item_failed_count > 0
        || terminal_unassigned > 0
        || failed_backlog_resolutions > 0
        || failed_pair_evaluations > 0
    {
        "completed_with_failures"
    } else {
        "completed"
    };
    sqlx::query(
        "UPDATE linggan_comment_research_run \
         SET state=$2,failure_counts=jsonb_strip_nulls(jsonb_build_object( \
               'runItems',NULLIF($3,0), \
               'embedding',NULLIF($4,0), \
               'problemResolution',NULLIF($5,0), \
               'backlogProblemResolution',NULLIF($6,0), \
               'problemPairResolution',NULLIF($7,0) \
             )),finished_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE run_ref=$1 AND state IN ('queued','running')",
    )
    .bind(run_ref)
    .bind(state)
    .bind(item_failed_count)
    .bind(embedding_failed_atoms)
    .bind(resolution_failed_atoms)
    .bind(failed_backlog_resolutions)
    .bind(failed_pair_evaluations)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Re-evaluates both the immutable source Run and the current execution Run for an Atom. A late
/// successful backlog resolution can therefore improve the source window's organization
/// coverage, while the later retry Run records only the execution health and never publishes a
/// replacement statistical window.
pub async fn refresh_run_completion_for_atom(
    database: &Database,
    atom_ref: Uuid,
) -> Result<(), CommentResearchKernelError> {
    let run_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT run_ref FROM ( \
             SELECT atom.run_ref FROM linggan_comment_research_atom atom WHERE atom.atom_ref=$1 \
             UNION \
             SELECT resolution.execution_run_ref FROM linggan_comment_research_problem_resolution resolution \
             WHERE resolution.atom_ref=$1 \
         ) involved_runs",
    )
    .bind(atom_ref)
    .fetch_all(database.pool())
    .await?;
    if run_refs.is_empty() {
        return Ok(());
    }
    let mut transaction = database.pool().begin().await?;
    for run_ref in run_refs {
        refresh_run_completion(&mut transaction, run_ref).await?;
    }
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
         SET state='failed',failure_code='embedding_configuration_unavailable', \
             lease_until=NULL,next_attempt_at=NULL,updated_at=scope_001_now() \
         FROM linggan_comment_research_atom atom \
         JOIN linggan_comment_research_run run ON run.run_ref=atom.run_ref \
         WHERE embedding.atom_ref=atom.atom_ref AND embedding.state IN ('pending','retryable') \
           AND run.state IN ('queued','running')",
    )
    .execute(database.pool())
    .await?;
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution resolution \
         SET state='incompatible',failure_code='embedding_configuration_unavailable', \
             finished_at=scope_001_now(),lease_until=NULL,next_attempt_at=NULL,updated_at=scope_001_now() \
         FROM linggan_comment_research_run run \
         WHERE resolution.execution_run_ref=run.run_ref AND resolution.state IN ('pending','retryable') \
           AND run.state IN ('queued','running')",
    )
    .execute(database.pool())
    .await?;
    // The live resolution row moves between execution Runs. Mirror terminalization into the
    // immutable execution record before refreshing each Run, otherwise a disabled embedding
    // profile leaves a retry Run displayed as pending forever.
    sqlx::query(
        "UPDATE linggan_comment_research_problem_resolution_execution history \
         SET state=resolution.state,attempts=resolution.attempts,last_attempt_at=resolution.last_attempt_at, \
             next_attempt_at=resolution.next_attempt_at,failure_code=resolution.failure_code, \
             invocation_ref=resolution.invocation_ref,updated_at=resolution.updated_at,finished_at=resolution.finished_at \
         FROM linggan_comment_research_problem_resolution resolution \
         WHERE history.atom_ref=resolution.atom_ref AND history.run_ref=resolution.execution_run_ref \
           AND resolution.state='incompatible' \
           AND resolution.failure_code='embedding_configuration_unavailable'",
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
    let parent_body: Option<String> = row.get("parent_body_text");
    let parent_hash = parent_body.as_deref().map(content_hash);
    let parent_requested = row
        .get::<Option<String>, _>("parent_comment_external_id")
        .is_some();
    let parent_author_role = parent_ref.map(|_| {
        classify_author_role(
            row.get::<Option<String>, _>("parent_author_external_id")
                .as_deref(),
            row.get::<Option<String>, _>("content_author_external_id")
                .as_deref(),
        )
    });
    let parent = match (parent_requested, parent_body.as_deref(), parent_author_role) {
        (false, _, _) => json!({"state":"not_requested"}),
        (true, Some(body), Some(role)) => {
            let research = derive_research_text(body, role);
            if research.text.trim().is_empty() {
                json!({"state":"unavailable"})
            } else {
                json!({"state":"available","researchText":research.text})
            }
        }
        (true, _, _) => json!({"state":"unavailable"}),
    };
    json!({
        "contract":CONTEXT_MANIFEST_CONTRACT,
        "semantic":{"parent":parent},
        "audit":{
            "workRef":content_ref,
            "parentSourceRef":parent_ref,
            "parentSourceSha256":parent_hash,
            "parentRequested":parent_requested,
            "parentAuthorRole":parent_author_role.map(CommentAuthorRole::as_db),
        }
    })
}

fn parent_context_from_manifest(context_manifest: &Value) -> ParentResearchContext {
    if context_manifest.get("contract").and_then(Value::as_str) != Some(CONTEXT_MANIFEST_CONTRACT) {
        return ParentResearchContext::Invalid;
    }
    let Some(parent) = context_manifest
        .get("semantic")
        .and_then(|semantic| semantic.get("parent"))
    else {
        return ParentResearchContext::Invalid;
    };
    match parent.get("state").and_then(Value::as_str) {
        Some("not_requested") if parent.get("researchText").is_none() => {
            ParentResearchContext::NotRequested
        }
        Some("unavailable") if parent.get("researchText").is_none() => {
            ParentResearchContext::Unavailable
        }
        Some("available") => parent
            .get("researchText")
            .and_then(Value::as_str)
            .filter(|text| !text.trim().is_empty())
            .map(|research_text| ParentResearchContext::Available {
                research_text: research_text.to_owned(),
            })
            .unwrap_or(ParentResearchContext::Invalid),
        _ => ParentResearchContext::Invalid,
    }
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

    #[test]
    fn parent_context_contract_never_treats_missing_required_text_as_no_context() {
        assert_eq!(
            parent_context_from_manifest(&json!({
                "contract":"comment-research.context.v2",
                "semantic":{"parent":{"state":"available","researchText":"习惯性熬夜"}}
            })),
            ParentResearchContext::Available {
                research_text: "习惯性熬夜".into()
            }
        );
        assert_eq!(
            parent_context_from_manifest(
                &json!({"contract":"comment-research.context.v2","semantic":{"parent":{"state":"unavailable"}}})
            ),
            ParentResearchContext::Unavailable
        );
        assert_eq!(
            parent_context_from_manifest(&json!({"workRef":"not-a-semantic-context"})),
            ParentResearchContext::Invalid
        );
    }
}
