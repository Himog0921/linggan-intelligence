//! T1.3 creator-density integration proof.
use super::*;

async fn creator_counts(
    db: &Database,
    creator: &str,
    protocol: Option<i32>,
    version: i32,
) -> (i32, i32, bool, f64, f64) {
    sqlx::query_as(
        "SELECT k,n,sufficient,ci_low,ci_high FROM linggan_opportunity_creator_problem_density
         WHERE problem_ref=md5('metric-problem:1')::uuid AND creator_id=$1
           AND protocol_version IS NOT DISTINCT FROM $2 AND metric_def_version=$3",
    )
    .bind(creator)
    .bind(protocol)
    .bind(version)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

async fn creator_snapshot(
    db: &Database,
) -> Vec<(String, Uuid, i32, Option<i32>, i32, i32, bool, f64, f64)> {
    sqlx::query_as(
        "SELECT creator_id,problem_ref,metric_def_version,protocol_version,k,n,sufficient,ci_low,ci_high
         FROM linggan_opportunity_creator_problem_density
         ORDER BY problem_ref,metric_def_version,protocol_version NULLS FIRST,creator_id",
    )
    .fetch_all(db.pool())
    .await
    .unwrap()
}

async fn add_future_missing_creator_detail(db: &Database) {
    sqlx::raw_sql(r#"
INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
VALUES (md5('creator-package:c2:detail')::uuid,1,'accepted_for_library_content',
 'synthetic later detail','2026-11-10T02:00:00Z');
INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,
 observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,
 published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,
 published_at,published_at_source_field,published_at_source_kind,published_at_precision,
 published_at_parser_version,created_at)
VALUES (md5('creator-c2-later-detail')::uuid,md5('creator-work:c2')::uuid,
 md5('creator-package:c2:detail')::uuid,1,'2026-11-10T02:00:00Z','later','KNOWN',NULL,'UNKNOWN',
 NULL,'UNKNOWN',NULL,'UNKNOWN','seed','creator-C2-later',NULL,NULL,'unknown','unknown',NULL,
 '2026-11-10T02:00:00Z');
"#).execute(db.pool()).await.unwrap();
}

async fn add_creator_window_boundaries(db: &Database) {
    sqlx::raw_sql(r#"
INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
SELECT package_ref,1,'accepted_for_library_content','synthetic window boundary','2026-11-09T12:00:00Z'
FROM linggan_material_content_detail
WHERE content_public_ref IN (md5('creator-work:c1-v1')::uuid,md5('creator-work:c2-v1')::uuid);
INSERT INTO linggan_material_content_detail(material_ref,content_public_ref,package_ref,record_ordinal,
 observed_at,title,title_state,body_text,body_state,creator_display_name,creator_display_name_state,
 published_at_source_text,published_at_source_text_state,searchable_text,author_external_id,
 published_at,published_at_source_field,published_at_source_kind,published_at_precision,
 published_at_parser_version,created_at)
SELECT md5('creator-boundary:'||detail.material_ref::text)::uuid,detail.content_public_ref,detail.package_ref,1,
 '2026-11-09T12:00:00Z',detail.title,detail.title_state,detail.body_text,detail.body_state,
 detail.creator_display_name,detail.creator_display_name_state,detail.published_at_source_text,
 detail.published_at_source_text_state,detail.searchable_text,detail.author_external_id,
 CASE WHEN detail.content_public_ref=md5('creator-work:c1-v1')::uuid THEN '2026-08-12T00:00:00Z'::timestamptz
      ELSE '2026-11-10T00:00:00Z'::timestamptz END,
 detail.published_at_source_field,detail.published_at_source_kind,detail.published_at_precision,
 detail.published_at_parser_version,'2026-11-09T12:00:00Z'
FROM linggan_material_content_detail detail
WHERE detail.content_public_ref IN (md5('creator-work:c1-v1')::uuid,md5('creator-work:c2-v1')::uuid)
 AND detail.record_ordinal=0;
"#).execute(db.pool()).await.unwrap();
}

const AS_OF: &str = "2026-11-10T00:00:00Z";

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn canonical_playbook_dataset_matches_problem_and_creator_answers_together() {
    let db = database_with_standard_creators(true).await;
    let domain = Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap();
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    verify_standard_results(&db, domain, &digest, AS_OF).await;
    let total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_eligible_comments($1,$2::timestamptz,$3,$4)",
    )
    .bind(domain)
    .bind(AS_OF)
    .bind(opportunity_dataset::TZ)
    .bind(opportunity_dataset::BASIS)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(total, 5800);
    let c1 = creator_counts(&db, "creator-C1", None, 1).await;
    let c2 = creator_counts(&db, "creator-C2", None, 1).await;
    assert_eq!((c1.0, c1.1, c1.2), (4, 60, true));
    assert!((c1.3 - 0.026229).abs() < 1e-6 && (c1.4 - 0.159254).abs() < 1e-6);
    assert_eq!((c2.0, c2.1, c2.2), (3, 40, false));
    assert!((c2.3 - 0.025836).abs() < 1e-6 && (c2.4 - 0.198642).abs() < 1e-6);
}

struct Fixture {
    db: Database,
    domain: Uuid,
    digest: CommenterDigest,
    problem: Uuid,
    shadow: i32,
}

async fn fixture() -> Fixture {
    let db = database().await;
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    opportunity_creator_dataset::build(&db).await.unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::query("UPDATE linggan_opportunity_comment_mention SET excluded_reason='marketing' WHERE comment_external_id='creator-comment-c1-a-58'")
        .execute(db.pool()).await.unwrap();
    let problem = sqlx::query_scalar("SELECT md5('metric-problem:1')::uuid")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let shadow = sqlx::query_scalar(
        "SELECT shadow_version FROM linggan_opportunity_metric_shadow WHERE current_version=1",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    Fixture {
        db,
        domain: Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap(),
        digest,
        problem,
        shadow,
    }
}

async fn verify_standard_and_protocols(f: &Fixture) {
    let db = &f.db;
    let first = rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Full)
        .await
        .unwrap();
    assert!(first.creator_rows_written > 0);
    let i7 = first
        .invariants
        .as_ref()
        .unwrap()
        .checks
        .iter()
        .find(|c| c.id == "I7")
        .unwrap();
    assert!(i7.available);
    assert_eq!(i7.violations, 0);
    let c1 = creator_counts(db, "creator-C1", None, 1).await;
    let c2 = creator_counts(db, "creator-C2", None, 1).await;
    assert_eq!((c1.0, c1.1, c1.2), (4, 60, true));
    assert!((c1.3 - 0.026229).abs() < 1e-6 && (c1.4 - 0.159254).abs() < 1e-6);
    assert_eq!((c2.0, c2.1, c2.2), (3, 40, false));
    assert!((c2.3 - 0.025836).abs() < 1e-6 && (c2.4 - 0.198642).abs() < 1e-6);
    let known_c1 = creator_counts(db, "creator-C1", Some(1), 1).await;
    assert_eq!((known_c1.0, known_c1.1), (1, 55));
    let shadow_c1 = creator_counts(db, "creator-C1", None, f.shadow).await;
    assert_eq!((shadow_c1.0, shadow_c1.1), (4, 60));

    let preview = rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Preview)
        .await
        .unwrap();
    let unknown = preview
        .creator_preview
        .iter()
        .find(|g| {
            g.problem_ref == f.problem && g.metric_def_version == 1 && g.protocol_version.is_none()
        })
        .unwrap();
    assert!(unknown.ranked.iter().any(|c| c.creator_id == "creator-C1"));
    assert!(
        unknown
            .insufficient
            .iter()
            .any(|c| c.creator_id == "creator-C2")
    );
    let known = preview
        .creator_preview
        .iter()
        .find(|g| {
            g.problem_ref == f.problem && g.metric_def_version == 1 && g.protocol_version == Some(1)
        })
        .unwrap();
    assert!(known.ranked.iter().any(|c| c.creator_id == "creator-C1"));
    assert!(
        known
            .insufficient
            .iter()
            .any(|c| c.creator_id == "creator-C2")
    );
}

