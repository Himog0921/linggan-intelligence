//! Disposable, bounded page projections. Authority remains in canonical readers.
//! Version fingerprints include deletes from the canonical readers’ source tables.
use super::{
    TopicMapError, TopicMapQuery, TopicMapSnapshot,
    read::{QualifiedTopicMap, SourceScope, qualify_in, validate_query},
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub struct TopicMapReadModel {
    entries: Mutex<BTreeMap<SourceScope, Arc<Entry>>>,
    builds: Arc<Mutex<()>>,
}
const REFRESH_AFTER: Duration = Duration::from_secs(60);
const MAX_PAGE_AGE: Duration = Duration::from_secs(300);

struct Entry {
    revision: SourceRevision,
    created: Instant,
    qualified: QualifiedTopicMap,
    views: Mutex<VecDeque<(String, Arc<TopicMapSnapshot>)>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SourceRevision {
    schema: String,
    tables: BTreeMap<String, String>,
    topics: BTreeMap<String, String>,
    append_only_rows: BTreeMap<String, BTreeSet<String>>,
    reading_authority: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
enum SnapshotReuse {
    QualifiedPage(Option<Uuid>),
    Current,
    Rebuild,
}

impl Entry {
    fn can_reuse(&self, revision: &SourceRevision, policy: SnapshotReuse) -> bool {
        match policy {
            SnapshotReuse::QualifiedPage(topic) => {
                self.created.elapsed() < MAX_PAGE_AGE
                    && self.revision.same_source_authority(revision)
                    && topic.is_none_or(|topic| {
                        let topic = topic.to_string();
                        self.revision.topics.contains_key(&topic)
                            || !revision.topics.contains_key(&topic)
                    })
            }
            SnapshotReuse::Current => {
                self.created.elapsed() < REFRESH_AFTER && self.revision == *revision
            }
            SnapshotReuse::Rebuild => false,
        }
    }

    async fn project(&self, q: &TopicMapQuery) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        let mut normalized = q.clone();
        normalized.reference_window_days = Some(q.reference_window_days.unwrap_or(30));
        let key = serde_json::to_string(&normalized)
            .map_err(|error| TopicMapError::Source(error.to_string()))?;
        let mut views = self.views.lock().await;
        if let Some((_, view)) = views.iter().find(|(cached, _)| *cached == key) {
            return Ok(Arc::clone(view));
        }
        let view = Arc::new(self.qualified.project(q)?);
        if views.len() >= 4 {
            views.pop_front();
        }
        views.push_back((key, Arc::clone(&view)));
        Ok(view)
    }
}

impl TopicMapReadModel {
    async fn reusable(
        &self,
        scope: &SourceScope,
        revision: &SourceRevision,
        policy: SnapshotReuse,
    ) -> Option<Arc<Entry>> {
        self.entries
            .lock()
            .await
            .get(scope)
            .filter(|entry| entry.can_reuse(revision, policy))
            .cloned()
    }

    /// The same qualified source revision serves overview and every topic view.
    /// Page readers retain a still-qualified projection while new data is loaded.
    pub async fn read_page(
        self: &Arc<Self>,
        db: &Database,
        q: &TopicMapQuery,
    ) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        validate_query(q)?;
        let scope = SourceScope::from_query(q);
        let revision = source_revision(db).await?;
        let policy = SnapshotReuse::QualifiedPage(q.topic_ref);
        if let Some(entry) = self.reusable(&scope, &revision, policy).await {
            if (entry.created.elapsed() >= REFRESH_AFTER || entry.revision != revision)
                && let Ok(build) = Arc::clone(&self.builds).try_lock_owned()
            {
                let model = Arc::clone(self);
                let db = db.clone();
                tokio::spawn(async move {
                    let _build = build;
                    if let Err(error) = model.build(&db, scope, SnapshotReuse::Rebuild).await {
                        eprintln!("topic map background refresh deferred: {error}");
                    }
                });
            }
            return entry.project(q).await;
        }
        let _build = self.builds.lock().await;
        self.build(db, scope, policy).await?.project(q).await
    }

    /// Current authority is validated before every reuse, including evidence opens.
    pub async fn read(
        &self,
        db: &Database,
        q: &TopicMapQuery,
    ) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        validate_query(q)?;
        let scope = SourceScope::from_query(q);
        let revision = source_revision(db).await?;
        let policy = SnapshotReuse::Current;
        if let Some(entry) = self.reusable(&scope, &revision, policy).await {
            return entry.project(q).await;
        }
        let _build = self.builds.lock().await;
        self.build(db, scope, policy).await?.project(q).await
    }

    /// Refresh atomically; requests may use the previous still-qualified version.
    pub async fn refresh(&self, db: &Database, q: &TopicMapQuery) -> Result<(), TopicMapError> {
        validate_query(q)?;
        let _build = self.builds.lock().await;
        self.build(db, SourceScope::from_query(q), SnapshotReuse::Rebuild)
            .await?;
        Ok(())
    }

    /// Keep the default domain warm through idle periods. A warm-up never
    /// competes with another build, and readers still verify authority.
    pub async fn warm_default_domain(
        &self,
        db: &Database,
        domain_ref: uuid::Uuid,
    ) -> Result<(), TopicMapError> {
        let Ok(_build) = self.builds.try_lock() else {
            return Ok(());
        };
        let scope = SourceScope::from_query(&TopicMapQuery {
            domain_ref: Some(domain_ref),
            ..Default::default()
        });
        let revision = source_revision(db).await?;
        let warm = {
            let entries = self.entries.lock().await;
            entries.get(&scope).is_some_and(|entry| {
                entry.revision == revision
                    && entry.created.elapsed() < MAX_PAGE_AGE - Duration::from_secs(120)
            })
        };
        if !warm {
            self.build(db, scope, SnapshotReuse::Rebuild).await?;
        }
        Ok(())
    }

    async fn build(
        &self,
        db: &Database,
        scope: SourceScope,
        policy: SnapshotReuse,
    ) -> Result<Arc<Entry>, TopicMapError> {
        for _ in 0..2 {
            let mut tx = db.pool().begin().await?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut *tx)
                .await?;
            let revision = source_revision_in(&mut tx).await?;
            if let Some(entry) = self.reusable(&scope, &revision, policy).await {
                tx.commit().await?;
                return Ok(entry);
            }
            let qualified = qualify_in(&mut tx, &scope, Some(db)).await?;
            tx.commit().await?;
            let after = source_revision(db).await?;
            if revision.same_source_authority(&after) {
                let mut entries = self.entries.lock().await;
                entries.retain(|_, entry| entry.created.elapsed() < MAX_PAGE_AGE);
                if entries.len() >= 4
                    && !entries.contains_key(&scope)
                    && let Some(oldest) = entries
                        .iter()
                        .min_by_key(|(_, entry)| entry.created)
                        .map(|(scope, _)| scope.clone())
                {
                    entries.remove(&oldest);
                }
                let entry = Arc::new(Entry {
                    revision,
                    created: Instant::now(),
                    qualified,
                    views: Mutex::default(),
                });
                entries.insert(scope, Arc::clone(&entry));
                return Ok(entry);
            }
        }
        Err(TopicMapError::Conflict)
    }
}

