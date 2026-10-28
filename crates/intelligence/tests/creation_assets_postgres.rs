//! CREATION-ASSETS-001 S1a isolated PostgreSQL proof. Synthetic data, no external calls.
#![recursion_limit = "256"]
#[path = "support/content_workbench_fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use linggan_intelligence::content_workbench::{
    BridgeChoice, BridgeGroup, BridgeUse, CreateRequest, ExperienceSlot, FreezeRequest, PageKind,
    Paragraph, PlaceRequest, SaveRequest, UseTarget, WorkDocument, WorkbenchError, create_work,
    freeze_export, place, read_request, read_work, revisions, save,
};
use linggan_intelligence::creation_assets::*;
use linggan_storage_postgres::Database;
use uuid::Uuid;

const FILL: &str = "\u{3164}";

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
            title: "虚构".into(),
            body: "合成经历，只供测试。".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap();
    let id = made.experience.id;
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
    assert!(prepare_copy(&db, made.work_id, &copy).await.is_ok());
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
        &identity(2, OrgRelation::Parent, None, Some("妈妈")),
    )
    .await
    .unwrap();
    withdraw_experience(&db, id, Uuid::new_v4()).await.unwrap();
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
}

#[tokio::test]
#[ignore = "random isolated PostgreSQL proof; no shared database"]
async fn experience_payload_is_erasable_and_confirmation_is_explicit() {
    let db = db("assets_experience").await;
    let (account, _, _) = accounts(&db).await;
    let cmd = ExperienceCreate {
        request_id: Uuid::new_v4(),
        account_ref: account,
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
        &identity(1, OrgRelation::Parent, None, Some("妈妈")),
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
        &identity(2, OrgRelation::Parent, None, Some("爸爸")),
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
    restrict_comment(&db, source_work).await;
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
    }
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
        &identity(2, OrgRelation::Parent, Some(true), Some("奶奶")),
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
    sqlx::query("INSERT INTO linggan_creation_copy_receipt(receipt_ref,work_ref,revision_ref,request_ref,ruleset_version,fact_cards,warn_rules,hint_rules,text_sha256,saved_by) VALUES($1,$2,$3,$4,'creation-checks.v1','[]','{}','{}',$5,'local_machine')")
        .bind(Uuid::new_v4()).bind(first.work_id).bind(saved.revision.id).bind(old_request).bind(text_hash).execute(db.pool()).await.unwrap();
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
