//! Disposable, bounded page projections. Authority remains in canonical readers.
//! Version fingerprints include deletes and every source table in the connected schema.
use super::{TopicMapError, TopicMapQuery, TopicMapSnapshot, read::read_topic_map_in};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct TopicMapReadModel {
    entries: Mutex<BTreeMap<String, Entry>>,
    builds: Mutex<()>,
}
struct Entry {
    revision: SourceRevision,
    created: Instant,
    snapshot: Arc<TopicMapSnapshot>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SourceRevision {
    schema: String,
    tables: BTreeMap<String, String>,
    topics: BTreeMap<String, String>,
}

impl TopicMapReadModel {
    fn key(q: &TopicMapQuery) -> Result<String, TopicMapError> {
        let mut normalized = q.clone();
        normalized.reference_window_days = Some(q.reference_window_days.unwrap_or(30));
        serde_json::to_string(&normalized).map_err(|e| TopicMapError::Source(e.to_string()))
    }
    async fn current(&self, key: &str, revision: &SourceRevision) -> Option<Arc<TopicMapSnapshot>> {
        let mut entries = self.entries.lock().await;
        entries.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(60));
        entries
            .get(key)
            .filter(|entry| {
                entry.created.elapsed() < Duration::from_secs(60) && entry.revision == *revision
            })
            .map(|entry| Arc::clone(&entry.snapshot))
    }
    /// Current authority is validated before every reuse, including evidence opens.
    pub async fn read(
        &self,
        db: &Database,
        q: &TopicMapQuery,
    ) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        let key = Self::key(q)?;
        let revision = source_revision(db).await?;
        if let Some(value) = self.current(&key, &revision).await {
            return Ok(value);
        }
        let _build = self.builds.lock().await;
        self.build(db, q, key, true).await
    }
    /// Refresh atomically; readers may use the previous *still-qualified* version
    /// during a clock refresh. A changed source revision can never use that version.
    pub async fn refresh(&self, db: &Database, q: &TopicMapQuery) -> Result<(), TopicMapError> {
        let _build = self.builds.lock().await;
        self.build(db, q, Self::key(q)?, false).await?;
        Ok(())
    }
    async fn build(
        &self,
        db: &Database,
        q: &TopicMapQuery,
        key: String,
        reuse_current: bool,
    ) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        for _ in 0..2 {
            let mut tx = db.pool().begin().await?;
            sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
                .execute(&mut *tx)
                .await?;
            let revision = source_revision_in(&mut tx).await?;
            if reuse_current && let Some(value) = self.current(&key, &revision).await {
                tx.commit().await?;
                return Ok(value);
            }
            let snapshot = Arc::new(read_topic_map_in(&mut tx, q, Some(db)).await?);
            tx.commit().await?;
            let after = source_revision(db).await?;
            if revision == after {
                let mut entries = self.entries.lock().await;
                entries.retain(|_, entry| entry.created.elapsed() < Duration::from_secs(60));
                if entries.len() >= 4 && !entries.contains_key(&key) {
                    entries.clear();
                }
                entries.insert(
                    key,
                    Entry {
                        revision,
                        created: Instant::now(),
                        snapshot: Arc::clone(&snapshot),
                    },
                );
                return Ok(snapshot);
            }
            if revision.same_source_authority(&after) {
                // A concurrent research result cannot tear an already consistent read.
                // Leave it out of the cache so the next request can see the new result.
                return Ok(snapshot);
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
            && self.topics.iter().all(|(topic, version)| newer.topics.get(topic) == Some(version))
            && self
                .tables
                .iter()
                .filter(|(table, _)| carries_authority(table))
                .eq(newer
                    .tables
                    .iter()
                    .filter(|(table, _)| carries_authority(table)))
    }
}

