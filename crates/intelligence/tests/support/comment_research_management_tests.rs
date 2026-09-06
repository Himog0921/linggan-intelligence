use super::*;
use linggan_intelligence::comment_research_management::*;
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

fn revision(request: &SaveCommentAsset, expected_revision: i32) -> ReviseCommentAsset {
    ReviseCommentAsset {
        revision_ref: Uuid::new_v4(),
        asset_ref: request.asset_ref,
        expected_revision,
        reason: Some("复核后的收存理由".into()),
        collection_ref: None,
        withdrawn: false,
        change_reason: "合成整理说明".into(),
    }
}
async fn collection(db: &Database, name: &str) -> Uuid {
    let collection_ref = Uuid::new_v4();
    save_research_collection(
        db,
        &SaveResearchCollection {
            collection_ref,
            name: name.into(),
        },
    )
    .await
    .unwrap();
    collection_ref
}
async fn saved_asset(db: &Database, key: &str) -> SaveCommentAsset {
    let body = "合成资产：我👨‍👩‍👧每天都很累";
    let source_ref = comment(db, key, key, body, "2026-08-28T10:00:00Z").await;
    let request = asset(source_ref, body);
    save_comment_asset(db, &request).await.unwrap();
    request
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn asset_revisions_organize_existing_assets_and_serialize_concurrent_edits() {
    let db = proof_database("comment_asset_revisions").await;
    let original = saved_asset(&db, "asset-edit").await;
    let first = collection(&db, "合成集合 A").await;
    let second = collection(&db, "合成集合 B").await;
    let mut left = revision(&original, 0);
    left.collection_ref = Some(first);
    let mut right = revision(&original, 0);
    right.collection_ref = Some(first);
    let (a, b) = tokio::join!(
        revise_comment_asset(&db, &left),
        revise_comment_asset(&db, &right)
    );
    assert!(a.is_ok() ^ b.is_ok());
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(CommentResearchError::RevisionConflict)
    ));
    let winner = if a.is_ok() { left } else { right };
    assert_eq!(
        read_comment_assets(&db, Some(first), None).await.unwrap()["total"],
        1
    );
    assert_eq!(
        save_comment_asset(&db, &original).await.unwrap()["revision"],
        1
    );
    assert_eq!(
        read_comment_assets(&db, None, None).await.unwrap()["items"][0]["reason"],
        "复核后的收存理由"
    );
    let mut move_edit = revision(&original, 1);
    move_edit.collection_ref = Some(second);
    revise_comment_asset(&db, &move_edit).await.unwrap();
    assert_eq!(
        read_comment_assets(&db, Some(first), None).await.unwrap()["total"],
        0
    );
    assert_eq!(
        read_comment_assets(&db, Some(second), None).await.unwrap()["total"],
        1
    );
    let remove = revision(&original, 2);
    revise_comment_asset(&db, &remove).await.unwrap();
    assert_eq!(
        read_comment_assets(&db, Some(second), None).await.unwrap()["total"],
        0
    );
    assert_eq!(
        revise_comment_asset(&db, &winner).await.unwrap()["revision"],
        3
    );
    let history = read_comment_asset_history(&db, original.asset_ref)
        .await
        .unwrap();
    assert_eq!(history["items"].as_array().unwrap().len(), 4);
    assert_eq!(history["items"][3]["reason"], original.reason);
    let fixed:(Uuid,i32,i32,String)=sqlx::query_as("SELECT source_ref,start_char,end_char,source_sha256 FROM linggan_comment_asset WHERE asset_ref=$1")
        .bind(original.asset_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        fixed,
        (
            original.source_ref,
            original.start_char,
            original.end_char,
            original.source_sha256
        )
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn withdrawal_and_restricted_history_cannot_be_undone_by_old_requests() {
    let db = proof_database("comment_asset_withdrawal").await;
    let original = saved_asset(&db, "asset-withdraw").await;
    let edit = revision(&original, 0);
    revise_comment_asset(&db, &edit).await.unwrap();
    let mut changed = revision(&original, 0);
    changed.revision_ref = edit.revision_ref;
    changed.reason = Some("不同的请求内容".into());
    assert!(matches!(
        revise_comment_asset(&db, &changed).await,
        Err(CommentResearchError::IdempotencyConflict)
    ));
    let mut withdraw = revision(&original, 1);
    withdraw.withdrawn = true;
    withdraw.reason = None;
    revise_comment_asset(&db, &withdraw).await.unwrap();
    assert_eq!(
        read_comment_assets(&db, None, None).await.unwrap()["total"],
        0
    );
    assert_eq!(
        save_comment_asset(&db, &original).await.unwrap()["state"],
        "WITHDRAWN"
    );
    assert_eq!(
        revise_comment_asset(&db, &edit).await.unwrap()["state"],
        "WITHDRAWN"
    );
    assert!(matches!(
        revise_comment_asset(&db, &revision(&original, 2)).await,
        Err(CommentResearchError::RevisionConflict)
    ));
    restrict_comment_research_source(&db, original.source_ref, "合成限制")
        .await
        .unwrap();
    let history = read_comment_asset_history(&db, original.asset_ref)
        .await
        .unwrap();
    assert!(
        history["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["reason"].is_null() && h["changeReason"].is_null())
    );
    let restricted = saved_asset(&db, "asset-restricted").await;
    restrict_comment_research_source(&db, restricted.source_ref, "合成限制")
        .await
        .unwrap();
    assert!(matches!(
        revise_comment_asset(&db, &revision(&restricted, 0)).await,
        Err(CommentResearchError::SourceUnavailable)
    ));
    let mut allowed = revision(&restricted, 0);
    allowed.withdrawn = true;
    allowed.reason = None;
    revise_comment_asset(&db, &allowed).await.unwrap();
    assert_eq!(
        read_comment_assets(&db, None, None).await.unwrap()["total"],
        0
    );
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn saved_queries_support_condition_edits_deletion_and_safe_replays() {
    let db = proof_database("comment_query_revisions").await;
    let source = comment(
        &db,
        "query-work",
        "query-comment",
        "合成查询材料",
        "2026-08-28T10:00:00Z",
    )
    .await;
    let work_ref = read_comment_research_source(&db, source)
        .await
        .unwrap()
        .work_ref;
    let original = SaveResearchQuery {
        query_ref: Uuid::new_v4(),
        name: "旧名称".into(),
        text: "旧条件".into(),
        work_ref: None,
    };
    save_research_query(&db, &original).await.unwrap();
    let edit = ReviseResearchQuery {
        revision_ref: Uuid::new_v4(),
        query_ref: original.query_ref,
        expected_revision: 0,
        name: "新名称".into(),
        text: "合成查询".into(),
        work_ref: Some(work_ref),
        deleted: false,
    };
    let mut concurrent = ReviseResearchQuery {
        revision_ref: Uuid::new_v4(),
        query_ref: edit.query_ref,
        expected_revision: 0,
        name: edit.name.clone(),
        text: edit.text.clone(),
        work_ref: edit.work_ref,
        deleted: false,
    };
    concurrent.name = "另一个名称".into();
    let (a, b) = tokio::join!(
        revise_research_query(&db, &edit),
        revise_research_query(&db, &concurrent)
    );
    assert!(a.is_ok() ^ b.is_ok());
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(CommentResearchError::RevisionConflict)
    ));
    let winner = if a.is_ok() { edit } else { concurrent };
    save_research_query(&db, &original).await.unwrap();
    let current = read_research_queries(&db).await.unwrap();
    assert_eq!(current["items"][0]["name"], winner.name);
    assert_eq!(current["items"][0]["text"], "合成查询");
    assert_eq!(current["items"][0]["workRef"], work_ref.to_string());
    let deletion = ReviseResearchQuery {
        revision_ref: Uuid::new_v4(),
        query_ref: original.query_ref,
        expected_revision: 1,
        name: winner.name.clone(),
        text: String::new(),
        work_ref: None,
        deleted: true,
    };
    revise_research_query(&db, &deletion).await.unwrap();
    assert_eq!(
        save_research_query(&db, &original).await.unwrap()["state"],
        "DELETED"
    );
    assert_eq!(
        revise_research_query(&db, &winner).await.unwrap()["state"],
        "DELETED"
    );
    assert!(
        read_research_queries(&db).await.unwrap()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let mut conflict = winner;
    conflict.name = "篡改旧请求".into();
    assert!(matches!(
        revise_research_query(&db, &conflict).await,
        Err(CommentResearchError::IdempotencyConflict)
    ));
    conflict.revision_ref = Uuid::new_v4();
    conflict.expected_revision = 2;
    assert!(matches!(
        revise_research_query(&db, &conflict).await,
        Err(CommentResearchError::RevisionConflict)
    ));
    let revisions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_query_revision WHERE query_ref=$1",
    )
    .bind(original.query_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(revisions, 2);
}

struct DropProof(Arc<AtomicUsize>);
impl Drop for DropProof {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
struct NeverProvider(Arc<AtomicUsize>);
impl CommentModelPort for NeverProvider {
    fn analyze(
        &self,
        _input: &CommentAnalysisInput,
    ) -> impl std::future::Future<Output = Result<CommentAnalysisOutput, CommentAnalysisFailure>> + Send
    {
        let proof = DropProof(self.0.clone());
        async move {
            let _proof = proof;
            std::future::pending().await
        }
    }
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL proof harness"]
async fn never_returning_provider_times_out_with_failure_and_bounded_retries() {
    let db = proof_database("comment_analysis_timeout").await;
    let source = comment(
        &db,
        "timeout-work",
        "timeout-comment",
        "合成超时材料",
        "2026-08-28T10:00:00Z",
    )
    .await;
    sync_comment_analysis_work(&db, "synthetic-never.v1")
        .await
        .unwrap();
    let drops = Arc::new(AtomicUsize::new(0));
    let provider = NeverProvider(drops.clone());
    assert!(
        run_comment_analysis_once_with_timeout(
            &db,
            "synthetic-never.v1",
            &provider,
            Duration::from_secs(61)
        )
        .await
        .is_err()
    );
    let started = std::time::Instant::now();
    for attempt in 1..=3 {
        assert!(
            run_comment_analysis_once_with_timeout(
                &db,
                "synthetic-never.v1",
                &provider,
                Duration::from_millis(250)
            )
            .await
            .unwrap()
        );
        let row:(Uuid,String,String,i32,Option<serde_json::Value>)=sqlx::query_as("SELECT work_ref,state,failure_code,attempts,result FROM linggan_comment_analysis_work WHERE source_ref=$1")
            .bind(source).fetch_one(db.pool()).await.unwrap();
        assert_eq!(
            (row.1.as_str(), row.2.as_str(), row.3),
            ("failed", "provider_timeout", attempt)
        );
        assert!(row.4.is_none());
        if attempt < 3 {
            retry_comment_analysis(&db, row.0).await.unwrap();
        } else {
            assert!(matches!(
                retry_comment_analysis(&db, row.0).await,
                Err(CommentResearchError::RevisionConflict)
            ));
        }
    }
    assert_eq!(drops.load(Ordering::SeqCst), 3);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        !run_comment_analysis_once(&db, "synthetic-never.v1", &provider)
            .await
            .unwrap()
    );
}
