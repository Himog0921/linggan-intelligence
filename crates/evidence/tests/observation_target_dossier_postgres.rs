#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_contracts::{
    AdmissionOutcome, ProducerTaskSpec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilityObservation, AcquisitionChainError, AuthorizationGrant, CatalogDetailState,
    CheckInOutcome, CreatorLifecycleAssociation, CreatorLifecycleMetric, CreatorLifecycleQuery,
    CreatorLifecycleStatus, CreatorLifecycleWindow, DispatchDecision, DispatchFailureCode,
    DispatchFailureOutcome, InstallationCheckIn, RequestLeaseError, RuntimeAttemptOutcome,
    RuntimeSubmissionOutcome, activate_installation_credential, bind_observation_account,
    check_in_installation, decide_dispatch, grant_authorization, list_targets, open_claim_window,
    read_archive_completeness, read_creator_directory, read_creator_lifecycle, read_target,
    register_station, report_account_eligibility, request_admit_and_lease,
    request_progressive_archive_and_lease, requeue_failed_dispatch, retire_materials,
    run_progressive_archives, set_station_accepting, start_producer_attempt,
    submit_producer_package,
};
use linggan_storage_postgres::Database;
use std::time::Duration;
use uuid::Uuid;

const DIGEST_KEY: &[u8] = b"observation-target-dossier-postgres-proof-v1";

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn target_drawer_read_uses_the_same_full_projection_as_the_target_list() {
    let database = proof_database("dossier_target_drawer_projection").await;
    let target_ref = seed_creator_target(&database, "creator-drawer-projection").await;

    let drawer_target = read_target(&database, target_ref)
        .await
        .expect("the target drawer query is available")
        .expect("the newly stored target is readable by exact reference");
    let list_target = list_targets(&database, Some("creator"), None, 10)
        .await
        .expect("the targets list query is available")
        .into_iter()
        .find(|target| target.target_ref == target_ref)
        .expect("the exact target appears in the matching list");

    assert_eq!(drawer_target.target_ref, list_target.target_ref);
    assert_eq!(drawer_target.identity_key, list_target.identity_key);
    assert_eq!(drawer_target.domain_name, list_target.domain_name);
    assert_eq!(drawer_target.domain_is_own, list_target.domain_is_own);
    assert_eq!(drawer_target.domain_name.as_deref(), Some("ADHD"));
    assert_eq!(drawer_target.domain_is_own, Some(true));
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn progressive_archive_tick_reads_a_root_without_ambiguous_target_ref_join() {
    let database = proof_database("dossier_progressive_tick_root_join").await;
    ready_installation(&database, "dossier-tick-root-join").await;
    let target_ref = seed_creator_target(&database, "creator-progressive-tick-root-join").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    request_progressive_archive_and_lease(&database, target_ref, "建立创作者档案", "person", 30)
        .await
        .expect("a progressive root is present for the worker tick");

    let summary = run_progressive_archives(&database)
        .await
        .expect("the worker can read a progressive root after joining request and decision facts");
    assert!(
        summary.queued.contains(&target_ref)
            || summary
                .skipped
                .iter()
                .any(|(skipped_target, _)| *skipped_target == target_ref),
        "the root is evaluated instead of being hidden by an ambiguous SQL join: {summary:?}"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn progressive_archive_requires_200_and_freezes_the_root_directory_contract() {
    let database = proof_database("dossier_progressive_root").await;
    ready_installation(&database, "dossier-root").await;
    let target_ref = seed_creator_target(&database, "creator-progressive-root").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;

    let outcome = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .expect("the explicitly 200-Work authorization admits one bounded root lease");
    assert!(matches!(
        outcome.request.outcome,
        AdmissionOutcome::Admitted { .. }
    ));
    let work_order_ref = outcome
        .request
        .work_order_ref
        .expect("admission creates the root Work Order");
    let lease = outcome.lease.expect("the admitted root is leased");

    let (max_works, stop_conditions): (i32, serde_json::Value) = sqlx::query_as(
        "SELECT max_works,stop_conditions FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(max_works, 200);
    assert_eq!(
        stop_conditions
            .pointer("/progressiveArchive/version")
            .and_then(serde_json::Value::as_i64),
        Some(1)
    );
    assert_eq!(
        stop_conditions
            .pointer("/progressiveArchive/rootWorkOrderRef")
            .and_then(serde_json::Value::as_str),
        Some(work_order_ref.to_string().as_str())
    );
    assert_eq!(
        stop_conditions
            .pointer("/progressiveArchive/maxDirectoryWorks")
            .and_then(serde_json::Value::as_i64),
        Some(200)
    );
    assert_eq!(
        stop_conditions
            .pointer("/progressiveArchive/batchSize")
            .and_then(serde_json::Value::as_i64),
        Some(3)
    );
    assert_eq!(
        stop_conditions
            .pointer("/progressiveArchive/commentLimit")
            .and_then(serde_json::Value::as_i64),
        Some(30)
    );

    let tasks: Vec<(i32, String, i32)> = sqlx::query_as(
        "SELECT lease_task.sequence_no,task.task_spec #>> '{capabilitiesRequested,0}', \
                (task.task_spec #>> '{maximumQuota}')::integer \
         FROM collection_work_order_lease_task lease_task \
         JOIN linggan_runtime_task task USING (task_id) \
         WHERE lease_task.lease_ref=$1 ORDER BY lease_task.sequence_no",
    )
    .bind(lease.lease_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        tasks,
        vec![
            (1, "author_profile".to_owned(), 1),
            (2, "profile_discovery".to_owned(), 200),
        ]
    );
    assert_eq!(lease.task_ids.len(), 2);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn clean_200_work_progressive_root_establishes_the_bounded_creator_baseline() {
    let database = proof_database("dossier_progressive_bounded_baseline").await;
    let installation = ready_installation(&database, "dossier-bounded-baseline").await;
    let target_ref = seed_creator_target(&database, "creator-bounded-baseline").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    request_progressive_archive_and_lease(&database, target_ref, "建立创作者档案", "person", 30)
        .await
        .unwrap();

    complete_progressive_root_at_the_200_work_bound(&database, &installation).await;

    let state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        state, "archived",
        "a clean, explicitly bounded 200-Work directory is established without claiming the platform surface ended",
    );
}

/// 人确认「这篇在平台上没了」之后，作品仍留在目录里，只是不再计入待补齐。
///
/// 抹掉分母会让「档案完成」建立在一个修饰过的数字上——这个博主当时确实发过这几篇。
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn a_retired_work_stays_in_the_directory_and_leaves_the_pending_count() {
    let database = proof_database("dossier_material_retirement").await;
    let installation = ready_installation(&database, "dossier-retirement").await;
    let target_ref = seed_creator_target(&database, "creator-retirement").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    request_progressive_archive_and_lease(&database, target_ref, "建立创作者档案", "person", 30)
        .await
        .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 31, "surface_ended")
        .await;

    let before = read_archive_completeness(&database, "xhs").await.unwrap();
    let before = before.get("creator-retirement").unwrap();
    assert_eq!((before.works_listed, before.details_captured), (31, 0));
    assert_eq!(before.retired_works, 0);

    let gone: Uuid = sqlx::query_scalar(
        "SELECT finding.content_public_ref FROM linggan_material_discovery_finding finding          ORDER BY finding.content_public_ref LIMIT 1",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let written = retire_materials(&database, target_ref, &[gone], "page_gone")
        .await
        .unwrap();
    assert_eq!(written, 1);

    let after = read_archive_completeness(&database, "xhs").await.unwrap();
    let after = after.get("creator-retirement").unwrap();
    assert_eq!(
        after.works_listed, 31,
        "确认失效不改写目录：这个博主当时确实发过这一篇"
    );
    assert_eq!(after.details_captured, 0, "确认失效不是取得详情");
    assert_eq!(after.retired_works, 1);

    // 再点一次是同一个结论，不该变成第二条事实。
    let again = retire_materials(&database, target_ref, &[gone], "page_gone")
        .await
        .unwrap();
    assert_eq!(again, 0);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_material_retirement")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

/// T19：**详情已经取到、只是平台没给标题**，仍然算「详情已取得」。
///
/// 此前这张目录用 `detail_title.is_some()` 回答「这一篇有没有详情」。于是一篇详情已经落库、
/// 标题为空的合格材料，在列表和抽屉里显示成还欠一篇、在补齐资格里被重新挑走，而在覆盖统计
/// 里它已经算取得了——同一条材料事实读出两种答案。标题是**字段覆盖度**（`0015` 的
/// `title_state='UNKNOWN'`），不是完成度：缺标题该说「标题未收录」，不是把整篇材料说成没取到。
///
/// 目录里三篇，只有中间那一篇有详情，这样「已取得」与「待取得」不是同一个数字的两种读法：
/// 另外两篇真的还欠着，要一起断言，才不会把「目录里有详情就把整页算成有详情」放过去。
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn a_detail_accepted_without_a_title_still_counts_as_captured() {
    let database = proof_database("dossier_untitled_detail").await;
    let installation = ready_installation(&database, "dossier-untitled-detail").await;
    let target_ref = seed_creator_target(&database, "creator-untitled-detail").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 3, "surface_ended")
        .await;

    let captured_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='creator-untitled-detail-partial-0'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_untitled_detail_observation(&database, captured_ref).await;

    let directory = read_creator_directory(&database, target_ref)
        .await
        .unwrap()
        .expect("已证明的目录可以被读出来");
    let captured = directory
        .works
        .iter()
        .find(|work| work.public_ref == captured_ref)
        .expect("有详情的那一篇就在目录里");
    assert_eq!(
        captured.detail_state,
        CatalogDetailState::Complete,
        "详情材料已经落库，欠的只是标题——这是字段覆盖度，不是还没去取"
    );
    assert_eq!(
        captured.title.as_deref(),
        Some("partial work 0"),
        "没有标题就如实没有：发现卡上的标题照旧显示，不因为详情缺标题就回写一个补出来的"
    );
    let mut pending: Vec<&str> = directory
        .works
        .iter()
        .filter(|work| work.detail_state == CatalogDetailState::Pending)
        .map(|work| work.content_external_id.as_str())
        .collect();
    pending.sort_unstable();
    assert_eq!(
        pending,
        vec![
            "creator-untitled-detail-partial-1",
            "creator-untitled-detail-partial-2"
        ],
        "另外两篇确实还欠着详情，不能被这一篇的结论一起带过"
    );

    let ledger = read_archive_completeness(&database, "xhs").await.unwrap();
    let ledger = ledger.get("creator-untitled-detail").unwrap();
    assert_eq!(
        (
            ledger.works_listed,
            ledger.details_captured,
            ledger.pending_details
        ),
        (3, 1, 2),
        "覆盖统计与目录读的是同一句判据，不能让同一份材料在两处各答一次"
    );

    // 「不列为待详情采集」走的不是展示层：补详情的子工单排出哪些篇，是执行侧自己挑的。
    run_progressive_archives(&database).await.unwrap();
    let child = pending_detail_batch(&database, target_ref, root).await;
    let scope: Vec<String> = sqlx::query_scalar(
        "SELECT content.content_external_id FROM collection_work_order_material_target scope \
         JOIN linggan_material_content content ON content.public_ref=scope.content_public_ref \
         WHERE scope.work_order_ref=$1 ORDER BY content.content_external_id",
    )
    .bind(child)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        scope,
        vec![
            "creator-untitled-detail-partial-1".to_owned(),
            "creator-untitled-detail-partial-2".to_owned()
        ],
        "已经取到详情的那一篇不再被排进补详情——它欠的是标题，不是材料"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn incomplete_directory_is_rebuilt_as_a_new_current_baseline_without_mutating_history() {
    let database = proof_database("dossier_rebuild_incomplete_directory").await;
    let installation = ready_installation(&database, "dossier-rebuild-incomplete").await;
    let target_ref = seed_creator_target(&database, "creator-rebuild-incomplete").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let old_root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 31, "risk_control")
        .await;

    let before = read_archive_completeness(&database, "xhs").await.unwrap();
    let before = before.get("creator-rebuild-incomplete").unwrap();
    assert_eq!(
        before.works_listed, 0,
        "a partial historical directory cannot become the current detail denominator"
    );
    assert!(before.requires_directory_rebuild());
    assert!(!before.has_displayable_directory());

    let historical_package_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_capture_package package \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE lease.work_order_ref=$1",
    )
    .bind(old_root)
    .fetch_one(database.pool())
    .await
    .unwrap();

    let rebuilt = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .expect("a non-baseline partial directory starts a normal new homepage directory request");
    let new_root = rebuilt.request.work_order_ref.unwrap();
    assert_ne!(new_root, old_root);

    let roots: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order \
         WHERE target_ref=$1 AND lane='deep_archive' \
           AND stop_conditions #>> '{progressiveArchive,version}'='1' \
           AND stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order_ref::text",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        roots, 2,
        "the former incomplete root remains immutable history"
    );
    let historical_package_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_capture_package package \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE lease.work_order_ref=$1",
    )
    .bind(old_root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(historical_package_count_after, historical_package_count);

    let current = read_archive_completeness(&database, "xhs").await.unwrap();
    let current = current.get("creator-rebuild-incomplete").unwrap();
    assert!(current.work_in_progress);
    assert_eq!(
        current.works_listed, 0,
        "old partial links are not the new directory denominator"
    );
    assert!(!current.has_displayable_directory());
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn surface_ended_directory_below_200_remains_a_valid_current_baseline() {
    let database = proof_database("dossier_surface_ended_directory").await;
    let installation = ready_installation(&database, "dossier-surface-ended-directory").await;
    let target_ref = seed_creator_target(&database, "creator-surface-ended-directory").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 31, "surface_ended")
        .await;

    let baseline = read_archive_completeness(&database, "xhs").await.unwrap();
    let baseline = baseline.get("creator-surface-ended-directory").unwrap();
    assert!(baseline.has_displayable_directory());
    assert_eq!(baseline.works_listed, 31);

    let continuation = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .expect("a valid surface-ended directory only queues a small detail continuation");
    assert_ne!(continuation.request.work_order_ref.unwrap(), root);
    let root_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order \
         WHERE target_ref=$1 AND lane='deep_archive' \
           AND stop_conditions #>> '{progressiveArchive,version}'='1' \
           AND stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order_ref::text",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(root_count, 1);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn surface_ended_directory_from_a_legacy_smaller_quota_requires_rebuild() {
    let database = proof_database("dossier_surface_ended_legacy_quota").await;
    let installation = ready_installation(&database, "dossier-surface-ended-legacy-quota").await;
    let target_ref = seed_creator_target(&database, "creator-surface-ended-legacy-quota").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let old_root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 31, "surface_ended")
        .await;
    // This synthetic fixture represents a pre-contract immutable runtime task.  Disable only
    // its append-only trigger long enough to express the historical smaller quota, then restore
    // it before exercising the current read/action contract.
    sqlx::query(
        "ALTER TABLE linggan_runtime_task DISABLE TRIGGER linggan_runtime_task_is_append_only",
    )
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_runtime_task task \
         SET task_spec=jsonb_set(task.task_spec,'{maximumQuota}','31'::jsonb) \
         FROM collection_work_order_lease_task lease_task \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE task.task_id=lease_task.task_id AND lease.work_order_ref=$1",
    )
    .bind(old_root)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE linggan_runtime_task ENABLE TRIGGER linggan_runtime_task_is_append_only",
    )
    .execute(database.pool())
    .await
    .unwrap();

    let baseline = read_archive_completeness(&database, "xhs").await.unwrap();
    let baseline = baseline.get("creator-surface-ended-legacy-quota").unwrap();
    assert!(!baseline.has_displayable_directory());
    assert!(baseline.requires_directory_rebuild());

    let rebuilt = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .expect("a legacy smaller quota must create a new current root");
    assert_ne!(rebuilt.request.work_order_ref.unwrap(), old_root);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn fully_detailed_legacy_directory_stays_visible_without_claiming_the_200_work_boundary() {
    let database = proof_database("dossier_historical_complete_directory").await;
    let installation = ready_installation(&database, "dossier-historical-complete").await;
    let target_ref = seed_creator_target(&database, "creator-historical-complete").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let old_root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 3, "surface_ended")
        .await;

    // Make this immutable root represent pre-200-contract history, then add a detail Package
    // for each accepted discovery work through the exact target → Work → Lease → Task chain.
    sqlx::query(
        "ALTER TABLE linggan_runtime_task DISABLE TRIGGER linggan_runtime_task_is_append_only",
    )
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_runtime_task task \
         SET task_spec=jsonb_set(task.task_spec,'{maximumQuota}','3'::jsonb) \
         FROM collection_work_order_lease_task lease_task \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE task.task_id=lease_task.task_id AND lease.work_order_ref=$1",
    )
    .bind(old_root)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "ALTER TABLE linggan_runtime_task ENABLE TRIGGER linggan_runtime_task_is_append_only",
    )
    .execute(database.pool())
    .await
    .unwrap();

    let lease_ref: Uuid = sqlx::query_scalar(
        "SELECT lease_ref FROM collection_work_order_lease WHERE work_order_ref=$1",
    )
    .bind(old_root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let works: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT finding.content_public_ref \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_runtime_capture_package package ON package.package_ref=finding.package_ref \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE lease.work_order_ref=$1",
    )
    .bind(old_root)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(works.len(), 3);
    for (index, work_ref) in works.iter().enumerate() {
        let task_id = seed_bound_task(
            &database,
            lease_ref,
            10 + i32::try_from(index).unwrap(),
            "content_detail",
            serde_json::json!({"contentExternalId": format!("legacy-detail-{index}")}),
        )
        .await;
        let package =
            seed_runtime_package(&database, task_id, "content_detail", "2026-09-04T02:00:00Z")
                .await;
        sqlx::query(
            "INSERT INTO linggan_runtime_record_disposition \
               (package_ref,record_ordinal,disposition,reason) \
             VALUES ($1,0,'accepted_for_library_content','historical complete directory proof')",
        )
        .bind(package)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_material_content_detail \
               (material_ref,content_public_ref,package_ref,record_ordinal,observed_at, \
                title,title_state,body_text,body_state,creator_display_name, \
                creator_display_name_state,published_at_source_text, \
                published_at_source_text_state,searchable_text) \
             VALUES ($1,$2,$3,0,'2026-09-04T02:00:00Z', \
                     '历史完整详情','KNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN', \
                     NULL,'UNKNOWN','历史完整详情')",
        )
        .bind(Uuid::new_v4())
        .bind(work_ref)
        .bind(package)
        .execute(database.pool())
        .await
        .unwrap();
    }

    let baseline = read_archive_completeness(&database, "xhs").await.unwrap();
    let baseline = baseline.get("creator-historical-complete").unwrap();
    assert_eq!(
        baseline.directory_baseline,
        linggan_evidence::ArchiveDirectoryBaseline::HistoricalDirectory
    );
    assert!(baseline.has_displayable_directory());
    assert_eq!((baseline.works_listed, baseline.details_captured), (3, 3));
    assert!(
        !baseline.requires_directory_rebuild(),
        "existing complete in-library history is not relabelled as missing"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn quarantined_directory_record_remains_an_archive_problem() {
    let database = proof_database("dossier_quarantined_directory").await;
    let installation = ready_installation(&database, "dossier-quarantined-directory").await;
    let target_ref = seed_creator_target(&database, "creator-quarantined-directory").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 31, "surface_ended")
        .await;
    let directory_package: Uuid = sqlx::query_scalar(
        "SELECT package.package_ref \
         FROM linggan_runtime_capture_package package \
         JOIN collection_work_order_lease_task lease_task ON lease_task.task_id=package.task_id \
         JOIN collection_work_order_lease lease USING(lease_ref) \
         WHERE lease.work_order_ref=$1 AND package.package_kind='profile_discovery'",
    )
    .bind(root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         VALUES ($1,999,'quarantined','synthetic directory conflict')",
    )
    .bind(directory_package)
    .execute(database.pool())
    .await
    .unwrap();

    let baseline = read_archive_completeness(&database, "xhs").await.unwrap();
    let baseline = baseline.get("creator-quarantined-directory").unwrap();
    assert!(!baseline.has_displayable_directory());
    assert!(baseline.requires_directory_rebuild());
    assert_eq!(baseline.quarantined, 1);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn successful_patrol_adds_new_work_to_the_current_directory_and_detail_denominator() {
    let database = proof_database("dossier_patrol_extends_directory").await;
    let installation = ready_installation(&database, "dossier-patrol-extends-directory").await;
    let target_ref = seed_creator_target(&database, "creator-patrol-extends-directory").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    request_progressive_archive_and_lease(&database, target_ref, "建立创作者档案", "person", 30)
        .await
        .unwrap();
    complete_progressive_root_at_the_200_work_bound(&database, &installation).await;
    grant_patrol(&database, "巡查建档创作者").await;

    submit_patrol_round(
        &database,
        &installation,
        target_ref,
        "巡查建档创作者",
        PatrolRound::OneUsableWork,
    )
    .await;
    let after_new = read_archive_completeness(&database, "xhs").await.unwrap();
    let after_new = after_new.get("creator-patrol-extends-directory").unwrap();
    assert!(after_new.has_displayable_directory());
    assert_eq!(after_new.works_listed, 201);
    assert_eq!(after_new.details_captured, 0);

    submit_patrol_round(
        &database,
        &installation,
        target_ref,
        "巡查建档创作者",
        PatrolRound::ValidZeroNew,
    )
    .await;
    let after_zero = read_archive_completeness(&database, "xhs").await.unwrap();
    let after_zero = after_zero.get("creator-patrol-extends-directory").unwrap();
    assert_eq!(
        after_zero.works_listed, 201,
        "a qualified zero-new patrol does not inflate the directory"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn completed_progressive_archive_never_turns_a_patrol_addition_into_deepening() {
    let database = proof_database("dossier_completed_archive_patrol").await;
    let installation = ready_installation(&database, "dossier-completed-archive-patrol").await;
    let target_ref = seed_creator_target(&database, "creator-completed-archive-patrol").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 1, "surface_ended")
        .await;
    let canonical_work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='creator-completed-archive-patrol-partial-0'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_detail_observation(
        &database,
        canonical_work,
        "creator-completed-archive-patrol-partial-0",
        Some("creator-completed-archive-patrol"),
        1,
        "2026-09-03T00:00:00Z",
    )
    .await;

    grant_patrol(&database, "巡查建档创作者").await;
    submit_patrol_round(
        &database,
        &installation,
        target_ref,
        "巡查建档创作者",
        PatrolRound::OneUsableWork,
    )
    .await;

    let summary = run_progressive_archives(&database).await.unwrap();
    assert!(
        !summary.queued.contains(&target_ref),
        "a patrol-only addition must not create a deep archive batch: {summary:?}"
    );
    assert!(summary.skipped.iter().any(|(target, reason)| {
        *target == target_ref && reason == "archive_baseline_complete"
    }));
    let status: String = sqlx::query_scalar(
        "SELECT stop_conditions #>> '{progressiveArchive,status}' \
         FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(status, "completed");
}

/// **「这一篇不再自动重试」不等于「这份目录已经齐了」。**
///
/// 目录里只剩一篇没有详情、而它恰好把页面读预算用光时，候选查询会一条都挑不出来。
/// 从前「挑不出候选」只有一种落法：把根标成完成。两层后果——界面上这份基线被说成齐了，
/// 而它没有；更重的是**完成的根不会再被自动续跑**（活根查询只认 `status='active'`），
/// 欠着的那一条详情于是永远等不到下一次机会，除非有人重新发起一次真实平台访问去建新目录。
///
/// 空候选集因此必须再问一句「目录里还欠不欠详情」：欠着就如实说欠着，根保持活着。
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn a_detail_that_spent_its_page_read_budget_keeps_the_baseline_open() {
    let database = proof_database("dossier_spent_budget_baseline_stays_open").await;
    let installation = ready_installation(&database, "dossier-spent-budget").await;
    let target_ref = seed_creator_target(&database, "creator-spent-budget").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_partial_directory(&database, &installation, 1, "surface_ended")
        .await;
    let member = "creator-spent-budget-partial-0";

    // 第一段是真的自动跑出来的：补详情为这一篇排了一张子工单，不是把状态摆好让系统去认。
    let first = run_progressive_archives(&database).await.unwrap();
    assert!(first.queued.contains(&target_ref), "{first:?}");
    let child = pending_detail_batch(&database, target_ref, root).await;
    // 这一篇连读三次都读不出来：退避阶梯 60/120，第三次用尽预算。
    for (ordinal, expected) in [(1_i32, 60_u32), (2, 120)] {
        clear_dispatch_backoff(&database, child).await;
        let outcome = report_detail_read_failure(&database, &installation, child, member).await;
        assert_eq!(
            outcome,
            DispatchFailureOutcome::Requeued {
                retry_after_seconds: expected
            },
            "第 {ordinal} 次页面读失败仍在退避阶梯上"
        );
    }
    clear_dispatch_backoff(&database, child).await;
    assert_eq!(
        report_detail_read_failure(&database, &installation, child, member).await,
        DispatchFailureOutcome::Blocked,
        "第三次用尽预算：这一篇不再自动重试"
    );
    let ledger: Vec<(String, i32)> = sqlx::query_as(
        "SELECT state,deduplicated_failure_count FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_kind='material_content' AND capability='content_detail' \
           AND object_ref=(SELECT public_ref FROM linggan_material_content \
                           WHERE content_external_id=$2)",
    )
    .bind(target_ref)
    .bind(member)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        ledger,
        vec![("budget_exhausted".to_owned(), 3)],
        "空候选集的原因是预算用尽，不是别的东西碰巧把候选挑空了"
    );

    // 预算用尽之后这一篇不再自动重试，目录里只剩它一条欠着详情。
    let tick = run_progressive_archives(&database).await.unwrap();
    assert!(
        !tick.queued.contains(&target_ref),
        "预算用尽的成员不该被再排一张新工单：{tick:?}"
    );
    assert!(
        !tick
            .skipped
            .iter()
            .any(|(target, reason)| *target == target_ref && reason == "archive_baseline_complete"),
        "目录里还欠着详情，这份基线不算完成：{tick:?}"
    );
    assert!(
        tick.skipped
            .iter()
            .any(|(target, reason)| *target == target_ref && reason == "detail_gap_not_schedulable"),
        "要如实说出「欠着的这一条现在排不进去」：{tick:?}"
    );
    let status: String = sqlx::query_scalar(
        "SELECT COALESCE(stop_conditions #>> '{progressiveArchive,status}','active') \
         FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        status, "active",
        "根必须保持活着，否则欠着的那一条详情再也没有自动续跑的机会"
    );
    let details: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content_detail detail \
         JOIN linggan_material_content content ON content.public_ref=detail.content_public_ref \
         WHERE content.content_external_id=$1",
    )
    .bind(member)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(details, 0, "这一篇确实还没有详情——收口就是一句假话");
}

/// **解析不出执行地址的目录成员不该被排进候选。**
///
/// 平台会在一些卡片上不给带 `xsec_token` 的链接，插件照实上报。这种成员排进工单只会得到一次
/// 「派不出去」：占一张工单、一次调度，一次页面都不打开，随后被停成 `input_blocked`、工单终结
/// 为 `cancelled`；下一轮 tick 的候选里它还在——停止本身变成新的循环。关键词那一侧已经用
/// 「此刻能不能解析出地址」把这类作品挡在候选之外（迁移 `0097` 的入口判据），渐进建档这一侧
/// 只有「有没有详情」和「在不在途」，同一件事问了两套。
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn a_directory_member_without_a_signed_link_never_enters_a_doomed_work_order() {
    let database = proof_database("dossier_directory_member_without_link").await;
    let installation = ready_installation(&database, "dossier-no-link").await;
    let target_ref = seed_creator_target(&database, "creator-no-link").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap()
    .request
    .work_order_ref
    .unwrap();
    complete_progressive_root_with_directory(&database, &installation, 1, "surface_ended", false)
        .await;

    let tick = run_progressive_archives(&database).await.unwrap();
    assert!(
        !tick.queued.contains(&target_ref),
        "没有执行地址的成员排不进工单：排进去只会被停一次，而下一轮还会再排一次：{tick:?}"
    );
    assert!(
        tick.skipped.iter().any(|(target, reason)| {
            *target == target_ref && reason == "detail_gap_not_schedulable"
        }),
        "要如实说出「欠着的这一条现在排不进去」：{tick:?}"
    );
    let children: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order work_order \
         JOIN collection_work_order_material_target scope USING(work_order_ref) \
         WHERE work_order.target_ref=$1 AND work_order.work_order_ref<>$2",
    )
    .bind(target_ref)
    .bind(root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(children, 0, "一篇都不该被排进子工单");
    let status: String = sqlx::query_scalar(
        "SELECT COALESCE(stop_conditions #>> '{progressiveArchive,status}','active') \
         FROM collection_work_order WHERE work_order_ref=$1",
    )
    .bind(root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        status, "active",
        "缺口还在，根就得活着——等平台下一次给出带链接的卡片时它要能接着跑"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn progressive_archive_rolls_each_source_to_the_ready_claimant_batch_cap() {
    let database = proof_database("dossier_progressive_batch_ready_cap").await;
    let first_installation = ready_installation(&database, "dossier-batch-cap-first").await;
    ready_installation(&database, "dossier-batch-cap-second").await;
    let first_target = seed_creator_target(&database, "creator-batch-cap-first").await;
    let second_target = seed_creator_target(&database, "creator-batch-cap-second").await;
    let authorization_ref = grant_authorization(
        &database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "deep_archive",
            purpose: "批量滚动容量证明",
            max_targets: Some(2),
            max_works_per_target: Some(200),
            valid_for_days: 1,
        },
    )
    .await
    .expect("one bounded authorization covers the two isolated targets");

    let first_root = request_progressive_archive_and_lease(
        &database,
        first_target,
        "批量滚动容量证明",
        "person",
        30,
    )
    .await
    .expect("first root is leased");
    let first_root_ref = first_root
        .request
        .work_order_ref
        .expect("first root reference");
    complete_progressive_root_at_the_200_work_bound(&database, &first_installation).await;
    let second_root = request_progressive_archive_and_lease(
        &database,
        second_target,
        "批量滚动容量证明",
        "person",
        30,
    )
    .await
    .expect("second root is leased after the first completes");
    let second_root_ref = second_root
        .request
        .work_order_ref
        .expect("second root reference");
    complete_progressive_root_at_the_200_work_bound(&database, &first_installation).await;

    let summary = run_progressive_archives(&database)
        .await
        .expect("scheduler produces bounded child batches from accepted directories");
    assert_eq!(
        summary.queued,
        vec![
            first_target,
            second_target,
            first_target,
            second_target,
            first_target,
            second_target,
            first_target,
            second_target,
        ],
        "two ready claimants and multiplier two yield four batches per source in durable round-robin order",
    );

    let children: Vec<(Uuid, String, i64)> = sqlx::query_as(
        "SELECT work_order.target_ref,work_order.dispatch_group_key,count(scope.content_public_ref) \
         FROM collection_work_order work_order \
         JOIN collection_work_order_material_target scope USING(work_order_ref) \
         WHERE work_order.target_ref IN ($1,$2) \
           AND work_order.work_order_ref<>ALL(ARRAY[$3,$4]::uuid[]) \
           AND work_order.queue_state='queued' \
         GROUP BY work_order.target_ref,work_order.dispatch_group_key \
         ORDER BY work_order.target_ref",
    )
    .bind(first_target)
    .bind(second_target)
    .bind(first_root_ref)
    .bind(second_root_ref)
    .fetch_all(database.pool())
    .await
    .expect("queued child WorkOrders are inspectable");
    assert_eq!(children.len(), 2);
    for (target_ref, dispatch_group_key, scoped_work_count) in children {
        assert!(matches!(target_ref, value if value == first_target || value == second_target));
        assert_eq!(dispatch_group_key, format!("target:{target_ref}"));
        assert_eq!(
            scoped_work_count, 12,
            "each source has four 3-work batches, never an unbounded material backlog"
        );
    }
    let considered_target_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_observation_target \
         WHERE target_ref IN ($1,$2) AND last_scheduler_considered_at IS NOT NULL",
    )
    .bind(first_target)
    .bind(second_target)
    .fetch_one(database.pool())
    .await
    .expect("each source records its scheduler-fairness turn");
    assert_eq!(considered_target_count, 2);
    let accepted_authorizations: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT decision.authorization_ref \
         FROM collection_work_order work_order \
         JOIN collection_admission_decision decision USING(decision_ref) \
         WHERE work_order.target_ref IN ($1,$2) \
           AND work_order.work_order_ref<>ALL(ARRAY[$3,$4]::uuid[]) \
           AND work_order.queue_state='queued'",
    )
    .bind(first_target)
    .bind(second_target)
    .bind(first_root_ref)
    .bind(second_root_ref)
    .fetch_all(database.pool())
    .await
    .expect("child batches retain their bounded authorization provenance");
    assert_eq!(accepted_authorizations, vec![authorization_ref]);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn concurrent_continue_reuses_one_root_and_freezes_one_three_work_child_under_renewed_authorization()
 {
    let database = proof_database("dossier_progressive_continue_mutex").await;
    let installation = ready_installation(&database, "dossier-continue-mutex").await;
    let target_ref = seed_creator_target(&database, "creator-continue-mutex").await;
    let original_authorization = grant_deep_archive(&database, "建立创作者档案", 200).await;
    let original = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap();
    let original_root = original.request.work_order_ref.unwrap();
    complete_progressive_root_at_the_200_work_bound(&database, &installation).await;
    sqlx::query(
        "UPDATE collection_acquisition_authorization \
         SET granted_at=scope_001_now()-interval '2 days', \
             expires_at=scope_001_now()-interval '1 second' WHERE authorization_ref=$1",
    )
    .bind(original_authorization)
    .execute(database.pool())
    .await
    .unwrap();
    let renewed_authorization = grant_deep_archive(&database, "建立创作者档案", 200).await;

    let (left, right) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(
            request_progressive_archive_and_lease(
                &database,
                target_ref,
                "建立创作者档案",
                "person",
                30,
            ),
            request_progressive_archive_and_lease(
                &database,
                target_ref,
                "建立创作者档案",
                "person",
                30,
            )
        )
    })
    .await
    .expect("target-to-authorization lock ordering must not deadlock");
    assert_eq!(
        [left.as_ref(), right.as_ref()]
            .into_iter()
            .filter(|outcome| outcome.is_ok_and(|value| value.lease.is_some()))
            .count(),
        1,
        "exactly one concurrent continue action may freeze a live child batch",
    );

    let root_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order \
         WHERE target_ref=$1 AND lane='deep_archive' \
           AND stop_conditions #>> '{progressiveArchive,version}'='1' \
           AND stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=work_order_ref::text",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        root_count, 1,
        "continue must not rescan the creator profile"
    );
    let child: (Uuid, Uuid, i32, i64) = sqlx::query_as(
        "SELECT work_order.work_order_ref,decision.authorization_ref,work_order.max_works, \
                count(scope.content_public_ref) \
         FROM collection_work_order work_order \
         JOIN collection_admission_decision decision USING(decision_ref) \
         JOIN collection_work_order_material_target scope USING(work_order_ref) \
         WHERE work_order.target_ref=$1 \
           AND work_order.stop_conditions #>> '{progressiveArchive,rootWorkOrderRef}'=$2 \
           AND work_order.work_order_ref<>$3 \
         GROUP BY work_order.work_order_ref,decision.authorization_ref,work_order.max_works",
    )
    .bind(target_ref)
    .bind(original_root.to_string())
    .bind(original_root)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(child.1, renewed_authorization);
    assert_eq!(child.2, 3);
    assert_eq!(child.3, 3);
    let child_capabilities: Vec<String> = sqlx::query_scalar(
        "SELECT task.task_spec #>> '{capabilitiesRequested,0}' \
         FROM collection_work_order_lease lease \
         JOIN collection_work_order_lease_task lease_task USING(lease_ref) \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         WHERE lease.work_order_ref=$1 ORDER BY lease_task.sequence_no",
    )
    .bind(child.0)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(child_capabilities.len(), 12);
    assert!(
        child_capabilities.iter().all(|capability| !matches!(
            capability.as_str(),
            "author_profile" | "profile_discovery"
        ))
    );
}

