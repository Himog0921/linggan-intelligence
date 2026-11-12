//! Called by the saved-Core PostgreSQL fixture, using genuine accepted source-bound Core.
use super::*;
use crate::ai_foundation::topic_research;

pub(crate) async fn assert_first_queue(
    db: &Database,
    domain: Uuid,
    work: Uuid,
    core: Uuid,
) -> Uuid {
    let initial = read(db, domain).await.unwrap();
    assert!(!initial.enabled);
    let mut cursor = None;
    assert_eq!(tick(db, &mut cursor).await.unwrap(), 0);
    let enabled = change(
        db,
        &Change {
            command_ref: Uuid::new_v4(),
            domain_ref: domain,
            expected_revision: initial.revision,
            enabled: true,
        },
    )
    .await
    .unwrap();
    assert_eq!(tick(db, &mut cursor).await.unwrap(), 1);
    let run:Uuid=sqlx::query_scalar("SELECT run_ref FROM linggan_topic_research_run WHERE domain_ref=$1 ORDER BY created_at DESC LIMIT 1").bind(domain).fetch_one(db.pool()).await.unwrap();
    let kinds:Vec<String>=sqlx::query_scalar("SELECT b.kind FROM linggan_topic_research_window w JOIN linggan_topic_research_stage s USING(window_ref) JOIN linggan_ai_batch_item i USING(item_ref) JOIN linggan_ai_batch b USING(batch_ref) WHERE w.run_ref=$1").bind(run).fetch_all(db.pool()).await.unwrap();
    assert_eq!(kinds.len(), 4);
    assert!(kinds.iter().all(|kind| kind == "automatic"));
    let frozen: serde_json::Value =
        sqlx::query_scalar("SELECT input_refs FROM linggan_topic_research_window WHERE run_ref=$1")
            .bind(run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(frozen["coverage"]["coreRef"], serde_json::json!(core));
    for _ in 0..3 {
        assert_eq!(tick(db, &mut cursor).await.unwrap(), 0);
    }
    let disabled = change(
        db,
        &Change {
            command_ref: Uuid::new_v4(),
            domain_ref: domain,
            expected_revision: enabled.revision,
            enabled: false,
        },
    )
    .await
    .unwrap();
    assert!(!disabled.enabled);
    assert_eq!(tick(db, &mut cursor).await.unwrap(), 0);
    assert!(matches!(
        topic_research::start_with_kind(
            db,
            &topic_research::Start {
                command_ref: Uuid::new_v4(),
                domain_ref: domain,
                work_refs: vec![work]
            },
            super::super::batch_store::BatchKind::Automatic,
            Some(core)
        )
        .await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    run
}

pub(crate) async fn assert_manual_not_requeued(db: &Database, domain: Uuid) {
    let initial = read(db, domain).await.unwrap();
    change(
        db,
        &Change {
            command_ref: Uuid::new_v4(),
            domain_ref: domain,
            expected_revision: initial.revision,
            enabled: true,
        },
    )
    .await
    .unwrap();
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_run WHERE domain_ref=$1")
            .bind(domain)
            .fetch_one(db.pool())
            .await
            .unwrap();
    let mut cursor = None;
    for _ in 0..3 {
        assert_eq!(tick(db, &mut cursor).await.unwrap(), 0);
    }
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_topic_research_run WHERE domain_ref=$1")
            .bind(domain)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(after, before);
}
