//! Versioned deterministic cleaning against isolated PostgreSQL synthetic material only.
#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;
use linggan_intelligence::{
    comment_daily::clean_pending,
    comment_intelligence::{ResearchScope, read},
    comment_research::comment_source_hash,
};
use research_fixture::{comment, detail};
use serde_json::json;
use uuid::Uuid;

fn all(domain: &str) -> ResearchScope {
    ResearchScope {
        domain: Some(Uuid::parse_str(domain).unwrap()),
        from: Some("2020-01-01T00:00:00Z".into()),
        to: Some("2099-01-01T00:00:00Z".into()),
        ..Default::default()
    }
}
const OWN: &str = "00000000-0000-4000-8000-000000000001";
const CROSS: &str = "00000000-0000-4000-8000-000000000002";

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn current_research_excludes_dropped_but_preserves_versioned_source_and_analysis() {
    let db = fixture::proof_database("clean_v2_current").await;
    detail(&db, "clean-v2", "SYNTHETIC 清洗上下文").await;
    let noise = comment(&db, "clean-v2", "noise", "😂😂", "2026-08-28T10:00:00Z").await;
    let mixed = comment(
        &db,
        "clean-v2",
        "mixed",
        "@小明 😂 有效果吗？",
        "2026-08-28T10:00:00Z",
    )
    .await;
    comment(&db, "clean-v2", "short", "蹲", "2026-08-28T10:00:00Z").await;
    sqlx::query("INSERT INTO linggan_comment_clean(source_ref,cleaner_version,source_sha256,state,result) VALUES($1,'comment-clean.v1',$2,'low_information',$3)")
        .bind(noise).bind(comment_source_hash("😂😂"))
        .bind(json!({"text":"😂😂","offsets":[[0,1],[1,2]],"state":"low_information","reasons":["reaction_only"]}))
        .execute(db.pool()).await.unwrap();
    let result = json!({"sourceRef":noise,"sourceSha256":comment_source_hash("😂😂"),"spans":[],"contextRefs":{},"semantic":{"outcome":"interpretable","labels":[{"label":"need","evidence":[]}],"problems":[],"stances":[]}});
    sqlx::query("INSERT INTO linggan_comment_analysis_work(work_ref,source_ref,rule_version,model_version,state,result) VALUES($1,$2,'comment-research.v3','SYNTHETIC','succeeded',$3)")
        .bind(Uuid::new_v4()).bind(noise).bind(result).execute(db.pool()).await.unwrap();
    let before = read(&db, &all(OWN)).await.unwrap();
    assert_eq!(before["page"]["total"], 3);
    assert_eq!(before["page"]["items"][0]["cleanState"], "pending");
    assert_eq!(clean_pending(&db).await.unwrap(), 3);
    assert_eq!(clean_pending(&db).await.unwrap(), 0);
    for view in ["overview", "voices", "problems", "daily"] {
        let after = read(
            &db,
            &ResearchScope {
                view: Some(view.into()),
                ..all(OWN)
            },
        )
        .await
        .unwrap();
        assert_eq!(after["summary"]["comments"], 2, "{view}");
        assert_eq!(after["summary"]["analyzed"], 0, "{view}");
        assert!(
            !after["page"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["sourceRef"] == json!(noise))
        );
        let display = after["page"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["sourceRef"] == json!(mixed))
            .unwrap();
        assert_eq!(display["body"], "有效果吗?");
        assert_eq!(display["cleanerVersion"], "comment-clean.v2");
        assert_ne!(
            before["scope"]["resultRevision"],
            after["scope"]["resultRevision"]
        );
        assert_eq!(after["representatives"], json!([]));
        assert_eq!(after["problemCandidates"], json!([]));
    }
    let search = read(
        &db,
        &ResearchScope {
            text: Some("小明".into()),
            ..all(OWN)
        },
    )
    .await
    .unwrap();
    assert_eq!(search["summary"]["comments"], 0);
    let original: String =
        sqlx::query_scalar("SELECT body_text FROM linggan_material_comment WHERE material_ref=$1")
            .bind(mixed)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(original, "@小明 😂 有效果吗？");
    let legacy: String = sqlx::query_scalar("SELECT state FROM linggan_comment_clean WHERE source_ref=$1 AND cleaner_version='comment-clean.v1'").bind(noise).fetch_one(db.pool()).await.unwrap();
    assert_eq!(legacy, "low_information");
    let analysis_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_comment_analysis_work WHERE source_ref=$1",
    )
    .bind(noise)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(analysis_count, 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn cross_domain_cleaning_filters_noise_without_changing_model_permissions() {
    let db = fixture::proof_database("clean_v2_cross").await;
    let sample = Uuid::new_v4();
    let domain = Uuid::parse_str(CROSS).unwrap();
    sqlx::query("INSERT INTO cross_industry_sample(sample_ref,domain_ref,platform,content_external_id,title) VALUES($1,$2,'xhs','SYNTHETIC-clean','SYNTHETIC 清洗')")
        .bind(sample).bind(domain).execute(db.pool()).await.unwrap();
    for (external, body) in [("noise", "@作者 😂"), ("text", "@作者 求分享")] {
        sqlx::query("INSERT INTO cross_industry_comment(comment_ref,sample_ref,domain_ref,comment_external_id,body_text,body_state) VALUES($1,$2,$3,$4,$5,'KNOWN')")
            .bind(Uuid::new_v4()).bind(sample).bind(domain).bind(external).bind(body).execute(db.pool()).await.unwrap();
    }
    assert_eq!(clean_pending(&db).await.unwrap(), 2);
    let result = read(&db, &all(CROSS)).await.unwrap();
    assert_eq!(result["page"]["total"], 1);
    assert_eq!(result["page"]["items"][0]["body"], "求分享");
    assert_eq!(result["daily"]["schedule"]["enabled"], false);
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}
