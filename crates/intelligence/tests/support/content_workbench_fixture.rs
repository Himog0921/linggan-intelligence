//! #425 isolated PostgreSQL + filesystem proof. Synthetic authorship, no external calls.
#[path = "../../../evidence/tests/support/research_dossier_fixture.rs"]
pub mod dossier;
use linggan_intelligence::content_workbench::*;
use linggan_intelligence::content_workbench_sources::{read_source_in, text_hash};
use linggan_storage_postgres::Database;
use uuid::Uuid;

pub async fn db(name: &str) -> Database {
    let db = dossier::fixture::proof_database(name).await;
    sqlx::raw_sql(include_str!(
        "../../../../database/migrations/0127_content_workbench.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../database/migrations/0128_content_workbench_ai.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../database/migrations/0140_ai_gen1_writing.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../database/migrations/0141_ai_gen1_legacy_settings.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    // These migrations really ran above. Record their exact sources for the dedicated
    // executor's startup check; the minimal material fixture intentionally omits them.
    for (id, source) in [
        (
            "0141_ai_gen1_legacy_settings",
            include_str!("../../../../database/migrations/0141_ai_gen1_legacy_settings.sql"),
        ),
        (
            "0127_content_workbench",
            include_str!("../../../../database/migrations/0127_content_workbench.sql"),
        ),
        (
            "0128_content_workbench_ai",
            include_str!("../../../../database/migrations/0128_content_workbench_ai.sql"),
        ),
        (
            "0140_ai_gen1_writing",
            include_str!("../../../../database/migrations/0140_ai_gen1_writing.sql"),
        ),
    ] {
        sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES($1,$2)")
            .bind(id).bind(text_hash(source)).execute(db.pool()).await.unwrap();
    }
    db
}
pub fn page(kind: PageKind) -> ImagePage {
    ImagePage {
        id: Uuid::new_v4(),
        kind,
        template_version: "creation-v1".into(),
        title: "合成封面".into(),
        subtitle: String::new(),
        blocks: vec![PageBlock {
            id: Uuid::new_v4(),
            text: String::new(),
        }],
        asset_id: None,
        crop: Crop {
            x: 0.,
            y: 0.,
            width: 1.,
            height: 1.,
        },
        style: PageStyle {
            alignment: Alignment::Start,
            palette: Palette::Cream,
            font_family: "system-cjk".into(),
            width: 1080,
            height: 1440,
        },
    }
}
pub fn doc() -> WorkDocument {
    WorkDocument {
        title: "合成🙂\n创作".into(),
        paragraphs: vec![Paragraph {
            id: Uuid::new_v4(),
            text: "手写正文，标点。".into(),
        }],
        pages: vec![page(PageKind::Cover), page(PageKind::Voice)],
        ..WorkDocument::default()
    }
}
pub async fn create(db: &Database, document: WorkDocument) -> SaveReceipt {
    create_work(
        db,
        &CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            document,
        },
    )
    .await
    .unwrap()
}
pub async fn account(db: &Database) -> Uuid {
    ops_seed(db).await.0
}
pub async fn cited_doc(db: &Database) -> WorkDocument {
    let work = dossier::work(db).await;
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let d = linggan_evidence::research_dossier::read_in(&mut tx, &[work], &as_of)
        .await
        .unwrap()
        .remove(&work)
        .unwrap();
    let f = d.fragments.iter().find(|f| f.field == "body").unwrap();
    let (source, _) = read_source_in(
        &mut tx,
        &SourceKind::Note,
        f.source_ref,
        Some(work),
        &SourceField::Body,
        &f.fragment_id,
        &as_of,
    )
    .await
    .unwrap();
    let end = source.text.chars().count();
    let start = end - 10;
    let selected: String = source.text.chars().skip(start).collect();
    let hash = text_hash(&selected);
    let id = Uuid::new_v4();
    let mut doc = doc();
    doc.excerpts.push(Excerpt {
        id,
        citation: Citation {
            block_ref: source.block_ref.clone(),
            as_of: source.as_of.clone(),
            source_kind: SourceKind::Note,
            source_ref: f.source_ref,
            work_ref: Some(work),
            field: SourceField::Body,
            source_version: source.source_version,
            start,
            end,
            text_hash: hash.clone(),
            location: None,
        },
        selected_text: Some(selected),
        text_hash: hash,
    });
    doc.canvas.push(CanvasBlock {
        id: Uuid::new_v4(),
        excerpt_id: Some(id),
        group: CanvasGroup::Argument,
        note: "自己的备注".into(),
        expression: String::new(),
        expression_revision: 1,
    });
    tx.commit().await.unwrap();
    doc
}
pub async fn comment_doc(db: &Database) -> WorkDocument {
    let work = dossier::work(db).await;
    dossier::fixture::submit_package(db,"comments",serde_json::json!({"contentExternalId":"dossier-note"}),serde_json::json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"commentId":"creation-quote","noteId":"dossier-note","text":"这是一条可精确定位的合成匿名原声🙂\n需要继续核验来源资格。"}})).await;
    let source_ref:Uuid=sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment_current WHERE comment_external_id='creation-quote'").fetch_one(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text").fetch_one(&mut *tx).await.unwrap();
    let (source, _) = read_source_in(
        &mut tx,
        &SourceKind::Comment,
        source_ref,
        Some(work),
        &SourceField::CommentBody,
        &format!("{source_ref}.comment_body"),
        &as_of,
    )
    .await
    .unwrap();
    let text = source.text;
    let hash = text_hash(&text);
    let id = Uuid::new_v4();
    let mut doc = doc();
    doc.excerpts.push(Excerpt {
        id,
        citation: Citation {
            block_ref: source.block_ref.clone(),
            as_of: source.as_of.clone(),
            source_kind: SourceKind::Comment,
            source_ref,
            work_ref: Some(work),
            field: SourceField::CommentBody,
            source_version: source.source_version,
            start: 0,
            end: text.chars().count(),
            text_hash: hash.clone(),
            location: None,
        },
        selected_text: Some(text),
        text_hash: hash,
    });
    doc.canvas.push(CanvasBlock {
        id: Uuid::new_v4(),
        excerpt_id: Some(id),
        group: CanvasGroup::Argument,
        note: String::new(),
        expression: String::new(),
        expression_revision: 1,
    });
    tx.commit().await.unwrap();
    doc
}
pub async fn restrict_comment(db: &Database, work: Uuid) {
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'creation-quote','synthetic proof restriction')").bind(work).execute(db.pool()).await.unwrap();
}
pub fn png(w: u32, h: u32, color: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, w, h);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(&vec![color; (w * h * 3) as usize])
            .unwrap();
    }
    bytes
}

#[path = "ops_export_fixture.rs"]
mod ops;
pub async fn ops_seed(db: &Database) -> (Uuid, Uuid) {
    let mut export = ops::first_export();
    for p in &mut export.publications {
        p["title"] = serde_json::json!("我方合成历史作品");
        p["copy"] = serde_json::json!("合成历史全文🙂\n可定位原文。");
    }
    let r = export.request("2026-10-12T22:43:00+08:00");
    let preview = linggan_intelligence::ops_ledger::preview_ops_import(db, &r)
        .await
        .unwrap();
    linggan_intelligence::ops_ledger::apply_ops_import(db, &r, &preview.preview_hash)
        .await
        .unwrap();
    let account:Uuid=sqlx::query_scalar("SELECT account_ref FROM linggan_ops_account WHERE ledger_state='in_ledger' AND domain_ref=$1 ORDER BY account_name LIMIT 1").bind(dossier::DOMAIN).fetch_one(db.pool()).await.unwrap();
    let publication:Uuid=sqlx::query_scalar("SELECT publication_ref FROM linggan_ops_publication WHERE body IS NOT NULL ORDER BY source_record_id LIMIT 1").fetch_one(db.pool()).await.unwrap();
    (account, publication)
}