impl SourceRevision {
    /// Research receipts and assignments can change a presentation, but do not carry
    /// material, privacy or definition authority. Other changes are checked conservatively.
    fn same_source_authority(&self, newer: &Self) -> bool {
        fn carries_authority(table: &str) -> bool {
            !matches!(
                table,
                "linggan_topic_research_dispatch"
                    | "linggan_topic_research_run"
                    | "linggan_topic_research_window"
                    | "linggan_topic_research_stage"
                    | "linggan_topic_research_stage_result"
                    | "linggan_topic_research_result"
                    | "linggan_topic_map_discussion_unit"
                    | "linggan_topic_map_unit_resolution"
                    | "linggan_topic_map_unit_assignment"
                    | "linggan_topic_map_concept_relation"
                    | "linggan_topic_map_receipt"
                    | "linggan_topic_workspace"
                    | "linggan_topic_definition"
                    | "linggan_topic_map_binding"
                    | "linggan_topic_map_concept_rule"
            )
        }
        self.schema == newer.schema
            && self.reading_authority == newer.reading_authority
            && self
                .topics
                .iter()
                .all(|(topic, version)| newer.topics.get(topic) == Some(version))
            && self
                .tables
                .iter()
                .filter(|(table, _)| {
                    carries_authority(table) && !self.append_only_rows.contains_key(*table)
                })
                .eq(newer.tables.iter().filter(|(table, _)| {
                    carries_authority(table) && !newer.append_only_rows.contains_key(*table)
                }))
            && self.append_only_rows.iter().all(|(table, old)| {
                newer
                    .append_only_rows
                    .get(table)
                    .is_some_and(|new| old.is_subset(new))
            })
    }
}