async fn verify_incremental_and_invariants(f: &Fixture) {
    let db = &f.db;
    let unchanged =
        rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Incremental)
            .await
            .unwrap();
    assert_eq!(unchanged.creator_rows_written, 0);
    assert_eq!(unchanged.period_rows_written, 0);
    sqlx::query("UPDATE linggan_opportunity_creator_problem_density SET sufficient=true WHERE creator_id='creator-C2' AND protocol_version IS NULL AND metric_def_version=1 AND problem_ref=md5('metric-problem:1')::uuid")
        .execute(db.pool()).await.unwrap();
    let violation =
        rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Preview).await;
    assert!(
        matches!(violation, Err(linggan_intelligence::opportunity::MetricCalculationError::InvariantViolations { ref checks }) if checks.contains(&"I7"))
    );
    sqlx::query("UPDATE linggan_opportunity_creator_problem_density SET sufficient=false WHERE creator_id='creator-C2' AND protocol_version IS NULL AND metric_def_version=1 AND problem_ref=md5('metric-problem:1')::uuid")
        .execute(db.pool()).await.unwrap();

    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,restricted_at,reason) VALUES(md5('creator-work:c1-a')::uuid,'creator-comment-c1-a-2','2026-11-09T00:00:00Z','synthetic restriction')")
        .execute(db.pool()).await.unwrap();
    let changed = rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Incremental)
        .await
        .unwrap();
    assert!(changed.creator_rows_written > 0);
    let c1_after = creator_counts(db, "creator-C1", None, 1).await;
    assert_eq!((c1_after.0, c1_after.1), (3, 59));
    let known_after = creator_counts(db, "creator-C1", Some(1), 1).await;
    assert_eq!((known_after.0, known_after.1), (1, 55));
    let incremental_rows = creator_snapshot(db).await;
    rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(incremental_rows, creator_snapshot(db).await);
}

