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

#[path = "content_workbench_ai_postgres/dispatch.rs"]
mod dispatch;

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
        r#"import sys,json,time,os
from pathlib import Path
r=json.load(sys.stdin)
p=json.loads(r['prompt'])
counter=Path({})
if 'PRE_SEND' in p['authorInstruction']:
    counter.with_suffix('.waiting').write_text('waiting')
    permit=counter.with_suffix('.permit');deadline=time.monotonic()+10
    while not permit.exists():
        if time.monotonic()>deadline: raise TimeoutError('owned test did not permit dispatch')
        time.sleep(0.01)
with counter.open('a') as f: f.write('called\n')
if os.environ.get('LINGGAN_DISPATCH_SIGNAL')=='1':
    sys.stderr.write('LGI_DISPATCH_STARTED\n');sys.stderr.flush()
if 'DELAY' in p['authorInstruction']:
    release=counter.with_suffix('.release')
    deadline=time.monotonic()+(55 if 'LONG' in p['authorInstruction'] else 10)
    while not release.exists():
        if time.monotonic()>deadline: raise TimeoutError('owned test did not release adapter')
        time.sleep(0.01)
    release.unlink()
a=p['action']
out={{'title':None,'paragraphs':[],'pages':[]}}
if a=='title_ideas': out={{'coverTitles':['先从一小步开始','根治拖延'],'noteTitles':['孩子开始困难时先缩小第一步']}}
if a=='draft': out['paragraphs']=[{{'paragraphId':None,'text':'合成起草😀'}}]
if a=='rewrite_selection': out['paragraphs']=[{{'paragraphId':p['paragraphs'][0]['id'],'text':'合成改写😀'}}]
if a=='page_copy': out['pages']=[{{'pageId':p['pages'][0]['pageId'],'heading':'合成图页','body':'合成图页正文'}}]
if 'QUOTE' in p['authorInstruction']:
    raw=p['materials'][0]['content']['text']
    if a=='draft':
        out['title']='作者标题'+raw+'标题收束'
        out['paragraphs']=[{{'paragraphId':None,'text':'作者前言😀'+raw+'作者中段'+raw+'作者结语'}}]
    if a=='rewrite_selection': out['paragraphs']=[{{'paragraphId':p['paragraphs'][0]['id'],'text':'经历改写😀'+raw}}]
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
    assert_eq!(read.revision.document.title, "[来源当前不可使用]");
    assert_eq!(
        read.revision.document.paragraphs[0].text,
        "[来源当前不可使用]"
    );
    assert_eq!(read.revision.document.pages[0].title, "[来源当前不可使用]");
    assert_eq!(
        read.revision.document.pages[0].blocks[0].text,
        "[来源当前不可使用]"
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
    assert_eq!(list[0].title, "[来源当前不可使用]");
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
        "[来源当前不可使用]"
    );
    let receipt = read_request(&db, adoption.request_id).await.unwrap();
    assert_eq!(
        receipt["receipt"]["revision"]["document"]["title"],
        "[来源当前不可使用]"
    );
    assert!(matches!(
        export_file(&db, export.id).await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        export_page_file(&db, export.id, export.pages[0].page_id).await,
        Err(WorkbenchError::Source)
    ));
    // Clear the dependent fields first. A later, independent edit can then be saved.
    let mut cleared = read.revision.document;
    cleared.title.clear();
    cleared.paragraphs[0].text.clear();
    cleared.pages[0].title.clear();
    cleared.pages[0].blocks[0].text.clear();
    let cleared = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: cleared,
        },
    )
    .await
    .unwrap();
    let mut independent = cleared.revision.document;
    independent.title = "作者独立标题".into();
    independent.paragraphs[0].text = "作者独立正文".into();
    independent.pages[0].title = "作者独立图题".into();
    independent.pages[0].blocks[0].text = "作者独立图文".into();
    let own = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: cleared.revision.id,
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

