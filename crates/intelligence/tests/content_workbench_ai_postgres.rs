#![recursion_limit = "256"]
use linggan_intelligence::{
    content_workbench::*, content_workbench_ai as ai, model_secrets::SyntheticModelSecrets,
    pi_adapter::PiAdapter,
};
use serde_json::json;
use uuid::Uuid;
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
#[path = "support/ai_writing_fixture.rs"]
mod foundation_fixture;
use foundation_fixture::run_request;

struct OwnedFakeRoot(std::path::PathBuf);
impl AsRef<std::path::Path> for OwnedFakeRoot {
    fn as_ref(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for OwnedFakeRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn adapter() -> (PiAdapter, std::path::PathBuf, OwnedFakeRoot) {
    let parent = std::env::var_os("LINGGAN_CREATION_ASSET_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let root = parent.join(format!("creation-ai-fake-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let count = root.join("calls");
    let script = root.join("adapter.py");
    let source = format!(
        r#"import sys,json,time
from pathlib import Path
r=json.load(sys.stdin)
p=json.loads(r['prompt'])
counter=Path({})
with counter.open('a') as f: f.write('called\n')
if 'DELAY' in p['authorInstruction']: time.sleep(0.3)
a=p['action']
out={{'title':None,'paragraphs':[],'pages':[]}}
if a=='draft': out['paragraphs']=[{{'paragraphId':None,'text':'合成起草😀'}}]
if a=='rewrite_selection': out['paragraphs']=[{{'paragraphId':p['paragraphs'][0]['id'],'text':'合成改写😀'}}]
if a=='page_copy': out['pages']=[{{'pageId':p['pages'][0]['pageId'],'heading':'合成图页','body':'合成图页正文'}}]
if 'QUOTE' in p['authorInstruction']:
    raw=p['materials'][0]['text']
    if a=='draft':
        out['title']='作者标题'+raw+'标题收束'
        out['paragraphs']=[{{'paragraphId':None,'text':'作者前言😀'+raw+'作者中段'+raw+'作者结语'}}]
    if a=='page_copy': out['pages']=[{{'pageId':p['pages'][0]['pageId'],'heading':'作者图题'+raw+'图题收束','body':'作者图文😀'+raw+'图文收束'}}]
if 'BADPAGE' in p['authorInstruction']: out['pages'][0]['pageId']=None
text=json.dumps(out,ensure_ascii=False)
if 'INVALID' in p['authorInstruction']: text='{{"outcome":"no_signal"}}'
print(json.dumps({{'version':'linggan.pi.v1/0.85.1','ok':True,'text':text,'failureCode':None,'modelIds':None,'modelListOrigin':None,'usage':{{'inputTokens':12,'outputTokens':8,'costUsd':None}},'elapsedMs':300}},ensure_ascii=False))
"#,
        serde_json::to_string(&count.to_string_lossy()).unwrap()
    );
    std::fs::write(&script, source).unwrap();
    (
        PiAdapter::configured_with_test_command("/usr/bin/python3".into(), script),
        count,
        OwnedFakeRoot(root),
    )
}
fn request(
    work: &SaveReceipt,
    config: Uuid,
    action: ai::WritingAction,
    instruction: &str,
) -> ai::PreviewRequest {
    ai::PreviewRequest {
        selection: None,
        preview_ref: Uuid::new_v4(),
        work_ref: work.work_id,
        expected_revision: work.revision.id,
        action,
        config_ref: config,
        citations: vec![],
        paragraph_ids: vec![work.revision.document.paragraphs[0].id],
        instruction: instruction.into(),
    }
}
async fn prepare(
    db: &linggan_storage_postgres::Database,
    r: &ai::PreviewRequest,
) -> ai::ExecuteRequest {
    let p = ai::preview(db, r).await.unwrap();
    ai::ExecuteRequest {
        request_id: Uuid::new_v4(),
        preview_ref: r.preview_ref,
        expected_revision: r.expected_revision,
        confirmed_input_hash: p["inputHash"].as_str().unwrap().into(),
        budget_tokens: p["reservedTokens"].as_i64().unwrap(),
        confirmed: true,
    }
}
fn call_count(path: &std::path::Path) -> usize {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .count()
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
    let future = run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter);
    let cancel = async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        ai::cancel(&db, exec.request_id).await.unwrap();
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
    let future = run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter);
    let disable = async {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        sqlx::query("UPDATE linggan_model_connection SET enabled=false")
            .execute(db.pool())
            .await
            .unwrap();
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
    // Not selecting a material checkbox cannot authorize sending a paragraph that still
    // carries its exact source-use link after the source has changed.
    let document = fixture::cited_doc(&db).await;
    let canvas = document.canvas[0].id;
    let paragraph = document.paragraphs[0].id;
    let source_work = fixture::create(&db, document).await;
    let placed = place(
        &db,
        source_work.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: source_work.revision.id,
            canvas_id: canvas,
            targets: vec![UseTarget::Paragraph {
                paragraph_id: paragraph,
            }],
        },
    )
    .await
    .unwrap();
    let r = request(&placed, config, ai::WritingAction::RewriteSelection, "");
    let exec = prepare(&db, &r).await;
    ai::execute(&db, &exec).await.unwrap();
    let before = call_count(&count);
    fixture::dossier::fixture::submit_package_at(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成替代版本","bodyText":"新版原文","authorId":"synthetic-author"}}),"2026-08-29T10:00:00Z").await;
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&count), before);
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["state"], "failed");
    assert!(status["invocationRef"].is_null());
    assert!(status["usage"]["chargedTokens"].is_null());
    assert_eq!(status["usage"]["state"], "not_reserved");
    assert!(status["suggestion"].is_null());
    let research_count:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation i WHERE i.operation='analyze' AND (NOT EXISTS(SELECT 1 FROM linggan_creation_ai_request r WHERE r.invocation_ref=i.invocation_ref) OR i.result->>'purpose' IS DISTINCT FROM 'content_workbench')").fetch_one(db.pool()).await.unwrap();
    assert_eq!(research_count, 0);
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL source adoption proof with local fake transport"]
async fn preview_only_citation_adoption_preserves_exact_spans_and_revokes_every_public_projection()
{
    let db = fixture::db("creation_ai_quote").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let cited = fixture::comment_doc(&db).await;
    let citation = cited.excerpts[0].citation.clone();
    let raw = cited.excerpts[0].selected_text.clone().unwrap();
    let mut document = fixture::doc();
    document.account_ref = Some(fixture::account(&db).await);
    let first = fixture::create(&db, document).await;
    assert!(first.revision.document.excerpts.is_empty());
    let (adapter, _, root) = adapter();
    let mut preview = request(&first, config, ai::WritingAction::Draft, "QUOTE");
    preview.citations = vec![citation.clone()];
    // An overlapping exact slice has its own frozen Citation identity.
    let mut overlapping = citation.clone();
    overlapping.start = 2;
    overlapping.end = 9;
    let overlap: String = raw.chars().skip(2).take(7).collect();
    overlapping.text_hash = linggan_intelligence::content_workbench_sources::text_hash(&overlap);
    preview.citations.push(overlapping);
    let execute = prepare(&db, &preview).await;
    ai::execute(&db, &execute).await.unwrap();
    run_request(&db, execute.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let adoption = ai::AdoptRequest {
        request_id: Uuid::new_v4(),
        expected_revision: first.revision.id,
        paragraph_indices: None,
        page_indices: None,
        adopt_title: true,
    };
    let adopted: SaveReceipt =
        serde_json::from_value(ai::adopt(&db, execute.request_id, &adoption).await.unwrap())
            .unwrap();
    assert_eq!(adopted.revision.document.excerpts.len(), 2);
    assert!(
        adopted
            .revision
            .document
            .uses
            .iter()
            .all(|u| u.source_span.is_some())
    );
    let mut preview = request(&adopted, config, ai::WritingAction::PageCopy, "QUOTE");
    preview.citations = vec![citation.clone()];
    let execute_pages = prepare(&db, &preview).await;
    ai::execute(&db, &execute_pages).await.unwrap();
    run_request(
        &db,
        execute_pages.request_id,
        &SyntheticModelSecrets,
        &adapter,
    )
    .await
    .unwrap();
    let pages = ai::AdoptRequest {
        request_id: Uuid::new_v4(),
        expected_revision: adopted.revision.id,
        paragraph_indices: None,
        page_indices: None,
        adopt_title: false,
    };
    let adopted_pages: SaveReceipt = serde_json::from_value(
        ai::adopt(&db, execute_pages.request_id, &pages)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        adopted_pages
            .revision
            .document
            .uses
            .iter()
            .any(|u| matches!(u.target, UseTarget::PageTitle { .. }))
    );
    assert!(
        adopted_pages
            .revision
            .document
            .uses
            .iter()
            .any(|u| matches!(u.target, UseTarget::PageBlock { .. }))
    );
    let mut document = adopted_pages.revision.document.clone();
    document.paragraphs[0].text = format!("新增🙂{}", document.paragraphs[0].text);
    // Dropping client lineage cannot declassify retained raw quotations.
    document.uses.clear();
    document.excerpts.clear();
    document.canvas.clear();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: adopted_pages.revision.id,
            document,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision.document.excerpts.len(), 2);
    let unchanged = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: saved.revision.document.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        unchanged.outcome, "unchanged",
        "exact span IDs must remain stable"
    );
    let (export, _) = freeze_export(
        &db,
        first.work_id,
        &FreezeRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            renderer_version: "creation-renderer.v1".into(),
            font_version: "synthetic-fixture".into(),
        },
    )
    .await
    .unwrap();
    for page in &export.pages {
        upload_export_page(&db, export.id, page.page_id, &fixture::png(1080, 1440, 70))
            .await
            .unwrap();
    }
    finalize_export(
        &db,
        export.id,
        &FinalizeRequest {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert!(export_file(&db, export.id).await.is_ok());
    fixture::restrict_comment(&db, citation.work_ref.unwrap()).await;
    let read = read_work(&db, first.work_id).await.unwrap();
    assert_eq!(
        read.revision.document.title,
        "作者标题[来源当前不可使用]标题收束"
    );
    assert_eq!(
        read.revision.document.paragraphs[0].text,
        "新增🙂作者前言😀[来源当前不可使用]作者中段[来源当前不可使用]作者结语"
    );
    assert_eq!(
        read.revision.document.pages[0].title,
        "作者图题[来源当前不可使用]图题收束"
    );
    assert_eq!(
        read.revision.document.pages[0].blocks[0].text,
        "作者图文😀[来源当前不可使用]图文收束"
    );
    assert!(
        read.revision
            .document
            .excerpts
            .iter()
            .all(|e| e.selected_text.is_none())
    );
    let escaped_raw = serde_json::to_string(&raw).unwrap();
    let escaped_raw = &escaped_raw[1..escaped_raw.len() - 1];
    for revision in revisions(&db, first.work_id).await.unwrap() {
        let public = serde_json::to_string(&revision.document).unwrap();
        assert!(!public.contains(escaped_raw));
    }
    let list = list_works(&db, None).await.unwrap();
    assert_eq!(list[0].title, "作者标题[来源当前不可使用]标题收束");
    // Listing uses a temporary title-only projection; the immutable saved document keeps full lineage.
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM linggan_creation_revision WHERE revision_ref=$1")
            .bind(saved.revision.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored["title"], json!(format!("作者标题{raw}标题收束")));
    assert_eq!(
        stored["uses"].as_array().unwrap().len(),
        saved.revision.document.uses.len()
    );
    assert!(!serde_json::to_string(&list).unwrap().contains(escaped_raw));
    let replay = ai::adopt(&db, execute.request_id, &adoption).await.unwrap();
    assert_eq!(
        replay["revision"]["document"]["title"],
        "作者标题[来源当前不可使用]标题收束"
    );
    let receipt = read_request(&db, adoption.request_id).await.unwrap();
    assert_eq!(
        receipt["receipt"]["revision"]["document"]["title"],
        "作者标题[来源当前不可使用]标题收束"
    );
    assert!(matches!(
        export_file(&db, export.id).await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        export_page_file(&db, export.id, export.pages[0].page_id).await,
        Err(WorkbenchError::Source)
    ));
    let mut independent = read.revision.document;
    independent.title = "作者独立标题".into();
    independent.paragraphs[0].text = "作者独立正文".into();
    independent.pages[0].title = "作者独立图题".into();
    independent.pages[0].blocks[0].text = "作者独立图文".into();
    let own = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: independent,
        },
    )
    .await
    .unwrap();
    assert_eq!(own.revision.document.paragraphs[0].text, "作者独立正文");
    assert!(
        freeze_export(
            &db,
            first.work_id,
            &FreezeRequest {
                request_id: Uuid::new_v4(),
                expected_revision: own.revision.id,
                renderer_version: "creation-renderer.v1".into(),
                font_version: "synthetic-fixture".into()
            }
        )
        .await
        .is_ok()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL UTF scalar selection proof with local fake transport"]
async fn selected_unicode_range_rewrite_splices_only_the_confirmed_substring() {
    let db = fixture::db("creation_ai_selection").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let mut doc = fixture::doc();
    doc.paragraphs[0].text = "前😀待修改🙂后文".into();
    let first = fixture::create(&db, doc).await;
    let mut r = request(&first, config, ai::WritingAction::RewriteSelection, "");
    r.selection = Some(ai::RewriteSelection {
        paragraph_id: r.paragraph_ids[0],
        start: 2,
        end: 6,
        text_hash: linggan_intelligence::content_workbench_sources::text_hash("待修改🙂"),
    });
    let preview = ai::preview(&db, &r).await.unwrap();
    assert_eq!(
        preview["outboundInput"]["paragraphs"][0]["text"],
        "待修改🙂"
    );
    let mut bad = r.clone();
    bad.preview_ref = Uuid::new_v4();
    bad.selection.as_mut().unwrap().end = 7;
    assert!(matches!(
        ai::preview(&db, &bad).await,
        Err(ai::AiError::Invalid)
    ));
    let execute = prepare(&db, &r).await;
    let (adapter, _, root) = adapter();
    ai::execute(&db, &execute).await.unwrap();
    run_request(&db, execute.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let adopted = ai::adopt(
        &db,
        execute.request_id,
        &ai::AdoptRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            paragraph_indices: None,
            page_indices: None,
            adopt_title: false,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        adopted["revision"]["document"]["paragraphs"][0]["text"],
        "前😀合成改写😀后文"
    );
    assert_eq!(
        adopted["revision"]["document"]["paragraphs"][0]["id"],
        json!(first.revision.document.paragraphs[0].id)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL full-known-citation movement and authored-field security proof"]
async fn moved_known_quote_retains_identity_and_restrictions_across_every_authored_surface() {
    let db = fixture::db("creation_quote_move").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let mut cited = fixture::comment_doc(&db).await;
    let raw = cited.excerpts[0]
        .selected_text
        .as_ref()
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned();
    let hash = linggan_intelligence::content_workbench_sources::text_hash(&raw);
    cited.excerpts[0].citation.end = raw.chars().count();
    cited.excerpts[0].citation.text_hash = hash.clone();
    cited.excerpts[0].text_hash = hash;
    cited.excerpts[0].selected_text = Some(raw.clone());
    cited.account_ref = Some(fixture::account(&db).await);
    let citation = cited.excerpts[0].citation.clone();
    let first = fixture::create(&db, cited.clone()).await;
    let placed = place(
        &db,
        first.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            canvas_id: first.revision.document.canvas[0].id,
            targets: vec![UseTarget::Paragraph {
                paragraph_id: first.revision.document.paragraphs[0].id,
            }],
        },
    )
    .await
    .unwrap();
    let moved_id = Uuid::new_v4();
    let own_canvas = Uuid::new_v4();
    let mut document = placed.revision.document.clone();
    document.paragraphs[0].text.clear();
    document.paragraphs.push(Paragraph {
        id: moved_id,
        text: format!("段落前😀{raw}段落后"),
    });
    document.title = format!("标题前{raw}标题后");
    document.angle = format!("角度前{raw}角度后");
    document.outline = vec![format!("提纲前{raw}提纲后")];
    document.tags = vec![format!("标签前{raw}标签后")];
    document.excerpts.clear();
    document.uses.clear();
    document.canvas.clear();
    document.canvas.push(CanvasBlock {
        id: own_canvas,
        excerpt_id: None,
        group: CanvasGroup::Argument,
        note: format!("备注前{raw}备注后"),
        expression: format!("表达前{raw}表达后"),
        expression_revision: 1,
    });
    let moved_request = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: placed.revision.id,
        document,
    };
    let moved = save(&db, first.work_id, &moved_request).await.unwrap();
    let preview = request(&moved, config, ai::WritingAction::Draft, "");
    let execution = prepare(&db, &preview).await;
    // Independently isolate each non-paragraph actual outbound field: body/title do not contain raw.
    let mut only_angle = cited.clone();
    only_angle.angle = format!("独立角度前{raw}后");
    let angle = fixture::create(&db, only_angle).await;
    let mut only_outline = cited.clone();
    only_outline.outline = vec![format!("独立提纲前{raw}后")];
    let outline = fixture::create(&db, only_outline).await;
    let instruction = fixture::create(&db, cited).await;
    let (export, _) = freeze_export(
        &db,
        first.work_id,
        &FreezeRequest {
            request_id: Uuid::new_v4(),
            expected_revision: moved.revision.id,
            renderer_version: "creation-renderer.v1".into(),
            font_version: "synthetic-fixture".into(),
        },
    )
    .await
    .unwrap();
    for page in &export.pages {
        upload_export_page(&db, export.id, page.page_id, &fixture::png(1080, 1440, 80))
            .await
            .unwrap();
    }
    finalize_export(
        &db,
        export.id,
        &FinalizeRequest {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let invocations: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    fixture::restrict_comment(&db, citation.work_ref.unwrap()).await;
    let read = read_work(&db, first.work_id).await.unwrap();
    let doc = &read.revision.document;
    assert_eq!(doc.paragraphs[0].text, "");
    assert_eq!(doc.paragraphs[1].text, "段落前😀[来源当前不可使用]段落后");
    assert_eq!(doc.title, "标题前[来源当前不可使用]标题后");
    assert_eq!(doc.angle, "角度前[来源当前不可使用]角度后");
    assert_eq!(doc.outline, vec!["提纲前[来源当前不可使用]提纲后"]);
    assert_eq!(doc.tags, vec!["标签前[来源当前不可使用]标签后"]);
    let canvas = doc.canvas.iter().find(|c| c.id == own_canvas).unwrap();
    assert_eq!(canvas.note, "备注前[来源当前不可使用]备注后");
    assert_eq!(canvas.expression, "表达前[来源当前不可使用]表达后");
    assert_eq!(doc.excerpts.len(), 1);
    assert_eq!(doc.excerpts[0].citation, citation);
    assert!(doc.excerpts[0].selected_text.is_none());
    assert!(
        doc.uses.iter().any(
            |u| matches!(u.target,UseTarget::Paragraph {paragraph_id} if paragraph_id==moved_id)
        )
    );
    assert!(
        doc.uses
            .iter()
            .any(|u| matches!(u.target, UseTarget::Angle))
    );
    assert!(
        doc.uses
            .iter()
            .any(|u| matches!(u.target, UseTarget::Outline { index: 0 }))
    );
    assert!(
        doc.uses
            .iter()
            .any(|u| matches!(u.target, UseTarget::Tag { index: 0 }))
    );
    assert!(
        doc.uses
            .iter()
            .any(|u| matches!(u.target,UseTarget::CanvasNote {canvas_id} if canvas_id==own_canvas))
    );
    assert!(doc.uses.iter().any(
        |u| matches!(u.target,UseTarget::CanvasExpression {canvas_id} if canvas_id==own_canvas)
    ));
    let escaped = serde_json::to_string(&raw).unwrap();
    let escaped = &escaped[1..escaped.len() - 1];
    for history in revisions(&db, first.work_id).await.unwrap() {
        assert!(!serde_json::to_string(&history).unwrap().contains(escaped));
    }
    assert!(
        !read_request(&db, moved_request.request_id)
            .await
            .unwrap()
            .to_string()
            .contains(escaped)
    );
    assert!(
        !serde_json::to_string(&list_works(&db, None).await.unwrap())
            .unwrap()
            .contains(escaped)
    );
    assert!(matches!(
        ai::preview(&db, &request(&moved, config, ai::WritingAction::Draft, "")).await,
        Err(ai::AiError::Source)
    ));
    assert!(matches!(
        ai::preview(&db, &request(&angle, config, ai::WritingAction::Draft, "")).await,
        Err(ai::AiError::Source)
    ));
    assert!(matches!(
        ai::preview(
            &db,
            &request(&outline, config, ai::WritingAction::Draft, "")
        )
        .await,
        Err(ai::AiError::Source)
    ));
    assert!(matches!(
        ai::preview(
            &db,
            &request(&instruction, config, ai::WritingAction::Draft, &raw)
        )
        .await,
        Err(ai::AiError::Source)
    ));
    assert!(
        ai::preview(
            &db,
            &request(
                &instruction,
                config,
                ai::WritingAction::Draft,
                "作者独立指令"
            )
        )
        .await
        .is_ok(),
        "an unused restricted excerpt must not block sending independent author text"
    );
    assert!(matches!(
        ai::execute(&db, &execution).await,
        Err(ai::AiError::Source)
    ));
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        invocations, after,
        "restricted inputs must be rejected before reserving/dispatching a call"
    );
    assert!(matches!(
        export_file(&db, export.id).await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        freeze_export(
            &db,
            first.work_id,
            &FreezeRequest {
                request_id: Uuid::new_v4(),
                expected_revision: moved.revision.id,
                renderer_version: "creation-renderer.v1".into(),
                font_version: "synthetic-fixture".into()
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    // Clearing the source references again cannot turn the retained full quotes into own expression.
    let mut clear = moved.revision.document.clone();
    clear.excerpts.clear();
    clear.uses.clear();
    let retained = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: moved.revision.id,
            document: clear,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        retained.revision.document.paragraphs[1].text,
        "段落前😀[来源当前不可使用]段落后"
    );
    // A full quote cannot be newly inserted into an old expression target after restriction.
    let mut insert = retained.revision.document;
    insert.paragraphs[0].text = format!("新复制{raw}");
    assert!(matches!(
        save(
            &db,
            first.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: retained.revision.id,
                document: insert
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
}
