//! Disposable, bounded page projections. Authority remains in canonical readers.
//! Version fingerprints include deletes and every source table in the connected schema.
use super::{TopicMapError, TopicMapQuery, TopicMapSnapshot, read_topic_map};
use linggan_storage_postgres::Database;
use serde_json::Value;
use sqlx::Row;
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
        let revision = source_revision(db).await?;
        if let Some(value) = self.current(&key, &revision).await {
            return Ok(value);
        }
        self.entries.lock().await.remove(&key);
        self.build(db, q, key, revision).await
    }
    /// Refresh atomically; readers may use the previous *still-qualified* version
    /// during a clock refresh. A changed source revision can never use that version.
    pub async fn refresh(&self, db: &Database, q: &TopicMapQuery) -> Result<(), TopicMapError> {
        let _build = self.builds.lock().await;
        let revision = source_revision(db).await?;
        self.build(db, q, Self::key(q)?, revision).await?;
        Ok(())
    }
    async fn build(
        &self,
        db: &Database,
        q: &TopicMapQuery,
        key: String,
        revision: SourceRevision,
    ) -> Result<Arc<TopicMapSnapshot>, TopicMapError> {
        let mut revision = revision;
        for _ in 0..2 {
            let snapshot = Arc::new(read_topic_map(db, q).await?);
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
            revision = after;
        }
        // A continuously changing source cannot be labelled a stable complete page.
        Err(TopicMapError::Conflict)
    }
}

/// MVCC versions plus semantic settings signatures, never asynchronous pg_stat counters.
/// One READ ONLY snapshot covers all table signatures. No source text is copied here.
/// Scheduler heartbeat/run/fairness ledgers cannot affect material, definition or
/// research qualification. The invocation ledger feeds independent live progress,
/// never this snapshot. The dedicated AI executor's instance, leases, batches,
/// rounds and command/control ledgers likewise cannot change Topic Map source facts.
/// Semantic row signatures omit only audited worker fields;
/// all their business fields (including future columns) remain in the signature.
pub async fn source_revision(db: &Database) -> Result<SourceRevision, sqlx::Error> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL jit=off").execute(&mut *tx).await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let tables = sqlx::query("SELECT c.oid::text AS id,c.relname AS relation,format('%I.%I',n.nspname,c.relname) AS name,c.relfilenode::text AS file FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() AND c.relkind='r' AND c.relname NOT IN ('collection_scheduler_run','collection_scheduler_run_step','collection_scheduler_heartbeat','collection_dispatch_lane_fairness','linggan_model_invocation','linggan_ai_failure_code','linggan_ai_executor_instance','linggan_ai_executor_current','linggan_ai_execution_attempt','linggan_ai_late_usage_receipt','linggan_ai_batch','linggan_ai_batch_control','linggan_ai_batch_item','linggan_ai_batch_item_control','linggan_ai_trial_comparison','linggan_ai_round','linggan_ai_task_round_state','linggan_ai_command','linggan_ai_control','linggan_ai_control_event') ORDER BY c.oid")
        .fetch_all(&mut *tx).await?;
    let mut parts = Vec::new();
    let mut names = BTreeMap::new();
    for table in tables {
        let id: String = table.get("id");
        let file: String = table.get("file");
        let name: String = table.get("name");
        names.insert(id.clone(), name.clone());
        let relation: String = table.get("relation");
        // Worker heartbeat and discovery scan position change every patrol tick,
        // but are not inputs to material or Topic Map source qualification.
        // Keep every business setting: a config, threshold, permission or identity
        // change must invalidate even when the row count stays unchanged.
        let content = match relation.as_str() {
            "linggan_creator_discovery_policy" => {
                Some("(to_jsonb(r)-'scan_after'-'updated_at')::text")
            }
            "linggan_creator_discovery_author_analysis" => Some(
                "(to_jsonb(r)-'job_state'-'attempt_count'-'next_attempt_at'-'lease_token'-'lease_expires_at'-'last_error_code'-'updated_at')::text",
            ),
            "collection_observation_target" => {
                Some("(to_jsonb(r)-'last_scheduler_considered_at')::text")
            }
            _ => None,
        };
        parts.push(match content {
            Some(content) => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg({content},',' ORDER BY {content}),'')) AS signature FROM {name} r"),
            None => format!("SELECT '{id}' AS id, '{id}:{file}:' || count(*)::text || ':' || md5(COALESCE(string_agg(xmin::text || ':' || ctid::text,',' ORDER BY ctid),'')) AS signature FROM {name}"),
        });
    }
    // Definitions/columns/functions affect qualification even without a row change.
    let schema: String = sqlx::query_scalar("SELECT current_database()||':'||current_user||':'||current_schema()||':'||md5(COALESCE(string_agg(signature,',' ORDER BY signature),'')) FROM (SELECT 'c:'||c.oid::text||':'||c.relname||':'||c.relkind::text||':'||c.relfilenode::text||':'||c.relrowsecurity::text||':'||c.relforcerowsecurity::text||':'||COALESCE(c.relacl::text,'')||':'||COALESCE(c.reloptions::text,'') AS signature FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'v:'||r.oid::text||':'||r.xmin::text FROM pg_rewrite r JOIN pg_class c ON c.oid=r.ev_class JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 's:'||n.oid::text||':'||COALESCE(n.nspacl::text,'') FROM pg_namespace n WHERE n.nspname=current_schema() UNION ALL SELECT 'r:'||p.oid::text||':'||p.xmin::text FROM pg_policy p JOIN pg_class c ON c.oid=p.polrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'p:'||p.oid::text||':'||p.xmin::text FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname=current_schema() UNION ALL SELECT 'a:'||a.attrelid::text||':'||a.attnum::text||':'||a.xmin::text FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname=current_schema()) definitions")
        .fetch_one(&mut *tx).await?;
    let mut revisions = BTreeMap::new();
    if !parts.is_empty() {
        // All identifiers were quoted by PostgreSQL format('%I.%I'); literal
        // IDs/filenodes are PostgreSQL numeric OIDs, never caller-supplied SQL.
        let rows = sqlx::query(sqlx::AssertSqlSafe(parts.join(" UNION ALL ")))
            .fetch_all(&mut *tx)
            .await?;
        for row in rows {
            let id: String = row.get("id");
            revisions.insert(
                names.remove(&id).unwrap(),
                row.get::<String, _>("signature"),
            );
        }
    }
    tx.commit().await?;
    Ok(SourceRevision {
        schema,
        tables: revisions,
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
