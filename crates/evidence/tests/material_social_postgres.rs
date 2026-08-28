#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{coverage_layer, proof_database, submit_custom_package, submit_package};
use sqlx::Row;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn comments_replies_and_author_keep_stable_identity_relationships_and_versions() {
    let database = proof_database("material_social_lanes_red").await;
    submit_package(
        &database,
        "comments",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        serde_json::json!({
            "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"note-social-1"},
            "payload":{"commentId":"comment-root-1","noteId":"note-social-1","text":"评论可检索原声","authorId":"author-social-1"}
        }),
    ).await;
    submit_package(
        &database,
        "replies",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        reply_record("reply-1", "comment-root-1", Some("comment-root-1"), None),
    )
    .await;
    submit_package(
        &database,
        "author_profile",
        serde_json::json!({"authorExternalId":"author-social-1"}),
        serde_json::json!({
            "kind":"author_profile","sourceObject":{"platform":"xhs","type":"author","externalId":"author-social-1"},
            "payload":{"userId":"author-social-1","nickname":"版本化作者","fans":17}
        }),
    ).await;
    submit_package(
        &database,
        "replies",
        serde_json::json!({"contentExternalId":"note-social-1"}),
        serde_json::json!({
            "kind":"reply","sourceObject":{"platform":"xhs","type":"content","externalId":"note-social-1"},
            "payload":{"commentId":"reply-without-parent","noteId":"note-social-1","text":"不能猜父级"}
        }),
    ).await;

    let relation = sqlx::query("SELECT comment_external_id,root_comment_external_id,parent_comment_external_id FROM linggan_material_comment ORDER BY is_reply")
        .fetch_all(database.pool()).await.unwrap();
    assert_eq!(relation.len(), 2);
    assert_eq!(
        relation[1].get::<String, _>("root_comment_external_id"),
        "comment-root-1"
    );
    assert_eq!(
        relation[1]
            .get::<Option<String>, _>("parent_comment_external_id")
            .as_deref(),
        Some("comment-root-1")
    );
    let author_count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_author_profile WHERE author_external_id='author-social-1'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(author_count, 1);
    let disposition: (String,String) = sqlx::query_as("SELECT disposition,reason FROM linggan_runtime_record_disposition disposition JOIN linggan_runtime_capture_package package USING(package_ref) WHERE package.package_kind='replies' AND package.payload::text LIKE '%reply-without-parent%'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(
        disposition,
        (
            "quarantined".into(),
            "typed_reply_relationship_invalid".into()
        )
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn relationship_conflicts_and_same_package_duplicates_are_quarantined_per_record() {
    let database = proof_database("material_discussion_attacks").await;
    let target = serde_json::json!({"contentExternalId":"note-discussion-attack"});
    submit_custom_package(
        &database,
        "xhs",
        &["comments"],
        target.clone(),
        "comments",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("comments",3)]}),
        vec![
            comment_record("duplicate-comment"),
            comment_record("duplicate-comment"),
            comment_record("healthy-comment"),
        ],
    )
    .await;
    let target = serde_json::json!({"contentExternalId":"note-discussion-attack"});
    submit_custom_package(
        &database,
        "xhs",
        &["replies"],
        target.clone(),
        "replies",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("replies",4)]}),
        vec![
            reply_record("healthy-reply", "root", Some("parent"), None),
            reply_record(
                "both-parent-fields",
                "root",
                Some("parent"),
                Some("reply-to"),
            ),
            reply_record("self-root", "self-root", Some("parent"), None),
            reply_record("self-parent", "root", Some("self-parent"), None),
        ],
    )
    .await;
    let identities: Vec<String> = sqlx::query_scalar(
        "SELECT comment_external_id FROM linggan_material_comment ORDER BY comment_external_id",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(identities, vec!["healthy-comment", "healthy-reply"]);
    let duplicates: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='typed_comment_identity_duplicate'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(duplicates, 2);
    let conflicts: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='typed_reply_relationship_invalid'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(conflicts, 3);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn database_rejects_self_reply_and_invalid_parent_source_even_when_rust_is_bypassed() {
    let database = proof_database("material_reply_database_checks").await;
    let package_ref = submit_package(
        &database,
        "comments",
        serde_json::json!({"contentExternalId":"note-db-reply-check"}),
        serde_json::json!({
            "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"note-db-reply-check"},
            "payload":{"commentId":"root-db-check","noteId":"note-db-reply-check","text":"root"}
        }),
    )
    .await;
    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id='note-db-reply-check'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    for (ordinal, comment_id, root_id, parent_id, source_field) in [
        (
            99,
            "self-root",
            "self-root",
            "root-db-check",
            "parentCommentId",
        ),
        (
            100,
            "self-parent",
            "root-db-check",
            "self-parent",
            "parentCommentId",
        ),
        (
            101,
            "bad-source",
            "root-db-check",
            "root-db-check",
            "inventedField",
        ),
    ] {
        sqlx::query(
            "INSERT INTO linggan_runtime_record_disposition \
             (package_ref,record_ordinal,disposition,reason) \
             VALUES($1,$2,'accepted_for_library_content','direct_sql_reply_check')",
        )
        .bind(package_ref)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .unwrap();
        let result = sqlx::query(
            "INSERT INTO linggan_material_comment \
             (material_ref,content_public_ref,package_ref,record_ordinal,comment_external_id, \
              root_comment_external_id,parent_comment_external_id,parent_identity_source_field, \
              is_reply,body_state,observed_at) \
             VALUES($1,$2,$3,$4,$5,$6,$7,$8,true,'UNKNOWN','2026-08-28T10:00:00Z')",
        )
        .bind(uuid::Uuid::new_v4())
        .bind(content_ref)
        .bind(package_ref)
        .bind(ordinal)
        .bind(comment_id)
        .bind(root_id)
        .bind(parent_id)
        .bind(source_field)
        .execute(database.pool())
        .await;
        assert!(
            result.is_err(),
            "database accepted invalid reply {comment_id}"
        );
    }
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
         (package_ref,record_ordinal,disposition,reason) \
         VALUES($1,102,'accepted_for_library_content','direct_sql_comment_check')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let non_reply = sqlx::query(
        "INSERT INTO linggan_material_comment \
         (material_ref,content_public_ref,package_ref,record_ordinal,comment_external_id, \
          root_comment_external_id,parent_comment_external_id,parent_identity_source_field, \
          is_reply,body_state,observed_at) \
         VALUES($1,$2,$3,102,'root-with-source','root-with-source',NULL,'parentCommentId', \
                false,'UNKNOWN','2026-08-28T10:00:00Z')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(content_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await;
    assert!(
        non_reply.is_err(),
        "database accepted non-reply parent source"
    );
}

fn comment_record(comment_id: &str) -> serde_json::Value {
    serde_json::json!({
        "kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"note-discussion-attack"},
        "payload":{"commentId":comment_id,"noteId":"note-discussion-attack","text":"sensitive family material"}
    })
}

fn reply_record(
    comment_id: &str,
    root_id: &str,
    parent_id: Option<&str>,
    reply_to_id: Option<&str>,
) -> serde_json::Value {
    let mut payload = serde_json::json!({"commentId":comment_id,"noteId":if comment_id=="reply-1"{"note-social-1"}else{"note-discussion-attack"},"rootCommentId":root_id,"text":"sensitive reply material"});
    if let Some(parent_id) = parent_id {
        payload["parentCommentId"] = serde_json::json!(parent_id);
    }
    if let Some(reply_to_id) = reply_to_id {
        payload["replyToCommentId"] = serde_json::json!(reply_to_id);
    }
    let external_id = if comment_id == "reply-1" {
        "note-social-1"
    } else {
        "note-discussion-attack"
    };
    serde_json::json!({"kind":"reply","sourceObject":{"platform":"xhs","type":"content","externalId":external_id},"payload":payload})
}
