//! Owned PostgreSQL and loopback-only V4 proof. No shared database or real provider.
#![recursion_limit = "256"]
#[allow(dead_code)]
#[path = "../../evidence/tests/support/domain_fixture.rs"]
mod domain_fixture;
#[allow(dead_code)]
#[path = "support/content_workbench_fixture.rs"]
mod fixture;
#[path = "support/ai_writing_fixture.rs"]
mod setup;
#[allow(dead_code)]
#[path = "support/ai_writing_endpoint.rs"]
mod wire;
use linggan_evidence::target_inspection::{self as inspection, ComparisonMode, Scope};
use linggan_intelligence::ai_foundation::{
    TaskKey,
    control::BudgetPolicy,
    control_store::{Actor, ControlScope},
    prompt::{self, ActivatePrompt, SavePrompt},
    provider_store::{ChangeEgress, MaterialClass, change_egress},
    skill_binding::{ActivateSkill, activate_skill},
    skill_package::{Encoding, SkillPackage, UploadFile},
    skill_store::{ImportSkill, import_skill},
    task_config::{MaterialColumn, ModelSelection, SaveTaskConfig, save_task_config},
    work_comparison as comparison,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

async fn db(name: &str) -> Database {
    let db = fixture::db(name).await;
    for (id, source) in [
        (
            "0142_ocr_adoption",
            include_str!("../../../database/migrations/0142_ocr_adoption.sql"),
        ),
        (
            "0143_ocr_quality_audit",
            include_str!("../../../database/migrations/0143_ocr_quality_audit.sql"),
        ),
        (
            "0144_ocr_manual_recognition",
            include_str!("../../../database/migrations/0144_ocr_manual_recognition.sql"),
        ),
        (
            "0145_ocr_manual_human_adoption",
            include_str!("../../../database/migrations/0145_ocr_manual_human_adoption.sql"),
        ),
        (
            "0146_research_dossier_v2",
            include_str!("../../../database/migrations/0146_research_dossier_v2.sql"),
        ),
        (
            "0147_comment_study_full_context",
            include_str!("../../../database/migrations/0147_comment_study_full_context.sql"),
        ),
        (
            "0148_dossier_consumer_contracts",
            include_str!("../../../database/migrations/0148_dossier_consumer_contracts.sql"),
        ),
        (
            "0149_target_inspector_comparison",
            include_str!("../../../database/migrations/0149_target_inspector_comparison.sql"),
        ),
    ] {
        if let Some(recorded) = sqlx::query_scalar::<_, String>(
            "SELECT migration_sha256 FROM linggan_local_schema_migration WHERE migration_id=$1",
        )
        .bind(id)
        .fetch_optional(db.pool())
        .await
        .unwrap()
        {
            assert_eq!(
                recorded,
                linggan_intelligence::research_text::content_hash(source)
            );
            continue;
        }
        sqlx::raw_sql(source).execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO linggan_local_schema_migration(migration_id,migration_sha256) VALUES($1,$2)")
            .bind(id).bind(linggan_intelligence::research_text::content_hash(source)).execute(db.pool()).await.unwrap();
    }
    db
}
async fn creator(db: &Database) -> Scope {
    let target = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_observation_target(target_ref,platform,target_kind,identity_key,display_name,source) VALUES($1,'xhs','creator','inspector-author','合成作者','manual')")
        .bind(target).execute(db.pool()).await.unwrap();
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES($1,$2,'primary')",
    )
    .bind(fixture::dossier::DOMAIN)
    .bind(target)
    .execute(db.pool())
    .await
    .unwrap();
    Scope {
        target_ref: target,
        domain_ref: Some(fixture::dossier::DOMAIN),
        knowledge_cutoff: None,
    }
}
async fn work(db: &Database, id: &str, likes: i64) -> Uuid {
    let package=fixture::dossier::fixture::submit_package(db,"content_detail",json!({"contentExternalId":id}),
        json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},
            "payload":{"title":format!("合成作品{id}"),"bodyText":"共享步骤与明确条件。","authorId":"inspector-author","likedCount":likes}})).await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4()).bind(work).bind(fixture::dossier::DOMAIN).bind(package).execute(db.pool()).await.unwrap();
    fixture::dossier::fixture::submit_custom_package(db,"xhs",&["media_slots"],json!({"contentExternalId":id}),"media_slots","xhs",
        json!({"target":{"contentExternalId":id,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":0}},"layers":[fixture::dossier::fixture::coverage_layer("media_slots",0)]}),vec![]).await;
    work
}
async fn counts(db: &Database) -> Value {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM collection_acquisition_request),(SELECT count(*) FROM collection_work_order),(SELECT count(*) FROM linggan_model_invocation),(SELECT count(*) FROM linggan_ai_batch))")
        .fetch_one(db.pool()).await.unwrap()
}
#[tokio::test]
#[ignore = "owned disposable PostgreSQL proof only"]
async fn inspector_material_scope_and_known_image_gaps_are_read_only() {
    let db = db("inspector_v4_materials").await;
    let scope = creator(&db).await;
    let a = work(&db, "inspector-a", 0).await;
    let b = work(&db, "inspector-b", 42).await;
    let before = counts(&db).await;
    let gaps = inspection::preview_detail_gaps(&db, &scope).await.unwrap();
    assert!(
        gaps.eligible_refs.is_empty(),
        "complete details cannot enter a new supplement"
    );
    assert!(matches!(
        inspection::request_detail_gaps(&db, &scope, &[a]).await,
        Err(inspection::Error::Invalid(_))
    ));
    let compared = inspection::compare_works(&db, &scope, &[a, b], ComparisonMode::Current)
        .await
        .unwrap();
    assert_eq!(compared.items[0].metrics.likes.value, Some(0));
    assert_eq!(compared.items[1].metrics.likes.value, Some(42));
    let age = inspection::compare_works(&db, &scope, &[a, b], ComparisonMode::Age24h)
        .await
        .unwrap();
    assert!(age.items.iter().all(|i| i.metrics.likes.value.is_none()));
    assert_eq!(counts(&db).await, before);
    assert!(
        inspection::comparison_materials(&db, &scope, &[a, a])
            .await
            .is_err()
    );
    let other = Scope {
        domain_ref: Some(Uuid::new_v4()),
        ..scope.clone()
    };
    assert!(inspection::read_dossier(&db, &other, a).await.is_err());
    let observation = Uuid::new_v4();
    fixture::dossier::fixture::submit_custom_package(&db,"xhs",&["media_slots"],json!({"contentExternalId":"inspector-a"}),"media_slots","xhs",
        json!({"target":{"contentExternalId":"inspector-a","imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":1}},"layers":[fixture::dossier::fixture::coverage_layer("media_slots",1)]}),vec![json!({"kind":"media_slot","slotKey":"xhs:inspector-a:image:1","observationRef":observation,
        "slot":{"role":"image","ordinal":1,"displayOrdinal":1,"displayOrderBasis":"platform_explicit"},
        "observation":{"externalUri":"https://media.example/image.jpg","candidateUris":["https://media.example/image.jpg"],"observedAt":"2026-09-11T02:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":"inspector-a"}})]).await;
    let dossier = inspection::read_dossier(&db, &scope, a).await.unwrap();
    assert!(
        dossier.partial,
        "a known image without a processing job cannot be complete"
    );
    assert!(
        dossier
            .limitations
            .contains(&"some_media_not_readable".to_owned())
    );
    assert!(dossier.fragments.iter().any(|f| f.field == "body"));
    assert_eq!(counts(&db).await, before);
    let materialization = linggan_evidence::admit_media_blob(
        &db,
        observation,
        &"a".repeat(64),
        "image/jpeg",
        12,
        "blobs/synthetic-empty.jpg",
    )
    .await
    .unwrap()
    .materialization_ref;
    let mut tx = db.pool().begin().await.unwrap();
    linggan_evidence::material_reading::publish_in(&mut tx,linggan_evidence::material_reading::NewReading {
        content_public_ref:a,unit_key:"image:xhs:inspector-a:image:1".into(),kind:"image".into(),slot_key:Some("xhs:inspector-a:image:1".into()),page:Some(1),source_version:materialization.to_string(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"cloud_vision".into(),rule_version:"dossier-content.v2:synthetic-empty".into(),blocks:vec![],issues:vec![json!({"key":"recognition_empty","code":"no_text_observed","message":"未检出文字","resolution":"reading_edit"})],source_location:json!({"blobSha256":"a".repeat(64),"materializationRef":materialization}),publication:linggan_evidence::material_reading::Publication::Automatic{expected_current:None}
    }).await.unwrap();
    tx.commit().await.unwrap();
    let raw_empty = inspection::read_dossier(&db, &scope, a).await.unwrap();
    assert!(
        raw_empty.partial,
        "V2 empty recognition is a gap until human confirmation"
    );
    let canonical = linggan_evidence::research_dossier::read(&db, a)
        .await
        .unwrap()
        .unwrap();
    let image = canonical
        .materials
        .iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap();
    let reading: Uuid = image["currentRevisionRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    linggan_evidence::material_reading::review_reading(
        &db,
        image["ref"].as_str().unwrap().parse().unwrap(),
        &linggan_evidence::material_reading::ReviewReadingRequest {
            command_ref: Uuid::new_v4(),
            expected_current_revision_ref: Some(reading),
            unit_ref: image["ref"].as_str().unwrap().into(),
            source_version: image["sourceVersion"].as_str().unwrap().into(),
            action: "edit".into(),
            actor: "synthetic-human-proof".into(),
            reason: "synthetic explicit image has no text".into(),
            edited_blocks: Some(vec![]),
            verified: true,
            resolved_issue_keys: vec!["recognition_empty".into()],
        },
    )
    .await
    .unwrap();
    let complete = inspection::read_dossier(&db, &scope, a).await.unwrap();
    assert!(
        !complete.partial,
        "human-confirmed empty reading is qualified: {:?}",
        complete.limitations
    );
    let frozen = Scope {
        knowledge_cutoff: Some(complete.knowledge_cutoff.clone()),
        ..scope.clone()
    };
    linggan_evidence::record_slot_disposition(
        &db,
        "xhs:inspector-a:image:1",
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic-proof",
        "synthetic withdrawal during a frozen comparison",
    )
    .await
    .unwrap();
    let restricted = inspection::read_dossier(&db, &frozen, a).await.unwrap();
    assert!(restricted.partial);
    assert_eq!(
        restricted
            .fragments
            .iter()
            .find(|f| f.field == "body")
            .unwrap()
            .reading_ref,
        complete
            .fragments
            .iter()
            .find(|f| f.field == "body")
            .unwrap()
            .reading_ref,
        "the native text reading did not change"
    );
    assert_ne!(
        comparison::fingerprint(&[complete]),
        comparison::fingerprint(&[restricted]),
        "source qualification changes must invalidate dispatch/publication even when the text is identical"
    );
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL proof only"]
async fn comparison_authorization_start_replay_and_revoke_use_formal_commands() {
    let db = db("inspector_v4_commands").await;
    let scope = creator(&db).await;
    let works = vec![
        work(&db, "dossier-note", 0).await,
        work(&db, "comparison-b", 8).await,
    ];
    fixture::dossier::current_image_reading(&db).await;
    let image_row=sqlx::query("SELECT u.*,r.source_location,r.derivative_ref FROM linggan_material_unit u JOIN linggan_material_reading r ON r.reading_ref=u.current_reading_ref WHERE u.content_public_ref=$1 AND u.kind='image'").bind(works[0]).fetch_one(db.pool()).await.unwrap();
    let image_reading: Uuid = image_row.get("current_reading_ref");
    let mut tx = db.pool().begin().await.unwrap();
    let rich=linggan_evidence::material_reading::publish_in(&mut tx,linggan_evidence::material_reading::NewReading {
        content_public_ref:works[0],unit_key:image_row.get("unit_key"),kind:"image".into(),slot_key:image_row.get("slot_key"),page:image_row.get("page_ordinal"),source_version:image_row.get("source_version"),source_state:"present".into(),
        derivative_ref:image_row.get("derivative_ref"),producer_ref:None,generated_by:"cloud_vision".into(),rule_version:"dossier-content.v2:synthetic-structure-proof".into(),
        blocks:vec![json!({"kind":"table","speaker":"other","columns":["条件","行动"],"rows":[["5mg TABLE-CANARY","不要加量"]],"bbox":[0.1,0.28,0.75,0.025]}),
            json!({"kind":"dialogue","turns":[{"speaker":"other","text":"没有改善 DIALOGUE-CANARY"},{"speaker":"author","text":"先复核"}]}),
            json!({"kind":"paragraph","speaker":"unconfirmed","text":"局部待核对","issues":[{"key":"synthetic-local","code":"unclear","message":"合成局部问题","resolution":"reading_edit"}]})],
        issues:vec![],source_location:image_row.get("source_location"),publication:linggan_evidence::material_reading::Publication::Automatic{expected_current:Some(image_reading)},
    }).await.unwrap();
    tx.commit().await.unwrap();
    assert!(rich.current);
    let endpoint = wire::Endpoint::start(false).await;
    let adapter = linggan_intelligence::pi_adapter::PiAdapter::configured_with_test_command(
        std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap().into(),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/pi-adapter/src/adapter.mjs"),
    );
    let mut owner = linggan_intelligence::ai_foundation::executor::ExecutorSession::acquire(
        &db,
        "synthetic-inspector-proof",
        4,
    )
    .await
    .unwrap();
    let configured = setup::prepare_with_actual_probe(&db, &endpoint.url, &owner, &adapter).await;
    owner.request_drain().await.unwrap();
    owner.complete_drain().await.unwrap();
    drop(owner);
    let comparison_revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM linggan_ai_task_setting WHERE task_key='work_comparison'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    let config = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_revision: comparison_revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: configured.limits.clone(),
            }),
            skill_version_ref: None,
            material_columns: [
                MaterialColumn::Title,
                MaterialColumn::Body,
                MaterialColumn::Tags,
                MaterialColumn::Ocr,
                MaterialColumn::Transcript,
                MaterialColumn::FrameText,
            ]
            .into(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let skill = import_skill(&db, &adapter, &ImportSkill { command_ref: Uuid::new_v4(), expected_version_ref: None,
        package: SkillPackage { directory: "comparison-method".into(), files: vec![UploadFile { path: "SKILL.md".into(), encoding: Encoding::Utf8,
            content: "---\nname: comparison-method\ndescription: Synthetic comparison method\n---\nSYNTHETIC COMPARISON SKILL\n".into() }] } }).await.unwrap();
    let skill_candidate = save_task_config(
        &db,
        &SaveTaskConfig {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_revision: config.revision,
            model: Some(ModelSelection {
                model_ref: configured.model,
                capability_ref: configured.capability,
                limits: configured.limits.clone(),
            }),
            skill_version_ref: Some(skill.version_ref),
            material_columns: [
                MaterialColumn::Title,
                MaterialColumn::Body,
                MaterialColumn::Tags,
                MaterialColumn::Ocr,
                MaterialColumn::Transcript,
                MaterialColumn::FrameText,
            ]
            .into(),
            budget: BudgetPolicy::Unlimited,
        },
    )
    .await
    .unwrap();
    let enabled_skill = activate_skill(
        &db,
        &ActivateSkill {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_task_revision: config.revision,
            expected_binding_revision: 0,
            candidate_config_ref: skill_candidate.task_config_ref,
            trial_comparison_ref: None,
            reason: "synthetic comparison skill".into(),
        },
    )
    .await
    .unwrap();
    let initial = prompt::read(&db, TaskKey::WorkComparison).await.unwrap();
    let original: Uuid = initial["activeVersionRef"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let edited = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_task_revision: enabled_skill.task_revision,
            system_text: "SYNTHETIC COMPARISON SYSTEM".into(),
            instruction_text: "SYNTHETIC COMPARISON INSTRUCTION".into(),
        },
    )
    .await
    .unwrap();
    let enabled_prompt = prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_task_revision: enabled_skill.task_revision,
            version_ref: edited.version_ref,
        },
    )
    .await
    .unwrap();
    for control in [
        ControlScope::Global,
        ControlScope::Executor,
        ControlScope::Task(TaskKey::WorkComparison),
    ] {
        setup::enable(&db, control).await;
    }
    let denied = comparison::preview(&db, &scope, &works).await.unwrap();
    assert!(!denied.can_start);
    assert!(denied.reasons.contains(&"egress_not_approved".into()));
    let grant = ChangeEgress {
        command_ref: Uuid::new_v4(),
        endpoint_ref: configured.endpoint,
        task_key: TaskKey::WorkComparison,
        material_class: MaterialClass::NoteMaterial,
        expected_event_ref: None,
        allowed: true,
        reason: "synthetic explicit comparison grant".into(),
    };
    let allowed = change_egress(&db, &grant, Actor::Human).await.unwrap();
    assert_eq!(
        change_egress(&db, &grant, Actor::Human).await.unwrap(),
        allowed
    );
    let preview = comparison::preview(&db, &scope, &works).await.unwrap();
    assert!(preview.can_start, "{:?}", preview.reasons);
    let start = comparison::StartComparison {
        command_ref: Uuid::new_v4(),
        target_ref: scope.target_ref,
        domain_ref: scope.domain_ref,
        work_refs: works.clone(),
        knowledge_cutoff: preview.knowledge_cutoff,
        expected_material_revisions: preview.material_revisions,
        task_config_ref: enabled_prompt.task_config_ref,
        expected_task_revision: enabled_prompt.task_revision,
    };
    let receipt = comparison::start(&db, &start).await.unwrap();
    assert_eq!(
        comparison::start(&db, &start).await.unwrap().request_ref,
        receipt.request_ref
    );
    let second = comparison::start(
        &db,
        &comparison::StartComparison {
            command_ref: Uuid::new_v4(),
            ..start.clone()
        },
    )
    .await
    .unwrap();
    assert!(second.reused);
    assert_eq!(second.batch_ref, receipt.batch_ref);
    prompt::activate(
        &db,
        &ActivatePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::WorkComparison,
            expected_task_revision: enabled_prompt.task_revision,
            version_ref: original,
        },
    )
    .await
    .unwrap();
    let read = comparison::read(&db, &scope, receipt.request_ref)
        .await
        .unwrap();
    assert_eq!(read.state, "pending");
    assert!(!read.outdated);
    assert!(read.result.is_none());
    assert_eq!(read.materials.len(), 2);
    change_egress(
        &db,
        &ChangeEgress {
            command_ref: Uuid::new_v4(),
            allowed: false,
            expected_event_ref: Some(allowed.event_ref),
            ..grant
        },
        Actor::Human,
    )
    .await
    .unwrap();
    assert!(
        !comparison::preview(&db, &scope, &works)
            .await
            .unwrap()
            .can_start
    );
    let n:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM linggan_work_comparison_request),(SELECT count(*) FROM linggan_model_invocation)").fetch_one(db.pool()).await.unwrap();
    assert_eq!(n, (1, 1), "only the actual loopback probe has run");
    let drain = linggan_intelligence::model_worker_drain::ModelWorkerDrain::new();
    let running = tokio::spawn(linggan_intelligence::ai_foundation::executor::service::run(
        db.clone(),
        "synthetic-inspector-service",
        4,
        std::sync::Arc::new(linggan_intelligence::model_secrets::SyntheticModelSecrets),
        adapter,
        drain.clone(),
    ));
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        loop {
            let read = comparison::read(&db, &scope, receipt.request_ref)
                .await
                .unwrap();
            if read.wait_reason.as_deref() == Some("egress_not_approved") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("revoked material waits without a model call");
    assert_eq!(endpoint.requests.lock().unwrap().len(), 1);
    let event: Uuid = sqlx::query_scalar(
        "SELECT event_ref FROM linggan_ai_current_egress WHERE task_key='work_comparison'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    change_egress(
        &db,
        &ChangeEgress {
            command_ref: Uuid::new_v4(),
            endpoint_ref: configured.endpoint,
            task_key: TaskKey::WorkComparison,
            material_class: MaterialClass::NoteMaterial,
            expected_event_ref: Some(event),
            allowed: true,
            reason: "synthetic comparison regrant".into(),
        },
        Actor::Human,
    )
    .await
    .unwrap();
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let read = comparison::read(&db, &scope, receipt.request_ref)
                .await
                .unwrap();
            if read.state == "succeeded" {
                break read;
            }
            assert_ne!(read.state, "failed", "{:?}", read.failure_code);
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("actual shared executor settles content comparison");
    assert!(
        !result.outdated,
        "time alone cannot invalidate unchanged material"
    );
    let quoted = &result.materials[0].fragments;
    assert_eq!(
        quoted.len(),
        1,
        "only the published claim's exact block is transported"
    );
    assert_eq!(quoted[0].reading_ref, Some(rich.reading_ref));
    assert_eq!(quoted[0].unit_ref, Some(rich.unit_ref));
    assert_eq!(quoted[0].structure["kind"], "table");
    assert_eq!(quoted[0].speaker, "other");
    assert_eq!(quoted[0].structure["bbox"], json!([0.1, 0.28, 0.75, 0.025]));
    assert_eq!(quoted[0].source_location["page"], 1);
    let public = serde_json::to_value(&result.materials).unwrap();
    assert!(public[0]["fragments"][0].get("text").is_none());
    let requests = endpoint.requests.lock().unwrap();
    assert!(wire::text(requests.last().unwrap(), "system").contains("SYNTHETIC COMPARISON SYSTEM"));
    assert!(wire::text(requests.last().unwrap(), "system").contains("SYNTHETIC COMPARISON SKILL"));
    let prompt = wire::text(requests.last().unwrap(), "user");
    assert!(prompt.contains("SYNTHETIC COMPARISON INSTRUCTION"));
    assert_eq!(prompt.matches("TABLE-CANARY").count(), 1);
    assert_eq!(prompt.matches("DIALOGUE-CANARY").count(), 1);
    let packet: Value = serde_json::from_str(&prompt).unwrap();
    let fragments = packet["documents"][0]["fragments"].as_array().unwrap();
    let table = fragments
        .iter()
        .find(|f| f["content"]["kind"] == "table")
        .unwrap();
    assert_eq!(table["speaker"], "other");
    assert_eq!(table["readingRef"], rich.reading_ref.to_string());
    assert_eq!(table["unitRef"], rich.unit_ref.to_string());
    assert!(
        fragments.iter().any(|f| f["content"]["kind"] == "dialogue"
            && f["content"]["turns"][0]["speaker"] == "other")
    );
    assert!(fragments.iter().any(|f| {
        f["issues"]
            .as_array()
            .is_some_and(|issues| !issues.is_empty())
    }));
    drop(requests);
    let analysis = result.result.unwrap();
    assert_eq!(analysis.claims.len(), 1);
    assert_eq!(analysis.claims[0].target_citations[0].work_ref, works[0]);
    assert_eq!(analysis.claims[0].reference_citations[0].work_ref, works[1]);
    drain.request();
    tokio::time::timeout(std::time::Duration::from_secs(10), running)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        endpoint.requests.lock().unwrap().len(),
        2,
        "probe and one comparison, no retry"
    );
    endpoint.finish().await;
}

