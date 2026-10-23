//! Native PostgreSQL source/save/reopen proofs; only synthetic local model calls.
#[path = "support/comment_research_fixture.rs"]
mod comments;
#[path = "support/topic_map_core_fixture.rs"]
mod core_fixture;
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;
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
    synthetic_citation_config(db, original, "synthetic-topic-map-cite-tail").await
}

async fn synthetic_citation_config(db: &Database, original: Uuid, selector: &str) -> Uuid {
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    // Model entries/configs are immutable. Prepare a separate synthetic model
    // and its callable receipt; keep the original model and every limit intact.
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) SELECT $1,m.connection_version_ref,$3,'manual' FROM linggan_model_entry m JOIN linggan_model_config c USING(model_ref) WHERE c.config_ref=$2")
        .bind(model).bind(original).bind(selector).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,$2,65536,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$3")
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
    sqlx::query_scalar("SELECT t.task_ref FROM linggan_topic_map_research_task t JOIN linggan_topic_map_research_run run USING(run_ref) WHERE t.work_public_ref=$1 AND run.config_ref=$2 AND EXISTS(SELECT 1 FROM jsonb_array_elements(t.input_refs->'fragments') f WHERE f->>'field'=$3 AND ($4::uuid IS NULL OR f->>'sourceRef'=$4::text) AND (f->>'start')::bigint<=$5 AND (f->>'end')::bigint>$5) ORDER BY t.created_at,t.task_ref LIMIT 1")
        .bind(work).bind(config).bind(field).bind(source).bind(start as i64).fetch_one(db.pool()).await.unwrap()
}