/// **授权不按目的文本逐字比对。**
///
/// 签一份额度够的 `deep_archive` 授权、请求时写另一句目的，渐进建档必须照常开始。此前这里
/// 有 `AND purpose=$3`，而同一条链另一头的 `gather_facts` 从来不看目的（形参就叫 `_purpose`）。
/// 两处判据不一致的后果是：授权签了、额度也够，界面却说「没有匹配的授权」——差别只是那一串
/// 文本不一字不差。2026-09-04 为此卡了一天。
///
/// 授权真正管的东西（lane、平台、目标类型、额度、撤销）照旧全查，下一条断言证明这一点。
#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn progressive_archive_does_not_require_the_purpose_text_to_match_the_grant() {
    let database = proof_database("dossier_progressive_purpose_text").await;
    ready_installation(&database, "dossier-purpose-text").await;
    let target_ref = seed_creator_target(&database, "creator-purpose-text").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;

    // 请求写的是另一句目的。目的说明的是「为什么观察」，不是授权的身份。
    let result = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "看看这个博主最近在讲什么",
        "person",
        30,
    )
    .await;
    assert!(
        result.is_ok(),
        "额度够的授权在手，渐进建档不该因为目的文本不同而被拒：{result:?}"
    );
    let work_orders: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
            .bind(target_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(work_orders, 1, "它该真的排出一张工单");
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn progressive_archive_rejects_a_smaller_grant_without_writing_a_request_or_work() {
    let database = proof_database("dossier_progressive_narrow_grant").await;
    let target_ref = seed_creator_target(&database, "creator-narrow-grant").await;
    grant_deep_archive(&database, "建立创作者档案", 199).await;

    let result = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await;
    assert!(matches!(
        result,
        Err(RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { current_bound: 199 }
        ))
    ));

    let counts: (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT \
           (SELECT count(*) FROM collection_acquisition_request WHERE target_ref=$1), \
           (SELECT count(*) FROM collection_admission_decision decision \
              JOIN collection_acquisition_request request USING (request_ref) \
              WHERE request.target_ref=$1), \
           (SELECT count(*) FROM collection_work_order WHERE target_ref=$1), \
           (SELECT count(*) FROM collection_work_order_lease lease \
              JOIN collection_work_order work_order USING (work_order_ref) \
              WHERE work_order.target_ref=$1)",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
    let lifecycle_state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(lifecycle_state, "pending_decision");
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn archive_completeness_distinguishes_started_attempted_and_quarantined_from_untouched() {
    let database = proof_database("dossier_archive_started_attempted").await;
    ready_installation(&database, "dossier-started-attempted").await;
    let target_ref = seed_creator_target(&database, "creator-started-attempted").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let root = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap();
    let lease_ref = root.lease.unwrap().lease_ref;

    let started = read_archive_completeness(&database, "xhs").await.unwrap();
    let started = started.get("creator-started-attempted").unwrap();
    assert!(started.started);
    assert!(!started.attempted);
    assert!(!started.is_untouched());

    let author_task_id: Uuid = sqlx::query_scalar(
        "SELECT lease_task.task_id FROM collection_work_order_lease_task lease_task \
         JOIN linggan_runtime_task task ON task.task_id=lease_task.task_id \
         WHERE lease_task.lease_ref=$1 \
           AND task.task_spec #>> '{capabilitiesRequested,0}'='author_profile'",
    )
    .bind(lease_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let package_ref = seed_runtime_package(
        &database,
        author_task_id,
        "author_profile",
        "2026-09-04T01:00:00Z",
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         VALUES ($1,0,'quarantined','started/attempted proof')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let attempted = read_archive_completeness(&database, "xhs").await.unwrap();
    let attempted = attempted.get("creator-started-attempted").unwrap();
    assert!(attempted.started);
    assert!(attempted.attempted);
    assert_eq!(attempted.quarantined, 1);
    assert!(!attempted.is_untouched());
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn archive_completeness_exposes_a_blocked_detail_as_a_problem_without_counting_it_as_captured()
 {
    let database = proof_database("dossier_archive_blocked_detail").await;
    let installation = ready_installation(&database, "dossier-blocked-detail").await;
    let target_ref = seed_creator_target(&database, "creator-blocked-detail").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    request_progressive_archive_and_lease(&database, target_ref, "建立创作者档案", "person", 30)
        .await
        .unwrap();
    complete_progressive_root_at_the_200_work_bound(&database, &installation).await;
    let child = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .expect("a bounded detail child is leased from the accepted current directory");
    let lease_ref = child.lease.unwrap().lease_ref;
    let child_work_order_ref = child.request.work_order_ref.unwrap();
    let blocked_material_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT content_public_ref FROM collection_work_order_material_target \
         WHERE work_order_ref=$1 ORDER BY ordinal",
    )
    .bind(child_work_order_ref)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(blocked_material_refs.len(), 3);
    // The dispatch proof owns the transition. This read-model proof uses a
    // genuine frozen detail child produced by progressive archiving and sets
    // only its mutable execution state; Runtime TaskSpec remains immutable.
    sqlx::query(
        "UPDATE collection_work_order_lease_task \
         SET execution_state='blocked',claimed_at=NULL,claimed_by_installation_ref=NULL \
         WHERE lease_ref=$1",
    )
    .bind(lease_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE collection_work_order_lease \
         SET released_at=scope_001_now(),release_reason='partial' WHERE lease_ref=$1",
    )
    .bind(lease_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE collection_work_order SET queue_state='completed' WHERE work_order_ref=$1")
        .bind(child_work_order_ref)
        .execute(database.pool())
        .await
        .unwrap();

    let completeness = read_archive_completeness(&database, "xhs").await.unwrap();
    let completeness = completeness.get("creator-blocked-detail").unwrap();
    assert_eq!(completeness.blocked_details, 3);
    assert_eq!(completeness.details_captured, 0);
    assert!(completeness.has_actionable_problems());
    assert!(!completeness.is_untouched());

    let summary = run_progressive_archives(&database)
        .await
        .expect("the automatic worker can continue with other accepted material");
    assert!(summary.queued.contains(&target_ref));
    let reintroduced: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order work_order \
         JOIN collection_work_order_material_target scope USING(work_order_ref) \
         WHERE work_order.target_ref=$1 AND work_order.queue_state='queued' \
           AND scope.content_public_ref=ANY($2)",
    )
    .bind(target_ref)
    .bind(&blocked_material_refs)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        reintroduced, 0,
        "automatic progressive scheduling never silently revives blocked details"
    );
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn archive_completeness_deduplicates_work_and_follows_the_exact_lease_target_chain() {
    let database = proof_database("dossier_archive_completeness").await;
    ready_installation(&database, "dossier-completeness").await;
    let target_ref = seed_creator_target(&database, "creator-completeness").await;
    grant_deep_archive(&database, "建立创作者档案", 200).await;
    let outcome = request_progressive_archive_and_lease(
        &database,
        target_ref,
        "建立创作者档案",
        "person",
        30,
    )
    .await
    .unwrap();
    let work_order_ref = outcome.request.work_order_ref.unwrap();
    let lease_ref = outcome.lease.unwrap().lease_ref;
    let profile_task_id: Uuid = sqlx::query_scalar(
        "SELECT task.task_id FROM collection_work_order_lease_task lease_task \
         JOIN linggan_runtime_task task USING (task_id) \
         WHERE lease_task.lease_ref=$1 \
           AND task.task_spec #>> '{capabilitiesRequested,0}'='profile_discovery'",
    )
    .bind(lease_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let discovery_package = seed_runtime_package(
        &database,
        profile_task_id,
        "profile_discovery",
        "2026-09-03T09:00:00Z",
    )
    .await;
    mark_bound_task_completed(&database, profile_task_id).await;

    let work_ref = Uuid::new_v4();
    for ordinal in [0_i32, 1_i32] {
        sqlx::query(
            "INSERT INTO linggan_runtime_record_disposition \
               (package_ref,record_ordinal,disposition,reason) \
             VALUES ($1,$2,'accepted_for_library_discovery','dossier dedupe proof')",
        )
        .bind(discovery_package)
        .bind(ordinal)
        .execute(database.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO linggan_material_content \
           (platform,content_external_id,public_ref,first_package_ref) \
         VALUES ('xhs','stable-work-one',$1,$2)",
    )
    .bind(work_ref)
    .bind(discovery_package)
    .execute(database.pool())
    .await
    .unwrap();
    for ordinal in [0_i32, 1_i32] {
        sqlx::query(
            "INSERT INTO linggan_material_discovery_finding \
               (material_ref,content_public_ref,package_ref,record_ordinal,discovery_kind, \
                result_position,observed_at,title,title_state,creator_display_name,creator_state, \
                published_at_source_text,published_at_source_text_state) \
             VALUES ($1,$2,$3,$4,'profile_discovery',$5,'2026-09-03T09:00:00Z', \
                     '同一稳定作品','KNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN')",
        )
        .bind(Uuid::new_v4())
        .bind(work_ref)
        .bind(discovery_package)
        .bind(ordinal)
        .bind(ordinal + 1)
        .execute(database.pool())
        .await
        .unwrap();
    }
    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
           (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit, \
            acquire_media,allow_ocr,allow_asr) \
         VALUES ($1,$2,1,30,2,true,true,true)",
    )
    .bind(work_order_ref)
    .bind(work_ref)
    .execute(database.pool())
    .await
    .unwrap();

    for (sequence_no, accepted_at) in [
        (3_i32, "2026-09-03T10:00:00Z"),
        (4_i32, "2026-09-03T11:00:00Z"),
    ] {
        let detail_task_id = seed_bound_task(
            &database,
            lease_ref,
            sequence_no,
            "content_detail",
            serde_json::json!({"contentExternalId":"stable-work-one"}),
        )
        .await;
        let detail_package =
            seed_runtime_package(&database, detail_task_id, "content_detail", accepted_at).await;
        sqlx::query(
            "INSERT INTO linggan_runtime_record_disposition \
               (package_ref,record_ordinal,disposition,reason) \
             VALUES ($1,0,'accepted_for_library_content','material deepening proof')",
        )
        .bind(detail_package)
        .execute(database.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO linggan_material_content_detail \
               (material_ref,content_public_ref,package_ref,record_ordinal,observed_at, \
                title,title_state,body_text,body_state,creator_display_name, \
                creator_display_name_state,published_at_source_text, \
                published_at_source_text_state,searchable_text) \
             VALUES ($1,$2,$3,0,$4,'稳定作品详情','KNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN', \
                     NULL,'UNKNOWN','稳定作品详情')",
        )
        .bind(Uuid::new_v4())
        .bind(work_ref)
        .bind(detail_package)
        .bind(accepted_at)
        .execute(database.pool())
        .await
        .unwrap();
    }

    let detail_task_has_no_author: bool = sqlx::query_scalar(
        "SELECT bool_and(task.task_spec #>> '{target,authorExternalId}' IS NULL) \
         FROM collection_work_order_lease_task lease_task \
         JOIN linggan_runtime_task task USING (task_id) \
         WHERE lease_task.lease_ref=$1 \
           AND task.task_spec #>> '{capabilitiesRequested,0}'='content_detail'",
    )
    .bind(lease_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(detail_task_has_no_author);

    let projection = read_archive_completeness(&database, "xhs").await.unwrap();
    let completeness = projection
        .get("creator-completeness")
        .expect("the exact Work Order target owns every bound package");
    assert!(completeness.work_in_progress);
    assert_eq!(completeness.author_profile_captures, 0);
    assert_eq!(
        completeness.works_listed, 0,
        "a still-building root does not expose partial links as a current directory"
    );
    assert_eq!(
        completeness.details_captured, 0,
        "details are not a visible denominator before the current directory is proven"
    );
    assert_eq!(completeness.quarantined, 0);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn lifecycle_distinguishes_directory_confirmed_and_latest_patrol_points() {
    let database = proof_database("dossier_lifecycle_association").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
         AS $$ SELECT timestamptz '2026-09-04T12:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let target_ref = seed_creator_target(&database, "creator-lifecycle-contract").await;
    let station_ref = register_station(&database, "lifecycle-history", 200)
        .await
        .unwrap();
    let old_lease =
        seed_historical_patrol(&database, target_ref, station_ref, "2026-09-01T08:00:00Z").await;
    let latest_lease =
        seed_historical_patrol(&database, target_ref, station_ref, "2026-09-03T08:00:00Z").await;

    let old_directory_ref = seed_surface_work(
        &database,
        old_lease,
        1,
        "creator-lifecycle-contract",
        "old-directory-work",
        "2026-09-01T09:00:00Z",
    )
    .await;
    seed_detail_observation(
        &database,
        old_directory_ref,
        "old-directory-work",
        None,
        11,
        "2026-08-01T00:00:00Z",
    )
    .await;

    let latest_directory_ref = seed_surface_work(
        &database,
        latest_lease,
        1,
        "creator-lifecycle-contract",
        "latest-directory-work",
        "2026-09-03T09:00:00Z",
    )
    .await;
    seed_detail_observation(
        &database,
        latest_directory_ref,
        "latest-directory-work",
        None,
        23,
        "2026-08-02T00:00:00Z",
    )
    .await;

    let confirmed_ref = Uuid::new_v4();
    seed_unbound_work(&database, confirmed_ref, "confirmed-work").await;
    seed_detail_observation(
        &database,
        confirmed_ref,
        "confirmed-work",
        Some("creator-lifecycle-contract"),
        37,
        "2026-08-03T00:00:00Z",
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
    assert_eq!(projection.status, CreatorLifecycleStatus::Ready);
    assert_eq!(projection.summary.linked_work_count, Some(3));
    assert_eq!(projection.summary.confirmed_author_work_count, 1);
    assert_eq!(projection.summary.eligible_point_count, 3);
    assert_eq!(projection.exclusions.author_not_verified, 0);

    let old_directory = projection
        .points
        .iter()
        .find(|point| point.work_public_ref == old_directory_ref)
        .unwrap();
    assert_eq!(
        old_directory.association_state,
        CreatorLifecycleAssociation::DirectoryLinked
    );
    assert!(!old_directory.new_in_latest_patrol);

    let latest_directory = projection
        .points
        .iter()
        .find(|point| point.work_public_ref == latest_directory_ref)
        .unwrap();
    assert_eq!(
        latest_directory.association_state,
        CreatorLifecycleAssociation::DirectoryLinked
    );
    assert!(latest_directory.new_in_latest_patrol);

    let confirmed = projection
        .points
        .iter()
        .find(|point| point.work_public_ref == confirmed_ref)
        .unwrap();
    assert_eq!(
        confirmed.association_state,
        CreatorLifecycleAssociation::AuthorConfirmed
    );
    assert!(!confirmed.new_in_latest_patrol);
}

#[tokio::test]
#[ignore = "requires a disposable PostgreSQL 16 proof database"]
async fn patrol_success_and_latest_new_ring_share_one_qualified_target_level_round() {
    let database = proof_database("dossier_patrol_success_round").await;
    set_proof_clock(&database, "2026-09-04T08:00:00Z").await;
    let installation = ready_installation(&database, "dossier-patrol-success").await;
    refresh_installation_at_proof_clock(&database, &installation).await;
    let target_ref = seed_creator_target(&database, "creator-patrol-success").await;
    sqlx::query(
        "UPDATE collection_observation_target SET lifecycle_state='archived' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();
    grant_patrol(&database, "巡查建档创作者").await;

    submit_patrol_round(
        &database,
        &installation,
        target_ref,
        "巡查建档创作者",
        PatrolRound::OneUsableWork,
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE platform='xhs' AND content_external_id='creator-patrol-success-new-work'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_detail_observation(
        &database,
        work_ref,
        "creator-patrol-success-new-work",
        Some("creator-patrol-success"),
        31,
        "2026-08-03T00:00:00Z",
    )
    .await;

    set_proof_clock(&database, "2026-09-04T09:00:00Z").await;
    refresh_installation_at_proof_clock(&database, &installation).await;
    submit_patrol_round(
        &database,
        &installation,
        target_ref,
        "巡查建档创作者",
        PatrolRound::ValidZeroNew,
    )
    .await;
    let after_zero: String = sqlx::query_scalar(
        "SELECT to_char(last_patrol_succeeded_at,'YYYY-MM-DD HH24:MI') \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    // 同一时刻按北京时间读出（会话时区固定 Asia/Shanghai）。
    assert_eq!(after_zero, "2026-09-04 17:00");
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
    .unwrap();
    assert!(
        lifecycle
            .points
            .iter()
            .all(|point| !point.new_in_latest_patrol),
        "a valid zero-new patrol is the latest successful round and clears the old new-work ring",
    );

    for (at, round) in [
        ("2026-09-04T10:00:00Z", PatrolRound::AllQuarantined),
        ("2026-09-04T11:00:00Z", PatrolRound::UnknownRiskStop),
    ] {
        set_proof_clock(&database, at).await;
        refresh_installation_at_proof_clock(&database, &installation).await;
        submit_patrol_round(
            &database,
            &installation,
            target_ref,
            "巡查建档创作者",
            round,
        )
        .await;
    }
    let after_unqualified: String = sqlx::query_scalar(
        "SELECT to_char(last_patrol_succeeded_at,'YYYY-MM-DD HH24:MI') \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        after_unqualified, "2026-09-04 17:00",
        "quarantined and unknown/risk-stopped patrols complete their leases without claiming success",
    );
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
    .unwrap();
    assert!(
        lifecycle
            .points
            .iter()
            .all(|point| !point.new_in_latest_patrol)
    );
}

async fn seed_creator_target(database: &Database, identity_key: &str) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
           (target_ref,platform,target_kind,identity_key,display_name,source,domain_ref) \
         VALUES ($1,'xhs','creator',$2,$2,'manual','00000000-0000-4000-8000-000000000001')",
    )
    .bind(target_ref)
    .bind(identity_key)
    .execute(database.pool())
    .await
    .unwrap();
    target_ref
}

async fn grant_deep_archive(database: &Database, purpose: &str, max_works: i32) -> Uuid {
    grant_authorization(
        database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "deep_archive",
            purpose,
            max_targets: Some(1),
            max_works_per_target: Some(max_works),
            valid_for_days: 1,
        },
    )
    .await
    .unwrap()
}

async fn grant_patrol(database: &Database, purpose: &str) -> Uuid {
    grant_authorization(
        database,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "creator",
            lane: "patrol",
            purpose,
            max_targets: Some(1),
            max_works_per_target: Some(20),
            valid_for_days: 1,
        },
    )
    .await
    .unwrap()
}

async fn set_proof_clock(database: &Database, at: &str) {
    let sql: &'static str = match at {
        "2026-09-04T08:00:00Z" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
             AS $$ SELECT timestamptz '2026-09-04T08:00:00Z' $$"
        }
        "2026-09-04T09:00:00Z" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
             AS $$ SELECT timestamptz '2026-09-04T09:00:00Z' $$"
        }
        "2026-09-04T10:00:00Z" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
             AS $$ SELECT timestamptz '2026-09-04T10:00:00Z' $$"
        }
        "2026-09-04T11:00:00Z" => {
            "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
             AS $$ SELECT timestamptz '2026-09-04T11:00:00Z' $$"
        }
        other => panic!("unsupported fixed proof clock {other}"),
    };
    sqlx::query(sql).execute(database.pool()).await.unwrap();
}

#[derive(Clone, Copy)]
enum PatrolRound {
    OneUsableWork,
    ValidZeroNew,
    AllQuarantined,
    UnknownRiskStop,
}

async fn submit_patrol_round(
    database: &Database,
    installation: &Installed,
    target_ref: Uuid,
    purpose: &str,
    round: PatrolRound,
) {
    let request = request_admit_and_lease(database, target_ref, "patrol", purpose, "person", 30)
        .await
        .unwrap();
    assert!(
        request.lease.is_some(),
        "patrol must be leased: {request:?}"
    );
    let producer_instance_id = Uuid::parse_str(&installation.install_key).unwrap();
    // A creator patrol is deliberately two browser steps.  The author-profile receipt must
    // complete before the same Lease exposes the profile-discovery task that supplies this
    // round's Work cards; never submit a discovery package against the author task.
    let (task, attempt) = loop {
        let decision = decide_dispatch(database, &installation.install_key, &installation.secret)
            .await
            .unwrap();
        let task_spec = match decision {
            linggan_evidence::DispatchDecision::Dispatch { task_spec, .. } => task_spec,
            other => panic!("expected a dispatched patrol task, got {other:?}"),
        };
        let task = parse_producer_task_spec(&task_spec.to_string()).unwrap();
        let attempt = parse_producer_attempt(
            &serde_json::json!({
                "contractVersion":"linggan.producer.attempt.v1",
                "producerInstanceId":producer_instance_id,
                "taskId":task.task_id(),
                "attemptId":Uuid::new_v4(),
            })
            .to_string(),
        )
        .unwrap();
        assert!(matches!(
            start_producer_attempt(database, &attempt).await,
            Ok(RuntimeAttemptOutcome::Started { .. })
        ));
        let capability = task.raw()["capabilitiesRequested"][0].as_str().unwrap();
        if capability == "author_profile" {
            let author_submission = bounded_root_submission(&task, &attempt, producer_instance_id);
            assert!(matches!(
                submit_producer_package(database, &author_submission).await,
                Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
            ));
            continue;
        }
        assert_eq!(capability, "profile_discovery");
        break (task, attempt);
    };
    let identity = task.raw()["target"]["authorExternalId"].as_str().unwrap();
    let (observed, attempted, acquired, unknown, stopped_reason, records) = match round {
        PatrolRound::OneUsableWork => (
            1,
            1,
            1,
            0,
            "surface_ended",
            vec![serde_json::json!({
                "kind":"profile_discovery_card",
                "resultPosition":1,
                "sourceObject":{
                    "platform":"xhs","type":"content",
                    "externalId":"creator-patrol-success-new-work"
                },
                "payload":{"title":"patrol success work"}
            })],
        ),
        PatrolRound::ValidZeroNew => (0, 0, 0, 0, "surface_ended", Vec::new()),
        PatrolRound::AllQuarantined => (
            1,
            1,
            1,
            0,
            "surface_ended",
            vec![serde_json::json!({
                "kind":"profile_discovery_card",
                "resultPosition":1,
                "sourceObject":{
                    "platform":"xhs","type":"author","externalId":"invalid-discovery-identity"
                },
                "payload":{"title":"must quarantine"}
            })],
        ),
        PatrolRound::UnknownRiskStop => (1, 1, 0, 1, "risk_budget", Vec::new()),
    };
    let submission = parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"profile_discovery",
                "platform":"xhs",
                "observedAt":"2026-09-04T00:00:00Z",
                "capturedAt":"2026-09-04T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":identity},
                    "layers":[{
                        "capability":"profile_discovery",
                        "observed":observed,"attempted":attempted,"acquired":acquired,
                        "verified":0,"failed":0,"notAttempted":0,"unknown":unknown,
                        "stoppedReason":stopped_reason
                    }]
                },
                "checkpoint":surface_receipt(stopped_reason),
                "records":records
            }
        })
        .to_string(),
    )
    .unwrap();
    let outcome = submit_producer_package(database, &submission).await;
    assert!(
        matches!(outcome, Ok(RuntimeSubmissionOutcome::Acknowledged { .. })),
        "patrol submission must be durably acknowledged: {outcome:?}",
    );
}

