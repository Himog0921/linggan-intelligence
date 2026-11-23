use super::{app_with_database, full_schema_fixture, ops_auth_tests};
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::Request,
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::Response,
};
use linggan_intelligence::content_workbench::{self as workbench, WorkDocument};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use tower::ServiceExt;
use uuid::Uuid;

async fn credentials(db: &Database, name: &str, role: &str) -> (Uuid, String) {
    let member =
        ops_auth_tests::active_member(db.pool(), &format!("{name}@example.test"), role).await;
    let cookie = ops_auth_tests::stored_session(
        db.pool(),
        member,
        &Uuid::new_v4().to_string(),
        "0 seconds",
        "1 hour",
    )
    .await;
    (member, cookie)
}

pub(super) async fn admin_app(db: Database) -> Router {
    let (_, cookie) = credentials(&db, "legacy-creation-http-admin", "admin").await;
    let cookie = HeaderValue::from_str(&cookie).unwrap();
    app_with_database(db).layer(middleware::from_fn(
        move |mut request: Request, next: Next| {
            let cookie = cookie.clone();
            async move {
                request
                    .headers_mut()
                    .entry(header::COOKIE)
                    .or_insert(cookie);
                next.run(request).await
            }
        },
    ))
}

async fn request(
    app: &Router,
    method: Method,
    path: &str,
    cookie: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, "127.0.0.1:4185");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    if body.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/json");
    }
    let response: Response = app
        .clone()
        .oneshot(
            request
                .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL; synthetic members, accounts and drafts"]
async fn creation_members_share_reads_and_keep_writes_and_assets_scoped() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &format!("writer_member_{}", Uuid::new_v4().simple()),
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    let domain: Uuid = sqlx::query_scalar("SELECT domain_ref FROM observation_domain WHERE status='active' ORDER BY domain_ref LIMIT 1")
        .fetch_one(db.pool()).await.unwrap();
    let (admin, admin_cookie) = credentials(&db, "admin-creation", "admin").await;
    let (writer, writer_cookie) = credentials(&db, "writer-creation", "operator").await;
    let (_, other_cookie) = credentials(&db, "other-creation", "operator").await;
    let (lead, lead_cookie) = credentials(&db, "lead-creation", "lead").await;
    let (_, other_lead_cookie) = credentials(&db, "other-lead-creation", "lead").await;
    let group = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_ops_group(group_ref,display_name,state) VALUES($1,'合成组','active')",
    )
    .bind(group)
    .execute(db.pool())
    .await
    .unwrap();
    for member in [writer, lead] {
        sqlx::query("INSERT INTO linggan_ops_group_membership_event(event_ref,group_ref,member_ref,action,acted_by_member_ref,reason,request_ref) VALUES($1,$2,$3,'joined',$4,'合成成员测试',$5)")
            .bind(Uuid::new_v4()).bind(group).bind(member).bind(admin).bind(Uuid::new_v4()).execute(db.pool()).await.unwrap();
    }
    let app = app_with_database(db.clone());
    let create_path = "/api/local/ops/creation/works";
    let account = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_ops_account(account_ref,source_system,source_record_id,domain_ref,ledger_state,platform,account_name,profile_link_state,native_record_state,current_responsible_member_ref,native_version,native_created_by_member_ref,native_updated_by_member_ref,status) VALUES($1,'linggan_native',$1::text,$2,'native_active','xhs','成员合成账号','missing','active',$3,1,$3,$3,'active')")
        .bind(account).bind(domain).bind(writer).execute(db.pool()).await.unwrap();
    let document = WorkDocument {
        title: "成员合成稿".into(),
        account_ref: Some(account),
        paragraphs: vec![workbench::Paragraph {
            id: Uuid::new_v4(),
            text: "共享正文".into(),
        }],
        ..Default::default()
    };
    let create = json!({"requestId":Uuid::new_v4(),"domainRef":domain,"document":document});
    assert_eq!(
        request(&app, Method::POST, create_path, None, Some(create.clone()))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &app,
            Method::POST,
            create_path,
            Some(&other_cookie),
            Some(create.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, receipt) = request(
        &app,
        Method::POST,
        create_path,
        Some(&writer_cookie),
        Some(create),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let work: Uuid = serde_json::from_value(receipt["workId"].clone()).unwrap();
    let owner: Option<Uuid> = sqlx::query_scalar(
        "SELECT created_by_member_ref FROM linggan_creation_work WHERE work_ref=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(owner, Some(writer));
    let work_path = format!("{create_path}/{work}");
    let (status, other) = request(&app, Method::GET, &work_path, Some(&other_cookie), None).await;
    assert_eq!(status, StatusCode::OK, "{other}");
    assert_eq!(other["draft"]["paragraphs"][0]["text"], "共享正文");
    assert_eq!(other["draft"]["accountRef"], Value::Null);
    assert_eq!(other["editable"], false);
    let save = json!({"requestId":Uuid::new_v4(),"expectedRevision":receipt["revision"]["id"],"document":document});
    assert_eq!(
        request(
            &app,
            Method::POST,
            &work_path,
            Some(&other_cookie),
            Some(save.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Method::POST,
            &work_path,
            Some(&writer_cookie),
            Some(save)
        )
        .await
        .0,
        StatusCode::OK
    );

    let identity_command = json!({
        "requestId": Uuid::new_v4(), "expectedRevision": 0, "orgRelation": "coach",
        "bioDisclosesRelation": true, "hasRealPerson": true, "backingPersonRole": "合成老师",
        "canProvideExperience": null
    });
    let (status, identity) = request(
        &app,
        Method::PUT,
        &format!("/ops/api/creation/accounts/{account}/identity"),
        Some(&admin_cookie),
        Some(identity_command),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{identity}");
    let identity_actor: Option<Uuid> = sqlx::query_scalar(
        "SELECT saved_by_member_ref FROM linggan_creation_account_identity WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(identity_actor, Some(admin));

    let asset_path = format!("/api/local/ops/creation/accounts/{account}/assets");
    for (cookie, status) in [
        (&writer_cookie, StatusCode::OK),
        (&lead_cookie, StatusCode::OK),
        (&other_lead_cookie, StatusCode::FORBIDDEN),
        (&other_cookie, StatusCode::FORBIDDEN),
        (&admin_cookie, StatusCode::OK),
    ] {
        let (actual, response) = request(&app, Method::GET, &asset_path, Some(cookie), None).await;
        assert_eq!(actual, status);
        if status == StatusCode::OK {
            assert_eq!(response["identity"]["backingPersonRole"], "合成老师");
        } else {
            assert!(!response.to_string().contains("合成老师"));
        }
    }

    let legacy = workbench::create_work(
        &db,
        &workbench::CreateRequest {
            request_id: Uuid::new_v4(),
            domain_ref: domain,
            document: WorkDocument {
                title: "旧稿".into(),
                ..Default::default()
            },
        },
    )
    .await
    .unwrap();
    let (status, item) = request(
        &app,
        Method::GET,
        &format!("{create_path}/{}", legacy.work_id),
        Some(&writer_cookie),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["ownership"], "unknown");
    assert_eq!(item["editable"], false);
    assert_eq!(
        request(
            &app,
            Method::GET,
            "/ops/api/creation/works",
            Some(&writer_cookie),
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Method::GET,
            "/ops/api/creation/works",
            Some(&admin_cookie),
            None
        )
        .await
        .0,
        StatusCode::OK
    );

    sqlx::query("UPDATE linggan_ops_session SET revoked_at=scope_001_now() WHERE member_ref=$1")
        .bind(writer)
        .execute(db.pool())
        .await
        .unwrap();
    assert_eq!(
        request(&app, Method::GET, &work_path, Some(&writer_cookie), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
