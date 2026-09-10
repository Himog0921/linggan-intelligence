use crate::fixture::{submit_package, submit_package_at};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;
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
