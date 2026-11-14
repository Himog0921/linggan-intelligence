//! T1.4 standard snapshot, debounce and source-withdrawal proofs.
use super::{
    database, database_with_standard_creators, opportunity_creator_dataset, opportunity_dataset,
};
use linggan_intelligence::opportunity::{
    CardRebuildMode, CommenterDigest, check_invariants, rebuild_opportunity_cards,
    sync_comment_mentions, sync_comment_mentions_for_domain,
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
    preserve_synthetic_marketing(&db).await;
    (
        db,
        Uuid::parse_str(opportunity_dataset::DOMAIN).unwrap(),
        CommenterDigest::from_key(vec![42; 32]).unwrap(),
    )
}

async fn preserve_synthetic_marketing(db: &linggan_storage_postgres::Database) {
    // D-013 does not classify marketing in v1; the fixed §8.2 fixture contains pre-labelled rows.
    sqlx::raw_sql(r#"
        CREATE FUNCTION synthetic_preserve_marketing() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF OLD.excluded_reason='marketing' AND NEW.excluded_reason IS NULL AND
             NEW.comment_external_id IN ('c-work-0-13','creator-comment-c1-a-58') THEN
            RETURN NULL;
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER synthetic_preserve_marketing BEFORE UPDATE ON linggan_opportunity_comment_mention
        FOR EACH ROW EXECUTE FUNCTION synthetic_preserve_marketing();
    "#).execute(db.pool()).await.unwrap();
}

async fn p1(db: &linggan_storage_postgres::Database) -> Value {
    sqlx::query_scalar("SELECT payload FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid")
        .fetch_one(db.pool()).await.unwrap()
}

async fn payload_contains_comment(db: &linggan_storage_postgres::Database, content: Uuid) -> bool {
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_opportunity_opportunity_card card \
         CROSS JOIN LATERAL jsonb_array_elements(card.payload#>'{demand,sampleVoices}') voice \
         WHERE (voice#>>'{commentRef,contentRef}')::uuid=$1 AND voice#>>'{commentRef,commentExternalId}'='c-work-0-1')")
        .bind(content)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn enable_source_events(db: &linggan_storage_postgres::Database) -> sqlx::PgConnection {
    let mut connection = db.pool().acquire().await.unwrap().detach();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        sqlx::query("SELECT pg_advisory_lock(linggan_opportunity_card_event_gate_key())")
            .execute(&mut connection),
    )
    .await
    .expect("the isolated test must acquire its event gate")
    .unwrap();
    connection
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn source_events_are_offline_without_a_live_enabled_executor() {
    use sqlx::Connection;
    let (db, domain, _digest) = prepare().await;
    let before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        before, 0,
        "disabled feature must not accumulate events during ingestion"
    );
    let mut offline_writer = db.pool().begin().await.unwrap();
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(domain.to_string())
        .execute(&mut *offline_writer)
        .await
        .unwrap();
    let mut gate = db.pool().acquire().await.unwrap().detach();
    let acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_lock(linggan_opportunity_card_event_gate_key())",
    )
    .fetch_one(&mut gate)
    .await
    .unwrap();
    assert!(
        !acquired,
        "startup must not race an uncommitted offline source write"
    );
    offline_writer.commit().await.unwrap();
    let acquired: bool = sqlx::query_scalar(
        "SELECT pg_try_advisory_lock(linggan_opportunity_card_event_gate_key())",
    )
    .fetch_one(&mut gate)
    .await
    .unwrap();
    assert!(acquired);
    let gate_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut gate)
        .await
        .unwrap();
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(domain.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    let online: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(online, 1);
    gate.close().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let closed: bool = sqlx::query_scalar(
                "SELECT NOT EXISTS(SELECT 1 FROM pg_stat_activity WHERE pid=$1)",
            )
            .bind(gate_pid)
            .fetch_one(db.pool())
            .await
            .unwrap();
            if closed {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("PostgreSQL must observe the owner disconnect");
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(domain.to_string())
        .execute(db.pool())
        .await
        .unwrap();
    let after: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        after, 1,
        "closing the owner connection must stop event accumulation"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn advisory_keys_use_the_object_schema_when_the_search_path_changes() {
    let (db, domain, _digest) = prepare().await;
    let _events_gate = enable_source_events(&db).await;
    let mut tx = db.pool().begin().await.unwrap();
    let before: i64 = sqlx::query_scalar("SELECT linggan_opportunity_card_event_gate_key()")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    sqlx::query("CREATE TEMP TABLE opportunity_lock_schema_test(id integer)")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("SELECT set_config('search_path',format('pg_temp,%I',current_schema()),true)")
        .execute(&mut *tx)
        .await
        .unwrap();
    let (current, owner, event_key): (String, String, i64) = sqlx::query_as(
        "SELECT current_schema()::text,linggan_opportunity_card_namespace(), \
         linggan_opportunity_card_event_gate_key()",
    )
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_ne!(current, owner, "the temporary schema must be first");
    assert_eq!(event_key, before);
    sqlx::query("SELECT linggan_opportunity_card_source_changed('domain',$1)")
        .bind(domain.to_string())
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        events, 1,
        "the enabled gate must work with the alternate path"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn source_events_are_drained_in_bounded_transactions_before_due_rebuilds() {
    let (db, domain, digest) = prepare().await;
    let _events_gate = enable_source_events(&db).await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO linggan_opportunity_card_source_event(event_kind,event_key) \
        SELECT 'domain',$1 FROM generate_series(1,1025)",
    )
    .bind(domain.to_string())
    .execute(db.pool())
    .await
    .unwrap();
    let first = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(first.cards_built, 0);
    assert!(first.changes_deferred >= 3);
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 1);
    let second = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(second.cards_built, 0);
    assert!(second.changes_deferred >= 3);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof and worker binary"]
async fn full_cli_syncs_membership_created_while_worker_was_offline() {
    use std::os::unix::fs::PermissionsExt;

    struct SyntheticSupport(std::path::PathBuf);
    impl Drop for SyntheticSupport {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("opportunity/commenter-digest.key"));
            let _ = std::fs::remove_dir(self.0.join("opportunity"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let (db, domain, digest) = prepare().await;
    let root = std::env::temp_dir().join(format!("opportunity-cli-proof-{}", Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let support = SyntheticSupport(root);
    std::fs::create_dir(support.0.join("opportunity")).unwrap();
    std::fs::set_permissions(
        support.0.join("opportunity"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let key = support.0.join("opportunity/commenter-digest.key");
    std::fs::write(&key, [42_u8; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    // Establish the existing 0177 baseline before adding the offline membership.
    sync_comment_mentions_for_domain(&db, &digest, domain)
        .await
        .unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let prior_card = p1(&db).await;
    let prior_k = prior_card["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    insert_offline_membership(&db).await;
    let before: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_mention \
        WHERE problem_ref=md5('metric-problem:1')::uuid AND comment_external_id='c-work-0-30'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(before, 0);
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let binary = std::env::var_os("OPPORTUNITY_TEST_WORKER_BIN").expect("synthetic worker binary");
    let output = tokio::process::Command::new(binary)
        .env_clear()
        .env(
            "LINGGAN_LOCAL_DATABASE_URL",
            std::env::var_os("OPPORTUNITY_PROOF_DATABASE_URL").unwrap(),
        )
        .env("LINGGAN_SUPPORT_DIR", &support.0)
        .env("LINGGAN_OPPORTUNITY_CARD", "enabled")
        .arg("rebuild-cards")
        .arg("--domain")
        .arg(domain.to_string())
        .arg("--full")
        .arg("--schema")
        .arg(schema)
        .output()
        .await
        .unwrap();
    assert!(
        output.status.success(),
        "full CLI failed with {}",
        output.status
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["cardsBuilt"], 3);
    let after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_comment_mention \
        WHERE problem_ref=md5('metric-problem:1')::uuid AND comment_external_id='c-work-0-30'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        after, 1,
        "full CLI must first project the offline membership"
    );
    let card = p1(&db).await;
    let series = card["demand"]["groups"][0]["series"].as_array().unwrap();
    assert_eq!(
        series
            .iter()
            .find(|period| period["periodStart"] == "2026-11-02")
            .unwrap()["k"],
        prior_k + 1
    );
}

async fn insert_offline_membership(db: &linggan_storage_postgres::Database) {
    sqlx::raw_sql(r#"
        ALTER TABLE linggan_comment_research_request DISABLE TRIGGER ALL;
        INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,
          observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version,created_at)
        SELECT md5('offline-cli-request')::uuid,md5('offline-cli-command')::uuid,
          '00000000-0000-4000-8000-000000000001',comment.content_public_ref,comment.material_ref,
          'primary','2026-11-05T00:00:00Z',repeat('b',64),repeat('c',64),setting.current_config_ref,
          md5('offline-cli-batch')::uuid,'comment-research-semantic.v1','2026-11-05T00:00:00Z'
        FROM linggan_material_comment comment CROSS JOIN linggan_ai_task_setting setting
        WHERE comment.comment_external_id='c-work-0-30' AND setting.task_key='comment_study';
        ALTER TABLE linggan_comment_research_request ENABLE TRIGGER ALL;
        INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref,created_at)
        VALUES(md5('offline-cli-request')::uuid,0,md5('metric-definition:1')::uuid,
          '00000000-0000-4000-8000-000000004003','2026-11-05T00:00:00Z');
    "#).execute(db.pool()).await.unwrap();
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn card_snapshot_includes_membership_committed_after_prior_sync() {
    let (db, domain, digest) = prepare().await;
    sync_comment_mentions_for_domain(&db, &digest, domain)
        .await
        .unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let previous: i64 = p1(&db).await["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    insert_offline_membership(&db).await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let current: i64 = p1(&db).await["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    assert_eq!(
        current,
        previous + 1,
        "the card must include a membership already committed before its snapshot"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn card_and_memberships_share_the_same_concurrent_snapshot() {
    let (db, domain, digest) = prepare().await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let previous: i64 = p1(&db).await["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    insert_offline_membership(&db).await;
    sqlx::raw_sql(r#"
        ALTER TABLE linggan_comment_research_request DISABLE TRIGGER ALL;
        INSERT INTO linggan_comment_research_request(request_ref,command_ref,domain_ref,work_ref,source_ref,
          observation_role,knowledge_cutoff,input_hash,input_fingerprint,task_config_ref,batch_ref,method_version,created_at)
        SELECT md5('snapshot-late-request')::uuid,md5('snapshot-late-command')::uuid,
          '00000000-0000-4000-8000-000000000001',comment.content_public_ref,comment.material_ref,
          'primary','2026-11-05T00:00:00Z',repeat('b',64),repeat('c',64),setting.current_config_ref,
          md5('snapshot-late-batch')::uuid,'comment-research-semantic.v1','2026-11-05T00:00:00Z'
        FROM linggan_material_comment comment CROSS JOIN linggan_ai_task_setting setting
        WHERE comment.comment_external_id='c-work-0-31' AND setting.task_key='comment_study';
        ALTER TABLE linggan_comment_research_request ENABLE TRIGGER ALL;
        CREATE FUNCTION synthetic_mention_snapshot_barrier() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.comment_external_id='c-work-0-30' THEN
            PERFORM pg_advisory_xact_lock(hashtextextended('synthetic-mention-snapshot-barrier',0));
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER synthetic_mention_snapshot_barrier BEFORE INSERT ON linggan_opportunity_comment_mention
        FOR EACH ROW EXECUTE FUNCTION synthetic_mention_snapshot_barrier();
    "#).execute(db.pool()).await.unwrap();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query(
        "SELECT pg_advisory_xact_lock(hashtextextended('synthetic-mention-snapshot-barrier',0))",
    )
    .execute(&mut *barrier)
    .await
    .unwrap();
    let rebuild_db = db.clone();
    let pending = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_opportunity_comment_mention%").await;
    sqlx::query("INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref,created_at) \
         VALUES(md5('snapshot-late-request')::uuid,0,md5('metric-definition:1')::uuid, \
           '00000000-0000-4000-8000-000000004003','2026-11-05T00:00:00Z')")
        .execute(db.pool()).await.unwrap();
    barrier.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), pending)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let first = p1(&db).await;
    let first_k = first["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    assert_eq!(
        first_k,
        previous + 1,
        "a later commit must not enter the earlier snapshot"
    );
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let second = p1(&db).await;
    let second_k = second["demand"]["groups"][0]["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|period| period["periodStart"] == "2026-11-02")
        .unwrap()["k"]
        .as_i64()
        .unwrap();
    assert_eq!(
        second_k,
        previous + 2,
        "the next snapshot must include the later commit"
    );
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
    let _events_gate = enable_source_events(&db).await;
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
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();

    // A withdrawal which starts first makes a stale card transaction retry safely.
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
    wait_for_lock(&db, "%linggan_opportunity_card_touch_source%").await;
    withdrawing.commit().await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), pending_rebuild)
        .await
        .unwrap()
        .unwrap();
    match result {
        Ok(_) => {}
        Err(linggan_intelligence::opportunity::CardError::Database(error)) => {
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|error| error.code())
                    .as_deref(),
                Some("40001")
            );
            rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
                .await
                .unwrap();
        }
        Err(error) => panic!("unexpected card rebuild failure: {error}"),
    }
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
    sqlx::raw_sql(r#"
        INSERT INTO observation_domain(domain_ref,name) VALUES(md5('unrelated-domain')::uuid,'synthetic unrelated domain');
        INSERT INTO linggan_material_content(platform,content_external_id,public_ref,first_package_ref,created_at)
        SELECT 'xhs','synthetic-unrelated',md5('unrelated-work')::uuid,first_package_ref,'2026-01-01T00:00:00Z'
        FROM linggan_material_content WHERE public_ref='00000000-0000-4000-8000-000000000100';
        INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,use_kind,basis_kind,package_ref,created_at)
        SELECT md5('unrelated-usage')::uuid,public_ref,md5('unrelated-domain')::uuid,'primary','domain_research',
          'legacy_domain_migration',first_package_ref,'2026-01-01T00:00:00Z'
        FROM linggan_material_content WHERE public_ref=md5('unrelated-work')::uuid;
    "#).execute(db.pool()).await.unwrap();
    let content = live["demand"]["sampleVoices"][0]["commentRef"]["contentRef"]
        .as_str()
        .unwrap()
        .to_owned();
    let comment = live["demand"]["sampleVoices"][0]["commentRef"]["commentExternalId"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query("SELECT problem_ref FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid FOR UPDATE")
        .fetch_one(&mut *barrier).await.unwrap();
    let rebuild_db = db.clone();
    let pending_rebuild = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_opportunity_opportunity_card%").await;
    tokio::time::timeout(std::time::Duration::from_secs(3), sqlx::query(
        "INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
         VALUES(md5('unrelated-work')::uuid,'unrelated-comment','synthetic unrelated withdrawal')")
        .execute(db.pool())).await.expect("an unrelated domain must not wait for the rebuild").unwrap();
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
async fn withdrawal_does_not_deadlock_when_a_voice_moves_between_cards() {
    let (db, domain, digest) = prepare().await;
    sqlx::raw_sql(r#"
        ALTER TABLE linggan_material_comment DISABLE TRIGGER USER;
        UPDATE linggan_material_comment SET body_text='SYNTHETIC additional voice '||comment_external_id,body_state='KNOWN'
        WHERE comment_external_id IN ('c-work-0-20','c-work-0-21','c-work-0-22');
        ALTER TABLE linggan_material_comment ENABLE TRIGGER USER;
        INSERT INTO linggan_comment_clean_cache(source_ref,cleaner_version,raw_sha256,research_text,clean_state,clean_reasons,created_at)
        SELECT material_ref,'comment-clean.v2',repeat('a',64),body_text,'direct','[]','2026-11-09T00:00:00Z'
        FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-20','c-work-0-21','c-work-0-22');
        INSERT INTO linggan_material_comment_engagement(material_ref,like_count,like_count_state,parser_version,created_at)
        SELECT material_ref,CASE comment_external_id WHEN 'c-work-0-20' THEN 30 WHEN 'c-work-0-21' THEN 20 ELSE 10 END,
          'KNOWN','comment-like.v1','2026-11-09T00:00:00Z'
        FROM linggan_material_comment WHERE comment_external_id IN ('c-work-0-20','c-work-0-21','c-work-0-22');
        DELETE FROM linggan_comment_clean_cache WHERE source_ref IN
         (SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-4');
        INSERT INTO linggan_comment_problem_membership(request_ref,signal_ordinal,definition_ref,review_invocation_ref,created_at)
        VALUES(md5('metric-request:c-work-0-1')::uuid,1,md5('metric-definition:2')::uuid,
          '00000000-0000-4000-8000-000000004003','2026-11-05T00:00:00Z');
    "#).execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let content: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'",
    ).fetch_one(db.pool()).await.unwrap();
    let original_cards: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_card_voice_ref \
         WHERE content_ref=$1 AND comment_external_id='c-work-0-1' \
           AND problem_ref IN (md5('metric-problem:1')::uuid,md5('metric-problem:2')::uuid)",
    )
    .bind(content)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        original_cards, 2,
        "both original cards must contain the shared voice"
    );
    sqlx::raw_sql(r#"
        ALTER TABLE linggan_material_comment_engagement DISABLE TRIGGER USER;
        UPDATE linggan_material_comment_engagement SET like_count=0
        WHERE material_ref IN (SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1');
        ALTER TABLE linggan_material_comment_engagement ENABLE TRIGGER USER;
        CREATE FUNCTION synthetic_cross_card_barrier() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.problem_ref=md5('metric-problem:2')::uuid THEN
            IF EXISTS (SELECT 1 FROM jsonb_array_elements(NEW.payload#>'{demand,sampleVoices}') voice
              WHERE voice#>>'{commentRef,commentExternalId}'='c-work-0-1') THEN
              RAISE EXCEPTION 'synthetic_first_card_must_drop_shared_voice';
            END IF;
            PERFORM pg_advisory_xact_lock(hashtextextended('synthetic-cross-card-barrier',0));
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER synthetic_cross_card_barrier AFTER UPDATE OF payload ON linggan_opportunity_opportunity_card
        FOR EACH ROW EXECUTE FUNCTION synthetic_cross_card_barrier();
    "#).execute(db.pool()).await.unwrap();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('synthetic-cross-card-barrier',0))")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let rebuild_db = db.clone();
    let pending = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_opportunity_opportunity_card%").await;
    let pool = db.pool().clone();
    let withdrawing = tokio::spawn(async move {
        sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
             VALUES($1,'c-work-0-1','synthetic cross-card withdrawal')")
            .bind(content).execute(&pool).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_material_comment_restriction%").await;
    barrier.rollback().await.unwrap();
    let rebuild = tokio::time::timeout(std::time::Duration::from_secs(30), pending)
        .await
        .unwrap()
        .unwrap();
    let withdrawal = tokio::time::timeout(std::time::Duration::from_secs(30), withdrawing)
        .await
        .unwrap()
        .unwrap();
    assert!(
        rebuild.is_ok(),
        "card rebuild must not deadlock: {rebuild:?}"
    );
    assert!(
        withdrawal.is_ok(),
        "withdrawal must not deadlock: {withdrawal:?}"
    );
    assert!(!payload_contains_comment(&db, content).await);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn legacy_empty_restriction_identity_remains_accepted_when_disabled() {
    let (db, _domain, _digest) = prepare().await;
    let content: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
         VALUES($1,'','synthetic legacy empty identity')")
        .bind(content)
        .execute(db.pool())
        .await
        .unwrap();
    let persisted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_comment_restriction \
         WHERE content_public_ref=$1 AND comment_external_id='')",
    )
    .bind(content)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        persisted,
        "the new trigger must preserve the existing table contract"
    );
    assert_eq!(
        events, 0,
        "the disabled feature must not queue source events"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn due_card_progresses_while_unrelated_source_events_remain() {
    let (db, domain, digest) = prepare().await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    sqlx::raw_sql(
        r#"
        INSERT INTO linggan_opportunity_card_change(problem_ref,changed_at)
        VALUES(md5('metric-problem:1')::uuid,clock_timestamp()-interval '301 seconds')
        ON CONFLICT(problem_ref) DO UPDATE SET changed_at=EXCLUDED.changed_at;
        INSERT INTO linggan_opportunity_card_source_event(event_kind,event_key)
        SELECT 'target',md5('unrelated-queue-target')::uuid::text FROM generate_series(1,1025);
    "#,
    )
    .execute(db.pool())
    .await
    .unwrap();
    let report = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .expect("an unrelated backlog must not starve an already-due card");
    let remaining: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(report.cards_built, 1);
    assert_eq!(remaining, 1, "the event transaction must stay bounded");
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn withdrawal_with_a_fixed_snapshot_cannot_leave_a_new_card_exposed() {
    let (db, domain, digest) = prepare().await;
    sqlx::raw_sql(r#"
        DELETE FROM linggan_opportunity_card_change WHERE problem_ref<>md5('metric-problem:1')::uuid;
        UPDATE linggan_opportunity_card_change SET changed_at=clock_timestamp()-interval '301 seconds';
        CREATE FUNCTION synthetic_card_write_barrier() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.problem_ref=md5('metric-problem:1')::uuid THEN
            IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(NEW.payload#>'{demand,sampleVoices}') voice
              WHERE voice#>>'{commentRef,commentExternalId}'='c-work-0-1'
                AND voice#>>'{commentRef,contentRef}'=md5('creator-work:c1-a')::uuid::text) THEN
              RAISE EXCEPTION 'synthetic_first_card_missing_expected_voice';
            END IF;
            PERFORM pg_advisory_xact_lock(hashtextextended('synthetic-card-write-barrier',0));
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER synthetic_card_write_barrier BEFORE INSERT ON linggan_opportunity_opportunity_card
        FOR EACH ROW EXECUTE FUNCTION synthetic_card_write_barrier();
    "#).execute(db.pool()).await.unwrap();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('synthetic-card-write-barrier',0))")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let rebuild_db = db.clone();
    let pending_rebuild = tokio::spawn(async move {
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%INSERT INTO linggan_opportunity_opportunity_card%").await;
    let content: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let pool = db.pool().clone();
    let (ready, snapshot_ready) = tokio::sync::oneshot::channel();
    let withdrawal = tokio::spawn(async move {
        let mut tx = pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .execute(&mut *tx)
            .await?;
        let existing: i64 =
            sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid")
                .fetch_one(&mut *tx)
                .await?;
        assert_eq!(
            existing, 0,
            "the fixed snapshot must precede the first card"
        );
        ready.send(()).unwrap();
        sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
             VALUES($1,'c-work-0-1','synthetic fixed-snapshot withdrawal')")
            .bind(content)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    snapshot_ready.await.unwrap();
    wait_for_lock(&db, "%INSERT INTO linggan_material_comment_restriction%").await;
    barrier.rollback().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(30), pending_rebuild)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), withdrawal)
        .await
        .unwrap()
        .unwrap();
    println!("fixed-snapshot withdrawal committed: {}", result.is_ok());
    if let Err(error) = result {
        assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("40001"),
            "a conflicting old snapshot may only fail with serialization retry"
        );
        sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
             VALUES($1,'c-work-0-1','synthetic retry after serialization')")
            .bind(content)
            .execute(db.pool())
            .await
            .unwrap();
    }
    let exposed: bool = sqlx::query_scalar("SELECT EXISTS(\
         SELECT 1 FROM linggan_opportunity_card_voice_ref WHERE content_ref=$1 AND comment_external_id='c-work-0-1')")
        .bind(content)
        .fetch_one(db.pool())
        .await
        .unwrap();
    let remaining_cards: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_opportunity_card")
            .fetch_one(db.pool())
            .await
            .unwrap();
    let restricted: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_comment_restriction \
         WHERE content_public_ref=$1 AND comment_external_id='c-work-0-1')",
    )
    .bind(content)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let payload_exposed = payload_contains_comment(&db, content).await;
    println!(
        "fixed-snapshot outcome: cards={remaining_cards}, restricted={restricted}, payload_exposed={payload_exposed}"
    );
    assert!(restricted);
    assert!(
        !exposed && !payload_exposed,
        "a committed withdrawal must not leave the new card exposed"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn withdrawal_protects_a_preexisting_mention_during_a_new_usage() {
    let (db, domain, digest) = prepare().await;
    let content: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    // Model an imported association whose mention predates the domain usage; no source is collected.
    sqlx::query("ALTER TABLE linggan_material_domain_usage DISABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM linggan_material_domain_usage WHERE content_public_ref=$1")
        .bind(content)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("ALTER TABLE linggan_material_domain_usage ENABLE TRIGGER USER")
        .execute(db.pool())
        .await
        .unwrap();
    let mut usage = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,use_kind,basis_kind,package_ref,created_at) \
         SELECT md5('synthetic-new-usage')::uuid,public_ref,$2,'primary','domain_research', \
           'legacy_domain_migration',first_package_ref,'2026-11-03T00:00:00Z' \
         FROM linggan_material_content WHERE public_ref=$1")
        .bind(content)
        .bind(domain)
        .execute(&mut *usage)
        .await
        .unwrap();
    let mut withdrawing = db.pool().begin().await.unwrap();
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
         VALUES($1,'c-work-0-1','synthetic withdrawal while usage is uncommitted')")
        .bind(content)
        .execute(&mut *withdrawing)
        .await
        .unwrap();
    usage.commit().await.unwrap();
    let rebuild_db = db.clone();
    let pending_rebuild = tokio::spawn(async move {
        let rebuild_digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &rebuild_digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%linggan_opportunity_card_touch_source%").await;
    withdrawing.commit().await.unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), pending_rebuild)
        .await
        .unwrap()
        .unwrap();
    match result {
        Ok(_) => {}
        Err(linggan_intelligence::opportunity::CardError::Database(error)) => {
            assert_eq!(
                error
                    .as_database_error()
                    .and_then(|error| error.code())
                    .as_deref(),
                Some("40001")
            );
            rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
                .await
                .unwrap();
        }
        Err(error) => panic!("unexpected card rebuild failure: {error}"),
    }
    let exposed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_opportunity_card_voice_ref \
         WHERE content_ref=$1 AND comment_external_id='c-work-0-1')",
    )
    .bind(content)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let payload_exposed = payload_contains_comment(&db, content).await;
    assert!(
        !exposed && !payload_exposed,
        "a committed withdrawal must cover an existing mention"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn withdrawal_does_not_wait_on_a_card_change_marker() {
    let (db, domain, digest) = prepare().await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let content: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::raw_sql(r#"
        CREATE FUNCTION synthetic_guard_barrier() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
          IF NEW.comment_external_id='c-work-0-1' AND
             current_query() LIKE 'SELECT linggan_opportunity_card_touch_source%' THEN
            PERFORM pg_advisory_xact_lock(hashtextextended('synthetic-guard-barrier',0));
          END IF;
          RETURN NEW;
        END $$;
        CREATE TRIGGER synthetic_guard_barrier BEFORE INSERT ON linggan_opportunity_card_source_guard
        FOR EACH ROW EXECUTE FUNCTION synthetic_guard_barrier();
    "#).execute(db.pool()).await.unwrap();
    let mut barrier = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('synthetic-guard-barrier',0))")
        .execute(&mut *barrier)
        .await
        .unwrap();
    let rebuild_db = db.clone();
    let pending = tokio::spawn(async move {
        let digest = CommenterDigest::from_key(vec![42; 32]).unwrap();
        rebuild_opportunity_cards(&rebuild_db, domain, &digest, CardRebuildMode::Full).await
    });
    wait_for_lock(&db, "%linggan_opportunity_card_touch_source%").await;
    let withdrawal = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
             VALUES($1,'c-work-0-1','synthetic marker contention')")
            .bind(content)
            .execute(db.pool()),
    )
    .await;
    barrier.rollback().await.unwrap();
    withdrawal
        .expect("withdrawal must not wait for the card change marker")
        .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), pending)
        .await
        .unwrap()
        .unwrap();
    match result {
        Err(linggan_intelligence::opportunity::CardError::Database(error)) => assert_eq!(
            error
                .as_database_error()
                .and_then(|error| error.code())
                .as_deref(),
            Some("40001")
        ),
        Ok(_) => assert!(!payload_contains_comment(&db, content).await),
        Err(error) => panic!("unexpected card rebuild failure: {error}"),
    }
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert!(!payload_contains_comment(&db, content).await);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn volume_shift_compares_previous_week_with_a_one_period_series() {
    let (db, domain, digest) = prepare().await;
    sqlx::query(
        "UPDATE linggan_opportunity_metric_definition SET \
         params=jsonb_set(params,'{series_periods}','1'::jsonb) WHERE version=1",
    )
    .execute(db.pool())
    .await
    .unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let card = p1(&db).await;
    assert_eq!(
        card["demand"]["groups"][0]["series"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(card["demand"]["groups"][0]["current"]["n"], 0);
    assert!(
        card["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "collection_volume_shift"),
        "the empty current week must still be compared with the previous week"
    );
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn withdrawal_uses_source_index_and_preserves_unrelated_cards() {
    let (db, domain, digest) = prepare().await;
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let value = p1(&db).await;
    let source = &value["demand"]["sampleVoices"][0]["commentRef"];
    let content = Uuid::parse_str(source["contentRef"].as_str().unwrap()).unwrap();
    let comment = source["commentExternalId"].as_str().unwrap();
    sqlx::raw_sql(r#"
        INSERT INTO linggan_opportunity_problem(problem_ref,source_ref,source_definition_ref,domain_ref,label)
        SELECT md5('index-proof-problem:'||n)::uuid,md5('index-proof-source:'||n)::uuid,
          md5('index-proof-definition:'||n)::uuid,'00000000-0000-4000-8000-000000000001','synthetic unrelated'
        FROM generate_series(1,1000) n;
        INSERT INTO linggan_opportunity_opportunity_card(problem_ref,payload,as_of,taxonomy_version,metric_def_version,input_fingerprint)
        SELECT md5('index-proof-problem:'||n)::uuid,
          jsonb_build_object('demand',jsonb_build_object('sampleVoices',jsonb_build_array(
            jsonb_build_object('commentRef',jsonb_build_object('contentRef',md5('index-proof-work:'||n)::uuid,
              'commentExternalId','synthetic-unrelated'))))),
          '2026-11-10T00:00:00Z',1,1,repeat('a',64)
        FROM generate_series(1,1000) n;
        ANALYZE linggan_opportunity_card_voice_ref;
    "#).execute(db.pool()).await.unwrap();
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT problem_ref FROM linggan_opportunity_card_voice_ref \
         WHERE content_ref=$1 AND comment_external_id=$2",
    )
    .bind(content)
    .bind(comment)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert!(
        plan.iter()
            .any(|line| line.contains("linggan_opportunity_card_voice_ref_source")),
        "the source lookup must use its index: {plan:?}"
    );
    sqlx::query("INSERT INTO linggan_material_comment_restriction(content_public_ref,comment_external_id,reason) \
         VALUES($1,$2,'synthetic indexed withdrawal')")
        .bind(content)
        .bind(comment)
        .execute(db.pool())
        .await
        .unwrap();
    let target: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_opportunity_card WHERE problem_ref=md5('metric-problem:1')::uuid",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let others: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_opportunity_opportunity_card \
         WHERE payload#>>'{demand,sampleVoices,0,commentRef,commentExternalId}'='synthetic-unrelated'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(target, 0);
    assert_eq!(others, 1000);
}

#[tokio::test]
#[ignore = "requires the disposable opportunity PostgreSQL proof"]
async fn card_refills_when_clean_cache_arrives_and_reports_skip_reasons() {
    let (db, domain, digest) = prepare().await;
    let _events_gate = enable_source_events(&db).await;
    sqlx::query(
        "DELETE FROM linggan_comment_clean_cache WHERE source_ref IN \
        (SELECT material_ref FROM linggan_material_comment WHERE comment_external_id='c-work-0-1')",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let first = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(first.voice_exclusion_reasons.get("indexPending"), Some(&1));
    assert_eq!(p1(&db).await["demand"]["sampleVoices"][0]["likes"], 30);
    sqlx::query("INSERT INTO linggan_comment_clean_cache(source_ref,cleaner_version,raw_sha256,research_text,clean_state,clean_reasons,created_at) \
        SELECT material_ref,'comment-clean.v2',repeat('a',64),body_text,'direct','[]'::jsonb,'2026-11-09T00:00:00Z' \
        FROM linggan_material_comment WHERE comment_external_id='c-work-0-1'")
        .execute(db.pool()).await.unwrap();
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_opportunity_card_source_event")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(events > 0, "the cache arrival must publish an event");
    let waiting = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(waiting.cards_built, 0);
    sqlx::query("UPDATE linggan_opportunity_card_change SET changed_at=clock_timestamp()-interval '301 seconds'")
        .execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Due)
        .await
        .unwrap();
    assert_eq!(p1(&db).await["demand"]["sampleVoices"][0]["likes"], 40);

    // A newer, readable observation with changed text cannot inherit the old association.
    sqlx::raw_sql(r#"
        INSERT INTO linggan_runtime_task(task_id,task_spec_hash,task_spec,source,platform,page_type,created_at)
        VALUES(md5('card-revision-task')::uuid,repeat('d',64),'{}','manual','xhs','seed','2026-11-06T00:00:00Z');
        INSERT INTO linggan_runtime_attempt(attempt_id,task_id,producer_instance_id,started_at)
        VALUES(md5('card-revision-attempt')::uuid,md5('card-revision-task')::uuid,
          md5('card-revision-producer')::uuid,'2026-11-06T00:00:00Z');
        INSERT INTO linggan_runtime_capture_package(package_ref,attempt_id,task_id,producer_instance_id,
          package_kind,platform,package_hash,observed_at,captured_at,coverage,payload,accepted_at)
        VALUES(md5('card-revision-package')::uuid,md5('card-revision-attempt')::uuid,
          md5('card-revision-task')::uuid,md5('card-revision-producer')::uuid,'comments','xhs',repeat('e',64),
          '2026-11-06T00:00:00Z','2026-11-06T00:00:00Z','{}','{}','2026-11-06T00:00:00Z');
        INSERT INTO linggan_runtime_record_disposition(package_ref,record_ordinal,disposition,reason,created_at)
        VALUES(md5('card-revision-package')::uuid,9999,'accepted_for_library_content',
          'synthetic body revision','2026-11-06T00:00:00Z');
        INSERT INTO linggan_material_comment(material_ref,content_public_ref,package_ref,record_ordinal,
          comment_external_id,root_comment_external_id,is_reply,body_text,body_state,author_external_id,observed_at,created_at)
        SELECT md5('card-changed-body')::uuid,content_public_ref,md5('card-revision-package')::uuid,9999,comment_external_id,
          root_comment_external_id,false,'SYNTHETIC changed body','KNOWN',author_external_id,
          '2026-11-06T00:00:00Z','2026-11-06T00:00:00Z'
        FROM linggan_material_comment WHERE comment_external_id='c-work-0-1';
        INSERT INTO linggan_comment_clean_cache(source_ref,cleaner_version,raw_sha256,research_text,clean_state,clean_reasons,created_at)
        VALUES(md5('card-changed-body')::uuid,'comment-clean.v2',repeat('b',64),'SYNTHETIC changed body','direct','[]','2026-11-06T00:00:00Z');
    "#).execute(db.pool()).await.unwrap();
    let changed = rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    assert_eq!(
        changed.voice_exclusion_reasons.get("sourceBodyChanged"),
        Some(&1)
    );
    assert_eq!(p1(&db).await["demand"]["sampleVoices"][0]["likes"], 30);
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
    preserve_synthetic_marketing(&db).await;
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
        !value["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "collection_protocol_changed"),
        "unknown and one known version cannot establish an actual protocol change"
    );
    assert!(
        value["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("未知协议"))
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
    // Synthetic fixture only: give one first-admission package a second known version.
    sqlx::raw_sql("ALTER TABLE linggan_runtime_capture_package DISABLE TRIGGER USER; \
        UPDATE linggan_runtime_capture_package SET payload=jsonb_set(payload,'{collectionProtocol,version}','2'::jsonb) \
        WHERE package_ref=md5('creator-package:c2-v1:comments')::uuid; \
        ALTER TABLE linggan_runtime_capture_package ENABLE TRIGGER USER")
        .execute(db.pool()).await.unwrap();
    rebuild_opportunity_cards(&db, domain, &digest, CardRebuildMode::Full)
        .await
        .unwrap();
    let known_change = p1(&db).await;
    assert!(
        known_change["flags"]
            .as_array()
            .unwrap()
            .iter()
            .any(|flag| flag == "collection_protocol_changed")
    );
    let versions = known_change["demand"]["groups"].as_array().unwrap();
    assert!(versions.iter().any(|group| group["protocolVersion"] == 1));
    assert!(versions.iter().any(|group| group["protocolVersion"] == 2));
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
    assert!(
        !outside_window["limitations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str().unwrap().contains("采集协议 2"))
    );
}
