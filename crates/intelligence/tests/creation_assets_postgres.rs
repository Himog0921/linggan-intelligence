//! CREATION-ASSETS-001 S1a isolated PostgreSQL proof. Synthetic data, no external calls.
#![recursion_limit = "256"]
#[path = "support/content_workbench_fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod full_schema;
use fixture::*;
use linggan_intelligence::content_workbench::{
    BridgeChoice, BridgeGroup, BridgeUse, CreateRequest, ExperienceSlot, FreezeRequest, PageKind,
    Paragraph, PlaceRequest, ResetWithdrawnRequest, RestoreRequest, SaveRequest, UseTarget,
    WorkDocument, WorkbenchError, create_work, freeze_export, list_idea_boxes, list_works, place,
    read_request, read_work, reset_withdrawn, restore, revisions, save,
};
use linggan_intelligence::creation_assets::*;
use linggan_storage_postgres::Database;
use uuid::Uuid;

const FILL: &str = "\u{3164}";

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn detached_draft_batch_waits_for_prior_experience_withdrawal() {
    let db = db("assets_batch_withdrawal_race").await;
    let (account, _, _) = accounts(&db).await;
    let person = put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap()
    .identity
    .person_ref
    .unwrap();
    let experience = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "合成".into(),
            body: "合成经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    confirm_experience(
        &db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = draft(Some(account), &["合成经历。"]);
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = create(&db, doc).await;
    let mut independent = work.revision.document.clone();
    independent.experience_slots.clear();
    independent.paragraphs = vec![Paragraph {
        id: Uuid::new_v4(),
        text: "独立正文".into(),
    }];
    let saved = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document: independent,
        },
    )
    .await
    .unwrap();
    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let withdraw_db = db.clone();
    let withdrawal =
        tokio::spawn(
            async move { withdraw_experience(&withdraw_db, experience, Uuid::new_v4()).await },
        );
    wait_for_advisory_waiters(&db, 1).await;
    let check_db = db.clone();
    let checking = tokio::spawn(async move {
        check_saved_drafts(
            &check_db,
            &SavedDraftCheckRequest {
                domain_ref: fixture::dossier::DOMAIN,
                items: vec![SavedDraftCheckItem {
                    work_id: work.work_id,
                    revision_id: saved.revision.id,
                }],
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    withdrawal.await.unwrap().unwrap();
    assert!(
        checking.await.unwrap().unwrap().is_empty(),
        "detached history must observe the prior withdrawal"
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn publication_link_rechecks_after_concurrent_operations_update() {
    let db = db("assets_publication_race").await;
    let _ = fixture::ops_seed(&db).await;
    let (publication, account): (Uuid, Uuid) = sqlx::query_as("SELECT p.publication_ref,p.account_ref FROM linggan_ops_publication p JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE a.domain_ref=$1 AND a.ledger_state='in_ledger' AND p.platform='xhs' AND p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL LIMIT 1")
        .bind(fixture::dossier::DOMAIN).fetch_one(db.pool()).await.unwrap();
    let different: Uuid = sqlx::query_scalar("SELECT account_ref FROM linggan_ops_account WHERE domain_ref=$1 AND ledger_state='in_ledger' AND account_ref<>$2 AND platform='xhs' LIMIT 1")
        .bind(fixture::dossier::DOMAIN).bind(account).fetch_one(db.pool()).await.unwrap();
    let work = create(&db, draft(Some(account), &["合成正文"])).await;
    let mut update = db.pool().begin().await.unwrap();
    sqlx::query("UPDATE linggan_ops_account SET account_name=account_name WHERE account_ref=$1")
        .bind(account)
        .execute(&mut *update)
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_ops_publication SET account_ref=$2 WHERE publication_ref=$1")
        .bind(publication)
        .bind(different)
        .execute(&mut *update)
        .await
        .unwrap();
    let task_db = db.clone();
    let mut task = tokio::spawn(async move {
        link_publication(
            &task_db,
            work.work_id,
            &PublicationLinkCommand {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
                publication_ref: publication,
            },
        )
        .await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(200), &mut task)
            .await
            .is_err()
    );
    update.commit().await.unwrap();
    assert!(matches!(
        tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap(),
        Err(AssetsError::SourceUnavailable)
    ));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn publication_link_is_explicit_idempotent_and_tracks_record_qualification() {
    let db = db("assets_publication_link").await;
    let _ = fixture::ops_seed(&db).await;
    let (publication, account): (Uuid, Uuid) = sqlx::query_as("SELECT p.publication_ref,p.account_ref FROM linggan_ops_publication p JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE a.domain_ref=$1 AND a.ledger_state='in_ledger' AND p.platform='xhs' AND p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL LIMIT 1")
        .bind(fixture::dossier::DOMAIN).fetch_one(db.pool()).await.unwrap();
    let work = create(&db, draft(Some(account), &["合成正文"])).await;
    assert!(
        publication_candidates(&db, work.work_id)
            .await
            .unwrap()
            .iter()
            .any(|item| item.publication_ref == publication)
    );
    let command = PublicationLinkCommand {
        request_id: Uuid::new_v4(),
        expected_revision: work.revision.id,
        publication_ref: publication,
    };
    let receipt = link_publication(&db, work.work_id, &command).await.unwrap();
    assert!(!receipt.replayed);
    assert!(
        link_publication(&db, work.work_id, &command)
            .await
            .unwrap()
            .replayed
    );
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == work.work_id)
        .unwrap();
    assert_eq!(summary.published_revision_id, Some(work.revision.id));
    assert!(!summary.publication_unavailable);
    let other = create(&db, draft(Some(account), &["另一篇"])).await;
    // An Operations import can correct the account independently of its publication.
    sqlx::query("UPDATE linggan_ops_account SET platform='douyin',platform_user_id=NULL,profile_link_state='pending_match' WHERE account_ref=$1")
        .bind(account).execute(db.pool()).await.unwrap();
    assert!(
        publication_candidates(&db, other.work_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        link_publication(&db, work.work_id, &command).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(!publication_links(&db, work.work_id).await.unwrap()[0].available);
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == work.work_id)
        .unwrap();
    assert!(summary.publication_unavailable);
    assert!(summary.published_revision_id.is_none());
    sqlx::query("UPDATE linggan_ops_account SET platform='xhs' WHERE account_ref=$1")
        .bind(account)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        link_publication(
            &db,
            other.work_id,
            &PublicationLinkCommand {
                request_id: Uuid::new_v4(),
                expected_revision: other.revision.id,
                publication_ref: publication
            }
        )
        .await,
        Err(AssetsError::RequestConflict)
    ));
    assert!(matches!(
        link_publication(
            &db,
            other.work_id,
            &PublicationLinkCommand {
                request_id: Uuid::new_v4(),
                expected_revision: Uuid::new_v4(),
                publication_ref: publication
            }
        )
        .await,
        Err(AssetsError::Stale)
    ));
    let different_account: Uuid = sqlx::query_scalar("SELECT account_ref FROM linggan_ops_account WHERE domain_ref=$1 AND ledger_state='in_ledger' AND account_ref<>$2 AND platform='xhs' LIMIT 1")
        .bind(fixture::dossier::DOMAIN).bind(account).fetch_one(db.pool()).await.unwrap();
    sqlx::query("UPDATE linggan_ops_publication SET account_ref=$2 WHERE publication_ref=$1")
        .bind(publication)
        .bind(different_account)
        .execute(db.pool())
        .await
        .unwrap();
    assert!(matches!(
        link_publication(&db, work.work_id, &command).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(
        list_works(&db, Some(fixture::dossier::DOMAIN))
            .await
            .unwrap()
            .into_iter()
            .find(|item| item.work_id == work.work_id)
            .unwrap()
            .publication_unavailable
    );
    assert!(!publication_links(&db, work.work_id).await.unwrap()[0].available);
    sqlx::query("UPDATE linggan_ops_publication SET account_ref=$2 WHERE publication_ref=$1")
        .bind(publication)
        .bind(account)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE linggan_ops_publication SET missing_since_batch_ref=last_changed_batch_ref WHERE publication_ref=$1").bind(publication).execute(db.pool()).await.unwrap();
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == work.work_id)
        .unwrap();
    assert!(summary.publication_unavailable);
    assert!(summary.published_revision_id.is_none());
    let unlink_id = Uuid::new_v4();
    assert!(
        !unlink_publication(&db, work.work_id, receipt.link_ref, unlink_id)
            .await
            .unwrap()
    );
    assert!(
        unlink_publication(&db, work.work_id, receipt.link_ref, unlink_id)
            .await
            .unwrap()
    );
    assert!(matches!(
        link_publication(&db, work.work_id, &command).await,
        Err(AssetsError::Stale)
    ));
    assert!(
        publication_links(&db, work.work_id)
            .await
            .unwrap()
            .is_empty()
    );
    sqlx::query(
        "UPDATE linggan_ops_publication SET missing_since_batch_ref=NULL WHERE publication_ref=$1",
    )
    .bind(publication)
    .execute(db.pool())
    .await
    .unwrap();
    assert!(
        link_publication(
            &db,
            other.work_id,
            &PublicationLinkCommand {
                request_id: Uuid::new_v4(),
                expected_revision: other.revision.id,
                publication_ref: publication
            }
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn title_seed_preserves_an_existing_same_name_skill() {
    let source = include_str!("../../../database/migrations/0185_creation_title_task.sql");
    let ledger = "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES ('0185_creation_title_task','058344a4fa949e8704f08e071bf725c9ae257e951072e6b56d89713dd9d52ede');\n";
    let links = include_str!("../../../database/migrations/0186_creation_publication_links.sql");
    let link_ledger = "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES ('0186_creation_publication_links','bdfbc5f78c25a6ac37718a08315978a3906e70a323e21a9b6b128531c62e775a');\n";
    let schema = full_schema::FULL_MIGRATIONS
        .replace(source, "")
        .replace(ledger, "")
        .replace(links, "")
        .replace(link_ledger, "");
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "title_existing_skill",
        &schema,
    )
    .await
    .unwrap();
    let existing = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_skill(skill_ref,name) VALUES($1,'creation-title-ideas')")
        .bind(existing)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::raw_sql(source).execute(db.pool()).await.unwrap();
    let (name, content, package_hash): (String, String, String) = sqlx::query_as("SELECT v.directory_name,f.text_content,v.package_sha256 FROM linggan_skill_binding b JOIN linggan_skill_version v ON v.version_ref=b.current_version_ref JOIN linggan_skill_file f ON f.version_ref=v.version_ref WHERE b.task_key='creation_title' AND f.path='SKILL.md'")
        .fetch_one(db.pool()).await.unwrap();
    assert_ne!(name, "creation-title-ideas");
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT skill_ref FROM linggan_skill WHERE name='creation-title-ideas'"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        existing
    );
    let prepared = linggan_intelligence::ai_foundation::skill_package::SkillPackage {
        directory: name,
        files: vec![
            linggan_intelligence::ai_foundation::skill_package::UploadFile {
                path: "SKILL.md".into(),
                encoding: linggan_intelligence::ai_foundation::skill_package::Encoding::Utf8,
                content,
            },
        ],
    }
    .prepare()
    .unwrap();
    assert_eq!(package_hash, prepared.package_sha256);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn title_task_has_editable_prompt_and_default_skill_with_dispatch_still_disabled() {
    use linggan_intelligence::ai_foundation::{
        TaskKey,
        prompt::{self, SavePrompt},
        settings_read::read_skill_version,
    };
    let db = db("assets_title_seed").await;
    let row: (String, bool, Uuid) = sqlx::query_as("SELECT t.lifecycle,c.enabled,b.current_version_ref FROM linggan_ai_task t JOIN linggan_ai_control c USING(task_key) JOIN linggan_skill_binding b USING(task_key) WHERE t.task_key='creation_title'")
        .fetch_one(db.pool()).await.unwrap();
    assert_eq!(row.0, "active");
    assert!(!row.1);
    let skill = read_skill_version(&db, row.2).await.unwrap();
    assert_eq!(skill["name"], "creation-title-ideas");
    let before = prompt::read(&db, TaskKey::CreationTitle).await.unwrap();
    assert_eq!(before["versions"].as_array().unwrap().len(), 1);
    let saved = prompt::save(
        &db,
        &SavePrompt {
            command_ref: Uuid::new_v4(),
            task_key: TaskKey::CreationTitle,
            expected_task_revision: 0,
            system_text: "合成系统提示词".into(),
            instruction_text: "合成任务指令".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(saved.version_no, 2);
    assert_eq!(
        prompt::read(&db, TaskKey::CreationTitle).await.unwrap()["versions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn saved_draft_batch_checks_actual_revision_and_keeps_unknown_unknown() {
    let db = db("assets_saved_draft_batch").await;
    let created = create_work(
        &db,
        &CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: fixture::dossier::DOMAIN,
            document: draft(None, &["合成正文"]),
        },
    )
    .await
    .unwrap();
    let request = SavedDraftCheckRequest {
        domain_ref: fixture::dossier::DOMAIN,
        items: vec![SavedDraftCheckItem {
            work_id: created.work_id,
            revision_id: created.revision.id,
        }],
    };
    let checks = check_saved_drafts(&db, &request).await.unwrap();
    let report = check_draft_for_work(&db, &created.revision.document, Some(created.work_id))
        .await
        .unwrap();
    assert_eq!(checks.len(), 1);
    assert_eq!(
        (checks[0].must, checks[0].warn, checks[0].not_run),
        (
            report.count(Level::Must),
            report.count(Level::Warn),
            report.not_run.len()
        )
    );
    let stale = SavedDraftCheckRequest {
        domain_ref: request.domain_ref,
        items: vec![SavedDraftCheckItem {
            work_id: created.work_id,
            revision_id: Uuid::new_v4(),
        }],
    };
    assert!(check_saved_drafts(&db, &stale).await.unwrap().is_empty());
    let wrong_domain = SavedDraftCheckRequest {
        domain_ref: Uuid::new_v4(),
        items: request.items.clone(),
    };
    assert!(
        check_saved_drafts(&db, &wrong_domain)
            .await
            .unwrap()
            .is_empty()
    );
    let duplicate = SavedDraftCheckRequest {
        domain_ref: request.domain_ref,
        items: vec![request.items[0].clone(), request.items[0].clone()],
    };
    assert!(matches!(
        check_saved_drafts(&db, &duplicate).await,
        Err(AssetsError::Invalid("items"))
    ));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn idea_box_links_drafts_without_conflating_note_title_or_research_topic() {
    let db = db("assets_idea_box").await;
    let (a, b, c) = accounts(&db).await;
    let fourth = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ops_account(account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,status,is_retained,review_status,profile_link_state,source_created_at,source_updated_at,first_batch_ref,last_changed_batch_ref) SELECT $1,source_system,'synthetic-fourth',domain_ref,ledger_state,platform,'合成第四号',status,is_retained,review_status,'missing',source_created_at,source_updated_at,first_batch_ref,last_changed_batch_ref FROM linggan_ops_account WHERE account_ref=$2")
        .bind(fourth).bind(a).execute(db.pool()).await.unwrap();
    let idea = Uuid::new_v4();
    let original = create_work(
        &db,
        &CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: fixture::dossier::DOMAIN,
            document: draft(Some(a), &["合成正文"]),
        },
    )
    .await
    .unwrap();
    let mut anchor_doc = original.revision.document.clone();
    anchor_doc.idea_ref = Some(idea);
    anchor_doc.idea_name = Some("合成选题".into());
    anchor_doc.angle = "合成选题意图".into();
    let anchor = save(
        &db,
        original.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: original.revision.id,
            document: anchor_doc,
        },
    )
    .await
    .unwrap();
    for account in [b, c] {
        let mut doc = draft(Some(account), &["第二份合成正文"]);
        doc.idea_ref = Some(idea);
        let work = create_work(
            &db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: fixture::dossier::DOMAIN,
                document: doc,
            },
        )
        .await
        .unwrap();
        assert_eq!(work.revision.document.idea_ref, Some(idea));
        assert_eq!(work.revision.document.topic_ref, None);
    }
    let mut too_many = draft(Some(fourth), &["第四份合成正文"]);
    too_many.idea_ref = Some(idea);
    assert!(matches!(
        create_work(
            &db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: fixture::dossier::DOMAIN,
                document: too_many
            }
        )
        .await,
        Err(WorkbenchError::Invalid("idea_account_limit"))
    ));
    let mut changed = anchor.revision.document.clone();
    changed.idea_name = Some("更正的选题".into());
    changed.title = "独立的笔记标题".into();
    let saved = save(
        &db,
        anchor.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: anchor.revision.id,
            document: changed,
        },
    )
    .await
    .unwrap();
    let boxes = list_idea_boxes(&db, fixture::dossier::DOMAIN)
        .await
        .unwrap();
    assert_eq!(boxes.len(), 1);
    assert_eq!(
        (&boxes[0].name, &boxes[0].intent, boxes[0].anchor_work_id),
        (
            &"更正的选题".to_string(),
            &"合成选题意图".to_string(),
            anchor.work_id
        )
    );
    assert_eq!(saved.revision.document.title, "独立的笔记标题");
    let mut invalid = saved.revision.document.clone();
    invalid.idea_ref = None;
    invalid.idea_name = None;
    assert!(matches!(
        save(
            &db,
            anchor.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                document: invalid
            }
        )
        .await,
        Err(WorkbenchError::Invalid("idea_anchor"))
    ));
    let restored = restore(
        &db,
        anchor.work_id,
        &RestoreRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            restore_revision: original.revision.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(restored.revision.document.idea_ref, Some(idea));
    assert_eq!(
        restored.revision.document.idea_name.as_deref(),
        Some("更正的选题")
    );
    assert_eq!(restored.revision.document.angle, "合成选题意图");
    assert_eq!(restored.revision.document.title, "合成标题");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn idea_save_waits_for_experience_gate_before_taking_idea_lock() {
    let db = db("assets_idea_lock_order").await;
    let idea = Uuid::new_v4();
    let mut held_gate = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *held_gate)
        .await
        .unwrap();
    let worker_db = db.clone();
    let saving = tokio::spawn(async move {
        let mut doc = draft(None, &["合成正文"]);
        doc.idea_ref = Some(idea);
        doc.idea_name = Some("合成选题".into());
        create_work(
            &worker_db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: fixture::dossier::DOMAIN,
                document: doc,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 1).await;
    let mut check = db.pool().begin().await.unwrap();
    let available: bool =
        sqlx::query_scalar("SELECT pg_try_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("creation_idea:{idea}"))
            .fetch_one(&mut *check)
            .await
            .unwrap();
    check.rollback().await.unwrap();
    held_gate.commit().await.unwrap();
    assert!(
        available,
        "a writer waiting for the experience gate must not hold the idea lock"
    );
    saving.await.unwrap().unwrap();
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn idea_list_does_not_drop_older_boxes_after_two_hundred() {
    let db = db("assets_idea_list").await;
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("CREATE TEMP TABLE synthetic_ideas(work_ref uuid,revision_ref uuid,idea_ref uuid) ON COMMIT DROP")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO synthetic_ideas SELECT gen_random_uuid(),gen_random_uuid(),gen_random_uuid() FROM generate_series(1,201)")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_creation_work(work_ref,domain_ref,saved_by) SELECT work_ref,$1,'local_machine' FROM synthetic_ideas")
        .bind(fixture::dossier::DOMAIN).execute(&mut *tx).await.unwrap();
    let blank = serde_json::to_value(WorkDocument::default()).unwrap();
    sqlx::query("INSERT INTO linggan_creation_revision(revision_ref,work_ref,revision_number,document,document_sha256) SELECT revision_ref,work_ref,1,$1::jsonb || jsonb_build_object('ideaRef',idea_ref::text,'ideaName','合成选题'),repeat('0',64) FROM synthetic_ideas")
        .bind(blank).execute(&mut *tx).await.unwrap();
    sqlx::query("UPDATE linggan_creation_work w SET current_revision_ref=s.revision_ref FROM synthetic_ideas s WHERE w.work_ref=s.work_ref")
        .execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO linggan_creation_idea_box(idea_ref,domain_ref,anchor_work_ref) SELECT idea_ref,$1,work_ref FROM synthetic_ideas")
        .bind(fixture::dossier::DOMAIN).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        list_idea_boxes(&db, fixture::dossier::DOMAIN)
            .await
            .unwrap()
            .len(),
        201
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn persona_facts_keep_conflicts_subject_ownership_and_erasable_history() {
    let db = db("assets_persona_facts").await;
    let (account, _, _) = accounts(&db).await;
    put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("合成角色")),
    )
    .await
    .unwrap();
    let person = read_identity(&db, account)
        .await
        .unwrap()
        .unwrap()
        .person_ref
        .unwrap();
    let create_fact = PersonaFactPut {
        request_id: Uuid::new_v4(),
        account_ref: account,
        expected_person_ref: person,
        key: "年级".into(),
        expected_version: 0,
        text: "三年级".into(),
        source: "oral".into(),
        conflict_decision: None,
    };
    let first = put_persona_fact(&db, &create_fact).await.unwrap();
    assert_eq!(first.fact.status.as_deref(), Some("new"));
    assert!(put_persona_fact(&db, &create_fact).await.unwrap().replayed);
    let mut draft_with_grade = draft(Some(account), &["合成示例：今年四年级"]);
    draft_with_grade.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let before_confirmation = check_draft(&db, &draft_with_grade).await.unwrap();
    assert!(
        before_confirmation
            .not_run
            .iter()
            .any(|item| item.rule == "R-W4")
    );
    assert!(
        !before_confirmation
            .findings
            .iter()
            .any(|item| item.rule == "R-W4")
    );
    let id = first.fact.id;
    let confirmed = confirm_persona_fact(
        &db,
        id,
        &PersonaFactConfirm {
            request_id: Uuid::new_v4(),
            expected_version: 1,
            subject_confirmed: true,
            confirmation_method: "subject_statement".into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(confirmed.fact.status.as_deref(), Some("confirmed"));
    let replay_after_confirmation = put_persona_fact(&db, &create_fact).await.unwrap();
    assert!(replay_after_confirmation.replayed);
    assert_eq!(replay_after_confirmation.result_version, 1);
    assert_eq!(replay_after_confirmation.fact.version, 2);
    let unchanged = PersonaFactPut {
        request_id: Uuid::new_v4(),
        expected_version: 2,
        ..create_fact.clone()
    };
    let unchanged_result = put_persona_fact(&db, &unchanged).await.unwrap();
    assert_eq!(unchanged_result.fact.version, 2);
    assert_eq!(unchanged_result.fact.status.as_deref(), Some("confirmed"));
    let mismatch = check_draft(&db, &draft_with_grade).await.unwrap();
    assert!(
        mismatch
            .findings
            .iter()
            .any(|item| item.rule == "R-W4" && item.level == Level::Warn)
    );
    // A directly placed comment is the commenter's statement, not the account person's fact.
    let mut quoted = comment_doc_with_text(&db, "合成评论：今年四年级").await;
    quoted.account_ref = Some(account);
    quoted.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let quote_work = create(&db, quoted).await;
    let placed = place(
        &db,
        quote_work.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: quote_work.revision.id,
            canvas_id: quote_work.revision.document.canvas[0].id,
            targets: vec![UseTarget::Paragraph {
                paragraph_id: quote_work.revision.document.paragraphs[0].id,
            }],
        },
    )
    .await
    .unwrap();
    let quoted = read_work(&db, quote_work.work_id)
        .await
        .unwrap()
        .revision
        .document;
    assert!(
        !check_draft(&db, &quoted)
            .await
            .unwrap()
            .findings
            .iter()
            .any(|item| item.rule == "R-W4")
    );
    let quoted_copy = prepare_copy(
        &db,
        quote_work.work_id,
        &CopyCommand {
            request_id: Uuid::new_v4(),
            expected_revision: placed.revision.id,
        },
    )
    .await
    .unwrap();
    let quoted_rules: serde_json::Value = sqlx::query_scalar(
        "SELECT warn_rules FROM linggan_creation_copy_receipt WHERE receipt_ref=$1",
    )
    .bind(quoted_copy.receipt_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(quoted_rules.get("R-W4").is_none());
    let saved = create(&db, draft_with_grade.clone()).await;
    let ready = prepare_copy(
        &db,
        saved.work_id,
        &CopyCommand {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
        },
    )
    .await
    .unwrap();
    let rules: serde_json::Value = sqlx::query_scalar(
        "SELECT warn_rules FROM linggan_creation_copy_receipt WHERE receipt_ref=$1",
    )
    .bind(ready.receipt_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rules["R-W4"], 1);
    draft_with_grade.paragraphs[0].text = "合成示例：今年三年级".into();
    assert!(
        !check_draft(&db, &draft_with_grade)
            .await
            .unwrap()
            .findings
            .iter()
            .any(|item| item.rule == "R-W4")
    );
    let mut conflict = PersonaFactPut {
        request_id: Uuid::new_v4(),
        expected_version: 2,
        text: "四年级".into(),
        ..create_fact.clone()
    };
    assert!(matches!(
        put_persona_fact(&db, &conflict).await,
        Err(AssetsError::Invalid("persona_conflict_decision"))
    ));
    conflict.conflict_decision = Some("keep_old".into());
    let kept = put_persona_fact(&db, &conflict).await.unwrap();
    assert_eq!(kept.fact.text.as_deref(), Some("三年级"));
    assert_eq!(kept.fact.status.as_deref(), Some("confirmed"));
    let versions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_persona_fact_revision WHERE fact_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(versions, 3);
    let history = persona_fact_history(&db, id).await.unwrap();
    assert!(
        history
            .iter()
            .any(|revision| revision.text == "四年级" && !revision.applied)
    );
    let abandoned = PersonaFactPut {
        request_id: Uuid::new_v4(),
        key: "合成其他事实".into(),
        ..create_fact.clone()
    };
    assert_eq!(
        cancel_persona_request(&db, abandoned.request_id)
            .await
            .unwrap()["outcome"],
        "cancelled"
    );
    assert!(matches!(
        put_persona_fact(&db, &abandoned).await,
        Err(AssetsError::Invalid("request_cancelled"))
    ));
    let new = PersonaFactPut {
        request_id: Uuid::new_v4(),
        expected_version: 3,
        conflict_decision: Some("use_new".into()),
        ..conflict
    };
    let changed = put_persona_fact(&db, &new).await.unwrap();
    assert_eq!(changed.fact.text.as_deref(), Some("四年级"));
    assert_eq!(changed.fact.status.as_deref(), Some("new"));
    put_identity(
        &db,
        account,
        &correction(identity(1, OrgRelation::Parent, None, Some("合成角色更正"))),
    )
    .await
    .unwrap();
    assert_eq!(
        list_persona_facts(&db, account).await.unwrap()[0]
            .text
            .as_deref(),
        Some("四年级")
    );
    let mut replacement = identity(2, OrgRelation::Unknown, None, Some("另一合成角色"));
    replacement.person_change = Some(PersonChange::Replacement);
    put_identity(&db, account, &replacement).await.unwrap();
    let former = list_persona_facts(&db, account).await.unwrap().remove(0);
    assert!(!former.current_person && former.text.is_none() && former.key.is_none());
    let withdrawn = withdraw_persona_fact(&db, id, Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(withdrawn.fact.state, "withdrawn");
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_persona_fact_revision WHERE fact_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(remaining, 0);
    assert!(matches!(
        put_persona_fact(&db, &create_fact).await,
        Err(AssetsError::NotFound)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL migration proof; no shared database"]
async fn upgrade_seeds_known_person_and_preserves_old_identity_request_replay() {
    let migration = include_str!("../../../database/migrations/0183_creation_experience_s1b.sql");
    let ledger = format!(
        "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES ('0183_creation_experience_s1b','{}');\n",
        linggan_intelligence::content_workbench_sources::text_hash(migration)
    );
    assert!(full_schema::FULL_MIGRATIONS.contains(migration));
    assert!(full_schema::FULL_MIGRATIONS.contains(&ledger));
    let before = full_schema::FULL_MIGRATIONS
        .replace(migration, "")
        .replace(&ledger, "")
        .replace(
            include_str!("../../../database/migrations/0190_creation_ai_experience_material.sql"),
            "",
        );
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "assets_person_upgrade",
        &before,
    )
    .await
    .unwrap();
    let (account, _, _) = accounts(&db).await;
    let cmd = identity(0, OrgRelation::Parent, None, Some("妈妈"));
    let hash = linggan_intelligence::content_workbench_sources::text_hash(&serde_json::json!({
        "kind":"identity", "command": {"account":account,"expected":0,"relation":"parent","bio":null,"person":true,"role":"妈妈","experience":null}
    }).to_string());
    sqlx::query("INSERT INTO linggan_creation_assets_request(request_ref,kind,command_sha256) VALUES($1,'identity',$2)").bind(cmd.request_id).bind(hash).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_creation_account_identity(identity_ref,account_ref,revision_number,request_ref,org_relation,has_real_person,backing_person_role,saved_by) VALUES($1,$2,1,$3,'parent',true,'妈妈','local_machine')")
        .bind(Uuid::new_v4()).bind(account).bind(cmd.request_id).execute(db.pool()).await.unwrap();
    sqlx::raw_sql(migration).execute(db.pool()).await.unwrap();
    assert!(
        read_identity(&db, account)
            .await
            .unwrap()
            .unwrap()
            .has_known_person
    );
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let replay = put_identity(&db, account, &cmd).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.identity.revision, 1);
    let recorded: Uuid = sqlx::query_scalar("SELECT subject_ref FROM linggan_creation_person_assignment WHERE account_ref=$1 AND action='bootstrap'").bind(account).fetch_one(db.pool()).await.unwrap();
    assert_eq!(subject, recorded);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn person_correction_keeps_consent_but_replacement_cannot_use_former_experiences() {
    let db = db("assets_person_change").await;
    let (account, _, _) = accounts(&db).await;
    let initial_command = identity(0, OrgRelation::Parent, Some(true), Some("妈妈"));
    let initial = put_identity(&db, account, &initial_command).await.unwrap();
    let original_subject: Uuid = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(db.pool())
    .await
    .unwrap();
    let original_create = ExperienceCreate {
        request_id: Uuid::new_v4(),
        account_ref: account,
        expected_person_ref: read_identity(&db, account)
            .await
            .unwrap()
            .and_then(|value| value.person_ref)
            .unwrap_or(Uuid::nil()),
        title: "虚构经历".into(),
        body: "合成经历正文。".into(),
        tags: vec![],
        origin: "oral".into(),
    };
    let created = create_experience(&db, &original_create).await.unwrap();
    assert_eq!(
        cancel_experience_request(&db, original_create.request_id)
            .await
            .unwrap()["outcome"],
        "committed"
    );
    let mut cancelled_create = original_create.clone();
    cancelled_create.request_id = Uuid::new_v4();
    assert_eq!(
        cancel_experience_request(&db, cancelled_create.request_id)
            .await
            .unwrap()["outcome"],
        "cancelled"
    );
    assert!(matches!(
        create_experience(&db, &cancelled_create).await,
        Err(AssetsError::Invalid("request_cancelled"))
    ));
    let experience = created.experience.id;
    let original_confirm = ExperienceConfirm {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        subject_confirmed: true,
        draft_use: true,
        send_to_model: None,
        confirmation_method: "subject_present".into(),
    };
    confirm_experience(&db, experience, &original_confirm)
        .await
        .unwrap();
    let mut doc = draft(Some(account), &["合成经历正文。"]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = create(&db, doc.clone()).await;
    let ambiguous = identity(1, OrgRelation::Parent, Some(true), Some("爸爸"));
    assert!(matches!(
        put_identity(&db, account, &ambiguous).await,
        Err(AssetsError::Invalid("person_change_required"))
    ));
    let corrected = put_identity(
        &db,
        account,
        &correction(identity(1, OrgRelation::Parent, Some(true), Some("爸爸"))),
    )
    .await
    .unwrap();
    assert_eq!(corrected.identity.org_relation, OrgRelation::Parent);
    assert_eq!(corrected.identity.bio_discloses_relation, Some(true));
    assert_eq!(
        corrected.identity.bio_checked_at,
        initial.identity.bio_checked_at
    );
    let same_subject: Uuid = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(same_subject, original_subject);
    let mut correction_capture = original_create.clone();
    correction_capture.request_id = Uuid::new_v4();
    assert!(create_experience(&db, &correction_capture).await.is_ok());
    assert!(
        read_usable_experience(&db, experience, 1, Some(account))
            .await
            .is_ok()
    );
    assert_eq!(
        read_work(&db, work.work_id)
            .await
            .unwrap()
            .revision
            .document
            .paragraphs[0]
            .text,
        "合成经历正文。"
    );
    let edit = ExperienceEdit {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        title: "虚构经历修订".into(),
        body: "另一版合成正文。".into(),
        tags: vec![],
        origin: "oral".into(),
    };
    edit_experience(&db, experience, &edit).await.unwrap();
    // Splitting a paragraph retains the existing confirmed source revision, even when the
    // library has since received a newer version that is awaiting confirmation.
    let mut split_doc = doc.clone();
    split_doc.paragraphs[0].text = "合成经历".into();
    let split_id = Uuid::new_v4();
    split_doc.paragraphs.push(Paragraph {
        id: split_id,
        text: "正文。".into(),
    });
    split_doc.experience_slots.push(ExperienceSlot {
        paragraph_id: split_id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let mut unrelated = split_doc.clone();
    unrelated.paragraphs[1].text = "无关的合成原创".into();
    assert!(matches!(
        save(
            &db,
            work.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
                document: unrelated
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_split_changed"))
    ));
    let split = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document: split_doc,
        },
    )
    .await
    .unwrap();
    let mut replacement = identity(2, OrgRelation::Parent, Some(true), Some("爸爸"));
    replacement.person_change = Some(PersonChange::Replacement);
    let replaced = put_identity(&db, account, &replacement).await.unwrap();
    assert_eq!(replaced.identity.org_relation, OrgRelation::Unknown);
    assert_eq!(replaced.identity.bio_discloses_relation, None);
    assert_eq!(replaced.identity.bio_checked_at, None);
    assert_eq!(replaced.identity.bio_checked_by, None);
    let historical: (String, Option<bool>) = sqlx::query_as("SELECT org_relation,bio_discloses_relation FROM linggan_creation_account_identity WHERE account_ref=$1 AND revision_number=1")
        .bind(account).fetch_one(db.pool()).await.unwrap();
    assert_eq!(historical, ("parent".to_owned(), Some(true)));
    assert!(
        put_identity(&db, account, &replacement)
            .await
            .unwrap()
            .replayed
    );
    let new_subject: Uuid = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(new_subject, original_subject);
    let historic_replay = put_identity(&db, account, &initial_command).await.unwrap();
    assert!(historic_replay.replayed);
    assert_eq!(historic_replay.identity.person_ref, Some(original_subject));
    let mut stale_capture = original_create.clone();
    stale_capture.request_id = Uuid::new_v4();
    assert!(matches!(
        create_experience(&db, &stale_capture).await,
        Err(AssetsError::Stale)
    ));
    let mut bridge_doc = draft(Some(account), &["合成正文"]);
    bridge_doc.bridge = None;
    assert!(
        check_draft(&db, &bridge_doc)
            .await
            .unwrap()
            .findings
            .iter()
            .any(|f| f.rule == "R-M1" && f.level == Level::Must)
    );
    let bridge_work = create(&db, bridge_doc).await;
    assert!(matches!(
        prepare_copy(&db, bridge_work.work_id, &CopyCommand { request_id: Uuid::new_v4(), expected_revision: bridge_work.revision.id }).await,
        Err(AssetsError::Blocked(findings)) if findings.iter().any(|f| f.rule == "R-M1")
    ));
    let reconfirmed = put_identity(
        &db,
        account,
        &identity(3, OrgRelation::Parent, Some(true), Some("爸爸")),
    )
    .await
    .unwrap();
    assert_eq!(reconfirmed.identity.org_relation, OrgRelation::Parent);
    assert_eq!(reconfirmed.identity.bio_discloses_relation, Some(true));
    let old_replay = put_identity(&db, account, &replacement).await.unwrap();
    assert!(old_replay.replayed);
    assert_eq!(old_replay.identity.org_relation, OrgRelation::Unknown);
    let latest = read_identity(&db, account).await.unwrap().unwrap();
    assert_eq!(latest.revision, 4);
    assert_eq!(latest.org_relation, OrgRelation::Parent);
    assert_eq!(latest.bio_discloses_relation, Some(true));
    let former = read_experience(&db, experience).await.unwrap();
    assert!(!former.current_person);
    assert!(former.title.is_none() && former.body.is_none() && former.tags.is_none());
    assert!(matches!(
        create_experience(&db, &original_create).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(matches!(
        edit_experience(&db, experience, &edit).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(matches!(
        confirm_experience(&db, experience, &original_confirm).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(matches!(
        read_usable_experience(&db, experience, 1, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert_eq!(
        read_work(&db, work.work_id)
            .await
            .unwrap()
            .revision
            .document
            .paragraphs[0]
            .text,
        "[这段经历当前不可使用]"
    );
    assert!(matches!(
        create_work(
            &db,
            &CreateRequest {
                request_id: Uuid::new_v4(),
                domain_ref: fixture::dossier::DOMAIN,
                document: doc
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    assert!(matches!(
        prepare_copy(
            &db,
            work.work_id,
            &CopyCommand {
                request_id: Uuid::new_v4(),
                expected_revision: split.revision.id
            }
        )
        .await,
        Err(AssetsError::SourceUnavailable)
    ));
    let new_experience = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: read_identity(&db, account)
                .await
                .unwrap()
                .and_then(|value| value.person_ref)
                .unwrap_or(Uuid::nil()),
            title: "新真人的合成经历".into(),
            body: "另一段合成经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap();
    let new_id = new_experience.experience.id;
    assert!(new_experience.experience.current_person);
    assert!(matches!(
        read_usable_experience(&db, new_id, 1, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    confirm_experience(
        &db,
        new_id,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    assert!(
        read_usable_experience(&db, new_id, 1, Some(account))
            .await
            .is_ok()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn replacement_committed_before_copy_rechecks_the_person() {
    let db = db("assets_person_copy_race").await;
    let (account, _, _) = accounts(&db).await;
    put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap();
    let experience = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: read_identity(&db, account)
                .await
                .unwrap()
                .and_then(|value| value.person_ref)
                .unwrap_or(Uuid::nil()),
            title: "合成经历".into(),
            body: "合成并发正文。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    confirm_experience(
        &db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = draft(Some(account), &["合成并发正文。"]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = create(&db, doc).await;

    // Queue replacement before copying, so the latter must recheck after the former commits.
    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let replacement_db = db.clone();
    let mut replacement = identity(1, OrgRelation::Parent, None, Some("妈妈"));
    replacement.person_change = Some(PersonChange::Replacement);
    let change =
        tokio::spawn(async move { put_identity(&replacement_db, account, &replacement).await });
    wait_for_advisory_waiters(&db, 1).await;
    let copy_db = db.clone();
    let copy = tokio::spawn(async move {
        prepare_copy(
            &copy_db,
            work.work_id,
            &CopyCommand {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    change.await.unwrap().unwrap();
    assert!(matches!(
        copy.await.unwrap(),
        Err(AssetsError::SourceUnavailable)
    ));
}

async fn wait_for_advisory_waiters(db: &Database, expected: i64) {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
            )
            .fetch_one(db.pool())
            .await
            .unwrap();
            if count >= expected {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("operation did not reach its advisory lock");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn history_edit_and_restore_save_serialize_without_deadlock() {
    let db = db("assets_history_lock_order").await;
    let (account, _, _) = accounts(&db).await;
    put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap();
    let mut ids = Vec::new();
    for text in ["甲合成经历。", "乙合成经历。"] {
        let id = create_experience(
            &db,
            &ExperienceCreate {
                request_id: Uuid::new_v4(),
                account_ref: account,
                expected_person_ref: read_identity(&db, account)
                    .await
                    .unwrap()
                    .and_then(|value| value.person_ref)
                    .unwrap_or(Uuid::nil()),
                title: "合成经历".into(),
                body: text.into(),
                tags: vec![],
                origin: "oral".into(),
            },
        )
        .await
        .unwrap()
        .experience
        .id;
        confirm_experience(
            &db,
            id,
            &ExperienceConfirm {
                request_id: Uuid::new_v4(),
                expected_revision: 1,
                subject_confirmed: true,
                draft_use: true,
                send_to_model: None,
                confirmation_method: "subject_present".into(),
            },
        )
        .await
        .unwrap();
        ids.push(id);
    }
    let mut doc = draft(Some(account), &["甲合成经历。"]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: ids[0],
        experience_revision: 1,
    });
    let first = create(&db, doc.clone()).await;
    let second_id = Uuid::new_v4();
    doc.paragraphs.push(Paragraph {
        id: second_id,
        text: "乙合成经历。".into(),
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: second_id,
        experience_ref: ids[1],
        experience_revision: 1,
    });
    let second = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: doc.clone(),
        },
    )
    .await
    .unwrap();

    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let history_db = db.clone();
    let work_id = first.work_id;
    let history = tokio::spawn(async move { revisions(&history_db, work_id).await });
    wait_for_advisory_waiters(&db, 1).await;
    let edit_db = db.clone();
    let experience = ids[1];
    let edit = tokio::spawn(async move {
        edit_experience(
            &edit_db,
            experience,
            &ExperienceEdit {
                request_id: Uuid::new_v4(),
                expected_revision: 1,
                title: "合成修订".into(),
                body: "乙合成修订。".into(),
                tags: vec![],
                origin: "oral".into(),
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        assert_eq!(history.await.unwrap().unwrap().len(), 2);
        edit.await.unwrap().unwrap();
    })
    .await
    .expect("history and edit did not finish");

    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let restore_db = db.clone();
    let old_revision = first.revision.id;
    let latest_revision = second.revision.id;
    let restoring = tokio::spawn(async move {
        restore(
            &restore_db,
            work_id,
            &RestoreRequest {
                request_id: Uuid::new_v4(),
                expected_revision: latest_revision,
                restore_revision: old_revision,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 1).await;
    let save_db = db.clone();
    let saving = tokio::spawn(async move {
        save(
            &save_db,
            work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: latest_revision,
                document: doc,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        assert_eq!(restoring.await.unwrap().unwrap().outcome, "accepted");
        assert_eq!(saving.await.unwrap().unwrap().outcome, "stale");
    })
    .await
    .expect("restore and save did not finish");
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn empty_experience_slot_is_reported_and_blocks_copy() {
    let db = db("assets_experience_gap").await;
    let (account, _, _) = accounts(&db).await;
    let mut doc = draft(Some(account), &["合成正文", ""]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    doc.experience_gaps.push(doc.paragraphs[1].id);
    let report = check_draft(&db, &doc).await.unwrap();
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.rule == "R-M8" && f.level == Level::Must)
    );
    let mut selected_but_empty = doc.clone();
    selected_but_empty.experience_gaps.clear();
    selected_but_empty.experience_slots.push(ExperienceSlot {
        paragraph_id: selected_but_empty.paragraphs[1].id,
        experience_ref: Uuid::new_v4(),
        experience_revision: 1,
    });
    assert!(
        check_draft(&db, &selected_but_empty)
            .await
            .unwrap()
            .findings
            .iter()
            .any(|f| f.rule == "R-M8")
    );
    let saved = create(&db, doc).await;
    assert!(
        matches!(prepare_copy(&db, saved.work_id, &CopyCommand { request_id: Uuid::new_v4(), expected_revision: saved.revision.id }).await,
        Err(AssetsError::Blocked(findings)) if findings.iter().any(|f| f.rule == "R-M8"))
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn first_person_advertising_copy_does_not_require_personal_experience_binding() {
    let db = db("assets_narrative_without_personal_slot").await;
    let (account, _, _) = accounts(&db).await;
    let mut doc = draft(Some(account), &["合成示例：我也遇到过这样的日常。"]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let report = check_draft(&db, &doc).await.unwrap();
    assert!(!report.findings.iter().any(|finding| finding.rule == "R-M7"));
    let work = create(&db, doc).await;
    assert!(
        prepare_copy(
            &db,
            work.work_id,
            &CopyCommand {
                request_id: Uuid::new_v4(),
                expected_revision: work.revision.id
            }
        )
        .await
        .is_ok()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn anonymous_comment_can_be_used_with_its_source_and_copied_without_personal_binding() {
    let db = db("assets_comment_reference").await;
    let (account, _, _) = accounts(&db).await;
    let mut document = comment_doc(&db).await;
    document.account_ref = Some(account);
    document.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let source = document.excerpts[0].citation.source_ref;
    let work = create(&db, document).await;
    let placed = place(
        &db,
        work.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            canvas_id: work.revision.document.canvas[0].id,
            targets: vec![UseTarget::Paragraph {
                paragraph_id: work.revision.document.paragraphs[0].id,
            }],
        },
    )
    .await
    .unwrap();
    let saved = read_work(&db, work.work_id).await.unwrap();
    assert_eq!(saved.revision.id, placed.revision.id);
    assert_eq!(
        saved.revision.document.excerpts[0].citation.source_ref,
        source
    );
    assert_eq!(
        serde_json::to_value(&saved.revision.document.excerpts[0].citation.source_kind).unwrap(),
        "comment"
    );
    assert_eq!(saved.revision.document.uses.len(), 1);
    assert!(saved.revision.document.experience_slots.is_empty());
    let ready = prepare_copy(
        &db,
        work.work_id,
        &CopyCommand {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
        },
    )
    .await
    .unwrap();
    assert!(ready.text.contains("合成匿名原声"));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn experience_draft_reference_erases_history_and_copy_on_withdrawal() {
    let db = db("assets_experience_draft").await;
    let (account, _, other) = accounts(&db).await;
    put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap();
    let made = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: read_identity(&db, account)
                .await
                .unwrap()
                .and_then(|value| value.person_ref)
                .unwrap_or(Uuid::nil()),
            title: "虚构".into(),
            body: "合成经历，只供测试。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap();
    let id = made.experience.id;
    assert_eq!(made.experience.used_count, 0);
    confirm_experience(
        &db,
        id,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut short = draft(Some(account), &["我"]);
    short.title = "我今天的想法".into();
    short.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    short.experience_slots.push(ExperienceSlot {
        paragraph_id: short.paragraphs[0].id,
        experience_ref: id,
        experience_revision: 1,
    });
    let short_work = create_work(
        &db,
        &CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: fixture::dossier::DOMAIN,
            document: short.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(read_experience(&db, id).await.unwrap().used_count, 1);
    short.tags.push("合成".into());
    assert!(
        save(
            &db,
            short_work.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: short_work.revision.id,
                document: short
            }
        )
        .await
        .is_ok()
    );
    assert_eq!(read_experience(&db, id).await.unwrap().used_count, 1);
    let repeated_doc = read_work(&db, short_work.work_id)
        .await
        .unwrap()
        .revision
        .document;
    let own_check = check_draft_for_work(&db, &repeated_doc, Some(short_work.work_id))
        .await
        .unwrap();
    assert!(
        !own_check
            .findings
            .iter()
            .any(|finding| finding.rule == "R-W5")
    );
    let other_work = create(&db, repeated_doc.clone()).await;
    assert_eq!(read_experience(&db, id).await.unwrap().used_count, 2);
    let repeated_check = check_draft_for_work(&db, &repeated_doc, Some(short_work.work_id))
        .await
        .unwrap();
    assert!(
        repeated_check
            .findings
            .iter()
            .any(|finding| finding.rule == "R-W5" && finding.level == Level::Warn)
    );
    let current_short = read_work(&db, short_work.work_id).await.unwrap();
    let batch_request = SavedDraftCheckRequest {
        domain_ref: fixture::dossier::DOMAIN,
        items: vec![SavedDraftCheckItem {
            work_id: short_work.work_id,
            revision_id: current_short.revision.id,
        }],
    };
    let checked = check_saved_drafts(&db, &batch_request).await.unwrap();
    assert_eq!(checked.len(), 1);
    assert_eq!(checked[0].warn, repeated_check.count(Level::Warn));
    let mut different_account = repeated_doc.clone();
    different_account.account_ref = Some(other);
    assert!(
        !check_draft_for_work(&db, &different_account, Some(short_work.work_id))
            .await
            .unwrap()
            .findings
            .iter()
            .any(|finding| finding.rule == "R-W5")
    );
    let mut with_spelling_warning = repeated_doc.clone();
    with_spelling_warning.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "adhd".into(),
    });
    let order = check_draft_for_work(&db, &with_spelling_warning, Some(short_work.work_id))
        .await
        .unwrap()
        .findings;
    assert!(
        order
            .iter()
            .position(|finding| finding.rule == "R-W5")
            .unwrap()
            < order
                .iter()
                .position(|finding| finding.rule == "R-W6")
                .unwrap()
    );
    let mut many = repeated_doc.clone();
    for _ in 0..35 {
        let paragraph_id = Uuid::new_v4();
        many.paragraphs.push(Paragraph {
            id: paragraph_id,
            text: "合成段落".into(),
        });
        many.experience_slots.push(ExperienceSlot {
            paragraph_id,
            experience_ref: id,
            experience_revision: 1,
        });
    }
    let warnings = check_draft_for_work(&db, &many, Some(short_work.work_id))
        .await
        .unwrap();
    assert_eq!(
        warnings
            .findings
            .iter()
            .filter(|finding| finding.rule == "R-W5")
            .count(),
        MAX_FINDINGS_PER_RULE
    );
    let copy = prepare_copy(
        &db,
        other_work.work_id,
        &CopyCommand {
            request_id: Uuid::new_v4(),
            expected_revision: other_work.revision.id,
        },
    )
    .await
    .unwrap();
    assert!(copy.warn_count > 0);
    let warnings: serde_json::Value = sqlx::query_scalar(
        "SELECT warn_rules FROM linggan_creation_copy_receipt WHERE receipt_ref=$1",
    )
    .bind(copy.receipt_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(warnings["R-W5"], 1);
    let mut unbound = repeated_doc.clone();
    unbound.experience_slots.clear();
    assert!(
        !check_draft_for_work(&db, &unbound, Some(other_work.work_id))
            .await
            .unwrap()
            .findings
            .iter()
            .any(|finding| finding.rule == "R-W5")
    );
    // Independently authored identical text remains movable after a later experience binding
    // is removed; an unchanged amount of unbound text is not an added copy.
    let mut independent = draft(Some(account), &["独立正文"]);
    independent.title = "独立同文".into();
    independent.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let independent_work = create(&db, independent.clone()).await;
    let bound_id = Uuid::new_v4();
    independent.paragraphs.push(Paragraph {
        id: bound_id,
        text: "独立同文".into(),
    });
    independent.experience_slots.push(ExperienceSlot {
        paragraph_id: bound_id,
        experience_ref: id,
        experience_revision: 1,
    });
    let bound = save(
        &db,
        independent_work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: independent_work.revision.id,
            document: independent.clone(),
        },
    )
    .await
    .unwrap();
    independent.experience_slots.clear();
    independent.paragraphs.retain(|p| p.id != bound_id);
    let unbound = save(
        &db,
        independent_work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: bound.revision.id,
            document: independent.clone(),
        },
    )
    .await
    .unwrap();
    independent.title = "另外的标题".into();
    independent.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "独立同文".into(),
    });
    assert!(
        save(
            &db,
            independent_work.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: unbound.revision.id,
                document: independent
            }
        )
        .await
        .is_ok()
    );
    let mut doc = draft(Some(account), &["合成改写经历，只供测试。"]);
    doc.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: id,
        experience_revision: 1,
    });
    let request = CreateRequest {
        request_id: Uuid::new_v4(),
        domain_ref: fixture::dossier::DOMAIN,
        document: doc.clone(),
    };
    let mut initial_leak = request.clone();
    initial_leak.request_id = Uuid::new_v4();
    initial_leak.document.title = "合成经历，只供测试。".into();
    assert!(matches!(
        create_work(&db, &initial_leak).await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    let mut candidate_leak = request.clone();
    candidate_leak.request_id = Uuid::new_v4();
    candidate_leak
        .document
        .note_title_candidates
        .push("合成经历，只供测试。".into());
    assert!(matches!(
        create_work(&db, &candidate_leak).await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    let mut other_request = request.clone();
    other_request.request_id = Uuid::new_v4();
    other_request.document.account_ref = Some(other);
    assert!(matches!(
        create_work(&db, &other_request).await,
        Err(WorkbenchError::Source)
    ));
    let made = create_work(&db, &request).await.unwrap();
    let mut history_request = request.clone();
    history_request.request_id = Uuid::new_v4();
    let historical_work = create_work(&db, &history_request).await.unwrap();
    let mut removed_slot = historical_work.revision.document.clone();
    removed_slot.experience_slots.clear();
    removed_slot.paragraphs.clear();
    removed_slot.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "独立正文".into(),
    });
    let removed_slot = save(
        &db,
        historical_work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: historical_work.revision.id,
            document: removed_slot,
        },
    )
    .await
    .unwrap();
    let mut pasted_history = removed_slot.revision.document.clone();
    pasted_history.title = "合成改写经历，只供测试。".into();
    assert!(matches!(
        save(
            &db,
            historical_work.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: removed_slot.revision.id,
                document: pasted_history
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    assert_eq!(
        made.revision.document.paragraphs[0].text,
        "合成改写经历，只供测试。"
    );
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT document FROM linggan_creation_revision WHERE revision_ref=$1")
            .bind(made.revision.id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(stored["paragraphs"][0]["text"], "");
    let stored_receipt: serde_json::Value =
        sqlx::query_scalar("SELECT receipt FROM linggan_creation_request WHERE request_ref=$1")
            .bind(request.request_id)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        stored_receipt["revision"]["document"]["paragraphs"][0]["text"],
        ""
    );
    assert_eq!(
        read_work(&db, made.work_id)
            .await
            .unwrap()
            .revision
            .document
            .paragraphs[0]
            .text,
        "合成改写经历，只供测试。"
    );
    for variant in 0..3 {
        let mut copied = doc.clone();
        let text = copied.paragraphs[0].text.clone();
        match variant {
            0 => {
                copied.experience_slots.clear();
                copied.paragraphs[0].id = Uuid::new_v4();
            }
            1 => copied.title = text,
            _ => copied.pages[0].blocks[0].text = text,
        }
        assert!(matches!(
            save(
                &db,
                made.work_id,
                &SaveRequest {
                    request_id: Uuid::new_v4(),
                    expected_revision: made.revision.id,
                    document: copied,
                }
            )
            .await,
            Err(WorkbenchError::Invalid("experience_untracked_text"))
        ));
    }
    let mut detached = doc.clone();
    detached.experience_slots.clear();
    assert!(matches!(
        save(
            &db,
            made.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: made.revision.id,
                document: detached
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    let mut revised_doc = doc.clone();
    revised_doc.paragraphs[0].text = "另一版合成改写经历。".into();
    let saved = save(
        &db,
        made.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: made.revision.id,
            document: revised_doc,
        },
    )
    .await
    .unwrap();
    let mut copied_old = saved.revision.document.clone();
    copied_old.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "合成改写经历，只供测试。".into(),
    });
    assert!(matches!(
        save(
            &db,
            made.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                document: copied_old
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    let mut copied_asset = saved.revision.document.clone();
    copied_asset.title = "合成经历，只供测试。".into();
    assert!(matches!(
        save(
            &db,
            made.work_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                document: copied_asset
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_untracked_text"))
    ));
    edit_experience(
        &db,
        id,
        &ExperienceEdit {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            title: "虚构新版本".into(),
            body: "另一段合成经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap();
    let mut new_work = request.clone();
    new_work.request_id = Uuid::new_v4();
    assert!(matches!(
        create_work(&db, &new_work).await,
        Err(WorkbenchError::Source)
    ));
    let mut continuing = saved.revision.document.clone();
    continuing.title = "仍在编辑原稿".into();
    let saved = save(
        &db,
        made.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: continuing,
        },
    )
    .await
    .unwrap();
    assert!(matches!(
        freeze_export(
            &db,
            made.work_id,
            &FreezeRequest {
                request_id: Uuid::new_v4(),
                expected_revision: saved.revision.id,
                renderer_version: "creation-renderer.v1".into(),
                font_version: "synthetic".into()
            }
        )
        .await,
        Err(WorkbenchError::Source)
    ));
    let copy = CopyCommand {
        request_id: Uuid::new_v4(),
        expected_revision: saved.revision.id,
    };
    let protected_copy = prepare_copy(&db, made.work_id, &copy).await.unwrap();
    let digest_before: String = sqlx::query_scalar(
        "SELECT text_sha256 FROM linggan_creation_copy_receipt WHERE receipt_ref=$1",
    )
    .bind(protected_copy.receipt_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_ne!(digest_before, "0".repeat(64));
    let mut no_person = identity(1, OrgRelation::Parent, None, None);
    no_person.has_real_person = Some(false);
    put_identity(&db, account, &no_person).await.unwrap();
    assert!(matches!(
        prepare_copy(&db, made.work_id, &copy).await,
        Err(AssetsError::SourceUnavailable)
    ));
    put_identity(
        &db,
        account,
        &correction(identity(2, OrgRelation::Parent, None, Some("妈妈"))),
    )
    .await
    .unwrap();
    // A second editor still holds a copy after the current draft removed the reference.
    // Queue withdrawal first; an unbound save must not persist the stale text afterward.
    let references_before_withdrawal = read_experience(&db, id).await.unwrap().used_count;
    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let withdraw_db = db.clone();
    let withdrawal =
        tokio::spawn(async move { withdraw_experience(&withdraw_db, id, Uuid::new_v4()).await });
    wait_for_advisory_waiters(&db, 1).await;
    let save_db = db.clone();
    let mut stale_text = removed_slot.revision.document.clone();
    stale_text.title = "合成改写经历，只供测试。".into();
    let old_revision = removed_slot.revision.id;
    let historical_id = historical_work.work_id;
    let saving = tokio::spawn(async move {
        save(
            &save_db,
            historical_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: old_revision,
                document: stale_text,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    withdrawal.await.unwrap().unwrap();
    let digest_after: String = sqlx::query_scalar(
        "SELECT text_sha256 FROM linggan_creation_copy_receipt WHERE receipt_ref=$1",
    )
    .bind(protected_copy.receipt_ref)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(digest_after, "0".repeat(64));
    assert!(matches!(
        prepare_copy(&db, made.work_id, &copy).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(
        check_saved_drafts(&db, &batch_request)
            .await
            .unwrap()
            .is_empty(),
        "withdrawn source must not retain an earlier check status"
    );
    assert_eq!(
        read_experience(&db, id).await.unwrap().used_count,
        references_before_withdrawal
    );
    assert!(matches!(
        saving.await.unwrap(),
        Err(WorkbenchError::Invalid(
            "experience_withdrawal_requires_reset"
        ))
    ));
    assert!(
        read_work(&db, historical_id)
            .await
            .unwrap()
            .experience_reset_required
    );
    let reset = ResetWithdrawnRequest {
        request_id: Uuid::new_v4(),
        expected_revision: old_revision,
    };
    let resumed = reset_withdrawn(&db, historical_id, &reset).await.unwrap();
    assert_eq!(resumed.outcome, "accepted");
    assert_ne!(resumed.revision.id, old_revision);
    assert!(
        !read_work(&db, historical_id)
            .await
            .unwrap()
            .experience_reset_required
    );
    assert!(
        reset_withdrawn(&db, historical_id, &reset)
            .await
            .unwrap()
            .replayed
    );
    let old_page = save(
        &db,
        historical_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: old_revision,
            document: removed_slot.revision.document.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(old_page.outcome, "stale");
    assert!(matches!(
        save(
            &db,
            historical_id,
            &SaveRequest {
                request_id: Uuid::new_v4(),
                expected_revision: resumed.revision.id,
                document: removed_slot.revision.document.clone()
            }
        )
        .await,
        Err(WorkbenchError::Invalid("experience_editor_reset_required"))
    ));
    let mut safe = resumed.revision.document.clone();
    safe.paragraphs[0].text = "重新开始的独立正文".into();
    let saved_after = save(
        &db,
        historical_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: resumed.revision.id,
            document: safe,
        },
    )
    .await
    .unwrap();
    assert_eq!(saved_after.outcome, "accepted");
    assert_eq!(
        read_work(&db, made.work_id)
            .await
            .unwrap()
            .revision
            .document
            .paragraphs[0]
            .text,
        "[本人已撤回这段经历]"
    );
    assert_eq!(
        revisions(&db, made.work_id).await.unwrap()[0]
            .document
            .paragraphs[0]
            .text,
        "[本人已撤回这段经历]"
    );
    assert!(
        revisions(&db, made.work_id)
            .await
            .unwrap()
            .iter()
            .all(|revision| revision.document.paragraphs[0].text == "[本人已撤回这段经历]")
    );
    let replay = read_request(&db, request.request_id).await.unwrap();
    assert_eq!(
        replay["receipt"]["revision"]["document"]["paragraphs"][0]["text"],
        "[本人已撤回这段经历]"
    );
    assert!(matches!(
        prepare_copy(&db, made.work_id, &copy).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let fragments: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_fragment WHERE experience_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(fragments, 0);
    let current = read_work(&db, made.work_id).await.unwrap();
    reset_withdrawn(
        &db,
        made.work_id,
        &ResetWithdrawnRequest {
            request_id: Uuid::new_v4(),
            expected_revision: current.revision.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        read_experience(&db, id).await.unwrap().used_count,
        references_before_withdrawal - 1
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn experience_payload_is_erasable_and_confirmation_is_explicit() {
    let db = db("assets_experience").await;
    let (account, _, _) = accounts(&db).await;
    let mut cmd = ExperienceCreate {
        request_id: Uuid::new_v4(),
        account_ref: account,
        expected_person_ref: read_identity(&db, account)
            .await
            .unwrap()
            .and_then(|value| value.person_ref)
            .unwrap_or(Uuid::nil()),
        title: "合成经历".into(),
        body: "这是一段仅用于测试的虚构经历。".into(),
        tags: vec!["合成".into()],
        origin: "oral".into(),
    };
    assert!(matches!(
        create_experience(&db, &cmd).await,
        Err(AssetsError::Invalid(_))
    ));
    let mut unknown_role = identity(0, OrgRelation::Parent, None, None);
    unknown_role.has_real_person = Some(true);
    put_identity(&db, account, &unknown_role).await.unwrap();
    cmd.expected_person_ref = read_identity(&db, account)
        .await
        .unwrap()
        .unwrap()
        .person_ref
        .unwrap();
    let made = create_experience(&db, &cmd).await.unwrap();
    assert_eq!(made.experience.state, "awaiting_subject");
    let id = made.experience.id;
    assert!(create_experience(&db, &cmd).await.unwrap().replayed);
    assert!(matches!(
        read_usable_experience(&db, id, 1, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let mut confirm = ExperienceConfirm {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        subject_confirmed: false,
        draft_use: true,
        send_to_model: Some(false),
        confirmation_method: "subject_statement".into(),
    };
    assert!(matches!(
        confirm_experience(&db, id, &confirm).await,
        Err(AssetsError::Invalid(_))
    ));
    confirm.subject_confirmed = true;
    assert!(matches!(
        confirm_experience(&db, id, &confirm).await,
        Err(AssetsError::Invalid("subject_role"))
    ));
    put_identity(
        &db,
        account,
        &correction(identity(1, OrgRelation::Parent, None, Some("妈妈"))),
    )
    .await
    .unwrap();
    let confirmed = confirm_experience(&db, id, &confirm).await.unwrap();
    assert_eq!(confirmed.experience.state, "confirmed");
    assert_eq!(confirmed.experience.send_to_model, Some(false));
    assert!(confirmed.experience.confirmed_at.is_some());
    assert_eq!(
        confirmed.experience.confirmed_subject_account_ref,
        Some(account)
    );
    assert_eq!(
        confirmed.experience.confirmed_subject_role.as_deref(),
        Some("妈妈")
    );
    assert_eq!(
        confirmed.experience.confirmation_method.as_deref(),
        Some("subject_statement")
    );
    put_identity(
        &db,
        account,
        &correction(identity(2, OrgRelation::Parent, None, Some("爸爸"))),
    )
    .await
    .unwrap();
    assert_eq!(
        read_experience(&db, id)
            .await
            .unwrap()
            .confirmed_subject_role
            .as_deref(),
        Some("妈妈")
    );
    assert!(
        read_usable_experience(&db, id, 1, Some(account))
            .await
            .is_ok()
    );
    assert!(matches!(
        read_usable_experience(&db, id, 1, Some(Uuid::new_v4())).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let edit = ExperienceEdit {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        title: "合成经历修订".into(),
        body: "另一段虚构测试内容。".into(),
        tags: vec!["合成".into()],
        origin: "oral".into(),
    };
    let revised = edit_experience(&db, id, &edit).await.unwrap();
    assert_eq!(
        (
            revised.experience.revision,
            revised.experience.state.as_str()
        ),
        (2, "awaiting_subject")
    );
    assert!(revised.experience.confirmed_at.is_none());
    let create_replay = create_experience(&db, &cmd).await.unwrap();
    assert!(create_replay.replayed);
    assert_eq!(create_replay.experience.revision, 1);
    assert_eq!(create_replay.experience.state, "awaiting_subject");
    assert!(create_replay.experience.confirmed_at.is_none());
    let confirm_replay = confirm_experience(&db, id, &confirm).await.unwrap();
    assert!(confirm_replay.replayed);
    assert_eq!(confirm_replay.experience.revision, 1);
    assert_eq!(confirm_replay.experience.state, "confirmed");
    assert!(matches!(
        read_usable_experience(&db, id, 2, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(matches!(
        read_usable_experience(&db, id, 1, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let stale = ExperienceEdit {
        request_id: Uuid::new_v4(),
        ..edit.clone()
    };
    assert!(matches!(
        edit_experience(&db, id, &stale).await,
        Err(AssetsError::Stale)
    ));
    let mut second_consent = confirm.clone();
    second_consent.request_id = Uuid::new_v4();
    second_consent.expected_revision = 2;
    assert_eq!(
        confirm_experience(&db, id, &second_consent)
            .await
            .unwrap()
            .experience
            .state,
        "confirmed"
    );
    let withdrawal = Uuid::new_v4();
    let removed = withdraw_experience(&db, id, withdrawal).await.unwrap();
    assert_eq!(removed.experience.state, "withdrawn");
    assert!(matches!(
        read_usable_experience(&db, id, 1, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(matches!(
        read_usable_experience(&db, id, 2, Some(account)).await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(
        removed.experience.body.is_none()
            && removed.experience.title.is_none()
            && removed.experience.tags.is_none()
    );
    assert!(
        withdraw_experience(&db, id, withdrawal)
            .await
            .unwrap()
            .replayed
    );
    assert!(matches!(
        create_experience(&db, &cmd).await,
        Err(AssetsError::NotFound)
    ));
    assert!(matches!(
        confirm_experience(&db, id, &confirm).await,
        Err(AssetsError::NotFound)
    ));
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_revision WHERE experience_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rows, 0);
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_creation_experience_consent WHERE experience_ref=$1",
    )
    .bind(id)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(rows, 0);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creation_experience_request WHERE experience_ref=$1 AND kind <> 'withdraw' AND command_sha256 IS NOT NULL").bind(id).fetch_one(db.pool()).await.unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn copy_and_replay_requalify_restricted_sources_without_blocking_unrelated_text() {
    let db = db("assets_copy_source").await;
    let mut document = comment_doc(&db).await;
    document.account_ref = Some(account(&db).await);
    document.bridge = Some(BridgeChoice {
        skip: true,
        sentence: None,
        after_paragraph: None,
    });
    let source_work = document.excerpts[0].citation.work_ref.unwrap();
    let first = create(&db, document).await;
    let placed = place(
        &db,
        first.work_id,
        &PlaceRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            canvas_id: first.revision.document.canvas[0].id,
            targets: vec![UseTarget::Paragraph {
                paragraph_id: first.revision.document.paragraphs[0].id,
            }],
        },
    )
    .await
    .unwrap();
    let copy = CopyCommand {
        request_id: Uuid::new_v4(),
        expected_revision: placed.revision.id,
    };
    assert!(prepare_copy(&db, first.work_id, &copy).await.is_ok());
    let batch = SavedDraftCheckRequest {
        domain_ref: fixture::dossier::DOMAIN,
        items: vec![SavedDraftCheckItem {
            work_id: first.work_id,
            revision_id: placed.revision.id,
        }],
    };
    assert_eq!(check_saved_drafts(&db, &batch).await.unwrap().len(), 1);
    restrict_comment(&db, source_work).await;
    assert!(check_saved_drafts(&db, &batch).await.unwrap().is_empty());
    assert!(matches!(
        prepare_copy(&db, first.work_id, &copy).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let fresh = CopyCommand {
        request_id: Uuid::new_v4(),
        expected_revision: placed.revision.id,
    };
    assert!(matches!(
        prepare_copy(&db, first.work_id, &fresh).await,
        Err(AssetsError::SourceUnavailable)
    ));
    let mut own = read_work(&db, first.work_id)
        .await
        .unwrap()
        .revision
        .document;
    own.paragraphs[0].text = "合成的独立正文".into();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: placed.revision.id,
            document: own,
        },
    )
    .await
    .unwrap();
    let independent = CopyCommand {
        request_id: Uuid::new_v4(),
        expected_revision: saved.revision.id,
    };
    let output = prepare_copy(&db, first.work_id, &independent)
        .await
        .unwrap();
    assert_eq!(output.body, "合成的独立正文");
}

async fn account_by_source(db: &Database, source: &str) -> Uuid {
    sqlx::query_scalar("SELECT account_ref FROM linggan_ops_account WHERE source_record_id=$1")
        .bind(source)
        .fetch_one(db.pool())
        .await
        .unwrap()
}
/// Seeds the ops ledger and returns (xhs account 1, douyin account, xhs account 3).
async fn accounts(db: &Database) -> (Uuid, Uuid, Uuid) {
    let _ = fixture::account(db).await;
    (
        account_by_source(db, "acc-1").await,
        account_by_source(db, "acc-2").await,
        account_by_source(db, "acc-3").await,
    )
}
fn identity(
    expected: i32,
    relation: OrgRelation,
    bio: Option<bool>,
    role: Option<&str>,
) -> IdentityCommand {
    IdentityCommand {
        request_id: Uuid::new_v4(),
        expected_revision: expected,
        org_relation: relation,
        bio_discloses_relation: bio,
        has_real_person: role.map(|_| true),
        backing_person_role: role.map(str::to_string),
        can_provide_experience: None,
        person_change: None,
    }
}
fn correction(mut command: IdentityCommand) -> IdentityCommand {
    command.person_change = Some(PersonChange::Correction);
    command
}
fn card(expected: i32, name: &str, allowed: &[&str], banned: &[&str]) -> FactCardCommand {
    FactCardCommand {
        request_id: Uuid::new_v4(),
        expected_revision: expected,
        name: name.into(),
        allowed_names: allowed.iter().map(|s| (*s).into()).collect(),
        banned_names: banned.iter().map(|s| (*s).into()).collect(),
        what: "合成：一个示例产品".into(),
        who: String::new(),
        can_say: String::new(),
        cannot_say: String::new(),
        must_note: String::new(),
    }
}
fn draft(account: Option<Uuid>, paragraphs: &[&str]) -> WorkDocument {
    WorkDocument {
        title: "合成标题".into(),
        account_ref: account,
        paragraphs: paragraphs
            .iter()
            .map(|text| Paragraph {
                id: Uuid::new_v4(),
                text: (*text).into(),
            })
            .collect(),
        pages: vec![page(PageKind::Cover)],
        ..WorkDocument::default()
    }
}
fn using(sentence: &Sentence) -> BridgeChoice {
    BridgeChoice {
        skip: false,
        sentence: Some(BridgeUse {
            sentence_code: sentence.code.clone(),
            sentence_revision: sentence.revision,
            group: sentence.group,
            text: sentence.text.clone(),
            edited: false,
        }),
        after_paragraph: None,
    }
}
async fn approved(db: &Database, group: BridgeGroup, text: &str) -> Sentence {
    let made = create_sentence(
        db,
        &SentenceCreate {
            request_id: Uuid::new_v4(),
            group,
            text: text.into(),
            origin: SentenceOrigin::Manual,
        },
    )
    .await
    .unwrap();
    review_sentence(
        db,
        &made.sentence.code,
        &SentenceReview {
            request_id: Uuid::new_v4(),
            expected_revision: made.sentence.revision,
            action: ReviewAction::Approve,
            note: None,
        },
    )
    .await
    .unwrap()
    .sentence
}
fn rules(report: &CheckReport) -> Vec<&str> {
    report.findings.iter().map(|f| f.rule.as_str()).collect()
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn identity_revisions_replay_conflict_stale_and_keep_unknown_unknown() {
    let db = db("assets_identity").await;
    let (xhs_a, douyin, xhs_c) = accounts(&db).await;
    assert!(read_identity(&db, xhs_a).await.unwrap().is_none());

    let listed = list_accounts(&db, None).await.unwrap();
    let ids: Vec<Uuid> = listed.iter().map(|a| a.account_ref).collect();
    assert!(
        ids.contains(&xhs_a) && ids.contains(&xhs_c),
        "xhs accounts are listed"
    );
    assert!(
        !ids.contains(&douyin),
        "only Xiaohongshu accounts are written for"
    );
    assert!(listed.iter().all(|a| a.identity.is_none()));

    let first = identity(0, OrgRelation::Parent, None, Some("妈妈"));
    let made = put_identity(&db, xhs_a, &first).await.unwrap();
    assert_eq!((made.identity.revision, made.replayed), (1, false));
    assert_eq!(
        made.identity.bio_discloses_relation, None,
        "unknown stays unknown"
    );
    assert!(made.identity.bio_checked_at.is_none() && made.identity.bio_checked_by.is_none());
    assert_eq!(made.identity.backing_person_role.as_deref(), Some("妈妈"));

    let again = put_identity(&db, xhs_a, &first).await.unwrap();
    assert!(again.replayed);
    assert_eq!(again.identity, made.identity);
    let mut changed = first.clone();
    changed.org_relation = OrgRelation::Coach;
    assert!(matches!(
        put_identity(&db, xhs_a, &changed).await,
        Err(AssetsError::RequestConflict)
    ));
    assert!(matches!(
        put_identity(&db, xhs_a, &identity(0, OrgRelation::Coach, None, None)).await,
        Err(AssetsError::Stale)
    ));

    let checked = put_identity(
        &db,
        xhs_a,
        &identity(1, OrgRelation::Parent, Some(true), Some("妈妈")),
    )
    .await
    .unwrap()
    .identity;
    assert_eq!(checked.revision, 2);
    assert!(checked.bio_checked_at.is_some());
    assert_eq!(checked.bio_checked_by.as_deref(), Some("local_machine"));
    let carried = put_identity(
        &db,
        xhs_a,
        &correction(identity(2, OrgRelation::Parent, Some(true), Some("奶奶"))),
    )
    .await
    .unwrap()
    .identity;
    assert_eq!(
        carried.bio_checked_at, checked.bio_checked_at,
        "an unchanged answer keeps its time"
    );
    let flipped = put_identity(
        &db,
        xhs_a,
        &identity(3, OrgRelation::Parent, Some(false), None),
    )
    .await
    .unwrap()
    .identity;
    assert_eq!(flipped.bio_discloses_relation, Some(false));
    assert!(flipped.bio_checked_at.is_some());
    let cleared = put_identity(&db, xhs_a, &identity(4, OrgRelation::Unknown, None, None))
        .await
        .unwrap()
        .identity;
    assert!(cleared.bio_checked_at.is_none());
    assert_eq!(
        read_identity(&db, xhs_a).await.unwrap().unwrap().revision,
        5
    );
    put_identity(
        &db,
        xhs_a,
        &identity(5, OrgRelation::Parent, Some(true), None),
    )
    .await
    .unwrap();
    let changed_relation = put_identity(&db, xhs_a, &identity(6, OrgRelation::Coach, None, None))
        .await
        .unwrap()
        .identity;
    assert_eq!(changed_relation.bio_discloses_relation, None);
    assert!(changed_relation.bio_checked_at.is_none());
    let listed = list_accounts(&db, None).await.unwrap();
    let row = listed.iter().find(|a| a.account_ref == xhs_a).unwrap();
    assert_eq!(row.identity.as_ref().unwrap().revision, 7);
    assert!(
        listed
            .iter()
            .find(|a| a.account_ref == xhs_c)
            .unwrap()
            .identity
            .is_none()
    );

    // A role needs a real person behind the account; an unknown account does not exist.
    let mut bad = identity(0, OrgRelation::Parent, None, None);
    bad.backing_person_role = Some("妈妈".into());
    assert!(matches!(
        put_identity(&db, xhs_c, &bad).await,
        Err(AssetsError::Invalid("backing_person_role"))
    ));
    assert!(matches!(
        put_identity(
            &db,
            Uuid::new_v4(),
            &identity(0, OrgRelation::NoRelation, None, None)
        )
        .await,
        Err(AssetsError::NotFound)
    ));
    assert!(
        sqlx::query("UPDATE linggan_creation_account_identity SET org_relation='none'")
            .execute(db.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn only_a_confirmed_fact_card_revision_drives_the_checks() {
    let db = db("assets_fact_card").await;
    let (xhs_a, _, _) = accounts(&db).await;
    assert!(list_fact_cards(&db).await.unwrap().is_empty());
    let text = "我报了示例课，也喜欢示例营，还有示例活动";
    let skip = BridgeChoice {
        skip: true,
        ..BridgeChoice::default()
    };
    let mut document = draft(Some(xhs_a), &[text]);
    document.bridge = Some(skip);

    let made = put_fact_card(&db, None, &card(0, "示例营", &["示例营"], &["示例课"]))
        .await
        .unwrap();
    assert_eq!(
        (made.card.code.as_str(), made.card.revision),
        ("CP-0001", 1)
    );
    assert!(made.card.confirmed_at.is_none());
    let before = check_draft(&db, &document).await.unwrap();
    assert!(
        before.not_run.iter().any(|n| n.rule == "R-W3"),
        "unconfirmed cards do not run"
    );
    assert!(!rules(&before).contains(&"R-W3"));
    assert!(before.fact_cards.is_empty());

    let confirmed = confirm_fact_card(
        &db,
        "CP-0001",
        &ConfirmCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        confirmed.card.confirmed_by.as_deref(),
        Some("local_machine")
    );
    let report = check_draft(&db, &document).await.unwrap();
    assert!(rules(&report).contains(&"R-W3") && rules(&report).contains(&"R-I2"));
    assert!(report.not_run.is_empty());
    assert_eq!(
        report.fact_cards,
        vec![CardRevision {
            code: "CP-0001".into(),
            revision: 1
        }]
    );

    // A new revision waits for confirmation while revision 1 stays in force.
    let second = put_fact_card(
        &db,
        Some("CP-0001"),
        &card(1, "示例营", &["示例营"], &["示例课", "示例"]),
    )
    .await
    .unwrap();
    assert_eq!(second.card.revision, 2);
    assert!(second.card.confirmed_at.is_none());
    let views = list_fact_cards(&db).await.unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].latest.revision, 2);
    assert_eq!(views[0].confirmed.as_ref().unwrap().revision, 1);
    let still = check_draft(&db, &document).await.unwrap();
    assert_eq!(still.fact_cards[0].revision, 1);
    assert_eq!(
        still.findings.iter().filter(|f| f.rule == "R-W3").count(),
        1,
        "revision 1 bans only 示例课; revision 2 would also ban a bare 示例"
    );
    confirm_fact_card(
        &db,
        "CP-0001",
        &ConfirmCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 2,
        },
    )
    .await
    .unwrap();
    let now = check_draft(&db, &document).await.unwrap();
    assert_eq!(now.fact_cards[0].revision, 2);
    assert_eq!(
        now.findings.iter().filter(|f| f.rule == "R-W3").count(),
        2,
        "示例课 and the bare 示例 in 示例活动; 示例营 stays allowed whole"
    );

    // Replay, stale, overlap, double confirmation and numbering.
    let again = card(0, "第二张", &[], &[]);
    let one = put_fact_card(&db, None, &again).await.unwrap();
    assert_eq!(one.card.code, "CP-0002");
    assert!(put_fact_card(&db, None, &again).await.unwrap().replayed);
    assert!(matches!(
        put_fact_card(&db, Some("CP-0001"), &card(1, "示例营", &[], &[])).await,
        Err(AssetsError::Stale)
    ));
    assert!(matches!(
        put_fact_card(&db, Some("CP-9999"), &card(1, "x", &[], &[])).await,
        Err(AssetsError::NotFound)
    ));
    assert!(matches!(
        put_fact_card(&db, Some("CP-0001"), &card(0, "示例营", &[], &[])).await,
        Err(AssetsError::Invalid("expected_revision"))
    ));
    assert!(matches!(
        put_fact_card(&db, None, &card(0, "名称", &["甲"], &["甲"])).await,
        Err(AssetsError::Invalid("names_overlap"))
    ));
    assert!(matches!(
        confirm_fact_card(
            &db,
            "CP-0001",
            &ConfirmCommand {
                request_id: Uuid::new_v4(),
                expected_revision: 2
            }
        )
        .await,
        Err(AssetsError::Invalid("already_confirmed"))
    ));
    assert!(
        sqlx::query("DELETE FROM linggan_creation_fact_card_confirmation")
            .execute(db.pool())
            .await
            .is_err()
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn bridge_sentence_library_reviews_and_serves_only_approved_matching_voices() {
    let db = db("assets_sentences").await;
    let (xhs_a, _, xhs_c) = accounts(&db).await;
    put_identity(
        &db,
        xhs_a,
        &identity(0, OrgRelation::Parent, Some(true), Some("妈妈")),
    )
    .await
    .unwrap();
    put_identity(
        &db,
        xhs_c,
        &identity(0, OrgRelation::Coach, Some(false), None),
    )
    .await
    .unwrap();

    // A cure promise cannot enter; nothing is written for the refused command.
    let refused = SentenceCreate {
        request_id: Uuid::new_v4(),
        group: BridgeGroup::Parent,
        text: "我家孩子保证能好".into(),
        origin: SentenceOrigin::Manual,
    };
    match create_sentence(&db, &refused).await {
        Err(AssetsError::Blocked(findings)) => assert_eq!(findings[0].rule, "R-L1"),
        other => panic!("{other:?}"),
    }
    assert!(list_sentences(&db, None, None).await.unwrap().is_empty());

    let create_cmd = SentenceCreate {
        request_id: Uuid::new_v4(),
        group: BridgeGroup::Parent,
        text: "我也是陪跑营学员家长，欢迎交流".into(),
        origin: SentenceOrigin::Manual,
    };
    let made = create_sentence(&db, &create_cmd).await.unwrap();
    let sentence = made.sentence.clone();
    assert_eq!((sentence.code.as_str(), sentence.revision), ("XJ-0001", 1));
    assert_eq!(sentence.status, SentenceStatus::Review);
    assert!(
        sentence
            .entry_findings
            .iter()
            .all(|f| f.level != Level::Must)
    );
    let replay = create_sentence(&db, &create_cmd).await.unwrap();
    assert!(replay.replayed);
    assert_eq!(
        replay.sentence.code, "XJ-0001",
        "a repeat does not consume another code"
    );
    let mut different = create_cmd.clone();
    different.text = "换了一句话".into();
    assert!(matches!(
        create_sentence(&db, &different).await,
        Err(AssetsError::RequestConflict)
    ));

    // Waiting for review means not usable.
    assert!(
        candidates_for_account(&db, xhs_a)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    let review = |expected: i32, action: ReviewAction, note: Option<&str>| SentenceReview {
        request_id: Uuid::new_v4(),
        expected_revision: expected,
        action,
        note: note.map(str::to_string),
    };
    assert!(matches!(
        review_sentence(&db, "XJ-0001", &review(1, ReviewAction::RequestFix, None)).await,
        Err(AssetsError::Invalid("review"))
    ));
    assert!(matches!(
        review_sentence(&db, "XJ-0001", &review(1, ReviewAction::Enable, None)).await,
        Err(AssetsError::Invalid("transition"))
    ));
    let sent_back = review_sentence(
        &db,
        "XJ-0001",
        &review(1, ReviewAction::RequestFix, Some("太像招揽")),
    )
    .await
    .unwrap()
    .sentence;
    assert_eq!(sent_back.status, SentenceStatus::Fix);
    assert_eq!(sent_back.status_note.as_deref(), Some("太像招揽"));
    let ok = review_sentence(&db, "XJ-0001", &review(1, ReviewAction::Approve, None))
        .await
        .unwrap()
        .sentence;
    assert_eq!(ok.status, SentenceStatus::Ok);
    assert!(matches!(
        review_sentence(&db, "XJ-0001", &review(1, ReviewAction::Approve, None)).await,
        Err(AssetsError::Invalid("transition"))
    ));

    let ready = candidates_for_account(&db, xhs_a).await.unwrap();
    assert_eq!(ready.state, BridgeState::Ready);
    assert_eq!(
        ready
            .items
            .iter()
            .map(|s| s.code.as_str())
            .collect::<Vec<_>>(),
        vec!["XJ-0001"]
    );
    let wrong_voice = candidates_for_account(&db, xhs_c).await.unwrap();
    assert_eq!(wrong_voice.state, BridgeState::BioMissing);
    assert!(
        wrong_voice.items.is_empty(),
        "no bridge until the bio says so"
    );
    assert!(
        list_sentences(&db, Some(BridgeGroup::Coach), None)
            .await
            .unwrap()
            .is_empty()
    );

    // Rewriting goes back to review; the old approved revision is no longer served.
    let edit = |expected: i32| SentenceEdit {
        request_id: Uuid::new_v4(),
        expected_revision: expected,
        group: BridgeGroup::Parent,
        text: "我也是陪跑营学员家长，一起慢慢来".into(),
    };
    let rewritten = edit_sentence(&db, "XJ-0001", &edit(1))
        .await
        .unwrap()
        .sentence;
    assert_eq!(
        (rewritten.revision, rewritten.status),
        (2, SentenceStatus::Review)
    );
    assert!(
        candidates_for_account(&db, xhs_a)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    assert!(matches!(
        edit_sentence(&db, "XJ-0001", &edit(1)).await,
        Err(AssetsError::Stale)
    ));
    assert!(matches!(
        review_sentence(&db, "XJ-0001", &review(1, ReviewAction::Approve, None)).await,
        Err(AssetsError::Stale)
    ));
    let off = review_sentence(
        &db,
        "XJ-0001",
        &review(2, ReviewAction::Disable, Some("不用了")),
    )
    .await
    .unwrap()
    .sentence;
    assert_eq!(off.status, SentenceStatus::Off);
    let back = review_sentence(&db, "XJ-0001", &review(2, ReviewAction::Enable, None))
        .await
        .unwrap()
        .sentence;
    assert_eq!(
        back.status,
        SentenceStatus::Review,
        "re-enabling never skips review"
    );
    assert!(matches!(
        edit_sentence(&db, "XJ-9999", &edit(1)).await,
        Err(AssetsError::NotFound)
    ));

    // Usage counts come from the current revision of drafts that carry the sentence.
    let approved_now = approved(&db, BridgeGroup::Parent, "我是学员家长，也在学习").await;
    let mut document = draft(Some(xhs_a), &["正文"]);
    document.bridge = Some(using(&approved_now));
    create(&db, document.clone()).await;
    document.bridge.as_mut().unwrap().skip = true;
    create(&db, document).await;
    let listed = list_sentences(&db, None, None).await.unwrap();
    let used = listed.iter().find(|s| s.code == approved_now.code).unwrap();
    assert_eq!(
        used.used_count, 1,
        "skipped bridges do not count as selected"
    );
    assert_eq!(
        listed
            .iter()
            .find(|s| s.code == "XJ-0001")
            .unwrap()
            .used_count,
        0
    );
    assert_eq!(
        list_sentences(&db, None, Some(SentenceStatus::Ok))
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn copy_checks_the_saved_revision_and_records_receipt_then_confirmation() {
    let db = db("assets_copy").await;
    let (xhs_a, _, _) = accounts(&db).await;
    put_identity(
        &db,
        xhs_a,
        &identity(0, OrgRelation::Parent, Some(true), Some("妈妈")),
    )
    .await
    .unwrap();
    let sentence = approved(&db, BridgeGroup::Parent, "我也是学员家长，慢慢来").await;

    // A bridge cannot turn an otherwise empty note into a copyable body.
    let mut empty = draft(Some(xhs_a), &[" "]);
    empty.bridge = Some(using(&sentence));
    let empty = create(&db, empty).await;
    assert!(matches!(
        prepare_copy(
            &db,
            empty.work_id,
            &CopyCommand {
                request_id: Uuid::new_v4(),
                expected_revision: empty.revision.id
            }
        )
        .await,
        Err(AssetsError::Invalid("body_empty"))
    ));

    // 必改 blocks the copy and writes nothing.
    let mut blocked = draft(Some(xhs_a), &["开头。", "观点。", "收尾。"]);
    blocked.tags = vec!["ADHD".into(), "#adhd".into()];
    let first = create(&db, blocked.clone()).await;
    let ask = |expected: Uuid| CopyCommand {
        request_id: Uuid::new_v4(),
        expected_revision: expected,
    };
    match prepare_copy(&db, first.work_id, &ask(first.revision.id)).await {
        Err(AssetsError::Blocked(findings)) => {
            assert_eq!(findings[0].rule, "R-M4");
            assert_eq!(findings[0].level, Level::Must);
        }
        other => panic!("{other:?}"),
    }
    let receipts: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_creation_copy_receipt")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(receipts, 0);
    assert!(matches!(
        prepare_copy(&db, first.work_id, &ask(Uuid::new_v4())).await,
        Err(AssetsError::Stale)
    ));
    assert!(matches!(
        prepare_copy(&db, Uuid::new_v4(), &ask(Uuid::new_v4())).await,
        Err(AssetsError::NotFound)
    ));

    // Decide the bridge and save: the next revision can be copied.
    let mut fixed = first.revision.document.clone();
    fixed.bridge = Some(using(&sentence));
    fixed.paragraphs[0].text = "开头，会根治拖拉。".into();
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: fixed.clone(),
        },
    )
    .await
    .unwrap();
    let command = ask(saved.revision.id);
    let ready = prepare_copy(&db, first.work_id, &command).await.unwrap();
    assert_eq!(
        ready.text,
        format!(
            "开头，会根治拖拉。\n{FILL}\n观点。\n{FILL}\n我也是学员家长，慢慢来\n{FILL}\n收尾。\n#ADHD"
        )
    );
    assert_eq!(ready.title, "合成标题");
    assert_eq!(
        (ready.warn_count, ready.hint_count),
        (2, 0),
        "cure promise and repeated topic only remind"
    );
    assert!(!ready.replayed);
    let replayed = prepare_copy(&db, first.work_id, &command).await.unwrap();
    assert!(replayed.replayed);
    assert_eq!(
        (replayed.receipt_ref, replayed.text.as_str()),
        (ready.receipt_ref, ready.text.as_str())
    );
    assert_eq!(replayed.warn_count, 2);
    // A receipt made under v1 remains attributed to v1 after the v2 rules are installed.
    let old_request = Uuid::new_v4();
    let fingerprint = linggan_intelligence::content_workbench_sources::text_hash(
        &serde_json::json!({
            "kind": "copy_prepare", "command": [first.work_id, saved.revision.id]
        })
        .to_string(),
    );
    sqlx::query("INSERT INTO linggan_creation_assets_request(request_ref,kind,command_sha256) VALUES($1,'copy_prepare',$2)")
        .bind(old_request).bind(fingerprint).execute(db.pool()).await.unwrap();
    let text_hash = linggan_intelligence::content_workbench_sources::text_hash(&format!(
        "{}\n{}",
        ready.title, ready.text
    ));
    let old_receipt = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_creation_copy_receipt(receipt_ref,work_ref,revision_ref,request_ref,ruleset_version,fact_cards,warn_rules,hint_rules,text_sha256,saved_by) VALUES($1,$2,$3,$4,'creation-checks.v1','[]','{}','{}',$5,'local_machine')")
        .bind(old_receipt).bind(first.work_id).bind(saved.revision.id).bind(old_request).bind(text_hash).execute(db.pool()).await.unwrap();
    let old_replay = prepare_copy(
        &db,
        first.work_id,
        &CopyCommand {
            request_id: old_request,
            expected_revision: saved.revision.id,
        },
    )
    .await
    .unwrap();
    assert!(old_replay.replayed);
    assert_eq!(old_replay.ruleset_version, "creation-checks.v1");
    assert!(matches!(
        prepare_copy(
            &db,
            first.work_id,
            &CopyCommand {
                request_id: command.request_id,
                expected_revision: first.revision.id
            }
        )
        .await,
        Err(AssetsError::RequestConflict)
    ));

    // A receipt is not a copy until the clipboard confirms it.
    assert!(
        copy_status(&db, first.work_id)
            .await
            .unwrap()
            .copied
            .is_none()
    );
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == first.work_id)
        .unwrap();
    assert_eq!(
        summary.copied_revision_id, None,
        "preparing text is not clipboard confirmation"
    );
    let confirm = CopyConfirm {
        request_id: Uuid::new_v4(),
    };
    let mark = confirm_copy(&db, first.work_id, ready.receipt_ref, &confirm)
        .await
        .unwrap();
    assert_eq!(mark.revision_id, saved.revision.id);
    let twice = confirm_copy(
        &db,
        first.work_id,
        ready.receipt_ref,
        &CopyConfirm {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        twice.copied_at, mark.copied_at,
        "a second confirmation returns the first"
    );
    assert!(matches!(
        confirm_copy(
            &db,
            Uuid::new_v4(),
            ready.receipt_ref,
            &CopyConfirm {
                request_id: Uuid::new_v4()
            }
        )
        .await,
        Err(AssetsError::NotFound)
    ));
    let status = copy_status(&db, first.work_id).await.unwrap();
    assert_eq!(status.copied.unwrap().revision_id, saved.revision.id);
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == first.work_id)
        .unwrap();
    assert_eq!(summary.copied_revision_id, Some(saved.revision.id));

    // Editing after the copy leaves the copied mark on the older revision.
    let mut later = fixed;
    later.paragraphs[1].text = "改过的观点。".into();
    let newer = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
            document: later,
        },
    )
    .await
    .unwrap();
    assert_ne!(
        copy_status(&db, first.work_id)
            .await
            .unwrap()
            .copied
            .unwrap()
            .revision_id,
        newer.revision.id
    );
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == first.work_id)
        .unwrap();
    assert_eq!(summary.revision_id, newer.revision.id);
    assert_eq!(summary.copied_revision_id, None);
    let new_copy = prepare_copy(&db, first.work_id, &ask(newer.revision.id))
        .await
        .unwrap();
    confirm_copy(
        &db,
        first.work_id,
        new_copy.receipt_ref,
        &CopyConfirm {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    confirm_copy(
        &db,
        first.work_id,
        old_receipt,
        &CopyConfirm {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let summary = list_works(&db, Some(fixture::dossier::DOMAIN))
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.work_id == first.work_id)
        .unwrap();
    assert_eq!(
        summary.copied_revision_id,
        Some(newer.revision.id),
        "late confirmation of an older version must not hide a confirmed current version"
    );
    assert_eq!(
        copy_status(&db, first.work_id)
            .await
            .unwrap()
            .copied
            .unwrap()
            .revision_id,
        newer.revision.id
    );

    // Skipping the bridge needs no account; an empty body is not copyable.
    let mut plain = draft(None, &["只有正文"]);
    plain.bridge = Some(BridgeChoice {
        skip: true,
        ..BridgeChoice::default()
    });
    let plain = create(&db, plain).await;
    let text = prepare_copy(&db, plain.work_id, &ask(plain.revision.id))
        .await
        .unwrap()
        .text;
    assert_eq!(text, "只有正文");
    let mut empty = draft(None, &["", "  "]);
    empty.bridge = Some(BridgeChoice {
        skip: true,
        ..BridgeChoice::default()
    });
    let empty = create(&db, empty).await;
    assert!(matches!(
        prepare_copy(&db, empty.work_id, &ask(empty.revision.id)).await,
        Err(AssetsError::Invalid("body_empty"))
    ));
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn request_ids_belong_to_one_command_and_every_new_table_is_append_only() {
    let db = db("assets_requests").await;
    let (xhs_a, _, _) = accounts(&db).await;
    let shared = Uuid::new_v4();
    let mut as_identity = identity(0, OrgRelation::NoRelation, None, None);
    as_identity.request_id = shared;
    put_identity(&db, xhs_a, &as_identity).await.unwrap();
    let mut as_card = card(0, "名称", &[], &[]);
    as_card.request_id = shared;
    assert!(matches!(
        put_fact_card(&db, None, &as_card).await,
        Err(AssetsError::RequestConflict)
    ));
    assert!(list_fact_cards(&db).await.unwrap().is_empty());
    let sentence = approved(&db, BridgeGroup::Parent, "一句普通的话").await;
    let first = create(&db, draft(Some(xhs_a), &["正文"])).await;
    let mut skipped = first.revision.document.clone();
    skipped.bridge = Some(BridgeChoice {
        skip: true,
        ..BridgeChoice::default()
    });
    let saved = save(
        &db,
        first.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: first.revision.id,
            document: skipped,
        },
    )
    .await
    .unwrap();
    let ready = prepare_copy(
        &db,
        first.work_id,
        &CopyCommand {
            request_id: Uuid::new_v4(),
            expected_revision: saved.revision.id,
        },
    )
    .await
    .unwrap();
    confirm_copy(
        &db,
        first.work_id,
        ready.receipt_ref,
        &CopyConfirm {
            request_id: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(sentence.status, SentenceStatus::Ok);
    for table in [
        "linggan_creation_assets_request",
        "linggan_creation_account_identity",
        "linggan_creation_bridge_sentence",
        "linggan_creation_bridge_sentence_status",
        "linggan_creation_copy_receipt",
        "linggan_creation_copy_confirmation",
    ] {
        let count: i64 =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(count > 0, "{table} should have been written by this test");
        assert!(
            sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table}")))
                .execute(db.pool())
                .await
                .is_err(),
            "{table} must refuse deletes"
        );
    }
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn structural_template_tracks_source_withdrawal_and_reset() {
    let db = db("assets_structural_template").await;
    let (account, _, _) = accounts(&db).await;
    let person = put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap()
    .identity
    .person_ref
    .unwrap();
    let experience = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "合成标题".into(),
            body: "合成私人经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    confirm_experience(
        &db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = draft(Some(account), &["合成私人经历。"]);
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = create(&db, doc).await;
    let command = CreateTemplate {
        request_id: Uuid::new_v4(),
        work_id: work.work_id,
        expected_revision: work.revision.id,
        steps: vec![
            StructuralStep::Opening,
            StructuralStep::Experience,
            StructuralStep::Reflection,
        ],
    };
    let receipt = create_structural_template(&db, &command).await.unwrap();
    let code = receipt.template.code.clone();
    assert_eq!(receipt.template.status, "review");
    assert!(receipt.template.source_available);
    assert!(
        create_structural_template(&db, &command)
            .await
            .unwrap()
            .replayed
    );
    assert!(matches!(
        save(
            &db,
            work.work_id,
            &SaveRequest {
                request_id: command.request_id,
                expected_revision: work.revision.id,
                document: work.revision.document.clone()
            }
        )
        .await,
        Err(WorkbenchError::RequestConflict)
    ));
    let stored: String =
        sqlx::query_scalar("SELECT steps::text FROM linggan_creation_template_revision")
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert!(!stored.contains("合成"));
    assert!(sqlx::query("INSERT INTO linggan_creation_template_revision(template_ref,revision,steps) SELECT template_ref,2,'[\"合成私人经历。\"]'::jsonb FROM linggan_creation_template WHERE code=$1").bind(&code).execute(db.pool()).await.is_err());
    let review = ReviewTemplate {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        action: TemplateAction::Approve,
    };
    assert_eq!(
        review_structural_template(&db, &code, &review)
            .await
            .unwrap()
            .template
            .status,
        "ok"
    );
    let revise = ReviseTemplate {
        request_id: Uuid::new_v4(),
        expected_revision: 1,
        steps: vec![StructuralStep::Opening, StructuralStep::Action],
    };
    let revised = revise_structural_template(&db, &code, &revise)
        .await
        .unwrap();
    assert_eq!(
        (revised.template.revision, revised.template.status.as_str()),
        (2, "review")
    );
    assert!(
        revise_structural_template(&db, &code, &revise)
            .await
            .unwrap()
            .replayed
    );
    assert!(matches!(
        revise_structural_template(
            &db,
            &code,
            &ReviseTemplate {
                request_id: Uuid::new_v4(),
                ..revise
            }
        )
        .await,
        Err(AssetsError::Stale)
    ));
    let versions = structural_template_history(&db, &code).await.unwrap();
    assert_eq!(
        versions
            .iter()
            .map(|v| (v.revision, v.status.as_str()))
            .collect::<Vec<_>>(),
        vec![(2, "review"), (1, "ok")]
    );
    withdraw_experience(&db, experience, Uuid::new_v4())
        .await
        .unwrap();
    assert!(matches!(
        revise_structural_template(
            &db,
            &code,
            &ReviseTemplate {
                request_id: Uuid::new_v4(),
                expected_revision: 2,
                steps: vec![StructuralStep::Opening]
            }
        )
        .await,
        Err(AssetsError::SourceUnavailable)
    ));
    assert!(
        structural_template_history(&db, &code)
            .await
            .unwrap()
            .iter()
            .all(|v| !v.source_available)
    );
    let listed = list_structural_templates(&db, fixture::dossier::DOMAIN)
        .await
        .unwrap();
    assert!(
        !listed
            .iter()
            .find(|item| item.code == code)
            .unwrap()
            .source_available
    );
    assert!(matches!(
        create_structural_template(
            &db,
            &CreateTemplate {
                request_id: Uuid::new_v4(),
                ..command
            }
        )
        .await,
        Err(AssetsError::SourceUnavailable)
    ));
    let reset = reset_withdrawn(
        &db,
        work.work_id,
        &ResetWithdrawnRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
        },
    )
    .await
    .unwrap();
    let mut fresh = reset.revision.document;
    fresh.paragraphs.push(Paragraph {
        id: Uuid::new_v4(),
        text: "新的独立正文".into(),
    });
    let saved = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: reset.revision.id,
            document: fresh,
        },
    )
    .await
    .unwrap();
    let newer = create_structural_template(
        &db,
        &CreateTemplate {
            request_id: Uuid::new_v4(),
            work_id: work.work_id,
            expected_revision: saved.revision.id,
            steps: vec![StructuralStep::Opening],
        },
    )
    .await
    .unwrap();
    assert!(newer.template.source_available);
    sqlx::query("SELECT setval('linggan_creation_template_code_seq',9999,true)")
        .execute(db.pool())
        .await
        .unwrap();
    let large_code = create_structural_template(
        &db,
        &CreateTemplate {
            request_id: Uuid::new_v4(),
            work_id: work.work_id,
            expected_revision: saved.revision.id,
            steps: vec![StructuralStep::Opening],
        },
    )
    .await
    .unwrap();
    assert_eq!(large_code.template.code, "YB-10000");
    assert!(
        !list_structural_templates(&db, fixture::dossier::DOMAIN)
            .await
            .unwrap()
            .iter()
            .find(|item| item.code == code)
            .unwrap()
            .source_available
    );
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn structural_template_with_detached_history_waits_for_withdrawal() {
    let db = db("assets_template_withdrawal_race").await;
    let (account, _, _) = accounts(&db).await;
    let person = put_identity(
        &db,
        account,
        &identity(0, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap()
    .identity
    .person_ref
    .unwrap();
    let experience = create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: person,
            title: "合成".into(),
            body: "合成经历。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    confirm_experience(
        &db,
        experience,
        &ExperienceConfirm {
            request_id: Uuid::new_v4(),
            expected_revision: 1,
            subject_confirmed: true,
            draft_use: true,
            send_to_model: None,
            confirmation_method: "subject_present".into(),
        },
    )
    .await
    .unwrap();
    let mut doc = draft(Some(account), &["合成经历。"]);
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: doc.paragraphs[0].id,
        experience_ref: experience,
        experience_revision: 1,
    });
    let work = create(&db, doc).await;
    let mut independent = work.revision.document.clone();
    independent.experience_slots.clear();
    independent.paragraphs = vec![Paragraph {
        id: Uuid::new_v4(),
        text: "独立正文".into(),
    }];
    let saved = save(
        &db,
        work.work_id,
        &SaveRequest {
            request_id: Uuid::new_v4(),
            expected_revision: work.revision.id,
            document: independent,
        },
    )
    .await
    .unwrap();
    let template = create_structural_template(
        &db,
        &CreateTemplate {
            request_id: Uuid::new_v4(),
            work_id: work.work_id,
            expected_revision: saved.revision.id,
            steps: vec![StructuralStep::Opening],
        },
    )
    .await
    .unwrap()
    .template;
    let mut hold = db.pool().begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind("creation_assets:experience-gate")
        .execute(&mut *hold)
        .await
        .unwrap();
    let withdrawal_db = db.clone();
    let withdrawal = tokio::spawn(async move {
        withdraw_experience(&withdrawal_db, experience, Uuid::new_v4()).await
    });
    wait_for_advisory_waiters(&db, 1).await;
    let review_db = db.clone();
    let review = tokio::spawn(async move {
        review_structural_template(
            &review_db,
            &template.code,
            &ReviewTemplate {
                request_id: Uuid::new_v4(),
                expected_revision: 1,
                action: TemplateAction::Approve,
            },
        )
        .await
    });
    wait_for_advisory_waiters(&db, 2).await;
    hold.commit().await.unwrap();
    withdrawal.await.unwrap().unwrap();
    assert!(matches!(
        review.await.unwrap(),
        Err(AssetsError::SourceUnavailable)
    ));
}
