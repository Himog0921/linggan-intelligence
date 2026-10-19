use super::{endpoint, fixture, model, new_connection, version};
use linggan_intelligence::ai_foundation::{
    TaskKey,
    capability::ModelLimits,
    control::BudgetPolicy,
    store::FoundationError,
    task_config::{MaterialColumn, ModelSelection, SaveTaskConfig, save_task_config},
};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_task_config_is_cas_frozen_capability_bounded_and_does_not_enable_work() {
    let db = fixture::proof_database("ai_task_configs").await;
    let connection = new_connection(&db).await;
    let v1 = version(&db, connection, 1, "https://a.example/api").await;
    let selected = model(&db, v1).await;
    let route = endpoint(&db, v1).await;
    let capability = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','config-proof',1000000,384000,600,'[\"off\",\"high\"]','{\"supportsReasoningEffort\":true}','{}','prompt_only')")
        .bind(capability).bind(selected).bind(route).execute(db.pool()).await.unwrap();
    let seed:String=sqlx::query_scalar("SELECT c.budget_mode FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='note_interpretation'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(seed, "unlimited");
    let first = SaveTaskConfig {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::NoteInterpretation,
        expected_revision: 0,
        model: Some(ModelSelection {
            model_ref: selected,
            capability_ref: capability,
            limits: ModelLimits {
                input_tokens: 500000,
                output_tokens: 384000,
                timeout_seconds: 300,
                attempts: 2,
                thinking_level: "high".into(),
            },
        }),
        skill_version_ref: None,
        material_columns: [
            MaterialColumn::Title,
            MaterialColumn::Body,
            MaterialColumn::Ocr,
        ]
        .into(),
        budget: BudgetPolicy::Unlimited,
    };
    let one = save_task_config(&db, &first).await.unwrap();
    assert_eq!(one.revision, 1);
    assert_eq!(save_task_config(&db, &first).await.unwrap(), one);
    let mut conn = db.pool().acquire().await.unwrap();
    let frozen = linggan_intelligence::ai_foundation::transport_store::load_frozen_in(
        &mut conn,
        one.model_config_ref.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(frozen.connection_version_ref, v1);
    assert_eq!(frozen.execution.capability_ref, capability);
    let request = frozen.request(
        "SYNTHETIC-NOT-A-SECRET".into(),
        "Synthetic".into(),
        "Return JSON".into(),
        None,
    );
    assert_eq!(request.max_output_tokens, 384000);
    assert_eq!(request.timeout_ms, 300000);
    assert_eq!(request.execution.unwrap().thinking_level, "high");
    drop(conn);
    let config: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(c) FROM linggan_ai_task_config c WHERE task_config_ref=$1",
    )
    .bind(one.task_config_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let mut competing = first.clone();
    competing.command_ref = Uuid::new_v4();
    competing.expected_revision = 1;
    competing.budget = BudgetPolicy::Capped(1000);
    let mut other = competing.clone();
    other.command_ref = Uuid::new_v4();
    other.budget = BudgetPolicy::Capped(2000);
    let (a, b) = tokio::join!(
        save_task_config(&db, &competing),
        save_task_config(&db, &other)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    assert!(matches!(
        a.as_ref().err().or(b.as_ref().err()),
        Some(FoundationError::Conflict)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT to_jsonb(c) FROM linggan_ai_task_config c WHERE task_config_ref=$1"
        )
        .bind(one.task_config_ref)
        .fetch_one(db.pool())
        .await
        .unwrap(),
        config
    );
    let mut exceeds = first.clone();
    exceeds.command_ref = Uuid::new_v4();
    exceeds.expected_revision = 2;
    exceeds.model.as_mut().unwrap().limits.input_tokens = 900000;
    assert!(matches!(
        save_task_config(&db, &exceeds).await,
        Err(FoundationError::Rule(_))
    ));
    assert!(sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,capability_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts,thinking_level) VALUES($1,$2,$3,900000,384000,300,1,'high')")
        .bind(Uuid::new_v4()).bind(selected).bind(capability).execute(db.pool()).await.is_err());
    let v2 = version(&db, connection, 2, "https://a.example/api").await;
    let mut conn = db.pool().acquire().await.unwrap();
    let carried = linggan_intelligence::ai_foundation::transport_store::load_frozen_in(
        &mut conn,
        one.model_config_ref.unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(
        carried.connection_version_ref, v2,
        "the frozen model selection must use the current key version in the same endpoint epoch"
    );
    assert_eq!(carried.execution.capability_ref, capability);
    drop(conn);
    let mut rotated = first.clone();
    rotated.command_ref = Uuid::new_v4();
    rotated.expected_revision = 2;
    let two = save_task_config(&db, &rotated).await.unwrap();
    let current:Uuid=sqlx::query_scalar("SELECT m.connection_version_ref FROM linggan_model_config c JOIN linggan_model_entry m USING(model_ref) WHERE c.config_ref=$1")
        .bind(two.model_config_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        current, v2,
        "stable model selection carries to the current secret version"
    );
    version(&db, connection, 3, "https://b.example/api").await;
    let mut conn = db.pool().acquire().await.unwrap();
    assert!(matches!(
        linggan_intelligence::ai_foundation::transport_store::load_frozen_in(
            &mut conn,
            one.model_config_ref.unwrap()
        )
        .await,
        Err(FoundationError::Conflict)
    ));
    drop(conn);
    let stale = SaveTaskConfig {
        command_ref: Uuid::new_v4(),
        expected_revision: 3,
        ..first.clone()
    };
    assert!(matches!(
        save_task_config(&db, &stale).await,
        Err(FoundationError::Conflict)
    ));
    assert!(
        sqlx::query(
            "UPDATE linggan_ai_task_config SET budget_mode='unconfigured' WHERE task_config_ref=$1"
        )
        .bind(one.task_config_ref)
        .execute(db.pool())
        .await
        .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "configuration save and frozen request construction are not dispatch"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control WHERE enabled")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_upgrade_keeps_the_original_writing_selection_without_inventing_capability_or_quota()
 {
    let migration = include_str!("../../../../database/migrations/0134_ai_gen1_task_config.sql");
    let (before, _) = fixture::MIGRATIONS.split_once(migration).unwrap();
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "ai_task_config_upgrade",
        before,
    )
    .await
    .unwrap();
    let conn = new_connection(&db).await;
    let v = version(&db, conn, 1, "https://legacy.example/api").await;
    let selected = model(&db, v).await;
    let config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,2048,45,2)")
        .bind(config).bind(selected).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_model_workspace SET default_config_ref=$1 WHERE singleton")
        .bind(config)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(migration.to_owned()))
        .execute(db.pool())
        .await
        .unwrap();
    let row=sqlx::query("SELECT s.revision,c.origin,c.model_config_ref,c.budget_mode,m.capability_ref FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE s.task_key='writing_legacy'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.get::<i64, _>("revision"), 0);
    assert_eq!(row.get::<Uuid, _>("model_config_ref"), config);
    assert_eq!(row.get::<String, _>("origin"), "legacy_import");
    assert_eq!(row.get::<String, _>("budget_mode"), "unconfigured");
    assert_eq!(row.get::<Option<Uuid>, _>("capability_ref"), None);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_control WHERE enabled")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_settings_use_connection_cas_and_preserve_selection_without_inheriting_key_probe()
 {
    use linggan_intelligence::ai_foundation::settings_read::read_settings;
    use linggan_intelligence::model_settings::{
        SetModelConnectionEnabled, set_model_connection_enabled,
    };
    let db = fixture::proof_database("ai_settings_revision_projection").await;
    let connection = new_connection(&db).await;
    let first = version(&db, connection, 1, "https://synthetic.example/v1").await;
    let selected = model(&db, first).await;
    sqlx::query("UPDATE linggan_model_connection SET revision=1 WHERE connection_ref=$1")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    set_model_connection_enabled(
        &db,
        &SetModelConnectionEnabled {
            connection_ref: connection,
            expected_revision: 1,
            enabled: false,
        },
    )
    .await
    .unwrap();
    let snapshot = read_settings(&db, true).await.unwrap();
    let current = snapshot["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["connectionRef"] == serde_json::json!(connection))
        .unwrap();
    assert_eq!(current["revision"], 2);
    assert_eq!(current["enabled"], false);
    let second = version(&db, connection, 3, "https://synthetic.example/v1").await;
    sqlx::query("UPDATE linggan_model_connection SET revision=3 WHERE connection_ref=$1")
        .bind(connection)
        .execute(db.pool())
        .await
        .unwrap();
    let rotated = read_settings(&db, true).await.unwrap();
    let current = rotated["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["connectionRef"] == serde_json::json!(connection))
        .unwrap();
    let selection = rotated["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["connectionRef"] == serde_json::json!(connection))
        .unwrap();
    assert_eq!(current["versionRef"], serde_json::json!(second));
    assert_eq!(selection["selectionModelRef"], serde_json::json!(selected));
    assert!(selection["modelRef"].is_null());
    assert!(selection["probe"].is_null());
    assert_eq!(rotated["resources"]["contract"], "ai-resource-limits.v1");
    assert_eq!(
        rotated["protocol"]["editableConnectionApis"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}
