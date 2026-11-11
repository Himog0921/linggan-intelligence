//! Real PG handoff, existing research sources and creation persistence; no model or publication.
use super::topic_handoff::*;
use super::*;
#[allow(dead_code)]
#[path = "../topic_map_research/tests/saved_source_fixture.rs"]
mod support;
use uuid::Uuid;

#[tokio::test]
#[ignore = "isolated PostgreSQL; synthetic research only"]
async fn saved_angle_handoff_is_atomic_and_reuses_one_idea_across_commands() {
    let (db, config) = support::setup(&format!("topic_handoff_{}", Uuid::new_v4().simple())).await;
    let work = support::work(
        &db,
        "handoff-source",
        "合成作者描述：先拆分作业，再做第一个步骤。",
    )
    .await;
    support::seed_research_inputs(&db, config, &[work]).await;
    let task = support::source_task(&db, work, config, "body", None, 0).await;
    support::seed_accepted_source_task(&db, task).await;
    let alternative = support::save_task_angle(&db, task, work, "handoff-angle").await;
    let request = HandoffRequest {
        command_ref: Uuid::new_v4(),
        domain_ref: support::D,
        alternative_ref: alternative,
    };
    let other = HandoffRequest {
        command_ref: Uuid::new_v4(),
        domain_ref: support::D,
        alternative_ref: alternative,
    };
    let (a, b) = tokio::join!(handoff_angle(&db, &request), handoff_angle(&db, &other));
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(a.work_id, b.work_id);
    assert_eq!(a.idea_ref, b.idea_ref);
    assert_eq!(
        handoff_angle(&db, &request).await.unwrap().work_id,
        a.work_id
    );
    let ideas = list_idea_boxes(&db, support::D).await.unwrap();
    assert_eq!(ideas.len(), 1);
    assert_eq!(ideas[0].anchor_work_id, a.work_id);
    let revision = read_work(&db, a.work_id).await.unwrap().revision;
    assert_eq!(revision.document.idea_ref, Some(a.idea_ref));
    assert!(!revision.document.title.is_empty());
    assert!(!revision.document.angle.is_empty());
    assert_eq!(revision.document.materials.len(), 1);
    assert_eq!(revision.document.materials[0].work_ref, Some(work));
    assert_ne!(
        revision.document.materials[0].source_version,
        "0".repeat(64)
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creation_work")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 1);
    let wrong_domain = HandoffRequest {
        command_ref: Uuid::new_v4(),
        domain_ref: Uuid::new_v4(),
        alternative_ref: alternative,
    };
    assert!(matches!(
        handoff_angle(&db, &wrong_domain).await,
        Err(WorkbenchError::NotFound)
    ));
    let product = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_topic_map_alternative(alternative_ref,domain_ref,topic_ref,definition_ref,title,angle,rationale,method_version,receipt_ref,research_manifest,kind) SELECT $1,domain_ref,topic_ref,definition_ref,title,angle,rationale,method_version,receipt_ref,research_manifest,'product_research' FROM linggan_topic_map_alternative WHERE alternative_ref=$2")
        .bind(product).bind(alternative).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_alternative_evidence(alternative_ref,work_public_ref,source_manifest) SELECT $1,work_public_ref,source_manifest FROM linggan_topic_map_alternative_evidence WHERE alternative_ref=$2")
        .bind(product).bind(alternative).execute(db.pool()).await.unwrap();
    let rejected = HandoffRequest {
        command_ref: Uuid::new_v4(),
        domain_ref: support::D,
        alternative_ref: product,
    };
    assert!(matches!(
        handoff_angle(&db, &rejected).await,
        Err(WorkbenchError::Invalid("angle_required"))
    ));
    assert_eq!(list_idea_boxes(&db, support::D).await.unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; synthetic image withdrawal"]
async fn handoff_withdrawal_redacts_reads_and_rejects_stale_editor_without_hiding_independent_edits()
 {
    let (db, config) = support::setup(&format!("handoff_w_{}", Uuid::new_v4().simple())).await;
    let work = support::dossier_fixture::work(&db).await;
    support::dossier_fixture::current_image_reading(&db).await;
    let dossier = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let image = dossier.fragments.iter().find(|f| f.field == "ocr").unwrap();
    support::seed_research_inputs(&db, config, &[work]).await;
    let task = support::source_task(&db, work, config, "ocr", None, 0).await;
    support::seed_accepted_source_task(&db, task).await;
    let alternative = support::save_task_angle(&db, task, work, "handoff-image").await;
    let linked = handoff_angle(
        &db,
        &HandoffRequest {
            command_ref: Uuid::new_v4(),
            domain_ref: support::D,
            alternative_ref: alternative,
        },
    )
    .await
    .unwrap();
    let before = read_work(&db, linked.work_id).await.unwrap().revision;
    let mut independent = before.document.clone();
    independent.topic_angle_refs.clear(); // Old editor omits server provenance.
    independent.title = "作者独立拟定的标题".into();
    independent.angle = "作者另行确定的创作意图".into();
    independent.idea_name = Some("作者独立命名".into());
    let quote = before.document.angle.clone();
    independent.cover_title_candidates = vec![before.document.title.clone()];
    independent.note_title_candidates = vec![format!("前言{quote}后记")];
    independent.material_preparation = Some(MaterialPreparation {
        method_version: "workbench.lexical-context.v1".into(),
        query: quote.clone(),
        terms: vec!["作业".into(), quote.clone()],
        state: "ready".into(),
        gaps: vec![quote.clone()],
    });
    independent.materials[0].match_reasons = vec![quote.clone()];
    let edited = save(
        &db,
        linked.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: before.id,
            document: independent.clone(),
        },
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,materialization_ref,state,authority_ref,reason,effective_at) VALUES($1,$2,'WITHDRAWN_OR_RESTRICTED','synthetic-proof','handoff image withdrawn',scope_001_now())")
        .bind(Uuid::new_v4()).bind(image.source_version.parse::<Uuid>().unwrap()).execute(db.pool()).await.unwrap();
    let saved = crate::topic_map::read_saved_alternative(&db, support::D, alternative)
        .await
        .unwrap();
    assert_eq!(saved.source_state, "source_unavailable");
    let current = read_work(&db, linked.work_id).await.unwrap();
    assert_eq!(
        current.revision.document.topic_angle_refs,
        vec![alternative]
    );
    assert_eq!(current.revision.document.title, independent.title);
    assert_eq!(current.revision.document.angle, independent.angle);
    assert_eq!(
        list_idea_boxes(&db, support::D).await.unwrap()[0].name,
        "作者独立命名"
    );
    assert_eq!(current.revision.document.cover_title_candidates, vec!["…"]);
    assert_eq!(
        current.revision.document.note_title_candidates,
        vec!["前言…后记"]
    );
    let prep = current
        .revision
        .document
        .material_preparation
        .as_ref()
        .unwrap();
    assert_eq!(prep.query, "…");
    assert!(!prep.terms.iter().any(|term| term.contains(&quote)));
    assert_eq!(prep.gaps, vec!["…"]);
    // Ordinary material qualification may replace reasons with its current canonical labels.
    assert!(
        current.revision.document.materials[0]
            .match_reasons
            .iter()
            .all(|reason| !reason.contains(&quote))
    );
    let mut metadata_only = independent.clone();
    metadata_only.topic_angle_refs.clear();
    assert!(matches!(
        save(
            &db,
            linked.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: edited.revision.id,
                document: metadata_only,
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    let historical = revision(&db, linked.work_id, before.id).await.unwrap();
    assert_eq!(historical.document.title, "[来源当前不可使用]");
    assert_eq!(historical.document.angle, "[来源当前不可使用]");
    assert_eq!(historical.document.idea_name.as_deref(), Some("…"));
    let mut stale_document = before.document.clone();
    stale_document.topic_angle_refs.clear();
    stale_document.idea_ref = None;
    let stale = save(
        &db,
        linked.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: edited.revision.id,
            document: stale_document,
        },
    )
    .await;
    assert!(matches!(stale, Err(WorkbenchError::Source)));
    // The existing transaction consumer used by exports also rejects the withdrawn original.
    let mut tx = db.pool().begin().await.unwrap();
    assert!(matches!(
        super::topic_handoff::qualify_document_in(&mut tx, support::D, &before.document).await,
        Err(WorkbenchError::Source)
    ));
    crate::content_workbench_ai::assert_handoff_outbound_unavailable(
        &mut tx,
        linked.work_id,
        edited.revision.id,
        before.id,
    )
    .await;
    tx.rollback().await.unwrap();
    let mut clean = independent.clone();
    clean.cover_title_candidates.clear();
    clean.note_title_candidates.clear();
    clean.material_preparation = None;
    clean.materials[0].match_reasons.clear();
    let clean = save(
        &db,
        linked.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: edited.revision.id,
            document: clean,
        },
    )
    .await
    .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    crate::content_workbench_ai::assert_handoff_instruction_recheck(
        &mut tx,
        linked.work_id,
        clean.revision.id,
        &quote,
    )
    .await;
    tx.rollback().await.unwrap();
}
