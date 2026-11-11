mod deep;
mod deep_grounding;
pub(crate) mod generate_deep;
mod source;
use super::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::content_understanding::{self as contract, METHOD};
pub use deep::{DeepReceipt, PublishDeep, publish_deep};
pub use generate_deep::{GenerateDeep, generate_deep};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) use source::{Packet, frozen_in, images_in};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const TASK: TaskKey = TaskKey::ContentUnderstanding;
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Start {
    pub command_ref: Uuid,
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub knowledge_cutoff: String,
    pub input_hash: String,
    pub task_config_ref: Uuid,
    pub task_revision: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub request_ref: Uuid,
    pub batch_ref: Uuid,
    pub item_ref: Uuid,
    pub reused: bool,
}
async fn configuration(tx: &mut Transaction<'_, Postgres>) -> Result<(Uuid, i64), FoundationError> {
    Ok(sqlx::query_as("SELECT current_config_ref,revision FROM linggan_ai_task_setting WHERE task_key='content_understanding'").fetch_one(&mut **tx).await?)
}
pub(crate) fn payload(p: &Packet) -> Value {
    json!({"material":p.snapshot,"imageInventory":p.inventory,"resultSchema":contract::response_schema()})
}
async fn reasons(
    tx: &mut Transaction<'_, Postgres>,
    config: Uuid,
    p: &Packet,
) -> Result<Vec<String>, FoundationError> {
    let mut reasons = Vec::new();
    for outcome in [
        admission::check_queue_configuration(tx, TASK, config, false).await,
        admission::check_queue_input_scope(
            tx,
            TASK,
            config,
            &InputRef::ContentUnderstanding {
                request_ref: Uuid::nil(),
            },
        )
        .await,
    ] {
        match outcome {
            Ok(()) => {}
            Err(FoundationError::AdmissionDenied(c)) => reasons.extend(c),
            Err(e) => return Err(e),
        }
    }
    if p.coverage["materialReadiness"]["state"] != "ready" {
        reasons.push("materials_not_ready".into());
    }
    if p.snapshot.images.is_empty()
        && !p
            .snapshot
            .texts
            .iter()
            .any(|t| ["body", "transcript", "frame_text"].contains(&t.kind.as_str()))
    {
        reasons.push("no_readable_material".into());
    }
    let limits: Option<(String,i32)> = sqlx::query_as("SELECT m.thinking_level,m.input_token_limit FROM linggan_ai_task_config c JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE c.task_config_ref=$1")
        .bind(config).fetch_optional(&mut **tx).await?;
    if let Some((thinking, limit)) = limits {
        if thinking != "high" {
            reasons.push("content_understanding_requires_high".into());
        }
        let (_, mut system, instruction, skill) =
            super::prompt::frozen_in(tx, config, TASK).await?;
        if let Some(skill) = skill {
            system.push_str("\n\n");
            system.push_str(&skill);
        }
        let mut body = payload(p);
        body["instruction"] = json!(instruction);
        if super::transport::estimate_input_tokens(&system, &body.to_string())
            + p.snapshot.images.len() as u64 * 1024
            > limit as u64
        {
            reasons.push("input_too_large".into());
        }
    }
    let unknown: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.work_ref=$1 AND r.material_snapshot_identity=$2 AND r.method_version=$3 AND c.failure_code='ai_dispatch_unknown')")
        .bind(p.snapshot.work_ref).bind(&p.snapshot.material_snapshot_identity).bind(METHOD).fetch_one(&mut **tx).await?;
    if unknown {
        reasons.push("ai_prior_dispatch_unresolved".into());
    }
    Ok(reasons)
}
pub async fn preview(db: &Database, domain: Uuid, work: Uuid) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None).await?;
    let p = source::load(&mut tx, domain, work, &cutoff).await?;
    let (config, revision) = configuration(&mut tx).await?;
    let reasons = reasons(&mut tx, config, &p).await?;
    let result = json!({"contract":"content-understanding.preview.v1","workRef":work,"domainRef":domain,
        "knowledgeCutoff":cutoff,"inputHash":p.hash(),"taskConfigRef":config,"taskRevision":revision,
        "methodVersion":METHOD,"canStart":reasons.is_empty(),"reasons":reasons,
        "imageCount":p.snapshot.images.len(),"coverage":p.coverage,"materialReadiness":p.coverage["materialReadiness"]});
    tx.commit().await?;
    Ok(result)
}
pub async fn start(db: &Database, r: &Start) -> Result<Receipt, FoundationError> {
    if r.domain_ref.is_nil() || r.work_ref.is_nil() || r.command_ref.is_nil() || r.task_revision < 0
    {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, "content_understanding", r).await?;
    if let Some(v) = c.replay()? {
        return Ok(v);
    }
    let config = lock_setting(&mut c.tx, TASK, r.task_revision).await?;
    if config != r.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let cutoff =
        linggan_evidence::research_dossier::cutoff_in(&mut c.tx, Some(&r.knowledge_cutoff)).await?;
    let p = source::load(&mut c.tx, r.domain_ref, r.work_ref, &cutoff).await?;
    if p.hash() != r.input_hash {
        return Err(FoundationError::Conflict);
    }
    let now = linggan_evidence::research_dossier::cutoff_in(&mut c.tx, None).await?;
    let current = source::load(&mut c.tx, r.domain_ref, r.work_ref, &now).await?;
    if current.snapshot.material_snapshot_identity != p.snapshot.material_snapshot_identity {
        return Err(FoundationError::Conflict);
    }
    if current.coverage["materialReadiness"]["state"] != "ready" {
        return Err(FoundationError::AdmissionDenied(vec![
            "materials_not_ready".into(),
        ]));
    }
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,196))")
        .bind(format!(
            "{}:{}:{}",
            r.work_ref, p.snapshot.material_snapshot_identity, METHOD
        ))
        .execute(&mut *c.tx)
        .await?;
    let existing = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_content_understanding_result result USING(request_ref) WHERE r.work_ref=$1 AND r.material_snapshot_identity=$2 AND r.method_version=$3 AND (c.state IN ('pending','running') OR result.result IS NOT NULL) ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 1")
        .bind(r.work_ref).bind(&p.snapshot.material_snapshot_identity).bind(METHOD).fetch_optional(&mut *c.tx).await?;
    if let Some(existing) = existing {
        return c
            .finish(Receipt {
                request_ref: existing.get("request_ref"),
                batch_ref: existing.get("batch_ref"),
                item_ref: existing.get("item_ref"),
                reused: true,
            })
            .await;
    }
    let denied = reasons(&mut c.tx, config, &p).await?;
    if !denied.is_empty() {
        return Err(FoundationError::AdmissionDenied(denied));
    }
    let request = Uuid::new_v4();
    let inputs = vec![BatchInput {
        item_key: request.to_string(),
        input: InputRef::ContentUnderstanding {
            request_ref: request,
        },
    }];
    let hash = batch_store::input_hash(TASK, &inputs, false)?;
    let batch = batch_store::insert_batch(
        &mut c.tx,
        r.command_ref,
        TASK,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(batch)
            .fetch_one(&mut *c.tx)
            .await?;
    sqlx::query("INSERT INTO linggan_content_understanding_request(request_ref,command_ref,domain_ref,work_ref,material_snapshot_identity,method_version,knowledge_cutoff,input_hash,input_manifest,task_config_ref,batch_ref,item_ref) VALUES($1,$2,$3,$4,$5,$6,$7::timestamptz,$8,$9,$10,$11,$12)")
        .bind(request).bind(r.command_ref).bind(r.domain_ref).bind(r.work_ref).bind(&p.snapshot.material_snapshot_identity).bind(METHOD).bind(cutoff).bind(p.hash()).bind(json!(p)).bind(config).bind(batch).bind(item).execute(&mut *c.tx).await?;
    c.finish(Receipt {
        request_ref: request,
        batch_ref: batch,
        item_ref: item,
        reused: false,
    })
    .await
}
pub async fn latest(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    as_of: Option<&str>,
) -> Result<Value, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, as_of).await?;
    let p = source::load(&mut tx, domain, work, &cutoff).await?;
    let row = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref,c.state,c.failure_code,result.result,
        d.analysis_ref AS deep_ref,d.method_version AS deep_method,d.findings,d.question
        FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control c USING(item_ref)
        LEFT JOIN linggan_content_understanding_result result USING(request_ref)
        LEFT JOIN LATERAL (
          SELECT analysis_ref,method_version,findings,question FROM linggan_content_understanding_deep
          WHERE core_ref=result.request_ref AND domain_ref=$4 AND created_at<=$5::timestamptz
          ORDER BY revision DESC LIMIT 1
        ) d ON true
        WHERE r.work_ref=$1 AND r.material_snapshot_identity=$2 AND r.method_version=$3
        ORDER BY (result.result IS NOT NULL) DESC,r.created_at DESC,r.request_ref DESC LIMIT 1")
        .bind(work).bind(&p.snapshot.material_snapshot_identity).bind(METHOD).bind(domain).bind(&cutoff)
        .fetch_optional(&mut *tx).await?;
    let result = match row {
        Some(r) => {
            let result = r.get::<Option<Value>, _>("result");
            let mut analysis = json!({
                "requestRef":r.get::<Uuid,_>("request_ref"),"batchRef":r.get::<Uuid,_>("batch_ref"),
                "itemRef":r.get::<Uuid,_>("item_ref"),"state":r.get::<String,_>("state"),
                "failureCode":r.get::<Option<String>,_>("failure_code"),"result":result
            });
            if let Some(deep_ref) = r.get::<Option<Uuid>, _>("deep_ref") {
                analysis["result"]["card"]["deep"] = json!({
                    "analysisRef":deep_ref,"methodVersion":r.get::<String,_>("deep_method"),
                    "findings":r.get::<Value,_>("findings")
                });
                analysis["deepQuestion"] = json!(r.get::<String, _>("question"));
            }
            analysis
        }
        None => Value::Null,
    };
    tx.commit().await?;
    Ok(
        json!({"contract":"content-understanding.read.v1","workRef":work,"analysis":result,"materialReadiness":p.coverage["materialReadiness"]}),
    )
}
