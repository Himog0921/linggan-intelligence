//! Creator discovery is a projection of domain work usage, never a Target roster.
use linggan_contracts::creator_discovery::{CreatorScope, creator_key};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, HashMap, HashSet};
use uuid::Uuid;

pub const RULE_VERSION: &str = "creator-discovery.v3-dossier";
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("{0}")]
    Invalid(&'static str),
    #[error("domain_not_found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fragment {
    pub fragment_id: String,
    pub source_ref: Uuid,
    pub field: String,
    pub source_version: String,
    pub start: usize,
    pub end: usize,
    pub text: String,
    #[serde(default = "unconfirmed_speaker")]
    pub speaker: String,
    #[serde(default)]
    pub structure: Value,
    #[serde(default)]
    pub issues: Vec<Value>,
    #[serde(default)]
    pub reading_ref: Option<Uuid>,
    #[serde(default)]
    pub unit_ref: Option<Uuid>,
}
fn unconfirmed_speaker() -> String {
    "unconfirmed".into()
}
impl Default for Fragment {
    fn default() -> Self {
        Self {
            fragment_id: String::new(),
            source_ref: Uuid::nil(),
            field: String::new(),
            source_version: String::new(),
            start: 0,
            end: 0,
            text: String::new(),
            speaker: unconfirmed_speaker(),
            structure: Value::Null,
            issues: Vec::new(),
            reading_ref: None,
            unit_ref: None,
        }
    }
}
impl Fragment {
    /// Every source constructs its own content block before reaching a consumer.
    /// Missing structure never causes a note's raw text to be silently substituted.
    pub fn model_content(&self) -> Value {
        self.structure.clone()
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryWork {
    pub work_ref: Uuid,
    #[serde(skip_serializing)]
    pub usage_roles: Vec<String>,
    pub creator_key: Option<String>,
    pub platform: String,
    pub author_external_id: Option<String>,
    pub display_name: Option<String>,
    pub title: Option<String>,
    pub reading_title: Value,
    pub published_date: Option<String>,
    pub published_epoch: Option<i64>,
    pub likes: Option<i64>,
    pub likes_observed_at: Option<String>,
    pub likes_observed_display: Option<String>,
    pub comments: Option<i64>,
    pub collects: Option<i64>,
    pub first_added: String,
    pub first_added_display: String,
    pub fingerprint: String,
    #[serde(skip_serializing)]
    pub search_text: String,
    #[serde(skip_serializing)]
    pub fragments: Vec<Fragment>,
    pub relevance: String,
    pub traits: Vec<String>,
    pub topic_hints: Vec<String>,
    pub analysis: Value,
    pub analysis_state: String,
    pub observation: Value,
    pub profile: Value,
    pub focus: Value,
    pub author_analysis: Value,
    pub acquisition_kind: String,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryData {
    pub works: Vec<DiscoveryWork>,
    pub as_of: String,
    pub display_as_of: String,
    pub now_epoch: i64,
    pub domain: Value,
    pub policy: Value,
    pub observation_lookup_available: bool,
    work_name_sources: HashMap<Uuid, Value>,
    profile_name_candidates: HashMap<String, (String, Uuid)>,
}

pub fn hash(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}
pub fn fragment(
    work: Uuid,
    field: &str,
    text: Option<&str>,
    source: Option<Uuid>,
    version: Option<&str>,
) -> Option<Fragment> {
    let text = text.filter(|value| !value.trim().is_empty())?;
    let source = source?;
    let source_version = version
        .map(str::to_owned)
        .unwrap_or_else(|| source.to_string());
    let fragment_id = format!("{work}.{source}.{field}");
    Some(Fragment {
        structure: json!({"ref":fragment_id,"kind":"paragraph","text":text,"speaker":"author"}),
        speaker: "author".into(),
        fragment_id,
        source_ref: source,
        field: field.into(),
        source_version,
        start: 0,
        end: text.chars().count(),
        text: text.into(),
        ..Fragment::default()
    })
}

fn profile_support_fragment(work: &DiscoveryWork) -> Option<Value> {
    let profile = &work.profile;
    let source = profile.get("sourceRef")?.as_str()?.parse::<Uuid>().ok()?;
    let f = fragment(
        work.work_ref,
        "biography",
        profile.get("biography")?.as_str(),
        Some(source),
        None,
    )?;
    Some(
        json!({"fragmentId":f.fragment_id,"field":"biography","sourceType":"author_profile","sourceRef":source,"text":f.text.chars().take(200).collect::<String>()}),
    )
}
pub fn work_support_fragments(work: &DiscoveryWork) -> Vec<Value> {
    work.fragments.iter().map(|f|json!({"fragmentId":f.fragment_id,"field":f.field,"sourceType":"work_material","workRef":work.work_ref,"sourceRef":f.source_ref,"text":f.text.chars().take(200).collect::<String>()})).collect()
}
pub fn author_support_fragments(works: &[&DiscoveryWork], field: &str) -> Vec<Value> {
    let mut fragments = Vec::new();
    let mut ordered = works.to_vec();
    ordered.sort_by(|a, b| {
        a.published_date
            .cmp(&b.published_date)
            .then_with(|| a.work_ref.cmp(&b.work_ref))
    });
    if let Some(profile) = ordered.first().and_then(|w| profile_support_fragment(w)) {
        fragments.push(profile);
    }
    if field != "institution_or_brand" {
        for work in ordered {
            fragments.extend(work_support_fragments(work));
        }
    }
    let mut seen = HashSet::new();
    fragments.retain(|v| {
        v["fragmentId"]
            .as_str()
            .is_some_and(|id| seen.insert(id.to_owned()))
    });
    fragments.truncate(40);
    fragments
}
fn current_author_evidence(result: &Value, fragments: &[Value]) -> Vec<Value> {
    let cited: HashSet<&str> = ["focus", "institution_or_brand"]
        .into_iter()
        .flat_map(|field| {
            result[field]["evidenceFragmentIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
        })
        .collect();
    fragments
        .iter()
        .filter(|f| {
            f["fragmentId"]
                .as_str()
                .is_some_and(|id| cited.contains(id))
        })
        .cloned()
        .collect()
}
pub fn is_positive_override(field: &str, value: &str) -> bool {
    match field {
        "relevance" => value == "related",
        "personal_experience"
        | "professional_output"
        | "explicit_promotion"
        | "institution_or_brand" => value == "yes",
        "focus" => value != "unknown",
        _ => false,
    }
}
fn project_manual_field(manual: &mut Value, field: &str, allowed: &HashSet<String>) {
    let Some(annotation) = manual.get(field) else {
        return;
    };
    let value = annotation
        .pointer("/value/value")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    if !is_positive_override(field, value) {
        return;
    }
    let supported = annotation
        .get("supportFragmentIds")
        .and_then(Value::as_array)
        .is_some_and(|ids| {
            ids.iter()
                .any(|id| id.as_str().is_some_and(|id| allowed.contains(id)))
        });
    if !supported {
        manual[field] = json!({"value":{"value":"unknown","reason":"当前依据已变化或不可读，请复核；原人工记录仍保留","supportUnavailable":true},"supportUnavailable":true});
    }
}
fn effective_value(result: &Value, manual: &Value, field: &str) -> Value {
    manual
        .get(field)
        .and_then(|v| v.get("value"))
        .cloned()
        .unwrap_or_else(|| result.get(field).cloned().unwrap_or(Value::Null))
}
pub async fn load(database: &Database, q: &CreatorScope) -> Result<DiscoveryData, DiscoveryError> {
    q.validate().map_err(DiscoveryError::Invalid)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let data = load_in(&mut tx, q).await?;
    tx.commit().await?;
    Ok(data)
}

/// Complete canonical fragments for referenced works under current eligibility.
pub async fn load_for_works(
    database: &Database,
    q: &CreatorScope,
    work_refs: &[Uuid],
) -> Result<DiscoveryData, DiscoveryError> {
    q.validate().map_err(DiscoveryError::Invalid)?;
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let data = load_materials_in(&mut tx, q, Some(work_refs), None).await?;
    tx.commit().await?;
    Ok(data)
}

/// Topic research reads the complete qualified material, including comment-only works.
/// Creator and topic consumers share the same complete note material contract.
/// Platforms here are discovered only from this domain's canonical material usage.
pub async fn load_topic_materials(
    database: &Database,
    domain: Uuid,
) -> Result<DiscoveryData, DiscoveryError> {
    load_topic_material_scope(database, domain, None, None).await
}

/// Read only the requested works through the same current domain/material gates.
/// An empty selection is an empty scope, never an unbounded domain fallback.
pub async fn load_topic_materials_for_works(
    database: &Database,
    domain: Uuid,
    work_refs: &[Uuid],
) -> Result<DiscoveryData, DiscoveryError> {
    load_topic_material_scope(database, domain, Some(work_refs), None).await
}

/// Reconstruct the exact observation cutoff while applying current source restrictions.
pub async fn load_topic_materials_for_works_at(
    database: &Database,
    domain: Uuid,
    work_refs: &[Uuid],
    as_of: &str,
) -> Result<DiscoveryData, DiscoveryError> {
    load_topic_material_scope(database, domain, Some(work_refs), Some(as_of)).await
}

async fn load_topic_material_scope(
    database: &Database,
    domain: Uuid,
    work_refs: Option<&[Uuid]>,
    as_of: Option<&str>,
) -> Result<DiscoveryData, DiscoveryError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    // Qualification reconstructs finite source windows, not an analytical scan.
    // Keep planner settings local so returning a pooled connection changes no caller.
    sqlx::query("SET LOCAL jit=off").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL plan_cache_mode=force_custom_plan")
        .execute(&mut *tx)
        .await?;
    let as_of = crate::research_dossier::cutoff_in(&mut tx, as_of).await?;
    let mut platforms: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT content.platform FROM linggan_material_domain_usage usage JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref WHERE usage.domain_ref=$1 AND ($2::uuid[] IS NULL OR usage.content_public_ref=ANY($2)) ORDER BY content.platform",
    )
    .bind(domain)
    .bind(work_refs)
    .fetch_all(&mut *tx)
    .await?;
    // An empty scope still returns the real domain definition and observation cutoff.
    if platforms.is_empty() {
        platforms.push(String::new());
    }
    let mut data: Option<DiscoveryData> = None;
    for platform in platforms {
        let scope: CreatorScope =
            serde_json::from_value(json!({"domain":domain,"platform":platform}))
                .map_err(|_| DiscoveryError::Invalid("invalid_topic_material_scope"))?;
        let mut part = load_materials_in(&mut tx, &scope, work_refs, Some(&as_of)).await?;
        if let Some(data) = data.as_mut() {
            data.works.append(&mut part.works);
            data.work_name_sources.extend(part.work_name_sources);
            data.profile_name_candidates
                .extend(part.profile_name_candidates);
            data.observation_lookup_available &= part.observation_lookup_available;
        } else {
            data = Some(part);
        }
    }
    let mut data = data.ok_or(DiscoveryError::NotFound)?;
    data.works.sort_by_key(|work| work.work_ref);
    tx.commit().await?;
    Ok(data)
}

/// Read one creator work with the same complete projection as the worker, without scheduling.
pub async fn load_creator_material_for_work(
    database: &Database,
    scope: &CreatorScope,
    work: Uuid,
) -> Result<DiscoveryData, DiscoveryError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let result = load_materials_in(&mut tx, scope, Some(&[work]), None).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn load_in(
    tx: &mut Transaction<'_, Postgres>,
    q: &CreatorScope,
) -> Result<DiscoveryData, DiscoveryError> {
    load_materials_in(tx, q, None, None).await
}

async fn load_materials_in(
    tx: &mut Transaction<'_, Postgres>,
    q: &CreatorScope,
    work_refs: Option<&[Uuid]>,
    as_of: Option<&str>,
) -> Result<DiscoveryData, DiscoveryError> {
    let as_of = crate::research_dossier::cutoff_in(tx, as_of).await?;
    let clock=sqlx::query("SELECT linggan_human_moment($1::timestamptz) AS display_now,(extract(epoch FROM $1::timestamptz)*1000)::bigint AS epoch").bind(&as_of).fetch_one(&mut **tx).await?;
    let display_as_of: String = clock.get("display_now");
    let now_epoch: i64 = clock.get("epoch");
    for date in [&q.published_from, &q.published_to].into_iter().flatten() {
        let valid: bool = sqlx::query_scalar("SELECT pg_input_is_valid($1,'date')")
            .bind(date)
            .fetch_one(&mut **tx)
            .await?;
        if !valid {
            return Err(DiscoveryError::Invalid("invalid_publication_date"));
        }
    }
    let domain:Value=sqlx::query_scalar("SELECT jsonb_build_object('domainRef',domain_ref,'name',name,'description',description,'researchGoal',research_goal,'status',status,'updatedAt',updated_at::text) FROM observation_domain WHERE domain_ref=$1").bind(q.domain).fetch_optional(&mut **tx).await?.ok_or(DiscoveryError::NotFound)?;
    let policy:Value=sqlx::query_scalar("SELECT jsonb_build_object('likeThreshold',like_threshold,'revision',revision,'analysisEnabled',analysis_enabled,'configRef',config_ref,'dailyTokenLimit',daily_token_limit,'updatedAt',updated_at::text) FROM linggan_creator_discovery_policy WHERE domain_ref=$1 AND platform=$2").bind(q.domain).bind(&q.platform).fetch_optional(&mut **tx).await?.unwrap_or(json!({"likeThreshold":null,"revision":0,"analysisEnabled":false,"configRef":null}));
    if q.sort == "viral_works" && policy["likeThreshold"].as_i64().is_none() {
        return Err(DiscoveryError::Invalid("viral_threshold_unset"));
    }
    let rows=sqlx::query("SELECT content_public_ref,COALESCE(min(created_at) FILTER(WHERE role=$2),min(created_at))::text AS first_added,linggan_human_moment(COALESCE(min(created_at) FILTER(WHERE role=$2),min(created_at))) AS first_added_display,array_agg(DISTINCT role) AS roles FROM linggan_material_domain_usage WHERE domain_ref=$1 AND ($3::uuid[] IS NULL OR content_public_ref=ANY($3)) GROUP BY content_public_ref").bind(q.domain).bind(&q.usage_role).bind(work_refs).fetch_all(&mut **tx).await?;
    let added: HashMap<Uuid, String> = rows
        .iter()
        .map(|r| (r.get("content_public_ref"), r.get("first_added")))
        .collect();
    let added_display: HashMap<Uuid, String> = rows
        .iter()
        .map(|r| (r.get("content_public_ref"), r.get("first_added_display")))
        .collect();
    let refs: Vec<Uuid> = added.keys().copied().collect();
    let roles: HashMap<Uuid, Vec<String>> = rows
        .iter()
        .map(|r| (r.get("content_public_ref"), r.get("roles")))
        .collect();
    let mut currents = Vec::new();
    for batch in refs.chunks(500) {
        currents.extend(
            crate::work_resource_current::read_work_resource_currents(tx, batch, &as_of).await?,
        );
    }
    let mut like_moments: Vec<String> = currents
        .iter()
        .filter_map(|c| c.like_source.observed_at.clone())
        .collect();
    like_moments.sort();
    like_moments.dedup();
    let like_displays: HashMap<String, String> = if like_moments.is_empty() {
        HashMap::new()
    } else {
        sqlx::query("SELECT moment,linggan_human_moment(moment::timestamptz) AS display FROM unnest($1::text[]) AS moment")
            .bind(&like_moments).fetch_all(&mut **tx).await?.into_iter().map(|r|(r.get("moment"),r.get("display"))).collect()
    };
    let analyses=sqlx::query("SELECT work_public_ref,result_json,manual_overrides,result_fingerprint,job_state,result_at::text AS result_at,linggan_human_moment(result_at) AS result_at_display,last_error_code FROM linggan_creator_discovery_work_analysis WHERE domain_ref=$1 AND work_public_ref=ANY($2)").bind(q.domain).bind(&refs).fetch_all(&mut **tx).await?;
    let analyses: HashMap<Uuid, _> = analyses
        .into_iter()
        .map(|r| (r.get("work_public_ref"), r))
        .collect();
    let profiles=sqlx::query("SELECT DISTINCT ON(author.author_external_id) author.author_external_id,jsonb_build_object('displayName',author.display_name,'biography',author.biography,'followerCount',author.follower_count,'sourceRef',author.material_ref,'observedAt',author.observed_at) AS profile FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package package USING(package_ref) WHERE author.platform=$1 AND package.accepted_at<=$2::timestamptz AND EXISTS(SELECT 1 FROM collection_work_order_lease_task lt JOIN collection_work_order_lease lease USING(lease_ref) JOIN collection_work_order_domain_usage usage USING(work_order_ref) WHERE lt.task_id=package.task_id AND usage.domain_ref=$3) ORDER BY author.author_external_id,author.observed_at::timestamptz DESC,author.created_at DESC,author.material_ref DESC").bind(&q.platform).bind(&as_of).bind(q.domain).fetch_all(&mut **tx).await?;
    let profiles: HashMap<String, Value> = profiles
        .into_iter()
        .map(|r| (r.get("author_external_id"), r.get("profile")))
        .collect();
    let mut profile_name_candidates = HashMap::new();
    let profile_names=sqlx::query("SELECT DISTINCT ON(author.author_external_id) author.author_external_id,author.display_name,author.material_ref FROM linggan_material_author_profile author JOIN linggan_runtime_capture_package package USING(package_ref) WHERE author.platform=$1 AND package.accepted_at<=$2::timestamptz AND NULLIF(btrim(author.display_name),'') IS NOT NULL AND EXISTS(SELECT 1 FROM collection_work_order_lease_task lt JOIN collection_work_order_lease lease USING(lease_ref) JOIN collection_work_order_domain_usage usage USING(work_order_ref) WHERE lt.task_id=package.task_id AND usage.domain_ref=$3) ORDER BY author.author_external_id,author.observed_at::timestamptz DESC,author.created_at DESC,author.material_ref DESC").bind(&q.platform).bind(&as_of).bind(q.domain).fetch_all(&mut **tx).await?;
    for row in profile_names {
        let author: String = row.get("author_external_id");
        profile_name_candidates.insert(
            creator_key(&q.platform, &author),
            (row.get("display_name"), row.get("material_ref")),
        );
    }
    sqlx::query("SAVEPOINT creator_observation_lookup")
        .execute(&mut **tx)
        .await?;
    let target_result=sqlx::query("SELECT target.identity_key,jsonb_build_object('targetRef',target.target_ref,'displayName',target.display_name,'inCurrentDomain',relation.target_ref IS NOT NULL,'usageRole',relation.role,'lifecycleState',target.lifecycle_state,'monitoringState',CASE WHEN target.lifecycle_state IN ('paused','dismissed') THEN target.lifecycle_state WHEN target.monitoring_enabled AND EXISTS(SELECT 1 FROM collection_monitor_rule rule JOIN collection_monitor_rule_revision revision ON revision.rule_revision_ref=rule.active_revision_ref WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL AND revision.automatic_enabled) THEN 'monitoring' ELSE 'not_enabled' END,'existsInOtherDomains',EXISTS(SELECT 1 FROM observation_domain_target other WHERE other.target_ref=target.target_ref AND other.domain_ref<>$1),'hasOtherPrimaryDomain',EXISTS(SELECT 1 FROM observation_domain_target other WHERE other.target_ref=target.target_ref AND other.domain_ref<>$1 AND other.role='primary')) AS observation FROM collection_observation_target target LEFT JOIN observation_domain_target relation ON relation.target_ref=target.target_ref AND relation.domain_ref=$1 WHERE target.target_kind='creator' AND target.platform=$2").bind(q.domain).bind(&q.platform).fetch_all(&mut **tx).await;
    let (targets, observation_lookup_available) = match target_result {
        Ok(rows) => {
            sqlx::query("RELEASE SAVEPOINT creator_observation_lookup")
                .execute(&mut **tx)
                .await?;
            (rows, true)
        }
        Err(_) => {
            sqlx::query("ROLLBACK TO SAVEPOINT creator_observation_lookup")
                .execute(&mut **tx)
                .await?;
            sqlx::query("RELEASE SAVEPOINT creator_observation_lookup")
                .execute(&mut **tx)
                .await?;
            if q.observation.is_some() {
                return Err(DiscoveryError::Invalid("observation_lookup_unavailable"));
            }
            (Vec::new(), false)
        }
    };
    let targets: HashMap<String, Value> = targets
        .into_iter()
        .map(|r| (r.get("identity_key"), r.get("observation")))
        .collect();
    let focus_rows=sqlx::query("SELECT author_external_id,result_json,manual_overrides,result_fingerprint,job_state,result_at::text AS result_at,linggan_human_moment(result_at) AS result_at_display,last_error_code FROM linggan_creator_discovery_author_analysis WHERE domain_ref=$1 AND platform=$2").bind(q.domain).bind(&q.platform).fetch_all(&mut **tx).await?;
    let focus_rows: HashMap<
        String,
        (
            Value,
            Value,
            Option<String>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        ),
    > = focus_rows
        .into_iter()
        .map(|r| {
            (
                r.get("author_external_id"),
                (
                    r.get("result_json"),
                    r.get("manual_overrides"),
                    r.get("result_fingerprint"),
                    r.get("job_state"),
                    r.get("last_error_code"),
                    r.get("result_at"),
                    r.get("result_at_display"),
                ),
            )
        })
        .collect();
    let broad_refs:Vec<Uuid>=sqlx::query_scalar("SELECT DISTINCT finding.content_public_ref FROM linggan_material_discovery_finding finding JOIN linggan_runtime_capture_package package USING(package_ref) WHERE finding.content_public_ref=ANY($1) AND package.package_kind='profile_discovery' AND package.accepted_at<=$2::timestamptz AND EXISTS(SELECT 1 FROM collection_work_order_lease_task lt JOIN collection_work_order_lease lease USING(lease_ref) JOIN collection_work_order_domain_usage usage USING(work_order_ref) WHERE lt.task_id=package.task_id AND usage.domain_ref=$3)").bind(&refs).bind(&as_of).bind(q.domain).fetch_all(&mut **tx).await?;
    let mut works = Vec::new();
    let mut work_name_sources = HashMap::new();
    let mut current_iter = currents.into_iter().filter(|c| c.platform == q.platform);
    loop {
        let batch: Vec<_> = current_iter.by_ref().take(100).collect();
        if batch.is_empty() {
            break;
        }
        let work_refs: Vec<_> = batch.iter().map(|current| current.public_ref).collect();
        let mut dossiers = crate::research_dossier::read_in(tx, &work_refs, &as_of).await?;
        for c in batch {
            let dossier = dossiers
                .remove(&c.public_ref)
                .ok_or(DiscoveryError::Invalid("dossier_unavailable"))?;
            let search_text = dossier
                .fragments
                .iter()
                .map(|f| f.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let reading_title = crate::research_dossier::reading_title(&dossier);
            let display_title = dossier.note["displayTitle"].as_str().map(str::to_owned);
            let fragments = dossier.fragments;
            let fingerprint=hash(&json!({"domain":{ "id":domain["domainRef"],"name":domain["name"],"description":domain["description"],"researchGoal":domain["researchGoal"]},"fragments":fragments,"readingTitle":reading_title,"rule":RULE_VERSION,"config":policy["configRef"]}).to_string());
            let mut result = json!({});
            let mut manual = json!({});
            let mut state = "not_analyzed".to_string();
            let mut error_code: Option<String> = None;
            let mut result_at: Option<String> = None;
            let mut result_at_display: Option<String> = None;
            if let Some(a) = analyses.get(&c.public_ref) {
                state = a.get("job_state");
                error_code = a.get("last_error_code");
                manual = a.get("manual_overrides");
                if a.get::<Option<String>, _>("result_fingerprint").as_deref() == Some(&fingerprint)
                {
                    result = a.get("result_json");
                    result_at = a.get("result_at");
                    result_at_display = a.get("result_at_display");
                }
            }
            let allowed: HashSet<String> =
                fragments.iter().map(|f| f.fragment_id.clone()).collect();
            for field in [
                "relevance",
                "personal_experience",
                "professional_output",
                "explicit_promotion",
            ] {
                project_manual_field(&mut manual, field, &allowed);
            }
            let relevance = effective_value(&result, &manual, "relevance");
            let relevance = relevance
                .get("value")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_owned();
            let mut traits = Vec::new();
            for name in [
                "personal_experience",
                "professional_output",
                "explicit_promotion",
            ] {
                let value = manual
                    .get(name)
                    .and_then(|v| v.get("value"))
                    .or_else(|| result.get("traits").and_then(|v| v.get(name)));
                if value.and_then(|v| v.get("value")).and_then(Value::as_str) == Some("yes") {
                    traits.push(name.into());
                }
            }
            let hints = result
                .get("topicHints")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v["text"].as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let profile = c
                .author_external_id
                .as_ref()
                .and_then(|a| profiles.get(a))
                .cloned()
                .unwrap_or(Value::Null);
            let current_name = c
                .creator_display_name
                .as_deref()
                .filter(|name| !name.trim().is_empty());
            let profile_name = profile["displayName"]
                .as_str()
                .filter(|name| !name.trim().is_empty());
            if current_name.is_some() {
                work_name_sources.insert(c.public_ref, json!({"kind":"accepted_work","workRef":c.public_ref,"sourceRef":c.creator_source.material_ref}));
            } else if profile_name.is_some() {
                work_name_sources.insert(
                    c.public_ref,
                    json!({"kind":"accepted_author_profile","sourceRef":profile["sourceRef"]}),
                );
            }
            let observation = if observation_lookup_available {
                c.author_external_id.as_ref().and_then(|a|targets.get(a)).cloned().unwrap_or(json!({"inCurrentDomain":false,"monitoringState":"not_enabled","existsInOtherDomains":false,"hasOtherPrimaryDomain":false,"targetRef":null,"usageRole":null,"lookupState":"available"}))
            } else {
                json!({"inCurrentDomain":null,"monitoringState":null,"existsInOtherDomains":null,"hasOtherPrimaryDomain":null,"targetRef":null,"usageRole":null,"lookupState":"unavailable"})
            };
            let focus = c
                .author_external_id
                .as_ref()
                .and_then(|a| focus_rows.get(a))
                .map(|(_, m, _, _, _, _, _)| effective_value(&json!({}), m, "focus"))
                .unwrap_or(json!({"value":"unknown"}));
            let mut evidence = Vec::new();
            for f in &fragments {
                if result.to_string().contains(&f.fragment_id) {
                    evidence.push(json!({"fragmentId":f.fragment_id,"sourceType":"work_material","workRef":c.public_ref,"sourceRef":f.source_ref,"field":f.field,"text":f.text.chars().take(200).collect::<String>(),"start":f.start,"end":f.end,"sourceVersion":f.source_version}));
                }
            }
            let support_fragments=fragments.iter().map(|f|json!({"fragmentId":f.fragment_id,"field":f.field,"sourceType":"work_material","workRef":c.public_ref,"sourceRef":f.source_ref,"text":f.text.chars().take(120).collect::<String>()})).collect::<Vec<_>>();
            let likes_observed_display = c
                .like_source
                .observed_at
                .as_ref()
                .and_then(|m| like_displays.get(m))
                .cloned();
            works.push(DiscoveryWork {
            work_ref: c.public_ref,
            usage_roles: roles[&c.public_ref].clone(),
            creator_key: c
                .author_external_id
                .as_ref()
                .map(|a| creator_key(&c.platform, a)),
            platform: c.platform,
            author_external_id: c.author_external_id,
            display_name: current_name.map(str::to_owned).or_else(|| profile_name.map(str::to_owned)),
            title: display_title,
            reading_title,
            published_date: c.published_local_date,
            published_epoch: c.published_at_epoch_ms,
            likes: c.like_count,
            likes_observed_at: c.like_source.observed_at,
            likes_observed_display,
            comments: c.comment_count,
            collects: c.collect_count,
            first_added: added[&c.public_ref].clone(),
            first_added_display: added_display[&c.public_ref].clone(),
            fingerprint,
            search_text,
            fragments,
            relevance,
            traits,
            topic_hints: hints,
            analysis: json!({"automatic":result,"manual":manual,"evidence":evidence,"supportFragments":support_fragments,"state":state,"lastErrorCode":error_code,"resultAt":result_at,"resultAtDisplay":result_at_display}),
            analysis_state: state,
            observation,
            profile,
            focus,
            author_analysis: json!({}),
            acquisition_kind: if broad_refs.contains(&c.public_ref){"profile_discovery"}else{"limited_domain_sample"}.into(),
        });
        }
    }
    let author_keys: std::collections::HashSet<_> = works
        .iter()
        .filter_map(|w| w.author_external_id.clone())
        .collect();
    for author in author_keys {
        let scoped: Vec<_> = works
            .iter()
            .filter(|w| w.author_external_id.as_deref() == Some(&author))
            .collect();
        let profile = scoped
            .first()
            .map(|w| w.profile.clone())
            .unwrap_or(Value::Null);
        let fingerprint = focus_fingerprint(&profile, &scoped, &policy["configRef"]);
        if let Some((result, manual, stored, state, error_code, result_at, result_at_display)) =
            focus_rows.get(&author)
        {
            let focus_fragments = author_support_fragments(&scoped, "focus");
            let identity_fragments = author_support_fragments(&scoped, "institution_or_brand");
            let mut projected_manual = manual.clone();
            let focus_ids: HashSet<String> = focus_fragments
                .iter()
                .filter_map(|v| v["fragmentId"].as_str().map(str::to_owned))
                .collect();
            let identity_ids: HashSet<String> = identity_fragments
                .iter()
                .filter_map(|v| v["fragmentId"].as_str().map(str::to_owned))
                .collect();
            project_manual_field(&mut projected_manual, "focus", &focus_ids);
            project_manual_field(&mut projected_manual, "institution_or_brand", &identity_ids);
            let effective = effective_value(
                if stored.as_deref() == Some(&fingerprint) {
                    result
                } else {
                    &Value::Null
                },
                &projected_manual,
                "focus",
            );
            let mut automatic = if stored.as_deref() == Some(&fingerprint) {
                result.clone()
            } else {
                json!({})
            };
            automatic["evidence"] = json!(current_author_evidence(&automatic, &focus_fragments));
            let identity = effective_value(&automatic, &projected_manual, "institution_or_brand");
            for w in works
                .iter_mut()
                .filter(|w| w.author_external_id.as_deref() == Some(&author))
            {
                w.focus = effective.clone();
                w.author_analysis = json!({"automatic":automatic,"manual":projected_manual,"institution_or_brand":identity,"supportFragments":focus_fragments,"identitySupportFragments":identity_fragments,"state":state,"lastErrorCode":error_code,"resultAt":if stored.as_deref()==Some(&fingerprint){result_at.clone()}else{None},"resultAtDisplay":if stored.as_deref()==Some(&fingerprint){result_at_display.clone()}else{None}});
            }
        }
    }
    Ok(DiscoveryData {
        works,
        as_of,
        display_as_of,
        now_epoch,
        domain,
        policy,
        observation_lookup_available,
        work_name_sources,
        profile_name_candidates,
    })
}
pub fn in_base(w: &DiscoveryWork, q: &CreatorScope, now: i64) -> bool {
    if w.platform != q.platform || !w.usage_roles.contains(&q.usage_role) {
        return false;
    }
    if q.published_from.is_none() && q.published_to.is_none() {
        return true;
    }
    w.published_date
        .as_ref()
        .zip(w.published_epoch)
        .is_some_and(|(d, t)| {
            t <= now
                && q.published_from.as_ref().is_none_or(|s| d >= s)
                && q.published_to.as_ref().is_none_or(|e| d < e)
        })
}
pub fn work_matches(w: &DiscoveryWork, q: &CreatorScope, threshold: Option<i64>) -> bool {
    if q.unknown_author && w.creator_key.is_some() {
        return false;
    }
    if q.creator_key
        .as_ref()
        .is_some_and(|k| w.creator_key.as_ref() != Some(k))
    {
        return false;
    }
    if q.relevance.as_ref().is_some_and(|r| &w.relevance != r) {
        return false;
    }
    if q.viral && (w.relevance != "related" || !w.likes.zip(threshold).is_some_and(|(n, t)| n >= t))
    {
        return false;
    }
    if q.high_likes && (w.relevance == "unrelated" || w.likes.is_none()) {
        return false;
    }
    if q.min_likes.is_some_and(|n| w.likes.is_none_or(|v| v < n)) {
        return false;
    }
    if q.topic_hint
        .as_ref()
        .is_some_and(|hint| !w.topic_hints.iter().any(|value| value == hint))
    {
        return false;
    }
    if let Some(traits) = &q.traits {
        if !traits.split(',').any(|t| {
            if t == "institution_or_brand" {
                w.author_analysis["institution_or_brand"]["value"] == "yes"
            } else {
                w.relevance == "related" && w.traits.iter().any(|x| x == t)
            }
        }) {
            return false;
        }
    }
    if q.search_mode == "work" {
        if let Some(text) = &q.query {
            let text = text.to_lowercase();
            if !w.search_text.to_lowercase().contains(&text)
                && !w
                    .topic_hints
                    .iter()
                    .any(|h| h.to_lowercase().contains(&text))
            {
                return false;
            }
        }
    }
    true
}
fn author_display(
    data: &DiscoveryData,
    latest: &DiscoveryWork,
    works: &[&DiscoveryWork],
) -> (Option<String>, Value) {
    if let Some(named) = works
        .iter()
        .filter(|work| {
            work.display_name
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty())
        })
        .max_by_key(|work| (&work.first_added, work.work_ref))
    {
        return (
            named.display_name.clone(),
            data.work_name_sources
                .get(&named.work_ref)
                .cloned()
                .unwrap_or(Value::Null),
        );
    }
    if let Some(candidate) = latest
        .creator_key
        .as_ref()
        .and_then(|key| data.profile_name_candidates.get(key))
    {
        return (
            Some(candidate.0.clone()),
            json!({"kind":"accepted_author_profile","sourceRef":candidate.1}),
        );
    }
    let name = latest.observation["displayName"]
        .as_str()
        .filter(|name| !name.trim().is_empty())
        .map(str::to_owned);
    let source = if name.is_some() {
        json!({"kind":"observation_target","targetRef":latest.observation["targetRef"]})
    } else {
        Value::Null
    };
    (name, source)
}
fn author_matches(
    data: &DiscoveryData,
    latest: &DiscoveryWork,
    works: &[&DiscoveryWork],
    q: &CreatorScope,
) -> bool {
    let outside = latest.observation["inCurrentDomain"] == false;
    if q.observation.as_deref().is_some_and(|s| match s {
        "outside" => !outside,
        "inside" => outside,
        "monitoring" => outside || latest.observation["monitoringState"] != "monitoring",
        "paused" => outside || latest.observation["monitoringState"] != "paused",
        "other_domains_only" => !outside || latest.observation["existsInOtherDomains"] != true,
        "dismissed" => outside || latest.observation["monitoringState"] != "dismissed",
        _ => true,
    }) {
        return false;
    }
    if q.focus.as_deref().is_some_and(|f| {
        latest.focus["value"].as_str().unwrap_or("unknown") != f
            || (f == "vertical_tendency" && !works.iter().any(|w| w.relevance == "related"))
    }) {
        return false;
    }
    if q.search_mode == "author" {
        if let Some(text) = &q.query {
            let text = text.to_lowercase();
            let display_name = author_display(data, latest, works).0;
            if ![
                display_name.as_deref(),
                latest.author_external_id.as_deref(),
                latest.profile["biography"].as_str(),
            ]
            .into_iter()
            .flatten()
            .any(|value| value.to_lowercase().contains(&text))
            {
                return false;
            }
        }
    }
    true
}
/// The precise evidence-library backlink uses the same author and same-work predicates as rows.
pub fn matching_work_refs(data: &DiscoveryData, q: &CreatorScope) -> Vec<Uuid> {
    let mut authors: BTreeMap<&str, Vec<&DiscoveryWork>> = BTreeMap::new();
    let mut unknown = Vec::new();
    for work in data.works.iter().filter(|w| in_base(w, q, data.now_epoch)) {
        if let Some(key) = work.creator_key.as_deref() {
            authors.entry(key).or_default().push(work);
        } else if q.unknown_author {
            unknown.push(work);
        }
    }
    let threshold = data.policy["likeThreshold"].as_i64();
    let mut refs = unknown
        .into_iter()
        .filter(|w| work_matches(w, q, threshold))
        .map(|w| w.work_ref)
        .collect::<Vec<_>>();
    if q.unknown_author {
        return refs;
    }
    for works in authors.values() {
        let latest = works
            .iter()
            .max_by_key(|w| &w.first_added)
            .expect("nonempty author");
        if author_matches(data, latest, works, q) {
            refs.extend(
                works
                    .iter()
                    .filter(|w| work_matches(w, q, threshold))
                    .map(|w| w.work_ref),
            );
        }
    }
    refs
}
pub fn aggregate(data: &DiscoveryData, q: &CreatorScope) -> Value {
    let threshold = data.policy["likeThreshold"].as_i64();
    let base: Vec<_> = data
        .works
        .iter()
        .filter(|w| in_base(w, q, data.now_epoch))
        .collect();
    let mut authors: BTreeMap<&str, Vec<&DiscoveryWork>> = BTreeMap::new();
    for w in &base {
        if let Some(k) = &w.creator_key {
            authors.entry(k).or_default().push(w);
        }
    }
    let mut stats = json!({"authors":authors.len(),"works":base.len(),"unknownAuthorWorks":base.iter().filter(|w|w.creator_key.is_none()).count(),"relatedAuthors":0,"vertical":0,"personal":0,"outside":if data.observation_lookup_available {Some(0)}else{None},"outsideViral":if data.observation_lookup_available {threshold.map(|_|0)}else{None},"unknownDateWorks":data.works.iter().filter(|w|w.published_date.is_none() && w.usage_roles.contains(&q.usage_role) && w.platform==q.platform).count()});
    let mut items = Vec::new();
    for (key, ws) in authors {
        let latest = *ws
            .iter()
            .max_by_key(|w| &w.first_added)
            .expect("nonempty author");
        let outside =
            data.observation_lookup_available && latest.observation["inCurrentDomain"] == false;
        let related: Vec<_> = ws.iter().filter(|w| w.relevance == "related").collect();
        let vertical = latest.focus["value"] == "vertical_tendency" && !related.is_empty();
        let personal = related
            .iter()
            .any(|w| w.traits.iter().any(|t| t == "personal_experience"));
        let viral = related
            .iter()
            .filter(|w| w.likes.zip(threshold).is_some_and(|(l, t)| l >= t))
            .count();
        for (field, yes) in [
            ("relatedAuthors", !related.is_empty()),
            ("vertical", vertical),
            ("personal", personal),
            ("outside", data.observation_lookup_available && outside),
            (
                "outsideViral",
                data.observation_lookup_available && outside && viral > 0,
            ),
        ] {
            if yes {
                stats[field] = json!(stats[field].as_u64().unwrap_or(0) + 1);
            }
        }
        if !author_matches(data, latest, &ws, q) {
            continue;
        }
        let mut candidates: Vec<_> = ws
            .iter()
            .copied()
            .filter(|w| work_matches(w, q, threshold))
            .collect();
        if candidates.is_empty() {
            continue;
        }
        candidates.sort_by(|a, b| {
            let rank = |w: &DiscoveryWork| {
                if w.relevance == "related" {
                    0
                } else if w.relevance == "unknown" {
                    1
                } else {
                    2
                }
            };
            let fallback = || {
                rank(a)
                    .cmp(&rank(b))
                    .then_with(|| b.first_added.cmp(&a.first_added))
                    .then_with(|| a.work_ref.cmp(&b.work_ref))
            };
            let eligible = |w: &DiscoveryWork| match q.sort.as_str() {
                "high_likes" => w.relevance != "unrelated" && w.likes.is_some(),
                "viral_works" => {
                    w.relevance == "related" && w.likes.zip(threshold).is_some_and(|(l, t)| l >= t)
                }
                _ => false,
            };
            if matches!(q.sort.as_str(), "high_likes" | "viral_works") {
                let a_eligible = eligible(a);
                let b_eligible = eligible(b);
                b_eligible
                    .cmp(&a_eligible)
                    .then_with(|| {
                        if a_eligible && b_eligible {
                            b.likes.cmp(&a.likes)
                        } else {
                            std::cmp::Ordering::Equal
                        }
                    })
                    .then_with(fallback)
            } else if q.high_likes || q.viral {
                b.likes.cmp(&a.likes).then_with(fallback)
            } else {
                fallback()
            }
        });
        let refs: Vec<_> = candidates.iter().map(|w| w.work_ref).collect();
        let matched_related = candidates
            .iter()
            .filter(|w| w.relevance == "related")
            .count();
        let matched_viral = candidates
            .iter()
            .filter(|w| {
                w.relevance == "related" && w.likes.zip(threshold).is_some_and(|(l, t)| l >= t)
            })
            .count();
        let max_related = related.iter().filter_map(|w| w.likes).max();
        let high = candidates
            .iter()
            .filter(|w| w.relevance != "unrelated")
            .filter_map(|w| w.likes)
            .max();
        let mut traits: Vec<_> = related.iter().flat_map(|w| w.traits.clone()).collect();
        if latest.author_analysis["institution_or_brand"]["value"] == "yes" {
            traits.push("institution_or_brand".into());
        }
        traits.sort();
        traits.dedup();
        let mut hints: Vec<_> = related.iter().flat_map(|w| w.topic_hints.clone()).collect();
        hints.sort();
        hints.dedup();
        if let Some(selected) = &q.topic_hint {
            // The exact same-work filter may match a phrase beyond the default two-item summary.
            hints.retain(|hint| hint != selected);
            hints.insert(0, selected.clone());
        }
        hints.truncate(2);
        let (display_name, display_name_source) = author_display(data, latest, &ws);
        items.push(json!({"creatorKey":key,"displayName":display_name,"displayNameSource":display_name_source,"authorExternalId":latest.author_external_id,"platform":latest.platform,"profile":latest.profile,"focus":latest.focus,"authorAnalysis":latest.author_analysis,"traits":traits,"topicHints":hints,"observation":latest.observation,"collectedWorkCount":ws.len(),"relatedWorkCount":related.len(),"matchedRelatedWorkCount":matched_related,"matchedViralWorkCount":threshold.map(|_|matched_viral),"unrelatedWorkCount":ws.iter().filter(|w|w.relevance=="unrelated").count(),"unknownRelevanceWorkCount":ws.iter().filter(|w|w.relevance=="unknown").count(),"knownLikeWorkCount":related.iter().filter(|w|w.likes.is_some()).count(),"viralWorkCount":threshold.map(|_|viral),"maxRelatedLikeCount":max_related,"candidateHighLikeCount":high,"matchedWorkRefs":refs,"representatives":candidates.iter().take(3).collect::<Vec<_>>(),"firstAdded":latest.first_added,"firstAddedDisplay":latest.first_added_display}));
    }
    items.sort_by(|a, b| {
        match q.sort.as_str() {
            "related_works" => b["matchedRelatedWorkCount"]
                .as_u64()
                .cmp(&a["matchedRelatedWorkCount"].as_u64()),
            "viral_works" => b["matchedViralWorkCount"]
                .as_u64()
                .cmp(&a["matchedViralWorkCount"].as_u64()),
            "high_likes" => b["candidateHighLikeCount"]
                .as_i64()
                .cmp(&a["candidateHighLikeCount"].as_i64()),
            _ => b["firstAdded"].as_str().cmp(&a["firstAdded"].as_str()),
        }
        .then_with(|| a["creatorKey"].as_str().cmp(&b["creatorKey"].as_str()))
    });
    let total = items.len();
    let items: Vec<_> = items
        .into_iter()
        .skip(q.page * q.page_size)
        .take(q.page_size)
        .collect();
    json!({"items":items,"total":total,"stats":stats,"policy":data.policy,"domain":data.domain,"asOf":data.as_of,"displayAsOf":data.display_as_of,"observationLookupState":if data.observation_lookup_available {"available"}else{"unavailable"},"page":q.page,"pageSize":q.page_size})
}

/// Semantic-only author input identity, shared by scheduling and reads.
pub fn focus_fingerprint(profile: &Value, works: &[&DiscoveryWork], config: &Value) -> String {
    let mut sources: Vec<_> = works
        .iter()
        .map(|w| {
            (
                w.work_ref,
                &w.fingerprint,
                &w.relevance,
                &w.topic_hints,
                &w.acquisition_kind,
            )
        })
        .collect();
    sources.sort_by_key(|x| x.0);
    hash(&json!({"biography":profile["biography"],"works":sources,"rule":RULE_VERSION,"config":config}).to_string())
}

#[cfg(test)]
mod topic_material_tests {
    use super::*;
    #[test]
    fn complete_unicode_material_keeps_tail_and_exact_source_identity() {
        let work = Uuid::from_u128(1);
        let source = Uuid::from_u128(2);
        let text = format!("{}关键反例出现在结尾。", "合成材料🙂\n".repeat(1500));
        let full = fragment(work, "body", Some(&text), Some(source), Some("v1")).unwrap();
        assert_eq!(full.text, text);
        assert_eq!(full.end, text.chars().count());
        assert_eq!(full.source_version, "v1");
        let later = fragment(
            work,
            "body",
            Some(&text),
            Some(Uuid::from_u128(3)),
            Some("v2"),
        )
        .unwrap();
        assert_ne!(full.fragment_id, later.fragment_id);
        assert_ne!(full.source_ref, later.source_ref);
    }
}