#[derive(Debug)]
struct Installed {
    installation_ref: Uuid,
    account_ref: Uuid,
    install_key: String,
    secret: String,
}

async fn ready_installation(database: &Database, label: &str) -> Installed {
    let station_ref = register_station(database, label, 1_000).await.unwrap();
    open_claim_window(database, station_ref, 1).await.unwrap();
    let install_key = Uuid::new_v4().to_string();
    let outcome = check_in_installation(
        database,
        &InstallationCheckIn {
            install_key: &install_key,
            installation_credential: None,
            plugin_version: "0.8.47",
            browser_label: Some(label),
            capabilities: serde_json::json!([
                "author_profile",
                "profile_discovery",
                "content_detail",
                "comments",
                "replies",
                "media_slots"
            ]),
        },
    )
    .await
    .unwrap();
    let (installation_ref, credential_ref, secret) = match outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            credential: Some(issued),
            ..
        } => (
            installation_ref,
            issued.credential_ref,
            issued.raw_credential.expose_once().to_owned(),
        ),
        other => panic!("the compatible installation must be claimed: {other:?}"),
    };
    activate_installation_credential(database, installation_ref, credential_ref, &secret)
        .await
        .unwrap();
    set_station_accepting(database, station_ref, true, "person")
        .await
        .unwrap();
    let receipt = report_account_eligibility(
        database,
        installation_ref,
        &secret,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: &format!("{label}-account"),
        },
        Some(DIGEST_KEY),
    )
    .await
    .unwrap();
    let account_ref = receipt.account_ref.unwrap();
    bind_observation_account(database, account_ref, installation_ref, "person")
        .await
        .unwrap();
    Installed {
        installation_ref,
        account_ref,
        install_key,
        secret,
    }
}

