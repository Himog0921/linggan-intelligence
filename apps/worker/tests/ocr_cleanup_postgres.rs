//! Destructive development-data migration, on synthetic isolated schemas only.
#[path = "../../api/src/local_web/full_schema_fixture.rs"]
mod schema;
#[path = "../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod support;
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;
const PURGE: &str = include_str!("../../../database/migrations/0168_dossier_ocr_cleanup.sql");
async fn baseline(name: &str) -> Database {
    let source = schema::FULL_MIGRATIONS.replacen(PURGE, "", 1);
    let source = source
        .lines()
        .filter(|line| !line.contains("VALUES ('0168_dossier_ocr_cleanup'"))
        .collect::<Vec<_>>()
        .join("\n");
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    isolated_proof_schema(
        &url,
        &format!("{name}_{}", Uuid::new_v4().simple()),
        &source,
    )
    .await
    .unwrap()
}
async fn native_human(db: &Database, work: Uuid) -> Uuid {
    let row=sqlx::query("SELECT u.unit_ref,u.source_version,u.revision,r.reading_ref FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref WHERE u.content_public_ref=$1 AND u.kind='body'").bind(work).fetch_one(db.pool()).await.unwrap();
    let unit: Uuid = row.get("unit_ref");
    let base: Uuid = row.get("reading_ref");
    let human = Uuid::new_v4();
    let command = Uuid::new_v4();
    let source: String = row.get("source_version");
    let revision: i64 = row.get("revision");
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_reading(reading_ref,unit_ref,content_public_ref,source_version,source_state,producer_ref,producer_kind,rule_version,blocks,issues,source_location,based_on_reading_ref,publication) VALUES($1,$2,$3,$4,'present',$5,'human_edit','synthetic-human.v1',$6,'[]','{}',$7,'human_edit')").bind(human).bind(unit).bind(work).bind(&source).bind(command).bind(json!([{"ref":format!("{work}.{human}.0"),"kind":"paragraph","speaker":"author","text":"Keep this genuine human transcription."}])).bind(base).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_reading_command(command_ref,request_hash,unit_ref,candidate_reading_ref,expected_reading_ref,resulting_reading_ref,action,actor,reason,verified,response) VALUES($1,$2,$3,$4,$4,$5,'edit','synthetic-editor','source comparison',true,'{}')").bind(command).bind("d".repeat(64)).bind(unit).bind(base).bind(human).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_reading_selection(selection_ref,unit_ref,revision,event,source_version,source_state,reading_ref,adoption,verification,command_ref) VALUES($1,$2,$3,'edit',$4,'present',$5,'human_edited','verified',$6)").bind(Uuid::new_v4()).bind(unit).bind(revision+1).bind(source).bind(human).bind(command).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_material_unit SET current_reading_ref=$2,revision=revision+1,adoption='human_edited',verification='verified' WHERE unit_ref=$1").bind(unit).bind(human).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    human
}
async fn old_rows(db: &Database, work: Uuid) -> (Uuid, Uuid) {
    let row=sqlx::query("SELECT j.slot_key,j.blob_sha256 FROM linggan_media_processing_job j WHERE j.processor_kind='image_ocr' LIMIT 1").fetch_one(db.pool()).await.unwrap();
    let job = Uuid::new_v4();
    let derivative = Uuid::new_v4();
    let layout = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'image_ocr','local-v2','full_blob')").bind(job).bind(row.get::<String,_>("blob_sha256")).bind(row.get::<String,_>("slot_key")).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) VALUES($1,$2,'succeeded','synthetic_old_result')").bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'ocr_text',$3,21,'derivatives/ocr/synthetic.txt')").bind(derivative).bind(job).bind("a".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,source_location) VALUES($1,$2,'ocr_text','SYNTHETIC-OLD-OCR','SYNTHETIC-OLD-OCR','{}')").bind(derivative).bind(work).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','synthetic-old',2,2,$5,30,'derivatives/ocr-layout/synthetic.json')").bind(layout).bind(derivative).bind(work).bind(row.get::<String,_>("blob_sha256")).bind("b".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_line(layout_ref,line_ref,ordinal,text_content,confidence,left_norm,top_norm,right_norm,bottom_norm) VALUES($1,$2,0,'SYNTHETIC-OLD-OCR',0.9,0,0,1,1)").bind(layout).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) VALUES($1,'tesseract_replaced_by_paddleocr')").bind(job).execute(db.pool()).await.unwrap();
    // A non-OCR row in this historically named shared table must survive the bounded purge.
    let asr = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'asr','local-v3','synthetic-preserved-scope')").bind(asr).bind(row.get::<String,_>("blob_sha256")).bind(row.get::<String,_>("slot_key")).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) VALUES($1,'tesseract_replaced_by_paddleocr')").bind(asr).execute(db.pool()).await.unwrap();
    let transcript = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'asr_text',$3,19,'derivatives/asr/synthetic.txt')").bind(transcript).bind(asr).bind("c".repeat(64)).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,source_location) VALUES($1,$2,'asr_text','Preserve ASR text.','Preserve ASR text.','{}')").bind(transcript).bind(work).execute(db.pool()).await.unwrap();
    (asr, transcript)
}
async fn counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_object('originals',(SELECT count(*) FROM linggan_media_materialization),'details',(SELECT count(*) FROM linggan_material_content_detail),'comments',(SELECT count(*) FROM linggan_material_comment),'thumbnail',(SELECT count(*) FROM linggan_media_processing_job WHERE processor_kind='thumbnail'),'models',(SELECT count(*) FROM linggan_model_config))").fetch_one(db.pool()).await.unwrap()
}
#[tokio::test]
#[ignore = "owned disposable PostgreSQL only; no provider calls"]
async fn historical_machine_results_are_deleted_while_native_human_asr_and_current_capabilities_survive()
 {
    let db = baseline("purge_positive").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    support::model_config(&db).await;
    let human = native_human(&db, work).await;
    let (asr, transcript) = old_rows(&db, work).await;
    let before = counts(&db).await;
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::raw_sql(PURGE).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(counts(&db).await, before);
    let remaining:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_media_processing_job WHERE processor_kind IN('image_ocr','video_frame_ocr')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(remaining, 0);
    let machine:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading WHERE producer_kind IN('ordinary_ocr','local_vision','cloud_vision','frame_ocr')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(machine, 0);
    let kept_human: String = sqlx::query_scalar(
        "SELECT blocks->0->>'text' FROM linggan_material_reading WHERE reading_ref=$1",
    )
    .bind(human)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(kept_human, "Keep this genuine human transcription.");
    let kept_asr: String = sqlx::query_scalar(
        "SELECT text_content FROM linggan_material_derived_text WHERE derivative_ref=$1",
    )
    .bind(transcript)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(kept_asr, "Preserve ASR text.");
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM linggan_media_ocr_retirement WHERE retired_job_ref=$1)"
        )
        .bind(asr)
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
    assert!(sqlx::query_scalar::<_,bool>("SELECT to_regclass('linggan_media_ocr_adoption') IS NOT NULL AND to_regclass('linggan_media_ocr_layout') IS NOT NULL AND to_regclass('linggan_ocr_audit_run') IS NULL").fetch_one(db.pool()).await.unwrap());
    let disabled:i64=sqlx::query_scalar("SELECT count(*) FROM pg_trigger WHERE tgrelid IN('linggan_material_reading'::regclass,'linggan_media_derivative'::regclass,'linggan_ai_command'::regclass) AND tgenabled='D'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(disabled, 0);
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(invocations, 0, "purge never dispatches a model");
    assert!(
        sqlx::query("DELETE FROM linggan_material_reading WHERE reading_ref=$1")
            .bind(human)
            .execute(db.pool())
            .await
            .is_err(),
        "append-only guard restored"
    );
}
#[tokio::test]
#[ignore = "owned disposable PostgreSQL only; no provider calls"]
async fn unexpected_human_dependency_aborts_atomically_without_deleting_transcription() {
    let db = baseline("purge_negative").await;
    let work = support::work(&db).await;
    support::current_image_reading(&db).await;
    let row=sqlx::query("SELECT u.*,r.reading_ref FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref WHERE u.content_public_ref=$1 AND u.kind='image'").bind(work).fetch_one(db.pool()).await.unwrap();
    let human = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_material_reading(reading_ref,unit_ref,content_public_ref,source_version,source_state,producer_kind,rule_version,blocks,issues,source_location,based_on_reading_ref,publication) VALUES($1,$2,$3,$4,'present','human_edit','synthetic-human.v1',$5,'[]','{}',$6,'human_edit')").bind(human).bind(row.get::<Uuid,_>("unit_ref")).bind(work).bind(row.get::<String,_>("source_version")).bind(json!([{"ref":"synthetic-human","kind":"paragraph","text":"Never delete human text.","speaker":"author"}])).bind(row.get::<Uuid,_>("reading_ref")).execute(db.pool()).await.unwrap();
    let before = counts(&db).await;
    let mut tx = db.pool().begin().await.unwrap();
    let error = sqlx::raw_sql(PURGE).execute(&mut *tx).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("human_reading_depends_on_machine_requires_explicit_detach")
    );
    tx.rollback().await.unwrap();
    assert_eq!(counts(&db).await, before);
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM linggan_material_reading WHERE reading_ref=$1)"
        )
        .bind(human)
        .fetch_one(db.pool())
        .await
        .unwrap()
    );
}
