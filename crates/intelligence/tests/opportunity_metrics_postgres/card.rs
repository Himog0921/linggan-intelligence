//! T1.4 standard snapshot, debounce and source-withdrawal proofs.
use super::{
    database, database_with_standard_creators, opportunity_creator_dataset, opportunity_dataset,
};
use linggan_intelligence::opportunity::{
    CardRebuildMode, CommenterDigest, check_invariants, rebuild_opportunity_cards,
    sync_comment_mentions,
};
use serde_json::Value;
use uuid::Uuid;

async fn prepare() -> (linggan_storage_postgres::Database, Uuid, CommenterDigest) {
    let db = database_with_standard_creators(true).await;
    sqlx::raw_sql(r#"
      CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE
      AS $$ SELECT timestamptz '2026-11-10T00:00:00Z' $$;
      ALTER TABLE linggan_material_comment DISABLE TRIGGER USER;
      UPDATE linggan_material_comment SET body_text='SYNTHETIC 原声 '||comment_external_id,body_state='KNOWN'
      WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
      ALTER TABLE linggan_material_comment ENABLE TRIGGER USER;
      INSERT INTO linggan_comment_clean_cache(source_ref,cleaner_version,raw_sha256,research_text,clean_state,clean_reasons,created_at)
      SELECT material_ref,'comment-clean.v2',repeat('a',64),body_text,'direct','[]'::jsonb,'2026-11-09T00:00:00Z'
      FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
      INSERT INTO linggan_material_comment_engagement(material_ref,like_count,like_count_state,parser_version,created_at)
      SELECT material_ref,CASE comment_external_id WHEN 'c-work-0-1' THEN 40 WHEN 'c-work-0-2' THEN 30
        WHEN 'c-work-0-3' THEN 20 ELSE 10 END,'KNOWN','comment-like.v1','2026-11-09T00:00:00Z'
      FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-1','c-work-0-2','c-work-0-3','c-work-0-4');
    "#).execute(db.pool()).await.unwrap();
    (
        db,
        Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap(),
        CommenterDigest::from_key(vec![42; 32]).unwrap(),
    )
}

async fn p1(db: &linggan_storage_postgres::Database) -> Value {
    sqlx::query_scalar("SELECT payload FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid")
        .fetch_one(db.pool()).await.unwrap()
}

async fn wait_for_lock(db: &linggan_storage_postgres::Database, pattern: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
                WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE $1)",
            )
            .bind(pattern)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the expected lock wait did not occur");
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn card_uses_canonical_metrics_debounces_and_removes_withdrawn_voices() {
    let (db, domain, digest) = prepare().await;
    let waiting = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(waiting.cards_built, 0);
    assert!(waiting.changes_deferred >= 3);
    sqlx::query("UPDATE linggan_opportunity_card_change SET changed_at=clock_timestamp()-interval '301 seconds'")
        .execute(db.pool()).await.unwrap();
    let first = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(first.cards_built, 3);
    let value = p1(&db).await;
    let group = &value["demand"]["groups"][0];
    assert!(group["protocolVersion"].is_null());
    assert_eq!(group["current"]["periodStart"], "2026-11-09");
    assert_eq!(
        (
            group["current"]["k"].as_i64(),
            group["current"]["n"].as_i64()
        ),
        (Some(0), Some(0))
    );
    assert!(group["current"]["ratePer1k"].is_null());
    assert!(group["current"]["ciPer1k"].is_null());
    assert!(
        group["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "insufficient_sample")
    );
    assert_eq!(
        (
            group["baseline"]["k"].as_i64(),
            group["baseline"]["n"].as_i64()
        ),
        (Some(13), Some(4600))
    );
    assert_eq!(group["mentionersTotal"], 14);
    assert_eq!(group["evidenceTier"], "established");
    let previous = group["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap();
    assert_eq!(
        (previous["k"].as_i64(), previous["n"].as_i64()),
        (Some(10), Some(1000))
    );
    assert_eq!(value["supply"]["state"], "not_evaluated");
    assert!(value["supply"]["counts"].is_null());
    assert_eq!(value["demand"]["userMethods"]["status"], "unavailable");
    let voices = value["demand"]["sampleVoices"].as_array().unwrap();
    assert_eq!(voices.len(), 3);
    assert_eq!(voices[0]["likes"], 40);
    assert_eq!(voices[1]["likes"], 30);
    assert_eq!(voices[2]["likes"], 20);
    let next = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(
        next.cards_built, 0,
        "an unchanged input must not generate another card"
    );
    if std::env::var_os("OPPORTUNITY_PRINT_SYNTHETIC_SNAPSHOT").is_some() {
        println!(
            "SYNTHETIC_P1_SNAPSHOT\n{}",
            serde_json::to_string_pretty(&value).unwrap()
        );
    }
    let expected: Value = serde_json::from_str(include_str!("p1-card.snapshot.json")).unwrap();
    assert_eq!(
        value, expected,
        "P1's synthetic card must match its reviewed snapshot"
    );

    // An upstream event is durably transferred and debounced, without needing another worker.
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(domain.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    let deferred = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(deferred.cards_built, 0);
    assert!(deferred.changes_deferred >= 3);
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        events, 0,
        "source events must be consumed only after problem changes are queued"
    );
    sqlx::query("UPDATE linggan_opportunity_card_change SET changed_at=clock_timestamp()-interval '301 seconds'")
        .execute(db.pool()).await.unwrap();
    let delivered = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(delivered.cards_built, 3);
    assert_eq!(p1(&db).await, value);

    // A known like in the existing discovery lane counts as a work engagement reference,
    // but only after its observation exists at the card's as_of.
    sqlx::raw_sql(r#"
        INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type,created_at)
        VALUES(md5('card-like-task')::uuid,repeat('a',64),'{}','manual','xhs','seed','2026-01-01T00:00:00Z');
        INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id,started_at)
        VALUES(md5('card-like-attempt')::uuid,md5('card-like-task')::uuid,md5('card-like-producer')::uuid,'2026-01-01T00:00:00Z');
        INSERT INTO linggan_runtime_capture_package(package_ref,attempt_id,task_id,producer_instance_id,
          package_kind,platform,package_hash,observed_at,captured_at,coverage,payload,accepted_at)
        VALUES(md5('card-like-package')::uuid,md5('card-like-attempt')::uuid,md5('card-like-task')::uuid,
          md5('card-like-producer')::uuid,'discovery_search','xhs',repeat('b',64),'2026-11-05T00:00:00Z',
          '2026-11-05T00:00:00Z','{}','{}','2026-11-05T00:00:00Z');
        INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
        VALUES(md5('card-like-package')::uuid,0,'accepted_for_library_content','synthetic','2026-11-05T00:00:00Z');
        INSERT INTO linggan_material_discovery_finding(material_ref,content_public_ref,package_ref,record_ordinal,
          discovery_kind,result_position,observed_at,title_state,creator_state,published_at_source_text_state,
          like_count,like_count_state,created_at)
        VALUES(md5('card-like-finding')::uuid,md5('creator-work:c1-a')::uuid,md5('card-like-package')::uuid,0,
          'discovery_search',1,'2026-11-05T00:00:00Z','UNKNOWN','UNKNOWN','UNKNOWN',600,'KNOWN','2026-11-11T00:00:00Z');
    "#).execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert!(
        p1(&db).await["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "engagement_reference_missing")
    );
    sqlx::raw_sql(
        "ALTER TABLE linggan_material_discovery_finding DISABLE TRIGGER USER; \
        UPDATE linggan_material_discovery_finding SET created_at='2026-11-09T00:00:00Z' \
        WHERE material_ref=md5('card-like-finding')::uuid; \
        ALTER TABLE linggan_material_discovery_finding ENABLE TRIGGER USER",
    )
    .execute(db.pool())
    .await
    .unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert!(
        !p1(&db).await["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "engagement_reference_missing")
    );

    let content = voices[0]["commentRef"]["contentRef"].as_str().unwrap();
    let comment = voices[0]["commentRef"]["commentExternalId"]
        .as_str()
        .unwrap();
    // A projection arriving after the card's as_of cannot influence the card's order.
    sqlx::raw_sql("ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER; \
        UPDATE linggan_material_comment_engagement SET created_at='2026-11-11T00:00:00Z' \
        WHERE material_ref IN (SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'); \
        ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let before_projection = p1(&db).await;
    let before_voices = before_projection["demand"]["sampleVoices"]
        .as_array()
        .unwrap();
    assert_eq!(
        before_voices
            .iter()
            .map(|voice| voice["likes"].as_i64())
            .collect::<Vec<_>>(),
        vec![Some(30), Some(20), Some(10)]
    );
    sqlx::raw_sql("ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER; \
        UPDATE linggan_material_comment_engagement SET created_at='2026-11-09T00:00:00Z' \
        WHERE material_ref IN (SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'); \
        ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(p1(&db).await["demand"]["sampleVoices"][0]["likes"], 40);

    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,restricted_at,reason) VALUES($1::uuid,$2,'2026-11-09T00:00:00Z','synthetic withdrawal')")
        .bind(content).bind(comment).execute(db.pool()).await.unwrap();
    let retained: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        retained, 0,
        "withdrawal must remove stale text before debounce expires"
    );
    let rebuilt = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(rebuilt.cards_built, 3);
    let after = p1(&db).await;
    let voices = after["demand"]["sampleVoices"].as_array().unwrap();
    assert_eq!(voices.len(), 3);
    assert!(
        voices
            .iter()
            .all(|voice| voice["commentRef"]["commentExternalId"] != comment)
    );
    assert_eq!(
        voices[2]["likes"], 10,
        "the next eligible voice fills the vacated slot"
    );
    let invariants = check_invariants(&db).await.unwrap();
    let i9 = invariants
        .checks
        .iter()
        .find(|check| check.id == "I9")
        .unwrap();
    assert!(i9.available);
    assert_eq!(i9.violations, 0);

    // A withdrawal which starts first must commit before the rebuild takes its snapshot.
    let live = p1(&db).await;
    let content = live["demand"]["sampleVoices"][0]["commentRef"]["contentRef"]
        .as_str()
        .unwrap()
        .to_owned();
    let comment = live["demand"]["sampleVoices"][0]["commentRef"]["commentExternalId"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut withdrawing = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,restricted_at,reason) \
        VALUES($1::uuid,$2,'2026-11-09T00:00:00Z','synthetic concurrent withdrawal')")
        .bind(&content).bind(&comment).execute(&mut *withdrawing).await.unwrap();
    let rebuild_db = db.clone();
    let pending_rebuild = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(
        &db,
        "%LOCK TABLE linggan_material_comment_restriction IN SHARE MODE%",
    )
    .await;
    withdrawing.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), pending_rebuild)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let live = p1(&db).await;
    assert!(
        live["demand"]["sampleVoices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|voice| voice["commentRef"]["commentExternalId"] != comment)
    );

    // When the rebuild starts first, the writer waits and then removes the saved card atomically.
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    let content = live["demand"]["sampleVoices"][0]["commentRef"]["contentRef"]
        .as_str()
        .unwrap()
        .to_owned();
    let comment = live["demand"]["sampleVoices"][0]["commentRef"]["commentExternalId"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_opportunity_card_change(problem_ref) VALUES(md5('metric-problem:1')::uuid) \
        ON CONFLICT(problem_ref) DO NOTHING").execute(&mut *barrier).await.unwrap();
    sqlx::query("SELECT problem_ref FROM linggan_opportunity_card_change WHERE problem_ref=md5('metric-problem:1')::uuid FOR UPDATE")
        .fetch_one(&mut *barrier).await.unwrap();
    let rebuild_db = db.clone();
    let pending_rebuild = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let held: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_locks lock \
                JOIN pg_class relation ON relation.oid=lock.relation \
                JOIN pg_namespace ns ON ns.oid=relation.relnamespace \
                WHERE ns.nspname=current_schema() AND relation.relname='linggan_material_comment_restriction' \
                  AND lock.mode='ShareLock' AND lock.granted AND lock.pid<>pg_backend_pid())")
                .fetch_one(db.pool()).await.unwrap();
            if held { break; }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }).await.expect("the rebuild did not acquire its restriction lock");
    let pool = db.pool().clone();
    let withdrawal_comment = comment.clone();
    let pending_withdrawal = tokio::spawn(async move {
        sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,restricted_at,reason) \
            VALUES($1::uuid,$2,'2026-11-09T00:00:00Z','synthetic concurrent withdrawal')")
            .bind(content).bind(withdrawal_comment).execute(&pool).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_material_comment_restriction%").await;
    barrier.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), pending_rebuild)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), pending_withdrawal)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let retained: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_opportunity_card \
        WHERE problem_ref=md5('metric-problem:1')::uuid",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        retained, 0,
        "the later withdrawal cannot leave a stale card"
    );
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert!(
        p1(&db).await["demand"]["sampleVoices"]
            .as_array()
            .unwrap()
            .iter()
            .all(|voice| voice["commentRef"]["commentExternalId"] != comment)
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn card_keeps_unknown_and_known_protocols_in_separate_groups() {
    let db = database().await;
    let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
    let domain = Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap();
    opportunity_creator_dataset::build(&db).await.unwrap();
    sync_comment_mentions(&db, &digest).await.unwrap();
    sqlx::query(
        "UPDATE linggan_opportunity_comment_mention SET excluded_reason='marketing' \
        WHERE comment_external_id='creator-comment-c1-a-58'",
    )
    .execute(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE \
        AS $$ SELECT timestamptz '2026-11-10T00:00:00Z' $$",
    )
    .execute(db.pool())
    .await
    .unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let value = p1(&db).await;
    let demand = value["demand"]["groups"].as_array().unwrap();
    assert_eq!(demand.len(), 2);
    let unknown_demand = demand
        .iter()
        .find(|g| g["protocolVersion"].is_null())
        .unwrap();
    let known_demand = demand.iter().find(|g| g["protocolVersion"] == 1).unwrap();
    assert!(
        unknown_demand["baseline"]["n"].as_i64().unwrap()
            > known_demand["baseline"]["n"].as_i64().unwrap()
    );
    assert!(
        known_demand["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "no_baseline")
    );
    assert!(
        value["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "collection_protocol_changed")
    );
    let groups = value["channel"]["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 2);
    for (protocol, expected_k, expected_n) in [(Value::Null, 4, 60), (Value::from(1), 1, 55)] {
        let group = groups
            .iter()
            .find(|g| g["protocolVersion"] == protocol)
            .unwrap();
        let c1 = group["ranked"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["creatorId"] == "creator-C1")
            .unwrap();
        assert_eq!(
            (c1["k"].as_i64(), c1["n"].as_i64()),
            (Some(expected_k), Some(expected_n))
        );
        assert!(c1["name"].is_null());
        assert!(
            group["insufficient"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["creatorId"] == "creator-C2")
        );
    }
    // Historical protocols remain separately stored, but an observation outside the
    // configured twelve-period window must not imply a current protocol change.
    sqlx::raw_sql("ALTER TABLE linggan_material_content_detail DISABLE TRIGGER USER; \
        UPDATE linggan_material_content_detail SET published_at='2026-08-03T00:00:00Z' \
        WHERE content_public_ref IN (md5('creator-work:c1-v1')::uuid,md5('creator-work:c2-v1')::uuid); \
        ALTER TABLE linggan_material_content_detail ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let outside_window = p1(&db).await;
    assert!(
        !outside_window["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "collection_protocol_changed")
    );
    assert!(
        !outside_window["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("采集协议 1"))
    );
}
