//! Current canonical source qualification; no legacy execution scaffolding.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier_fixture;
use dossier_fixture::fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use research_fixture::{comment_with_author, reply_with_author};
use uuid::Uuid;
fn domain() -> Uuid {
    dossier_fixture::DOMAIN
}
#[tokio::test]
#[ignore = "isolated PostgreSQL; exact source and restriction proof"]
async fn selected_comment_keeps_target_parent_and_dossier_separate() {
    use linggan_intelligence::comment_study_source::{StudySourceRole, selected_source_in};
    let db = dossier_fixture::fixture::proof_database("selected_comment_research").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::current_image_reading(&db).await;
    comment_with_author(
        &db,
        "dossier-note",
        "parent-a",
        "家长原话：如何安排练习？",
        Some("parent"),
        "2026-10-14T00:00:00Z",
    )
    .await;
    let target = reply_with_author(
        &db,
        "dossier-note",
        "child-a",
        "parent-a",
        "我的孩子需要每天有人提醒。",
        Some("reader"),
        "2026-10-14T00:01:00Z",
    )
    .await;
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await
        .unwrap();
    let selected = selected_source_in(
        &mut tx,
        domain(),
        work,
        target,
        StudySourceRole::Primary,
        &cutoff,
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(selected.raw_text, "我的孩子需要每天有人提醒。");
    assert_eq!(selected.comment_external_id, "child-a");
    assert_eq!(
        selected.parent.as_ref().unwrap()["commentText"],
        "家长原话：如何安排练习？"
    );
    assert_eq!(
        selected.source.context_manifest["dossierContract"],
        "research-dossier.v2"
    );
    assert!(selected.source.context_manifest["coverage"].is_object());
    assert!(
        !selected
            .source
            .context_manifest
            .to_string()
            .contains("家长原话")
    );
    tx.commit().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) VALUES($1,'child-a','synthetic restriction')").bind(work).execute(db.pool()).await.unwrap();
    let mut tx = db.pool().begin().await.unwrap();
    assert!(
        selected_source_in(
            &mut tx,
            domain(),
            work,
            target,
            StudySourceRole::Primary,
            &cutoff
        )
        .await
        .unwrap()
        .is_none()
    );
    tx.rollback().await.unwrap();
}
