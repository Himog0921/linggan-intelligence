//! Handwritten grouped-source proofs in isolated PostgreSQL; synthetic adapter only.
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
use core_fixture::*;
use linggan_intelligence::{
    comment_study_source,
    topic_map::{self, TopicMapCommand},
    topic_map_research::apply_research_command,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn start_run(db: &Database, work: Uuid) -> Uuid {
    apply_research_command(db, &start(vec![work]))
        .await
        .unwrap()["runRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}

async fn source_manifests(db: &Database, run: Uuid) -> Vec<Value> {
    sqlx::query_scalar("SELECT input_refs FROM linggan_topic_map_research_task WHERE run_ref=$1 AND NOT(recall_manifest ? 'backfill') AND COALESCE(input_refs#>>'{coverage,kind}','source')<>'comparison' ORDER BY created_at,task_ref")
        .bind(run).fetch_all(db.pool()).await.unwrap()
}

fn comment_sources(manifests: &[Value]) -> Vec<Uuid> {
    manifests
        .iter()
        .flat_map(|m| m["fragments"].as_array().into_iter().flatten())
        .filter(|f| {
            matches!(
                f["field"].as_str(),
                Some("studied_comment" | "unresearched_comment")
            )
        })
        .map(|f| f["sourceRef"].as_str().unwrap().parse().unwrap())
        .collect()
}

async fn add_comment(db: &Database, note: &str, id: &str, body: &str) -> Uuid {
    comments::comment_with_author(
        db,
        note,
        id,
        body,
        Some("synthetic-reader"),
        "2026-09-16T08:00:00Z",
    )
    .await
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn grouped_admission_reuses_old_voices_but_reassesses_changed_required_author_context() {
    let (db, config, adapter) = setup("topic_core_grouped_admission").await;
    let body = "SYNTHETIC 作者建议先从一次短练习开始。";
    let w = work_with_title(&db, "grouped-admission", "合成练习启动", body).await;
    let mut old = Vec::new();
    for index in 0..7 {
        old.push(
            add_comment(
                &db,
                "grouped-admission",
                &format!("voice-{index}"),
                &format!("合成用户{index}：我开始练习前仍需要提示。"),
            )
            .await,
        );
    }
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let first = start_run(&db, w).await;
    let manifests = source_manifests(&db, first).await;
    assert_eq!(
        manifests.len(),
        3,
        "one title/body group and two bounded comment groups"
    );
    let mut sources = comment_sources(&manifests);
    sources.sort();
    old.sort();
    assert_eq!(
        sources, old,
        "all seven distinct qualified voices are retained"
    );
    finish_pending(&db, &adapter).await;
    assert_finished_sources(&db, first).await;
    let fresh = add_comment(
        &db,
        "grouped-admission",
        "new-voice",
        "合成新增用户：开始练习需要明确第一步。",
    )
    .await;
    let next = start_run(&db, w).await;
    let manifests = source_manifests(&db, next).await;
    assert_eq!(
        manifests.len(),
        1,
        "one new voice does not replay the finished author or comment batches"
    );
    assert_eq!(comment_sources(&manifests), vec![fresh]);
    assert!(
        manifests[0]["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["field"] == "work_context:body" && f["contextOnly"] == true)
    );
    finish_pending(&db, &adapter).await;
    work_at(
        &db,
        "grouped-admission",
        "合成练习启动",
        "SYNTHETIC 作者现在建议先理解限制，再选择启动步骤。",
        10,
        "2026-09-30T10:00:00Z",
    )
    .await;
    let changed = start_run(&db, w).await;
    let manifests = source_manifests(&db, changed).await;
    let mut reassessed = comment_sources(&manifests);
    reassessed.sort();
    old.push(fresh);
    old.sort();
    assert_eq!(
        reassessed, old,
        "changed required context invalidates old child-window reuse"
    );
    assert_eq!(manifests.len(), 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_topic_map_research_request WHERE run_ref=$1"
        )
        .bind(changed)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0,
        "admission does not transmit the reassessment before dispatch"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn a_legacy_unknown_voice_is_excluded_before_packing_without_blocking_new_voices() {
    let (db, config, adapter) = setup("topic_core_grouped_unknown").await;
    let w = work(&db, "grouped-unknown", "SYNTHETIC 已知作者的练习建议。").await;
    let unknown = add_comment(
        &db,
        "grouped-unknown",
        "uncertain-voice",
        "SYNTHETIC SLOW_UNKNOWN 我开始练习时仍有困难。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let original = start_run(&db, w).await;
    finish_pending(&db, &adapter).await;
    let before: (i64,i64)=sqlx::query_as("SELECT count(*),COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1")
        .bind(original).fetch_one(db.pool()).await.unwrap();
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state='unknown_dispatch'").bind(original).fetch_one(db.pool()).await.unwrap(),1);
    sqlx::query("UPDATE linggan_topic_map_research_run SET method_version='topic-map.research.v2' WHERE run_ref=$1").bind(original).execute(db.pool()).await.unwrap();
    let mut fresh = Vec::new();
    for index in 0..7 {
        fresh.push(
            add_comment(
                &db,
                "grouped-unknown",
                &format!("fresh-{index}"),
                &format!("合成新的用户{index}：我需要练习启动的帮助。"),
            )
            .await,
        );
    }
    let next = start_run(&db, w).await;
    let manifests = source_manifests(&db, next).await;
    let mut selected = comment_sources(&manifests);
    selected.sort();
    fresh.sort();
    assert_eq!(
        selected, fresh,
        "old uncertain voice cannot poison the fresh group"
    );
    assert!(!selected.contains(&unknown));
    finish_pending(&db, &adapter).await;
    let after: (i64,i64)=sqlx::query_as("SELECT count(*),COALESCE(sum(i.charged_tokens),0)::bigint FROM linggan_topic_map_research_request q JOIN linggan_model_invocation i USING(invocation_ref) WHERE q.run_ref=$1")
        .bind(original).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        after, before,
        "unknown requests and charges remain unchanged"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_discussion_unit WHERE work_public_ref=$1 AND speaker_role='commenter'").bind(w).fetch_one(db.pool()).await.unwrap(),7);
}

async fn author_asr(db: &Database, note: &str, work: Uuid, text: &str) -> Uuid {
    let slot = format!("xhs:{note}:video:1");
    let observation = Uuid::new_v4();
    fixture::submit_package(db,"media_slots",json!({"contentExternalId":note}),json!({
        "kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":"video","ordinal":1},
        "observation":{"externalUri":"https://media.example/synthetic.mp4","candidateUris":["https://media.example/synthetic.mp4"],"observedAt":"2026-09-20T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":note}
    })).await;
    let blob = "e".repeat(64);
    linggan_evidence::admit_media_blob(
        db,
        observation,
        &blob,
        "video/mp4",
        4096,
        "blobs/synthetic/grouped-context",
    )
    .await
    .unwrap();
    let job = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_processing_job(job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) VALUES($1,$2,$3,'asr','synthetic-grouped-context-v1','full')")
        .bind(job).bind(&blob).bind(slot).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,occurred_at) VALUES($1,$2,'succeeded',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    let derivative = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_media_derivative(derivative_ref,job_ref,derivative_kind,content_hash,byte_size,storage_key) VALUES($1,$2,'asr_text',$3,$4,$5)")
        .bind(derivative).bind(job).bind(linggan_evidence::creator_discovery::hash(text)).bind(text.len() as i64).bind(format!("derivatives/grouped/{derivative}.txt")).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_derived_text(derivative_ref,content_public_ref,kind,text_content,display_text,source_location) VALUES($1,$2,'asr_text',$3,$3,$4)")
        .bind(derivative).bind(work).bind(text).bind(json!({"processorVersion":"synthetic-grouped-context-v1"})).execute(db.pool()).await.unwrap();
    derivative
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn saved_author_context_restores_exact_ranges_and_withdrawal_keeps_independent_comment_raw_voice()
 {
    let (db, config, adapter) = setup("topic_core_grouped_saved").await;
    let w = work(&db, "grouped-saved", "").await;
    let author = "SYNTHETIC 先从一次短练习开始，并保留限制👩‍👧。";
    let derivative = author_asr(&db, "grouped-saved", w, author).await;
    let child = add_comment(
        &db,
        "grouped-saved",
        "independent-child",
        "合成用户：短练习已经试过，开始时仍需要提醒。",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let run = start_run(&db, w).await;
    finish_pending(&db, &adapter).await;
    let row=sqlx::query("SELECT r.result_ref,r.method_version,r.core_json FROM linggan_topic_map_research_result r JOIN linggan_topic_map_research_request q USING(invocation_ref) JOIN linggan_topic_map_research_task t USING(task_ref) WHERE t.run_ref=$1 AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.input_refs->'fragments') f WHERE f->>'field'='work_context:transcript') ORDER BY r.created_at LIMIT 1")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    let core: Value = row.get("core_json");
    let assignment = &core["units"][0]["assignments"][0];
    let command = TopicMapCommand::SaveAlternative {
        idempotency_key: "synthetic-grouped-saved".into(),
        domain_ref: D,
        topic_ref: assignment["topicRef"].as_str().unwrap().parse().unwrap(),
        definition_ref: assignment["definitionRef"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
        title: "合成分步练习角度".into(),
        angle: "保留评论和同篇作者语境".into(),
        rationale: "SYNTHETIC / NOT EVIDENCE".into(),
        evidence_work_refs: vec![w],
        method_version: row.get("method_version"),
        research_result_ref: Some(row.get("result_ref")),
        research_angle_index: Some(0),
        research_opportunity_index: None,
    };
    let saved = topic_map::save_topic_map_command(&db, &command)
        .await
        .unwrap()
        .subject_ref
        .unwrap();
    let reopened = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    let context = reopened
        .fragments
        .iter()
        .find(|f| f.field == "work_context:transcript")
        .unwrap();
    assert!(context.context_only && context.fragment_id.contains(".context.chars."));
    assert_eq!(context.text, author);
    assert_eq!(context.start, 0);
    assert_eq!(context.end, author.chars().count());
    let frozen = reopened.research_manifest["inputRefs"]["fragments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fragmentId"] == context.fragment_id)
        .unwrap();
    assert_eq!(
        frozen["textHash"],
        linggan_evidence::creator_discovery::hash(&context.text)
    );
    assert!(
        reopened
            .fragments
            .iter()
            .any(|f| f.source_ref == child && !f.context_only)
    );
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','withdraw grouped author context',clock_timestamp())")
        .bind(Uuid::new_v4()).bind(derivative).execute(db.pool()).await.unwrap();
    let hidden = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_eq!(hidden.source_state, "source_unavailable");
    assert!(hidden.fragments.is_empty() && hidden.angle.is_none());
    // The source gate freezes database acceptance/creation as well as observed
    // time. Obtain this proof database's current cutoff after the withdrawal;
    // a historical platform timestamp cannot expose newly accepted records.
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let raw = comment_study_source::eligible_sources_for_works(&db, D, &as_of, &[w], 10)
        .await
        .unwrap();
    assert!(
        raw.iter()
            .any(|s| s.source_ref == child && s.research_text.contains("短练习已经试过")),
        "withdrawing author context cannot withdraw independent comment raw material"
    );
}

async fn assert_finished_sources(db: &Database, run: Uuid) {
    let states:Value=sqlx::query_scalar("SELECT coalesce(jsonb_agg(jsonb_build_object('state',state,'phase',phase,'reason',last_reason,'units',jsonb_array_length(coalesce(distilled_json->'discussions','[]'::jsonb)))),'[]'::jsonb) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND NOT(recall_manifest ? 'backfill') AND COALESCE(input_refs#>>'{coverage,kind}','source')<>'comparison'")
        .bind(run).fetch_one(db.pool()).await.unwrap();
    assert!(
        states.as_array().unwrap().iter().all(|s| matches!(
            s["state"].as_str(),
            Some("succeeded" | "no_signal" | "insufficient")
        )),
        "source tasks must settle successfully before coverage reuse: {states}"
    );
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn unknown_child_with_unfinished_long_parent_pages_cannot_poison_five_fresh_voices() {
    let (db, config, adapter) = setup("topic_core_grouped_unknown_parent").await;
    let w = work(
        &db,
        "grouped-unknown-parent",
        "SYNTHETIC 已知作者提供练习建议。",
    )
    .await;
    comments::comment_with_author(
        &db,
        "grouped-unknown-parent",
        "creator-long-parent",
        &"合成作者：先核对练习步骤，再说明不同条件和限制。".repeat(170),
        Some("synthetic-creator"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let unknown = comments::reply_with_author(
        &db,
        "grouped-unknown-parent",
        "unknown-child",
        "creator-long-parent",
        "SYNTHETIC SLOW_UNKNOWN 我已经试过，仍需要提醒。",
        Some("synthetic-reader"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let original = start_run(&db, w).await;
    finish_pending(&db, &adapter).await;
    let unknown_count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state='unknown_dispatch'").bind(original).fetch_one(db.pool()).await.unwrap();
    assert!(
        unknown_count > 0,
        "the fixture must actually transmit an uncertain child request"
    );
    sqlx::query("UPDATE linggan_topic_map_research_run SET method_version='topic-map.research.v2' WHERE run_ref=$1").bind(original).execute(db.pool()).await.unwrap();
    let mut fresh = Vec::new();
    for index in 0..5 {
        fresh.push(
            add_comment(
                &db,
                "grouped-unknown-parent",
                &format!("new-parent-voice-{index}"),
                &format!("合成新用户{index}：我需要明确练习启动步骤。"),
            )
            .await,
        );
    }
    let next = start_run(&db, w).await;
    let mut selected = comment_sources(&source_manifests(&db, next).await);
    selected.sort();
    selected.dedup();
    fresh.sort();
    assert_eq!(
        selected, fresh,
        "unfinished parent context cannot undo unknown-source exclusion before grouping"
    );
    assert!(!selected.contains(&unknown));
    finish_pending(&db, &adapter).await;
    assert_finished_sources(&db, next).await;
    assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state='unknown_dispatch'").bind(original).fetch_one(db.pool()).await.unwrap(),unknown_count);
}