/// Ordinary collection and L1 completion append facts. A dated page can keep
/// serving them at its original cutoff while the next version is built.
/// Updates, deletes, and policy or revision-control changes invalidate it.
fn additive_facts(table: &str) -> bool {
    matches!(
        table,
        "linggan_material_content"
            | "linggan_material_content_detail"
            | "linggan_material_discovery_finding"
            | "linggan_material_author_profile"
            | "linggan_material_content_author"
            | "linggan_material_comment"
            | "linggan_material_comment_engagement"
            | "linggan_material_comment_platform_time"
            | "linggan_material_comment_first_admission"
            | "linggan_material_domain_usage"
            | "linggan_material_lane_observation"
            | "linggan_material_content_hashtag"
            | "linggan_material_hashtag_projection"
            | "linggan_runtime_attempt"
            | "linggan_runtime_task"
            | "linggan_runtime_capture_package"
            | "linggan_runtime_submission_receipt"
            | "linggan_runtime_record_disposition"
            | "collection_work_order_domain_usage"
            | "collection_acquisition_request"
            | "collection_admission_decision"
            | "linggan_content_understanding_request"
            | "linggan_content_understanding_result"
            | "linggan_content_understanding_offline_recovery"
            | "linggan_creator_account_research_request"
            | "linggan_creator_account_research_result"
            | "linggan_creator_research_request"
            | "linggan_creator_research_result"
            | "linggan_historical_image_inventory"
            | "linggan_material_media_origin"
            | "linggan_material_media_candidate"
            | "linggan_material_derived_text"
            | "linggan_media_blob"
            | "linggan_media_derivative"
            | "linggan_media_download_attempt"
            | "linggan_media_materialization"
            | "linggan_media_observation"
            | "linggan_media_processing_job"
            | "linggan_media_processing_job_event"
            | "linggan_media_resource_relation"
            | "linggan_media_slot"
            | "linggan_topic_classification_run"
            | "linggan_topic_map_membership"
            | "linggan_topic_material_member"
    )
}

// Cloud OCR may append a historical reading without replacing the selected
// current source. Once selected, it again participates in strict authority.
const ADDITIVE_READING: &str = "
    r.publication='automatic' AND r.source_state<>'restricted' AND (
        r.producer_kind='platform' OR (
            r.producer_kind='cloud_vision' AND NOT EXISTS (
                SELECT 1 FROM linggan_material_reading_selection s
                WHERE s.unit_ref=r.unit_ref AND s.reading_ref=r.reading_ref
            )
        )
    )";

// The automatic selection ledger certifies the native body pointer. As long
// as the text status stays the same, a newly captured platform reading can
// advance that pointer without invalidating the previous dated projection.
const NATIVE_BODY_POINTER: &str = "
    r.kind='body' AND r.adoption='automatic' AND r.source_state IN ('present','known_empty')
    AND EXISTS (
        SELECT 1 FROM linggan_material_reading_selection s
        JOIN linggan_material_reading reading ON reading.reading_ref=s.reading_ref
        WHERE s.unit_ref=r.unit_ref AND s.revision=r.revision AND s.event='automatic'
          AND s.reading_ref=r.current_reading_ref AND s.source_version=r.source_version
          AND s.source_state=r.source_state AND reading.producer_kind='platform'
          AND reading.publication='automatic'
    )";
const ADDITIVE_SELECTION: &str = "
    r.source_state<>'restricted' AND (
        r.event IN ('initialization','automatic') OR
        (r.event='source_changed' AND EXISTS (
            SELECT 1 FROM linggan_material_reading_selection selected
            JOIN linggan_material_reading reading ON reading.reading_ref=selected.reading_ref
            WHERE selected.unit_ref=r.unit_ref AND selected.revision=r.revision+1
              AND selected.event='automatic' AND selected.source_version=r.source_version
              AND selected.source_state=r.source_state AND reading.producer_kind='platform'
              AND reading.publication='automatic'
        ))
    )";

