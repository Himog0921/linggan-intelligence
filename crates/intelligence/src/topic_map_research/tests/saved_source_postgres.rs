//! Live canonical saved-source proofs with explicit synthetic accepted decisions.
//! No retired scheduler or real model is invoked.
#[path = "saved_signal_fixture.rs"]
mod signal_support;
#[path = "saved_source_fixture.rs"]
mod support;
use crate::topic_map::{self, TopicMapCommand};
use serde_json::json;
use support::*;
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn unicode_tail_survives_save_and_same_text_refresh_across_model_configs() {
    let (db, config) = setup("topic_saved_unicode_tail").await;
    let config = tail_fixture_config(&db, config).await;
    let body = format!(
        "{}尾部👩‍👧这里才说明开始练习遇到的限制。",
        "前文练习。".repeat(2400)
    );
    let work = work(&db, "saved-tail", &body).await;
    seed_research_inputs(&db, config, &vec![work]).await;
    let task = source_task(&db, work, config, "body", None, 12000).await;
    seed_accepted_source_task(&db, task).await;
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
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) SELECT $1,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts FROM linggan_model_config WHERE config_ref=$2")
        .bind(second_config).bind(config).execute(db.pool()).await.unwrap();
    seed_research_inputs(&db, second_config, &vec![work]).await;
    let next = source_task(&db, work, second_config, "body", None, 12000).await;
    seed_accepted_source_task(&db, next).await;
    assert_tail_angle_citation(&db, next, "body", &body, "👩‍👧").await;
    let accepted: serde_json::Value = sqlx::query_scalar("SELECT fragment FROM linggan_topic_map_research_request request JOIN linggan_topic_map_research_result result USING(invocation_ref) CROSS JOIN LATERAL jsonb_array_elements(request.request_manifest#>'{source,fragments}') fragment WHERE request.task_ref=$1 AND fragment->>'field'='body'")
        .bind(next).fetch_one(db.pool()).await.unwrap();
    assert_ne!(accepted["sourceRef"], json!(source));
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
    topic_map::save_topic_map_command(
        &db,
        &TopicMapCommand::OwnCreator {
            idempotency_key: "synthetic-later-own-mark".into(),
            domain_ref: D,
            platform: "xhs".into(),
            author_external_id: "synthetic-creator".into(),
            active: true,
        },
    )
    .await
    .unwrap();
    topic_map::save_topic_map_command(
        &db,
        &TopicMapCommand::Breakout {
            idempotency_key: "synthetic-later-breakout-mark".into(),
            domain_ref: D,
            work_ref: work,
            marked: true,
        },
    )
    .await
    .unwrap();
    let current = super::load_inputs_for_works(&db, D, second_config, &[work])
        .await
        .unwrap();
    assert_eq!(current[0].role_metadata["own"], true);
    assert_eq!(current[0].role_metadata["manualOwnBreakout"], true);
    work_at(&db, "saved-tail", "", "", 1000, "2026-10-01T10:00:00Z").await;
    let after_empty_capture = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(after_empty_capture.source_state, "source_unavailable");
    assert!(
        after_empty_capture
            .fragments
            .iter()
            .any(|f| f.source_ref == source && f.text == body)
    );
    let projected = projected_research(&db, work).await;
    let projected_body = projected["fragments"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["fragmentId"] == accepted["fragmentId"])
        .expect("overview keeps the newest accepted revision of this semantic window");
    assert_eq!(projected_body["text"], body);
    for key in [
        "sourceRef",
        "sourceVersion",
        "field",
        "readingRef",
        "unitRef",
        "start",
        "end",
    ] {
        assert_eq!(
            projected_body[key], accepted[key],
            "overview must keep accepted {key}"
        );
    }
    assert_eq!(projected_body["structure"]["ref"], accepted["blockRef"]);
    let frozen = linggan_evidence::FrozenWorkSource {
        source_ref: accepted["sourceRef"].as_str().unwrap().parse().unwrap(),
        source_version: accepted["sourceVersion"].as_str().unwrap().into(),
        field: "body".into(),
        block_ref: accepted["blockRef"].as_str().unwrap().into(),
        as_of: accepted["asOf"].as_str().unwrap().into(),
    };
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(
        linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen)
            .await
            .unwrap(),
        vec![body]
    );
    tx.commit().await.unwrap();
    assert_eq!(projected["core"]["coverage"]["sourceChars"], 0);
    assert_eq!(projected["core"]["coverage"]["coveredChars"], 0);
    assert_eq!(projected["core"]["coverage"]["completedWindows"], 0);
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn thirty_first_comment_and_real_creator_parent_reopen_then_restrict_together() {
    let (db, config) = setup("topic_saved_comment_tail").await;
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
    seed_research_inputs(&db, config, &vec![work]).await;
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
    seed_accepted_source_task(&db, task).await;
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
    let parent = original
        .fragments
        .iter()
        .find(|f| f.field == "parent_comment_context")
        .unwrap()
        .source_ref;
    let signal = signal_support::seed_signal(&db, config, child, parent).await;
    let latest = super::load_inputs_for_works(&db, D, config, &[work])
        .await
        .unwrap();
    assert!(
        latest[0]
            .fragments
            .iter()
            .any(|f| f.source_ref == child && f.field == "studied_comment")
    );
    assert!(
        latest[0]
            .comment_study
            .as_array()
            .unwrap()
            .iter()
            .any(|comment| comment["signals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["signalRef"] == json!(signal)))
    );
    let after_signal = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(after_signal.source_state, "source_unavailable");
    assert!(
        after_signal
            .fragments
            .iter()
            .any(|f| f.source_ref == child && f.field == "unresearched_comment")
    );
    let replacement_parent = comments::comment_with_author(
        &db,
        "saved-comment-tail",
        "creator-parent",
        "",
        Some("synthetic-creator"),
        "2026-10-01T08:00:00Z",
    )
    .await;
    let replacement_child = comments::reply_with_author(
        &db,
        "saved-comment-tail",
        "old-eligible-reply",
        "creator-parent",
        "",
        Some("synthetic-reader"),
        "2026-10-01T08:01:00Z",
    )
    .await;
    assert_ne!(replacement_child, child);
    assert_ne!(replacement_parent, parent);
    let latest = super::load_inputs_for_works(&db, D, config, &[work])
        .await
        .unwrap();
    assert!(
        latest[0].fragments.iter().all(|f| {
            ![child, parent, replacement_child, replacement_parent].contains(&f.source_ref)
        }),
        "latest unknown/empty bodies are not researchable"
    );
    let after_empty_capture = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(after_empty_capture.source_state, "source_unavailable");
    for expected in original
        .fragments
        .iter()
        .filter(|f| [child, parent].contains(&f.source_ref))
    {
        let restored = after_empty_capture
            .fragments
            .iter()
            .find(|f| f.source_ref == expected.source_ref && f.field == expected.field)
            .expect("new unknown observations do not revoke frozen child or parent sources");
        assert_eq!(restored.source_version, expected.source_version);
        assert_eq!(restored.fragment_id, expected.fragment_id);
        assert_eq!(restored.text, expected.text);
        assert_eq!(restored.context_only, expected.context_only);
    }
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
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn canonical_full_image_reading_reopens_exact_frozen_tuple_and_obeys_restrictions() {
    use linggan_evidence::{
        FrozenWorkSource,
        material_reading::{NewReading, Publication, publish_in},
    };
    let (db, original_config) = setup("topic_saved_canonical_media").await;
    let config = tail_fixture_config(&db, original_config).await;
    let work = dossier_fixture::work(&db).await;
    let derivative = dossier_fixture::ordinary_ocr(&db).await;
    let initial = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let image = initial.fragments.iter().find(|f| f.field == "ocr").unwrap();
    let slot: String =
        sqlx::query_scalar("SELECT slot_key FROM linggan_material_unit WHERE unit_ref=$1")
            .bind(image.unit_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let text = format!(
        "{}合格图片尾部👩‍👧仍然属于这一张图的开始练习证据。",
        "完整正文。".repeat(2400)
    );
    let publish = |text: String, current| NewReading {
        content_public_ref: work,
        unit_key: format!("image:{slot}"),
        kind: "image".into(),
        slot_key: Some(slot.clone()),
        page: Some(1),
        source_version: image.source_version.clone(),
        source_state: "present".into(),
        derivative_ref: Some(derivative),
        producer_ref: Some(Uuid::new_v4()),
        generated_by: "ordinary_ocr".into(),
        rule_version: "synthetic-canonical-proof.v2".into(),
        blocks: vec![
            json!({"kind":"paragraph","text":text,"speaker":"author","bbox":[0.1,0.1,0.8,0.8]}),
        ],
        issues: vec![],
        source_location: json!({"synthetic":true}),
        publication: Publication::Automatic {
            expected_current: current,
        },
    };
    let mut tx = db.pool().begin().await.unwrap();
    let published = publish_in(&mut tx, publish(text.clone(), image.reading_ref))
        .await
        .unwrap();
    assert!(published.current);
    tx.commit().await.unwrap();
    let canonical = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let image = canonical
        .fragments
        .iter()
        .find(|f| f.reading_ref == Some(published.reading_ref))
        .unwrap();
    assert_eq!(image.text, text);
    let frozen = FrozenWorkSource {
        source_ref: image.source_ref,
        block_ref: image.fragment_id.clone(),
        field: image.field.clone(),
        source_version: image.source_version.clone(),
        as_of: canonical.as_of.clone(),
    };
    let materials = linggan_evidence::creator_discovery::load_topic_materials(&db, D)
        .await
        .unwrap();
    let full = &materials
        .works
        .iter()
        .find(|w| w.work_ref == work)
        .unwrap()
        .fragments;
    assert!(
        full.iter()
            .any(|f| f.fragment_id == image.fragment_id && f.text == text && f.speaker == "author")
    );
    seed_research_inputs(&db, config, &vec![work]).await;
    let task = source_task(&db, work, config, "ocr", Some(image.source_ref), 12000).await;
    seed_accepted_source_task(&db, task).await;
    assert_tail_angle_citation(&db, task, "ocr", &text, "👩‍👧").await;
    let saved = save_task_angle(&db, task, work, "saved-canonical-image-tail").await;
    let reopened = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert_ne!(reopened.source_state, "source_unavailable");
    assert!(
        reopened
            .fragments
            .iter()
            .any(|f| f.field == "ocr" && f.text == text && !f.cited_ranges.is_empty())
    );
    let before_update = projected_research(&db, work).await;
    assert_eq!(before_update["core"]["coverage"]["state"], "complete");
    assert_eq!(before_update["core"]["coverage"]["completedWindows"], 1);
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(
        linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen)
            .await
            .unwrap(),
        vec![text.clone()]
    );
    for field in ["sourceRef", "sourceVersion", "blockRef", "asOf"] {
        let mut wrong = frozen.clone();
        match field {
            "sourceRef" => wrong.source_ref = derivative, // a derivative/job alias never authorizes a block
            "sourceVersion" => wrong.source_version = Uuid::new_v4().to_string(),
            "blockRef" => wrong.block_ref.push_str(".foreign"),
            _ => wrong.as_of = "2000-01-01T00:00:00Z".into(),
        }
        assert!(
            linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &wrong)
                .await
                .unwrap()
                .is_empty(),
            "must bind {field}"
        );
    }
    let next = publish_in(
        &mut tx,
        publish("明确更新后的识读".into(), Some(published.reading_ref)),
    )
    .await
    .unwrap();
    assert!(next.current);
    tx.commit().await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(
        linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen)
            .await
            .unwrap(),
        vec![text.clone()],
        "new reading cannot replace the old cutoff selection"
    );
    tx.commit().await.unwrap();
    let reopened = topic_map::read_saved_alternative(&db, D, saved)
        .await
        .unwrap();
    assert!(reopened.fragments.iter().any(|f| f.text == text));
    let after_update = projected_research(&db, work).await;
    assert!(
        after_update["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["text"] == text)
    );
    assert_eq!(after_update["core"]["coverage"]["state"], "partial");
    assert_eq!(after_update["core"]["coverage"]["completedWindows"], 0);
    assert_eq!(after_update["core"]["coverage"]["coveredChars"], 0);
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,derivative_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','restricted canonical source',scope_001_now())")
        .bind(Uuid::new_v4()).bind(derivative).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(
        linggan_evidence::read_frozen_work_text_in_transaction(&mut tx, work, &frozen)
            .await
            .unwrap()
            .is_empty()
    );
    tx.commit().await.unwrap();
    let hidden = topic_map::read_saved_alternative(&db, D, saved)
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
    let (db, config) = setup(name).await;
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
    seed_research_inputs(&db, config, &vec![first]).await;
    seed_all_accepted(&db).await;
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
    seed_research_inputs(&db, config, &vec![second]).await;
    seed_all_accepted(&db).await;
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
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn withdrawn_machine_definition_hides_qualified_saved_angle_and_rejects_new_save() {
    assert_definition_withdrawal_hides_saved(false).await;
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic accepted records, no provider or worker"]
async fn withdrawn_unassigned_neighbor_hides_dependent_machine_definition_and_saved_angle() {
    assert_definition_withdrawal_hides_saved(true).await;
}
