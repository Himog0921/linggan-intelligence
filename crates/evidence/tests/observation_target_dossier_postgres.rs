#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_contracts::{
    AdmissionOutcome, ProducerTaskSpec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilitySignal, AcquisitionChainError, AuthorizationGrant, CheckInOutcome,
    CreatorLifecycleAssociation, CreatorLifecycleMetric, CreatorLifecycleQuery,
    CreatorLifecycleStatus, CreatorLifecycleWindow, InstallationCheckIn, RequestLeaseError,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, activate_installation_credential,
    bind_observation_account, check_in_installation, decide_dispatch, grant_authorization,
    open_claim_window, read_archive_completeness, read_creator_lifecycle, register_station,
    report_account_eligibility, request_admit_and_lease, request_progressive_archive_and_lease,
    run_progressive_archives, set_station_accepting, start_producer_attempt,
    submit_producer_package,
};
use linggan_storage_postgres::Database;
use std::time::Duration;
use uuid::Uuid;

const DIGEST_KEY: &[u8] = b"observation-target-dossier-postgres-proof-v1";

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
    assert_eq!(completeness.works_listed, 1);
    assert_eq!(completeness.details_captured, 1);
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
    assert_eq!(after_zero, "2026-09-04 09:00");
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
        after_unqualified, "2026-09-04 09:00",
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
           (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator',$2,$2,'manual')",
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
            plugin_version: "0.8.34",
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
        Some(&format!("{label}-account")),
        AccountEligibilitySignal::AuthenticatedObserved,
        DIGEST_KEY,
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
                serde_json::json!({
                    "kind":"profile_discovery_card",
                    "resultPosition":ordinal + 1,
                    "sourceObject":{
                        "platform":"xhs",
                        "type":"content",
                        "externalId":format!("{identity}-work-{ordinal}")
                    },
                    "payload":{"title":format!("bounded work {ordinal}")}
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
            package_hash,observed_at,captured_at,coverage,payload,accepted_at) \
         VALUES ($1,$2,$3,$4,$5,'xhs',$6,$7,$7,$8,'{}',$7::timestamptz)",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .bind(package_kind)
    .bind(&package_hash)
    .bind(accepted_at)
    .bind(coverage)
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

fn hash_for(value: Uuid) -> String {
    let half = value.simple().to_string();
    format!("{half}{half}")
}
