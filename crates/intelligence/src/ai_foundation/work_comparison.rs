//! Explicit, object-scoped content comparison on the shared AI queue.
//! This owner stores source identities and derived claims, never a second source-text copy.
mod contract;
use super::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::research_text::content_hash;
pub use contract::{Analysis, Citation, Claim, prompt, response_schema, validate_analysis};
use linggan_evidence::target_inspection::{self as inspection, DossierView, Scope};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const CONTRACT: &str = "work-content-comparison.v1";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartComparison {
    pub command_ref: Uuid,
    pub target_ref: Uuid,
    pub domain_ref: Option<Uuid>,
    pub work_refs: Vec<Uuid>,
    pub knowledge_cutoff: String,
    pub expected_material_revisions: Vec<String>,
    pub task_config_ref: Uuid,
    pub expected_task_revision: i64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ComparisonReceipt {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
    pub reused: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub target_ref: Uuid,
    pub work_refs: Vec<Uuid>,
    pub knowledge_cutoff: String,
    pub material_revisions: Vec<String>,
    pub partial: bool,
    pub task_config_ref: Option<Uuid>,
    pub task_revision: Option<i64>,
    pub can_start: bool,
    pub reasons: Vec<String>,
    pub method_version: &'static str,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonRead {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
    pub knowledge_cutoff: String,
    pub state: String,
    pub wait_reason: Option<String>,
    pub failure_code: Option<String>,
    pub result: Option<Analysis>,
    pub materials: Vec<DossierView>,
    pub outdated: bool,
    pub method_version: &'static str,
}

pub fn map_source_error(error: inspection::Error) -> FoundationError {
    match error {
        inspection::Error::Database(e) => FoundationError::Database(e),
        inspection::Error::Invalid(_) => FoundationError::Invalid,
        _ => FoundationError::AdmissionDenied(vec!["comparison_source_unavailable".into()]),
    }
}
pub fn material_revisions(materials: &[DossierView]) -> Vec<String> {
    materials
        .iter()
        .map(|d| d.material_revision.clone())
        .collect()
}
// The immutable request binds the knowledge cutoff. Time alone is not a new material version.
pub fn fingerprint(materials: &[DossierView]) -> String {
    content_hash(
        &json!(
            materials
                .iter()
                .map(|d| json!([
                    d.work_ref,
                    d.owner_contract,
                    d.material_revision,
                    d.partial,
                    d.limitations,
                    d.fragments
                        .iter()
                        .map(|f| json!([
                            f.citation_id,
                            f.source_ref,
                            f.source_version,
                            f.start,
                            f.end,
                            f.speaker,
                            f.structure,
                            f.issues,
                            f.reading_ref,
                            f.unit_ref,
                            f.source_location
                        ]))
                        .collect::<Vec<_>>()
                ]))
                .collect::<Vec<_>>()
        )
        .to_string(),
    )
}

/// Content-level comparisons belong to one stable creator identity. A keyword result
/// may contain many authors; the caller cannot elevate it to a same-author judgment.
async fn require_creator(db: &Database, target: Uuid) -> Result<(), FoundationError> {
    let ok: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collection_observation_target \
         WHERE target_ref=$1 AND target_kind='creator')",
    )
    .bind(target)
    .fetch_one(db.pool())
    .await?;
    if ok {
        Ok(())
    } else {
        Err(FoundationError::Invalid)
    }
}

