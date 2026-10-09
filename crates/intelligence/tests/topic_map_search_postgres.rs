//! Temporary search boundary proof against a disposable PostgreSQL schema; no platform traffic.
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod schema;
use linggan_contracts::LifecycleState;
use linggan_evidence::{
    AuthorizationGrant, KeywordDetailAdvance, advance_keyword_archive_detail, count_targets,
    grant_authorization, list_targets, list_targets_in_state,
};
use linggan_intelligence::topic_map_collection_search::*;
use linggan_intelligence::topic_map_research::ResearchError;
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use uuid::Uuid;
async fn database() -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = isolated_proof_schema(&url, "topic_map_search_boundaries", schema::FULL_MIGRATIONS)
        .await
        .unwrap();
    let installed: bool =
        sqlx::query_scalar("SELECT to_regclass('linggan_topic_map_search_target')IS NOT NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
    if !installed {
        sqlx::raw_sql(include_str!(
            "../../../database/migrations/0119_topic_map_collection_search.sql"
        ))
        .execute(db.pool())
        .await
        .unwrap();
    }
    db
}
async fn policy(db: &Database, domain: Uuid) {
    let conn = Uuid::new_v4();
    let version = Uuid::new_v4();
    let model = Uuid::new_v4();
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_connection(connection_ref,revision)VALUES($1,1)")
        .bind(conn)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO linggan_model_connection_version(version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref)VALUES($1,$2,1,'Synthetic','openai-completions','http://127.0.0.1:18080',true,$3)").bind(version).bind(conn).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin)VALUES($1,$2,'synthetic-no-call','manual')").bind(model).bind(version).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts)VALUES($1,$2,8192,1000,30,3)").bind(config).bind(model).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_topic_map_research_policy(domain_ref,model_config_ref,daily_token_limit,run_token_limit,collection_enabled)VALUES($1,$2,100000,100000,true)").bind(domain).bind(config).execute(db.pool()).await.unwrap();
}
#[tokio::test]
#[ignore = "disposable PostgreSQL proof harness"]
async fn temporary_search_reuses_round_hides_targets_and_never_fans_out() {
    let db = database().await;
    let domain = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    policy(&db, domain).await;
    let grant = grant_authorization(
        &db,
        &AuthorizationGrant {
            platform: "xhs",
            target_kind: "keyword",
            lane: "deep_archive",
            purpose: "SYNTHETIC 限定搜索测试",
            max_targets: Some(3),
            max_works_per_target: Some(200),
            valid_for_days: 1,
        },
    )
    .await
    .unwrap();
    let command = SearchCommand::StartSearch {
        request_ref: Uuid::new_v4(),
        domain_ref: domain,
        topic_ref: None,
        authorization_ref: grant,
        keywords: vec![
            "合成屏幕讨论".into(),
            "合成实践支持".into(),
            "合成环境转换".into(),
        ],
        purpose: "SYNTHETIC 严格200发现/10详情".into(),
        new_round: false,
    };
    let receipt = apply_search_command(&db, &command).await.unwrap();
    assert_eq!(receipt, apply_search_command(&db, &command).await.unwrap());
    let round: Uuid = receipt["roundRef"].as_str().unwrap().parse().unwrap();
    let quotas:Vec<i32>=sqlx::query_scalar("SELECT candidate_quota FROM linggan_topic_map_search_target WHERE round_ref=$1 ORDER BY ordinal").bind(round).fetch_all(db.pool()).await.unwrap();
    assert_eq!(quotas.len(), 3);
    assert_eq!(quotas.iter().sum::<i32>(), 200);
    let listed = list_targets(&db, None, Some(domain), 100).await.unwrap();
    assert!(listed.is_empty());
    assert_eq!(count_targets(&db, Some(domain)).await.unwrap().total, 0);
    assert!(
        list_targets_in_state(&db, LifecycleState::PendingDecision, 100)
            .await
            .unwrap()
            .is_empty()
    );
    let target: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM linggan_topic_map_search_target WHERE round_ref=$1 LIMIT 1",
    )
    .bind(round)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(matches!(
        advance_keyword_archive_detail(&db, target, "must not fanout", "person")
            .await
            .unwrap(),
        KeywordDetailAdvance::Skipped(
            "topic_map_round_owns_detail_scope" | "collection_upgrade_recovery_only"
        )
    ));
    assert!(
        sqlx::query(
            "UPDATE collection_observation_target SET monitoring_enabled=true WHERE target_ref=$1"
        )
        .bind(target)
        .execute(db.pool())
        .await
        .is_err()
    );
    assert!(sqlx::query("UPDATE collection_observation_target SET purpose_kind='long_term',purpose_round_ref=NULL WHERE target_ref=$1").bind(target).execute(db.pool()).await.is_err());
    let progress = read_search_progress(&db, domain).await.unwrap();
    assert_eq!(progress["authorizations"].as_array().unwrap().len(), 1);
    assert_eq!(progress["rounds"][0]["candidateCount"], 0);
    assert_eq!(progress["rounds"][0]["detailSlotsReserved"], 0);
    sqlx::query(
        "UPDATE linggan_topic_map_research_policy SET collection_enabled=false WHERE domain_ref=$1",
    )
    .bind(domain)
    .execute(db.pool())
    .await
    .unwrap();
    let disabled_freeze = SearchCommand::FreezeSearchDetails {
        request_ref: Uuid::new_v4(),
        domain_ref: domain,
        round_ref: round,
        work_refs: vec![Uuid::new_v4()],
    };
    assert!(matches!(
        apply_search_command(&db, &disabled_freeze).await,
        Err(ResearchError::Invalid("collection_not_enabled"))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_topic_map_collection_slot")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    sqlx::query(
        "UPDATE linggan_topic_map_research_policy SET collection_enabled=true WHERE domain_ref=$1",
    )
    .bind(domain)
    .execute(db.pool())
    .await
    .unwrap();
    let frozen = SearchCommand::FreezeSearchDetails {
        request_ref: Uuid::new_v4(),
        domain_ref: domain,
        round_ref: round,
        work_refs: (0..11).map(|_| Uuid::new_v4()).collect(),
    };
    assert!(matches!(
        apply_search_command(&db, &frozen).await,
        Err(ResearchError::Invalid(_))
    ));
    let stop = SearchCommand::StopSearch {
        request_ref: Uuid::new_v4(),
        domain_ref: domain,
        round_ref: round,
    };
    apply_search_command(&db, &stop).await.unwrap();
    advance_search_rounds_once(&db).await.unwrap();
    assert_eq!(
        read_search_round_progress(&db, domain, round)
            .await
            .unwrap()["state"],
        "stopped"
    );
    let mut reopen = command.clone();
    if let SearchCommand::StartSearch { request_ref, .. } = &mut reopen {
        *request_ref = Uuid::new_v4();
    }
    assert_eq!(
        apply_search_command(&db, &reopen).await.unwrap()["roundRef"],
        round.to_string()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_topic_map_collection_round")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*)FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}