#[tokio::test]
#[ignore = "owned disposable PostgreSQL proof only"]
async fn keyword_round_membership_and_exact_observation_changes_preserve_cutoff() {
    let db = db("inspector_v4_rounds").await;
    let query = "合成轮次关键词";
    let cards = |ids: &[(&str, &str)]| {
        ids.iter()
            .map(|(id, likes)| {
                domain_fixture::discovery_card(
                    id,
                    id,
                    likes,
                    &format!("https://www.xiaohongshu.com/explore/{id}?xsec_token=SYNTHETIC"),
                )
            })
            .collect()
    };
    let first = domain_fixture::submit_peer_domain_package(
        &db,
        query,
        "deep_archive",
        json!({"query":query}),
        "discovery_search",
        domain_fixture::search_coverage(query, 2),
        json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        cards(&[("round-common", "10"), ("round-left", "0")]),
    )
    .await;
    let target: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
    )
    .bind(query)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let scope = Scope {
        target_ref: target,
        domain_ref: Some(domain_fixture::PEER_DOMAIN.parse().unwrap()),
        knowledge_cutoff: None,
    };
    domain_fixture::submit_package_for_target_at(
        &db,
        target,
        query,
        "deep_archive",
        json!({"query":query}),
        200,
        "discovery_search",
        domain_fixture::search_coverage(query, 2),
        json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        cards(&[("round-common", "15"), ("round-right", "3")]),
        "2026-09-12T02:00:00Z",
    )
    .await;
    let before = counts(&db).await;
    let page = inspection::read_rounds(&db, &scope, None, 0).await.unwrap();
    assert_eq!(page.rounds.len(), 2);
    let left = page.rounds.iter().find(|r| r.package_ref == first).unwrap();
    let right = page.rounds.iter().find(|r| r.package_ref != first).unwrap();
    let single = inspection::read_round_result(&db, &scope, right.round_ref)
        .await
        .unwrap();
    assert_eq!(single.works.len(), 2);
    let compared = inspection::compare_rounds(&db, &scope, left.round_ref, right.round_ref)
        .await
        .unwrap();
    assert_eq!(
        (
            compared.common_count,
            compared.entered_count,
            compared.absent_count
        ),
        (Some(1), Some(1), Some(1))
    );
    assert!(compared.reasons.contains(&"historical_rule_unknown"));
    let common = compared
        .rows
        .iter()
        .find(|r| r.membership == "both")
        .unwrap();
    assert_eq!(common.metrics.likes.delta, Some(5));
    assert_eq!(common.metrics.likes.elapsed_seconds, Some(86400.0));
    assert_eq!(common.discovery_label, "repeat");
    let trace = inspection::read_trace(&db, &scope, common.work_ref, 0)
        .await
        .unwrap();
    assert_eq!(trace.observations.len(), 2);
    let a: Uuid = trace.observations[1].observation_ref.parse().unwrap();
    let b: Uuid = trace.observations[0].observation_ref.parse().unwrap();
    assert_eq!(
        inspection::compare_observations(&db, &scope, common.work_ref, a, b)
            .await
            .unwrap()
            .metrics
            .likes
            .delta,
        Some(5)
    );
    let reverse = inspection::compare_observations(&db, &scope, common.work_ref, b, a).await;
    assert!(matches!(reverse, Err(inspection::Error::Invalid(_))));
    let frozen = Scope {
        knowledge_cutoff: Some("2026-09-11T23:59:59Z".into()),
        ..scope.clone()
    };
    assert!(
        matches!(
            inspection::read_rounds(&db, &frozen, None, 0)
                .await
                .unwrap()
                .rounds
                .as_slice(),
            []
        ),
        "material accepted after the requested cutoff must not appear in history"
    );
    assert_eq!(
        counts(&db).await,
        before,
        "inspection reads cannot schedule, collect or invoke"
    );
}

/// 作者巡检里的「相似旧作」读的是这位作者在领域里的研究材料：只在作者主页上出现过的作品不算。
#[tokio::test]
#[ignore = "owned disposable PostgreSQL proof only"]
async fn creator_context_works_are_not_similar_old_works_of_the_observed_creator() {
    let db = db("inspector_creator_context").await;
    let scope = creator(&db).await;
    let current = work(&db, "similar-current", 10).await;
    let research = work(&db, "similar-research", 8).await;
    let context = work(&db, "similar-context", 7).await;
    let related = |label: &'static str| {
        let (db, scope) = (&db, &scope);
        async move {
            let read = inspection::related_works(db, scope, current).await.unwrap();
            let mut refs: Vec<Uuid> = read.candidates.iter().map(|work| work.work_ref).collect();
            refs.sort();
            (label, refs)
        }
    };
    let mut both = vec![research, context];
    both.sort();
    assert_eq!(
        related("control").await,
        ("control", both),
        "对照：两篇都还是研究材料时，都是相似旧作的候选"
    );
    fixture::dossier::fixture::make_creator_context_only(
        &db,
        fixture::dossier::DOMAIN,
        "similar-context",
    )
    .await;
    assert_eq!(related("after").await, ("after", vec![research]));
}