async fn verify_window_and_history(f: &Fixture) {
    let db = &f.db;
    add_creator_window_boundaries(db).await;
    let boundaries =
        rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Incremental)
            .await
            .unwrap();
    assert!(
        boundaries
            .creator_window_exclusions
            .iter()
            .any(|e| e.metric_def_version == 1 && e.outside_window >= 2)
    );
    let included = creator_counts(db, "creator-C1", Some(1), 1).await;
    assert_eq!((included.0, included.1), (1, 55)); // lower bound is inclusive
    let excluded: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_creator_problem_density WHERE creator_id='creator-C2' AND protocol_version=1 AND metric_def_version=1")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(excluded, 0); // the as-of upper bound is exclusive

    add_future_missing_creator_detail(db).await;
    let before = rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Preview)
        .await
        .unwrap();
    assert!(
        before
            .creator_preview
            .iter()
            .any(|g| g.problem_ref == f.problem
                && g.metric_def_version == 1
                && g.protocol_version.is_none()
                && g.insufficient.iter().any(|c| c.creator_id == "creator-C2"))
    );
    let after = rebuild_problem_metrics(
        db,
        f.domain,
        "2026-11-11T00:00:00Z",
        &f.digest,
        RebuildMode::Full,
    )
    .await
    .unwrap();
    assert!(
        after
            .creator_window_exclusions
            .iter()
            .any(|e| e.metric_def_version == f.shadow && e.missing_work_published_at == 40)
    );
    let missing: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_creator_problem_density WHERE creator_id='creator-C2' AND protocol_version IS NULL")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(missing, 0);
    let history = rebuild_problem_metrics(db, f.domain, AS_OF, &f.digest, RebuildMode::Preview)
        .await
        .unwrap();
    assert!(
        history
            .creator_preview
            .iter()
            .any(|g| g.problem_ref == f.problem
                && g.metric_def_version == 1
                && g.protocol_version.is_none()
                && g.insufficient.iter().any(|c| c.creator_id == "creator-C2"))
    );
    let still_missing: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_creator_problem_density WHERE creator_id='creator-C2' AND protocol_version IS NULL")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(still_missing, 0);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn creator_density_uses_the_same_facts_and_never_mixes_protocols() {
    let fixture = fixture().await;
    verify_standard_and_protocols(&fixture).await;
    verify_incremental_and_invariants(&fixture).await;
    verify_window_and_history(&fixture).await;
}
