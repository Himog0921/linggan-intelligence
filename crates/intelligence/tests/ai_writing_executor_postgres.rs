//! Actual queued writing and skill comparison with owned PostgreSQL/HTTP and fixed SDK only.
#![recursion_limit = "256"]
#[path = "support/ai_writing_endpoint.rs"]
mod endpoint;
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        batch_store::{BatchInput, CreateTrial, InputRef, create_trial},
        control::BudgetPolicy,
        control_store::{Actor, ChangeControl, ControlScope, change_control},
        executor::{
            ExecutorError, ExecutorSession,
            queue::{claim_next, recover_expired},
            writing::execute_writing,
        },
        settings_read::read_comparison,
        skill_binding::{ActivateSkill, activate_skill},
        skill_package::{Encoding, SkillPackage, UploadFile},
        skill_store::{ImportSkill, import_skill},
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    },
    content_workbench::*,
    content_workbench_ai as ai,
    content_workbench_sources::{read_source_in, text_hash},
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::{Value, json};
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;
fn adapter() -> PiAdapter {
    PiAdapter::configured_with_test_command(
        PathBuf::from(
            std::env::var_os("AI_FOUNDATION_PROOF_NODE").expect("owned proof runner supplies Node"),
        ),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../apps/pi-adapter/src/adapter.mjs"),
    )
}

async fn pre_writing_bridge_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    // This historical subset extends through 0150 but deliberately
    // omits creation 0127/0128 and writing cutovers 0140/0141/0148, applied below.
    isolated_proof_schema(&url, schema, fixture::dossier::fixture::MIGRATIONS)
        .await
        .expect("pre-writing-bridge proof schema is created")
}

