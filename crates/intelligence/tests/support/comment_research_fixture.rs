use crate::fixture::{submit_package, submit_package_at};
use linggan_intelligence::comment_research::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
pub async fn comment(
    database: &Database,
    note: &str,
    id: &str,
    body: &str,
    observed_at: &str,
) -> Uuid {
    comment_with_author(
        database,
        note,
        id,
        body,
        Some("synthetic-hidden-author"),
        observed_at,
    )
    .await
}
pub async fn comment_with_author(
    database: &Database,
    note: &str,
    id: &str,
    body: &str,
    author_id: Option<&str>,
    observed_at: &str,
) -> Uuid {
    let mut payload = json!({
        "commentId":id,
        "noteId":note,
        "text":body,
    });
    if let Some(author_id) = author_id {
        payload["authorId"] = json!(author_id);
    }
    let package=submit_package_at(database,"comments",json!({"contentExternalId":note}),
        json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":note},
            "payload":payload}),observed_at).await;
    sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
        .bind(package)
        .fetch_one(database.pool())
        .await
        .unwrap()
}
pub async fn detail(database: &Database, note: &str, title: &str) {
    detail_with_author(database, note, title, None).await;
}
pub async fn detail_with_author(
    database: &Database,
    note: &str,
    title: &str,
    author_id: Option<&str>,
) {
    let mut payload = json!({
        "noteId":note,
        "title":title,
        "bodyText":"SYNTHETIC / NOT EVIDENCE · 作品上下文测试",
        "authorName":"合成作品作者",
    });
    if let Some(author_id) = author_id {
        payload["authorId"] = json!(author_id);
    }
    submit_package(database,"content_detail",json!({"contentExternalId":note}),json!({
        "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":note},
        "payload":payload
    })).await;
}
pub fn asset(source_ref: Uuid, body: &str) -> SaveCommentAsset {
    SaveCommentAsset {
        asset_ref: Uuid::new_v4(),
        source_ref,
        start_char: 0,
        end_char: body.chars().count() as i32,
        source_sha256: comment_source_hash(body),
        reason: "合成研究理由".into(),
        collection_ref: None,
    }
}
pub fn problem(label: &str) -> ResearchFacet {
    ResearchFacet {
        dimension: ResearchDimension::Problem,
        label: label.into(),
        basis: ResearchBasis::Explicit,
    }
}