async fn refresh_installation_at_proof_clock(database: &Database, installation: &Installed) {
    sqlx::query(
        "UPDATE plugin_installation SET last_seen_at=scope_001_now() WHERE installation_ref=$1",
    )
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE platform_observation_account_eligibility_observation \
         SET observed_at=scope_001_now(),expires_at=scope_001_now()+interval '20 minutes' \
         WHERE account_ref=$1 AND installation_ref=$2",
    )
    .bind(installation.account_ref)
    .bind(installation.installation_ref)
    .execute(database.pool())
    .await
    .unwrap();
}

async fn complete_progressive_root_at_the_200_work_bound(
    database: &Database,
    installation: &Installed,
) {
    for _ in 0..2 {
        let decision = decide_dispatch(database, &installation.install_key, &installation.secret)
            .await
            .unwrap();
        let task_spec = match decision {
            linggan_evidence::DispatchDecision::Dispatch { task_spec, .. } => task_spec,
            other => panic!("expected a dispatched progressive root task, got {other:?}"),
        };
        let task = parse_producer_task_spec(&task_spec.to_string()).unwrap();
        let producer_instance_id = Uuid::parse_str(&installation.install_key).unwrap();
        let attempt = parse_producer_attempt(
            &serde_json::json!({
                "contractVersion":"linggan.producer.attempt.v1",
                "producerInstanceId":producer_instance_id,
                "taskId":task.task_id(),
                "attemptId":Uuid::new_v4(),
            })
            .to_string(),
        )
        .unwrap();
        assert!(matches!(
            start_producer_attempt(database, &attempt).await,
            Ok(RuntimeAttemptOutcome::Started { .. })
        ));
        let submission = bounded_root_submission(&task, &attempt, producer_instance_id);
        let outcome = submit_producer_package(database, &submission).await;
        assert!(
            matches!(outcome, Ok(RuntimeSubmissionOutcome::Acknowledged { .. })),
            "bounded root submission must be acknowledged: {outcome:?}",
        );
    }
}

