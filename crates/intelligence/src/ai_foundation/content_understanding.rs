mod core_revision;
pub use core_revision::{CoreRevisionReceipt, ReviewedCore, publish_reviewed_core};
mod coverage;
pub(crate) use coverage::scope_coverage;
mod deep;
mod deep_grounding;
mod deep_manifest;
mod deep_v2;
pub(crate) mod generate_deep;
mod offline_recovery;
mod source;
use super::{
    TaskKey, admission,
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::{Command, FoundationError},
    task_config::lock_setting,
};
use crate::content_understanding::{self as contract, METHOD};
pub use deep::{DeepReceipt, PublishDeep, publish_deep};
pub(crate) use deep_manifest::build as build_manifest;
pub use generate_deep::{GenerateDeep, generate_deep};
use linggan_storage_postgres::Database;
pub use offline_recovery::{RecoveryReceipt, recover_saved_replies};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
pub(crate) use source::{Packet, frozen_in, images_for_manifest_in, images_in};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeSet;
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
    json!({"draftProtocol":contract::model_draft::PROTOCOL,"material":contract::model_draft::material(&p.snapshot),"imageInventory":p.inventory,"resultSchema":contract::response_schema()})
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
            .any(|t| ["body", "transcript"].contains(&t.kind.as_str()))
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
async fn lock_work_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    material_identity: &str,
) -> Result<(), FoundationError> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,196))")
        .bind(format!("{work}:{material_identity}:{METHOD}"))
        .execute(&mut **tx)
        .await?;
    Ok(())
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
    lock_work_snapshot(
        &mut c.tx,
        r.work_ref,
        &p.snapshot.material_snapshot_identity,
    )
    .await?;
    let existing = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control c USING(item_ref) LEFT JOIN linggan_content_understanding_saved_core result USING(request_ref) WHERE r.work_ref=$1 AND r.material_snapshot_identity=$2 AND r.method_version=$3 AND (c.state IN ('pending','running') OR result.result IS NOT NULL) ORDER BY r.created_at DESC,r.request_ref DESC LIMIT 1")
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
/// Current L2 method, default sub-questions and default material kinds.
pub fn deep_modes() -> Value {
    let modes: Vec<Value> = deep_manifest::MODES
        .iter()
        .map(|mode| {
            let material_kinds: &[&str] = match *mode {
                "creation_method" => &["core_unit", "core_evidence", "title", "body", "tag", "asr"],
                _ => &[
                    "core_unit",
                    "core_evidence",
                    "title",
                    "body",
                    "tag",
                    "asr",
                    "cover",
                ],
            };
            json!({
                "researchMode":mode,
                "defaultSubQuestions":deep_manifest::default_sub_questions(mode),
                "defaultMaterialKinds":material_kinds
            })
        })
        .collect();
    json!({
        "contract":"content-understanding.deep-modes.v1",
        "methodVersion":generate_deep::METHOD,
        "imageLimit":deep_manifest::MAX_IMAGES,
        "modes":modes
    })
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
    let row = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref,
        CASE WHEN result.offline_recovery THEN 'succeeded' ELSE c.state END AS state,
        CASE WHEN result.offline_recovery THEN NULL ELSE c.failure_code END AS failure_code,
        COALESCE(review.result,result.result) AS result, review.revision_ref,
        COALESCE(result.offline_recovery,false) AS offline_recovery,
        deeps.list AS deep_analyses
        FROM linggan_content_understanding_request r JOIN linggan_ai_batch_item_control c USING(item_ref)
        LEFT JOIN linggan_content_understanding_saved_core result ON result.request_ref=r.request_ref
          AND (NOT result.offline_recovery OR result.created_at<=$5::timestamptz)
        LEFT JOIN LATERAL (
          SELECT revision_ref,result FROM linggan_content_understanding_core_revision
          WHERE base_request_ref=result.request_ref AND created_at<=$5::timestamptz
          ORDER BY created_at DESC,revision_ref DESC LIMIT 1
        ) review ON true
        LEFT JOIN LATERAL (
          SELECT jsonb_agg(jsonb_build_object(
            'analysisRef',d.analysis_ref,'coreRef',d.core_ref,'domainRef',d.domain_ref,
            'researchMode',q.research_mode,'baseMode',q.base_mode,'question',d.question,
            'subQuestions',q.sub_questions,'subQuestionStatus',d.sub_question_status,
            'methodVersion',d.method_version,'skillVersion',ic.executed_skill_version_ref,
            'findings',d.findings,'coreBasis',q.core_basis,'inputManifest',q.input_manifest,
            'createdAt',to_char(d.created_at AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'))
            ORDER BY d.created_at DESC,d.revision DESC) AS list
          FROM linggan_content_understanding_deep d
          LEFT JOIN linggan_content_understanding_deep_request q ON q.request_ref=d.analysis_ref
          LEFT JOIN linggan_ai_batch_item_control ic ON ic.item_ref=q.item_ref
          WHERE d.core_ref=result.request_ref AND d.domain_ref=$4 AND d.created_at<=$5::timestamptz
        ) deeps ON true
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
            if r.get::<bool, _>("offline_recovery") {
                analysis["recoveredFromSavedReply"] = json!(true);
            }
            if let Some(revision_ref) = r.get::<Option<Uuid>, _>("revision_ref") {
                analysis["coreRevisionRef"] = json!(revision_ref);
            }
            // v2 records are all returned; historical v1/manual records project
            // as one entry (the newest) with null mode and no manifest.
            let deeps = r
                .get::<Option<Value>, _>("deep_analyses")
                .unwrap_or_else(|| json!([]));
            let mut analyses = Vec::new();
            let mut historical = false;
            for entry in deeps.as_array().into_iter().flatten() {
                let v2 = entry["methodVersion"]
                    .as_str()
                    .is_some_and(|version| version == "content-understanding.l2.model.v2");
                if !v2 {
                    if historical {
                        continue;
                    }
                    historical = true;
                }
                let mut refs = BTreeSet::new();
                for finding in entry["findings"].as_array().into_iter().flatten() {
                    for support in finding["support"].as_array().into_iter().flatten() {
                        if let Some(reference) = support["materialRef"].as_str() {
                            refs.insert(reference.to_string());
                        }
                    }
                }
                let materials: Vec<Value> = entry["inputManifest"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|material| material["id"].as_str().is_some_and(|id| refs.contains(id)))
                    .cloned()
                    .collect();
                analyses.push(json!({
                    "analysisRef":entry["analysisRef"],"coreRef":entry["coreRef"],
                    "coreBasis":entry["coreBasis"],"domainRef":entry["domainRef"],
                    "researchMode":entry["researchMode"],"baseMode":entry["baseMode"],
                    "question":entry["question"],"subQuestions":entry["subQuestions"],
                    "subQuestionStatus":entry["subQuestionStatus"],
                    "methodVersion":entry["methodVersion"],"skillVersion":entry["skillVersion"],
                    "findings":entry["findings"],"materials":materials,
                    "createdAt":entry["createdAt"]
                }));
            }
            analysis["deepAnalyses"] = json!(analyses);
            analysis
        }
        None => Value::Null,
    };
    tx.commit().await?;
    Ok(
        json!({"contract":"content-understanding.read.v1","workRef":work,"analysis":result,"materialReadiness":p.coverage["materialReadiness"]}),
    )
}

