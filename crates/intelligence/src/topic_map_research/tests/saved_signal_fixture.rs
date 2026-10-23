//! Synthetic accepted Comment Study state for frozen-reader qualification only.
//! Uses the real method compiler and schema constraints; no model is dispatched.
use super::support::D;
use crate::comment_study_policy::{
    CreateStudyPolicyCommand, StudyModelIdentity, StudyModelSnapshot, compile_study_method,
};
use linggan_evidence::creator_discovery::hash;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

pub(super) async fn install_schema(db: &Database) {
    // Production initializes this derived layer separately from retained Evidence.
    // Use its real bootstrap and ordered deltas, without optional-table fallbacks.
    sqlx::raw_sql(concat!(
        include_str!("../../../../../database/bootstrap/comment-study-001.sql"),
        include_str!(
            "../../../../../database/migrations/0107_comment_study_productization_schema.sql"
        ),
        include_str!(
            "../../../../../database/migrations/0108_comment_study_policy_constraints.sql"
        ),
        include_str!("../../../../../database/migrations/0109_comment_study_start_constraints.sql"),
        include_str!(
            "../../../../../database/migrations/0110_comment_study_request_snapshot_constraints.sql"
        ),
        include_str!(
            "../../../../../database/migrations/0111_comment_study_pair_failure_state.sql"
        ),
        include_str!(
            "../../../../../database/migrations/0112_comment_study_membership_revision.sql"
        ),
        include_str!("../../../../../database/migrations/0114_comment_study_effective_head.sql"),
        include_str!("../../../../../database/migrations/0146_comment_study_full_context.sql"),
    ))
    .execute(db.pool())
    .await
    .unwrap();
}

pub(super) async fn seed_signal(
    db: &Database,
    original_config: Uuid,
    source: Uuid,
    parent: Uuid,
) -> Uuid {
    let (config, policy, run, target, attempt, signal) = (
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
    );
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,model_ref,capability_ref,32768,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$2")
        .bind(config).bind(original_config).execute(&mut *tx).await.unwrap();
    let row = sqlx::query("SELECT model.model_ref,model.connection_version_ref,model.model_id,config.input_token_limit,config.output_token_limit,config.timeout_seconds FROM linggan_model_config config JOIN linggan_model_entry model USING(model_ref) WHERE config.config_ref=$1")
        .bind(config).fetch_one(&mut *tx).await.unwrap();
    let model = StudyModelSnapshot {
        model_config_ref: config,
        identity: StudyModelIdentity {
            model_ref: row.get("model_ref"),
            connection_version_ref: row.get("connection_version_ref"),
            model_id: row.get("model_id"),
        },
        input_token_limit: row.get("input_token_limit"),
        output_token_limit: row.get("output_token_limit"),
        timeout_seconds: row.get("timeout_seconds"),
    };
    let command: CreateStudyPolicyCommand = serde_json::from_value(json!({
        "domainRef":D,"methodName":"SYNTHETIC accepted Signal read fixture","parentPolicyRef":null,
        "modelConfigRef":config,"defaults":{"commentBudget":1},
        "stageInstructions":{"semantic":"","resolution":"","pair":""}
    }))
    .unwrap();
    let method = compile_study_method(&command, &model).unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,model_config_ref,contract,comment_budget,method_name,method_manifest,method_hash) VALUES($1,$2,$3,'comment-study.v1',1,$4,$5,$6)")
        .bind(policy).bind(D).bind(config).bind(&command.method_name).bind(json!(method.manifest)).bind(&method.method_hash).execute(&mut *tx).await.unwrap();
    let source_row = sqlx::query("SELECT content_public_ref,comment_external_id,body_text FROM linggan_material_comment WHERE material_ref=$1")
        .bind(source).fetch_one(&mut *tx).await.unwrap();
    let work: Uuid = source_row.get("content_public_ref");
    let text: String = source_row.get("body_text");
    let selection = json!({"contract":"comment-study.run-selection.v2","commentBudget":1,"tokenLimit":65536,
        "requestedWorkRefs":[work],"coveredWorkRefs":[work],"targetSourceRefs":[source]});
    let execution = json!({"contract":"comment-study.execution.v1","methodHash":method.method_hash,
        "builderRevision":method.manifest.builder_revision,"engineRevision":"0".repeat(40),
        "selectionOrder":"work_round_robin_oldest_first.v1","maxTargetsPerBatch":12,
        "fixture":"SYNTHETIC accepted state; no execution"});
    sqlx::query("INSERT INTO linggan_comment_study_run(run_ref,policy_ref,as_of,state,selection_manifest,selection_hash,comment_budget,token_limit,execution_manifest,dispatch_state,dispatch_reason) VALUES($1,$2,scope_001_now(),'queued',$3,$4,1,65536,$5,'enabled',NULL)")
        .bind(run).bind(policy).bind(&selection).bind(hash(&selection.to_string())).bind(execution).execute(&mut *tx).await.unwrap();
    let context = json!({"workRef":work,"sources":[],"fixture":"SYNTHETIC empty author context"});
    let context_hash = hash(&context.to_string());
    sqlx::query("INSERT INTO linggan_comment_study_work(run_ref,content_public_ref,domain_ref,observation_role,selection_reason,context_state,context_manifest,context_hash) VALUES($1,$2,$3,'reference','user_selected','missing',$4,$5)")
        .bind(run).bind(work).bind(D).bind(context).bind(&context_hash).execute(&mut *tx).await.unwrap();
    let input = json!({"contract":"comment-study.target-input.v2","targetSourceRef":source,"workRef":work,"workContextHash":context_hash});
    let input_hash = hash(&input.to_string());
    sqlx::query("INSERT INTO linggan_comment_study_target(target_ref,run_ref,content_public_ref,source_ref,parent_source_ref,research_text,research_sha256,dependency_state,state,input_manifest,input_hash,comment_external_id,input_fingerprint,finished_at) VALUES($1,$2,$3,$4,$5,$6,$7,'parent_available','succeeded',$8,$9,$10,$9,scope_001_now())")
        .bind(target).bind(run).bind(work).bind(source).bind(parent).bind(&text).bind(hash(&text)).bind(input).bind(&input_hash).bind(source_row.get::<String,_>("comment_external_id")).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_semantic_attempt(attempt_ref,target_ref,attempt_ordinal,request_hash,state,output_manifest,finished_at) VALUES($1,$2,1,$3,'accepted',$4,scope_001_now())")
        .bind(attempt).bind(target).bind(&input_hash).bind(json!({"synthetic":true,"notProviderOutput":true})).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_signal(signal_ref,target_ref,semantic_attempt_ref,kind,proposition,evidence,evidence_start,evidence_end,eligibility_state) VALUES($1,$2,$3,'experience','SYNTHETIC later accepted Signal',$4,0,$5,'not_applicable')")
        .bind(signal).bind(target).bind(attempt).bind(&text).bind(i32::try_from(text.chars().count()).unwrap()).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_comment_study_run SET state='completed',finished_at=scope_001_now(),dispatch_state='stopped',dispatch_reason='user_stopped' WHERE run_ref=$1")
        .bind(run).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    signal
}