/// The shared material, dossier, research and Operations readers supply the page.
/// Rows in other projections and worker ledgers have no role in its validity.
const SOURCE_TABLES: &[&str] = &[
    "observation_domain",
    "observation_domain_target",
    "collection_monitor_rule",
    "collection_monitor_rule_revision",
    "collection_observation_target",
    "collection_work_order",
    "collection_work_order_domain_usage",
    "collection_work_order_lease",
    "collection_work_order_lease_task",
    "collection_acquisition_request",
    "collection_admission_decision",
    "plugin_installation",
    "linggan_ai_task_config",
    "linggan_creator_account_research_request",
    "linggan_creator_account_research_result",
    "linggan_creator_author_override",
    "linggan_creator_discovery_threshold",
    "linggan_creator_research_request",
    "linggan_creator_research_result",
    "linggan_creator_work_override",
    "linggan_model_workspace",
    "linggan_model_config",
    "linggan_historical_image_inventory",
    "linggan_material_content",
    "linggan_material_content_detail",
    "linggan_material_discovery_finding",
    "linggan_material_author_profile",
    "linggan_material_comment",
    "linggan_material_comment_engagement",
    "linggan_material_comment_platform_time",
    "linggan_material_comment_restriction",
    "linggan_material_comment_first_admission",
    "linggan_material_domain_usage",
    "linggan_material_lane_observation",
    "linggan_material_content_hashtag",
    "linggan_material_hashtag_projection",
    "linggan_material_derived_text",
    "linggan_material_media_candidate",
    "linggan_material_media_disposition_event",
    "linggan_material_media_origin",
    "linggan_material_unit",
    "linggan_material_reading",
    "linggan_material_reading_selection",
    "linggan_media_acquisition_work",
    "linggan_media_blob",
    "linggan_media_derivative",
    "linggan_media_download_attempt",
    "linggan_media_materialization",
    "linggan_media_observation",
    "linggan_media_ocr_retirement",
    "linggan_media_processing_job",
    "linggan_media_processing_job_event",
    "linggan_media_resource_relation",
    "linggan_media_slot",
    "linggan_runtime_attempt",
    "linggan_runtime_task",
    "linggan_runtime_capture_package",
    "linggan_runtime_submission_receipt",
    "linggan_runtime_record_disposition",
    "linggan_content_understanding_request",
    "linggan_content_understanding_result",
    "linggan_content_understanding_offline_recovery",
    "linggan_comment_research_request",
    "linggan_comment_research_result",
    "linggan_ops_account",
    "linggan_ops_publication",
    "linggan_ops_publication_material_projection",
    "linggan_ops_view_report",
    "linggan_topic_classification_run",
    "linggan_topic_definition",
    "linggan_topic_map_alternative",
    "linggan_topic_map_alternative_evidence",
    "linggan_topic_map_binding",
    "linggan_topic_map_breakout",
    "linggan_topic_map_concept_relation",
    "linggan_topic_map_concept_rule",
    "linggan_topic_map_discussion_unit",
    "linggan_topic_map_membership",
    "linggan_topic_map_own_creator",
    "linggan_topic_map_performance_rule",
    "linggan_topic_map_structure_source",
    "linggan_topic_map_unit_assignment",
    "linggan_topic_map_unit_resolution",
    "linggan_topic_map_viewed",
    "linggan_topic_map_work_annotation",
    "linggan_topic_material_member",
    "linggan_topic_research_dispatch",
    "linggan_topic_research_result",
    "linggan_topic_research_run",
    "linggan_topic_research_window",
    "linggan_topic_workspace",
];

