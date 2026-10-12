//! Frozen window sizes must survive the actual source-to-map projection.
use super::*;
use linggan_intelligence::topic_map::{self, TopicMapQuery};

async fn coverage(db: &Database, work: Uuid) -> Value {
    let snapshot = topic_map::read_topic_map(
        db,
        &TopicMapQuery {
            domain_ref: Some(D),
            reference_window_days: Some(0),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    snapshot
        .works
        .iter()
        .find(|item| item.work_ref == work)
        .unwrap()
        .research
        .as_ref()
        .unwrap()["core"]["coverage"]
        .clone()
}

#[tokio::test]
#[ignore = "isolated native PostgreSQL; synthetic adapter only"]
async fn two_nine_hundred_character_windows_project_one_of_two_before_both_are_accepted() {
    let (db, config, adapter) = setup("topic_core_frozen_window_coverage").await;
    let body = format!("{}{}", "甲".repeat(900), "乙".repeat(900));
    let w = work(&db, "frozen-window-coverage", &body).await;
    apply_research_command(&db, &configure(config, 100000, false))
        .await
        .unwrap();
    let receipt = apply_research_command(&db, &start(vec![w])).await.unwrap();
    let run: Uuid = receipt["runRef"].as_str().unwrap().parse().unwrap();
    let scopes: Vec<Value> = sqlx::query_scalar(
        "SELECT input_refs FROM linggan_topic_map_research_task WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(scopes.len(), 2);
    assert!(
        scopes
            .iter()
            .all(|scope| scope["coverage"]["maxChars"] == 900)
    );
    for _ in 0..16 {
        let finished: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_topic_map_research_task WHERE run_ref=$1 AND state='succeeded'",
        )
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap();
        if finished == 1 {
            let half = coverage(&db, w).await;
            assert_eq!(half["sourceChars"], 1800);
            assert_eq!(half["coveredChars"], 900);
            assert_eq!(half["completedWindows"], 1);
            assert_eq!(
                half["totalWindows"], 2,
                "must not use the legacy 3000 denominator"
            );
            assert_eq!(half["state"], "partial");
            finish_pending(&db, &adapter).await;
            let full = coverage(&db, w).await;
            assert_eq!(full["coveredChars"], 1800);
            assert_eq!(full["completedWindows"], 2);
            assert_eq!(full["totalWindows"], 2);
            assert_eq!(full["state"], "complete");
            return;
        }
        assert!(
            run_once(&db, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap()
        );
    }
    panic!("first window never completed in the bounded synthetic execution");
}
