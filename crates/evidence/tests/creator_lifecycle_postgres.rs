#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{proof_database, submit_package, submit_package_at};
use linggan_evidence::{
    CreatorLifecycleAssociation, CreatorLifecycleMetric, CreatorLifecycleQuery,
    CreatorLifecycleStatus, CreatorLifecycleWindow, read_creator_lifecycle, read_work_resource,
};

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn creator_lifecycle_isolates_same_named_targets_by_stable_author_identity() {
    let database = proof_database("creator_lifecycle_identity").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-03T12:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();

    let target_a = uuid::Uuid::new_v4();
    let target_b = uuid::Uuid::new_v4();
    for (target_ref, identity_key) in [(target_a, "author-stable-a"), (target_b, "author-stable-b")]
    {
        sqlx::query(
            "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
             VALUES ($1,'xhs','creator',$2,'同名创作者','manual')",
        )
        .bind(target_ref)
        .bind(identity_key)
        .execute(database.pool())
        .await
        .unwrap();
    }

    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-author-a"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-author-a"},
            "payload":{
                "title":"A 的真实作品",
                "authorId":"author-stable-a",
                "likes":17,
                "publishedAt":1785542400000_i64,
                "publishedAtText":"1785542400",
                "publishedAtSourceField":"publishTime",
                "publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-author-b"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-author-b"},
            "payload":{
                "title":"B 的真实作品",
                "authorId":"author-stable-b",
                "likes":91,
                "publishedAt":1785628800000_i64,
                "publishedAtText":"1785628800",
                "publishedAtSourceField":"publishTime",
                "publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    let projection = read_creator_lifecycle(
        &database,
        target_a,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .expect("the target exists");

    assert_eq!(projection.status, CreatorLifecycleStatus::Ready);
    assert_eq!(projection.target_ref, target_a);
    // 同一时刻，读出来是北京时间：会话时区固定为 Asia/Shanghai，页面上的每个时间
    // 都按它格式化。断言写 UTC 会让这条测试要求一件与界面相反的事。
    assert_eq!(projection.as_of, "2026-09-03 20:00:00+08");
    assert_eq!(projection.summary.linked_work_count, Some(1));
    assert_eq!(projection.summary.linked_work_count_lower_bound, 1);
    assert_eq!(projection.summary.confirmed_author_work_count, 1);
    assert_eq!(projection.summary.eligible_point_count, 1);
    assert_eq!(projection.points.len(), 1);
    assert_eq!(projection.points[0].metric_value, 17);
    assert_eq!(
        projection.points[0].association_state,
        CreatorLifecycleAssociation::AuthorConfirmed
    );
    assert_eq!(projection.points[0].title.as_deref(), Some("A 的真实作品"));
    assert_eq!(projection.exclusions.author_not_verified, 0);
    assert_eq!(projection.exclusions.author_mismatch, 0);
    assert!(!projection.receipt.truncated);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn creator_lifecycle_preserves_unknown_zero_and_exclusion_reasons() {
    let database = proof_database("creator_lifecycle_truth_states").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-03T12:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','author-truth','真相边界作者','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();

    for content_external_id in [
        "surface-not-verified",
        "surface-mismatch",
        "relative-time",
        "metric-unknown",
        "known-zero",
    ] {
        submit_package(
            &database,
            "profile_discovery",
            serde_json::json!({"authorExternalId":"author-truth"}),
            serde_json::json!({
                "kind":"profile_discovery_card",
                "resultPosition":1,
                "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
                "payload":{"title":format!("发现 {content_external_id}")}
            }),
        )
        .await;
    }
    let stored_profile_targets: Vec<Option<String>> = sqlx::query_scalar(
        "SELECT task.task_spec #>> '{target,authorExternalId}' \
         FROM linggan_runtime_capture_package package \
         JOIN linggan_runtime_task task USING (task_id) \
         WHERE package.package_kind='profile_discovery'",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(stored_profile_targets.len(), 5);
    assert!(
        stored_profile_targets
            .iter()
            .all(|value| value.as_deref() == Some("author-truth")),
        "the fixture must preserve the exact target author identity"
    );
    let profile_finding_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_discovery_finding \
         WHERE discovery_kind='profile_discovery'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(profile_finding_count, 5);
    let target_surface_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT finding.content_public_ref) \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_runtime_capture_package package USING (package_ref) \
         JOIN linggan_runtime_task task ON task.task_id=package.task_id \
         WHERE finding.discovery_kind='profile_discovery' \
           AND task.task_spec #>> '{target,authorExternalId}'='author-truth'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(target_surface_count, 5);

    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"surface-mismatch"}),
        qualified_detail("surface-mismatch", "other-author", Some(5)),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"relative-time"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"relative-time"},
            "payload":{
                "authorId":"author-truth","likes":6,
                "publishedAt":1787283600000_i64,"publishedAtText":"3小时前",
                "publishedAtSourceField":"time","publishedAtSourceKind":"visible_text",
                "publishedAtPrecision":"relative",
                "publishedAtReferenceObservedAt":"2026-09-03T12:00:00Z",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"metric-unknown"}),
        qualified_detail("metric-unknown", "author-truth", None),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"known-zero"}),
        qualified_detail("known-zero", "author-truth", Some(0)),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"known-zero"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"known-zero"},
            "payload":{"authorId":"author-truth"}
        }),
    )
    .await;

    let projection = read_creator_lifecycle(
        &database,
        target_ref,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::All,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(projection.summary.linked_work_count, Some(5));
    assert_eq!(projection.summary.linked_work_count_lower_bound, 5);
    assert_eq!(projection.summary.confirmed_author_work_count, 3);
    assert_eq!(projection.summary.eligible_point_count, 1);
    assert_eq!(projection.exclusions.author_not_verified, 0);
    assert_eq!(projection.exclusions.author_mismatch, 1);
    assert_eq!(projection.exclusions.published_at_not_qualified, 2);
    assert_eq!(projection.exclusions.metric_unknown, 1);
    assert_eq!(
        projection.points[0].metric_value, 0,
        "KNOWN zero is a real point"
    );
    assert_eq!(
        projection.points[0].association_state,
        CreatorLifecycleAssociation::AuthorConfirmed
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn lifecycle_and_work_resource_share_one_field_wise_current_owner() {
    let database = proof_database("creator_lifecycle_work_resource_parity").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-03T12:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','author-current-owner','Current owner 作者','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();

    submit_package_at(
        &database,
        "profile_discovery",
        serde_json::json!({"authorExternalId":"author-current-owner"}),
        serde_json::json!({
            "kind":"profile_discovery_card",
            "resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":"current-owner-work"},
            "payload":{"title":"更晚观察到的发现标题"}
        }),
        "2026-08-29T10:00:00Z",
    )
    .await;
    let first_detail_package = submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"current-owner-work"}),
        qualified_detail_at(
            "current-owner-work",
            "author-current-owner",
            Some(11),
            1_785_542_400_000,
        ),
    )
    .await;
    let second_detail_package = submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"current-owner-work"}),
        qualified_detail_at(
            "current-owner-work",
            "author-current-owner",
            Some(37),
            1_785_628_800_000,
        ),
    )
    .await;
    let tied_observed_at_count: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT observed_at)::bigint \
         FROM linggan_material_content_detail WHERE package_ref=ANY($1::uuid[])",
    )
    .bind(vec![first_detail_package, second_detail_package])
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        tied_observed_at_count, 1,
        "the parity fixture must exercise the owner's deterministic tie-break, not two timestamps",
    );
    let (expected_package, expected_metric, expected_local_date) =
        if second_detail_package > first_detail_package {
            (second_detail_package, 37, "2026-08-02")
        } else {
            (first_detail_package, 11, "2026-08-01")
        };
    let expected_package = expected_package.to_string();
    let work_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='current-owner-work'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();

    let work = read_work_resource(&database, work_ref)
        .await
        .unwrap()
        .expect("the shared Work Resource exists");
    let lifecycle = read_creator_lifecycle(
        &database,
        target_ref,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::All,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .expect("the creator target exists");
    let point = lifecycle
        .points
        .iter()
        .find(|point| point.work_public_ref == work_ref)
        .expect("the Work Resource current author is the selected creator");

    assert_eq!(
        work.collection_context.author_identity_match_state,
        "MATCHED"
    );
    assert_eq!(point.title, work.display.title);
    assert_eq!(point.title_state, work.display.title_state);
    assert_eq!(
        Some(point.published_at.as_str()),
        work.display.published_at.as_deref()
    );
    assert_eq!(Some(point.metric_value), work.display.engagement.like_count);
    assert_eq!(
        point.title.as_deref(),
        Some("详情 current-owner-work"),
        "a later discovery observation must not replace Work Resource's qualified detail title",
    );
    assert_eq!(point.metric_value, expected_metric);
    assert_eq!(point.published_local_date, expected_local_date);
    assert_eq!(
        work.inspector
            .pointer("/engagementCurrent/metrics/likeCount/current/packageRef")
            .and_then(serde_json::Value::as_str),
        Some(expected_package.as_str()),
    );
    assert_eq!(
        work.inspector
            .pointer("/detailCurrent/publishedAt/source/packageRef")
            .and_then(serde_json::Value::as_str),
        Some(expected_package.as_str()),
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn recent_window_uses_inclusive_asia_shanghai_calendar_dates() {
    let database = proof_database("creator_lifecycle_shanghai_window").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-04T15:59:59Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','author-window','日期边界作者','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();

    for (content_external_id, published_at_ms, likes) in [
        ("before-start", 1_780_761_599_000_i64, 1_i64),
        ("at-start", 1_780_761_600_000_i64, 2_i64),
        ("at-end", 1_788_537_599_000_i64, 3_i64),
    ] {
        submit_package(
            &database,
            "content_detail",
            serde_json::json!({"contentExternalId":content_external_id}),
            qualified_detail_at(
                content_external_id,
                "author-window",
                Some(likes),
                published_at_ms,
            ),
        )
        .await;
    }

    let recent = read_creator_lifecycle(
        &database,
        target_ref,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::Recent90Days,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(recent.summary.linked_work_count, Some(3));
    assert_eq!(recent.summary.linked_work_count_lower_bound, 3);
    assert_eq!(recent.summary.eligible_point_count, 2);
    assert_eq!(recent.exclusions.outside_window, 1);
    assert_eq!(
        recent
            .points
            .iter()
            .map(|point| point.metric_value)
            .collect::<Vec<_>>(),
        vec![2, 3],
        "Shanghai 2026-06-07 through 2026-09-04 are the 90 inclusive calendar dates",
    );
    assert_eq!(
        recent
            .points
            .iter()
            .map(|point| point.published_local_date.as_str())
            .collect::<Vec<_>>(),
        vec!["2026-06-07", "2026-09-04"],
        "the read model must expose the same Shanghai calendar dates used by the window",
    );

    let all = read_creator_lifecycle(
        &database,
        target_ref,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::All,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(all.summary.eligible_point_count, 3);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn creator_lifecycle_reports_its_bounded_scan_receipt() {
    let database = proof_database("creator_lifecycle_scan_receipt").await;
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','author-limit','有界扫描作者','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let task_id = uuid::Uuid::new_v4();
    let attempt_id = uuid::Uuid::new_v4();
    let producer_id = uuid::Uuid::new_v4();
    let package_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
           (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,repeat('1',64),'{\"target\":{\"authorExternalId\":\"author-limit\"}}','manual','xhs','synthetic')",
    )
    .bind(task_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_attempt (attempt_id,task_id,producer_instance_id) \
         VALUES ($1,$2,$3)",
    )
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_capture_package \
           (package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform,package_hash,observed_at,captured_at,coverage,payload) \
         VALUES ($1,$2,$3,$4,'profile_discovery','xhs',repeat('2',64),'2026-09-03T12:00:00Z','2026-09-03T12:00:01Z','{}','{}')",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_submission_receipt \
           (submission_id,task_id,attempt_id,producer_instance_id,package_hash,package_ref, \
            receipt_ref,execution_effect,material_admission) \
         VALUES ($1,$2,$3,$4,repeat('2',64),$5,$6,'NOT_APPLICABLE','ACCEPTED')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(task_id)
    .bind(attempt_id)
    .bind(producer_id)
    .bind(package_ref)
    .bind(uuid::Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         SELECT $1,ordinal,'accepted_for_library_discovery','synthetic scan receipt proof' \
         FROM generate_series(0,2000) ordinal",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_content \
           (platform,content_external_id,public_ref,first_package_ref) \
         SELECT 'xhs','limit-' || ordinal,md5('content-' || ordinal)::uuid,$1 \
         FROM generate_series(0,2000) ordinal",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_discovery_finding \
           (material_ref,content_public_ref,package_ref,record_ordinal,discovery_kind,result_position,observed_at,title,title_state,creator_display_name,creator_state,published_at_source_text,published_at_source_text_state) \
         SELECT md5('material-' || ordinal)::uuid,md5('content-' || ordinal)::uuid,$1,ordinal, \
                'profile_discovery',ordinal + 1,'2026-09-03T12:00:00Z',NULL,'UNKNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN' \
         FROM generate_series(0,2000) ordinal",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let projection = read_creator_lifecycle(
        &database,
        target_ref,
        &CreatorLifecycleQuery {
            window: CreatorLifecycleWindow::All,
            metric: CreatorLifecycleMetric::Likes,
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(projection.summary.linked_work_count, None);
    assert_eq!(projection.summary.linked_work_count_lower_bound, 2_001);
    assert_eq!(projection.receipt.scan_limit, 2_000);
    assert_eq!(projection.receipt.probed_count, 2_001);
    assert_eq!(projection.receipt.scanned_count, 2_000);
    assert_eq!(projection.receipt.returned_count, 0);
    assert!(projection.receipt.truncated);
    assert!(projection.exclusions.scan_truncated);
    assert_eq!(projection.exclusions.author_not_verified, 0);
    assert_eq!(projection.exclusions.published_at_not_qualified, 2_000);
    assert_eq!(
        projection.status,
        CreatorLifecycleStatus::InsufficientObservation
    );
}

fn qualified_detail(
    content_external_id: &str,
    author_external_id: &str,
    likes: Option<i64>,
) -> serde_json::Value {
    qualified_detail_at(
        content_external_id,
        author_external_id,
        likes,
        1_785_542_400_000_i64,
    )
}

fn qualified_detail_at(
    content_external_id: &str,
    author_external_id: &str,
    likes: Option<i64>,
    published_at_ms: i64,
) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "title":format!("详情 {content_external_id}"),
        "authorId":author_external_id,
        "publishedAt":published_at_ms,
        "publishedAtText":(published_at_ms / 1000).to_string(),
        "publishedAtSourceField":"publishTime",
        "publishedAtSourceKind":"platform_epoch",
        "publishedAtPrecision":"second",
        "publishedAtParserVersion":"xhs-detail-time-v2"
    });
    if let Some(likes) = likes {
        payload["likes"] = serde_json::json!(likes);
    }
    serde_json::json!({
        "kind":"content_detail",
        "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
        "payload":payload
    })
}
