//! Domain opt-in and bounded queue production for L1 into the existing content-understanding ledger.
use super::{FoundationError, METHOD, Start, preview_automatic, source, start_with_kind};
use crate::ai_foundation::{batch_store::BatchKind, store::Command};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::Row;
use std::{collections::HashSet, time::{Duration, Instant}};
use uuid::Uuid;

/// Automatic work waiting or running; the scan pauses at this size.
pub(super) const QUEUE_LIMIT: i64 = 20;
pub(super) const AUTOMATIC_WAITING: &str = "SELECT count(*) FROM linggan_content_understanding_request r JOIN linggan_ai_batch b USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE b.kind='automatic' AND c.state IN('pending','running')";
const SCAN_PAGE: i64 = 40;
/// A finished pass over every work waits this long before it looks again.
const IDLE_AFTER_PASS: Duration = Duration::from_secs(60);

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub domain_ref: Uuid,
    pub enabled: bool,
    pub revision: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub configuration_ready: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<Value>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Change {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub expected_revision: i64,
    pub enabled: bool,
}

/// Works are counted once by their best request: a later success hides an earlier failure.
async fn progress(db: &Database, domain: Uuid) -> Result<Value, FoundationError> {
    let rows = sqlx::query(
        "WITH w AS (
           SELECT r.work_ref,
                  bool_or(s.result IS NOT NULL) AS done,
                  bool_or(s.result IS NULL AND c.state IN ('pending','running')) AS waiting,
                  max(c.failure_code) FILTER (WHERE s.result IS NULL AND c.state='failed') AS failure
           FROM linggan_content_understanding_request r
           JOIN linggan_ai_batch_item_control c USING(item_ref)
           LEFT JOIN linggan_content_understanding_saved_core s USING(request_ref)
           WHERE r.domain_ref=$1 AND r.method_version=$2
           GROUP BY r.work_ref)
         SELECT CASE WHEN done THEN 'done' WHEN waiting THEN 'waiting' WHEN failure IS NOT NULL THEN failure ELSE 'other' END AS bucket,
                count(*) AS works
         FROM w GROUP BY 1",
    )
    .bind(domain)
    .bind(METHOD)
    .fetch_all(db.pool())
    .await?;
    let (mut done, mut waiting, mut failures) = (0_i64, 0_i64, serde_json::Map::new());
    for row in rows {
        let works: i64 = row.get("works");
        match row.get::<String, _>("bucket").as_str() {
            "done" => done = works,
            "waiting" => waiting = works,
            "other" => {}
            code => {
                failures.insert(code.to_owned(), json!(works));
            }
        }
    }
    Ok(json!({"done": done, "waiting": waiting, "failedByCode": failures}))
}

pub async fn read(db: &Database, domain: Uuid) -> Result<Setting, FoundationError> {
    let row = sqlx::query("SELECT d.domain_ref,COALESCE(a.enabled,false) AS enabled,COALESCE(a.revision,0) AS revision FROM observation_domain d LEFT JOIN linggan_content_understanding_automation a USING(domain_ref) WHERE d.domain_ref=$1")
        .bind(domain)
        .fetch_optional(db.pool())
        .await?
        .ok_or(FoundationError::NotFound)?;
    let mut tx = db.pool().begin().await?;
    let config: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='content_understanding'",
    )
    .fetch_one(&mut *tx)
    .await?;
    let ready = match crate::ai_foundation::admission::check_queue_configuration(
        &mut tx,
        super::TASK,
        config,
        false,
    )
    .await
    {
        Ok(()) => true,
        Err(FoundationError::AdmissionDenied(_)) => false,
        Err(error) => return Err(error),
    };
    tx.commit().await?;
    Ok(Setting {
        domain_ref: domain,
        enabled: row.get("enabled"),
        revision: row.get("revision"),
        configuration_ready: Some(ready),
        progress: Some(progress(db, domain).await?),
    })
}

pub async fn change(db: &Database, request: &Change) -> Result<Setting, FoundationError> {
    if request.command_ref.is_nil() || request.domain_ref.is_nil() || request.expected_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut command = Command::begin(
        db,
        request.command_ref,
        "content_understanding_automation",
        request,
    )
    .await?;
    if let Some(receipt) = command.replay()? {
        return Ok(receipt);
    }
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)")
            .bind(request.domain_ref)
            .fetch_one(&mut *command.tx)
            .await?;
    if !exists {
        return Err(FoundationError::NotFound);
    }
    sqlx::query("INSERT INTO linggan_content_understanding_automation(domain_ref) VALUES($1) ON CONFLICT DO NOTHING")
        .bind(request.domain_ref)
        .execute(&mut *command.tx)
        .await?;
    let current: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_content_understanding_automation WHERE domain_ref=$1 FOR UPDATE",
    )
    .bind(request.domain_ref)
    .fetch_one(&mut *command.tx)
    .await?;
    if current != request.expected_revision {
        return Err(FoundationError::Conflict);
    }
    let revision: i64 = sqlx::query_scalar("UPDATE linggan_content_understanding_automation SET enabled=$2,revision=revision+1,updated_at=scope_001_now() WHERE domain_ref=$1 RETURNING revision")
        .bind(request.domain_ref)
        .bind(request.enabled)
        .fetch_one(&mut *command.tx)
        .await?;
    command
        .finish(Setting {
            domain_ref: request.domain_ref,
            enabled: request.enabled,
            revision,
            configuration_ready: None,
            progress: None,
        })
        .await
}