async fn complete_progressive_root_with_partial_directory(
    database: &Database,
    installation: &Installed,
    directory_size: i64,
    stopped_reason: &str,
) {
    complete_progressive_root_with_directory(database, installation, directory_size, stopped_reason, true)
        .await;
}

/// `signed_links=false` 时目录成员没有可执行地址——用来证明这类成员不会进候选。
async fn complete_progressive_root_with_directory(
    database: &Database,
    installation: &Installed,
    directory_size: i64,
    stopped_reason: &str,
    signed_links: bool,
) {
    for _ in 0..2 {
        let decision = decide_dispatch(database, &installation.install_key, &installation.secret)
            .await
            .unwrap();
        let task_spec = match decision {
            linggan_evidence::DispatchDecision::Dispatch { task_spec, .. } => task_spec,
            other => panic!("expected a dispatched progressive root task, got {other:?}"),
        };
        let task = parse_producer_task_spec(&task_spec.to_string()).unwrap();
        let producer_instance_id = Uuid::parse_str(&installation.install_key).unwrap();
        let attempt = parse_producer_attempt(
            &serde_json::json!({
                "contractVersion":"linggan.producer.attempt.v1",
                "producerInstanceId":producer_instance_id,
                "taskId":task.task_id(),
                "attemptId":Uuid::new_v4(),
            })
            .to_string(),
        )
        .unwrap();
        assert!(matches!(
            start_producer_attempt(database, &attempt).await,
            Ok(RuntimeAttemptOutcome::Started { .. })
        ));
        let submission = if task.raw()["capabilitiesRequested"][0] == "profile_discovery" {
            partial_directory_submission(
                &task,
                &attempt,
                producer_instance_id,
                directory_size,
                stopped_reason,
                signed_links,
            )
        } else {
            bounded_root_submission(&task, &attempt, producer_instance_id)
        };
        let outcome = submit_producer_package(database, &submission).await;
        assert!(
            matches!(outcome, Ok(RuntimeSubmissionOutcome::Acknowledged { .. })),
            "partial root submission must remain accepted history: {outcome:?}",
        );
    }
}