pub async fn preview(
    db: &Database,
    scope: &Scope,
    works: &[Uuid],
) -> Result<Preview, FoundationError> {
    require_creator(db, scope.target_ref).await?;
    let materials = inspection::comparison_materials(db, scope, works)
        .await
        .map_err(map_source_error)?;
    let cutoff = materials
        .first()
        .ok_or(FoundationError::Invalid)?
        .knowledge_cutoff
        .clone();
    let mut tx = db.pool().begin().await?;
    let setting:Option<(Uuid,i64)>=sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='work_comparison'").fetch_optional(&mut *tx).await?;
    let mut reasons = Vec::new();
    if materials.iter().any(|d| d.fragments.is_empty()) {
        reasons.push("comparison_source_unavailable".into());
    }
    if let Some((config, _)) = setting {
        match admission::check_queue_configuration(&mut tx, TaskKey::WorkComparison, config, false)
            .await
        {
            Ok(()) => match admission::check_queue_input_scope(
                &mut tx,
                TaskKey::WorkComparison,
                config,
                &InputRef::WorkComparison {
                    request_ref: Uuid::nil(),
                },
            )
            .await
            {
                Ok(()) => (),
                Err(FoundationError::AdmissionDenied(codes)) => reasons.extend(codes),
                Err(e) => return Err(e),
            },
            Err(FoundationError::AdmissionDenied(codes)) => reasons.extend(codes),
            Err(e) => return Err(e),
        }
    } else {
        reasons.push("task_not_connected".into());
    }
    tx.commit().await?;
    Ok(Preview {
        target_ref: scope.target_ref,
        work_refs: works.to_vec(),
        knowledge_cutoff: cutoff,
        material_revisions: material_revisions(&materials),
        partial: materials.iter().any(|d| d.partial),
        task_config_ref: setting.map(|s| s.0),
        task_revision: setting.map(|s| s.1),
        can_start: reasons.is_empty(),
        reasons,
        method_version: CONTRACT,
    })
}

pub async fn start(
    db: &Database,
    request: &StartComparison,
) -> Result<ComparisonReceipt, FoundationError> {
    require_creator(db, request.target_ref).await?;
    let scope = Scope {
        target_ref: request.target_ref,
        domain_ref: request.domain_ref,
        knowledge_cutoff: Some(request.knowledge_cutoff.clone()),
    };
    // The read owner validates the timestamp and object scope before any command write.
    inspection::comparison_materials(db, &scope, &request.work_refs)
        .await
        .map_err(map_source_error)?;
    let mut command = Command::begin(db, request.command_ref, "work_comparison", request).await?;
    if let Some(receipt) = command.replay::<ComparisonReceipt>()? {
        return Ok(receipt);
    }
    let config = lock_setting(
        &mut command.tx,
        TaskKey::WorkComparison,
        request.expected_task_revision,
    )
    .await?;
    if config != request.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let materials = inspection::comparison_materials_in(
        &mut command.tx,
        &scope,
        &request.work_refs,
        &request.knowledge_cutoff,
    )
    .await
    .map_err(map_source_error)?;
    if material_revisions(&materials) != request.expected_material_revisions {
        return Err(FoundationError::Conflict);
    }
    if materials.iter().any(|d| d.fragments.is_empty()) {
        return Err(FoundationError::AdmissionDenied(vec![
            "comparison_source_unavailable".into(),
        ]));
    }
    let digest = fingerprint(&materials);
    let scope_key = content_hash(
        &json!([
            request.target_ref,
            request.domain_ref,
            request.work_refs,
            digest,
            config,
            CONTRACT
        ])
        .to_string(),
    );
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,410149))")
        .bind(scope_key)
        .execute(&mut *command.tx)
        .await?;
    let existing:Option<(Uuid,Uuid)>=sqlx::query_as(
        "SELECT r.request_ref,r.batch_ref FROM linggan_work_comparison_request r \
         JOIN linggan_ai_batch_item item USING(batch_ref) JOIN linggan_ai_batch_item_control control USING(item_ref) \
         WHERE r.target_ref=$1 AND r.domain_ref IS NOT DISTINCT FROM $2 AND r.work_refs=$3 \
           AND r.input_fingerprint=$4 AND r.task_config_ref=$5 AND r.method_version=$6 \
           AND control.state IN('pending','running','succeeded') ORDER BY r.created_at DESC LIMIT 1",
    ).bind(request.target_ref).bind(request.domain_ref).bind(&request.work_refs).bind(&digest).bind(config).bind(CONTRACT).fetch_optional(&mut *command.tx).await?;
    if let Some((request_ref, batch_ref)) = existing {
        return command
            .finish(ComparisonReceipt {
                request_ref,
                batch_ref,
                reused: true,
            })
            .await;
    }
    let request_ref = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request_ref.to_string(),
        input: InputRef::WorkComparison { request_ref },
    }];
    let hash = batch_store::input_hash(TaskKey::WorkComparison, &inputs, false)?;
    let batch = batch_store::insert_batch(
        &mut command.tx,
        request.command_ref,
        TaskKey::WorkComparison,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    sqlx::query("INSERT INTO linggan_work_comparison_request(request_ref,command_ref,target_ref,domain_ref,work_refs,knowledge_cutoff,material_revisions,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5,$6::timestamptz,$7,$8,$9,$10,$11)")
        .bind(request_ref).bind(request.command_ref).bind(request.target_ref).bind(request.domain_ref).bind(&request.work_refs).bind(&request.knowledge_cutoff)
        .bind(json!(request.expected_material_revisions)).bind(digest).bind(config).bind(batch).bind(CONTRACT).execute(&mut *command.tx).await?;
    command
        .finish(ComparisonReceipt {
            request_ref,
            batch_ref: batch,
            reused: false,
        })
        .await
}