/// One read-only MVCC snapshot covers the source signatures without copying text.
pub async fn source_revision(db: &Database) -> Result<SourceRevision, sqlx::Error> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let revision = source_revision_in(&mut tx).await?;
    tx.commit().await?;
    Ok(revision)
}
async fn source_revision_in(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<SourceRevision, sqlx::Error> {
    // The fingerprint has its own statement ceiling, without imposing it on the
    // map's canonical material queries in the same transaction.
    let signature_timeout: String = sqlx::query_scalar("SHOW statement_timeout")
        .fetch_one(&mut **tx)
        .await?;
    let signature_jit: String = sqlx::query_scalar("SHOW jit").fetch_one(&mut **tx).await?;
    sqlx::query("SET LOCAL jit=off").execute(&mut **tx).await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    let tables = sqlx::query("SELECT c.oid::text AS id,c.relname AS relation,format('%I.%I',n.nspname,c.relname) AS name,c.relfilenode::text AS file FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() AND c.relkind='r' AND c.relname::text=ANY($1)")
        .bind(SOURCE_TABLES)
        .fetch_all(&mut **tx).await?;
    let mut parts = Vec::new();
    let mut names = BTreeMap::new();
    for table in tables {
        let id: String = table.get("id");
        let file: String = table.get("file");
        let name: String = table.get("name");
        let relation: String = table.get("relation");
        names.insert(id.clone(), relation.clone());
        // Worker heartbeat and discovery scan position change every patrol tick,
        // but are not inputs to material or Topic Map source qualification.
        // Keep every business setting: a config, threshold, permission or identity
        // change must invalidate even when the row count stays unchanged.
        let content = match relation.as_str() {
            "collection_observation_target" => {
                Some("(to_jsonb(r)-'last_scheduler_considered_at')::text")
            }
            "plugin_installation" => Some("(to_jsonb(r)-'last_seen_at'-'selector_health')::text"),
            "collection_work_order" => Some(
                "(to_jsonb(r)-'queue_state'-'station_ref'-'installation_ref'-'account_ref'-'eligibility_ref'-'retry_not_before_at')::text",
            ),
            "collection_work_order_lease" => {
                Some("(to_jsonb(r)-'released_at'-'release_reason')::text")
            }
            "collection_work_order_lease_task" => Some(
                "(to_jsonb(r)-'execution_state'-'claimed_at'-'claimed_by_installation_ref'-'completed_at')::text",
            ),
            "linggan_media_acquisition_work" => Some(
                "jsonb_build_array(r.observation_ref,r.component_kind,(r.state='completed'))::text",
            ),
            "linggan_material_media_disposition_event" => Some(
                "jsonb_build_array(r.xmin::text,r.ctid::text,(r.recorded_at<=scope_001_now() AND r.effective_at<=scope_001_now()))::text",
            ),
            _ => None,
        };
        let automatic_reading = match relation.as_str() {
            "linggan_material_unit" => Some(NATIVE_BODY_POINTER),
            "linggan_material_reading" => Some(ADDITIVE_READING),
            "linggan_material_reading_selection" => Some(ADDITIVE_SELECTION),
            _ => None,
        };
        let completed_component = relation == "linggan_media_acquisition_work";
        let include_rows = completed_component
            || additive_facts(&relation)
            || automatic_reading.is_some()
            || matches!(
                relation.as_str(),
                "collection_work_order"
                    | "collection_work_order_lease"
                    | "collection_work_order_lease_task"
            );
        let filter = automatic_reading
            .map(|value| format!(" FILTER (WHERE {value})"))
            .unwrap_or_else(|| {
                if completed_component {
                    " FILTER (WHERE r.state='completed')".into()
                } else {
                    String::new()
                }
            });
        let authority = automatic_reading.map(|value| format!(
            "md5(COALESCE(string_agg(xmin::text||':'||ctid::text,',' ORDER BY ctid) FILTER (WHERE NOT ({value})),''))"
        )).unwrap_or_else(|| "NULL::text".into());
        let rows = if include_rows {
            let token = if relation == "linggan_material_unit" {
                "md5((to_jsonb(r)-'current_reading_ref'-'source_version'-'revision'-'updated_at')::text)".into()
            } else if let Some(content) = content {
                format!("md5({content})")
            } else {
                "xmin::text||':'||ctid::text".into()
            };
            format!("COALESCE(array_agg({token}){filter},'{{}}'::text[])")
        } else {
            "NULL::text[]".into()
        };
        parts.push(match content {
            Some(content) => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg({content},',' ORDER BY {content}),'')) AS signature, {rows} AS append_only_rows, {authority} AS reading_authority FROM {name} r"),
            None => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg(xmin::text || ':' || ctid::text,',' ORDER BY ctid),'')) AS signature, {rows} AS append_only_rows, {authority} AS reading_authority FROM {name} r"),
        });
    }
    // Definitions/columns/functions affect qualification even without a row change.
    let schema: String = sqlx::query_scalar("SELECT current_database()||':'||current_user||':'||current_schema()||':'||md5(COALESCE(string_agg(signature,',' ORDER BY signature),'')) FROM (SELECT 'c:'||c.oid::text||':'||c.relname||':'||c.relkind::text||':'||c.relfilenode::text||':'||c.relrowsecurity::text||':'||c.relforcerowsecurity::text||':'||COALESCE(c.relacl::text,'')||':'||COALESCE(c.reloptions::text,'') AS signature FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'v:'||r.oid::text||':'||r.xmin::text FROM pg_rewrite r JOIN pg_class c ON c.oid=r.ev_class JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 's:'||n.oid::text||':'||COALESCE(n.nspacl::text,'') FROM pg_namespace n WHERE n.nspname=current_schema() UNION ALL SELECT 'r:'||p.oid::text||':'||p.xmin::text FROM pg_policy p JOIN pg_class c ON c.oid=p.polrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'p:'||p.oid::text||':'||p.xmin::text FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'a:'||a.attrelid::text||':'||a.attnum::text||':'||a.xmin::text FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema()) definitions")
        .fetch_one(&mut **tx).await?;
    let mut revisions = BTreeMap::new();
    let mut append_only_rows = BTreeMap::new();
    let mut reading_authority = BTreeMap::new();
    if !parts.is_empty() {
        // All identifiers were quoted by PostgreSQL format('%I.%I'); literal
        // IDs/filenodes are PostgreSQL numeric OIDs, never caller-supplied SQL.
        let rows = sqlx::query(sqlx::AssertSqlSafe(parts.join(" UNION ALL ")))
            .fetch_all(&mut **tx)
            .await?;
        for row in rows {
            let id: String = row.get("id");
            let name = names.remove(&id).unwrap();
            if let Some(rows) = row.get::<Option<Vec<String>>, _>("append_only_rows") {
                append_only_rows.insert(name.clone(), rows.into_iter().collect());
            }
            if let Some(authority) = row.get::<Option<String>, _>("reading_authority") {
                reading_authority.insert(name.clone(), authority);
            }
            revisions.insert(name, row.get::<String, _>("signature"));
        }
    }
    // New topics can appear while a page is built. Existing topic identity,
    // current definition, binding and definition source must still agree.
    let topics = sqlx::query_as::<_, (String, String)>(r#"
        SELECT t.topic_ref::text, concat_ws('|',t.xmin::text,t.ctid::text,
            d.definition_ref::text,d.row_version,b.row_version,c.xmin::text,c.ctid::text)
        FROM linggan_topic_workspace t
        LEFT JOIN LATERAL (SELECT definition_ref,xmin::text||':'||ctid::text AS row_version
            FROM linggan_topic_definition WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1) d ON true
        LEFT JOIN LATERAL (SELECT xmin::text||':'||ctid::text AS row_version
            FROM linggan_topic_map_binding WHERE topic_ref=t.topic_ref ORDER BY version DESC LIMIT 1) b ON true
        LEFT JOIN linggan_topic_map_concept_rule c ON c.definition_ref=d.definition_ref
    "#).fetch_all(&mut **tx).await?.into_iter().collect();
    sqlx::query("SELECT set_config('statement_timeout',$1,true),set_config('jit',$2,true)")
        .bind(signature_timeout)
        .bind(signature_jit)
        .execute(&mut **tx)
        .await?;
    Ok(SourceRevision {
        schema,
        tables: revisions,
        topics,
        append_only_rows,
        reading_authority,
    })
}

