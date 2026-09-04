#[allow(dead_code)]
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_contracts::AdmissionOutcome;
use linggan_evidence::{
    AccountEligibilitySignal, AcquisitionChainError, AuthorizationGrant, CheckInOutcome,
    CreatorLifecycleAssociation, CreatorLifecycleMetric, CreatorLifecycleQuery,
    CreatorLifecycleStatus, CreatorLifecycleWindow, InstallationCheckIn, RequestLeaseError,
    activate_installation_credential, bind_observation_account, check_in_installation,
    grant_authorization, open_claim_window, read_archive_completeness, read_creator_lifecycle,
    register_station, report_account_eligibility, request_progressive_archive_and_lease,
    set_station_accepting,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

const DIGEST_KEY: &[u8] = b"observation-target-dossier-postgres-proof-v1";

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

async fn ready_installation(database: &Database, label: &str) {
    let station_ref = register_station(database, label, 200).await.unwrap();
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
    bind_observation_account(
        database,
        receipt.account_ref.unwrap(),
        installation_ref,
        "person",
    )
    .await
    .unwrap();
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
         VALUES ($1,$2,$3,$4,$5,'xhs',$6,$7,$7,'{}','{}',$7::timestamptz)",
    )
    .bind(package_ref)
    .bind(attempt_id)
    .bind(task_id)
    .bind(producer_instance_id)
    .bind(package_kind)
    .bind(&package_hash)
    .bind(accepted_at)
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
            purpose,granted_by,granted_at,expires_at) \
         VALUES ($1,'xhs','creator','patrol',1,20,'历史巡查','person', \
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