pub(crate) async fn frozen_in(
    tx: &mut Transaction<'_, Postgres>,
    request_ref: Uuid,
) -> Result<Vec<DossierView>, FoundationError> {
    let row=sqlx::query("SELECT target_ref,domain_ref,work_refs,to_char(knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,input_fingerprint FROM linggan_work_comparison_request WHERE request_ref=$1")
        .bind(request_ref).fetch_optional(&mut **tx).await?.ok_or(FoundationError::NotFound)?;
    let cutoff: String = row.get("cutoff");
    let scope = Scope {
        target_ref: row.get("target_ref"),
        domain_ref: row.get("domain_ref"),
        knowledge_cutoff: Some(cutoff.clone()),
    };
    let works: Vec<Uuid> = row.get("work_refs");
    let materials = inspection::comparison_materials_in(tx, &scope, &works, &cutoff)
        .await
        .map_err(map_source_error)?;
    if fingerprint(&materials) != row.get::<String, _>("input_fingerprint") {
        return Err(FoundationError::AdmissionDenied(vec![
            "comparison_source_changed".into(),
        ]));
    }
    Ok(materials)
}

pub async fn read(
    db: &Database,
    scope: &Scope,
    request_ref: Uuid,
) -> Result<ComparisonRead, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    let row=sqlx::query("SELECT r.*,to_char(r.knowledge_cutoff AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS cutoff,c.state,c.wait_reason,c.failure_code,result.result FROM linggan_work_comparison_request r JOIN linggan_ai_batch_item item USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_work_comparison_result result USING(request_ref) WHERE r.request_ref=$1 AND r.target_ref=$2 AND r.domain_ref IS NOT DISTINCT FROM $3")
        .bind(request_ref).bind(scope.target_ref).bind(scope.domain_ref).fetch_optional(&mut *tx).await?.ok_or(FoundationError::NotFound)?;
    let mut materials = frozen_in(&mut tx, request_ref).await?;
    let result: Option<Value> = row.get("result");
    let result = result
        .map(serde_json::from_value::<Analysis>)
        .transpose()
        .map_err(|_| FoundationError::Invalid)?;
    let works: Vec<Uuid> = row.get("work_refs");
    let now: String = sqlx::query_scalar(
        "SELECT to_char(scope_001_now() AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .fetch_one(&mut *tx)
    .await?;
    let current = inspection::comparison_materials_in(&mut tx, scope, &works, &now)
        .await
        .map_err(map_source_error)?;
    let outdated = fingerprint(&current) != row.get::<String, _>("input_fingerprint");
    // Full frozen/current inputs have been validated above. Public evidence is an explicit
    // selection of published claim blocks; full-note reading stays with canonical V2.
    let selected: std::collections::BTreeSet<_> = result
        .iter()
        .flat_map(|analysis| &analysis.claims)
        .flat_map(|claim| {
            claim
                .target_citations
                .iter()
                .chain(&claim.reference_citations)
        })
        .map(|citation| (citation.work_ref, citation.citation_id.clone()))
        .collect();
    for material in &mut materials {
        material
            .fragments
            .retain(|source| selected.contains(&(material.work_ref, source.citation_id.clone())));
    }
    let response = ComparisonRead {
        request_ref,
        batch_ref: row.get("batch_ref"),
        knowledge_cutoff: row.get("cutoff"),
        state: row.get("state"),
        wait_reason: row.get("wait_reason"),
        failure_code: row.get("failure_code"),
        result,
        materials,
        outdated,
        method_version: CONTRACT,
    };
    tx.commit().await?;
    Ok(response)
}