/// A scan position only, not a second task ledger. Restarting safely rescans existing command IDs.
#[derive(Default)]
pub struct CursorState {
    after: Option<(Uuid, Uuid)>,
    idle_until: Option<Instant>,
    /// Whether this pass looked at any work; an empty pass (nothing enabled) is cheap and need not idle.
    scanned: bool,
}
pub type Cursor = Option<CursorState>;

/// Queue only new, qualified material versions. An attempt on the same material version
/// remains authoritative, so a persistent output failure cannot turn into a paid retry loop.
pub async fn tick(db: &Database, cursor: &mut Cursor) -> Result<usize, FoundationError> {
    let state = cursor.get_or_insert_with(CursorState::default);
    if state.idle_until.is_some_and(|until| Instant::now() < until) {
        return Ok(0);
    }
    state.idle_until = None;
    // A full queue pauses the scan in place. It must not look like the end of a pass, or the
    // cursor would reset and the scan would idle while the queue drains.
    let waiting: i64 = sqlx::query_scalar(AUTOMATIC_WAITING)
        .fetch_one(db.pool())
        .await?;
    if waiting >= QUEUE_LIMIT {
        return Ok(0);
    }
    let rows = sqlx::query(include_str!("automation_candidates.sql"))
        .bind(state.after.map(|v| v.0))
        .bind(state.after.map(|v| v.1))
        .bind(SCAN_PAGE)
        .fetch_all(db.pool())
        .await?;
    let refs: Vec<Uuid> = rows.iter().map(|row| row.get("work_ref")).collect();
    let check = new_material_versions(db, &refs).await?;
    let mut queued = 0;
    state.scanned |= !rows.is_empty();
    for row in &rows {
        let domain: Uuid = row.get("domain_ref");
        let work: Uuid = row.get("work_ref");
        let previous = state.after;
        state.after = Some((domain, work));
        if !check.contains(&work) {
            continue;
        }
        // One unreadable work must not stop the others.
        let (plan, material_identity) = match preview_automatic(db, domain, work).await {
            Ok(Some(plan)) => plan,
            Err(FoundationError::Database(error)) => {
                state.after = previous;
                return Err(error.into());
            }
            _ => continue,
        };
        if plan["canStart"] != true {
            continue;
        }
        let (Some(cutoff), Some(hash), Some(config), Some(revision)) = (
            plan["knowledgeCutoff"].as_str(),
            plan["inputHash"].as_str(),
            plan["taskConfigRef"].as_str().and_then(|v| v.parse().ok()),
            plan["taskRevision"].as_i64(),
        ) else {
            continue;
        };
        let command_ref: Uuid = sqlx::query_scalar("SELECT md5($1)::uuid")
            .bind(format!(
                "l1-auto:{domain}:{work}:{material_identity}:{METHOD}"
            ))
            .fetch_one(db.pool())
            .await?;
        let request = Start {
            command_ref,
            domain_ref: domain,
            work_ref: work,
            knowledge_cutoff: cutoff.to_owned(),
            input_hash: hash.to_owned(),
            task_config_ref: config,
            task_revision: revision,
        };
        match start_with_kind(db, &request, BatchKind::Automatic).await {
            Ok(receipt) if !receipt.reused => queued += 1,
            Ok(_) => {}
            Err(FoundationError::Database(error)) => return Err(error.into()),
            Err(FoundationError::AdmissionDenied(reasons))
                if reasons.iter().any(|reason| reason == "automatic_l1_queue_full") =>
            {
                // This work is first to be reconsidered after the queue drains.
                state.after = previous;
                return Ok(queued);
            }
            Err(_) => {}
        }
    }
    if (rows.len() as i64) < SCAN_PAGE {
        state.after = None;
        if std::mem::take(&mut state.scanned) {
            state.idle_until = Some(Instant::now() + IDLE_AFTER_PASS);
        }
    }
    Ok(queued)
}

/// Batch the canonical dossiers as a cheap candidate check. Start still
/// reconstructs the exact packet, including strict source-kind and current permissions.
async fn new_material_versions(
    db: &Database,
    refs: &[Uuid],
) -> Result<HashSet<Uuid>, FoundationError> {
    if refs.is_empty() {
        return Ok(HashSet::new());
    }
    let mut tx = super::begin_preview(db).await?;
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let dossiers = linggan_evidence::research_dossier::read_in(&mut tx, refs, &cutoff).await?;
    let attempted = sqlx::query_as::<_, (Uuid, String)>("SELECT work_ref,material_snapshot_identity FROM linggan_content_understanding_request WHERE work_ref=ANY($1) AND method_version=$2")
        .bind(refs).bind(METHOD).fetch_all(&mut *tx).await?
        .into_iter().collect::<HashSet<_>>();
    tx.commit().await?;
    Ok(dossiers.into_iter().filter_map(|(work,dossier)| {
        let packet = source::from_dossier(dossier, &cutoff).ok()?;
        (packet.coverage["materialReadiness"]["state"] == "ready"
            && !attempted.contains(&(work, packet.snapshot.material_snapshot_identity)))
            .then_some(work)
    }).collect())
}
