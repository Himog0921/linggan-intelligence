//! Native PostgreSQL source/save/reopen proofs; only synthetic local model calls.
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
    topic_map::{self, TopicMapCommand},
    topic_map_research::apply_research_command,
    topic_map_research_worker::run_once,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn tail_fixture_config(db: &Database, original: Uuid) -> Uuid {
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    // Model entries/configs are immutable. Prepare a separate synthetic model
    // and its callable receipt; keep the original model and every limit intact.
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) SELECT $1,m.connection_version_ref,'synthetic-topic-map-cite-tail','manual' FROM linggan_model_entry m JOIN linggan_model_config c USING(model_ref) WHERE c.config_ref=$2")
        .bind(model).bind(original).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,$2,input_token_limit,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$3")
        .bind(config).bind(model).bind(original).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result) SELECT $1,connection_version_ref,model_ref,'probe','synthetic-cite-tail','succeeded',0,0,$3 FROM linggan_model_entry WHERE model_ref=$2")
        .bind(Uuid::new_v4()).bind(model).bind(json!({"ok":true,"modelCallable":true,"semanticQualified":true})).execute(db.pool()).await.unwrap();
    config
}

async fn source_task(
    db: &Database,
    work: Uuid,
    config: Uuid,
    field: &str,
    source: Option<Uuid>,
    start: usize,
) -> Uuid {
    sqlx::query_scalar("SELECT t.task_ref FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run run USING(run_ref) WHERE t.work_public_ref=$1 AND run.config_ref=$2 AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.input_refs->'fragments') f WHERE f->>'field'=$3 AND ($4::uuid IS NULL OR f->>'sourceRef'=$4::text) AND (f->>'start')::bigint>=$5) ORDER BY t.created_at,t.task_ref LIMIT 1")
        .bind(work).bind(config).bind(field).bind(source).bind(start as i64).fetch_one(db.pool()).await.unwrap()
}

async fn finish_source_task(db: &Database, adapter: &PiAdapter, task: Uuid) {
    // Prioritize just this requested proof window. Other queued sources remain
    // untouched, so a 257-media regression need not invoke 257 synthetic analyses.
    sqlx::query("UPDATE linggan_topic_map_research_task SET created_at=scope_001_now()-interval '1 hour' WHERE task_ref=$1")
        .bind(task).execute(db.pool()).await.unwrap();
    for _ in 0..8 {
        let row = sqlx::query(
            "SELECT state,phase,last_reason,attempt_count FROM linggan_topic_map_research_task WHERE task_ref=$1",
        )
        .bind(task)
        .fetch_one(db.pool())
        .await
        .unwrap();
        let state: String = row.get("state");
        if state == "succeeded" {
            return;
        }
        assert!(
            !["failed", "unknown_dispatch", "stopped", "stale"].contains(&state.as_str()),
            "source task {task} became {state}; phase={}, reason={:?}, attempts={}",
            row.get::<String, _>("phase"),
            row.get::<Option<String>, _>("last_reason"),
            row.get::<i32, _>("attempt_count")
        );
        assert!(run_once(db, &SyntheticModelSecrets, adapter).await.unwrap());
    }
    panic!("selected synthetic window did not settle");
}

