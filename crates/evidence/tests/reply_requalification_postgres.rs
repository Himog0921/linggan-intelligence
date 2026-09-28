#[path = "support/material_fixture.rs"]
mod fixture;

use linggan_evidence::requalify_reply_contract_records;
use sqlx::Row;
use uuid::Uuid;

async fn historical_reply(
    database: &linggan_storage_postgres::Database,
    reported_note: &str,
    mixed_invalid: bool,
    manual_root: bool,
) -> Uuid {
    let task_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let package_ref = Uuid::new_v4();
    let instance_id = Uuid::new_v4();
    let source = if manual_root { "manual" } else { "scheduled" };
    let risk_policy = if manual_root {
        "local_trusted_user_initiated"
    } else {
        "server_authorized_leased"
    };
    let target = if manual_root {
        serde_json::json!({"contentExternalId":"note-historical","commentScope":"maximum_quota","requestedCommentLimit":20})
    } else {
        serde_json::json!({"contentExternalId":"note-historical","replyExpandLimit":2})
    };
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,
        "source":source,"platform":"xhs","pageType":"note_detail",
        "target":target,
        "capabilitiesRequested":["replies"],"maximumQuota":if manual_root {1} else {20},"commentLimit":20,
        "acquireMedia":"not_requested","riskPolicy":risk_policy,
        "stopConditions":["maximum_quota","time_budget"]
    });
    let mut records = vec![
        serde_json::json!({"kind":"reply","sourceObject":{"platform":"xhs","type":"content","externalId":reported_note},
        "payload":{"noteId":reported_note,"commentId":format!("reply-{package_ref}"),
            "rootCommentId":"root-1","parentCommentId":"root-1","text":"historical reply"}}),
    ];
    if manual_root {
        records[0]["payload"]["rootCommentId"] = records[0]["payload"]["commentId"].clone();
        records[0]["payload"]
            .as_object_mut()
            .unwrap()
            .remove("parentCommentId");
        for ordinal in 1..6 {
            let comment_id = format!("manual-{ordinal}-{package_ref}");
            let root_id = if ordinal % 2 == 0 {
                comment_id.clone()
            } else {
                format!("root-{ordinal}-{package_ref}")
            };
            let mut payload = serde_json::json!({
                "noteId":reported_note,"commentId":comment_id,"rootCommentId":root_id,
                "text":"historical manual reply"
            });
            if ordinal % 2 == 1 {
                payload["parentCommentId"] = serde_json::json!(root_id);
                payload["replyToCommentId"] = serde_json::json!(root_id);
            }
            records.push(serde_json::json!({
                "kind":"reply","sourceObject":{"platform":"xhs","type":"content","externalId":reported_note},
                "payload":payload
            }));
        }
    }
    if mixed_invalid {
        records.push(serde_json::json!({"kind":"reply","sourceObject":{"platform":"xhs","type":"content","externalId":reported_note},
            "payload":{"noteId":reported_note,"commentId":format!("invalid-{package_ref}"),
                "rootCommentId":"root-1","parentCommentId":"root-1","replyToCommentId":"root-1","text":"invalid relationship"}}));
    }
    let record_count = records.len();
    let package = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1","packageRef":package_ref,
        "packageKind":"replies","platform":"xhs",
        "observedAt":"2026-09-20T10:00:00Z","capturedAt":"2026-09-20T10:00:01Z",
        "coverage":{"target":{"basis":"known_set","contentExternalId":reported_note},"layers":[{
            "capability":"replies","observed":record_count,"attempted":record_count,"acquired":record_count,"verified":0,
            "failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"collector_complete"
        }]},
        "records":records
    });
    sqlx::query("INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type) VALUES($1,$2,$3,$4,'xhs','note_detail')")
        .bind(task_id).bind(format!("{}{}", task_id.simple(), "a".repeat(32)))
        .bind(task).bind(source).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id) VALUES($1,$2,$3)")
        .bind(attempt_id).bind(task_id).bind(instance_id).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_capture_package(package_ref,attempt_id,task_id,producer_instance_id,package_kind,platform,package_hash,observed_at,captured_at,coverage,payload) VALUES($1,$2,$3,$4,'replies','xhs',$5,'2026-09-20T10:00:00Z','2026-09-20T10:00:01Z',$6,$7)")
        .bind(package_ref).bind(attempt_id).bind(task_id).bind(instance_id)
        .bind(format!("{}{}", package_ref.simple(), "b".repeat(32)))
        .bind(&package["coverage"]).bind(package).execute(database.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_runtime_submission_receipt(submission_id,task_id,attempt_id,producer_instance_id,package_hash,package_ref,receipt_ref,execution_effect,material_admission) VALUES($1,$2,$3,$4,$5,$6,$7,$8,'ACCEPTED')")
        .bind(Uuid::new_v4()).bind(task_id).bind(attempt_id).bind(instance_id)
        .bind(format!("{}{}", package_ref.simple(), "b".repeat(32)))
        .bind(package_ref).bind(Uuid::new_v4())
        .bind(if manual_root {"NOT_APPLICABLE"} else {"COMPLETED_LIVE_STEP"})
        .execute(database.pool()).await.unwrap();
    for ordinal in 0..record_count {
        let original_reason = if manual_root && ordinal >= 1 {
            "task_package_contract_mismatch__beyond_task_maximum_quota"
        } else {
            "task_package_contract_mismatch"
        };
        sqlx::query("INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason) VALUES($1,$2,'quarantined',$3)")
            .bind(package_ref).bind(ordinal as i32).bind(original_reason)
            .execute(database.pool()).await.unwrap();
    }
    package_ref
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn historical_reply_is_requalified_once_through_the_current_validator_and_projection() {
    let database = fixture::proof_database("reply_requalification").await;
    let valid = historical_reply(&database, "note-historical", false, false).await;
    let invalid = historical_reply(&database, "another-note", false, false).await;
    let mixed = historical_reply(&database, "note-historical", true, false).await;
    let manual_root = historical_reply(&database, "note-historical", false, true).await;
    let preview = requalify_reply_contract_records(&database, false)
        .await
        .unwrap();
    assert_eq!(
        (
            preview.candidate_packages,
            preview.eligible_packages,
            preview.eligible_records
        ),
        (4, 3, 9)
    );
    assert_eq!(preview.eligible_admissions, 2);
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_comment")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(before, 0);
    let applied = requalify_reply_contract_records(&database, true)
        .await
        .unwrap();
    assert_eq!(
        (applied.requalified_packages, applied.requalified_records),
        (3, 9)
    );
    assert_eq!(applied.admitted_records, 2);
    let row = sqlx::query("SELECT disposition,reason,initial_disposition,initial_reason,requalification_basis FROM linggan_runtime_record_disposition WHERE package_ref=$1")
        .bind(valid).fetch_one(database.pool()).await.unwrap();
    assert_eq!(
        row.get::<String, _>("disposition"),
        "accepted_for_library_content"
    );
    assert_eq!(
        row.get::<String, _>("reason"),
        "typed_reply_relationship_valid"
    );
    assert_eq!(row.get::<String, _>("initial_disposition"), "quarantined");
    assert_eq!(
        row.get::<String, _>("initial_reason"),
        "task_package_contract_mismatch"
    );
    assert_eq!(
        row.get::<String, _>("requalification_basis"),
        "reply_task_identity_v2"
    );
    let historical: String = sqlx::query_scalar(
        "SELECT CASE WHEN requalified_at>created_at THEN initial_disposition ELSE disposition END \
         FROM linggan_runtime_record_disposition WHERE package_ref=$1",
    )
    .bind(valid)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let current: String = sqlx::query_scalar(
        "SELECT CASE WHEN requalified_at>requalified_at + interval '1 second' \
         THEN initial_disposition ELSE disposition END \
         FROM linggan_runtime_record_disposition WHERE package_ref=$1",
    )
    .bind(valid)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        (historical.as_str(), current.as_str()),
        ("quarantined", "accepted_for_library_content")
    );
    let projected: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_comment WHERE package_ref=$1 AND is_reply",
    )
    .bind(valid)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(projected, 1);
    let mixed_rows = sqlx::query("SELECT record_ordinal,disposition,reason,initial_reason FROM linggan_runtime_record_disposition WHERE package_ref=$1 ORDER BY record_ordinal")
        .bind(mixed).fetch_all(database.pool()).await.unwrap();
    assert_eq!(
        mixed_rows[0].get::<String, _>("disposition"),
        "accepted_for_library_content"
    );
    assert_eq!(mixed_rows[1].get::<String, _>("disposition"), "quarantined");
    assert_eq!(
        mixed_rows[1].get::<String, _>("reason"),
        "typed_reply_relationship_invalid"
    );
    assert_eq!(
        mixed_rows[1].get::<String, _>("initial_reason"),
        "task_package_contract_mismatch"
    );
    let mixed_projected: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_comment WHERE package_ref=$1")
            .bind(mixed)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(mixed_projected, 1);
    let manual_decisions = sqlx::query("SELECT disposition,reason,initial_reason FROM linggan_runtime_record_disposition WHERE package_ref=$1 ORDER BY record_ordinal")
        .bind(manual_root).fetch_all(database.pool()).await.unwrap();
    assert_eq!(manual_decisions.len(), 6);
    for (ordinal, decision) in manual_decisions.iter().enumerate() {
        assert_eq!(decision.get::<String, _>("disposition"), "quarantined");
        let suffix = if ordinal >= 1 {
            "__beyond_task_maximum_quota"
        } else {
            ""
        };
        assert_eq!(
            decision.get::<String, _>("reason"),
            format!("typed_reply_relationship_invalid{suffix}")
        );
        assert_eq!(
            decision.get::<String, _>("initial_reason"),
            format!("task_package_contract_mismatch{suffix}")
        );
    }
    let invalid_decision: String = sqlx::query_scalar(
        "SELECT disposition FROM linggan_runtime_record_disposition WHERE package_ref=$1",
    )
    .bind(invalid)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(invalid_decision, "quarantined");
    assert_eq!(
        requalify_reply_contract_records(&database, true)
            .await
            .unwrap()
            .requalified_records,
        0
    );
    assert!(
        sqlx::query(
            "UPDATE linggan_runtime_record_disposition SET reason='arbitrary' WHERE package_ref=$1"
        )
        .bind(valid)
        .execute(database.pool())
        .await
        .is_err()
    );
}
