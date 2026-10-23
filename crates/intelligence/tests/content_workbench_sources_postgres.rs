#![recursion_limit = "256"]
use linggan_intelligence::{content_workbench::types::*, content_workbench_sources as sources};
use serde_json::json;
use uuid::Uuid;
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
use fixture::dossier::{self, DOMAIN};

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn typed_material_search_reads_only_requested_family() {
    use sources::materials::{SearchQuery, search};
    let schema = "creation_kind_read_only";
    let db = fixture::db(schema).await;
    fixture::comment_doc(&db).await;
    fixture::ops_seed(&db).await;
    // Fix only this isolated schema's read clock so every document field can be compared.
    sqlx::raw_sql("CREATE TABLE kind_search_clock AS SELECT clock_timestamp() AS value; CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql STABLE AS $$ SELECT value FROM kind_search_clock $$;")
        .execute(db.pool()).await.unwrap();
    let kinds = [
        SourceKind::Note,
        SourceKind::Comment,
        SourceKind::OpsPublication,
    ];
    let patterns = [
        "%FROM linggan_material_content material WHERE EXISTS%",
        "%FROM linggan_material_comment_current c JOIN linggan_runtime_capture_package package USING(package_ref) WHERE package.accepted_at%",
        "%FROM linggan_ops_publication p JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE p.merged_into_ref%",
    ];
    let query = |kind| SearchQuery {
        domain_ref: DOMAIN,
        query: "合成".into(),
        kind,
        cursor: None,
    };
    let all_before = search(&db, &query(None)).await.unwrap();
    assert!(all_before["nextCursor"].is_null());
    let mut baselines = Vec::new();
    let mut failures = Vec::new();
    for (selected, kind) in kinds.iter().enumerate() {
        let baseline = search(&db, &query(Some(kind.clone()))).await.unwrap();
        assert!(!baseline["documents"].as_array().unwrap().is_empty());
        let expected: Vec<_> = all_before["documents"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|document| document["sourceKind"] == json!(kind))
            .cloned()
            .collect();
        assert_eq!(baseline["documents"], json!(expected));
        sqlx::query("SELECT public.pg_stat_statements_reset()")
            .execute(db.pool())
            .await
            .unwrap();
        let measured = search(&db, &query(Some(kind.clone()))).await.unwrap();
        assert_eq!(measured["documents"], baseline["documents"]);
        for (family, pattern) in patterns.iter().enumerate() {
            let calls: i64 = sqlx::query_scalar("SELECT COALESCE(sum(calls),0)::bigint FROM public.pg_stat_statements WHERE query LIKE $1")
                .bind(pattern).fetch_one(db.pool()).await.unwrap();
            if family == selected {
                assert!(calls > 0, "selected reader must actually run: {kind:?}");
            } else if calls != 0 {
                failures.push(format!(
                    "{kind:?} unnecessarily read family {family}: {calls} directory calls"
                ));
            }
        }
        baselines.push(baseline["documents"].clone());
    }
    assert_eq!(
        search(&db, &query(None)).await.unwrap()["documents"],
        all_before["documents"]
    );

    // The selected readers must also survive an unavailable unrelated publication reader.
    // A fresh pool prevents a pre-DDL prepared plan from masking the missing relation.
    sqlx::query("ALTER TABLE linggan_ops_publication RENAME TO unavailable_publication_reader")
        .execute(db.pool())
        .await
        .unwrap();
    let isolated_url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let fresh = linggan_storage_postgres::Database::connect_within_schema(&isolated_url, schema)
        .await
        .unwrap();
    for (selected, kind) in kinds.iter().take(2).enumerate() {
        match search(&fresh, &query(Some(kind.clone()))).await {
            Ok(result) => assert_eq!(result["documents"], baselines[selected]),
            Err(error) => failures.push(format!(
                "{kind:?} failed on unrelated publication reader: {error}"
            )),
        }
    }
    sqlx::query("ALTER TABLE unavailable_publication_reader RENAME TO linggan_ops_publication")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn exact_sources_keep_domain_version_parent_and_ocr_image_identity() {
    let db = fixture::db("creation_sources").await;
    let doc = fixture::cited_doc(&db).await;
    let c = doc.excerpts[0].citation.clone();
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(
        sources::validate_citation_in_domain(&mut tx, &c, DOMAIN)
            .await
            .unwrap()
            .selected_text,
        doc.excerpts[0].selected_text.clone().unwrap()
    );
    assert!(
        sources::validate_citation_in_domain(&mut tx, &c, Uuid::new_v4())
            .await
            .is_err()
    );
    let mut forged = c.clone();
    forged.text_hash = "0".repeat(64);
    assert!(
        sources::validate_citation_in_domain(&mut tx, &forged, DOMAIN)
            .await
            .is_err()
    );
    tx.commit().await.unwrap();
    let work = c.work_ref.unwrap();
    dossier::ordinary_ocr(&db).await;
    let canonical = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let image = canonical.fragments.iter().find(|f| f.field == "ocr" && f.text.starts_with("第0行"))
        .expect("ordinary OCR publishes canonical blocks before consumers read");
    let mut tx = db.pool().begin().await.unwrap();
    let (source, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Ocr,
        image.source_ref,
        Some(work),
        &SourceField::Ocr,
        &image.fragment_id,
        &canonical.as_of,
    )
    .await
    .unwrap();
    let location = source.location.unwrap();
    assert_eq!(location.reading_ref, image.reading_ref.unwrap());
    assert_eq!(location.unit_ref, image.unit_ref.unwrap());
    assert_eq!(location.block_ref, image.fragment_id);
    assert!(location.bbox[0] > 0.);
    assert!((location.bbox[2] - 0.76).abs() < 1e-6);
    assert!(
        context["dossier"]["materials"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["currentRevisionRef"] == json!(image.reading_ref) && m["originalUrl"].is_string())
    );
    tx.commit().await.unwrap();
    for (kind, id, parent) in [
        ("comments", "parent", None),
        ("replies", "reply", Some("parent")),
        ("replies", "unknown-reply", Some("not-captured")),
    ] {
        let mut payload =
            json!({"commentId":id,"noteId":"dossier-note","text":format!("{id}原声😀\n确切全文")});
        if let Some(parent) = parent {
            payload["rootCommentId"] = json!(parent);
            payload["parentCommentId"] = json!(parent);
        }
        dossier::fixture::submit_package(&db,kind,json!({"contentExternalId":"dossier-note"}),json!({"kind":if kind=="comments"{"comment"}else{"reply"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":payload})).await;
    }
    let reply:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id='reply'").fetch_one(db.pool()).await.unwrap();
    let unknown:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id='unknown-reply'").fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        reply,
        Some(work),
        &SourceField::CommentBody,
        &format!("{reply}.comment_body"),
        &as_of,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "known");
    assert_eq!(context["parentWork"]["state"], "known");
    assert!(!context.to_string().contains("authorId"));
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        unknown,
        Some(work),
        &SourceField::CommentBody,
        &format!("{unknown}.comment_body"),
        &as_of,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "unknown");
    tx.commit().await.unwrap();
    let before_parent_restriction = sources::materials::read_document(
        &db,
        &sources::materials::DocumentQuery {
            domain_ref: DOMAIN,
            source_kind: SourceKind::Comment,
            source_ref: reply,
            work_ref: Some(work),
            as_of: None, block_ref: None, source_version: None, citation_source_ref: None, field: None,
        },
    )
    .await
    .unwrap();
    let context_selection:MaterialSelection=serde_json::from_value(json!({
        "documentId":before_parent_restriction["documentId"],"sourceKind":"comment","sourceRef":reply,"workRef":work,
        "sourceVersion":before_parent_restriction["sourceVersion"],"addedBy":"manual","matchReasons":[]
    })).unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'parent','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    let (_, context) = sources::read_source_in(
        &mut tx,
        &SourceKind::Comment,
        reply,
        Some(work),
        &SourceField::CommentBody,
        &format!("{reply}.comment_body"),
        &as_of,
    )
    .await
    .unwrap();
    assert_eq!(context["parentComment"]["state"], "restricted");
    assert!(context["parentComment"]["body"].is_null());
    tx.commit().await.unwrap();
    let after_parent_restriction = sources::materials::resolve(
        &db,
        &sources::materials::ResolveRequest {
            domain_ref: DOMAIN,
            selections: vec![context_selection],
            terms: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        after_parent_restriction["documents"][0]["qualification"],
        "changed"
    );
    assert_eq!(
        after_parent_restriction["documents"][0]["fragments"][0]["sourceVersion"],
        before_parent_restriction["fragments"][0]["sourceVersion"]
    );
    assert!(after_parent_restriction["documents"][0]["context"]["parentComment"]["body"].is_null());

    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'reply','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(matches!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::Comment,
            reply,
            Some(work),
            &SourceField::CommentBody,
            &format!("{reply}.comment_body"),
            &as_of,
        )
        .await,
        Err(sources::SourceError::Restricted)
    ));
    tx.commit().await.unwrap();
    dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成新版本","bodyText":"新版本正文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    let mut tx = db.pool().begin().await.unwrap();
    assert_eq!(sources::validate_citation_in_domain(&mut tx, &c, DOMAIN).await.unwrap().selected_text,
        doc.excerpts[0].selected_text.clone().unwrap(), "a refresh does not rewrite frozen citations");
    for mutation in ["sourceRef", "sourceVersion", "blockRef", "asOf"] {
        let mut changed = c.clone();
        match mutation {
            "sourceRef" => changed.source_ref = Uuid::new_v4(),
            "sourceVersion" => changed.source_version = Uuid::new_v4().to_string(),
            "blockRef" => changed.block_ref = format!("{}.forged", changed.block_ref),
            _ => changed.as_of = "2000-01-01T00:00:00Z".into(),
        }
        assert!(sources::validate_citation_in_domain(&mut tx, &changed, DOMAIN).await.is_err(), "must bind {mutation}");
    }
    tx.commit().await.unwrap();
    let frozen_document = sources::materials::read_document(&db, &sources::materials::DocumentQuery {
        domain_ref: DOMAIN, source_kind: SourceKind::Note, source_ref: work, work_ref: Some(work),
        as_of: Some(c.as_of.clone()), block_ref: Some(c.block_ref.clone()), source_version: Some(c.source_version.clone()),
        citation_source_ref: Some(c.source_ref), field: Some(c.field.clone()),
    }).await.unwrap();
    assert_eq!(frozen_document["title"], "合成底稿验收");
    assert!(frozen_document["fragments"].as_array().unwrap().iter()
        .any(|f| f["blockRef"] == c.block_ref && f["sourceVersion"] == c.source_version));
    assert!(!frozen_document.to_string().contains("新版本正文"));
    let (_, publication) = fixture::ops_seed(&db).await;
    let query = sources::SourceQuery {
        domain_ref: Some(DOMAIN),
        kind: Some(SourceKind::OpsPublication),
        ..Default::default()
    };
    let page = sources::list_sources(&db, &query).await.unwrap();
    assert_eq!(page["totalBasis"], "source_directory");
    assert!(page["returnedFragmentCount"].as_u64().unwrap() > 0);
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    assert!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::OpsPublication,
            publication,
            None,
            &SourceField::Body,
            &format!("{publication}.body"),
            &as_of,
        )
        .await
        .is_ok()
    );
    tx.commit().await.unwrap();
    sqlx::query("UPDATE linggan_ops_publication SET missing_since_batch_ref=first_batch_ref WHERE publication_ref=$1").bind(publication).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    assert!(
        sources::read_source_in(
            &mut tx,
            &SourceKind::OpsPublication,
            publication,
            None,
            &SourceField::Body,
            &format!("{publication}.body"),
            &as_of,
        )
        .await
        .is_err()
    );
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn prepared_reading_is_related_complete_persistent_and_permission_current() {
    use sources::materials::*;
    let db = fixture::db("creation_materials").await;
    let work = dossier::work(&db).await;
    let long = format!(
        "写作业总是拖延，先把第一步变小。😀\n{}末尾仍须完整",
        "连续原文。".repeat(900)
    );
    dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"作业启动的具体场景","bodyText":long,"authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    for (id, text) in [
        ("useful", "写作业时一直磨蹭，第一步不知道怎么开始😀"),
        ("generic", "ADHD孩子加油"),
        ("vague", "我也开始了"),
    ] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":id,"noteId":"dossier-note","text":text}})).await;
    }
    let request = PrepareRequest {
        domain_ref: DOMAIN,
        title: "每天催三遍，不如把第一步变小".into(),
        angle: "作业启动".into(),
        outline: vec![],
        topic_ref: None,
        terms: None,
    };
    let result = prepare(&db, &request).await.unwrap();
    assert_eq!(result["state"], "ready", "{result}");
    let documents = result["documents"].as_array().unwrap();
    assert_eq!(documents.len(), 2);
    let note = documents
        .iter()
        .find(|d| d["sourceKind"] == "note")
        .unwrap();
    assert_eq!(note["workRef"], json!(work));
    assert_eq!(note["fragments"].as_array().unwrap().len(), 2);
    assert_eq!(note["fragments"][1]["text"], long);
    let voice = documents
        .iter()
        .find(|d| d["sourceKind"] == "comment")
        .unwrap();
    assert_eq!(voice["context"]["parentWork"]["state"], "known");
    assert!(
        voice["fragments"][0]["text"]
            .as_str()
            .unwrap()
            .contains("磨蹭")
    );
    assert!(!result.to_string().contains("ADHD孩子加油"));
    let selections: Vec<MaterialSelection> = documents.iter().map(|d| serde_json::from_value(json!({"documentId":d["documentId"],"sourceKind":d["sourceKind"],"sourceRef":d["sourceRef"],"workRef":d["workRef"],"sourceVersion":d["sourceVersion"],"addedBy":"prepared","matchReasons":[]})).unwrap()).collect();
    let mut document = WorkDocument::default();
    document.title = "隔离稿件".into();
    document.materials = selections.clone();
    let saved = fixture::create(&db, document).await;
    let loaded = linggan_intelligence::content_workbench::read_work(&db, saved.work_id)
        .await
        .unwrap();
    assert_eq!(loaded.revision.document.materials.len(), 2);
    assert!(loaded.revision.document.excerpts.is_empty());
    assert!(loaded.revision.document.canvas.is_empty());
    let repeated = resolve(
        &db,
        &ResolveRequest {
            domain_ref: DOMAIN,
            selections: selections.clone(),
            terms: Some(vec!["作业".into(), "启动".into(), "磨蹭".into()]),
        },
    )
    .await
    .unwrap();
    assert_eq!(repeated["documents"].as_array().unwrap().len(), 2);
    let fresh_note = repeated["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["sourceKind"] == "note")
        .unwrap();
    assert!(
        fresh_note["matchReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().contains("当前原文命中：作业、启动"))
    );
    let fresh_voice = repeated["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["sourceKind"] == "comment")
        .unwrap();
    assert!(
        fresh_voice["matchReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r.as_str().unwrap().contains("当前原文命中：作业、磨蹭"))
    );
    assert!(
        fresh_voice["matchReasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r
                .as_str()
                .unwrap()
                .contains("父笔记当前原文命中：作业、启动"))
    );

    let mut bad = loaded.revision.document.clone();
    bad.materials.push(bad.materials[0].clone());
    assert!(
        linggan_intelligence::content_workbench::create_work(
            &db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: DOMAIN,
                document: bad
            }
        )
        .await
        .is_err()
    );
    let outside = resolve(
        &db,
        &ResolveRequest {
            domain_ref: Uuid::new_v4(),
            selections: selections.clone(),
            terms: Some(vec!["作业".into(), "启动".into(), "磨蹭".into()]),
        },
    )
    .await
    .unwrap();
    assert!(
        outside["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["fragments"] == json!([]) && d["context"].is_null())
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'useful','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let restricted = resolve(
        &db,
        &ResolveRequest {
            domain_ref: DOMAIN,
            selections,
            terms: Some(vec!["作业".into(), "启动".into(), "磨蹭".into()]),
        },
    )
    .await
    .unwrap();
    let restricted_voice = restricted["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["sourceKind"] == "comment")
        .unwrap();
    assert_eq!(restricted_voice["qualification"], "restricted");
    assert_eq!(restricted_voice["fragments"], json!([]));
    assert!(restricted_voice["context"].is_null());
    assert!(
        !restricted_voice["matchReasons"]
            .to_string()
            .contains("当前原文命中")
    );
    let missing = prepare(
        &db,
        &PrepareRequest {
            terms: Some(vec!["不存在的具体场景".into()]),
            ..request
        },
    )
    .await
    .unwrap();
    assert_eq!(missing["state"], "insufficient");
    assert_eq!(missing["documents"], json!([]));
    assert!(
        search(
            &db,
            &SearchQuery {
                domain_ref: DOMAIN,
                query: "的".into(),
                kind: None,
                cursor: None
            }
        )
        .await
        .is_err()
    );
    for i in 0..25 {
        let id = format!("search-ordinary-{i}");
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"dossier-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":id,"noteId":"dossier-note","text":format!("作业普通材料搜索分页{i}")}})).await;
    }
    let latest = search(
        &db,
        &SearchQuery {
            domain_ref: DOMAIN,
            query: "作业".into(),
            kind: Some(SourceKind::Note),
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert!(
        latest["documents"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["sourceKind"] == "note")
    );
    assert_eq!(latest["documents"].as_array().unwrap().len(), 1);
    assert!(latest["nextCursor"].is_null());
    let comments = search(
        &db,
        &SearchQuery {
            domain_ref: DOMAIN,
            kind: Some(SourceKind::Comment),
            query: "作业".into(),
            cursor: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(comments["documents"].as_array().unwrap().len(), 24);
    assert!(
        comments["documents"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["sourceKind"] == "comment")
    );
    assert_eq!(comments["nextCursor"], 24);
    let last = search(
        &db,
        &SearchQuery {
            domain_ref: DOMAIN,
            kind: Some(SourceKind::Comment),
            query: "作业".into(),
            cursor: Some(24),
        },
    )
    .await
    .unwrap();
    assert_eq!(last["documents"].as_array().unwrap().len(), 1);
    assert!(last["nextCursor"].is_null());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn preparation_metadata_keeps_known_citation_restrictions_across_history_and_receipts() {
    use linggan_intelligence::content_workbench as core;
    let db = fixture::db("creation_metadata_sources").await;
    let mut document = fixture::comment_doc(&db).await;
    let quote = document.excerpts[0].selected_text.clone().unwrap();
    let parent = document.excerpts[0].citation.work_ref.unwrap();
    document.title = quote.clone();
    let prepared = sources::materials::prepare(
        &db,
        &sources::materials::PrepareRequest {
            domain_ref: DOMAIN,
            title: quote.clone(),
            angle: "作业启动".into(),
            outline: vec![],
            topic_ref: None,
            terms: Some(vec!["作业".into(), "启动".into()]),
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared["query"], quote);
    document.material_preparation = Some(MaterialPreparation {
        method_version: sources::materials::METHOD.into(),
        query: format!("作者前言{}作者后记", prepared["query"].as_str().unwrap()),
        terms: vec!["作业".into(), quote.clone()],
        state: "insufficient".into(),
        gaps: vec!["普通范围说明".into(), format!("待核对{quote}，作者备注")],
    });
    let create = CreateRequest {
        request_id: Uuid::new_v4(),
        domain_ref: DOMAIN,
        document,
    };
    let first = core::create_work(&db, &create).await.unwrap();
    let mut updated = first.revision.document.clone();
    updated.title = "独立作者标题".into();
    updated.excerpts.clear();
    updated.canvas.clear();
    updated.uses.clear();
    let command = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        document: updated,
    };
    let second = core::save(&db, first.work_id, &command).await.unwrap();
    assert_eq!(
        second.revision.document.excerpts.len(),
        1,
        "metadata retains the exact known Citation even if canvas/quote placements were removed"
    );
    fixture::restrict_comment(&db, parent).await;
    let safe = |document: &WorkDocument| {
        assert!(!document.title.contains(&quote));
        assert!(
            document
                .excerpts
                .iter()
                .all(|excerpt| excerpt.selected_text.is_none())
        );
        let preparation = document.material_preparation.as_ref().unwrap();
        assert_eq!(preparation.terms[0], "作业");
        assert_eq!(preparation.gaps[0], "普通范围说明");
        assert_eq!(preparation.query, "作者前言…作者后记");
        assert!(
            preparation
                .terms
                .iter()
                .chain(&preparation.gaps)
                .all(|value| !value.contains(&quote))
        );
    };
    let current = core::read_work(&db, first.work_id).await.unwrap();
    safe(&current.revision.document);
    for revision in core::revisions(&db, first.work_id).await.unwrap() {
        safe(&revision.document);
    }
    safe(
        &core::create_work(&db, &create)
            .await
            .unwrap()
            .revision
            .document,
    );
    safe(
        &core::save(&db, first.work_id, &command)
            .await
            .unwrap()
            .revision
            .document,
    );
    for request in [create.request_id, command.request_id] {
        let receipt = core::read_request(&db, request).await.unwrap();
        let document: WorkDocument =
            serde_json::from_value(receipt["receipt"]["revision"]["document"].clone()).unwrap();
        safe(&document);
    }
    let mut forbidden = current.revision.document.clone();
    forbidden
        .material_preparation
        .as_mut()
        .unwrap()
        .gaps
        .push(quote.clone());
    assert!(matches!(
        core::save(
            &db,
            first.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: second.revision.id,
                document: forbidden
            }
        )
        .await,
        Err(core::WorkbenchError::Source)
    ));
    let mut independent = current.revision.document;
    independent.material_preparation.as_mut().unwrap().query = "作者独立检索意图".into();
    let independent = core::save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: second.revision.id,
            document: independent,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        independent
            .revision
            .document
            .material_preparation
            .unwrap()
            .query,
        "作者独立检索意图"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn excluded_thirteenth_parent_cannot_consume_selected_parent_comment_window() {
    use sources::materials::*;
    let db = fixture::db("creation_parent_window").await;
    let mut notes = Vec::new();
    for i in 0..13 {
        let external = format!("parent-window-{i}");
        let title = if i == 12 {
            "另一本相关材料".to_owned()
        } else {
            format!("作业启动场景{i}")
        };
        let package=dossier::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":external}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"title":title,"bodyText":"写作业一直拖延，启动需要第一步。","authorId":"synthetic-parent-window"}})).await;
        let work: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(&external)
        .fetch_one(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(work).bind(DOMAIN).bind(package).execute(db.pool()).await.unwrap();
        notes.push((work, external));
    }
    let request = PrepareRequest {
        domain_ref: DOMAIN,
        title: "作业启动".into(),
        angle: "第一步".into(),
        outline: vec![],
        topic_ref: None,
        terms: None,
    };
    for (_, external) in &notes[..12] {
        let id = format!("selected-{external}");
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":external}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"commentId":id,"noteId":external,"text":"写作业一直磨蹭，启动很难。"}})).await;
    }
    let selected_refs: Vec<_> = notes[..12].iter().map(|(work, _)| *work).collect();
    let highest:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE content_public_ref=ANY($1) ORDER BY material_ref DESC LIMIT 1").bind(&selected_refs).fetch_one(db.pool()).await.unwrap();
    // Construct an explicit window-ordering counterexample using admitted immutable samples.
    // Restriction facts select the adversarial inventory; no source row/UUID is mutated.
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic adversarial inventory' FROM linggan_material_comment_current WHERE content_public_ref=ANY($1) AND material_ref<>$2").bind(&selected_refs).bind(highest).execute(db.pool()).await.unwrap();
    let (excluded, external) = &notes[12];
    let mut batch = 0;
    let kept: Vec<Uuid> = loop {
        let records=(0..120).map(|i|json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"commentId":format!("crowding-{batch}-{i}"),"noteId":external,"text":"写作业启动一直磨蹭。"}})).collect();
        dossier::fixture::submit_custom_package(&db,"xhs",&["comments"],json!({"contentExternalId":external}),"comments","xhs",json!({"target":{"contentExternalId":external},"layers":[dossier::fixture::coverage_layer("comments",120)]}),records).await;
        let below:Vec<Uuid>=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE content_public_ref=$1 AND material_ref<$2 ORDER BY material_ref LIMIT 120").bind(excluded).bind(highest).fetch_all(db.pool()).await.unwrap();
        if below.len() == 120 {
            break below;
        }
        batch += 1;
        assert!(
            batch < 32,
            "bounded synthetic ordering fixture budget exceeded"
        );
    };
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) SELECT content_public_ref,comment_external_id,'synthetic adversarial inventory' FROM linggan_material_comment_current WHERE content_public_ref=$1 AND NOT(material_ref=ANY($2))").bind(excluded).bind(&kept).execute(db.pool()).await.unwrap();
    let legacy_window:Vec<Uuid>=sqlx::query_scalar("SELECT c.content_public_ref FROM linggan_material_comment_current c WHERE NOT EXISTS(SELECT 1 FROM linggan_material_comment_restriction r WHERE r.content_public_ref=c.content_public_ref AND r.comment_external_id=c.comment_external_id) ORDER BY c.material_ref LIMIT 120").fetch_all(db.pool()).await.unwrap();
    assert_eq!(legacy_window.len(), 120);
    assert!(
        legacy_window.iter().all(|parent| parent == excluded),
        "the old global window would discard every selected parent's voice"
    );
    let result = prepare(&db, &request).await.unwrap();
    assert_eq!(result["state"], "ready");
    let documents = result["documents"].as_array().unwrap();
    assert_eq!(
        documents
            .iter()
            .filter(|d| d["sourceKind"] == "note")
            .count(),
        12
    );
    let voices: Vec<_> = documents
        .iter()
        .filter(|d| d["sourceKind"] == "comment")
        .collect();
    assert_eq!(voices.len(), 1);
    assert_eq!(voices[0]["sourceRef"], json!(highest));
    assert!(documents.iter().all(|d| d["workRef"] != json!(excluded)));
    // Each selected parent now has an eligible voice. Final recommendation order stays fair.
    for (_, external) in &notes[..12] {
        dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":external}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":external},"payload":{"commentId":format!("fair-{external}"),"noteId":external,"text":"写作业总是磨蹭，启动需要陪伴。"}})).await;
    }
    let fair = prepare(&db, &request).await.unwrap();
    assert_eq!(
        fair["state"], "ready",
        "normal twelve-note/twelve-voice quota must not manufacture missing material"
    );
    let parents: std::collections::HashSet<_> = fair["documents"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["sourceKind"] == "comment")
        .map(|d| d["workRef"].as_str().unwrap())
        .collect();
    assert_eq!(
        parents.len(),
        12,
        "final twelve voices must not be consumed by one prolific parent"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn restricted_short_quote_at_metadata_limits_keeps_independent_edits_saveable() {
    use linggan_intelligence::content_workbench as core;
    let db = fixture::db("creation_metadata_limits").await;
    let mut document = fixture::comment_doc(&db).await;
    let excerpt = &mut document.excerpts[0];
    let quote: String = excerpt
        .selected_text
        .as_ref()
        .unwrap()
        .chars()
        .take(3)
        .collect();
    excerpt.citation.end = 3;
    excerpt.citation.text_hash = sources::text_hash(&quote);
    excerpt.text_hash = excerpt.citation.text_hash.clone();
    excerpt.selected_text = Some(quote.clone());
    let parent = excerpt.citation.work_ref.unwrap();
    document.material_preparation = Some(MaterialPreparation {
        method_version: sources::materials::METHOD.into(),
        query: format!("{}{quote}", "问".repeat(497)),
        terms: vec![
            format!("{}{quote}", "词".repeat(37)),
            quote.clone(),
            "作业".into(),
            "独立检索词".into(),
        ],
        state: "ready".into(),
        gaps: vec![
            format!("{}{quote}", "缺".repeat(197)),
            "普通缺口说明".into(),
        ],
    });
    let first = fixture::create(&db, document).await;
    fixture::restrict_comment(&db, parent).await;
    let read = core::read_work(&db, first.work_id).await.unwrap();
    let preparation = read
        .revision
        .document
        .material_preparation
        .as_ref()
        .unwrap();
    assert_eq!(preparation.query.chars().count(), 498);
    assert_eq!(preparation.terms[0].chars().count(), 38);
    assert_eq!(preparation.gaps[0].chars().count(), 198);
    assert_eq!(
        &preparation.terms[1..],
        &["作业".to_owned(), "独立检索词".to_owned()]
    );
    assert_eq!(preparation.gaps[1], "普通缺口说明");
    assert!(!preparation.query.contains(&quote));
    let mut independently_edited = read.revision.document;
    independently_edited.paragraphs[0].text = "继续写作者自己的正文".into();
    let saved = core::save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: independently_edited,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.outcome, "accepted");
    assert_eq!(
        saved.revision.document.paragraphs[0].text,
        "继续写作者自己的正文"
    );
    for field in ["query", "term", "gap"] {
        let mut forged = saved.revision.document.clone();
        let preparation = forged.material_preparation.as_mut().unwrap();
        match field {
            "query" => preparation.query = "…".repeat(501),
            "term" => preparation.terms[0] = "…".repeat(41),
            _ => preparation.gaps[0] = "…".repeat(201),
        }
        assert!(
            matches!(
                core::save(
                    &db,
                    first.work_id,
                    &SaveRequest {
                        request_id: Uuid::new_v4(),
                        expected_revision: saved.revision.id,
                        document: forged
                    }
                )
                .await,
                Err(core::WorkbenchError::Invalid("material_preparation"))
            ),
            "client-supplied placeholders must not bypass {field} limits"
        );
    }
}

