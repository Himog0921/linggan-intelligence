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
    let seed=sqlx::query("SELECT c.budget_mode,s.revision FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='note_interpretation'").fetch_one(db.pool()).await.unwrap();
    assert_eq!(seed.get::<String, _>("budget_mode"), "unlimited");
    let base_revision = seed.get::<i64, _>("revision");
    let first = SaveTaskConfig {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::NoteInterpretation,
        expected_revision: base_revision,
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
    assert_eq!(one.revision, base_revision + 1);
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
    competing.expected_revision = base_revision + 1;
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
    exceeds.expected_revision = base_revision + 2;
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
    rotated.expected_revision = base_revision + 2;
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
        expected_revision: base_revision + 3,
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

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn prompt_switch_preserves_an_unqualified_model_without_qualifying_it() {
    use linggan_intelligence::ai_foundation::prompt::{self, ActivatePrompt, SavePrompt};
    use linggan_intelligence::ai_foundation::skill_binding::{SwitchSkill, switch_skill};
    use linggan_intelligence::ai_foundation::skill_store::{ImportSkill, import_skill};
    let db = fixture::proof_database("ai_prompt_legacy_selection").await;
    let conn = new_connection(&db).await;
    let v = version(&db, conn, 1, "https://legacy.example/api").await;
    let selected = model(&db, v).await;
    let model_config = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_config(config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts) VALUES($1,$2,8192,2048,45,2)")
        .bind(model_config).bind(selected).execute(db.pool()).await.unwrap();
    let seeded = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,prompt_version_ref) SELECT $1,c.task_key,$2,c.skill_version_ref,c.execution_contract,c.material_columns,c.budget_mode,c.daily_token_limit,c.allowed_tools,'seed',c.prompt_version_ref FROM linggan_ai_task_setting s JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref WHERE s.task_key='writing_legacy'")
        .bind(seeded).bind(model_config).execute(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_ai_task_setting SET current_config_ref=$1,revision=revision+1 WHERE task_key='writing_legacy'")
        .bind(seeded).execute(db.pool()).await.unwrap();
    let current = prompt::read(&db, TaskKey::WritingLegacy).await.unwrap();
    let revision = current["revision"].as_i64().unwrap();
    let saved = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_task_revision: revision,
            system_text: "synthetic system".into(),
            instruction_text: "synthetic instruction".into(),
        },
    )
    .await
    .unwrap();
    let enabled = prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WritingLegacy,
            expected_task_revision: revision,
            version_ref: saved.version_ref,
        },
    )
    .await
    .unwrap();
    let (stored, capability): (Uuid, Option<Uuid>) = sqlx::query_as("SELECT c.model_config_ref,m.capability_ref FROM linggan_ai_task_config c JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE c.task_config_ref=$1")
        .bind(enabled.task_config_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(stored, model_config);
    assert!(
        capability.is_none(),
        "switching a prompt cannot qualify the model"
    );
    let skill = import_skill(
        &db,
        &linggan_intelligence::pi_adapter::PiAdapter::configured(),
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: None,
            package: super::skills::package(),
        },
    )
    .await
    .unwrap();
    let switch = SwitchSkill {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::WritingLegacy,
        expected_task_revision: enabled.task_revision,
        expected_binding_revision: 0,
        version_ref: skill.version_ref,
    };
    let skill_switched = switch_skill(&db, &switch).await.unwrap();
    assert_eq!(switch_skill(&db, &switch).await.unwrap(), skill_switched);
    let (stored_model, stored_prompt):(Uuid, Uuid) = sqlx::query_as("SELECT model_config_ref,prompt_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$1")
        .bind(skill_switched.task_config_ref).fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        (stored_model, stored_prompt),
        (model_config, saved.version_ref)
    );
    let denied = sqlx::query("INSERT INTO linggan_ai_task_config(task_config_ref,task_key,model_config_ref,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,origin,command_ref,prompt_version_ref) SELECT $1,task_key,NULL,skill_version_ref,execution_contract,material_columns,budget_mode,daily_token_limit,allowed_tools,'prompt_activation',$2,prompt_version_ref FROM linggan_ai_task_config WHERE task_config_ref=$3")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(enabled.task_config_ref).execute(db.pool()).await.unwrap_err();
    assert!(
        denied
            .to_string()
            .contains("ai_prompt_activation_changed_other_settings")
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; configuration changes never dispatch"]
async fn research_methods_are_editable_and_switching_never_dispatches() {
    use linggan_intelligence::ai_foundation::control_store::{
        Actor, ChangeControl, ControlScope, change_control,
    };
    use linggan_intelligence::ai_foundation::prompt::{self, ActivatePrompt, SavePrompt};
    use linggan_intelligence::ai_foundation::skill_binding::{SwitchSkill, switch_skill};
    use linggan_intelligence::ai_foundation::skill_store::{ImportSkill, import_skill};
    use linggan_intelligence::ai_foundation::store::FoundationError;
    let db = fixture::proof_database("ai_research_methods_current").await;
    let skill = import_skill(
        &db,
        &linggan_intelligence::pi_adapter::PiAdapter::configured(),
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: None,
            package: super::skills::package(),
        },
    )
    .await
    .unwrap();
    let mut revised_package = super::skills::package();
    revised_package.files[0]
        .content
        .push_str("\nA revised synthetic method.\n");
    let second_skill = import_skill(
        &db,
        &linggan_intelligence::pi_adapter::PiAdapter::configured(),
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: Some(skill.version_ref),
            package: revised_package,
        },
    )
    .await
    .unwrap();
    for task in [
        TaskKey::CommentStudy,
        TaskKey::CommentProblemResolution,
        TaskKey::CommentProblemPairReview,
        TaskKey::TopicResearch,
        TaskKey::TopicResearchExtractReview,
        TaskKey::TopicResearchResolve,
        TaskKey::TopicResearchResolveReview,
        TaskKey::CreatorResearch,
        TaskKey::CreatorAccountResearch,
    ] {
        let original = prompt::read(&db, task).await.unwrap();
        let original_ref: Uuid =
            serde_json::from_value(original["activeVersionRef"].clone()).unwrap();
        let revision = original["revision"].as_i64().unwrap();
        let saved = prompt::save(
            &db,
            &SavePrompt {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: revision,
                system_text: "synthetic editable system".into(),
                instruction_text: "synthetic editable instruction".into(),
            },
        )
        .await
        .unwrap();
        let enabled = prompt::activate(
            &db,
            &ActivatePrompt {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: revision,
                version_ref: saved.version_ref,
            },
        )
        .await
        .unwrap();
        let rollback = prompt::activate(
            &db,
            &ActivatePrompt {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: enabled.task_revision,
                version_ref: original_ref,
            },
        )
        .await
        .unwrap();
        let changed = switch_skill(
            &db,
            &SwitchSkill {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: rollback.task_revision,
                expected_binding_revision: 0,
                version_ref: skill.version_ref,
            },
        )
        .await
        .unwrap();
        let newer = switch_skill(
            &db,
            &SwitchSkill {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: changed.task_revision,
                expected_binding_revision: changed.binding_revision,
                version_ref: second_skill.version_ref,
            },
        )
        .await
        .unwrap();
        let restored = switch_skill(
            &db,
            &SwitchSkill {
                command_ref: Uuid::new_v4(),
                task_key: task,
                expected_task_revision: newer.task_revision,
                expected_binding_revision: newer.binding_revision,
                version_ref: skill.version_ref,
            },
        )
        .await
        .unwrap();
        let stored: (String, Option<String>, Option<Uuid>, Option<Uuid>, bool) = sqlx::query_as(
            "SELECT t.lifecycle,t.handler_contract,c.prompt_version_ref,c.skill_version_ref,ctl.enabled \
             FROM linggan_ai_task t JOIN linggan_ai_task_setting s USING(task_key) \
             JOIN linggan_ai_task_config c ON c.task_config_ref=s.current_config_ref \
             JOIN linggan_ai_control ctl ON ctl.task_key=t.task_key WHERE t.task_key=$1")
            .bind(task.as_str()).fetch_one(db.pool()).await.unwrap();
        assert_eq!(
            stored,
            (
                "active".into(),
                Some(
                    match task {
                        TaskKey::CommentStudy => "comment-research-semantic.v1",
                        TaskKey::CommentProblemResolution | TaskKey::CommentProblemPairReview =>
                            "comment-problem-stage.v1",
                        TaskKey::CreatorResearch => "creator-work-research.v1",
                        TaskKey::CreatorAccountResearch => "creator-account-research.v1",
                        _ => "topic-research-stage.v1",
                    }
                    .into()
                ),
                Some(original_ref),
                Some(skill.version_ref),
                false
            )
        );
        assert_eq!(changed.task_revision, rollback.task_revision + 1);
        assert_eq!(restored.task_revision, rollback.task_revision + 3);
        let attempted = change_control(
            &db,
            &ChangeControl {
                command_ref: Uuid::new_v4(),
                scope: ControlScope::Task(task),
                expected_revision: 0,
                enabled: true,
                reason: "synthetic proof".into(),
            },
            Actor::Human,
        )
        .await;
        assert!(attempted.is_ok());
    }
    let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_model_invocation")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(calls, 0);
}
