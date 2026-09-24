#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{coverage_layer, proof_database, submit_custom_package, submit_package};
use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    AccountEligibilityObservation, DeliveryConclusion, DetailPageSessionProgress, DispatchDecision,
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, activate_installation_credential,
    bind_observation_account, content_reobservation, decide_dispatch, grant_detail_page_session,
    issue_work_order_lease, read_content_reobservation, read_detail_delivery_reconciliation,
    record_detail_page_session_navigation, record_detail_page_session_progress,
    report_account_eligibility, rotate_installation_credential, set_station_accepting,
    start_producer_attempt, submit_producer_package,
};
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn reobservation_never_merges_into_a_claimed_work_order() {
    let database = proof_database("content_reobservation_authorized_lease").await;
    let content_external_id = "note-reobserve-1";
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
            "payload":{"title":"首次观察","likes":10,"publicCommentCount":20,"collects":3,"shares":1}
        }),
    )
    .await;
    submit_profile_discovery(&database, content_external_id).await;

    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let fixture = seed_authorized_material_context(&database, content_public_ref).await;
    let shadow_authorization_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_acquisition_authorization (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target,allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) VALUES ($1,'xhs','creator','deep_archive',100,200,ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],200,'unlinked broader grant','person',scope_001_now()+interval '7 days')")
        .bind(shadow_authorization_ref).execute(database.pool()).await.unwrap();

    let command = content_reobservation(&database, content_public_ref)
        .await
        .expect("one existing target-linked XHS work can request a bounded reobservation");
    assert_eq!(command.admission, "ADMITTED");
    let admitted_authorization_ref: Uuid = sqlx::query_scalar(
        "SELECT authorization_ref FROM collection_admission_decision WHERE decision_ref=$1",
    )
    .bind(command.decision_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        admitted_authorization_ref, fixture.authorization_ref,
        "a later, broader but unlinked grant must not shadow the authorization that linked this work"
    );
    assert_ne!(admitted_authorization_ref, shadow_authorization_ref);
    assert_eq!(command.media.state, "NOT_REQUESTED");
    assert_eq!(command.media.reason, "EXISTING_ASSETS_REUSED");
    assert_eq!(command.execution, "QUEUED");
    assert!(command.work_order_ref.is_some());
    assert_eq!(command.lease_ref, None);
    assert!(
        command.tasks.is_empty(),
        "tasks form only after station claim"
    );

    let merged = content_reobservation(&database, content_public_ref)
        .await
        .expect("the same material merges into the exact queued work order");
    assert_eq!(merged.admission, "MERGE");
    assert_eq!(merged.execution, "QUEUED");
    assert_eq!(merged.work_order_ref, command.work_order_ref);
    assert_eq!(merged.lease_ref, None);
    assert!(merged.tasks.is_empty());

    let dispatch = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .expect("the first bounded lane is dispatchable");
    let detail = scheduled_task(&dispatch);
    assert_eq!(detail.raw()["capabilitiesRequested"][0], "content_detail");
    assert_eq!(detail.raw()["commentLimit"], "not_requested");
    let plan = match &dispatch {
        DispatchDecision::Dispatch {
            page_session_plan, ..
        } => page_session_plan
            .as_ref()
            .expect("detail dispatch carries a same-page plan"),
        other => panic!("expected a dispatch, got {other:?}"),
    };
    assert_eq!(
        plan["lanes"],
        serde_json::json!(["content_detail", "comments", "replies"])
    );
    assert_eq!(plan["commentLimit"], 30);
    assert!(
        !plan["lanes"]
            .as_array()
            .expect("page session lanes are a list")
            .iter()
            .any(|lane| lane == "media_slots" || lane == "media_bytes"),
        "the same-page read plan must not add any media lane"
    );
    let lease_ref: Uuid = sqlx::query_scalar(
        "SELECT lease_ref FROM collection_work_order_lease WHERE work_order_ref=$1 \
         AND released_at IS NULL",
    )
    .bind(command.work_order_ref.expect("admitted work order"))
    .fetch_one(database.pool())
    .await
    .unwrap();
    let queued = read_content_reobservation(&database, content_public_ref, lease_ref)
        .await
        .expect("the claimed lease state is readable")
        .expect("the claimed lease remains linked to this work");
    assert_eq!(queued.tasks.len(), 3);
    assert_eq!(queued.tasks[0].state, "CLAIMED");
    assert!(queued.tasks[1..].iter().all(|task| task.state == "QUEUED"));
    assert!(
        queued
            .tasks
            .iter()
            .all(|task| task.acquire_media == "not_requested")
    );
    assert_eq!(queued.tasks[1].comment_limit, serde_json::json!(30));
    assert_eq!(queued.tasks[2].comment_limit, serde_json::json!(30));

    let after_claim = content_reobservation(&database, content_public_ref)
        .await
        .expect("a claimed Work Order keeps its usage set frozen, even for the same scope");
    assert_eq!(after_claim.admission, "ADMITTED");
    assert_eq!(after_claim.execution, "QUEUED");
    assert_ne!(after_claim.work_order_ref, command.work_order_ref);
    assert_eq!(after_claim.lease_ref, None);
    assert!(after_claim.tasks.is_empty());

    let attempt = parse_producer_attempt(
        &serde_json::json!({
            "contractVersion":"linggan.producer.attempt.v1",
            "producerInstanceId":fixture.producer_instance_id,
            "taskId":detail.task_id(),
            "attemptId":Uuid::new_v4()
        })
        .to_string(),
    )
    .unwrap();
    assert!(matches!(
        start_producer_attempt(&database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));
    let submission = parse_producer_submission(
        &serde_json::json!({
            "contractVersion":"linggan.producer.capture-package.v1",
            "producerInstanceId":fixture.producer_instance_id,
            "taskId":detail.task_id(),
            "attemptId":attempt.attempt_id(),
            "submissionId":Uuid::new_v4(),
            "capturePackage":{
                "contractVersion":"linggan.producer.capture-package.v1",
                "packageRef":Uuid::new_v4(),
                "packageKind":"content_detail","platform":"xhs",
                "observedAt":"2026-09-01T10:00:00Z","capturedAt":"2026-09-01T10:00:01Z",
                "coverage":{"target":{"basis":"known_set","contentExternalId":content_external_id},"layers":[coverage_layer("content_detail",1)]},
                "records":[{"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},"payload":{"title":"第二次观察","likes":12,"publicCommentCount":22,"collects":4,"shares":1}}]
            }
        })
        .to_string(),
    )
    .unwrap();
    assert!(matches!(
        submit_producer_package(&database, &submission).await,
        Ok(RuntimeSubmissionOutcome::Acknowledged { .. })
    ));

    let after_detail = read_content_reobservation(&database, content_public_ref, lease_ref)
        .await
        .expect("accepted lane and queued lanes remain separately readable")
        .expect("the issued lease remains linked to this work");
    assert_eq!(after_detail.tasks[0].state, "ACCEPTED");
    assert_eq!(after_detail.tasks[1].state, "QUEUED");
    assert_eq!(after_detail.tasks[2].state, "QUEUED");
    let media_task_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_task \
         WHERE task_spec->'target'->>'contentExternalId'=$1 \
           AND task_spec->'capabilitiesRequested'->>0 IN ('media_slots','media_bytes')",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        media_task_count, 0,
        "a normal reobservation never creates media work"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn detail_page_grant_replays_one_request_and_suppresses_a_new_request_for_the_same_work() {
    let database = proof_database("detail_page_session_grant_idempotency").await;
    let content_external_id = "note-detail-session-grant";
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
            "payload":{"title":"首次观察","likes":1,"publicCommentCount":0,"collects":0,"shares":0}
        }),
    )
    .await;
    submit_profile_discovery(&database, content_external_id).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let fixture = seed_authorized_material_context(&database, content_public_ref).await;
    let queued = content_reobservation(&database, content_public_ref)
        .await
        .expect("the bounded reobservation creates its one detail work order");
    assert_eq!(queued.execution, "QUEUED");
    let dispatched = decide_dispatch(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
    )
    .await
    .unwrap();
    let (task_id, execution_source_url) = match dispatched {
        DispatchDecision::Dispatch {
            task_id,
            execution_source_url: Some(url),
            ..
        } => (task_id, url),
        other => panic!("expected detail dispatch, got {other:?}"),
    };
    let request_id = Uuid::new_v4();
    let first = grant_detail_page_session(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        request_id,
        &execution_source_url,
    )
    .await
    .unwrap();
    let replay = grant_detail_page_session(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        request_id,
        &execution_source_url,
    )
    .await
    .unwrap();
    let suppressed = grant_detail_page_session(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        Uuid::new_v4(),
        &execution_source_url,
    )
    .await
    .unwrap();
    let session_ref = match first {
        linggan_evidence::DetailPageSessionGrant::Authorized { session_ref, .. } => session_ref,
        other => panic!("first grant must authorize, got {other:?}"),
    };
    record_detail_page_session_navigation(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        session_ref,
    )
    .await
    .expect("only the owner of the original task can record the observed tab");
    record_detail_page_session_progress(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        session_ref,
        DetailPageSessionProgress::DeliveryPending,
    )
    .await
    .expect("the detail body may be pending delivery while comment collection continues");
    // 旧插件的会话没有任何冻结通道身份：它说了「待交付」，读层却无从对账。这时只能说
    // 「恢复待核实」加最后观察时间——不写 0（那会读成「没有待交付」），也不写「待交付」
    // （那会让一个无法核对的标记顶替结论）。
    let unverified = read_detail_delivery_reconciliation(&database, 100)
        .await
        .expect("delivery reconciliation is readable");
    assert_eq!(unverified.len(), 1);
    assert_eq!(
        unverified[0].conclusion,
        DeliveryConclusion::RecoveryUnverified
    );
    assert_eq!(unverified[0].prepared_lanes, 0);
    assert_eq!(unverified[0].state, "delivery_pending");
    assert!(matches!(
        replay,
        linggan_evidence::DetailPageSessionGrant::Replay { session_ref: replay_ref, .. }
        if replay_ref == session_ref
    ));
    assert!(matches!(
        suppressed,
        linggan_evidence::DetailPageSessionGrant::Suppressed { session_ref: suppressed_ref, reason_code: "session_already_authorized" }
        if suppressed_ref == session_ref
    ));
    let sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_detail_page_session")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(
        sessions, 1,
        "one work/material owns exactly one server session"
    );
    let observation: (String, bool) = sqlx::query_as(
        "SELECT state,navigation_observed_at IS NOT NULL \
         FROM collection_detail_page_session WHERE session_ref=$1",
    )
    .bind(session_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(observation.0, "delivery_pending");
    assert!(
        observation.1,
        "a recorded Chrome tab is distinct from authorization"
    );
    record_detail_page_session_progress(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        session_ref,
        DetailPageSessionProgress::Stopped {
            reason: "page_unavailable",
        },
    )
    .await
    .expect("a failed page session becomes terminal instead of being silently replayed");
    let stopped: (String, Option<String>, bool) = sqlx::query_as(
        "SELECT state,stop_reason,finished_at IS NOT NULL \
         FROM collection_detail_page_session WHERE session_ref=$1",
    )
    .bind(session_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(stopped.0, "stopped");
    assert_eq!(stopped.1.as_deref(), Some("page_unavailable"));
    assert!(stopped.2);
    // 读取层不把终态改写回「恢复待核实」：会话终结后，这一行说的是它终结在哪。
    let closed = read_detail_delivery_reconciliation(&database, 100)
        .await
        .expect("delivery reconciliation is readable");
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].conclusion, DeliveryConclusion::Closed);
    assert_eq!(closed[0].stop_reason.as_deref(), Some("page_unavailable"));
    assert!(closed[0].closed_at.is_some());
    let stopped_replay = grant_detail_page_session(
        &database,
        &fixture.install_key,
        &fixture.installation_credential,
        task_id,
        request_id,
        &execution_source_url,
    )
    .await
    .unwrap();
    assert!(matches!(
        stopped_replay,
        linggan_evidence::DetailPageSessionGrant::Suppressed { session_ref: replay_ref, reason_code: "session_stopped" }
        if replay_ref == session_ref
    ));
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn concurrent_reobservation_is_one_atomic_frozen_scope_and_one_queued_work_order() {
    let database = proof_database("content_reobservation_atomic_concurrency").await;
    let content_external_id = "note-reobserve-concurrent";
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
            "payload":{"title":"并发复观测","likes":10,"publicCommentCount":20,"collects":3,"shares":1}
        }),
    )
    .await;
    submit_profile_discovery(&database, content_external_id).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let fixture = seed_authorized_material_context(&database, content_public_ref).await;

    let (first, second) = tokio::join!(
        content_reobservation(&database, content_public_ref),
        content_reobservation(&database, content_public_ref),
    );
    let first = first.expect("first concurrent request completes");
    let second = second.expect("second concurrent request completes");
    let mut admissions = vec![first.admission, second.admission];
    admissions.sort_unstable();
    assert_eq!(admissions, ["ADMITTED", "MERGE"]);
    assert_eq!(first.execution, "QUEUED");
    assert_eq!(second.execution, "QUEUED");
    assert_eq!(first.lease_ref, None);
    assert_eq!(second.lease_ref, None);
    assert_eq!(first.work_order_ref, second.work_order_ref);
    let queued_work_orders: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order work_order \
         JOIN collection_admission_decision decision ON decision.decision_ref=work_order.decision_ref \
         JOIN collection_work_order_material_target scope ON scope.work_order_ref=work_order.work_order_ref \
         WHERE work_order.work_order_ref=$1 AND work_order.target_ref=$2 \
           AND decision.authorization_ref=$3 AND scope.content_public_ref=$4 \
           AND work_order.queue_state='queued'",
    )
    .bind(first.work_order_ref.expect("concurrent requests share a Work Order"))
    .bind(fixture.target_ref)
    .bind(fixture.authorization_ref)
    .bind(content_public_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        queued_work_orders, 1,
        "the target lock covers admission through exact queued scope creation"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn reobservation_never_merges_a_live_lease_with_a_different_frozen_policy() {
    let database = proof_database("content_reobservation_exact_scope").await;
    let content_external_id = "note-reobserve-exact-scope";
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
            "payload":{"title":"策略边界复观测","likes":10,"publicCommentCount":20,"collects":3,"shares":1}
        }),
    )
    .await;
    submit_profile_discovery(&database, content_external_id).await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let fixture = seed_authorized_material_context(&database, content_public_ref).await;
    sqlx::query(
        "UPDATE collection_work_order_material_target SET acquire_media=true \
         WHERE work_order_ref=$1 AND content_public_ref=$2",
    )
    .bind(fixture.work_order_ref)
    .bind(content_public_ref)
    .execute(database.pool())
    .await
    .unwrap();
    let different_policy_lease = issue_work_order_lease(&database, fixture.work_order_ref, 60)
        .await
        .expect("the deliberately broader media policy is a live fixture lease");

    let command = content_reobservation(&database, content_public_ref)
        .await
        .expect("the normal no-media reobservation is admitted as a distinct frozen scope");
    assert_eq!(command.admission, "ADMITTED");
    assert_eq!(command.lease_ref, None);
    assert!(command.work_order_ref.is_some());
    assert_eq!(command.execution, "QUEUED");
    assert_eq!(
        command.admission_reason, "已在作品原先关联的有效授权范围内准入",
        "a live different-policy lease does not bypass the queue or erase a newly requested scope"
    );
    let leased_media: bool = sqlx::query_scalar(
        "SELECT EXISTS ( \
             SELECT 1 FROM collection_work_order_lease lease \
             JOIN collection_work_order_material_target scope ON scope.work_order_ref=lease.work_order_ref \
             WHERE lease.lease_ref=$1 AND scope.acquire_media=true)",
    )
    .bind(different_policy_lease.lease_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(
        leased_media,
        "the pre-existing lease really differs in policy"
    );
}