/// 这一篇的补详情子工单：根之外、带着材料范围的那一张。
async fn pending_detail_batch(database: &Database, target_ref: Uuid, root: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT work_order.work_order_ref FROM collection_work_order work_order \
         JOIN collection_work_order_material_target scope USING(work_order_ref) \
         WHERE work_order.target_ref=$1 AND work_order.work_order_ref<>$2 \
         ORDER BY work_order.created_at DESC,work_order.work_order_ref DESC LIMIT 1",
    )
    .bind(target_ref)
    .bind(root)
    .fetch_one(database.pool())
    .await
    .expect("补详情的子工单是系统自己排出来的，不是用例摆出来的")
}

/// 退避是真实写进工单的，所以这里显式跨过它——跨的是时钟，不是把退避抹掉。
async fn clear_dispatch_backoff(database: &Database, work_order_ref: Uuid) {
    sqlx::query(
        "UPDATE collection_work_order SET retry_not_before_at=scope_001_now()-interval '1 second' \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .execute(database.pool())
    .await
    .expect("退避只是等待，不是停止");
}

/// 在指定的那张工单上派一次 `content_detail`，再如实上报一次页面读失败。
///
/// 先认工单、再认能力，最后认作品：三条里错一条，这次失败就会记到别的缺口上，而这条用例的
/// 全部结论都建立在「失败确实记在这一篇上」。
async fn report_detail_read_failure(
    database: &Database,
    installation: &Installed,
    work_order_ref: Uuid,
    content_external_id: &str,
) -> DispatchFailureOutcome {
    let decision = decide_dispatch(database, &installation.install_key, &installation.secret)
        .await
        .expect("证明库会把这张补详情的子工单派出去");
    match decision {
        DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
            ..
        } => {
            let leased_work_order: Uuid = sqlx::query_scalar(
                "SELECT work_order_ref FROM collection_work_order_lease WHERE lease_ref=$1",
            )
            .bind(lease_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
            assert_eq!(
                leased_work_order, work_order_ref,
                "这一步要报的是这张工单上的失败"
            );
            assert_eq!(
                task_spec["capabilitiesRequested"][0].as_str(),
                Some("content_detail")
            );
            assert_eq!(
                task_spec["target"]["contentExternalId"].as_str(),
                Some(content_external_id)
            );
            requeue_failed_dispatch(
                database,
                &installation.install_key,
                &installation.secret,
                task_id,
                Uuid::new_v4(),
                DispatchFailureCode::PageReadFailed,
            )
            .await
            .expect("页面读失败的上报被接受")
        }
        other => panic!("expected a dispatched detail task, got {other:?}"),
    }
}

/// `signed_links=false` 是**真实存在的一种目录**：平台在某些卡片上不给带 `xsec_token` 的链接，
/// 插件照实上报。这种成员有没有执行输入，由候选判据回答，不由夹具替它回答。
fn partial_directory_submission(
    task: &ProducerTaskSpec,
    attempt: &linggan_contracts::ProducerAttempt,
    producer_instance_id: Uuid,
    directory_size: i64,
    stopped_reason: &str,
    signed_links: bool,
) -> linggan_contracts::ProducerSubmission {
    let identity = task.raw()["target"]["authorExternalId"].as_str().unwrap();
    let records = (0..directory_size)
        .map(|ordinal| {
            let external_id = format!("{identity}-partial-{ordinal}");
            let mut payload = serde_json::json!({"title":format!("partial work {ordinal}")});
            if signed_links {
                // 真实插件的发现卡带的就是这条签名链接，补详情要靠它当执行入口。夹具里少了
                // 它，目录成员一到派发就被停成「缺执行输入」——补详情那条链一步都走不出去，
                // 拿它写的用例会以为自己测的是别的东西。
                payload["url"] = serde_json::Value::String(format!(
                    "https://www.xiaohongshu.com/explore/{external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user"
                ));
            }
            serde_json::json!({
                "kind":"profile_discovery_card",
                "resultPosition":ordinal + 1,
                "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
                "payload":payload
            })
        })
        .collect::<Vec<_>>();
    parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"profile_discovery",
                "platform":"xhs",
                "observedAt":"2026-09-04T00:00:00Z",
                "capturedAt":"2026-09-04T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":identity},
                    "layers":[{
                        "capability":"profile_discovery",
                        "observed":directory_size,"attempted":directory_size,"acquired":directory_size,
                        // 真实插件在这两个字段里写的就是这个：一个不携带信息的常量，和一个
                        // 恒不为零的 `unknown`（「可见页面不等于完整结果集」）。判据不该看
                        // 它们，这里照实写死，任何一次回头去看它们的改动都会被这条挡住。
                        "verified":0,"failed":0,"notAttempted":0,"unknown":4,
                        "stoppedReason":"surface_read_complete"
                    }]
                },
                "checkpoint":surface_receipt(stopped_reason),
                "records":records
            }
        })
        .to_string(),
    )
    .unwrap()
}

