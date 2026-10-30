//! Real guarded HTTP composition against synthetic, isolated PostgreSQL.
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_intelligence::content_workbench::{self as core, types::*};
use tower::ServiceExt;
use uuid::Uuid;
#[path = "../../../../crates/evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
#[path = "../../../../crates/intelligence/tests/support/content_workbench_fixture.rs"]
mod work_fixture;
async fn proof(name: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        name,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap()
}
async fn call(
    app: Router,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut b = Request::builder().uri(uri).header("Host", "127.0.0.1:4185");
    if body.is_some() {
        b = b.method("POST").header("Content-Type", "application/json");
    }
    let response = app
        .oneshot(
            b.body(body.map_or_else(Body::empty, |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 3 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!({"rawLength":bytes.len()})),
    )
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_withdrawal_reset_requires_discard() {
    use assets::{ExperienceConfirm, ExperienceCreate, IdentityCommand, OrgRelation};
    use linggan_intelligence::creation_assets as assets;
    let db = proof("creation_http_experience_reset").await;
    let (account, _) = work_fixture::ops_seed(&db).await;
    assets::put_identity(
        &db,
        account,
        &IdentityCommand {
            request_id: Uuid::new_v4(),
            expected_revision: 0,
            org_relation: OrgRelation::Parent,
            bio_discloses_relation: None,
            has_real_person: Some(true),
            backing_person_role: Some("妈妈".into()),
            can_provide_experience: None,
            person_change: None,
        },
    )
    .await
    .unwrap();
    let id = assets::create_experience(
        &db,
        &ExperienceCreate {
            request_id: Uuid::new_v4(),
            account_ref: account,
            expected_person_ref: assets::read_identity(&db, account)
                .await
                .unwrap()
                .unwrap()
                .person_ref
                .unwrap(),
            title: "合成经历".into(),
            body: "合成经历正文".into(),
            tags: vec![],
            origin: "oral".into(),
        },
    )
    .await
    .unwrap()
    .experience
    .id;
    assets::confirm_experience(
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
    let paragraph = Uuid::new_v4();
    let mut doc = WorkDocument::default();
    doc.account_ref = Some(account);
    doc.paragraphs.push(Paragraph {
        id: paragraph,
        text: "合成经历正文".into(),
    });
    doc.experience_slots.push(ExperienceSlot {
        paragraph_id: paragraph,
        experience_ref: id,
        experience_revision: 1,
    });
    let work = core::create_work(
        &db,
        &CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: dossier::DOMAIN,
            document: doc,
        },
    )
    .await
    .unwrap();
    assets::withdraw_experience(&db, id, Uuid::new_v4())
        .await
        .unwrap();
    let app = app_with_database(db);
    let uri = format!("/ops/api/creation/works/{}", work.work_id);
    let (_, read) = call(app.clone(), &uri, None).await;
    let mut edited = read["revision"]["document"].clone();
    edited["title"] = json!("合成改动");
    let (status, failure) = call(app.clone(), &format!("{uri}/save"), Some(json!({"requestId": Uuid::new_v4(), "expectedRevision":work.revision.id,"document":edited.clone()}))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(failure["code"], "creation_experience_reset_required");
    let command = json!({"requestId":Uuid::new_v4(),"expectedRevision":work.revision.id});
    let (status, result) = call(
        app.clone(),
        &format!("{uri}/experience-reset"),
        Some(command.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(result["revision"]["document"]["experienceSlots"].is_null());
    assert!(result["revision"]["document"]["experienceEpoch"].is_string());
    assert!(!result.to_string().contains("合成经历正文"));
    let (status, old_page) = call(app.clone(), &format!("{uri}/save"), Some(json!({"requestId":Uuid::new_v4(), "expectedRevision":result["revision"]["id"], "document":edited}))).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        old_page["code"],
        "creation_experience_editor_reset_required"
    );
    let (_, current) = call(app.clone(), &uri, None).await;
    let (_, history) = call(app.clone(), &format!("{uri}/revisions"), None).await;
    assert!(!current.to_string().contains("合成经历正文"));
    assert!(!history.to_string().contains("合成经历正文"));
    let (status, replay) = call(app, &format!("{uri}/experience-reset"), Some(command)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(replay["replayed"], true);
}
#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_routes_save_conflict_history_and_guard() {
    let db = proof("creation_http").await;
    let app = app_with_database(db);
    let domain = dossier::DOMAIN;
    let request =
        json!({"requestId":Uuid::new_v4(),"domainRef":domain,"document":WorkDocument::default()});
    let (status, first) = call(
        app.clone(),
        "/ops/api/creation/works",
        Some(request.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let id = first["workId"].as_str().unwrap();
    let uri = format!("/ops/api/creation/works/{id}");
    let (status, read) = call(app.clone(), &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["domainRef"], json!(domain));
    let mut doc = read["revision"]["document"].clone();
    doc["title"] = json!("真实HTTP保存🙂");
    let command = json!({"requestId":Uuid::new_v4(),"expectedRevision":read["revision"]["id"],"document":doc});
    let (status, saved) = call(app.clone(), &format!("{uri}/save"), Some(command.clone())).await;
    assert_eq!(status, StatusCode::OK);
    let (status, receipt) = call(
        app.clone(),
        &format!(
            "/ops/api/creation/requests/{}",
            command["requestId"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(receipt["kind"], "work");
    assert_eq!(
        receipt["receipt"]["revision"]["id"],
        saved["revision"]["id"]
    );
    let (status, _) = call(
        app.clone(),
        &format!("/ops/api/creation/requests/{}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, replay) = call(app.clone(), &format!("{uri}/save"), Some(command.clone())).await;
    assert_eq!(replay["replayed"], true);
    assert_eq!(replay["revision"]["id"], saved["revision"]["id"]);
    let mut stale = command;
    stale["requestId"] = json!(Uuid::new_v4());
    let (status, conflict) = call(app.clone(), &format!("{uri}/save"), Some(stale)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        conflict["receipt"]["revision"]["id"],
        saved["revision"]["id"]
    );
    let (status, history) = call(app.clone(), &format!("{uri}/revisions"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history["items"].as_array().unwrap().len(), 2);
    let (status, prior) = call(
        app.clone(),
        &format!(
            "{uri}/revisions/{}",
            first["revision"]["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prior["id"], first["revision"]["id"]);
    assert_eq!(prior["document"]["title"], "");
    let (status, _) = call(
        app.clone(),
        &format!("{uri}/revisions/{}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, list) = call(
        app.clone(),
        &format!("/ops/api/creation/works?domainRef={domain}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    let bad = Request::builder()
        .uri("/ops/api/creation/works")
        .header("Host", "127.0.0.1:4185")
        .header("Origin", "https://evil.example")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        app.oneshot(bad).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_denies_unknown_fields_and_nonexistent_domain() {
    let db = proof("creation_http_contract").await;
    let app = app_with_database(db);
    let document = WorkDocument::default();
    let request =
        json!({"requestId":Uuid::new_v4(),"domainRef":Uuid::new_v4(),"document":document});
    let (status, _) = call(app.clone(), "/ops/api/creation/works", Some(request)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let request = json!({"requestId":Uuid::new_v4(),"domainRef":dossier::DOMAIN,"document":document,"serverOverride":true});
    let (status, _) = call(app, "/ops/api/creation/works", Some(request)).await;
    assert!(status.is_client_error());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_idea_box_and_title_candidates_round_trip() {
    let db = proof("creation_http_idea_box").await;
    let app = app_with_database(db);
    let idea = Uuid::new_v4();
    let mut anchor = WorkDocument::default();
    anchor.title = "独立的笔记标题".into();
    anchor.idea_ref = Some(idea);
    anchor.idea_name = Some("合成选题".into());
    anchor.angle = "这篇想说的合成意图".into();
    anchor.note_title_candidates.push("合成标题候选".into());
    anchor.cover_title_candidates.push("合成封面".into());
    let (status, created) = call(
        app.clone(),
        "/ops/api/creation/works",
        Some(json!({"requestId":Uuid::new_v4(),"domainRef":dossier::DOMAIN,"document":anchor})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(created["revision"]["document"]["title"], "独立的笔记标题");
    assert_eq!(
        created["revision"]["document"]["noteTitleCandidates"][0],
        "合成标题候选"
    );
    let (status, boxes) = call(
        app.clone(),
        &format!("/ops/api/creation/idea-boxes?domainRef={}", dossier::DOMAIN),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(boxes["items"][0]["ideaRef"], idea.to_string());
    assert_eq!(boxes["items"][0]["name"], "合成选题");
    assert_eq!(boxes["items"][0]["intent"], "这篇想说的合成意图");
    let mut other = WorkDocument::default();
    other.title = "另一个笔记".into();
    other.idea_ref = Some(idea);
    let (status, linked) = call(
        app.clone(),
        "/ops/api/creation/works",
        Some(json!({"requestId":Uuid::new_v4(),"domainRef":dossier::DOMAIN,"document":other})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(linked["revision"]["document"]["ideaRef"], idea.to_string());
    let (_, works) = call(
        app,
        &format!("/ops/api/creation/works?domainRef={}", dossier::DOMAIN),
        None,
    )
    .await;
    assert_eq!(
        works["items"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["ideaRef"] == idea.to_string())
            .count(),
        2
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof; no shared database"]
async fn content_workbench_http_qualifies_new_page_source_and_rejects_withdrawal() {
    let db = proof("creation_http_page_source").await;
    let doc = work_fixture::comment_doc(&db).await;
    let citation = &doc.excerpts[0].citation;
    let command = json!({"domainRef":dossier::DOMAIN,"asOf":citation.as_of,"blockRef":citation.block_ref,
        "sourceKind":citation.source_kind,"sourceRef":citation.source_ref,"workRef":citation.work_ref,
        "field":citation.field,"sourceVersion":citation.source_version,"start":citation.start,"end":citation.end});
    let app = app_with_database(db.clone());
    let (status, answer) = call(
        app.clone(),
        "/ops/api/creation/sources/validate",
        Some(command.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["textHash"], citation.text_hash);
    work_fixture::restrict_comment(&db, citation.work_ref.unwrap()).await;
    let (status, _) = call(app, "/ops/api/creation/sources/validate", Some(command)).await;
    assert_ne!(status, StatusCode::OK);
}
