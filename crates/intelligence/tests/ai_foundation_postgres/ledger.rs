//! Real PostgreSQL ledger and upgrade contracts, with entirely constructed usage facts.
use super::*;
use linggan_intelligence::{
    model_invocation::{checkpoint_invocation_usage, finish_invocation},
    pi_adapter::{PI_PROTOCOL, PiResponse, PiUsage},
};
use sqlx::Row;

const MIGRATION: &str = include_str!("../../../../database/migrations/0138_ai_gen1_invocation.sql");
async fn legacy_database(schema: &str) -> Database {
    let (before, _) = fixture::MIGRATIONS.split_once(MIGRATION).unwrap();
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    linggan_storage_postgres::testing::isolated_proof_schema(&url, schema, before)
        .await
        .unwrap()
}
async fn route(db: &Database) -> Uuid {
    let c = new_connection(db).await;
    version(db, c, 1, "https://synthetic-ledger.example/api").await
}
async fn reserve(db: &Database, version: Uuid, result: serde_json::Value) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,task_key,quota_bucket,classification_source)
        VALUES($1,$2,'analyze',repeat('0',64),'running',80,0,$3,'writing_legacy','request','producer_v1')")
        .bind(id).bind(version).bind(result).execute(db.pool()).await.unwrap();
    id
}
fn response(input: Option<i64>, output: Option<i64>) -> PiResponse {
    PiResponse {
        version: PI_PROTOCOL.into(),
        ok: true,
        text: None,
        failure_code: None,
        model_ids: None,
        model_list_origin: None,
        usage: PiUsage {
            input_tokens: input,
            output_tokens: output,
            cost_usd: None,
            ..Default::default()
        },
        elapsed_ms: Some(1),
        diagnostic: None,
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_usage_checkpoint_survives_failure_and_terminal_receipts_cannot_be_rewritten()
{
    let db = fixture::proof_database("ai_ledger_settlement").await;
    let v = route(&db).await;
    let id = reserve(&db, v, json!({"callStarted":false})).await;
    let row = sqlx::query(
        "SELECT usage_state,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("usage_state"), "unknown_reserved");
    assert_eq!(
        row.get::<i64, _>("charged_tokens"),
        80,
        "an unstarted running reservation still holds quota"
    );
    checkpoint_invocation_usage(&db, id, Some(&response(Some(100), Some(40))))
        .await
        .unwrap();
    finish_invocation(
        &db,
        id,
        None,
        false,
        Some("fixture_validation_failed"),
        &json!({"validationPending":false}),
    )
    .await
    .unwrap();
    let row=sqlx::query("SELECT input_tokens,output_tokens,charged_tokens,measured_tokens,usage_state,dispatch_state FROM linggan_model_invocation WHERE invocation_ref=$1").bind(id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i64, _>("input_tokens"), 100);
    assert_eq!(row.get::<i64, _>("output_tokens"), 40);
    assert_eq!(row.get::<i64, _>("charged_tokens"), 140);
    assert_eq!(row.get::<i64, _>("measured_tokens"), 140);
    assert_eq!(row.get::<String, _>("usage_state"), "measured");
    for sql in [
        "UPDATE linggan_model_invocation SET charged_tokens=0 WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET result='{}' WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET state='running',finished_at=NULL WHERE invocation_ref=$1",
        "DELETE FROM linggan_model_invocation WHERE invocation_ref=$1",
    ] {
        assert!(
            sqlx::query(sql).bind(id).execute(db.pool()).await.is_err(),
            "{sql}"
        );
    }
    // The generic replay is a no-op, rather than a new mutable terminal write.
    finish_invocation(&db, id, None, true, None, &json!({"overwrite":true}))
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        "failed"
    );
    let no_send = reserve(&db, v, json!({"callStarted":false})).await;
    finish_invocation(
        &db,
        no_send,
        None,
        false,
        Some("model_secret_unavailable"),
        &json!({}),
    )
    .await
    .unwrap();
    let row=sqlx::query("SELECT usage_state,charged_tokens,measured_tokens FROM linggan_model_invocation WHERE invocation_ref=$1").bind(no_send).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("usage_state"), "not_dispatched");
    assert_eq!(row.get::<i64, _>("charged_tokens"), 0);
    assert_eq!(row.get::<i64, _>("measured_tokens"), 0);
    let unknown = reserve(&db, v, json!({})).await;
    finish_invocation(
        &db,
        unknown,
        None,
        false,
        Some("new_unknown_code"),
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(unknown)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        80
    );
    let flags=sqlx::query("SELECT retryable,counts_as_failure FROM linggan_ai_failure_code WHERE code='new_unknown_code'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(flags.get::<Option<bool>, _>("retryable"), None);
    assert_eq!(flags.get::<Option<bool>, _>("counts_as_failure"), None);
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_partial_and_cache_usage_cannot_release_quota_or_count_twice() {
    let db = fixture::proof_database("ai_ledger_partial").await;
    let v = route(&db).await;
    let id = reserve(&db, v, json!({"callStarted":true})).await;
    sqlx::query("UPDATE linggan_model_invocation SET cached_input_tokens=90,cached_write_tokens=10,reasoning_tokens=30 WHERE invocation_ref=$1").bind(id).execute(db.pool()).await.unwrap();
    let row=sqlx::query("SELECT charged_tokens,measured_tokens,usage_state FROM linggan_model_invocation WHERE invocation_ref=$1").bind(id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i64, _>("charged_tokens"), 130);
    assert_eq!(row.get::<Option<i64>, _>("measured_tokens"), None);
    assert_eq!(row.get::<String, _>("usage_state"), "unknown_reserved");
    assert!(
        sqlx::query("UPDATE linggan_model_invocation SET input_tokens=99 WHERE invocation_ref=$1")
            .bind(id)
            .execute(db.pool())
            .await
            .is_err()
    );
    finish_invocation(
        &db,
        id,
        Some(&response(Some(120), Some(40))),
        true,
        None,
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        160
    );
    let id = reserve(&db, v, json!({})).await;
    sqlx::query("UPDATE linggan_model_invocation SET input_tokens=200 WHERE invocation_ref=$1")
        .bind(id)
        .execute(db.pool())
        .await
        .unwrap();
    finish_invocation(
        &db,
        id,
        None,
        false,
        Some("ai_dispatch_unknown"),
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1"
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        200
    );
    let id = reserve(&db, v, json!({})).await;
    for sql in [
        "UPDATE linggan_model_invocation SET state='failed' WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET finished_at=scope_001_now() WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET task_key='model_probe' WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET input_tokens=9223372036854775807,output_tokens=1 WHERE invocation_ref=$1",
        "UPDATE linggan_model_invocation SET input_tokens=1,dispatch_state='not_dispatched' WHERE invocation_ref=$1",
    ] {
        assert!(
            sqlx::query(sql).bind(id).execute(db.pool()).await.is_err(),
            "{sql}"
        );
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_invocation_upgrade_retains_unknowns_and_original_completion_evidence() {
    let db = legacy_database("ai_ledger_upgrade").await;
    let v = route(&db).await;
    let ids: [Uuid; 5] = std::array::from_fn(|_| Uuid::new_v4());
    for (id, op, result, input, output) in [
        (
            ids[0],
            "analyze",
            json!({"purpose":"unknown-future","domainRef":"not-a-uuid"}),
            None,
            None,
        ),
        (
            ids[1],
            "analyze",
            json!({"purpose":"creator_discovery","domainRef":"bad"}),
            Some(90),
            None,
        ),
        (ids[2], "analyze", json!({"callStarted":false}), None, None),
        (
            ids[3],
            "probe",
            json!({"purpose":"creator_discovery"}),
            None,
            None,
        ),
        (
            ids[4],
            "probe",
            json!({"fixture":"measured"}),
            Some(10),
            Some(20),
        ),
    ] {
        sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,input_tokens,output_tokens,created_at,finished_at,failure_code)
            VALUES($1,$2,$3,'fixture','failed',80,0,$4,$5,$6,'2026-10-18 16:00:00+00','2026-10-18 16:01:00+00','opaque-old-code')")
            .bind(id).bind(v).bind(op).bind(result).bind(input).bind(output).execute(db.pool()).await.unwrap();
    }
    let before:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',invocation_ref,'result',result,'finishedAt',finished_at,'createdAt',created_at) FROM linggan_model_invocation ORDER BY invocation_ref").fetch_all(db.pool()).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(MIGRATION.to_owned()))
        .execute(db.pool())
        .await
        .unwrap();
    let after:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('id',invocation_ref,'result',result,'finishedAt',finished_at,'createdAt',created_at) FROM linggan_model_invocation ORDER BY invocation_ref").fetch_all(db.pool()).await.unwrap();
    assert_eq!(
        before, after,
        "classification cannot rewrite historic timestamps or response facts"
    );
    for (id, task, usage, quota) in [
        (ids[0], "legacy_unclassified", "unknown_reserved", 80_i64),
        (ids[1], "legacy_creator_discovery", "unknown_reserved", 90),
        (ids[2], "legacy_unclassified", "not_dispatched", 0),
        (ids[3], "legacy_unclassified", "unknown_reserved", 80),
        (ids[4], "model_probe", "measured", 30),
    ] {
        let row=sqlx::query("SELECT task_key,usage_state,charged_tokens,budget_day::text AS day FROM linggan_model_invocation WHERE invocation_ref=$1").bind(id).fetch_one(db.pool()).await.unwrap();
        assert_eq!(row.get::<String, _>("task_key"), task);
        assert_eq!(row.get::<String, _>("usage_state"), usage);
        assert_eq!(row.get::<i64, _>("charged_tokens"), quota);
        assert_eq!(row.get::<String, _>("day"), "2026-10-19");
    }
    assert_eq!(
        sqlx::query_scalar::<_, Option<bool>>(
            "SELECT retryable FROM linggan_ai_failure_code WHERE code='opaque-old-code'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        None
    );
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_invocation_upgrade_refuses_missing_or_contradictory_finish_times_atomically()
{
    for (schema, state, finished) in [
        ("ai_ledger_dirty_terminal", "failed", false),
        ("ai_ledger_dirty_running", "running", true),
    ] {
        let db = legacy_database(schema).await;
        let v = route(&db).await;
        sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,finished_at)
            VALUES($1,$2,'analyze','dirty-fixture',$3,80,80,CASE WHEN $4 THEN scope_001_now() ELSE NULL END)")
            .bind(Uuid::new_v4()).bind(v).bind(state).bind(finished).execute(db.pool()).await.unwrap();
        let err = sqlx::raw_sql(sqlx::AssertSqlSafe(MIGRATION.to_owned()))
            .execute(db.pool())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("ai_invocation_preflight_required"));
        assert_eq!(
            sqlx::query_scalar::<_, bool>("SELECT to_regclass('linggan_ai_failure_code') IS NULL")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            true
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
                .fetch_one(db.pool())
                .await
                .unwrap(),
            1
        );
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_frozen_price_quote_is_exact_and_missing_cache_or_price_stays_unknown() {
    let db = fixture::proof_database("ai_ledger_price").await;
    let v = route(&db).await;
    let m = model(&db, v).await;
    let e = endpoint(&db, v).await;
    let price = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_price(price_ref,identity_ref,endpoint_ref,input_micro_usd_per_million,output_micro_usd_per_million,cached_read_micro_usd_per_million,cached_write_micro_usd_per_million,source_kind,source_version)
        VALUES($1,$2,$3,2,5,1,3,'manual','constructed-ledger-proof')")
        .bind(price).bind(m).bind(e).execute(db.pool()).await.unwrap();
    for (has_price, known_cache, expected) in [
        (true, true, Some("53959")),
        (true, false, None),
        (false, true, None),
    ] {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,price_ref,task_key,quota_bucket,classification_source)
            VALUES($1,$2,$3,'analyze','constructed-price','running',20000,20000,$4,'writing_legacy','request','producer_v1')")
            .bind(id).bind(v).bind(m).bind(has_price.then_some(price)).execute(db.pool()).await.unwrap();
        let mut r = response(Some(2345), Some(9876));
        r.usage.cached_input_tokens = known_cache.then_some(222);
        r.usage.cached_write_tokens = known_cache.then_some(111);
        r.usage.reasoning_tokens = Some(567);
        finish_invocation(&db, id, Some(&r), true, None, &json!({}))
            .await
            .unwrap();
        let row=sqlx::query("SELECT cost_pico_usd::text AS cost,charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1").bind(id).fetch_one(db.pool()).await.unwrap();
        assert_eq!(row.get::<Option<String>, _>("cost").as_deref(), expected);
        assert_eq!(row.get::<i64, _>("charged_tokens"), 12221);
    }
    let other_conn = new_connection(&db).await;
    let other_v = version(&db, other_conn, 1, "https://other-price.example/api").await;
    let other_m = model(&db, other_v).await;
    assert!(sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,price_ref)
        VALUES($1,$2,$3,'analyze','wrong-price','running',80,80,$4)")
        .bind(Uuid::new_v4()).bind(other_v).bind(other_m).bind(price).execute(db.pool()).await.is_err());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_upgrade_reads_actual_frozen_comment_schema_and_policy_domain() {
    let db = legacy_database("ai_ledger_comment_upgrade").await;
    sqlx::raw_sql(include_str!(
        "../../../../database/bootstrap/comment-study-001.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../database/migrations/0107_comment_study_productization_schema.sql"
    ))
    .execute(db.pool())
    .await
    .unwrap();
    let policy = Uuid::new_v4();
    let run = Uuid::new_v4();
    let d = Uuid::from_u128(0x00000000000040008000000000000001);
    sqlx::query("INSERT INTO linggan_comment_study_policy(policy_ref,domain_ref,contract,comment_budget,context_character_budget)
        VALUES($1,$2,'comment-study.v1',1,1024)").bind(policy).bind(d).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_comment_study_run(run_ref,policy_ref,as_of,state,selection_manifest,selection_hash)
        VALUES($1,$2,scope_001_now(),'prepared','{}',repeat('0',64))").bind(run).bind(policy).execute(db.pool()).await.unwrap();
    let v = route(&db).await;
    let id = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,operation,request_hash,state,reserved_tokens,charged_tokens,result,finished_at)
        VALUES($1,$2,'analyze','constructed-history','failed',80,40,$3,scope_001_now())")
        .bind(id).bind(v).bind(json!({"legacyRequestLedgerMissing":true,"legacyRunRef":run,"callStarted":true})).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(MIGRATION.to_owned()))
        .execute(db.pool())
        .await
        .unwrap();
    let row=sqlx::query("SELECT task_key,budget_run_ref,budget_domain_ref,charged_tokens,legacy_charged_tokens FROM linggan_model_invocation WHERE invocation_ref=$1")
        .bind(id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<String, _>("task_key"), "comment_study");
    assert_eq!(row.get::<Uuid, _>("budget_run_ref"), run);
    assert_eq!(row.get::<Uuid, _>("budget_domain_ref"), d);
    assert_eq!(row.get::<i64, _>("charged_tokens"), 80);
    assert_eq!(
        row.get::<i64, _>("legacy_charged_tokens"),
        40,
        "the original ledger number remains auditable"
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_history_is_bounded_stable_read_only_and_never_exposes_original_payloads() {
    use linggan_intelligence::ai_foundation::invocation_read::{HistoryQuery, read_history};
    let db = fixture::proof_database("ai_history_projection").await;
    let version = route(&db).await;
    let mut expected = std::collections::BTreeSet::new();
    for position in 0..52 {
        let id = reserve(
            &db,
            version,
            json!({"prompt":"SYNTHETIC-DO-NOT-EXPOSE","position":position}),
        )
        .await;
        expected.insert(id.to_string());
        if position == 0 {
            checkpoint_invocation_usage(&db, id, Some(&response(Some(0), Some(0))))
                .await
                .unwrap();
            finish_invocation(
                &db,
                id,
                None,
                true,
                None,
                &json!({"prompt":"SYNTHETIC-DO-NOT-EXPOSE"}),
            )
            .await
            .unwrap();
        }
    }
    let first = read_history(
        &db,
        &HistoryQuery {
            before: None,
            task_key: Some(TaskKey::WritingLegacy),
        },
    )
    .await
    .unwrap();
    assert_eq!(first["items"].as_array().unwrap().len(), 50);
    assert_eq!(first["hasMore"], true);
    let cursor = first["nextCursor"].as_str().unwrap().parse().unwrap();
    let second = read_history(
        &db,
        &HistoryQuery {
            before: Some(cursor),
            task_key: Some(TaskKey::WritingLegacy),
        },
    )
    .await
    .unwrap();
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert_eq!(second["hasMore"], false);
    assert!(second["nextCursor"].is_null());
    let rows: Vec<_> = first["items"]
        .as_array()
        .unwrap()
        .iter()
        .chain(second["items"].as_array().unwrap())
        .collect();
    let actual: std::collections::BTreeSet<_> = rows
        .iter()
        .map(|row| row["invocationRef"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(actual, expected);
    assert!(rows.iter().any(|row| row["measuredTokens"] == "0"));
    assert!(
        rows.iter()
            .any(|row| row["usageState"] == "unknown_reserved" && row["measuredTokens"].is_null())
    );
    assert!(rows.iter().all(|row| row["quotePicoUsd"].is_null()));
    for payload in [first, second] {
        let serialized = payload.to_string();
        assert!(!serialized.contains("SYNTHETIC-DO-NOT-EXPOSE"));
        assert!(!serialized.contains("requestHash"));
        assert!(!serialized.contains("secretRef"));
    }
    let empty = read_history(
        &db,
        &HistoryQuery {
            before: None,
            task_key: Some(TaskKey::ModelProbe),
        },
    )
    .await
    .unwrap();
    assert_eq!(empty["items"], json!([]));
    assert!(matches!(
        read_history(
            &db,
            &HistoryQuery {
                before: Some(Uuid::new_v4()),
                task_key: None
            }
        )
        .await,
        Err(FoundationError::NotFound)
    ));
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(count, 52);
}