async fn task_angle_command(db: &Database, task: Uuid, work: Uuid, key: &str) -> TopicMapCommand {
    let row = sqlx::query("SELECT result.result_ref,result.method_version,result.core_json FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request request USING(invocation_ref) WHERE request.task_ref=$1 ORDER BY result.created_at DESC LIMIT 1")
        .bind(task).fetch_one(db.pool()).await.unwrap();
    let core: Value = row.get("core_json");
    let assignment = &core["units"][0]["assignments"][0];
    TopicMapCommand::SaveAlternative {
        idempotency_key: key.into(),
        domain_ref: D,
        topic_ref: assignment["topicRef"].as_str().unwrap().parse().unwrap(),
        definition_ref: assignment["definitionRef"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        title: "合成完整来源角度".into(),
        angle: "引用已有合格原文的合成测试".into(),
        rationale: "SYNTHETIC / NOT EVIDENCE".into(),
        evidence_work_refs: vec![work],
        method_version: row.get("method_version"),
        research_result_ref: Some(row.get("result_ref")),
        research_angle_index: Some(0),
        research_opportunity_index: None,
    }
}

async fn save_task_angle(db: &Database, task: Uuid, work: Uuid, key: &str) -> Uuid {
    topic_map::save_topic_map_command(db, &task_angle_command(db, task, work, key).await)
        .await
        .unwrap()
        .subject_ref
        .unwrap()
}

async fn assert_tail_angle_citation(
    db: &Database,
    task: Uuid,
    field: &str,
    source: &str,
    emoji: &str,
) {
    let row = sqlx::query("SELECT result.output_json,task.input_refs FROM linggan_topic_map_research_result result JOIN linggan_topic_map_research_request request USING(invocation_ref) JOIN linggan_topic_map_research_task task USING(task_ref) WHERE request.task_ref=$1 ORDER BY result.created_at DESC LIMIT 1")
        .bind(task).fetch_one(db.pool()).await.unwrap();
    let output: Value = row.get("output_json");
    let manifest: Value = row.get("input_refs");
    let input = manifest.get("source").unwrap_or(&manifest);
    let tail = input["fragments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|fragment| fragment["field"] == field && fragment["start"] == 12000)
        .expect("requested tail fragment must be frozen in the accepted input");
    let end = source.chars().count();
    assert_eq!(tail["end"], json!(end));
    let citations = output["angles"][0]["evidence"].as_array().unwrap();
    assert_eq!(citations.len(), 1, "fixture must explicitly cite its tail");
    assert_eq!(citations[0]["fragmentId"], tail["fragmentId"]);
    assert_eq!(citations[0]["start"], json!(12000));
    assert_eq!(citations[0]["end"], json!(end));
    let emoji_byte = source.find(emoji).expect("fixture tail contains its emoji");
    let emoji_start = source[..emoji_byte].chars().count();
    let emoji_end = emoji_start + emoji.chars().count();
    assert!(12000 <= emoji_start && emoji_end <= end);
    let quoted: String = source.chars().skip(12000).take(end - 12000).collect();
    assert!(
        quoted.contains(emoji),
        "actual cited scalar range covers the emoji"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn unicode_tail_survives_save_and_same_text_refresh_across_model_configs() {
    let (db, config, adapter) = setup("topic_saved_unicode_tail").await;
    let config = tail_fixture_config(&db, config).await;
    let body = format!(
        "{}尾部👩‍👧这里才说明开始练习遇到的限制。",
        "前文练习。".repeat(2400)
    );
    let work = work(&db, "saved-tail", &body).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let task = source_task(&db, work, config, "body", None, 12000).await;
    finish_source_task(&db, &adapter, task).await;
    assert_tail_angle_citation(&db, task, "body", &body, "👩‍👧").await;
    let saved = save_task_angle(&db, task, work, "saved-unicode-tail").await;
    let original = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(original.source_state, "source_unavailable");
    let tail = original
        .fragments
        .iter()
        .find(|f| f.start >= 12000)
        .unwrap();
    assert!(tail.text.contains("尾部👩‍👧"));
    assert!(tail.end > 10000 && !tail.cited_ranges.is_empty());
    assert!(
        tail.cited_ranges
            .iter()
            .all(|range| tail.start <= range.start && range.end <= tail.end)
    );
    let source = tail.source_ref;
    work_at(&db, "saved-tail", "", &body, 999, "2026-09-30T10:00:00Z").await;
    let refreshed = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(refreshed.source_state, "source_unavailable");
    assert_eq!(
        refreshed
            .fragments
            .iter()
            .find(|f| f.start >= 12000)
            .unwrap()
            .source_ref,
        source
    );
    let second_config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$2")
        .bind(second_config).bind(config).execute(db.pool()).await.unwrap();
    apply_research_command(&db, &configure(second_config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let next = source_task(&db, work, second_config, "body", None, 12000).await;
    finish_source_task(&db, &adapter, next).await;
    assert_tail_angle_citation(&db, next, "body", &body, "👩‍👧").await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_discussion_unit WHERE work_public_ref=$1"
        )
        .bind(work)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        1,
        "same logical quotation must not become a second discussion after recapture/config change"
    );
}

async fn reference_only_work(db: &Database, id: &str) -> Uuid {
    let package = fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},
            "payload":{"title":"","bodyText":"","authorId":"synthetic-creator"}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'reference','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(D).bind(package).execute(db.pool()).await.unwrap();
    work
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn thirty_first_comment_and_real_creator_parent_reopen_then_restrict_together() {
    let (db, config, adapter) = setup("topic_saved_comment_tail").await;
    let work = reference_only_work(&db, "saved-comment-tail").await;
    comments::comment_with_author(
        &db,
        "saved-comment-tail",
        "creator-parent",
        "你可以先试着从一次很短的练习开始。",
        Some("synthetic-creator"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let child = comments::reply_with_author(
        &db,
        "saved-comment-tail",
        "old-eligible-reply",
        "creator-parent",
        "我已经这样试过，但开始练习后仍然需要提醒。",
        Some("synthetic-reader"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    for index in 0..35 {
        comments::comment_with_author(
            &db,
            "saved-comment-tail",
            &format!("later-{index:03}"),
            &format!("合成第{index}条独立读者反馈，练习需要明确步骤。"),
            Some("another-reader"),
            "2026-09-17T08:00:00Z",
        )
        .await;
    }
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_research_task WHERE work_public_ref=$1"
        )
        .bind(work)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        36,
        "no 30-comment cut and no independent creator-parent task"
    );
    let task = source_task(&db, work, config, "unresearched_comment", Some(child), 0).await;
    finish_source_task(&db, &adapter, task).await;
    let saved = save_task_angle(&db, task, work, "saved-comment-tail-angle").await;
    let original = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(original.source_state, "source_unavailable");
    assert!(
        original
            .fragments
            .iter()
            .any(|f| f.source_ref == child && !f.context_only && !f.cited_ranges.is_empty())
    );
    assert!(
        original
            .fragments
            .iter()
            .any(|f| f.field == "parent_comment_context"
                && f.context_only
                && f.text.contains("很短的练习"))
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_work_annotation WHERE work_public_ref=$1"
        )
        .bind(work)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "comment-only work has no author journey"
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'creator-parent','synthetic parent withdrawal')")
        .bind(work).execute(db.pool()).await.unwrap();
    let hidden = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_eq!(hidden.source_state, "source_unavailable");
    assert!(
        hidden.fragments.is_empty()
            && hidden.angle.is_none()
            && hidden.original_definition.is_none()
    );
}

async fn media_slot(db: &Database, id: &str) -> (String, String) {
    let slot = format!("xhs:{id}:cover:1");
    let observation = Uuid::new_v4();
    fixture::submit_package(db,"media_slots",json!({"contentExternalId":id}),json!({
        "kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"cover","ordinal":1},
        "observation":{"externalUri":"https://media.example/synthetic.jpg","candidateUris":["https://media.example/synthetic.jpg"],"observedAt":"2026-09-20T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":id}
    })).await;
    let blob = "f".repeat(64);
    linggan_evidence::admit_media_blob(
        db,
        observation,
        &blob,
        "image/jpeg",
        4096,
        "blobs/synthetic/saved-tail",
    )
    .await
    .unwrap();
    // Every pending row consumes one preview position; complete topic pagination
    // must still reach the acquired sources added after this first full page.
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) SELECT gen_random_uuid(),$1,$2,'asr','saved-preview-padding-'||n::text,'full' FROM generate_series(1,260)n")
        .bind(&blob).bind(&slot).execute(db.pool()).await.unwrap();
    (slot, blob)
}

async fn media_job(db: &Database, slot: &str, blob: &str, kind: &str, version: &str) -> Uuid {
    let job = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,$4,$5,'saved-tail')")
        .bind(job).bind(blob).bind(slot).bind(kind).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,occurred_at) VALUES($1,$2,'succeeded',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    job
}

async fn media_text(
    db: &Database,
    work: Uuid,
    job: Uuid,
    field: &str,
    text: &str,
    position: i32,
) -> Uuid {
    let derivative = Uuid::new_v4();
    let kind = if field == "ocr" {
        "ocr_text"
    } else {
        "asr_text"
    };
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(derivative).bind(job).bind(kind).bind(linggan_evidence::creator_discovery::hash(text))
        .bind(text.len() as i64).bind(format!("derivatives/saved/{derivative}.txt")).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,source_location) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(derivative).bind(work).bind(kind).bind(text).bind(text.chars().take(80).collect::<String>())
        .bind(json!({"segment":position,"processorVersion":"synthetic-media-v1"})).execute(db.pool()).await.unwrap();
    derivative
}

async fn accepted_layout(db: &Database, work: Uuid, derivative: Uuid, blob: &str, text: &str) {
    let layout = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_ocr_layout(layout_ref,ocr_derivative_ref,content_public_ref,blob_sha256,engine,engine_version,image_width,image_height,layout_content_hash,layout_byte_size,layout_storage_key) VALUES($1,$2,$3,$4,'paddleocr','synthetic-saved',1080,1440,$5,20,$6)")
        .bind(layout).bind(derivative).bind(work).bind(blob).bind(linggan_evidence::creator_discovery::hash(text))
        .bind(format!("derivatives/saved/{layout}.json")).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_ocr_layering_result(layering_ref,layout_ref,layer_version,state,decision_source,image_substantive_text,retained_line_refs,excluded_lines) VALUES($1,$2,'synthetic-v1','ACCEPTED','rules',$3,'[]','[]')")
        .bind(Uuid::new_v4()).bind(layout).bind(text).execute(db.pool()).await.unwrap();
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn later_media_pages_and_full_asr_ocr_reopen_without_authorizing_restricted_siblings() {
    let (db, config, adapter) = setup("topic_saved_media_tail").await;
    let config = tail_fixture_config(&db, config).await;
    let work = work(&db, "saved-media-tail", "").await;
    let (slot, blob) = media_slot(&db, "saved-media-tail").await;
    let asr = media_job(&db, &slot, &blob, "asr", "synthetic-full-asr").await;
    let transcript = format!(
        "{}尾段🙂开始练习还需要明确下一步。",
        "转录正文。".repeat(2400)
    );
    media_text(&db, work, asr, "transcript", &transcript, 1).await;
    let ocr = media_job(&db, &slot, &blob, "image_ocr", "synthetic-full-ocr").await;
    let allowed = media_text(&db, work, ocr, "ocr", "同一个job中另一张图的原始预览。", 1).await;
    accepted_layout(&db, work, allowed, &blob, "另一个合格位置的独立正文。").await;
    let restricted = media_text(
        &db,
        work,
        ocr,
        "ocr",
        "待验证的原始OCR预览，不可替代正文。",
        2,
    )
    .await;
    let ocr_text = format!(
        "{}合格OCR尾部👩‍👧仍然属于这一张图的开始练习证据。",
        "分层正文。".repeat(2400)
    );
    accepted_layout(&db, work, restricted, &blob, &ocr_text).await;
    let preview = linggan_evidence::read_work_resource(&db, work)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(preview.inspector["derivativesReceipt"]["truncated"], true);
    assert!(
        !preview.inspector["derivatives"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["jobRef"] == json!(asr))
    );
    let full = linggan_evidence::creator_discovery::load_topic_materials(&db, D)
        .await
        .unwrap();
    let material = full.works.iter().find(|w| w.work_ref == work).unwrap();
    assert!(
        material
            .fragments
            .iter()
            .any(|f| f.field == "transcript" && f.text == transcript)
    );
    assert!(
        material
            .fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text == ocr_text)
    );
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![work]))
        .await
        .unwrap();
    let asr_task = source_task(&db, work, config, "transcript", Some(asr), 12000).await;
    finish_source_task(&db, &adapter, asr_task).await;
    assert_tail_angle_citation(&db, asr_task, "transcript", &transcript, "🙂").await;
    let asr_saved = save_task_angle(&db, asr_task, work, "saved-full-asr-tail").await;
    let reopened = topic_map::read_saved_alternative(&db, D, asr_saved)
        .await
        .unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    assert!(reopened.fragments.iter().any(|f| f.field == "transcript"
        && f.text.contains("尾段🙂")
        && !f.cited_ranges.is_empty()));
    let ocr_task = source_task(&db, work, config, "ocr", Some(ocr), 12000).await;
    finish_source_task(&db, &adapter, ocr_task).await;
    assert_tail_angle_citation(&db, ocr_task, "ocr", &ocr_text, "👩‍👧").await;
    let ocr_saved = save_task_angle(&db, ocr_task, work, "saved-full-ocr-tail").await;
    let reopened = topic_map::read_saved_alternative(&db, D, ocr_saved)
        .await
        .unwrap();
    assert!(
        reopened
            .fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text.contains("合格OCR尾部"))
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','restricted sibling',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(restricted).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let texts = linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, ocr, "ocr")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(texts.iter().any(|t| t == "另一个合格位置的独立正文。"));
    assert!(
        !texts.contains(&ocr_text),
        "an allowed sibling cannot authorize the restricted derivative's historical layer"
    );
    let hidden = topic_map::read_saved_alternative(&db, D, ocr_saved)
        .await
        .unwrap();
    assert_eq!(hidden.source_state, "source_unavailable");
    assert!(
        hidden.fragments.is_empty()
            && hidden.title.is_none()
            && hidden.original_definition.is_none()
    );
}