#[tokio::test]
#[ignore = "isolated PostgreSQL paraphrase lineage and withdrawal proof with local fake transport"]
async fn adopted_paraphrase_keeps_source_through_edits_and_later_generation() {
    let db = fixture::db("creation_ai_derived_source").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let cited = fixture::comment_doc(&db).await;
    let citation = cited.excerpts[0].citation.clone();
    let mut document = fixture::doc();
    document.account_ref = Some(fixture::account(&db).await);
    let first = fixture::create(&db, document).await;
    let (adapter, _, root) = adapter();
    let mut preview = request(&first, config, ai::WritingAction::Draft, "");
    preview.citations = vec![citation.clone()];
    let execute = prepare(&db, &preview).await;
    ai::execute(&db, &execute).await.unwrap();
    run_request(&db, execute.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let adopted: SaveReceipt = serde_json::from_value(
        ai::adopt(
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
        .unwrap(),
    )
    .unwrap();
    assert_eq!(adopted.revision.document.paragraphs[0].text, "合成起草😀");
    assert_eq!(adopted.revision.document.derived_sources.len(), 1);
    let mut edited = adopted.revision.document.clone();
    edited.paragraphs[0].text.push_str("，补充说明");
    edited.derived_sources.clear(); // Client omission cannot erase the server's lineage.
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: adopted.revision.id,
            document: edited,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision.document.derived_sources.len(), 1);
    // Even an older client which drops lineage on Enter cannot sever the new tail.
    let mut split = saved.revision.document.clone();
    let full = split.paragraphs[0].text.clone();
    split.paragraphs[0].text = full.chars().take(3).collect();
    split.paragraphs.insert(
        1,
        Paragraph {
            id: Uuid::new_v4(),
            text: full.chars().skip(3).collect(),
        },
    );
    split.derived_sources.clear();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: split,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.revision.document.derived_sources.len(), 2);
    // A short generated word must not classify unrelated text merely containing it.
    let mut shortened = saved.revision.document.clone();
    shortened.paragraphs[0].text = "我".into();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: shortened,
        },
    )
    .await
    .unwrap();
    let mut independent = saved.revision.document.clone();
    independent.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "我今天上课".into(),
    });
    independent.derived_sources.clear();
    let saved = save(
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
    assert_eq!(saved.revision.document.derived_sources.len(), 2);
    let template = linggan_intelligence::creation_assets::create_structural_template(
        &db,
        &linggan_intelligence::creation_assets::CreateTemplate {
            request_id: Uuid::new_v4(),
            work_id: first.work_id,
            expected_revision: saved.revision.id,
            steps: vec![linggan_intelligence::creation_assets::StructuralStep::Opening],
        },
    )
    .await
    .unwrap()
    .template;
    // A later action inherits the source even when no quote is present and no citation is reselected.
    let next_preview = request(&saved, config, ai::WritingAction::Draft, "");
    let next_execute = prepare(&db, &next_preview).await;
    ai::execute(&db, &next_execute).await.unwrap();
    run_request(
        &db,
        next_execute.request_id,
        &SyntheticModelSecrets,
        &adapter,
    )
    .await
    .unwrap();
    fixture::restrict_comment(&db, citation.work_ref.unwrap()).await;
    let projection = read_work(&db, first.work_id).await.unwrap();
    assert_eq!(
        projection.revision.document.paragraphs[0].text,
        "[来源当前不可使用]"
    );
    assert_eq!(
        projection.revision.document.paragraphs[1].text,
        "[来源当前不可使用]"
    );
    assert_eq!(
        projection.revision.document.paragraphs[2].text,
        "我今天上课"
    );
    let listed = linggan_intelligence::creation_assets::list_structural_templates(
        &db,
        fixture::dossier::DOMAIN,
    )
    .await
    .unwrap();
    assert!(
        !listed
            .iter()
            .find(|item| item.code == template.code)
            .unwrap()
            .source_available
    );
    let checks = linggan_intelligence::creation_assets::check_saved_drafts(
        &db,
        &linggan_intelligence::creation_assets::SavedDraftCheckRequest {
            domain_ref: fixture::dossier::DOMAIN,
            items: vec![linggan_intelligence::creation_assets::SavedDraftCheckItem {
                work_id: first.work_id,
                revision_id: saved.revision.id,
            }],
        },
    )
    .await
    .unwrap();
    assert!(checks.is_empty());
    assert!(matches!(
        ai::preview(&db, &request(&saved, config, ai::WritingAction::Draft, "")).await,
        Err(ai::AiError::Source)
    ));
    assert!(matches!(
        ai::adopt(
            &db,
            next_execute.request_id,
            &ai::AdoptRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                paragraph_indices: None,
                page_indices: None,
                adopt_title: false
            }
        )
        .await,
        Err(ai::AiError::Source)
    ));
    assert!(matches!(
        freeze_export(
            &db,
            first.work_id,
            &FreezeRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                renderer_version: "creation-renderer.v1".into(),
                font_version: "synthetic-fixture".into()
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    // Clearing a target and reintroducing its exact old content cannot sever lineage.
    let removed = saved.revision.document.paragraphs[0].clone();
    let mut cleared_doc = projection.revision.document.clone();
    cleared_doc.paragraphs[0].text.clear();
    let cleared = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: cleared_doc,
        },
    )
    .await
    .unwrap();
    let mut reintroduced = cleared.revision.document.clone();
    reintroduced.paragraphs[0].text = removed.text.clone();
    reintroduced.derived_sources.clear();
    assert!(matches!(
        save(
            &db,
            first.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: cleared.revision.id,
                document: reintroduced
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    // Restoring a previously deleted target cannot lose its historical lineage.
    let mut deletion = cleared.revision.document.clone();
    deletion.paragraphs.remove(0);
    let deletion = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: cleared.revision.id,
            document: deletion,
        },
    )
    .await
    .unwrap();
    let mut restored = deletion.revision.document.clone();
    restored.paragraphs.insert(0, removed);
    restored.derived_sources.clear();
    assert!(matches!(
        save(
            &db,
            first.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: deletion.revision.id,
                document: restored
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL title task and fake model; no external egress"]
async fn title_skill_dispatch_adoption_budget_and_source_restriction_are_bound() {
    use linggan_intelligence::ai_foundation::{
        TaskKey,
        control::BudgetPolicy,
        control_store::ControlScope,
        provider_store::MaterialClass,
        task_config::{ModelSelection, SaveTaskConfig, save_task_config},
    };
    let db = fixture::db("creation_title_dispatch").await;
    let config = foundation_fixture::declare(&db, "http://127.0.0.1:1").await;
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,result,finished_at) VALUES($1,$2,$3,'probe',repeat('0',64),'succeeded',0,0,0,0,'{\"modelCallable\":true}',scope_001_now())")
        .bind(Uuid::new_v4()).bind(config.version).bind(config.model).execute(db.pool()).await.unwrap();
    let skill: Uuid = sqlx::query_scalar(
        "SELECT current_version_ref FROM linggan_skill_binding WHERE task_key='creation_title'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let title = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreationTitle,
            expected_revision: 0,
            model: Some(ModelSelection {
                model_ref: config.model,
                capability_ref: config.capability,
                limits: config.limits.clone(),
            }),
            skill_version_ref: Some(skill),
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::CreationTitle),
    ] {
        foundation_fixture::enable(&db, scope).await;
    }
    foundation_fixture::allow(
        &db,
        config.endpoint,
        TaskKey::CreationTitle,
        MaterialClass::SelectedWritingMaterial,
    )
    .await;
    let cited = fixture::comment_doc(&db).await;
    let citation = cited.excerpts[0].citation.clone();
    let work = fixture::create(&db, fixture::doc()).await;
    let (adapter, calls, _root) = adapter();
    let mut preview = request(
        &work,
        title.model_config_ref.unwrap(),
        ai::WritingAction::TitleIdeas,
        "",
    );
    preview.citations = vec![citation.clone()];
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let status = ai::status(&db, command.request_id).await.unwrap();
    assert_eq!(status["state"], "succeeded");
    assert_eq!(status["suggestion"]["coverTitles"][0], "先从一小步开始");
    assert_eq!(status["titleWarnings"]["cover"], json!([false, true]));
    let invocation: Uuid = sqlx::query_scalar(
        "SELECT invocation_ref FROM linggan_creation_ai_request WHERE request_ref=$1",
    )
    .bind(command.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let ledger:(String,bool)=sqlx::query_as("SELECT task_key,EXISTS(SELECT 1 FROM linggan_creation_ai_budget_reservation WHERE invocation_ref=i.invocation_ref) FROM linggan_model_invocation i WHERE invocation_ref=$1").bind(invocation).fetch_one(db.pool()).await.unwrap();
    assert_eq!(ledger, ("creation_title".into(), true));
    let adopted: SaveReceipt = serde_json::from_value(
        ai::adopt_title(
            &db,
            command.request_id,
            &ai::AdoptTitleRequest {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
                kind: ai::TitleKind::Note,
                index: 0,
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        adopted.revision.document.title,
        "孩子开始困难时先缩小第一步"
    );
    assert!(
        adopted
            .revision
            .document
            .derived_sources
            .iter()
            .any(|source| source.target == UseTarget::Title && source.citation == citation)
    );
    let both: SaveReceipt = serde_json::from_value(
        ai::adopt_title(
            &db,
            command.request_id,
            &ai::AdoptTitleRequest {
                request_id: Uuid::new_v4(),
                expected_revision: adopted.revision.id,
                kind: ai::TitleKind::Cover,
                index: 0,
            },
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        both.revision
            .document
            .pages
            .iter()
            .find(|page| page.kind == PageKind::Cover)
            .unwrap()
            .title,
        "先从一小步开始"
    );
    let mut edited = both.revision.document.clone();
    edited.paragraphs[0].text.push_str("后来补充的一句话。");
    let saved = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: both.revision.id,
            document: edited,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        ai::adopt_title(
            &db,
            command.request_id,
            &ai::AdoptTitleRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                kind: ai::TitleKind::Note,
                index: 1
            }
        )
        .await,
        Err(ai::AiError::Stale)
    ));
    fixture::restrict_comment(&db, citation.work_ref.unwrap()).await;
    assert_eq!(
        read_work(&db, work.work_id)
            .await
            .unwrap()
            .revision
            .document
            .title,
        "[来源当前不可使用]"
    );
    assert!(ai::status(&db, command.request_id).await.unwrap()["suggestion"].is_null());
    sqlx::query("UPDATE linggan_creation_ai_budget SET cap_pico_usd=0 WHERE singleton")
        .execute(db.pool())
        .await
        .unwrap();
    let other = fixture::create(&db, fixture::doc()).await;
    let blocked = prepare(
        &db,
        &request(
            &other,
            title.model_config_ref.unwrap(),
            ai::WritingAction::TitleIdeas,
            "",
        ),
    )
    .await;
    ai::execute(&db, &blocked).await.unwrap();
    let previous_calls = call_count(&calls);
    run_request(&db, blocked.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&calls), previous_calls);
    let blocked_status = ai::status(&db, blocked.request_id).await.unwrap();
    assert_eq!(
        blocked_status["failureCode"],
        "ai_creative_month_budget_exhausted"
    );
    assert!(blocked_status["invocationRef"].is_null());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL experience rewrite, erasure and local fake transport"]
async fn experience_rewrite_retains_erasable_lineage_and_replay_after_withdrawal() {
    use linggan_intelligence::creation_assets::{
        self as assets, ExperienceConfirm, ExperienceCreate, IdentityCommand, OrgRelation,
    };
    let db = fixture::db("creation_ai_experience").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let account = fixture::account(&db).await;
    let person = assets::put_identity(
        &db,
        account,
        &IdentityCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            org_relation: OrgRelation::Parent,
            bio_discloses_relation: Some(true),
            has_real_person: Some(true),
            backing_person_role: Some("妈妈".into()),
            can_provide_experience: None,
            person_change: None,
        },
    )
    .await
    .unwrap()
    .identity
    .person_ref
    .unwrap();
    let raw = "合成私密经历，先一起迈出第一步。";
    let experience = assets::create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "合成标题".into(),
            body: raw.into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    assets::confirm_experience(
        &db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = fixture::doc();
    doc.account_ref = Some(account);
    doc.paragraphs[0].text = raw.into();
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = fixture::create(&db, doc).await;
    let mut r = request(
        &work,
        config,
        ai::WritingAction::RewriteSelection,
        "合成私人补充指令",
    );
    r.selection = Some(ai::RewriteSelection {
        paragraph_id: r.paragraph_ids[0],
        start: 0,
        end: raw.chars().count(),
        text_hash: linggan_intelligence::content_workbench_sources::text_hash(raw),
    });
    let p = ai::preview(&db, &r).await.unwrap();
    assert_eq!(p["outboundInput"]["paragraphs"][0]["text"], raw);
    let public: String = sqlx::query_scalar(
        "SELECT input_manifest::text FROM linggan_creation_ai_preview WHERE preview_ref=$1",
    )
    .bind(r.preview_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!public.contains(raw));
    assert!(!public.contains("合成私人补充指令"));
    let exec = ai::ExecuteRequest {
        request_id: Uuid::new_v4(),
        preview_ref: r.preview_ref,
        expected_revision: r.expected_revision,
        confirmed_input_hash: p["inputHash"].as_str().unwrap().into(),
        budget_tokens: p["reservedTokens"].as_i64().unwrap(),
        confirmed: true,
    };
    ai::execute(&db, &exec).await.unwrap();
    let (adapter, calls, _root) = adapter();
    run_request(&db, exec.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&calls), 1);
    let status = ai::status(&db, exec.request_id).await.unwrap();
    assert_eq!(status["suggestion"]["paragraphs"][0]["text"], "合成改写😀");
    let public: String = sqlx::query_scalar(
        "SELECT suggestion::text FROM linggan_creation_ai_request WHERE request_ref=$1",
    )
    .bind(exec.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!public.contains("合成改写"));
    let adoption = ai::AdoptRequest {
        request_id: Uuid::new_v4(),
        expected_revision: work.revision.id,
        paragraph_indices: None,
        page_indices: Some(vec![]),
        adopt_title: false,
    };
    let accepted = ai::adopt(&db, exec.request_id, &adoption).await.unwrap();
    assert_eq!(
        accepted["revision"]["document"]["paragraphs"][0]["text"],
        "合成改写😀"
    );
    let stored: String = sqlx::query_scalar(
        "SELECT receipt::text FROM linggan_creation_ai_adoption_request WHERE request_ref=$1",
    )
    .bind(adoption.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!stored.contains(raw));
    assert!(!stored.contains("合成改写"));
    assets::withdraw_experience(&db, experience, Uuid::new_v4())
        .await
        .unwrap();
    assert!(ai::status(&db, exec.request_id).await.unwrap()["suggestion"].is_null());
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_ai_experience_material WHERE experience_ref=$1",
    )
    .bind(experience)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(count, 0);
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_fragment WHERE experience_ref=$1",
    )
    .bind(experience)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    let fingerprints:i64=sqlx::query_scalar("SELECT count(*) FROM linggan_creation_experience_command_link l JOIN linggan_creation_request r USING(request_ref) WHERE l.experience_ref=$1 AND r.command_sha256<>repeat('0',64)").bind(experience).fetch_one(db.pool()).await.unwrap();
    assert_eq!(fingerprints, 0);
    let replay = ai::adopt(&db, exec.request_id, &adoption).await.unwrap();
    let text = replay["revision"]["document"]["paragraphs"][0]["text"]
        .as_str()
        .unwrap();
    assert!(!text.contains(raw));
    assert!(!text.contains("合成改写"));
    let again = ai::execute(&db, &exec).await.unwrap();
    assert!(again["suggestion"].is_null());
}

async fn experience_fixture(db: &linggan_storage_postgres::Database) -> (SaveReceipt, Uuid) {
    use linggan_intelligence::creation_assets::{
        self as assets, ExperienceConfirm, ExperienceCreate, IdentityCommand, OrgRelation,
    };
    let account = fixture::account(db).await;
    let person = assets::put_identity(
        db,
        account,
        &IdentityCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            org_relation: OrgRelation::Parent,
            bio_discloses_relation: Some(true),
            has_real_person: Some(true),
            backing_person_role: Some("妈妈".into()),
            can_provide_experience: None,
            person_change: None,
        },
    )
    .await
    .unwrap()
    .identity
    .person_ref
    .unwrap();
    let experience = assets::create_experience(
        db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "合成".into(),
            body: "合成私密经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    assets::confirm_experience(
        db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = fixture::doc();
    doc.account_ref = Some(account);
    doc.paragraphs[0].text = "合成私密经历。".into();
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    (fixture::create(db, doc).await, experience)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL withdrawal gate with local fake transport"]
async fn experience_withdrawal_after_dispatch_erases_late_result_without_blocking_other_saves() {
    let db = fixture::db("creation_ai_experience_gate").await;
    let config = foundation_fixture::configure_fake_prerequisites_with_timeout(&db, 60).await;
    let (work, experience) = experience_fixture(&db).await;
    let unrelated = vec![
        fixture::create(&db, fixture::doc()).await,
        fixture::create(&db, fixture::doc()).await,
        fixture::create(&db, fixture::doc()).await,
    ];
    let preview = request(
        &work,
        config,
        ai::WritingAction::RewriteSelection,
        "DELAY_LONG",
    );
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, calls, _root) = adapter();
    let execute = run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter);
    let withdraw = async {
        for _ in 0..500 {
            if call_count(&calls) > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(call_count(&calls), 1);
        let save_one = |item: &SaveReceipt| {
            let db = &db;
            let work_id = item.work_id;
            let expected_revision = item.revision.id;
            let mut document = item.revision.document.clone();
            document.title.push_str("并发保存");
            async move {
                save(
                    db,
                    work_id,
                    &SaveRequest {
                        request_id: Uuid::new_v4(),
                        expected_revision,
                        document,
                    },
                )
                .await
            }
        };
        let saved = tokio::time::timeout(std::time::Duration::from_secs(4), async {
            tokio::join!(
                save_one(&unrelated[0]),
                save_one(&unrelated[1]),
                save_one(&unrelated[2])
            )
        })
        .await
        .expect("unrelated saves must not wait for a slow model response");
        assert!(saved.0.is_ok() && saved.1.is_ok() && saved.2.is_ok());
        tokio::time::timeout(
            std::time::Duration::from_secs(4),
            linggan_intelligence::creation_assets::withdraw_experience(
                &db,
                experience,
                Uuid::new_v4(),
            ),
        )
        .await
        .expect("withdrawal must not wait for an in-flight response")
        .unwrap();
        // Cross the 30-second lease boundary while renewal and independent saves remain live.
        tokio::time::sleep(std::time::Duration::from_secs(31)).await;
        std::fs::write(calls.with_extension("release"), b"release").unwrap();
    };
    let (finished, ()) = tokio::join!(execute, withdraw);
    finished.unwrap();
    assert_eq!(call_count(&calls), 1);
    let status = ai::status(&db, command.request_id).await.unwrap();
    assert!(status["suggestion"].is_null());
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_ai_experience_material WHERE experience_ref=$1",
    )
    .bind(experience)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL experience owner requalification"]
async fn experience_model_preview_and_result_are_unavailable_after_person_replacement() {
    use linggan_intelligence::creation_assets::{
        self as assets, IdentityCommand, OrgRelation, PersonChange,
    };
    let db = fixture::db("creation_ai_experience_replacement").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let (work, _experience) = experience_fixture(&db).await;
    let preview = request(&work, config, ai::WritingAction::RewriteSelection, "");
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, _calls, _root) = adapter();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert!(ai::status(&db, command.request_id).await.unwrap()["suggestion"].is_object());
    assets::put_identity(
        &db,
        work.revision.document.account_ref.unwrap(),
        &IdentityCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            org_relation: OrgRelation::Unknown,
            bio_discloses_relation: None,
            has_real_person: Some(true),
            backing_person_role: Some("另一位真人".into()),
            can_provide_experience: None,
            person_change: Some(PersonChange::Replacement),
        },
    )
    .await
    .unwrap();
    assert!(ai::status(&db, command.request_id).await.unwrap()["suggestion"].is_null());
    assert!(matches!(
        ai::adopt(
            &db,
            command.request_id,
            &ai::AdoptRequest {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
                paragraph_indices: None,
                page_indices: Some(vec![]),
                adopt_title: false
            }
        )
        .await,
        Err(ai::AiError::Source)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL queued experience withdrawal, no provider call"]
async fn experience_withdrawal_before_dispatch_prevents_model_call() {
    let db = fixture::db("creation_ai_experience_before_dispatch").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let (work, experience) = experience_fixture(&db).await;
    let preview = request(&work, config, ai::WritingAction::RewriteSelection, "");
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    linggan_intelligence::creation_assets::withdraw_experience(&db, experience, Uuid::new_v4())
        .await
        .unwrap();
    let (adapter, calls, _root) = adapter();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    assert_eq!(call_count(&calls), 0);
    let status = ai::status(&db, command.request_id).await.unwrap();
    assert!(status["suggestion"].is_null());
    assert_eq!(status["sourceQualification"], "unavailable");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL stale experience fingerprint erasure"]
async fn stale_save_with_old_experience_is_linked_even_if_current_revision_has_none() {
    let db = fixture::db("creation_ai_experience_stale").await;
    let (work, experience) = experience_fixture(&db).await;
    let mut cleared = work.revision.document.clone();
    cleared.experience_slots.clear();
    cleared.paragraphs[0].text.clear();
    let newer = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document: cleared,
        },
    )
    .await
    .unwrap();
    assert!(newer.revision.document.experience_slots.is_empty());
    let stale = SaveRequest {
        request_id: Uuid::new_v4(),
        expected_revision: work.revision.id,
        document: work.revision.document.clone(),
    };
    assert_eq!(
        save(&db, work.work_id, &stale).await.unwrap().outcome,
        "stale"
    );
    let linked:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_experience_command_link WHERE request_ref=$1 AND experience_ref=$2)").bind(stale.request_id).bind(experience).fetch_one(db.pool()).await.unwrap();
    assert!(linked);
    linggan_intelligence::creation_assets::withdraw_experience(&db, experience, Uuid::new_v4())
        .await
        .unwrap();
    let hash: String = sqlx::query_scalar(
        "SELECT command_sha256 FROM linggan_creation_request WHERE request_ref=$1",
    )
    .bind(stale.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(hash, "0".repeat(64));
    let after = SaveRequest {
        request_id: Uuid::new_v4(),
        ..stale
    };
    assert!(matches!(
        save(&db, work.work_id, &after).await,
        Err(WorkbenchError::Invalid(
            "experience_withdrawal_requires_reset"
        ))
    ));
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_request WHERE request_ref=$1)",
    )
    .bind(after.request_id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(!exists);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL mixed public source and experience erasure"]
async fn experience_rewrite_with_public_citation_keeps_source_uses_erasable() {
    let db = fixture::db("creation_ai_experience_public_source").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let (original, experience) = experience_fixture(&db).await;
    let source = fixture::comment_doc(&db).await;
    let excerpt = source.excerpts[0].clone();
    let mut document = original.revision.document.clone();
    document.excerpts.push(excerpt.clone());
    let work = save(
        &db,
        original.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: original.revision.id,
            document,
        },
    )
    .await
    .unwrap();
    let mut preview = request(&work, config, ai::WritingAction::RewriteSelection, "QUOTE");
    preview.citations.push(excerpt.citation.clone());
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, _calls, _root) = adapter();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let output = ai::status(&db, command.request_id).await.unwrap()["suggestion"]["paragraphs"][0]
        ["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(output.contains(excerpt.selected_text.as_ref().unwrap()));
    let adopted = ai::adopt(
        &db,
        command.request_id,
        &ai::AdoptRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            paragraph_indices: None,
            page_indices: Some(vec![]),
            adopt_title: false,
        },
    )
    .await
    .unwrap();
    let receipt: SaveReceipt = serde_json::from_value(adopted).unwrap();
    assert!(receipt.revision.document.uses.iter().any(|u|matches!(u.target,UseTarget::Paragraph { paragraph_id } if paragraph_id==work.revision.document.paragraphs[0].id)));
    let saved: String = sqlx::query_scalar(
        "SELECT document::text FROM linggan_creation_revision WHERE revision_ref=$1",
    )
    .bind(receipt.revision.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let private_hash = linggan_intelligence::content_workbench_sources::text_hash(&output);
    assert!(!saved.contains(&private_hash));
    assert!(!saved.contains("经历改写😀"));
    let protected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_use_fragment WHERE revision_ref=$1",
    )
    .bind(receipt.revision.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(protected > 0);
    let reread = read_work(&db, work.work_id).await.unwrap();
    assert!(reread.revision.document.uses.iter().any(|u|matches!(u.target,UseTarget::Paragraph { paragraph_id } if paragraph_id==work.revision.document.paragraphs[0].id)));
    linggan_intelligence::creation_assets::withdraw_experience(&db, experience, Uuid::new_v4())
        .await
        .unwrap();
    let protected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_use_fragment WHERE revision_ref=$1",
    )
    .bind(receipt.revision.id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(protected, 0);
    let reread = read_work(&db, work.work_id).await.unwrap();
    assert!(!reread.revision.document.uses.iter().any(|u|matches!(u.target,UseTarget::Paragraph { paragraph_id } if paragraph_id==work.revision.document.paragraphs[0].id)));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL multi-experience source order"]
async fn withdrawing_one_of_two_experiences_preserves_other_source_use_order() {
    use linggan_intelligence::creation_assets::{
        self as assets, ExperienceConfirm, ExperienceCreate,
    };
    let db = fixture::db("creation_ai_experience_use_order").await;
    let (original, first) = experience_fixture(&db).await;
    let account = original.revision.document.account_ref.unwrap();
    let person = assets::read_identity(&db, account)
        .await
        .unwrap()
        .unwrap()
        .person_ref
        .unwrap();
    let second = assets::create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "另一条合成经历".into(),
            body: "第二条合成经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    assets::confirm_experience(
        &db,
        second,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let excerpt = fixture::comment_doc(&db).await.excerpts[0].clone();
    let quote = excerpt.selected_text.as_ref().unwrap();
    let mut doc = original.revision.document.clone();
    let first_id = doc.paragraphs[0].id;
    doc.paragraphs[0].text = format!("第一条合成经历。{quote}");
    let ordinary_id = Uuid::new_v4();
    let second_id = Uuid::new_v4();
    doc.paragraphs.push(Paragraph {
        id: ordinary_id,
        text: format!("普通段落。{quote}"),
    });
    doc.paragraphs.push(Paragraph {
        id: second_id,
        text: format!("第二条合成经历。{quote}"),
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: second_id,
        experience_ref: second,
        experience_revision: 1,
    });
    doc.excerpts.push(excerpt);
    let saved = save(
        &db,
        original.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: original.revision.id,
            document: doc,
        },
    )
    .await
    .unwrap();
    let target_ids = |doc: &WorkDocument| {
        doc.uses
            .iter()
            .filter_map(|u| match u.target {
                UseTarget::Paragraph { paragraph_id }
                    if [first_id, ordinary_id, second_id].contains(&paragraph_id) =>
                {
                    Some(paragraph_id)
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        target_ids(&saved.revision.document),
        vec![first_id, ordinary_id, second_id]
    );
    assets::withdraw_experience(&db, first, Uuid::new_v4())
        .await
        .unwrap();
    let after = read_work(&db, original.work_id).await.unwrap();
    assert_eq!(
        target_ids(&after.revision.document),
        vec![ordinary_id, second_id]
    );
    assert!(
        after
            .revision
            .document
            .paragraphs
            .iter()
            .find(|p| p.id == second_id)
            .unwrap()
            .text
            .contains("第二条合成经历")
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL adoption lock order"]
async fn experience_adoption_waits_for_work_before_locking_experiences() {
    let db = fixture::db("creation_ai_adoption_lock_order").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let (work, _) = experience_fixture(&db).await;
    let preview = request(
        &work,
        config,
        ai::WritingAction::RewriteSelection,
        "合成改写",
    );
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, _calls, _root) = adapter();
    run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter)
        .await
        .unwrap();
    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!("creation:{}", work.work_id))
        .execute(&mut *hold)
        .await
        .unwrap();
    let other_db = db.clone();
    let request_id = command.request_id;
    let revision_id = work.revision.id;
    let adoption = tokio::spawn(async move {
        ai::adopt(
            &other_db,
            request_id,
            &ai::AdoptRequest {
                request_id: Uuid::new_v4(),
                expected_revision: revision_id,
                paragraph_indices: None,
                page_indices: Some(vec![]),
                adopt_title: false,
            },
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting > 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let mut probe = db.pool().begin().await.unwrap();
    let gate_free: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind("creation_assets:experience-gate")
            .fetch_one(&mut *probe)
            .await
            .unwrap();
    assert!(
        gate_free,
        "an adoption waiting on the work lock must not hold the experience gate"
    );
    probe.rollback().await.unwrap();
    hold.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), adoption)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL pre-transport withdrawal ordering"]
async fn withdrawal_cannot_finish_between_adapter_input_and_provider_dispatch() {
    let db = fixture::db("creation_ai_experience_pre_transport").await;
    let config = foundation_fixture::configure_fake_prerequisites(&db).await;
    let (work, experience) = experience_fixture(&db).await;
    let preview = request(
        &work,
        config,
        ai::WritingAction::RewriteSelection,
        "PRE_SEND",
    );
    let command = prepare(&db, &preview).await;
    ai::execute(&db, &command).await.unwrap();
    let (adapter, calls, _root) = adapter();
    let execute = run_request(&db, command.request_id, &SyntheticModelSecrets, &adapter);
    let withdraw = async {
        for _ in 0..500 {
            if calls.with_extension("waiting").exists() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(calls.with_extension("waiting").exists());
        assert_eq!(call_count(&calls), 0);
        let mut action = Box::pin(linggan_intelligence::creation_assets::withdraw_experience(
            &db,
            experience,
            Uuid::new_v4(),
        ));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(150), &mut action)
                .await
                .is_err(),
            "withdrawal cannot complete before transport starts"
        );
        std::fs::write(calls.with_extension("permit"), b"start").unwrap();
        action.await.unwrap();
    };
    let (finished, ()) = tokio::join!(execute, withdraw);
    finished.unwrap();
    assert_eq!(call_count(&calls), 1);
    assert!(ai::status(&db, command.request_id).await.unwrap()["suggestion"].is_null());
}
