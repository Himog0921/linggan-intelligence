use super::{endpoint, fixture, model, new_connection, skills, version};
use linggan_intelligence::{
    ai_foundation::{
        TaskKey,
        batch_store::{
            BatchInput, BatchKind, CreateBatch, CreateTrial, InputRef, ProbeOperation,
            create_batch, create_trial,
        },
        capability::ModelLimits,
        control::BudgetPolicy,
        control_store::{Actor, ChangeControl, ControlScope, change_control},
        provider_store::{ChangeEgress, MaterialClass, change_egress},
        skill_binding::{ActivateSkill, activate_skill},
        skill_store::{ImportSkill, import_skill},
        store::FoundationError,
        task_config::{
            MaterialColumn, ModelSelection, SaveTaskConfig, TaskConfigReceipt, save_task_config,
        },
    },
    pi_adapter::PiAdapter,
};
use linggan_storage_postgres::Database;
use serde_json::json;
use uuid::Uuid;

pub(super) async fn policy(db: &Database) -> TaskConfigReceipt {
    save_task_config(
        db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::ModelProbe,
            expected_revision: 0,
            model: None,
            skill_version_ref: None,
            material_columns: Default::default(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap()
}
pub(super) async fn on(db: &Database, task: TaskKey) {
    for scope in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(task),
    ] {
        change_control(
            db,
            &ChangeControl {
                command_ref: Uuid::new_v4(),
                scope,
                expected_revision: 0,
                enabled: true,
                reason: "synthetic fixture intent".into(),
            },
            Actor::Human,
        )
        .await
        .unwrap();
    }
}
pub(super) async fn grant(
    db: &Database,
    endpoint: Uuid,
    task: TaskKey,
    material_class: MaterialClass,
) {
    change_egress(
        db,
        &ChangeEgress {
            command_ref: Uuid::new_v4(),
            endpoint_ref: endpoint,
            task_key: task,
            material_class,
            expected_event_ref: None,
            allowed: true,
            reason: "synthetic fixture grant".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
}
fn inputs(version: Uuid) -> Vec<BatchInput> {
    vec![BatchInput {
        item_key: "probe-1".into(),
        input: InputRef::Probe {
            sample_ref: Uuid::new_v4(),
            connection_version_ref: version,
            model_ref: None,
            operation: ProbeOperation::Connect,
            profile: None,
        },
    }]
}
/// Storage proof only. The complete executor proof later uses an actual loopback provider.
async fn recorded_synthetic_attempt(db: &Database, batch: Uuid, version: Uuid) {
    let item: Uuid = sqlx::query_scalar(
        "SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1 ORDER BY position LIMIT 1",
    )
    .bind(batch)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        db,
        "synthetic-storage-fixture",
        4,
    )
    .await
    .unwrap();
    let mut tx = owner.fence().begin(db, true).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_generation=1,lease_token=$2,lease_instance_ref=$3,lease_instance_generation=$4,lease_expires_at=scope_001_now()+interval '1 minute' WHERE item_ref=$1")
        .bind(item).bind(Uuid::new_v4()).bind(owner.fence().instance_ref()).bind(owner.fence().generation()).execute(&mut *tx).await.unwrap();
    let invocation = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens)
        SELECT $1,$2,COALESCE((i.input_ref->>'modelRef')::uuid,m.model_ref),COALESCE(i.input_ref->>'operation','analyze'),repeat('0',64),'running',0,0
        FROM linggan_ai_batch_item i JOIN linggan_ai_batch b USING(batch_ref)
        JOIN linggan_ai_task_config c ON c.task_config_ref=b.task_config_ref LEFT JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE i.item_ref=$3")
        .bind(invocation).bind(version).bind(item).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control control SET invocation_ref=$2,attempt_count=1,
        executed_config_ref=config.task_config_ref,executed_skill_version_ref=config.skill_version_ref,execution_contract=config.execution_contract
        FROM linggan_ai_batch_item item JOIN linggan_ai_batch batch USING(batch_ref)
        JOIN linggan_ai_task_config config ON config.task_config_ref=batch.task_config_ref
        WHERE control.item_ref=item.item_ref AND item.item_ref=$1")
        .bind(item).bind(invocation).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_model_invocation SET state='succeeded',input_tokens=0,output_tokens=0,finished_at=scope_001_now(),result='{\"syntheticFixture\":true}' WHERE invocation_ref=$1")
        .bind(invocation).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_ai_batch_item_control SET state='succeeded',lease_token=NULL,lease_expires_at=NULL,lease_instance_ref=NULL,lease_instance_generation=NULL,usable=true,output='{\"syntheticFixture\":true}',finished_at=scope_001_now() WHERE item_ref=$1")
        .bind(item).execute(&mut *tx).await.unwrap();
    let round: Option<Uuid> = sqlx::query_scalar("SELECT linggan_ai_evaluate_next_round($1)")
        .bind(batch)
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(round.is_some());
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_queue_requires_current_controls_config_and_egress_but_does_not_dispatch() {
    let db = fixture::proof_database("ai_batch_queue").await;
    let config = policy(&db).await;
    let connection = new_connection(&db).await;
    let version = version(&db, connection, 1, "https://queue.example/api").await;
    let route = endpoint(&db, version).await;
    let request = CreateBatch {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::ModelProbe,
        expected_task_revision: 1,
        task_config_ref: config.task_config_ref,
        kind: BatchKind::Probe,
        items: inputs(version),
    };
    assert!(matches!(
        create_batch(&db, &request).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    on(&db, TaskKey::ModelProbe).await;
    assert!(matches!(
        create_batch(&db, &request).await,
        Err(FoundationError::AdmissionDenied(_))
    ));
    grant(&db, route, TaskKey::ModelProbe, MaterialClass::ModelCatalog).await;
    let queued = create_batch(&db, &request).await.unwrap();
    assert_eq!(create_batch(&db, &request).await.unwrap(), queued);
    let item: Uuid =
        sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
            .bind(queued.batch_ref)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(
        sqlx::query("UPDATE linggan_ai_batch_item SET input_ref='{}' WHERE item_ref=$1")
            .bind(item)
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(sqlx::query("INSERT INTO linggan_ai_batch_item(item_ref,batch_ref,position,item_key,input_ref) VALUES($1,$2,1,'late','{}')")
        .bind(Uuid::new_v4()).bind(queued.batch_ref).execute(db.pool()).await.is_err());
    assert!(sqlx::query("UPDATE linggan_ai_batch_item_control SET state='running',lease_token=$2,lease_expires_at=scope_001_now()+interval '1 minute' WHERE item_ref=$1")
        .bind(item).bind(Uuid::new_v4()).execute(db.pool()).await.is_err(),"claims must advance generation");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_ai_batch")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL proof only"]
async fn ai_foundation_skill_activation_requires_candidate_trial_and_replay_cannot_replace_current_version()
 {
    let db = fixture::proof_database("ai_skill_activation").await;
    let base_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='note_interpretation'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();

    let connection = new_connection(&db).await;
    let version = version(&db, connection, 1, "https://trials.example/api").await;
    // Synthetic storage fixture: the production note handler remains unconnected in PR-1B.
    sqlx::query("UPDATE linggan_ai_task SET lifecycle='active',handler_contract='synthetic.note-storage.v1' WHERE task_key='note_interpretation'").execute(db.pool()).await.unwrap();
    let selected_model = model(&db, version).await;
    let capability = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_model_capability(capability_ref,identity_ref,endpoint_ref,source_kind,source_version,context_tokens,max_output_tokens,max_timeout_seconds,thinking_levels,compat,thinking_level_map,output_mode) VALUES($1,$2,$3,'manual','synthetic-storage-fixture',32768,8192,60,'[\"off\"]','{}','{}','prompt_only')")
        .bind(capability).bind(selected_model).bind(endpoint(&db,version).await).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_model_invocation(invocation_ref,connection_version_ref,model_ref,operation,request_hash,state,reserved_tokens,charged_tokens,input_tokens,output_tokens,finished_at,result) VALUES($1,$2,$3,'probe',repeat('0',64),'succeeded',0,0,0,0,scope_001_now(),'{\"modelCallable\":true,\"syntheticFixture\":true}')")
        .bind(Uuid::new_v4()).bind(version).bind(selected_model).execute(db.pool()).await.unwrap();
    let selection = ModelSelection {
        model_ref: selected_model,
        capability_ref: capability,
        limits: ModelLimits {
            input_tokens: 8192,
            output_tokens: 1024,
            timeout_seconds: 10,
            attempts: 1,
            thinking_level: "off".into(),
        },
    };
    save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_revision: base_revision,
            model: Some(selection.clone()),
            skill_version_ref: None,
            material_columns: [MaterialColumn::Title].into(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    on(&db, TaskKey::NoteInterpretation).await;
    grant(
        &db,
        endpoint(&db, version).await,
        TaskKey::NoteInterpretation,
        MaterialClass::NoteMaterial,
    )
    .await;
    let adapter = PiAdapter::configured();
    let first = import_skill(
        &db,
        &adapter,
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: None,
            package: skills::package(),
        },
    )
    .await
    .unwrap();
    let candidate = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_revision: base_revision + 1,
            model: Some(selection.clone()),
            skill_version_ref: Some(first.version_ref),
            material_columns: [MaterialColumn::Title].into(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    assert!(!candidate.applied);
    assert_eq!(candidate.revision, base_revision + 1);
    let initial = ActivateSkill {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::NoteInterpretation,
        expected_task_revision: base_revision + 1,
        expected_binding_revision: 0,
        candidate_config_ref: candidate.task_config_ref,
        trial_comparison_ref: None,
        first_version_exception: false,
        reason: "synthetic first approval".into(),
    };
    assert!(matches!(
        activate_skill(&db, &initial).await,
        Err(FoundationError::SkillTrialRequired)
    ));
    let first_approval = ActivateSkill {
        command_ref: Uuid::new_v4(),
        first_version_exception: true,
        ..initial
    };
    let approved = activate_skill(&db, &first_approval).await.unwrap();
    assert_eq!(approved.approval_source, "first_version");
    assert_eq!(approved.task_revision, base_revision + 2);
    assert_eq!(approved.binding_revision, 1);
    let mut package = skills::package();
    package.files[0].content.push_str("Changed method\n");
    let second = import_skill(
        &db,
        &adapter,
        &ImportSkill {
            command_ref: Uuid::new_v4(),
            expected_version_ref: Some(first.version_ref),
            package,
        },
    )
    .await
    .unwrap();
    let saved = SaveTaskConfig {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::NoteInterpretation,
        expected_revision: base_revision + 2,
        model: Some(selection.clone()),
        skill_version_ref: Some(second.version_ref),
        material_columns: [MaterialColumn::Title].into(),
        budget: BudgetPolicy::Unlimited,
    };
    let candidate = save_task_config(&db, &saved).await.unwrap();
    assert!(!candidate.applied);
    let activate = ActivateSkill {
        command_ref: Uuid::new_v4(),
        task_key: TaskKey::NoteInterpretation,
        expected_task_revision: base_revision + 2,
        expected_binding_revision: 1,
        candidate_config_ref: candidate.task_config_ref,
        trial_comparison_ref: None,
        first_version_exception: true,
        reason: "synthetic candidate approval".into(),
    };
    assert!(matches!(
        activate_skill(&db, &activate).await,
        Err(FoundationError::SkillTrialRequired)
    ));
    let trial = create_trial(
        &db,
        &CreateTrial {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_task_revision: base_revision + 2,
            baseline_config_ref: Some(approved.task_config_ref),
            candidate_config_ref: candidate.task_config_ref,
            items: vec![BatchInput {
                item_key: "skill-case-1".into(),
                input: InputRef::Note {
                    snapshot_ref: Uuid::new_v4(),
                },
            }],
        },
    )
    .await
    .unwrap();
    let activate = ActivateSkill {
        command_ref: Uuid::new_v4(),
        trial_comparison_ref: Some(trial.comparison_ref),
        first_version_exception: false,
        ..activate
    };
    let pending = linggan_intelligence::ai_foundation::settings_read::read_comparison(
        &db,
        trial.comparison_ref,
        0,
    )
    .await
    .unwrap();
    assert_eq!(pending["qualifiesForActivation"], false);
    assert_eq!(pending["candidate"]["itemState"], "pending");
    assert!(matches!(
        activate_skill(&db, &activate).await,
        Err(FoundationError::SkillTrialRequired)
    ));
    recorded_synthetic_attempt(&db, trial.candidate_batch_ref, version).await;
    assert!(
        matches!(
            activate_skill(&db, &activate).await,
            Err(FoundationError::SkillTrialRequired)
        ),
        "baseline is still pending"
    );
    recorded_synthetic_attempt(&db, trial.baseline_batch_ref.unwrap(), version).await;
    let compared = linggan_intelligence::ai_foundation::settings_read::read_comparison(
        &db,
        trial.comparison_ref,
        0,
    )
    .await
    .unwrap();
    assert_eq!(compared["qualifiesForActivation"], true);
    assert!(compared["candidate"]["output"].is_null());
    assert_eq!(compared["sourceQualification"], "not_connected");
    let overview = linggan_intelligence::ai_foundation::settings_read::read_settings(&db, true)
        .await
        .unwrap();
    assert_eq!(overview["trials"].as_array().unwrap().len(), 1);
    let changed = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            budget: BudgetPolicy::Capped(1000),
            ..saved
        },
    )
    .await
    .unwrap();
    let wrong = ActivateSkill {
        command_ref: Uuid::new_v4(),
        candidate_config_ref: changed.task_config_ref,
        ..activate.clone()
    };
    assert!(
        matches!(
            activate_skill(&db, &wrong).await,
            Err(FoundationError::SkillTrialRequired)
        ),
        "tested configuration must match"
    );
    let enabled = activate_skill(&db, &activate).await.unwrap();
    assert_eq!(enabled.approval_source, "trial");
    let rollback = activate_skill(
        &db,
        &ActivateSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::NoteInterpretation,
            expected_task_revision: base_revision + 3,
            expected_binding_revision: 2,
            candidate_config_ref: approved.task_config_ref,
            trial_comparison_ref: None,
            first_version_exception: false,
            reason: "explicit reuse of approved immutable version".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(rollback.approval_source, "prior_approval");
    assert_eq!(activate_skill(&db, &activate).await.unwrap(), enabled);
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT current_version_ref FROM linggan_skill_binding WHERE task_key='note_interpretation'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        first.version_ref
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_skill_binding_event")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        3
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        3,
        "only the synthetic qualification and two storage-attempt fixtures exist"
    );
    let frozen:Vec<Uuid>=sqlx::query_scalar("SELECT task_config_ref FROM linggan_ai_batch WHERE batch_ref IN ($1,$2) ORDER BY task_config_ref")
        .bind(trial.baseline_batch_ref).bind(trial.candidate_batch_ref).fetch_all(db.pool()).await.unwrap();
    assert!(
        frozen.contains(&approved.task_config_ref) && frozen.contains(&candidate.task_config_ref)
    );
    assert_eq!(
        serde_json::to_value(BudgetPolicy::Capped(i64::MAX as u64)).unwrap(),
        json!({"mode":"capped","tokens":i64::MAX.to_string()})
    );
}
