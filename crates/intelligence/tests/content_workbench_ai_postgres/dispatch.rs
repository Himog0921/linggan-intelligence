use super::*;
use linggan_intelligence::content_workbench_sources::{read_source_in, text_hash};

async fn wait_for_call(path: &std::path::Path, before: usize) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while call_count(path) == before {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(call_count(path), before + 1);
    })
    .await
    .expect("the owned adapter must begin its call before the concurrent command");
}
fn release_call(path: &std::path::Path) {
    std::fs::write(path.with_extension("release"), "owned test release").unwrap();
}
async fn frozen_body_source(
    db: &linggan_storage_postgres::Database,
    citation: &Citation,
) -> serde_json::Value {
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

#[tokio::test]
#[ignore = "isolated PostgreSQL proof with local fake transport"]
async fn explicit_ai_dispatch_replay_stale_cancel_and_usage_are_separate() {
    let db = fixture::db("creation_ai").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let work = fixture::create(&db, fixture::doc()).await;
    let (adapter, count, root) = adapter();
    let ready = ai::readiness(&db).await.unwrap();
    assert_eq!(ready["configured"], true);
    assert!(ready["writingQualified"].is_null());
    let r = request(&work, config, ai::WritingAction::Draft, "");
    let exec = prepare(&db, &r).await;
    assert_eq!(call_count(&count), 0);
    ai::execute(&db, &exec).await.unwrap();
    ai::execute(&db, &exec).await.unwrap();
    let mut changed = exec.clone();
    changed.budget_tokens += 1;
    assert!(matches!(
        ai::execute(&db, &changed).await,
        Err(ai::AiError::Conflict)
    ));
    let second_preview = request(&work, config, ai::WritingAction::Draft, "different preview");
    let mut cross_preview = prepare(&db, &second_preview).await;
    cross_preview.request_id = exec.request_id;
    assert!(matches!(
        ai::execute(&db, &cross_preview).await,
        Err(ai::AiError::Conflict)
    ));
    let mut work_collision = prepare(&db, &second_preview).await;
    work_collision.request_id = work.request_id;
    assert!(matches!(
        ai::execute(&db, &work_collision).await,
        Err(ai::AiError::Conflict)
    ));
    assert!(matches!(
        save(
            &db,
            work.work_id,
            &SaveRequest {
                request_id: exec.request_id,
                expected_revision: work.revision.id,
                document: work.revision.document.clone()
            }
        )
        .await,
        Err(WorkbenchError::RequestConflict)
    ));
    let (a, b) = tokio::join!(
        run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter),
        run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
    );
    a.unwrap();
    b.unwrap();
    assert_eq!(call_count(&count), 1);
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "succeeded");
    assert_eq!(status["usage"]["chargedTokens"], 20);
    assert_eq!(status["suggestion"]["paragraphs"][0]["text"], "合成起草😀");
    assert_eq!(ai::readiness(&db).await.unwrap()["writingQualified"], true);
    let mut doc = work.revision.document.clone();
    doc.paragraphs[0].text = "作者新编辑".into();
    let newer = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document: doc,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        ai::status(&db, exec.request_id).await.unwrap()["staleBase"],
        true
    );
    assert!(matches!(
        ai::adopt(
            &db,
            exec.request_id,
            &ai::AdoptRequest {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
                paragraph_indices: None,
                page_indices: None,
                adopt_title: false
            }
        )
        .await,
        Err(ai::AiError::Stale)
    ));
    let r = request(&newer, config, ai::WritingAction::RewriteSelection, "");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let adoption = ai::AdoptRequest {
        request_id: Uuid::new_v4(),
        expected_revision: newer.revision.id,
        paragraph_indices: Some(vec![0]),
        page_indices: Some(vec![]),
        adopt_title: false,
    };
    let adopted = ai::adopt(&db, exec.request_id, &adoption).await.unwrap();
    let replay = ai::adopt(&db, exec.request_id, &adoption).await.unwrap();
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["revision"]["id"], adopted["revision"]["id"]);
    assert_eq!(
        adopted["revision"]["document"]["paragraphs"][0]["id"],
        json!(newer.revision.document.paragraphs[0].id)
    );
    assert_eq!(
        adopted["revision"]["document"]["paragraphs"][0]["text"],
        "合成改写😀"
    );
    let current = read_work(&db, work.work_id).await.unwrap();
    let latest = SaveReceipt {
        request_id: Uuid::new_v4(),
        work_id: work.work_id,
        outcome: "accepted".into(),
        replayed: false,
        revision: current.revision,
    };
    let r = request(&latest, config, ai::WritingAction::Draft, "DELAY");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    let before = call_count(&count);
    let future = run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter);
    let cancel = async {
        wait_for_call(&count, before).await;
        ai::cancel(&db, exec.request_id).await.unwrap();
        release_call(&count);
    };
    let (result, _) = tokio::join!(future, cancel);
    result.unwrap();
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "cancelled");
    assert!(status["suggestion"].is_null());
    assert_eq!(status["usage"]["chargedTokens"], 20);
    let r = request(&latest, config, ai::WritingAction::Draft, "INVALID");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "failed");
    assert_eq!(status["usage"]["chargedTokens"], 20);
    assert_eq!(ai::readiness(&db).await.unwrap()["writingQualified"], false);
    let r = request(&latest, config, ai::WritingAction::PageCopy, "BADPAGE");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        ai::status(&db, exec.request_id).await.unwrap()["state"],
        "failed"
    );
    let r = request(&latest, config, ai::WritingAction::PageCopy, "");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(
        ai::status(&db, exec.request_id).await.unwrap()["state"],
        "succeeded"
    );
    let r = request(&latest, config, ai::WritingAction::Draft, "DELAY");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    let before = call_count(&count);
    let future = run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter);
    let disable = async {
        wait_for_call(&count, before).await;
        sqlx::query("UPDATE linggan_model_connection SET enabled=false")
            .execute(db.pool())
            .await
            .unwrap();
        release_call(&count);
    };
    let (result, _) = tokio::join!(future, disable);
    result.unwrap();
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "succeeded");
    assert_eq!(status["usage"]["chargedTokens"], 20);
    sqlx::query("UPDATE linggan_model_connection SET enabled=true")
        .execute(db.pool())
        .await
        .unwrap();
    let r = request(&latest, config, ai::WritingAction::Draft, "");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    ai::cancel(&db, exec.request_id).await.unwrap();
    let before = call_count(&count);
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&count), before);
    let cancelled = ai::status(&db, exec.request_id).await.unwrap();
    assert!(cancelled["invocationRef"].is_null());
    assert!(cancelled["usage"]["chargedTokens"].is_null());
    assert_eq!(cancelled["usage"]["state"], "not_reserved");
    // Ordinary new observations preserve frozen citations. An actual current restriction
    // still forbids sending their placed text, even with no material checkbox selected.
    let mut document = fixture::cited_doc(&db).await;
    let body = document.excerpts[0].citation.clone();
    let comment = fixture::comment_doc_for_work(&db, body.work_ref.unwrap()).await;
    let comment_work = comment.excerpts[0].citation.work_ref.unwrap();
    let comment_excerpt = comment.excerpts[0].id;
    let comment_text = comment.excerpts[0].selected_text.clone().unwrap();
    let frozen_body = frozen_body_source(&db, &body).await;
    document.excerpts.extend(comment.excerpts);
    document.canvas.extend(comment.canvas);
    let canvases: Vec<_> = document.canvas.iter().map(|c| c.id).collect();
    let paragraph = document.paragraphs[0].id;
    let mut placed = fixture::create(&db, document).await;
    for canvas in canvases {
        placed = place(
            &db,
            placed.work_id,
            &PlaceRequest {
                request_id: Uuid::new_v4(),
                expected_revision: placed.revision.id,
                canvas_id: canvas,
                targets: vec![UseTarget::Paragraph {
                    paragraph_id: paragraph,
                }],
            },
        )
        .await
        .unwrap();
    }
    assert_eq!(placed.revision.document.uses.len(), 2);
    assert!(placed.revision.document.uses.iter().any(|u| {
        u.excerpt_id == Some(comment_excerpt)
            && u.target
                == UseTarget::Paragraph {
                    paragraph_id: paragraph,
                }
    }));
    assert!(
        placed.revision.document.paragraphs[0]
            .text
            .contains(&comment_text)
    );
    let r = request(&placed, config, ai::WritingAction::RewriteSelection, "");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    let before = call_count(&count);
    fixture::dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成替代版本","bodyText":"新版原文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    let current = linggan_evidence::research_dossier::read(&db, body.work_ref.unwrap())
        .await
        .unwrap()
        .unwrap();
    let current_body = current
        .fragments
        .iter()
        .find(|f| f.field == "body")
        .unwrap();
    assert_eq!(current_body.text, "新版原文");
    assert_ne!(current_body.fragment_id, body.block_ref);
    assert_ne!(current_body.source_version, body.source_version);
    assert_eq!(frozen_body_source(&db, &body).await, frozen_body);
    let still_readable = request(&placed, config, ai::WritingAction::RewriteSelection, "");
    assert!(still_readable.citations.is_empty());
    ai::preview(&db, &still_readable).await.unwrap();
    assert_eq!(call_count(&count), before);
    fixture::restrict_comment(&db, comment_work).await;
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&count), before);
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "failed");
    assert_eq!(status["failureCode"], "ai_source_unavailable");
    assert!(status["invocationRef"].is_null());
    assert!(status["usage"]["chargedTokens"].is_null());
    assert_eq!(status["usage"]["state"], "not_reserved");
    assert!(status["suggestion"].is_null());
    let research_count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation i WHERE i.operation='analyze' AND (NOT EXISTS(SELECT 1 FROM linggan_creation_ai_request r WHERE r.invocation_ref=i.invocation_ref) OR i.result->>'purpose' IS DISTINCT FROM 'content_workbench')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(research_count, 0);
    std::fs::remove_dir_all(root).unwrap();
}