fn bounded_root_submission(
    task: &ProducerTaskSpec,
    attempt: &linggan_contracts::ProducerAttempt,
    producer_instance_id: Uuid,
) -> linggan_contracts::ProducerSubmission {
    let capability = task.raw()["capabilitiesRequested"][0].as_str().unwrap();
    let identity = task.raw()["target"]["authorExternalId"].as_str().unwrap();
    let maximum_quota = task.raw()["maximumQuota"].as_i64().unwrap();
    let records = if capability == "author_profile" {
        vec![serde_json::json!({
            "kind":"author_profile",
            "sourceObject":{"platform":"xhs","type":"author","externalId":identity},
            "payload":{"userId":identity,"nickname":"bounded baseline proof"}
        })]
    } else {
        (0..maximum_quota)
            .map(|ordinal| {
                let external_id = format!("{identity}-work-{ordinal}");
                serde_json::json!({
                    "kind":"profile_discovery_card",
                    "resultPosition":ordinal + 1,
                    "sourceObject":{
                        "platform":"xhs",
                        "type":"content",
                        "externalId":external_id
                    },
                    // 真实插件的发现卡带签名链接：它是补详情的执行入口，也是候选判据问的那一句
                    // 「此刻能不能解析出地址」。夹具里少了它，这一份目录会整片地被判成
                    // 「排不进去」——用例测到的就不是它以为自己测的那个行为了。
                    "payload":{
                        "title":format!("bounded work {ordinal}"),
                        "url":format!("https://www.xiaohongshu.com/explore/{external_id}?xsec_token=SIGNED_FIXTURE&xsec_source=pc_user")
                    }
                })
            })
            .collect()
    };
    let acquired = i64::try_from(records.len()).unwrap();
    parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":producer_instance_id,
            "taskId":task.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":capability,
                "platform":"xhs",
                "observedAt":"2026-09-04T00:00:00Z",
                "capturedAt":"2026-09-04T00:00:01Z",
                "coverage":{
                    "target":{"basis":"known_set","authorExternalId":identity},
                    "layers":[{
                        "capability":capability,
                        "observed":acquired,"attempted":acquired,"acquired":acquired,
                        "verified":0,"failed":0,"notAttempted":0,"unknown":0,
                        "stoppedReason":"maximum_quota"
                    }]
                },
                "checkpoint":surface_receipt("maximum_quota"),
                "records":records
            }
        })
        .to_string(),
    )
    .unwrap()
}

async fn seed_bound_task(
    database: &Database,
    lease_ref: Uuid,
    sequence_no: i32,
    capability: &str,
    target: serde_json::Value,
) -> Uuid {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
           (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,$2,$3,'scheduled','xhs','note_detail')",
    )
    .bind(task_id)
    .bind(hash_for(task_id))
    .bind(serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1",
        "taskId":task_id,
        "source":"scheduled",
        "platform":"xhs",
        "pageType":"note_detail",
        "target":target,
        "capabilitiesRequested":[capability],
        "maximumQuota":1,
        "commentLimit":"not_requested",
        "acquireMedia":"not_requested",
        "riskPolicy":"server_leased_controlled",
        "stopConditions":["maximum_quota","time_budget"]
    }))
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task \
           (lease_ref,task_id,sequence_no,execution_state,claimed_at,completed_at) \
         VALUES ($1,$2,$3,'completed',scope_001_now(),scope_001_now())",
    )
    .bind(lease_ref)
    .bind(task_id)
    .bind(sequence_no)
    .execute(database.pool())
    .await
    .unwrap();
    task_id
}

async fn seed_standalone_task(
    database: &Database,
    capability: &str,
    target: serde_json::Value,
) -> Uuid {
    let task_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
           (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,$2,$3,'manual','xhs','synthetic_dossier_proof')",
    )
    .bind(task_id)
    .bind(hash_for(task_id))
    .bind(serde_json::json!({
        "target":target,
        "capabilitiesRequested":[capability],
        "maximumQuota":1
    }))
    .execute(database.pool())
    .await
    .unwrap();
    task_id
}

