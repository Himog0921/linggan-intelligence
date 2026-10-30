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

#[tokio::test]
#[ignore = "isolated PostgreSQL only; synthetic price and no provider calls"]
async fn creative_monthly_budget_reserves_and_keeps_unknown_spend_unknown() {
    let db = fixture::db("creative_month_budget").await;
    let configured = foundation_fixture::declare(&db, "http://127.0.0.1:9").await;
    let price: Uuid = sqlx::query_scalar("SELECT price_ref FROM linggan_model_price WHERE endpoint_ref=$1 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(configured.endpoint).fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let missing: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,NULL,2,scope_001_now())")
            .bind(Uuid::new_v4())
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(missing, "price_unavailable");
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=1000000 WHERE singleton")
        .execute(&mut *tx)
        .await
        .unwrap();
    let refused: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(Uuid::new_v4())
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(refused, "monthly_budget_exhausted");
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=3000000 WHERE singleton")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invocation = Uuid::new_v4();
    let allowed: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(invocation)
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(allowed, "reserved");
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,price_ref) VALUES($1,$2,$3,'analyze',repeat('0',64),'running',2,2,'writing_legacy',$4)")
        .bind(invocation).bind(configured.version).bind(configured.model).bind(price).execute(&mut *tx).await.unwrap();
    let blocked: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(Uuid::new_v4())
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(blocked, "monthly_budget_exhausted");
    tx.rollback().await.unwrap();

    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key) VALUES($1,$2,$3,'analyze',repeat('0',64),'running',2,2,'writing_legacy')")
        .bind(Uuid::new_v4()).bind(configured.version).bind(configured.model).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let unknown: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(Uuid::new_v4())
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(unknown, "historic_spend_unknown");
    tx.rollback().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL budget settings proof"]
async fn creative_budget_setting_is_revisioned_and_retryable() {
    use linggan_intelligence::creation_assets::{
        AssetsError, ChangeCreativeBudget, change_creative_budget, read_creative_budget,
    };
    let db = fixture::db("creative_month_setting").await;
    let initial = read_creative_budget(&db).await.unwrap();
    assert_eq!(initial.cap_cents, 10_000);
    assert_eq!(initial.unknown_calls, 0);
    let request = Uuid::new_v4();
    let command = ChangeCreativeBudget {
        request_id: request,
        expected_revision: initial.revision,
        cap_cents: 12_345,
    };
    let saved = change_creative_budget(&db, &command).await.unwrap();
    assert_eq!(
        (saved.cap_cents, saved.revision, saved.replayed),
        (12_345, 2, false)
    );
    assert!(
        change_creative_budget(&db, &command)
            .await
            .unwrap()
            .replayed
    );
    let conflicting = ChangeCreativeBudget {
        cap_cents: 12_346,
        ..command
    };
    assert!(matches!(
        change_creative_budget(&db, &conflicting).await,
        Err(AssetsError::RequestConflict)
    ));
    let stale = ChangeCreativeBudget {
        request_id: Uuid::new_v4(),
        ..conflicting
    };
    assert!(matches!(
        change_creative_budget(&db, &stale).await,
        Err(AssetsError::Stale)
    ));
    let current = read_creative_budget(&db).await.unwrap();
    assert_eq!((current.cap_cents, current.revision), (12_345, 2));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL budget concurrency proof"]
async fn creative_monthly_budget_serializes_competing_reservations() {
    let db = fixture::db("creative_month_concurrent").await;
    let configured = foundation_fixture::declare(&db, "http://127.0.0.1:9").await;
    let price: Uuid = sqlx::query_scalar("SELECT price_ref FROM linggan_model_price WHERE endpoint_ref=$1 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(configured.endpoint).fetch_one(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=3000000 WHERE singleton")
        .execute(db.pool())
        .await
        .unwrap();
    let mut first = db.pool().begin().await.unwrap();
    let invocation = Uuid::new_v4();
    let result: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(invocation)
            .bind(price)
            .fetch_one(&mut *first)
            .await
            .unwrap();
    assert_eq!(result, "reserved");
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,price_ref) VALUES($1,$2,$3,'analyze',repeat('0',64),'running',2,2,'writing_legacy',$4)")
        .bind(invocation).bind(configured.version).bind(configured.model).bind(price).execute(&mut *first).await.unwrap();
    let other_db = db.clone();
    let (started, ready) = tokio::sync::oneshot::channel();
    let mut second = tokio::spawn(async move {
        let mut tx = other_db.pool().begin().await.unwrap();
        started.send(()).unwrap();
        let outcome: String =
            sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
                .bind(Uuid::new_v4())
                .bind(price)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
        tx.rollback().await.unwrap();
        outcome
    });
    ready.await.unwrap();
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut second)
            .await
            .is_err()
    );
    first.commit().await.unwrap();
    assert_eq!(second.await.unwrap(), "monthly_budget_exhausted");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL budget gate with local fake transport"]
async fn creative_budget_denial_is_visible_and_never_dispatches() {
    let db = fixture::db("creative_budget_no_dispatch").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=0 WHERE singleton")
        .execute(db.pool())
        .await
        .unwrap();
    let work = fixture::create(&db, fixture::doc()).await;
    let preview = request(&work, config, ai::WritingAction::Draft, "");
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, calls, _root) = adapter();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&calls), 0);
    let status = ai::status(&db, command.request_id).await.unwrap();
    assert_eq!(status["state"], "failed");
    assert_eq!(status["failureCode"], "ai_creative_month_budget_exhausted");
    assert!(status["invocationRef"].is_null());
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_creation_ai_budget_reservation")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL budget settlement proof"]
async fn creative_budget_replaces_a_known_reservation_with_its_actual_cost() {
    let db = fixture::db("creative_month_settlement").await;
    let configured = foundation_fixture::declare(&db, "http://127.0.0.1:9").await;
    let price: Uuid = sqlx::query_scalar("SELECT price_ref FROM linggan_model_price WHERE endpoint_ref=$1 ORDER BY revision_sequence DESC LIMIT 1")
        .bind(configured.endpoint).fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=3000000 WHERE singleton")
        .execute(&mut *tx)
        .await
        .unwrap();
    let invocation = Uuid::new_v4();
    let result: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(invocation)
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(result, "reserved");
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,task_key,price_ref) VALUES($1,$2,$3,'analyze',repeat('0',64),'running',2,2,'writing_legacy',$4)")
        .bind(invocation).bind(configured.version).bind(configured.model).bind(price).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_model_invocation SET state='succeeded',input_tokens=1,output_tokens=0,cached_input_tokens=0,cached_write_tokens=0,finished_at=scope_001_now() WHERE invocation_ref=$1")
        .bind(invocation).execute(&mut *tx).await.unwrap();
    let cost: String = sqlx::query_scalar(
        "SELECT cost_pico_usd::text FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(invocation)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(cost, "1000000");
    let another: String =
        sqlx::query_scalar("SELECT linggan_creation_ai_reserve($1,$2,2,scope_001_now())")
            .bind(Uuid::new_v4())
            .bind(price)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(another, "reserved");
    tx.rollback().await.unwrap();
}
