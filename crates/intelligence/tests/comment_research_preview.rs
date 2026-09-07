#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
use fixture::proof_database;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{comment_analysis::*, comment_research::*};
use research_fixture::*;
use uuid::Uuid;

#[tokio::test]
#[ignore = "creates only the isolated synthetic browser preview schema"]
async fn prepare_comment_research_browser_preview() {
    let db = proof_database("comment_research_preview").await;
    for (id, sql) in [
        (
            "0001_scope_001_capture_evidence",
            include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
        ),
        (
            "0002_local_001_discovery",
            include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
        ),
    ] {
        sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES($1,$2) ON CONFLICT DO NOTHING")
            .bind(id).bind(comment_source_hash(sql)).execute(db.pool()).await.unwrap();
    }
    detail(&db, "preview-note-1", "合成材料 · 陪写作业的晚间安排").await;
    detail(&db, "preview-note-2", "合成材料 · 一周奖励表的使用记录").await;
    let mut first = None;
    for i in 0..24 {
        let body = match i % 4 {
            0 => "SYNTHETIC / NOT EVIDENCE：我下班后还要做饭，每天坐在旁边提醒真的做不到。",
            1 => {
                "SYNTHETIC / NOT EVIDENCE：奖励表刚开始有用，第三天就不想贴了，希望方法更容易坚持。"
            }
            2 => {
                "SYNTHETIC / NOT EVIDENCE：两个孩子的时间怎么安排？我试过分开写，还是需要一直提醒。"
            }
            _ => "SYNTHETIC / NOT EVIDENCE：谢谢记录，我准备试试这个晚间流程。",
        };
        let source_ref = comment(
            &db,
            if i % 2 == 0 {
                "preview-note-1"
            } else {
                "preview-note-2"
            },
            &format!("preview-{i}"),
            body,
            "2026-08-28T10:00:00Z",
        )
        .await;
        if first.is_none() {
            first = Some((source_ref, body));
        }
    }
    comment(
        &db,
        "preview-note-2",
        "preview-reaction",
        "😭😭😭",
        "2026-08-28T10:00:00Z",
    )
    .await;
    comment(
        &db,
        "preview-note-2",
        "preview-short",
        "我也是！！",
        "2026-08-28T10:00:00Z",
    )
    .await;
    comment(
        &db,
        "preview-note-2",
        "preview-long",
        &"SYNTHETIC / NOT EVIDENCE：很长的原声仍应保持两行预览，点击后研读。".repeat(100),
        "2026-08-28T10:00:00Z",
    )
    .await;
    for (id, likes) in [("preview-zero", 0_i64), ("preview-large", 9000000)] {
        fixture::submit_package(&db,"comments",serde_json::json!({"contentExternalId":"preview-note-1"}),serde_json::json!({"kind":"comment","sourceObject":{"platform":"xhs","type":"content","externalId":"preview-note-1"},"payload":{"commentId":id,"noteId":"preview-note-1","text":"SYNTHETIC / NOT EVIDENCE：这是点赞边界样本，保留已知零与大数。","likes":likes,"publishedAt":1788750000000_i64}})).await;
    }
    let (source_ref, body) = first.unwrap();
    save_comment_asset(&db, &asset(source_ref, body))
        .await
        .unwrap();
    correct_research_annotation(
        &db,
        &CorrectResearchAnnotation {
            annotation_ref: Uuid::new_v4(),
            source_ref,
            expected_revision: 0,
            facets: vec![problem("家长时间不足（合成）")],
            reason: "合成材料中的直接表达，非真实研究结论".into(),
        },
    )
    .await
    .unwrap();
    sync_comment_analysis_work(&db, UNCONFIGURED_MODEL)
        .await
        .unwrap();
    println!(
        "SYNTHETIC preview schema ready: comment_research_preview; 29 accepted comments, 1 manual asset, 1 manual problem label; no real provider calls"
    );
}