async fn seed_runtime_package(
    database: &Database,
    task_id: Uuid,
    package_kind: &str,
    accepted_at: &str,
) -> Uuid {
    let task_target: serde_json::Value =
        sqlx::query_scalar("SELECT task_spec->'target' FROM linggan_runtime_task WHERE task_id=$1")
            .bind(task_id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    let coverage = serde_json::json!({
        "target":task_target,
        "layers":[{
            "capability":package_kind,
            "observed":1,"attempted":1,"acquired":1,"verified":0,
            "failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"surface_ended"
        }]
    });
    let checkpoint = surface_receipt("surface_ended");
    let attempt_id = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    let package_ref = Uuid::new_v4();
    let package_hash = hash_for(package_ref);
    sqlx::query(
        "INSERT INTO linggan_runtime_attempt (attempt_id,task_id,producer_instance_id,started_at) \
         VALUES ($1,$2,$3,$4::timestamptz)",
    )
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .bind(accepted_at)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_capture_package \
           (package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform, \
            package_hash,observed_at,captured_at,coverage,checkpoint,payload,accepted_at) \
         VALUES ($1,$2,$3,$4,$5,'xhs',$6,$7,$7,$8,$9,'{}',$7::timestamptz)",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .bind(package_kind)
    .bind(&package_hash)
    .bind(accepted_at)
    .bind(coverage)
    .bind(checkpoint)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_runtime_submission_receipt \
           (submission_id,task_id,attempt_id,producer_instance_id,package_hash,package_ref, \
            receipt_ref,received_at,execution_effect,material_admission) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8::timestamptz,'COMPLETED_LIVE_STEP','ACCEPTED')",
    )
    .bind(Uuid::new_v4())
    .bind(task_id)
    .bind(attempt_id)
    .bind(producer_instance_id)
    .bind(package_hash)
    .bind(package_ref)
    .bind(Uuid::new_v4())
    .bind(accepted_at)
    .execute(database.pool())
    .await
    .unwrap();
    package_ref
}

async fn mark_bound_task_completed(database: &Database, task_id: Uuid) {
    sqlx::query(
        "UPDATE collection_work_order_lease_task \
         SET execution_state='completed',claimed_at=scope_001_now(),completed_at=scope_001_now() \
         WHERE task_id=$1",
    )
    .bind(task_id)
    .execute(database.pool())
    .await
    .unwrap();
}

async fn seed_historical_patrol(
    database: &Database,
    target_ref: Uuid,
    station_ref: Uuid,
    issued_at: &str,
) -> Uuid {
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let lease_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
           (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target, \
            allowed_task_templates,allowed_dispatch_lanes,max_work_units, \
            purpose,granted_by,granted_at,expires_at) \
         VALUES ($1,'xhs','creator','patrol',1,20, \
                 ARRAY['creator_patrol'],ARRAY['immediate','scheduled'],20,'历史巡查','person', \
                 $2::timestamptz,$2::timestamptz + interval '30 days')",
    )
    .bind(authorization_ref)
    .bind(issued_at)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
           (request_ref,target_ref,lane,purpose,requested_by,requested_at) \
         VALUES ($1,$2,'patrol','历史巡查','person',$3::timestamptz)",
    )
    .bind(request_ref)
    .bind(target_ref)
    .bind(issued_at)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_admission_decision \
           (decision_ref,request_ref,outcome,reason_code,reason,authorization_ref,target_ref, \
            station_ref,decided_at) \
         VALUES ($1,$2,'admitted','within_authorization','历史巡查',$3,$4,$5,$6::timestamptz)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(issued_at)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_work_order \
           (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,station_ref,created_at) \
         VALUES ($1,$2,$3,'patrol',20,'{\"maximumQuota\":20}'::jsonb,$4,$5::timestamptz)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(station_ref)
    .bind(issued_at)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO collection_work_order_lease \
           (lease_ref,work_order_ref,station_ref,capture_identity,issued_at,expires_at, \
            released_at,release_reason) \
         VALUES ($1,$2,$3,'{}',$4::timestamptz,$4::timestamptz + interval '1 hour', \
                 $4::timestamptz + interval '30 minutes','completed')",
    )
    .bind(lease_ref)
    .bind(work_order_ref)
    .bind(station_ref)
    .bind(issued_at)
    .execute(database.pool())
    .await
    .unwrap();
    lease_ref
}

async fn seed_surface_work(
    database: &Database,
    lease_ref: Uuid,
    sequence_no: i32,
    author_external_id: &str,
    content_external_id: &str,
    accepted_at: &str,
) -> Uuid {
    let task_id = seed_bound_task(
        database,
        lease_ref,
        sequence_no,
        "profile_discovery",
        serde_json::json!({"authorExternalId":author_external_id}),
    )
    .await;
    let package_ref =
        seed_runtime_package(database, task_id, "profile_discovery", accepted_at).await;
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         VALUES ($1,0,'accepted_for_library_discovery','target-scoped patrol proof')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let work_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_material_content \
           (platform,content_external_id,public_ref,first_package_ref) \
         VALUES ('xhs',$1,$2,$3)",
    )
    .bind(content_external_id)
    .bind(work_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_discovery_finding \
           (material_ref,content_public_ref,package_ref,record_ordinal,discovery_kind, \
            result_position,observed_at,title,title_state,creator_display_name,creator_state, \
            published_at_source_text,published_at_source_text_state) \
         VALUES ($1,$2,$3,0,'profile_discovery',1,$4,$5,'KNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN')",
    )
    .bind(Uuid::new_v4())
    .bind(work_ref)
    .bind(package_ref)
    .bind(accepted_at)
    .bind(content_external_id)
    .execute(database.pool())
    .await
    .unwrap();
    work_ref
}

async fn seed_unbound_work(database: &Database, work_ref: Uuid, content_external_id: &str) {
    let seed_task = seed_standalone_task(
        database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
    )
    .await;
    let package_ref = seed_runtime_package(
        database,
        seed_task,
        "content_detail",
        "2026-09-03T10:00:00Z",
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_material_content \
           (platform,content_external_id,public_ref,first_package_ref) \
         VALUES ('xhs',$1,$2,$3)",
    )
    .bind(content_external_id)
    .bind(work_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
}

async fn seed_detail_observation(
    database: &Database,
    work_ref: Uuid,
    content_external_id: &str,
    author_external_id: Option<&str>,
    likes: i64,
    published_at: &str,
) {
    let task_id = seed_standalone_task(
        database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
    )
    .await;
    let package_ref =
        seed_runtime_package(database, task_id, "content_detail", "2026-09-03T11:00:00Z").await;
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         VALUES ($1,0,'accepted_for_library_content','qualified lifecycle detail')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_content_detail \
           (material_ref,content_public_ref,package_ref,record_ordinal,observed_at, \
            title,title_state,body_text,body_state,creator_display_name, \
            creator_display_name_state,published_at_source_text,published_at_source_text_state, \
            searchable_text,author_external_id,like_count,like_count_state,published_at, \
            published_at_source_field,published_at_source_kind,published_at_precision, \
            published_at_parser_version) \
         VALUES ($1,$2,$3,0,'2026-09-03T11:00:00Z',$4,'KNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN', \
                 $5,'KNOWN',$4,$6,$7,'KNOWN',$5::timestamptz,'publishTime','platform_epoch', \
                 'second','xhs-detail-time-v2')",
    )
    .bind(Uuid::new_v4())
    .bind(work_ref)
    .bind(package_ref)
    .bind(content_external_id)
    .bind(published_at)
    .bind(author_external_id)
    .bind(likes)
    .execute(database.pool())
    .await
    .unwrap();
}

/// 一次**合格但没带标题**的详情：材料本体、已接纳来源、非隔离处置三样都在，只有 `title` 为空。
///
/// 这不是残缺夹具，是平台真实会发生的形态——详情页读回来了，标题字段没有。`0015` 的
/// `CHECK ((title_state='KNOWN') = (title IS NOT NULL))` 要求这种材料把 `title_state` 记成
/// `UNKNOWN`，夹具照实写。
async fn seed_untitled_detail_observation(database: &Database, work_ref: Uuid) {
    let content_external_id: String = sqlx::query_scalar(
        "SELECT content_external_id FROM linggan_material_content WHERE public_ref=$1",
    )
    .bind(work_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let task_id = seed_standalone_task(
        database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
    )
    .await;
    let package_ref =
        seed_runtime_package(database, task_id, "content_detail", "2026-09-03T11:00:00Z").await;
    sqlx::query(
        "INSERT INTO linggan_runtime_record_disposition \
           (package_ref,record_ordinal,disposition,reason) \
         VALUES ($1,0,'accepted_for_library_content','qualified detail without a platform title')",
    )
    .bind(package_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_content_detail \
           (material_ref,content_public_ref,package_ref,record_ordinal,observed_at, \
            title,title_state,body_text,body_state,creator_display_name, \
            creator_display_name_state,published_at_source_text,published_at_source_text_state, \
            searchable_text,author_external_id,like_count,like_count_state,published_at, \
            published_at_source_field,published_at_source_kind,published_at_precision, \
            published_at_parser_version) \
         VALUES ($1,$2,$3,0,'2026-09-03T11:00:00Z',NULL,'UNKNOWN',NULL,'UNKNOWN',NULL,'UNKNOWN', \
                 NULL,'UNKNOWN',$4,NULL,NULL,'UNKNOWN',NULL,NULL,'unknown','unknown',NULL)",
    )
    .bind(Uuid::new_v4())
    .bind(work_ref)
    .bind(package_ref)
    .bind(&content_external_id)
    .execute(database.pool())
    .await
    .unwrap();
}

fn hash_for(value: Uuid) -> String {
    let half = value.simple().to_string();
    format!("{half}{half}")
}

/// 把夹具表达的「意图」翻成插件真实报回的滚动收尾。
///
/// 夹具原来只写 `coverage.stoppedReason`，用的是 `surface_ended` / `maximum_quota`——
/// 而真实插件从来不发这两个词，它在那个字段里写死一个不携带信息的常量，真话报在
/// `checkpoint.surfaceReceipt.stopReason`。夹具照着一个不存在的形态构造，测试就只能
/// 证明服务端会接受一种现实中不会到来的输入。这里补上真实形态。
fn surface_receipt(stopped_reason: &str) -> serde_json::Value {
    let stop_reason = match stopped_reason {
        // 拿满了本次配额就停——真实插件报 `target_reached`。
        "maximum_quota" => "target_reached",
        // 翻到底了——真实插件报 `bottom_confirmed`。
        "surface_ended" => "bottom_confirmed",
        // 其余（`risk_control` / `no_progress` / `max_rounds_reached`）本来就是
        // 「没到底也没拿满」，原样传下去——把它们也翻成完成，等于把夹具想表达的残缺
        // 悄悄改成了成功。
        other => other,
    };
    serde_json::json!({
        "kind": "search_surface_receipt",
        "surfaceReceipt": {"kind": "xhs_profile_surface", "stopReason": stop_reason},
    })
}