async fn finish_source_task(db: &Database, adapter: &PiAdapter, task: Uuid) {
    // Prioritize just this requested proof window. Other queued sources remain
    // untouched, so a 257-media regression need not invoke 257 synthetic analyses.
    sqlx::query("UPDATE linggan_topic_map_research_task SET created_at=scope_001_now()-interval '1 hour' WHERE task_ref=$1")
        .bind(task).execute(db.pool()).await.unwrap();
    for _ in 0..24 {
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
        topic_ref: Some(assignment["topicRef"].as_str().unwrap().parse().unwrap()),
        definition_ref: Some(
            assignment["definitionRef"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
        ),
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
        .find(|fragment| {
            fragment["field"] == field
                && fragment["start"].as_u64().unwrap() <= 12000
                && fragment["end"].as_u64().unwrap() > 12000
        })
        .expect("requested tail fragment must be frozen in the accepted input");
    let end = source.chars().count();
    assert_eq!(tail["end"], json!(end));
    let citations = output["angles"][0]["evidence"].as_array().unwrap();
    assert_eq!(citations.len(), 1, "fixture must explicitly cite its tail");
    assert_eq!(citations[0]["fragmentId"], tail["fragmentId"]);
    let window_start = end.saturating_sub(1000).max(tail["start"].as_u64().unwrap() as usize);
    assert_eq!(citations[0]["start"], json!(window_start));
    assert_eq!(citations[0]["end"], json!(end));
    let emoji_byte = source.find(emoji).expect("fixture tail contains its emoji");
    let emoji_start = source[..emoji_byte].chars().count();
    let emoji_end = emoji_start + emoji.chars().count();
    assert!(window_start <= emoji_start && emoji_end <= end);
    let quoted: String = source
        .chars()
        .skip(window_start)
        .take(end - window_start)
        .collect();
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
        .find(|f| f.start <= 12000 && f.end > 12000)
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
            .find(|f| f.start <= 12000 && f.end > 12000)
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
    let config =
        synthetic_citation_config(&db, config, "synthetic-topic-map-cite-linked-reply").await;
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
    let mut expected_comments = vec![child];
    for index in 0..35 {
        let added = comments::comment_with_author(
            &db,
            "saved-comment-tail",
            &format!("later-{index:03}"),
            &format!("合成第{index}条独立读者反馈，练习需要明确步骤。"),
            Some("another-reader"),
            "2026-09-17T08:00:00Z",
        )
        .await;
        expected_comments.push(added);
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
        12,
        "36 primary comments pack into twelve groups, without independent creator-parent task"
    );
    let mut selected_comments: Vec<Uuid> = sqlx::query_scalar("SELECT DISTINCT (f->>'sourceRef')::uuid FROM linggan_topic_map_research_task t CROSS JOIN LATERAL jsonb_array_elements(t.input_refs->'fragments') f WHERE t.work_public_ref=$1 AND f->>'field' IN('studied_comment','unresearched_comment')")
        .bind(work).fetch_all(db.pool()).await.unwrap();
    selected_comments.sort();
    expected_comments.sort();
    assert_eq!(
        selected_comments, expected_comments,
        "every primary comment identity, including after position 30, survives grouping; creator parent is context only"
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

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn canonical_full_image_reading_reopens_exact_frozen_tuple_and_obeys_restrictions() {
    use linggan_evidence::{FrozenWorkSource, material_reading::{NewReading, Publication, publish_in}};
    let (db, original_config, adapter) = setup("topic_saved_canonical_media").await;
    let config = tail_fixture_config(&db, original_config).await;
    let work = dossier_fixture::work(&db).await;
    let derivative = dossier_fixture::ordinary_ocr(&db).await;
    let initial = linggan_evidence::research_dossier::read(&db, work).await.unwrap().unwrap();
    let image = initial.fragments.iter().find(|f| f.field == "ocr").unwrap();
    let slot: String = sqlx::query_scalar("SELECT slot_key FROM linggan_material_unit WHERE unit_ref=$1")
        .bind(image.unit_ref).fetch_one(db.pool()).await.unwrap();
    let text = format!("{}合格图片尾部👩‍👧仍然属于这一张图的开始练习证据。", "完整正文。".repeat(2400));
    let publish = |text: String, current| NewReading {
        content_public_ref: work, unit_key: format!("image:{slot}"), kind: "image".into(),
        slot_key: Some(slot.clone()), page: Some(1), source_version: image.source_version.clone(),
        source_state: "present".into(), derivative_ref: Some(derivative), producer_ref: Some(Uuid::new_v4()),
        generated_by: "ordinary_ocr".into(), rule_version: "synthetic-canonical-proof.v2".into(),
        blocks: vec![json!({"kind":"paragraph","text":text,"speaker":"author","bbox":[0.1,0.1,0.8,0.8]})],
        issues: vec![], source_location: json!({"synthetic":true}),
        publication: Publication::Automatic { expected_current: current },
    };
    let mut tx = db.pool().begin().await.unwrap();
    let published = publish_in(&mut tx, publish(text.clone(), image.reading_ref)).await.unwrap();
    assert!(published.current);
    tx.commit().await.unwrap();
    let canonical = linggan_evidence::research_dossier::read(&db, work).await.unwrap().unwrap();
    let image = canonical.fragments.iter().find(|f| f.reading_ref == Some(published.reading_ref)).unwrap();
    assert_eq!(image.text, text);
    let frozen = FrozenWorkSource { source_ref:image.source_ref, block_ref:image.fragment_id.clone(),
        field:image.field.clone(), source_version:image.source_version.clone(), as_of:canonical.as_of.clone() };
    let materials = linggan_evidence::creator_discovery::load_topic_materials(&db, D).await.unwrap();
    let full = &materials.works.iter().find(|w| w.work_ref == work).unwrap().fragments;
    assert!(full.iter().any(|f| f.fragment_id == image.fragment_id && f.text == text && f.speaker == "author"));
    apply_research_command(&db, &configure(config, 1000000, false)).await.unwrap();
    apply_research_command(&db, &start(vec![work])).await.unwrap();
    let task = source_task(&db, work, config, "ocr", Some(image.source_ref), 12000).await;
    finish_source_task(&db, &adapter, task).await;
    assert_tail_angle_citation(&db, task, "ocr", &text, "👩‍👧").await;
    let saved = save_task_angle(&db, task, work, "saved-canonical-image-tail").await;
    let reopened = topic_map::read_saved_alternative(&db, D, saved).await.unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    assert!(reopened.fragments.iter().any(|f| f.field == "ocr" && f.text == text && !f.cited_ranges.is_empty()));
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen).await.unwrap(), vec![text.clone()]);
    for field in ["sourceRef", "sourceVersion", "blockRef", "asOf"] {
        let mut wrong = frozen.clone();
        match field {
            "sourceRef" => wrong.source_ref = derivative, // a derivative/job alias never authorizes a block
            "sourceVersion" => wrong.source_version = Uuid::new_v4().to_string(),
            "blockRef" => wrong.block_ref.push_str(".foreign"),
            _ => wrong.as_of = "2000-01-01T00:00:00Z".into(),
        }
        assert!(linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &wrong).await.unwrap().is_empty(), "must bind {field}");
    }
    let next = publish_in(&mut tx, publish("明确更新后的识读".into(), Some(published.reading_ref))).await.unwrap();
    assert!(next.current);
    tx.commit().await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen).await.unwrap(), vec![text.clone()],
        "new reading cannot replace the old cutoff selection");
    tx.commit().await.unwrap();
    let reopened = topic_map::read_saved_alternative(&db, D, saved).await.unwrap();
    assert!(reopened.fragments.iter().any(|f| f.text == text));
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','restricted canonical source',scope_001_now())")
        .bind(Uuid::new_v4()).bind(derivative).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen).await.unwrap().is_empty());
    tx.commit().await.unwrap();
    let hidden = topic_map::read_saved_alternative(&db, D, saved).await.unwrap();
    assert_eq!(hidden.source_state, "source_unavailable");
    assert!(hidden.fragments.is_empty() && hidden.title.is_none() && hidden.original_definition.is_none());
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
        *definition_ref == Some(original_definition),
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