/// Qualified saved Core cards, newest first. This does not schedule understanding or L2.
/// A consumer freezes analysisRef and subsequently selects that same saved Core.
pub(crate) async fn core_cards_at(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    cutoff: &str,
) -> Result<Vec<Value>, FoundationError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cards = core_cards_in(&mut tx, domain, work, cutoff).await?;
    tx.commit().await?;
    Ok(cards)
}

pub(crate) async fn core_cards_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
    cutoff: &str,
) -> Result<Vec<Value>, FoundationError> {
    let rows = sqlx::query("SELECT result->'card' AS card,material_snapshot_identity FROM linggan_content_understanding_saved_core WHERE work_ref=$1 AND method_version=$2 AND result IS NOT NULL AND created_at<=$3::timestamptz ORDER BY created_at DESC,request_ref DESC")
        .bind(work).bind(METHOD).bind(cutoff).fetch_all(&mut **tx).await?;
    if rows.is_empty() {
        return Ok(vec![]);
    }
    let packet = match source::load(tx, domain, work, cutoff).await {
        Ok(packet) => packet,
        Err(FoundationError::NotFound) => return Ok(vec![]),
        Err(error) => return Err(error),
    };
    let ready = packet.coverage["materialReadiness"]["state"] == "ready";
    Ok(rows
        .into_iter()
        .filter(|row| {
            coverage::qualified(
                &row.get::<String, _>("material_snapshot_identity"),
                &packet.snapshot.material_snapshot_identity,
                ready,
            )
        })
        .map(|row| row.get("card"))
        .collect())
}

#[cfg(test)]
#[path = "content_understanding/topic_tests.rs"]
mod topic_tests;