async fn assert_comment_batch_matches_singles(
    db: &linggan_storage_postgres::Database,
    refs: &[Uuid],
) -> std::collections::BTreeMap<Uuid, serde_json::Value> {
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await
        .unwrap();
    let batch = linggan_evidence::read_creation_comments_in(&mut tx, refs)
        .await
        .unwrap();
    for reference in refs {
        let single = linggan_evidence::read_creation_comment_in(&mut tx, *reference)
            .await
            .unwrap();
        assert_eq!(
            batch.get(reference).cloned(),
            single,
            "complete anonymous source/parent contract changed for {reference}"
        );
    }
    tx.commit().await.unwrap();
    batch
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn batched_comments_preserve_complete_single_reads_and_refresh_restrictions() {
    let db = fixture::db("creation_comment_batch").await;
    let document = fixture::comment_doc(&db).await;
    let work = document.excerpts[0].citation.work_ref.unwrap();
    for (kind, id, parent) in [
        ("comments", "batch-parent", None),
        ("replies", "batch-reply", Some("batch-parent")),
        ("replies", "batch-unknown", Some("missing-parent")),
    ] {
        let mut payload =
            json!({"commentId":id,"noteId":"dossier-note","text":format!("{id}原声😀\n完整文本")});
        if let Some(parent) = parent {
            payload["rootCommentId"] = json!(parent);
            payload["parentCommentId"] = json!(parent);
        }
        dossier::fixture::submit_package(&db,kind,json!({"contentExternalId":"dossier-note"}),json!({"kind":if parent.is_some(){"reply"}else{"comment"},"sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":payload})).await;
    }
    let identities: std::collections::BTreeMap<String, Uuid> = sqlx::query_as(
        "SELECT comment_external_id,material_ref FROM linggan_material_comment_current WHERE content_public_ref=$1",
    ).bind(work).fetch_all(db.pool()).await.unwrap().into_iter().collect();
    let mut refs:Vec<Uuid>=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE content_public_ref=$1 ORDER BY material_ref").bind(work).fetch_all(db.pool()).await.unwrap();
    let other_package = dossier::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"batch-other-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"batch-other-note"},"payload":{"title":"另一篇合成底稿","bodyText":"另一篇独立正文，不能串入第一篇。","authorId":"synthetic-other-author","likedCount":0}})).await;
    let other_work: Uuid = sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE content_external_id='batch-other-note'").fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)").bind(Uuid::new_v4()).bind(other_work).bind(dossier::DOMAIN).bind(other_package).execute(db.pool()).await.unwrap();
    dossier::fixture::submit_package(&db,"comments",json!({"contentExternalId":"batch-other-note"}),json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"batch-other-note"},"payload":{"commentId":"batch-other-comment","noteId":"batch-other-note","text":"另一篇的独立原声"}})).await;
    let other_comment: Uuid = sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE content_public_ref=$1 AND comment_external_id='batch-other-comment'").bind(other_work).fetch_one(db.pool()).await.unwrap();
    refs.push(other_comment);
    let missing = Uuid::new_v4();
    refs.push(missing);
    refs.push(refs[0]);
    let initial = assert_comment_batch_matches_singles(&db, &refs).await;
    let parent = identities["batch-parent"];
    let reply = identities["batch-reply"];
    let unknown = identities["batch-unknown"];
    assert!(!initial.contains_key(&missing));
    assert_eq!(
        initial.len(),
        identities.len() + 1,
        "duplicates must not invent extra identities"
    );
    assert_ne!(work, other_work);
    assert_eq!(
        initial[&other_comment]["parentWork"]["workRef"],
        json!(other_work)
    );
    assert_eq!(initial[&reply]["parentWork"]["workRef"], json!(work));
    let other_fragments = initial[&other_comment]["parentWork"]["fragments"]
        .as_array()
        .unwrap();
    assert!(
        other_fragments
            .iter()
            .any(|fragment| fragment["text"] == "另一篇独立正文，不能串入第一篇。")
    );
    assert!(!other_fragments.iter().any(|fragment| {
        fragment["text"]
            .as_str()
            .is_some_and(|text| text.contains("结尾不能丢失"))
    }));
    assert_eq!(initial[&parent]["parentComment"]["state"], "not_applicable");
    assert_eq!(initial[&reply]["parentComment"]["state"], "known");
    assert_eq!(initial[&reply]["parentComment"]["sourceRef"], json!(parent));
    assert_eq!(initial[&unknown]["parentComment"]["state"], "unknown");
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'batch-parent','synthetic parent restriction')").bind(work).execute(db.pool()).await.unwrap();
    let parent_restricted = assert_comment_batch_matches_singles(&db, &refs).await;
    assert_eq!(
        parent_restricted[&reply]["parentComment"],
        json!({"state":"restricted"})
    );
    assert_eq!(
        parent_restricted[&parent],
        json!({"sourceRef":parent,"qualification":"restricted"})
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'batch-reply','synthetic own restriction')").bind(work).execute(db.pool()).await.unwrap();
    let own_restricted = assert_comment_batch_matches_singles(&db, &refs).await;
    assert_eq!(
        own_restricted[&reply],
        json!({"sourceRef":reply,"qualification":"restricted"})
    );
}