#[tokio::test]
#[ignore = "disposable PostgreSQL contract proof; no provider or child process"]
async fn dossier_contract_cutover_keeps_history_inert_and_accepts_v2_preview() {
    let db = fixture::db("dossier_consumer_contracts").await;
    let columns: Value = sqlx::query_scalar("SELECT c.material_columns FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='note_interpretation'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        columns,
        json!(["title", "body", "tags", "ocr", "transcript", "frame_text"])
    );
    let supported: (bool, bool) = sqlx::query_as("SELECT linggan_ai_columns_valid('[\"frame_text\"]'),linggan_ai_columns_valid('[\"cover_headline\"]')")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(supported, (true, false));
    let archived: Uuid = sqlx::query_scalar("SELECT task_config_ref FROM linggan_ai_task_config WHERE task_key='note_interpretation' AND material_columns ? 'cover_headline' ORDER BY config_sequence LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    assert!(sqlx::query("UPDATE linggan_ai_task_setting SET current_config_ref=$1,revision=revision+1 WHERE task_key='note_interpretation'")
        .bind(archived).execute(db.pool()).await.unwrap_err().to_string().contains("ai_dossier_configuration_required"));
    let active_contracts: (String, String) = sqlx::query_as("SELECT t.handler_contract,c.execution_contract FROM linggan_ai_task t JOIN linggan_ai_task_setting s USING(task_key) JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE t.task_key='writing_legacy'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(active_contracts, (ai::METHOD.into(), ai::METHOD.into()));

    let config = setup::declare(&db, "http://127.0.0.1:1").await;
    let work = fixture::create(&db, fixture::doc()).await;
    let request = preview(&work, config.task.model_config_ref.unwrap(), "");
    let result = ai::preview(&db, &request).await.unwrap();
    assert_eq!(result["methodVersion"], ai::METHOD);
    let persisted: String = sqlx::query_scalar(
        "SELECT method_version FROM linggan_creation_ai_preview WHERE preview_ref=$1",
    )
    .bind(request.preview_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(persisted, ai::METHOD);
    let stale = sqlx::query("INSERT INTO linggan_creation_ai_preview(preview_ref,work_ref,input_revision,config_ref,request_hash,method_version,action,input_manifest,reserved_tokens,task_config_ref) SELECT $2,work_ref,input_revision,config_ref,request_hash,'content-workbench-writing.v1',action,input_manifest,reserved_tokens,task_config_ref FROM linggan_creation_ai_preview WHERE preview_ref=$1")
        .bind(request.preview_ref).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap_err();
    assert!(
        stale
            .to_string()
            .contains("ai_dossier_writing_contract_required")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at) VALUES($1,$2,$3,'probe',repeat('0',64),'succeeded',0,0,0,0,'{\"syntheticContractFixture\":true}',scope_001_now())")
        .bind(invocation).bind(config.version).bind(config.model).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_creation_writing_qualification(model_ref,connection_version_ref,method_version,invocation_ref,qualified) VALUES($1,$2,$3,$4,false)")
        .bind(config.model).bind(config.version).bind(ai::METHOD).bind(invocation).execute(db.pool()).await.unwrap();
    let stale_qualification = sqlx::query("INSERT INTO linggan_creation_writing_qualification(model_ref,connection_version_ref,method_version,invocation_ref,qualified) VALUES($1,$2,'content-workbench-writing.v1',$3,false)")
        .bind(config.model).bind(config.version).bind(invocation).execute(db.pool()).await.unwrap_err();
    assert!(
        stale_qualification
            .to_string()
            .contains("ai_dossier_writing_contract_required")
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL deferred-constraint migration proof; no provider"]
async fn dossier_contract_cutover_validates_pending_seed_constraints_before_ddl() {
    let db = pre_writing_bridge_database("dossier_deferred_seed").await;
    let mut tx = db.pool().begin().await.unwrap();
    for migration in [
        include_str!("../../../database/migrations/0127_content_workbench.sql"),
        include_str!("../../../database/migrations/0128_content_workbench_ai.sql"),
        include_str!("../../../database/migrations/0140_ai_gen1_writing.sql"),
        include_str!("../../../database/migrations/0141_ai_gen1_legacy_settings.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *tx).await.unwrap();
    }
    // Match a fresh install: a seed is inserted before the same transaction
    // reaches the cutover DDL, leaving its deferred command FK check queued.
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,material_columns,budget_mode,origin) VALUES($1,'note_interpretation','[\"title\",\"cover_headline\",\"body\",\"tags\",\"ocr\",\"transcript\"]','unconfigured','seed')")
        .bind(Uuid::new_v4()).execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0148_dossier_consumer_contracts.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let canonical: bool = sqlx::query_scalar("SELECT bool_and(linggan_ai_columns_valid(c.material_columns) AND (s.task_key<>'writing_legacy' OR c.execution_contract='content-workbench-writing.v2')) FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref")
        .fetch_one(&mut *tx).await.unwrap();
    assert!(canonical);
    tx.commit().await.unwrap();

    let mut invalid = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,material_columns,budget_mode,origin,command_ref) VALUES($1,'note_interpretation','[\"body\"]','unconfigured','seed',$2)")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&mut *invalid).await.unwrap();
    let error = sqlx::raw_sql("SET CONSTRAINTS linggan_ai_task_config_command_ref_fkey IMMEDIATE")
        .execute(&mut *invalid)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23503")
    );
    invalid.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL migration proof; synthetic legacy rows only"]
async fn writing_upgrade_rejects_old_inflight_without_rewriting_history_then_accepts_a_settled_legacy_receipt()
 {
    let db = pre_writing_bridge_database("ai_writing_upgrade").await;
    for migration in [
        include_str!("../../../database/migrations/0127_content_workbench.sql"),
        include_str!("../../../database/migrations/0128_content_workbench_ai.sql"),
    ] {
        sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    }
    fixture::dossier::model_config(&db).await;
    let (config,model,version):(Uuid,Uuid,Uuid)=sqlx::query_as("SELECT config_ref,model_ref,connection_version_ref FROM linggan_model_config JOIN linggan_model_entry USING(model_ref) LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    let work = fixture::create(&db, fixture::doc()).await;
    let request = preview(&work, config, "");
    sqlx::query("INSERT INTO linggan_creation_ai_preview(preview_ref,work_ref,input_revision,config_ref,request_hash,method_version,action,input_manifest,reserved_tokens)
        VALUES($1,$2,$3,$4,repeat('0',64),'content-workbench-writing.v1','draft',$5,1000)")
        .bind(request.preview_ref).bind(work.work_id).bind(work.revision.id).bind(config).bind(serde_json::to_value(&request).unwrap()).execute(db.pool()).await.unwrap();
    let invocation = Uuid::new_v4();
    let reference = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,config_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result)
        VALUES($1,$2,$3,$4,'analyze',repeat('0',64),'running',1000,1000,'{\"purpose\":\"content_workbench\",\"callStarted\":false,\"syntheticLegacyFixture\":true}')")
        .bind(invocation).bind(version).bind(model).bind(config).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_creation_ai_request(request_ref,command_hash,preview_ref,invocation_ref,state) VALUES($1,repeat('0',64),$2,$3,'running')")
        .bind(reference).bind(request.preview_ref).bind(invocation).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let failure = sqlx::raw_sql(include_str!(
        "../../../database/migrations/0140_ai_gen1_writing.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("ai_writing_bridge_preflight_required")
    );
    tx.rollback().await.unwrap();
    let unchanged:(String,i64,Option<String>)=sqlx::query_as("SELECT state,charged_tokens,finished_at::text FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(unchanged, ("running".into(), 1000, None));
    let partial:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='linggan_creation_ai_request' AND column_name='task_config_ref')")
        .fetch_one(db.pool()).await.unwrap();
    assert!(!partial);
    let mut cutover = db.pool().begin().await.unwrap();
    let missing_bridge = sqlx::raw_sql(include_str!(
        "../../../database/migrations/0141_ai_gen1_legacy_settings.sql"
    ))
    .execute(&mut *cutover)
    .await
    .unwrap_err();
    assert!(
        missing_bridge
            .to_string()
            .contains("ai_settings_cutover_requires_writing_bridge")
    );
    cutover.rollback().await.unwrap();
    // A synthetic old owner explicitly finishes its known no-send call. The migration does not.
    sqlx::query("UPDATE linggan_model_invocation SET state='failed',finished_at=scope_001_now() WHERE invocation_ref=$1").bind(invocation).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_creation_ai_request SET state='failed',finished_at=scope_001_now() WHERE request_ref=$1").bind(reference).execute(db.pool()).await.unwrap();
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM linggan_model_invocation i WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0140_ai_gen1_writing.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let after: Value = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM linggan_model_invocation i WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../database/migrations/0141_ai_gen1_legacy_settings.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let legacy_columns:i64=sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema=current_schema() AND table_name='linggan_model_workspace' AND column_name IN('default_config_ref','worker_last_seen_at','worker_state','worker_last_error')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(legacy_columns, 0);
    let after_cutover: Value = sqlx::query_scalar(
        "SELECT to_jsonb(i) FROM linggan_model_invocation i WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, after_cutover);
    assert_eq!(before, after);
    let historical = ai::status(&db, reference).await.unwrap();
    assert_eq!(historical["state"], "failed");
    assert_eq!(historical["invocationRef"], json!(invocation));
}
async fn ready(
    db: &Database,
    endpoint: &endpoint::Endpoint,
    owner: &ExecutorSession,
) -> setup::Configured {
    setup::prepare_with_actual_probe(db, &endpoint.url, owner, &adapter()).await
}

fn preview(work: &SaveReceipt, config: Uuid, instruction: &str) -> ai::PreviewRequest {
    ai::PreviewRequest {
        selection: None,
        preview_ref: Uuid::new_v4(),
        work_ref: work.work_id,
        expected_revision: work.revision.id,
        action: ai::WritingAction::Draft,
        config_ref: config,
        citations: work
            .revision
            .document
            .excerpts
            .iter()
            .map(|e| e.citation.clone())
            .collect(),
        paragraph_ids: vec![],
        instruction: instruction.into(),
    }
}
async fn enqueue(db: &Database, request: &ai::PreviewRequest) -> ai::ExecuteRequest {
    let preview = ai::preview(db, request).await.unwrap();
    assert_eq!(preview["reservedTokens"], 8192 + 1000);
    assert!(preview["estimatedInputTokens"].as_u64().unwrap() < 9192);
    let confirmation = ai::ExecuteRequest {
        request_id: Uuid::new_v4(),
        preview_ref: request.preview_ref,
        expected_revision: request.expected_revision,
        confirmed_input_hash: preview["inputHash"].as_str().unwrap().into(),
        budget_tokens: preview["reservedTokens"].as_i64().unwrap(),
        confirmed: true,
    };
    let accepted = ai::execute(db, &confirmation).await.unwrap();
    assert_eq!(accepted["state"], "queued");
    assert!(accepted["invocationRef"].is_null());
    assert_eq!(accepted["usage"]["state"], "not_reserved");
    confirmation
}
async fn run(db: &Database, owner: &ExecutorSession) {
    let item = claim_next(db, owner.fence()).await.unwrap().unwrap();
    assert_eq!(item.task_key, TaskKey::WritingLegacy);
    tokio::time::timeout(
        Duration::from_secs(15),
        execute_writing(db, &item, &SyntheticModelSecrets, &adapter()),
    )
    .await
    .unwrap()
    .unwrap();
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK proof"]
async fn actual_writing_is_queued_once_then_uses_one_typed_invocation_and_the_existing_adoption_chain()
 {
    let db = fixture::db("ai_writing_actual").await;
    let endpoint = endpoint::Endpoint::start(false).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-writing", 4)
        .await
        .unwrap();
    let config = ready(&db, &endpoint, &owner).await;
    let work = fixture::create(&db, fixture::doc()).await;
    let request = preview(&work, config.task.model_config_ref.unwrap(), "");
    let confirmed = enqueue(&db, &request).await;
    ai::execute(&db, &confirmed).await.unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='writing_legacy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    run(&db, &owner).await;
    assert!(claim_next(&db, owner.fence()).await.unwrap().is_none());
    let result = ai::status(&db, confirmed.request_id).await.unwrap();
    assert_eq!(result["state"], "succeeded");
    assert_eq!(result["usage"]["measuredTokens"], 26);
    assert_eq!(
        result["suggestion"]["paragraphs"][0]["text"],
        "合成写作建议"
    );
    let output: Value = sqlx::query_scalar(
        "SELECT output FROM linggan_ai_batch_item_control WHERE invocation_ref=$1",
    )
    .bind(
        result["invocationRef"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap(),
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(output["requestRef"], json!(confirmed.request_id));
    assert!(
        output.get("paragraphs").is_none(),
        "normal outputs have one canonical suggestion"
    );
    let adopted = ai::adopt(
        &db,
        confirmed.request_id,
        &ai::AdoptRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            paragraph_indices: None,
            page_indices: None,
            adopt_title: false,
        },
    )
    .await
    .unwrap();
    assert!(adopted.to_string().contains("合成写作建议"));
    assert_eq!(endpoint.requests.lock().unwrap().len(), 2);
    endpoint.finish().await;
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK proof"]
async fn pause_and_unavailable_sources_never_allocate_a_writing_invocation_or_start_a_child() {
    let db = fixture::db("ai_writing_no_dispatch").await;
    let endpoint = endpoint::Endpoint::start(false).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-writing", 4)
        .await
        .unwrap();
    let config = ready(&db, &endpoint, &owner).await;
    let work = fixture::create(&db, fixture::doc()).await;
    let queued = enqueue(
        &db,
        &preview(&work, config.task.model_config_ref.unwrap(), ""),
    )
    .await;
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    change_control(
        &db,
        &ChangeControl {
            command_ref: Uuid::new_v4(),
            scope: ControlScope::Global,
            expected_revision: 1,
            enabled: false,
            reason: "Synthetic last-moment pause".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    let unavailable = PiAdapter::configured_with_test_command(
        PathBuf::from("/nonexistent-owned-node"),
        PathBuf::from("/unused"),
    );
    execute_writing(&db, &item, &SyntheticModelSecrets, &unavailable)
        .await
        .unwrap();
    assert_eq!(
        ai::status(&db, queued.request_id).await.unwrap()["state"],
        "queued"
    );
    ai::cancel(&db, queued.request_id).await.unwrap();
    assert_eq!(
        ai::status(&db, queued.request_id).await.unwrap()["state"],
        "cancelled"
    );
    setup::enable(&db, ControlScope::Global).await;
    let cited = body_and_comment_doc(&db).await;
    let body = cited.excerpts[0].citation.clone();
    let comment_work = cited.excerpts[1].citation.work_ref.unwrap();
    let frozen_body = frozen_source(&db, &body).await;
    let work = fixture::create(&db, cited).await;
    let queued = enqueue(
        &db,
        &preview(&work, config.task.model_config_ref.unwrap(), ""),
    )
    .await;
    observe_new_body(&db, &body).await;
    assert_eq!(frozen_source(&db, &body).await, frozen_body);
    ai::preview(
        &db,
        &preview(&work, config.task.model_config_ref.unwrap(), ""),
    )
    .await
    .unwrap();
    fixture::restrict_comment(&db, comment_work).await;
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    execute_writing(&db, &item, &SyntheticModelSecrets, &unavailable)
        .await
        .unwrap();
    let result = ai::status(&db, queued.request_id).await.unwrap();
    assert_eq!(result["state"], "failed");
    assert_eq!(result["failureCode"], "ai_source_unavailable");
    assert!(result["invocationRef"].is_null());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_model_invocation WHERE task_key='writing_legacy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    assert_eq!(endpoint.requests.lock().unwrap().len(), 1);
    endpoint.finish().await;
}
async fn body_and_comment_doc(db: &Database) -> WorkDocument {
    let mut body = fixture::cited_doc(db).await;
    let comment =
        fixture::comment_doc_for_work(db, body.excerpts[0].citation.work_ref.unwrap()).await;
    body.excerpts.extend(comment.excerpts);
    body.canvas.extend(comment.canvas);
    body
}
async fn frozen_source(db: &Database, citation: &Citation) -> Value {
    let mut tx = db.pool().begin().await.unwrap();
    let (source, _) = read_source_in(
        &mut tx,
        &citation.source_kind,
        citation.source_ref,
        citation.work_ref,
        &citation.field,
        &citation.block_ref,
        &citation.as_of,
    )
    .await
    .unwrap();
    assert_eq!(source.source_ref, citation.source_ref);
    assert_eq!(source.work_ref, citation.work_ref);
    assert_eq!(source.source_kind, citation.source_kind);
    assert_eq!(source.field, citation.field);
    assert_eq!(source.block_ref, citation.block_ref);
    assert_eq!(source.as_of, citation.as_of);
    assert_eq!(source.source_version, citation.source_version);
    let selected: String = source
        .text
        .chars()
        .skip(citation.start)
        .take(citation.end - citation.start)
        .collect();
    assert_eq!(text_hash(&selected), citation.text_hash);
    assert_eq!(
        source.text,
        format!("{}结尾不能丢失🙂", "完整正文。".repeat(900))
    );
    tx.commit().await.unwrap();
    serde_json::to_value(source).unwrap()
}
async fn observe_new_body(db: &Database, frozen: &Citation) {
    fixture::dossier::fixture::submit_package_at(db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成替代版本","bodyText":"新版原文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    let current = linggan_evidence::research_dossier::read(db, frozen.work_ref.unwrap())
        .await
        .unwrap()
        .unwrap();
    let body = current
        .fragments
        .iter()
        .find(|f| f.field == "body")
        .unwrap();
    assert_eq!(body.text, "新版原文");
    assert_ne!(body.fragment_id, frozen.block_ref);
    assert_ne!(body.source_version, frozen.source_version);
}
async fn inflight(name: &str, cancel: bool) {
    let db = fixture::db(name).await;
    let mut endpoint = endpoint::Endpoint::start(true).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-writing", 4)
        .await
        .unwrap();
    let config = ready(&db, &endpoint, &owner).await;
    let work = fixture::create(&db, fixture::doc()).await;
    let confirmed = enqueue(
        &db,
        &preview(&work, config.task.model_config_ref.unwrap(), ""),
    )
    .await;
    let item = claim_next(&db, owner.fence()).await.unwrap().unwrap();
    let worker = {
        let db = db.clone();
        let item = item.clone();
        tokio::spawn(async move {
            execute_writing(&db, &item, &SyntheticModelSecrets, &adapter()).await
        })
    };
    tokio::time::timeout(Duration::from_secs(15), endpoint.seen.take().unwrap())
        .await
        .unwrap()
        .unwrap();
    if cancel {
        ai::cancel(&db, confirmed.request_id).await.unwrap();
    } else {
        let mut tx = owner.fence().begin(&db, false).await.unwrap();
        sqlx::query("UPDATE linggan_ai_batch_item_control SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE item_ref=$1")
            .bind(item.item_ref).execute(&mut *tx).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(recover_expired(&db, owner.fence()).await.unwrap(), 1);
        tokio::time::sleep(Duration::from_secs(6)).await;
        assert!(
            !worker.is_finished(),
            "lease loss cannot discard a bounded in-flight usage receipt"
        );
    }
    endpoint.release.take().unwrap().send(()).unwrap();
    let outcome = worker.await.unwrap();
    let status = ai::status(&db, confirmed.request_id).await.unwrap();
    assert!(status["suggestion"].is_null());
    if cancel {
        outcome.unwrap();
        assert_eq!(status["state"], "cancelled");
        assert_eq!(status["usage"]["chargedTokens"], 26);
    } else {
        assert!(matches!(outcome, Err(ExecutorError::FenceLost)));
        assert_eq!(status["state"], "failed");
        assert_eq!(status["usage"]["state"], "unknown_reserved");
        let late:(Option<i64>,Option<i64>)=sqlx::query_as("SELECT input_tokens,output_tokens FROM linggan_ai_late_usage_receipt WHERE item_ref=$1").bind(item.item_ref).fetch_one(db.pool()).await.unwrap();
        assert_eq!(late, (Some(17), Some(9)));
    }
    let suggestions: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_creation_ai_suggestion")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(suggestions, 0);
    endpoint.finish().await;
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK proof"]
async fn actual_inflight_writing_cancel_keeps_usage_and_cannot_publish() {
    inflight("ai_writing_actual_cancel", true).await;
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK proof"]
async fn actual_late_writing_receipt_cannot_reopen_public_request_or_unknown_quota() {
    inflight("ai_writing_actual_late", false).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK proof"]
async fn actual_invalid_writing_output_keeps_measured_usage_without_automatic_interactive_braking()
{
    let db = fixture::db("ai_writing_actual_invalid").await;
    let endpoint = endpoint::Endpoint::start(false).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-writing", 4)
        .await
        .unwrap();
    let config = ready(&db, &endpoint, &owner).await;
    let work = fixture::create(&db, fixture::doc()).await;
    let confirmed = enqueue(
        &db,
        &preview(
            &work,
            config.task.model_config_ref.unwrap(),
            "invalid-output",
        ),
    )
    .await;
    run(&db, &owner).await;
    let result = ai::status(&db, confirmed.request_id).await.unwrap();
    assert_eq!(result["state"], "failed");
    assert_eq!(result["usage"]["chargedTokens"], 26);
    assert_eq!(result["failureCode"], "ai_output_invalid");
    assert_eq!(ai::readiness(&db).await.unwrap()["writingQualified"], false);
    let enabled: bool = sqlx::query_scalar(
        "SELECT enabled FROM linggan_ai_control WHERE scope_key='task:writing_legacy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(enabled);
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and actual owned loopback SDK/skill proof"]
async fn actual_skill_versions_compare_the_same_preview_then_activate_without_enqueuing_or_leaking_withdrawn_material()
 {
    let db = fixture::db("ai_writing_skill_actual").await;
    let endpoint = endpoint::Endpoint::start(false).await;
    let owner = ExecutorSession::acquire(&db, "synthetic-writing", 4)
        .await
        .unwrap();
    let config = ready(&db, &endpoint, &owner).await;
    let import = |expected, body: &str| ImportSkill {
        command_ref: Uuid::new_v4(),
        expected_version_ref: expected,
        package: SkillPackage {
            directory: "writing-method".into(),
            files: vec![UploadFile {
                path: "SKILL.md".into(),
                encoding: Encoding::Utf8,
                content: format!(
                    "---\nname: writing-method\ndescription: Synthetic fixed writing method\n---\n{body}\n"
                ),
            }],
        },
    };
    let first = import_skill(&db, &adapter(), &import(None, "baseline-v1"))
        .await
        .unwrap();
    let selection = ModelSelection {
        model_ref: config.model,
        capability_ref: config.capability,
        limits: config.limits.clone(),
    };
    let save = |version, revision| SaveTaskConfig {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::WritingLegacy,
        expected_revision: revision,
        model: Some(selection.clone()),
        skill_version_ref: Some(version),
        material_columns: Default::default(),
        budget: BudgetPolicy::Unlimited,
    };
    let baseline = save_task_config(&db, &save(first.version_ref, config.task.revision))
        .await
        .unwrap();
    let enabled = activate_skill(
        &db,
        &ActivateSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_task_revision: config.task.revision,
            expected_binding_revision: 0,
            candidate_config_ref: baseline.task_config_ref,
            trial_comparison_ref: None,
            first_version_exception: true,
            reason: "Synthetic first version exception".into(),
        },
    )
    .await
    .unwrap();
    let document = body_and_comment_doc(&db).await;
    let body = document.excerpts[0].citation.clone();
    let comment_work = document.excerpts[1].citation.work_ref.unwrap();
    let frozen_body = frozen_source(&db, &body).await;
    let work = fixture::create(&db, document).await;
    let input = preview(&work, config.task.model_config_ref.unwrap(), "");
    ai::preview(&db, &input).await.unwrap();
    let second = import_skill(
        &db,
        &adapter(),
        &import(Some(first.version_ref), "candidate-v2"),
    )
    .await
    .unwrap();
    let candidate = save_task_config(&db, &save(second.version_ref, enabled.task_revision))
        .await
        .unwrap();
    let activate = ActivateSkill {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::WritingLegacy,
        expected_task_revision: enabled.task_revision,
        expected_binding_revision: enabled.binding_revision,
        candidate_config_ref: candidate.task_config_ref,
        trial_comparison_ref: None,
        first_version_exception: false,
        reason: "Synthetic paired trial approval".into(),
    };
    assert!(activate_skill(&db, &activate).await.is_err());
    let trial = create_trial(
        &db,
        &CreateTrial {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_task_revision: enabled.task_revision,
            baseline_config_ref: Some(enabled.task_config_ref),
            candidate_config_ref: candidate.task_config_ref,
            items: vec![BatchInput {
                item_key: "same-preview".into(),
                input: InputRef::WritingPreview {
                    preview_ref: input.preview_ref,
                },
            }],
        },
    )
    .await
    .unwrap();
    run(&db, &owner).await;
    run(&db, &owner).await;
    let compared = read_comparison(&db, trial.comparison_ref, 0).await.unwrap();
    assert_eq!(compared["qualifiesForActivation"], true);
    assert_eq!(compared["sourceQualification"], "readable");
    let texts = [
        compared["baseline"]["output"]["paragraphs"][0]["text"]
            .as_str()
            .unwrap(),
        compared["candidate"]["output"]["paragraphs"][0]["text"]
            .as_str()
            .unwrap(),
    ];
    assert_eq!(texts, ["合成基线建议", "合成候选建议"]);
    assert_eq!(
        compared["baseline"]["skillVersionRef"],
        json!(first.version_ref)
    );
    assert_eq!(
        compared["candidate"]["skillVersionRef"],
        json!(second.version_ref)
    );
    let requests: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creation_ai_request")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(requests, 0);
    let approve = ActivateSkill {
        command_ref: Uuid::new_v4(),
        trial_comparison_ref: Some(trial.comparison_ref),
        ..activate
    };
    activate_skill(&db, &approve).await.unwrap();
    assert!(claim_next(&db, owner.fence()).await.unwrap().is_none());
    assert_eq!(endpoint.requests.lock().unwrap().len(), 3);
    observe_new_body(&db, &body).await;
    assert_eq!(frozen_source(&db, &body).await, frozen_body);
    let after_observation = read_comparison(&db, trial.comparison_ref, 0).await.unwrap();
    assert_eq!(after_observation["sourceQualification"], "readable");
    assert_eq!(
        after_observation["baseline"]["output"],
        compared["baseline"]["output"]
    );
    assert_eq!(
        after_observation["candidate"]["output"],
        compared["candidate"]["output"]
    );
    fixture::restrict_comment(&db, comment_work).await;
    let redacted = read_comparison(&db, trial.comparison_ref, 0).await.unwrap();
    assert_eq!(redacted["sourceQualification"], "unavailable");
    assert!(redacted["baseline"]["output"].is_null());
    assert!(redacted["candidate"]["output"].is_null());
    assert_eq!(endpoint.requests.lock().unwrap().len(), 3);
    endpoint.finish().await;
}