/// Overview keeps full scope, search snippets and discussion coverage; unused
/// audit/media details are supplied by the qualified evidence endpoint instead.
pub fn overview(snapshot: &TopicMapSnapshot) -> Result<Value, serde_json::Error> {
    let mut value = serde_json::to_value(snapshot)?;
    for work in value["works"].as_array_mut().into_iter().flatten() {
        if let Some(object) = work.as_object_mut() {
            object.remove("sourceManifest");
            let cover = object
                .get("media")
                .and_then(|v| v.get("cover"))
                .cloned()
                .unwrap_or(Value::Null);
            object.insert("media".into(), serde_json::json!({"cover":cover}));
        }
    }
    Ok(value)
}

/// The first page is bounded independently of the complete statistical scope.
pub fn first_page(snapshot: &TopicMapSnapshot) -> Result<Value, serde_json::Error> {
    let mut scope = snapshot.scope.clone();
    scope["loadedWorkCount"] = serde_json::json!(snapshot.works.len().min(12));
    let works = snapshot
        .works
        .iter()
        .take(12)
        .map(serde_json::to_value)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        serde_json::json!({"domains":snapshot.domains,"scope":scope,"topics":snapshot.topics,"works":works,"statistics":snapshot.statistics,"journey":snapshot.journey,"sources":snapshot.sources,"candidates":snapshot.candidates,"changes":snapshot.changes,"alternatives":snapshot.alternatives,"ownCreators":snapshot.own_creators,"methodVersion":snapshot.method_version,"sourceBoundary":snapshot.source_boundary,"materialPage":{"limit":12,"returned":snapshot.works.len().min(12),"complete":snapshot.works.len()<=12}}),
    )
}