async fn assert_definition_withdrawal_hides_saved(indirect: bool) {
    let name = if indirect {
        "topic_saved_neighbor_withdrawal"
    } else {
        "topic_saved_definition_withdrawal"
    };
    let (db, config, adapter) = setup(name).await;
    let first = work(&db, "private-definition-source", "").await;
    let comment = comments::comment_with_author(
        &db,
        "private-definition-source",
        "private-rule",
        "合成私有来源：开始练习前需要支持。",
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    apply_research_command(&db, &start(vec![first]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let original_definition: Uuid = sqlx::query_scalar(
        "SELECT definition_ref FROM linggan_topic_map_concept_rule ORDER BY created_at LIMIT 1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let second = work(
        &db,
        "still-qualified-angle",
        if indirect {
            "SUSTAIN_ONLY 开始之后维持练习的注意力。"
        } else {
            "合成另一个合格作者：开始练习可以拆成第一步。"
        },
    )
    .await;
    apply_research_command(&db, &start(vec![second]))
        .await
        .unwrap();
    finish_pending(&db, &adapter).await;
    let task = source_task(&db, second, config, "body", None, 0).await;
    let command = task_angle_command(&db, task, second, "saved-before-definition-withdrawal").await;
    let TopicMapCommand::SaveAlternative { definition_ref, .. } = &command else {
        unreachable!()
    };
    assert_eq!(
        *definition_ref == original_definition,
        !indirect,
        "the second rule must actually depend on the first by assignment or compared candidate"
    );
    let saved = topic_map::save_topic_map_command(&db, &command)
        .await
        .unwrap()
        .subject_ref
        .unwrap();
    assert!(
        topic_map::read_saved_alternative(&db, D, saved)
            .await
            .unwrap()
            .original_definition
            .is_some()
    );
    if indirect {
        let compared: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_topic_map_research_request q WHERE q.task_ref=$1 AND q.phase='resolve' AND q.request_manifest::text LIKE '%'||$2::text||'%')")
            .bind(task).bind(original_definition).fetch_one(db.pool()).await.unwrap();
        assert!(
            compared,
            "the saved second definition must have compared the now-withdrawn neighbor"
        );
    }
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason)SELECT content_public_ref,comment_external_id,'synthetic rule source withdrawal' FROM linggan_material_comment WHERE material_ref=$1")
        .bind(comment).execute(db.pool()).await.unwrap();
    let qualified = linggan_evidence::creator_discovery::load_topic_materials(&db, D)
        .await
        .unwrap();
    assert!(
        qualified
            .works
            .iter()
            .find(|w| w.work_ref == second)
            .is_some_and(|w| !w.fragments.is_empty()),
        "the angle's own source remains qualified; only its definition dependency was withdrawn"
    );
    let hidden = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_eq!(hidden.source_state, "source_unavailable");
    assert!(
        hidden.title.is_none()
            && hidden.angle.is_none()
            && hidden.rationale.is_none()
            && hidden.original_definition.is_none()
            && hidden.fragments.is_empty()
    );
    let snapshot = topic_map::read_topic_map(
        &db,
        &topic_map::TopicMapQuery {
            domain_ref: Some(D),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let listed = snapshot
        .alternatives
        .iter()
        .find(|a| a["alternativeRef"] == json!(saved))
        .unwrap();
    assert_eq!(listed["sourceState"], "source_unavailable");
    assert!(
        listed["title"].is_null() && listed["angle"].is_null() && listed["rationale"].is_null()
    );
    let retry = task_angle_command(&db, task, second, "saved-after-definition-withdrawal").await;
    assert!(
        topic_map::save_topic_map_command(&db, &retry)
            .await
            .is_err(),
        "new save cannot admit a withdrawn definition or comparison dependency"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn withdrawn_machine_definition_hides_qualified_saved_angle_and_rejects_new_save() {
    assert_definition_withdrawal_hides_saved(false).await;
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn withdrawn_unassigned_neighbor_hides_dependent_machine_definition_and_saved_angle() {
    assert_definition_withdrawal_hides_saved(true).await;
}
