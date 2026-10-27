//! Explicit synthetic accepted semantic record, solely for frozen-reader source qualification.
//! No provider, executor or legacy tables are involved.
use super::support::D;
use crate::{
    comment_study_selection::{CommentKey, prepare_study_input},
    comment_study_source::{self, StudySourceRole},
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
pub(super) async fn seed_signal(
    db: &Database,
    config: Uuid,
    source: Uuid,
    _parent: Uuid,
) -> String {
    let request = Uuid::new_v4();
    let command = Uuid::new_v4();
    let batch = Uuid::new_v4();
    let item = Uuid::new_v4();
    let invocation = Uuid::new_v4();
    let mut owner = crate::ai_foundation::executor::ExecutorSession::acquire(
        db,
        "synthetic-semantic-reader-fixture",
        4,
    )
    .await
    .unwrap();
    let fence = owner.fence();
    let mut tx = fence.begin(db, true).await.unwrap();
    let work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut tx, None)
        .await
        .unwrap();
    let role:String=sqlx::query_scalar("SELECT role FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2 ORDER BY role LIMIT 1").bind(work).bind(D).fetch_one(&mut *tx).await.unwrap();
    let selected = comment_study_source::selected_source_in(
        &mut tx,
        D,
        work,
        source,
        if role == "primary" {
            StudySourceRole::Primary
        } else {
            StudySourceRole::Reference
        },
        &cutoff,
    )
    .await
    .unwrap()
    .unwrap();
    let prepared = prepare_study_input(
        &CommentKey {
            work_ref: work,
            comment_external_id: selected.comment_external_id,
        },
        source,
        &selected.raw_text,
        &selected.source.context_manifest,
        selected.parent.as_ref(),
    )
    .unwrap();
    let task: Uuid = sqlx::query_scalar(
        "SELECT current_config_ref FROM linggan_ai_task_setting WHERE task_key='comment_study'",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_ai_command(command_ref,operation,payload,receipt) VALUES($1,'SYNTHETIC_SEMANTIC_FIXTURE','{}','{}')").bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch(batch_ref,task_key,task_config_ref,kind,inputs_sha256,item_count,command_ref) VALUES($1,'comment_study',$2,'interactive',repeat('0',64),1,$3)").bind(batch).bind(task).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,0,'SYNTHETIC_ACCEPTED_SEMANTIC',$3)").bind(item).bind(batch).bind(json!({"kind":"comment_research","requestRef":request})).execute(&mut *tx).await.unwrap();
    let result = json!({"contract":"comment-study.semantic.v1","outcome":"signals","reason":null,"signals":[{"kind":"experience","proposition":"SYNTHETIC later accepted Signal","evidenceSpan":{"start":0,"end":selected.raw_text.chars().count()},"sourceRef":source,"problemFrame":null,"eligibilityState":"not_applicable","eligibilityReason":null}]});
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at) SELECT $1,m.connection_version_ref,m.model_ref,$2,'analyze',repeat('0',64),'succeeded',0,0,$3,scope_001_now() FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$2").bind(invocation).bind(config).bind(json!({"synthetic":true,"notProviderOutput":true})).execute(&mut *tx).await.unwrap();
    // Synthetic settlement follows normal pending → claimed → attempted → terminal
    // transitions; immutable controls and terminal guards stay enabled.
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=scope_001_now()+interval '1 minute',started_at=scope_001_now() WHERE item_ref=$1").bind(item).bind(Uuid::new_v4()).bind(fence.instance_ref()).bind(fence.generation()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET attempt_count=1,invocation_ref=$2,executed_config_ref=$3,execution_contract='comment-research-semantic.v1' WHERE item_ref=$1").bind(item).bind(invocation).bind(task).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',usable=true,output=$2,finished_at=scope_001_now(),lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL WHERE item_ref=$1").bind(item).bind(&result).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version) VALUES($1,$2,$3,$4,$5,$6,$7::timestamptz,$8,$9,$10,$11,'comment-research-semantic.v1')").bind(request).bind(command).bind(D).bind(work).bind(source).bind(role).bind(cutoff).bind(prepared.input_hash).bind(prepared.input_fingerprint).bind(task).bind(batch).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_research_stage(item_ref,request_ref,stage_number) VALUES($1,$2,0)").bind(item).bind(request).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result) VALUES($1,$2,$3)").bind(request).bind(invocation).bind(result).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    format!("{request}:0")
}
