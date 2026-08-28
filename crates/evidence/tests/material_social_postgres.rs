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