/// MVCC versions plus semantic settings signatures, never asynchronous pg_stat counters.
/// One READ ONLY snapshot covers all table signatures. No source text is copied here.
/// Scheduler heartbeat/run/fairness ledgers cannot affect material, definition or
/// research qualification. The invocation ledger feeds independent live progress,
/// never this snapshot. The dedicated AI executor's instance, leases, batches,
/// rounds and command/control ledgers likewise cannot change Topic Map source facts.
/// Comment intake schedules and selection receipts are also independent of saved research.
/// Semantic row signatures omit only audited worker fields;
/// all their business fields (including future columns) remain in the signature.
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
    let tables = sqlx::query("SELECT c.oid::text AS id,c.relname AS relation,format('%I.%I',n.nspname,c.relname) AS name,c.relfilenode::text AS file FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() AND c.relkind='r' AND c.relname NOT IN ('collection_scheduler_run','collection_scheduler_run_step','collection_scheduler_heartbeat','collection_dispatch_lane_fairness','linggan_model_invocation','linggan_ai_failure_code','linggan_ai_executor_instance','linggan_ai_executor_current','linggan_ai_execution_attempt','linggan_ai_late_usage_receipt','linggan_ai_batch','linggan_ai_batch_control','linggan_ai_batch_item','linggan_ai_batch_item_control','linggan_ai_trial_comparison','linggan_ai_round','linggan_ai_task_round_state','linggan_ai_command','linggan_ai_control','linggan_ai_control_event','linggan_comment_research_selection','linggan_comment_research_selection_target') ORDER BY c.oid")
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
            "execution_station" => Some(
                "(to_jsonb(r)-'last_dispatch_answer_at'-'last_dispatch_answer_code'-'last_dispatch_answer_reason')::text",
            ),
            "plugin_installation" => Some("(to_jsonb(r)-'last_seen_at'-'selector_health')::text"),
            _ => None,
        };
        parts.push(match content {
            Some(content) => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg({content},',' ORDER BY {content}),'')) AS signature FROM {name} r"),
            None => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg(xmin::text || ':' || ctid::text,',' ORDER BY ctid),'')) AS signature FROM {name}"),
        });
    }
    // Definitions/columns/functions affect qualification even without a row change.
    let schema: String = sqlx::query_scalar("SELECT current_database()||':'||current_user||':'||current_schema()||':'||md5(COALESCE(string_agg(signature,',' ORDER BY signature),'')) FROM (SELECT 'c:'||c.oid::text||':'||c.relname||':'||c.relkind::text||':'||c.relfilenode::text||':'||c.relrowsecurity::text||':'||c.relforcerowsecurity::text||':'||COALESCE(c.relacl::text,'')||':'||COALESCE(c.reloptions::text,'') AS signature FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'v:'||r.oid::text||':'||r.xmin::text FROM pg_rewrite r JOIN pg_class c ON c.oid=r.ev_class JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 's:'||n.oid::text||':'||COALESCE(n.nspacl::text,'') FROM pg_namespace n WHERE n.nspname=current_schema() UNION ALL SELECT 'r:'||p.oid::text||':'||p.xmin::text FROM pg_policy p JOIN pg_class c ON c.oid=p.polrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'p:'||p.oid::text||':'||p.xmin::text FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'a:'||a.attrelid::text||':'||a.attnum::text||':'||a.xmin::text FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema()) definitions")
        .fetch_one(&mut **tx).await?;
    let mut revisions = BTreeMap::new();
    if !parts.is_empty() {
        // All identifiers were quoted by PostgreSQL format('%I.%I'); literal
        // IDs/filenodes are PostgreSQL numeric OIDs, never caller-supplied SQL.
        let rows = sqlx::query(sqlx::AssertSqlSafe(parts.join(" UNION ALL ")))
            .fetch_all(&mut **tx)
            .await?;
        for row in rows {
            let id: String = row.get("id");
            revisions.insert(
                names.remove(&id).unwrap(),
                row.get::<String, _>("signature"),
            );
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
