use crate::fixture::{submit_package, submit_package_at};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

const PROOF_DOMAIN_REF: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);

async fn attach_to_proof_domain(database: &Database, content_external_id: &str) {
    // These tests seed already accepted historical material directly through Package fixtures.
    // Give that synthetic legacy material an explicit Domain membership so the production
    // Comment Study source gate exercises the same Domain usage contract as current data.
    sqlx::query(
        "INSERT INTO linggan_material_domain_usage( \
            usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref \
         ) SELECT gen_random_uuid(),public_ref,$2,'primary','legacy_domain_migration',first_package_ref \
             FROM linggan_material_content \
            WHERE platform='xhs' AND content_external_id=$1 \
         ON CONFLICT (content_public_ref,domain_ref) \
             WHERE basis_kind='legacy_domain_migration' DO NOTHING",
    )
    .bind(content_external_id)
    .bind(PROOF_DOMAIN_REF)
    .execute(database.pool())
    .await
    .unwrap();
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
    attach_to_proof_domain(database, note).await;
    sqlx::query_scalar("SELECT material_ref FROM linggan_material_comment WHERE package_ref=$1")
        .bind(package)
        .fetch_one(database.pool())
        .await
        .unwrap()
}

pub async fn reply_with_author(
    database: &Database,
    note: &str,
    id: &str,
    parent_comment_id: &str,
    body: &str,
    author_id: Option<&str>,
    observed_at: &str,
) -> Uuid {
    let mut payload = json!({
        "commentId":id,
        "noteId":note,
        "rootCommentId":parent_comment_id,
        "parentCommentId":parent_comment_id,
        "text":body,
    });
    if let Some(author_id) = author_id {
        payload["authorId"] = json!(author_id);
    }
    let package = submit_package_at(
        database,
        "replies",
        json!({"contentExternalId":note}),
        json!({
            "kind":"reply",
            "sourceObject":{"platform":"xhs","type":"content","externalId":note},
            "payload":payload,
        }),
        observed_at,
    )
    .await;
    attach_to_proof_domain(database, note).await;
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
    attach_to_proof_domain(database, note).await;
}