struct AuthorizedFixture {
    install_key: String,
    installation_credential: String,
    producer_instance_id: Uuid,
    authorization_ref: Uuid,
    target_ref: Uuid,
    work_order_ref: Uuid,
}

async fn seed_authorized_material_context(
    database: &linggan_storage_postgres::Database,
    content_public_ref: Uuid,
) -> AuthorizedFixture {
    let target_ref = Uuid::new_v4();
    let authorization_ref = Uuid::new_v4();
    let request_ref = Uuid::new_v4();
    let decision_ref = Uuid::new_v4();
    let work_order_ref = Uuid::new_v4();
    let station_ref = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    let install_key = producer_instance_id.to_string();
    // 已建档目标必须通过明确 Domain relation 归属，准入不会猜用途。
    sqlx::query("INSERT INTO collection_observation_target (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) VALUES ($1,'xhs','creator','creator-reobserve','复观测夹具','manual','archived')")
        .bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO observation_domain_target (domain_ref,target_ref,role) VALUES ('00000000-0000-4000-8000-000000000001',$1,'primary')")
        .bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_authorization (authorization_ref,platform,target_kind,lane,max_targets,max_works_per_target,allowed_task_templates,allowed_dispatch_lanes,max_work_units,purpose,granted_by,expires_at) VALUES ($1,'xhs','creator','deep_archive',10,20,ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],20,'content reobservation proof','person',scope_001_now()+interval '1 day')")
        .bind(authorization_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_acquisition_request (request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) VALUES ($1,$2,'00000000-0000-4000-8000-000000000001','primary','deep_archive','content reobservation proof','person')")
        .bind(request_ref).bind(target_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_admission_decision (decision_ref,request_ref,outcome,reason_code,authorization_ref) VALUES ($1,$2,'admitted','fixture',$3)")
        .bind(decision_ref).bind(request_ref).bind(authorization_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO execution_station (station_ref,display_name,daily_work_quota) VALUES ($1,'复观测夹具工位',200)")
        .bind(station_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO plugin_installation (installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) VALUES ($1,$2,$3,'person',scope_001_now(),'0.8.47','[\"content_detail\",\"comments\",\"replies\",\"media_slots\",\"media_bytes\"]'::jsonb)")
        .bind(installation_ref).bind(&install_key).bind(station_ref).execute(database.pool()).await.unwrap();
    set_station_accepting(database, station_ref, true, "person")
        .await
        .unwrap();
    let credential = rotate_installation_credential(database, installation_ref)
        .await
        .unwrap();
    let installation_credential = credential.raw_credential.expose_once().to_owned();
    activate_installation_credential(
        database,
        installation_ref,
        credential.credential_ref,
        &installation_credential,
    )
    .await
    .unwrap();
    let account = report_account_eligibility(
        database,
        installation_ref,
        &installation_credential,
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "xhs-account-reobservation-fixture",
        },
        Some(b"reobservation-fixture-digest-key-at-least-32"),
    )
    .await
    .unwrap();
    let account_ref = account.account_ref.unwrap();
    bind_observation_account(database, account_ref, installation_ref, "person")
        .await
        .unwrap();
    sqlx::query("UPDATE collection_admission_decision SET target_ref=$2,station_ref=$3,installation_ref=$4,account_ref=$5 WHERE decision_ref=$1")
        .bind(decision_ref).bind(target_ref).bind(station_ref).bind(installation_ref).bind(account_ref)
        .execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,dispatch_lane,queue_state,scheduled_for,station_ref,installation_ref,account_ref) VALUES ($1,$2,$3,'deep_archive',20,'[\"maximum_quota\",\"time_budget\"]'::jsonb,'batch','queued',scope_001_now()+interval '1 day',$4,$5,$6)")
        .bind(work_order_ref).bind(decision_ref).bind(target_ref).bind(station_ref).bind(installation_ref).bind(account_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_domain_usage (work_order_ref,request_ref,domain_ref,role,basis_kind) VALUES ($1,$2,'00000000-0000-4000-8000-000000000001','primary','admitted')")
        .bind(work_order_ref).bind(request_ref).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO collection_work_order_material_target (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit,acquire_media,allow_ocr,allow_asr) VALUES ($1,$2,1,30,2,false,false,false)")
        .bind(work_order_ref).bind(content_public_ref).execute(database.pool()).await.unwrap();
    AuthorizedFixture {
        install_key,
        installation_credential,
        producer_instance_id,
        authorization_ref,
        target_ref,
        work_order_ref,
    }
}

async fn submit_profile_discovery(
    database: &linggan_storage_postgres::Database,
    content_external_id: &str,
) {
    let target = serde_json::json!({"authorExternalId":"creator-reobserve"});
    submit_custom_package(
        database,
        "xhs",
        &["profile_discovery"],
        target.clone(),
        "profile_discovery",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("profile_discovery",1)]}),
        vec![serde_json::json!({
            "kind":"profile_discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
            "payload":{"title":"可执行来源","url":format!("https://www.xiaohongshu.com/explore/{content_external_id}?xsec_token=fixture-token&xsec_source=pc_feed")}
        })],
    )
    .await;
}

fn scheduled_task(decision: &DispatchDecision) -> linggan_contracts::ProducerTaskSpec {
    let value = match decision {
        DispatchDecision::Dispatch { task_spec, .. } => task_spec,
        other => panic!("expected a dispatch, got {other:?}"),
    };
    let task = parse_producer_task_spec(&value.to_string()).unwrap();
    assert_eq!(task.source(), "scheduled");
    assert_eq!(task.raw()["riskPolicy"], "server_authorized_leased");
    task
}
