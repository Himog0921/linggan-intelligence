//! 创作者主页浅看与精选研究（迁移 0180）在真实 Axum 路由 + 隔离 PostgreSQL + 合成工位上的证明。
//! 全部默认忽略，由 `scripts/test-web-app-postgres.sh` 运行；只用合成作者与合成交卷，不访问平台、不调用模型。
use super::creators_browser_tests::{count, forbidden_side_effects, get_json, isolated, seed_work};
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod material_fixture;

// ── 创作者主页浅看与精选研究（迁移 0180）的真实库证明 ─────────────────────────────────
// 浅看只到发现为止：不自动深采、不登记长期观察目标；精选研究只认已冻结样本里的作品。
// 工位交卷全部是合成的；不访问平台、不调用模型。

/// 派发只在 governance 阶段放行，与常驻服务一致。运行脚本在启动本模块时声明它；
/// 没声明就明确失败，而不是悄悄少跑一半。
fn require_governance_phase() {
    assert_eq!(
        std::env::var("LINGGAN_COLLECTION_UPGRADE_PHASE").as_deref(),
        Ok("governance"),
        "dispatch proofs run in the governance phase; scripts/test-web-app-postgres.sh declares it"
    );
}

async fn post_json(db: &Database, uri: &str, body: Value) -> (StatusCode, Value) {
    let response = app_with_database(db.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header(header::HOST, "127.0.0.1:3000")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn error_code(body: &Value) -> &str {
    body["code"].as_str().unwrap_or("")
}

/// 一个合成工位：具备作者资料、主页卡片、详情、评论等能力，绑定了一个合成账号。
async fn proof_station(db: &Database) -> (Uuid, String) {
    let install = Uuid::new_v4();
    let station = Uuid::new_v4();
    sqlx::query("INSERT INTO execution_station(station_ref,display_name,daily_work_quota) VALUES($1,'合成测试工位',10000)")
        .bind(station)
        .execute(db.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO plugin_installation(installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version,capabilities) VALUES($1,$2,$3,'person',scope_001_now(),'0.8.61','[\"discovery_search\",\"author_profile\",\"profile_discovery\",\"content_detail\",\"comments\",\"replies\",\"media_slots\",\"xhs.pageAccess\",\"collection.single_run.v1\",\"collection.all_public.v1\"]'::jsonb)")
        .bind(install)
        .bind(install.to_string())
        .bind(station)
        .execute(db.pool())
        .await
        .unwrap();
    linggan_evidence::set_station_accepting(db, station, true, "person")
        .await
        .unwrap();
    let grant = linggan_evidence::rotate_installation_credential(db, install)
        .await
        .unwrap();
    let credential = grant.raw_credential.expose_once().to_owned();
    linggan_evidence::activate_installation_credential(
        db,
        install,
        grant.credential_ref,
        &credential,
    )
    .await
    .unwrap();
    let account = linggan_evidence::report_account_eligibility(
        db,
        install,
        &credential,
        linggan_evidence::AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "creator-preview-synthetic-account",
        },
        Some(b"creator-preview-fixture-digest-32-bytes"),
    )
    .await
    .unwrap();
    linggan_evidence::bind_observation_account(db, account.account_ref.unwrap(), install, "person")
        .await
        .unwrap();
    (install, credential)
}

/// 工位按领到的任务交一份合成包；返回前已被接纳。
async fn deliver(
    db: &Database,
    install: Uuid,
    spec: &Value,
    kind: &str,
    records: Vec<Value>,
    target: Value,
) {
    let attempt = Uuid::new_v4();
    let count = records.len();
    let command = linggan_contracts::parse_producer_attempt(
        &json!({"contractVersion":"linggan.producer.attempt.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install}).to_string(),
    )
    .unwrap();
    linggan_evidence::start_producer_attempt(db, &command)
        .await
        .unwrap();
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":Uuid::new_v4(),"packageKind":kind,"platform":"xhs","observedAt":"2026-10-13T09:00:00Z","capturedAt":"2026-10-13T09:00:01Z","coverage":{"target":target,"layers":[{"capability":kind,"observed":count,"attempted":count,"acquired":count,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"surface_ended"}]},"records":records});
    let submission = linggan_contracts::parse_producer_submission(
        &json!({"contractVersion":"linggan.producer.capture-package.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install,"submissionId":Uuid::new_v4(),"capturePackage":package}).to_string(),
    )
    .unwrap();
    linggan_evidence::submit_producer_package(db, &submission)
        .await
        .unwrap();
}

/// 与 `deliver` 相同的合成交卷，但走插件真正使用的两个 HTTP 入口（开始尝试、提交包），返回提交的响应。
/// 目标观察登记的守卫在路由层，只有走这里才会被覆盖。
async fn deliver_via_routes(
    db: &Database,
    install: Uuid,
    spec: &Value,
    kind: &str,
    records: Vec<Value>,
    target: Value,
) -> Value {
    let attempt = Uuid::new_v4();
    let count = records.len();
    let (status, started) = post_json(
        db,
        "/api/local/producer/runtime-attempts",
        json!({"contractVersion":"linggan.producer.attempt.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let package = json!({"contractVersion":"linggan.producer.capture-package.v1","packageRef":Uuid::new_v4(),"packageKind":kind,"platform":"xhs","observedAt":"2026-10-13T09:00:00Z","capturedAt":"2026-10-13T09:00:01Z","coverage":{"target":target,"layers":[{"capability":kind,"observed":count,"attempted":count,"acquired":count,"verified":0,"failed":0,"notAttempted":0,"unknown":0,"stoppedReason":"surface_ended"}]},"records":records});
    let (status, response) = post_json(
        db,
        "/api/local/producer/runtime-submissions",
        json!({"contractVersion":"linggan.producer.capture-package.v1","taskId":spec["taskId"],"attemptId":attempt,"producerInstanceId":install,"submissionId":Uuid::new_v4(),"capturePackage":package}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{response}");
    response
}

/// 工位领下一个任务；没有可领的就是 None。
async fn next_task(db: &Database, install: Uuid, credential: &str) -> Option<Value> {
    match linggan_evidence::decide_dispatch(db, &install.to_string(), credential)
        .await
        .unwrap()
    {
        linggan_evidence::DispatchDecision::Dispatch { task_spec, .. } => {
            eprintln!(
                "dispatched {} target={}",
                task_spec["capabilitiesRequested"], task_spec["target"]
            );
            Some(task_spec)
        }
        other => {
            eprintln!("nothing to dispatch: {other:?}");
            None
        }
    }
}

async fn run_phase(db: &Database, run: Uuid) -> String {
    sqlx::query_scalar("SELECT phase FROM collection_run WHERE run_ref=$1")
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

/// 页面「精选研究」发出的请求体（与 creatorHomepageApi.ts 的 studyCreatorWorks 同形）。
fn selected_study(key: Uuid, domain: Uuid, preview: Uuid, works: &[Uuid], mode: &str) -> Value {
    json!({
        "idempotencyKey": key,
        "domainRef": domain,
        "source": {"kind": "creator_selected_works", "previewRef": preview, "workRefs": works},
        "ranking": null,
        "workLimit": works.len(),
        "commentMode": mode,
        "commentTarget": null,
    })
}

/// 入口规则：每一条越界请求都有明确的原因码，被拒的请求不留下任何运行、工单或模型调用。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn creator_preview_and_selected_study_reject_every_out_of_scope_request() {
    let db = isolated("preview_rules").await;
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    seed_work(
        &db,
        domain,
        "rules-known-1",
        "rules-author",
        "规则作者",
        "规则作品",
        80,
    )
    .await;
    let creator = |platform: &str, id: &str| {
        format!(
            "/api/local/creators/{}/preview",
            linggan_contracts::creator_discovery::creator_key(platform, id)
        )
    };

    // 作者入口：看不懂的标识、别的平台、领域里没有发现过的作者、不存在的领域，各有各的原因。
    for (uri, domain, status, code) in [
        (
            "/api/local/creators/not-a-key/preview".to_owned(),
            domain,
            StatusCode::BAD_REQUEST,
            "invalid_creator_key",
        ),
        (
            creator("douyin", "rules-author"),
            domain,
            StatusCode::BAD_REQUEST,
            "invalid_creator_key",
        ),
        (
            creator("xhs", "ghost-author"),
            domain,
            StatusCode::UNPROCESSABLE_ENTITY,
            "creator_not_discovered_in_domain",
        ),
        (
            creator("xhs", "rules-author"),
            Uuid::new_v4(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "domain_not_active",
        ),
    ] {
        let (got, body) = post_json(
            &db,
            &uri,
            json!({"domain": domain, "commandRef": Uuid::new_v4()}),
        )
        .await;
        assert_eq!(got, status, "{uri}");
        assert_eq!(error_code(&body), code, "{uri}");
    }
    // 页面不能自己放大范围：多带一个字段（如 workLimit）整条请求被拒。
    let (status, _) = post_json(
        &db,
        &creator("xhs", "rules-author"),
        json!({"domain": domain, "commandRef": Uuid::new_v4(), "workLimit": 200}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // 采集入口：浅看固定 30 篇、不采评论；作者标识必须是单个可读标识。
    let preview = |limit: i64, mode: &str, author: &str| {
        json!({
            "idempotencyKey": Uuid::new_v4(),
            "domainRef": domain,
            "source": {"kind": "creator_preview", "authorExternalId": author},
            "ranking": null,
            "workLimit": limit,
            "commentMode": mode,
            "commentTarget": null,
        })
    };
    let too_long = "x".repeat(257);
    for (body, code) in [
        (
            preview(29, "none", "rules-author"),
            "creator_preview_scope_invalid",
        ),
        (
            preview(30, "sample_30", "rules-author"),
            "creator_preview_scope_invalid",
        ),
        (
            preview(30, "none", "   "),
            "invalid_creator_preview_identity",
        ),
        (
            preview(30, "none", "bad\u{7}id"),
            "invalid_creator_preview_identity",
        ),
        (
            preview(30, "none", &too_long),
            "invalid_creator_preview_identity",
        ),
    ] {
        let (status, body) = post_json(&db, "/api/collection/runs", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}");
        assert_eq!(error_code(&body), code);
    }

    // 精选研究：没有这个浅看、空选择、重复、超过 30 篇、篇数与请求不符，都被拒。
    let fresh = |n: usize| (0..n).map(|_| Uuid::new_v4()).collect::<Vec<_>>();
    let ghost = Uuid::new_v4();
    let one = fresh(1);
    let mut mismatch = selected_study(Uuid::new_v4(), domain, ghost, &one, "none");
    mismatch["workLimit"] = json!(2);
    for (body, code) in [
        (
            selected_study(Uuid::new_v4(), domain, ghost, &one, "none"),
            "invalid_creator_preview_selection",
        ),
        (
            selected_study(Uuid::new_v4(), domain, ghost, &[], "none"),
            "invalid_work_limit",
        ),
        (
            selected_study(Uuid::new_v4(), domain, ghost, &[one[0], one[0]], "none"),
            "invalid_creator_selected_works",
        ),
        (
            selected_study(Uuid::new_v4(), domain, ghost, &fresh(31), "none"),
            "invalid_creator_selected_works",
        ),
        (mismatch, "invalid_creator_selected_works"),
    ] {
        let (status, body) = post_json(&db, "/api/collection/runs", body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}");
        assert_eq!(error_code(&body), code);
    }

    assert_eq!(
        count(&db, "SELECT count(*) FROM collection_run").await,
        0,
        "被拒的请求不留下任何运行"
    );
    assert_eq!(forbidden_side_effects(&db).await, (0, 0, 0));
}

/// 全链路：开始浅看 → 工位交卷 → 冻结 30 篇以内的主页样本 → 只到发现为止 → 精选研究只认样本里的作品，
/// 文字版不取媒体与评论，标准版仍然继续评论采集；浅看与精选运行的材料用途被标成创作者语境。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn creator_homepage_preview_ends_at_discovery_and_selected_study_takes_only_frozen_cards() {
    require_governance_phase();
    let db = isolated("preview_flow").await;
    let create_domain = |name: &'static str| {
        let db = db.clone();
        async move {
            linggan_evidence::observation_domain::create_observation_domain_with_details(
                &db, name, None, None,
            )
            .await
            .unwrap()
            .domain_ref
        }
    };
    let domain = create_domain("合成领域").await;
    let other_domain = create_domain("另一个合成领域").await;
    let author = "preview-author";
    seed_work(
        &db,
        domain,
        "preview-known-1",
        author,
        "预览作者",
        "已知作品",
        120,
    )
    .await;
    let outsider = seed_work(
        &db,
        domain,
        "preview-other-1",
        "other-author",
        "其他作者",
        "其他作品",
        5,
    )
    .await;
    let uri = format!(
        "/api/local/creators/{}/preview",
        linggan_contracts::creator_discovery::creator_key("xhs", author)
    );
    let read = format!("{uri}?domain={domain}");
    let (install, credential) = proof_station(&db).await;
    assert_eq!(forbidden_side_effects(&db).await, (0, 0, 0));

    // 还没展开过：读到「尚未展开」，不是错误。
    let (status, empty) = get_json(&db, &read).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["state"], "not_requested");
    assert_eq!(empty["runRef"], Value::Null);
    assert_eq!(empty["requestedLimit"], 30);
    assert_eq!(empty["works"], json!([]));

    // 开始浅看：同一个请求标识重放得到同一个运行；换成别的作者是冲突，不静默改写。
    let command = Uuid::new_v4();
    let (status, started) =
        post_json(&db, &uri, json!({"domain": domain, "commandRef": command})).await;
    assert_eq!(status, StatusCode::OK, "{started}");
    assert_eq!(started["replayed"], false);
    assert_eq!(started["domainRef"], json!(domain));
    let run: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
    let (_, replay) = post_json(&db, &uri, json!({"domain": domain, "commandRef": command})).await;
    assert_eq!(
        (replay["replayed"].clone(), replay["runRef"].clone()),
        (json!(true), json!(run))
    );
    let other_uri = format!(
        "/api/local/creators/{}/preview",
        linggan_contracts::creator_discovery::creator_key("xhs", "other-author")
    );
    let (status, conflict) = post_json(
        &db,
        &other_uri,
        json!({"domain": domain, "commandRef": command}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error_code(&conflict), "creator_preview_request_conflict");
    let (kind, limit, mode, phase): (String, i32, String, String) = sqlx::query_as(
        "SELECT source_kind,work_limit,comment_mode,phase FROM collection_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), limit, mode.as_str(), phase.as_str()),
        ("creator_preview", 30, "none", "discovery")
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM collection_run").await,
        1,
        "重放与冲突都没有多建运行"
    );
    let (_, running) = get_json(&db, &read).await;
    assert_eq!(running["state"], "running");
    assert_eq!(running["runRef"], json!(run));
    assert_eq!(running["works"], json!([]));

    // 工位交卷：作者资料 + 4 张主页卡片（第 2 张是库里已有的作品）。只会被问到作者资料和主页卡片。
    let cards: Vec<Value> = [
        (1, "preview-card-1"),
        (2, "preview-known-1"),
        (3, "preview-card-3"),
        (4, "preview-card-4"),
    ]
    .iter()
    .map(|(position, external)| {
        json!({
            "kind": "profile_discovery_card",
            "resultPosition": position,
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": external},
            "payload": {
                "title": format!("主页卡片 {position}"),
                "url": format!("https://www.xiaohongshu.com/explore/{external}?xsec_token=proof{position}")
            }
        })
    })
    .collect();
    let mut asked = Vec::new();
    for _ in 0..6 {
        let Some(spec) = next_task(&db, install, &credential).await else {
            break;
        };
        let kind = spec["capabilitiesRequested"][0]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(spec["target"]["authorExternalId"], author);
        let target = json!({"basis": "known_set", "authorExternalId": author});
        match kind.as_str() {
            "author_profile" => {
                let profile = json!({"kind": "author_profile", "sourceObject": {"platform": "xhs", "type": "author", "externalId": author}, "payload": {"userId": author, "nickname": "预览作者", "fans": 17}});
                // 作者资料走插件真正使用的提交入口：浅看的作者资料不得悄悄登记成长期观察目标。
                let response =
                    deliver_via_routes(&db, install, &spec, &kind, vec![profile], target).await;
                assert_eq!(
                    response["targetSync"],
                    json!({"state": "notApplicable", "reason": "creator_preview_is_not_observation"}),
                    "{response}"
                );
            }
            "profile_discovery" => {
                deliver(&db, install, &spec, &kind, cards.clone(), target).await;
            }
            other => panic!("浅看只该要作者资料和主页卡片，却被要求 {other}"),
        }
        asked.push(kind);
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
    }
    assert!(
        asked.iter().any(|kind| kind == "profile_discovery"),
        "{asked:?}"
    );
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();

    // 只到发现为止：运行已结束、范围已冻结；没有任何详情/评论/媒体阶段，也没有后续任务可领。
    let (phase, frozen, ended, reason): (String, bool, bool, Option<String>) = sqlx::query_as(
        "SELECT phase,scope_frozen_at IS NOT NULL,ended_at IS NOT NULL,discovery_reason FROM collection_run WHERE run_ref=$1",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!((phase.as_str(), frozen, ended), ("ended", true, true));
    assert_eq!(reason.as_deref(), Some("surface_ended"));
    let deeper: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_run_order WHERE run_ref=$1 AND stage<>'discovery'",
    )
    .bind(run)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(deeper, 0, "浅看不能自动深采");
    assert!(next_task(&db, install, &credential).await.is_none());
    let snapshot: Vec<(i32, String)> = sqlx::query_as(
        "SELECT w.ordinal,c.content_external_id FROM collection_run_work w JOIN linggan_material_content c ON c.public_ref=w.content_public_ref WHERE w.run_ref=$1 ORDER BY w.ordinal",
    )
    .bind(run)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        snapshot,
        vec![
            (1, "preview-card-1".to_owned()),
            (2, "preview-known-1".to_owned()),
            (3, "preview-card-3".to_owned()),
            (4, "preview-card-4".to_owned()),
        ]
    );
    // 浅看不是长期观察：没有登记任何长期目标，也没有把作者挂进领域的观察名单。
    let durable: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_observation_target WHERE identity_key=$1 AND purpose_kind IS DISTINCT FROM 'single_run'")
        .bind(author)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(durable, 0);
    let monitored: i64 = sqlx::query_scalar("SELECT count(*) FROM observation_domain_target relation JOIN collection_observation_target target USING(target_ref) WHERE relation.domain_ref=$1 AND target.purpose_kind IS DISTINCT FROM 'single_run'")
        .bind(domain)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(monitored, 0);

    // 页面读到的就是冻结样本：4 篇、按序、已知作品带着它的点赞数，卡片没有点赞数就是未知。
    let (status, done) = get_json(&db, &read).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(done["state"], "complete");
    assert_eq!(done["runRef"], json!(run));
    assert_eq!(done["requestedLimit"], 30);
    assert_eq!(done["stopReason"], "surface_ended");
    assert!(done["scopeFrozenAt"].is_string());
    let works = done["works"].as_array().unwrap();
    assert_eq!(works.len(), 4, "{done}");
    assert_eq!(
        works
            .iter()
            .map(|w| w["ordinal"].clone())
            .collect::<Vec<_>>(),
        vec![json!(1), json!(2), json!(3), json!(4)]
    );
    assert_eq!(works[1]["likes"], 120);
    assert_eq!(works[0]["likes"], Value::Null);
    // 「本领域已是研究材料」由服务端逐张标出：只有库里原有的那篇是，新发现的卡片不是。
    assert_eq!(
        works
            .iter()
            .map(|w| w["inDomain"].clone())
            .collect::<Vec<_>>(),
        vec![json!(false), json!(true), json!(false), json!(false)]
    );
    assert_eq!(done["frozenCount"], 4);
    // 领域隔离：别的领域读不到这份浅看，也读不到它的卡片。
    let (status, foreign) = get_json(&db, &format!("{uri}?domain={other_domain}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(foreign["state"], "not_requested");
    assert_eq!(foreign["works"], json!([]));
    let frozen_refs: Vec<Uuid> = works
        .iter()
        .map(|w| serde_json::from_value(w["workRef"].clone()).unwrap())
        .collect();

    // 精选研究只认样本里的作品：样本外、重复、篇数不符、没冻结的浅看、别的领域，都被拒。
    let studies = "/api/collection/runs";
    let mut mismatch = selected_study(Uuid::new_v4(), domain, run, &frozen_refs[..2], "none");
    mismatch["workLimit"] = json!(3);
    let (unfrozen_status, unfrozen) = post_json(
        &db,
        &other_uri,
        json!({"domain": domain, "commandRef": Uuid::new_v4()}),
    )
    .await;
    assert_eq!(unfrozen_status, StatusCode::OK, "{unfrozen}");
    let unfrozen_run: Uuid = serde_json::from_value(unfrozen["runRef"].clone()).unwrap();
    for (body, code) in [
        (
            selected_study(
                Uuid::new_v4(),
                domain,
                run,
                &[frozen_refs[0], outsider],
                "none",
            ),
            "invalid_creator_preview_selection",
        ),
        (
            selected_study(
                Uuid::new_v4(),
                domain,
                run,
                &[frozen_refs[0], frozen_refs[0]],
                "none",
            ),
            "invalid_creator_selected_works",
        ),
        (mismatch, "invalid_creator_selected_works"),
        (
            selected_study(
                Uuid::new_v4(),
                domain,
                unfrozen_run,
                &frozen_refs[..1],
                "none",
            ),
            "invalid_creator_preview_selection",
        ),
        (
            selected_study(Uuid::new_v4(), other_domain, run, &frozen_refs[..1], "none"),
            "invalid_creator_preview_selection",
        ),
    ] {
        let (status, body) = post_json(&db, studies, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}");
        assert_eq!(error_code(&body), code);
    }
    assert_eq!(
        count(
            &db,
            "SELECT count(*) FROM collection_run WHERE source_kind='creator_selected_works'"
        )
        .await,
        0,
        "被拒的精选请求不留下运行"
    );
    // 没冻结的那份浅看只是拿来做反例：用页面「停止这次浅看」同一条命令停掉它，读到「已停止」，
    // 它的发现任务之后不再派发。
    let (status, stopped) = post_json(
        &db,
        &format!("/api/collection/runs/{unfrozen_run}/commands"),
        json!({"domainRef": domain, "commandRef": Uuid::new_v4(), "expectedRevision": 1, "action": "stop"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stopped}");
    let (_, stopped_read) = get_json(&db, &format!("{other_uri}?domain={domain}")).await;
    assert_eq!(stopped_read["state"], "stopped");
    assert_eq!(stopped_read["runRef"], json!(unfrozen_run));

    // 文字版精选（commentMode none）：只取笔记文字，不取媒体与评论。第 1、3 张卡片没有详情，各要一次详情任务。
    let key_a = Uuid::new_v4();
    let pair = [frozen_refs[0], frozen_refs[2]];
    let (status, receipt) = post_json(
        &db,
        studies,
        selected_study(key_a, domain, run, &pair, "none"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["replayed"], false);
    let study_a: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let (kind, limit, mode, phase, reason): (String, i32, String, String, Option<String>) =
        sqlx::query_as("SELECT source_kind,work_limit,comment_mode,phase,discovery_reason FROM collection_run WHERE run_ref=$1")
            .bind(study_a)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(
        (
            kind.as_str(),
            limit,
            mode.as_str(),
            phase.as_str(),
            reason.as_deref()
        ),
        (
            "creator_selected_works",
            2,
            "none",
            "materials",
            Some("person_selected_exact_set")
        )
    );
    let chosen: Vec<Uuid> = sqlx::query_scalar(
        "SELECT content_public_ref FROM collection_run_work WHERE run_ref=$1 ORDER BY ordinal",
    )
    .bind(study_a)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(chosen, pair.to_vec(), "精选运行的作品恰好是所选的那几篇");
    let (_, again) = post_json(
        &db,
        studies,
        selected_study(key_a, domain, run, &pair, "none"),
    )
    .await;
    assert_eq!(
        (again["replayed"].clone(), again["runRef"].clone()),
        (json!(true), json!(study_a))
    );
    let (status, clash) = post_json(
        &db,
        studies,
        selected_study(key_a, domain, run, &frozen_refs[..1], "none"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error_code(&clash), "collection_run_conflict");
    let detail_record = |note: &str| {
        json!({
            "kind": "content_detail",
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": note},
            "payload": {"noteId": note, "title": format!("详情 {note}"), "bodyText": "合成正文：先做两分钟。", "authorId": author, "authorName": "预览作者", "likedCount": 9}
        })
    };
    let mut asked = Vec::new();
    for _ in 0..8 {
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
        let Some(spec) = next_task(&db, install, &credential).await else {
            break;
        };
        let kind = spec["capabilitiesRequested"][0]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(
            spec["collectionRunRef"],
            json!(study_a),
            "领到的任务属于这次精选运行"
        );
        assert_eq!(
            kind, "content_detail",
            "文字版只取笔记文字，却被要求 {kind}"
        );
        let note = spec["target"]["contentExternalId"]
            .as_str()
            .unwrap()
            .to_owned();
        deliver(
            &db,
            install,
            &spec,
            &kind,
            vec![detail_record(&note)],
            json!({"basis": "known_set", "contentExternalId": note}),
        )
        .await;
        asked.push(note);
    }
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    asked.sort();
    assert_eq!(asked, vec!["preview-card-1", "preview-card-3"]);
    assert_eq!(
        run_phase(&db, study_a).await,
        "ended",
        "文字版取完详情就结束"
    );

    // 文字版里已有详情的作品（第 2 篇是库里已有的）不再重取；其余照常只取文字。
    let (_, receipt) = post_json(
        &db,
        studies,
        selected_study(
            Uuid::new_v4(),
            domain,
            run,
            &[frozen_refs[1], frozen_refs[3]],
            "none",
        ),
    )
    .await;
    let study_b: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let mut asked = Vec::new();
    for _ in 0..8 {
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
        let Some(spec) = next_task(&db, install, &credential).await else {
            break;
        };
        let kind = spec["capabilitiesRequested"][0]
            .as_str()
            .unwrap()
            .to_owned();
        assert_eq!(kind, "content_detail");
        let note = spec["target"]["contentExternalId"]
            .as_str()
            .unwrap()
            .to_owned();
        deliver(
            &db,
            install,
            &spec,
            &kind,
            vec![detail_record(&note)],
            json!({"basis": "known_set", "contentExternalId": note}),
        )
        .await;
        asked.push(note);
    }
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(asked, vec!["preview-card-4"], "已有详情的作品不重复取");
    assert_eq!(run_phase(&db, study_b).await, "ended");

    // 标准版精选（commentMode sample_30）：第一个任务同样是取详情，但详情交完之后还会继续排媒体或评论任务——
    // 文字版到详情为止，这是两种预设真正的区别。
    let (_, receipt) = post_json(
        &db,
        studies,
        selected_study(Uuid::new_v4(), domain, run, &frozen_refs[1..2], "sample_30"),
    )
    .await;
    let study_c: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let spec = next_task(&db, install, &credential)
        .await
        .expect("标准版精选应先派发详情任务");
    assert_eq!(spec["capabilitiesRequested"][0], "content_detail");
    assert_eq!(spec["collectionRunRef"], json!(study_c));
    deliver(
        &db,
        install,
        &spec,
        "content_detail",
        vec![detail_record("preview-known-1")],
        json!({"basis": "known_set", "contentExternalId": "preview-known-1"}),
    )
    .await;
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    let follow = next_task(&db, install, &credential)
        .await
        .expect("标准版详情之后还应继续派发媒体或评论任务");
    assert_eq!(follow["collectionRunRef"], json!(study_c));
    let follow_kind = follow["capabilitiesRequested"][0]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(
        ["media_slots", "comments"].contains(&follow_kind.as_str()),
        "标准版详情之后应是媒体或评论，却是 {follow_kind}"
    );
    assert_eq!(run_phase(&db, study_c).await, "materials");

    // 浅看冻结的样本没有被后来的精选研究改动。
    assert_eq!(run_phase(&db, run).await, "ended");
    assert_eq!(
        count(&db, "SELECT count(*) FROM collection_run_work w JOIN collection_run r USING(run_ref) WHERE r.source_kind='creator_preview' AND r.phase='ended'").await,
        4
    );
    // 材料用途：浅看与精选运行带来的材料在库里被标成「创作者语境」。读取侧哪些点按这个标记过滤、哪些按引用读取不过滤，
    // 见 collection-single-run-001.md；整个领域的列表与统计不含它们，由 creator_context_works_stay_out_of_domain_wide_lists_and_statistics 证明。
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_domain_usage usage JOIN collection_run_order relation ON relation.work_order_ref=usage.work_order_ref JOIN collection_run run ON run.run_ref=relation.run_ref WHERE run.source_kind IN ('creator_preview','creator_selected_works') AND usage.use_kind<>'creator_context'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(leaked, 0, "浅看与精选运行写入的用量行一律标成创作者语境");
    let context: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_domain_usage WHERE use_kind='creator_context'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert!(context > 0, "上面的不变式不能是空转");
    let promoted_cards: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_domain_usage usage JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref WHERE content.content_external_id LIKE 'preview-card-%' AND usage.use_kind='domain_research'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(promoted_cards, 0);
    // 已经在领域里的作品（第 2 篇）原来的研究用途不被改写。
    let known_use: Vec<String> = sqlx::query_scalar("SELECT usage.use_kind FROM linggan_material_domain_usage usage JOIN linggan_material_content content ON content.public_ref=usage.content_public_ref WHERE content.content_external_id='preview-known-1' AND usage.basis_kind='legacy_domain_migration'")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert_eq!(known_use, vec!["domain_research".to_owned()]);
    assert_eq!(
        forbidden_side_effects(&db).await.2,
        0,
        "整条链没有任何模型调用"
    );
}

fn detail_record_for(note: &str, author: &str) -> Value {
    json!({
        "kind": "content_detail",
        "sourceObject": {"platform": "xhs", "type": "content", "externalId": note},
        "payload": {"noteId": note, "title": format!("详情 {note}"), "bodyText": "合成正文：先做两分钟。", "authorId": author, "authorName": "多篇作者", "likedCount": 9}
    })
}

/// 工位把一次文字版精选运行的详情任务领完并交卷，每轮先推进再领；返回被要求取详情的笔记。
async fn drive_text_only(
    db: &Database,
    install: Uuid,
    credential: &str,
    run: Uuid,
    author: &str,
) -> Vec<String> {
    let mut asked = Vec::new();
    for _ in 0..40 {
        linggan_evidence::collection_run_progress::advance_runs(db)
            .await
            .unwrap();
        while let Some(spec) = next_task(db, install, credential).await {
            assert_eq!(spec["capabilitiesRequested"][0], "content_detail");
            assert_eq!(
                spec["collectionRunRef"],
                json!(run),
                "领到的任务属于这次运行"
            );
            let note = spec["target"]["contentExternalId"]
                .as_str()
                .unwrap()
                .to_owned();
            deliver(
                db,
                install,
                &spec,
                "content_detail",
                vec![detail_record_for(&note, author)],
                json!({"basis": "known_set", "contentExternalId": note}),
            )
            .await;
            asked.push(note);
        }
        if run_phase(db, run).await == "ended" {
            break;
        }
    }
    linggan_evidence::collection_run_progress::advance_runs(db)
        .await
        .unwrap();
    asked
}

/// 文字版精选一次选很多篇：推进每次只看 3 篇，处理完的作品必须让出窗口——
/// 否则前 3 篇有旧详情时第 4 篇起永远排不上队，或同时完成 4 篇以上时后面的永远收不了尾，运行永不结束。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn text_only_study_of_many_works_ends_even_when_the_first_ones_already_have_details() {
    require_governance_phase();
    let db = isolated("preview_many").await;
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let author = "many-author";
    // 26 篇库里已有详情的领域作品（前 3 篇会进主页样本），再加后面 8 张卡片取详情后，作者名下共 34 篇，超过账号研究的 30 篇窗口。
    let mut known_refs = Vec::new();
    for index in 1..=26 {
        known_refs.push(
            seed_work(
                &db,
                domain,
                &format!("many-known-{index}"),
                author,
                "多篇作者",
                &format!("已知作品 {index}"),
                10 * index,
            )
            .await,
        );
    }
    let uri = format!(
        "/api/local/creators/{}/preview",
        linggan_contracts::creator_discovery::creator_key("xhs", author)
    );
    let (install, credential) = proof_station(&db).await;
    let (status, started) = post_json(
        &db,
        &uri,
        json!({"domain": domain, "commandRef": Uuid::new_v4()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    // 11 张卡片：库里已有详情的 3 篇排在最前，后面是 8 张全新卡片。
    let externals: Vec<String> = (1..=3)
        .map(|n| format!("many-known-{n}"))
        .chain((1..=8).map(|n| format!("many-card-{n}")))
        .collect();
    let cards: Vec<Value> = externals
        .iter()
        .enumerate()
        .map(|(index, external)| {
            json!({
                "kind": "profile_discovery_card",
                "resultPosition": index + 1,
                "sourceObject": {"platform": "xhs", "type": "content", "externalId": external},
                "payload": {"title": format!("卡片 {external}"), "url": format!("https://www.xiaohongshu.com/explore/{external}?xsec_token=t{index}")}
            })
        })
        .collect();
    for _ in 0..6 {
        let Some(spec) = next_task(&db, install, &credential).await else {
            break;
        };
        let kind = spec["capabilitiesRequested"][0]
            .as_str()
            .unwrap()
            .to_owned();
        let target = json!({"basis": "known_set", "authorExternalId": author});
        if kind == "author_profile" {
            let profile = json!({"kind": "author_profile", "sourceObject": {"platform": "xhs", "type": "author", "externalId": author}, "payload": {"userId": author, "nickname": "多篇作者", "fans": 3}});
            deliver(&db, install, &spec, &kind, vec![profile], target).await;
        } else {
            deliver(&db, install, &spec, &kind, cards.clone(), target).await;
        }
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
    }
    let (_, snapshot) = get_json(&db, &format!("{uri}?domain={domain}")).await;
    assert_eq!(snapshot["state"], "complete", "{snapshot}");
    let preview: Uuid = serde_json::from_value(snapshot["runRef"].clone()).unwrap();
    let refs: Vec<Uuid> = snapshot["works"]
        .as_array()
        .unwrap()
        .iter()
        .map(|work| serde_json::from_value(work["workRef"].clone()).unwrap())
        .collect();
    assert_eq!(refs.len(), 11);

    // 前 3 篇已有详情、第 4 篇是新卡片：已有详情的不再取，第 4 篇必须被排上，运行结束。
    let first = [refs[0], refs[1], refs[2], refs[3]];
    let (status, receipt) = post_json(
        &db,
        "/api/collection/runs",
        selected_study(Uuid::new_v4(), domain, preview, &first, "none"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let reused_first: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let asked = drive_text_only(&db, install, &credential, reused_first, author).await;
    assert_eq!(asked, vec!["many-card-1"]);
    assert_eq!(
        run_phase(&db, reused_first).await,
        "ended",
        "前 3 篇已有详情时，后面的新作品仍要被排上队并收尾"
    );

    // 7 张全新卡片一起取：同时完成的篇数超过窗口，后面的也必须收得了尾。
    let many: Vec<Uuid> = refs[4..11].to_vec();
    let (status, receipt) = post_json(
        &db,
        "/api/collection/runs",
        selected_study(Uuid::new_v4(), domain, preview, &many, "none"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let seven: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let mut asked = drive_text_only(&db, install, &credential, seven, author).await;
    asked.sort();
    assert_eq!(
        asked,
        (2..=8)
            .map(|n| format!("many-card-{n}"))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        run_phase(&db, seven).await,
        "ended",
        "一次选 7 篇的文字版精选必须结束"
    );

    // 账号研究的样本只取领域研究作品：作者名下 34 篇（26 篇领域作品 + 8 张创作者语境卡片）时，
    // 样本仍是那 26 篇，不会被卡片挤出 30 篇的窗口。
    let (status, account) = get_json(
        &db,
        &format!(
            "/api/local/creator-account-research/preview?domainRef={domain}&platform=xhs&authorExternalId={author}"
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{account}");
    let mut sampled: Vec<Uuid> = account["workRefs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|work| serde_json::from_value(work.clone()).unwrap())
        .collect();
    sampled.sort();
    let mut expected = known_refs.clone();
    expected.sort();
    assert_eq!(sampled, expected, "账号研究样本恰好是 26 篇领域研究作品");
}

/// 一篇只在作者主页上出现过、并且带一条评论的作品（例如某次标准版精选研究取回了它的评论）。
async fn seed_context_work_with_comment(db: &Database, domain: Uuid, external: &str) -> Uuid {
    seed_work(
        db,
        domain,
        external,
        "context-author",
        "语境作者",
        "语境评论作品",
        1,
    )
    .await;
    material_fixture::submit_package(
        db,
        "comments",
        json!({"contentExternalId": external}),
        json!({
            "kind": "comment",
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": external},
            "payload": {"commentId": format!("{external}-c1"), "noteId": external, "text": "合成评论：只出现在作者主页的作品下。", "authorId": "reader"}
        }),
    )
    .await;
    material_fixture::make_creator_context_only(db, domain, external).await
}

/// 一次领域级读取的快照：「整个领域的列表与统计」里各取一个有代表性的读取点。
async fn domain_wide_reads(db: &Database, domain: Uuid) -> Value {
    let domains = linggan_evidence::observation_domain::read_observation_domains(db)
        .await
        .unwrap();
    let sample_count = domains
        .iter()
        .find(|item| item.domain_ref == domain)
        .expect("domain is readable")
        .sample_count;
    let (status, management) = get_json(db, "/api/collection/workspace/domains").await;
    assert_eq!(status, StatusCode::OK, "{management}");
    let entry = management["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["domainRef"] == json!(domain))
        .expect("domain is in the management statistics")
        .clone();
    let (status, list) = get_json(db, &format!("/api/local/work-resources?domain={domain}")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let mut listed: Vec<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["identity"]["publicRef"].as_str().unwrap().to_owned())
        .collect();
    listed.sort();
    let mut tx = db.pool().begin().await.unwrap();
    let (mut directory, total) =
        linggan_evidence::research_dossier::creation_work_directory_in(&mut tx, domain, None, 0)
            .await
            .unwrap();
    tx.rollback().await.unwrap();
    directory.sort();
    json!({
        "sampleCount": sample_count,
        "management": {
            "sampleCount": entry["sampleCount"],
            "commentCount": entry["commentCount"],
            "lastObservation": entry["lastObservation"],
            "lanes": entry["lanes"],
        },
        "listed": listed,
        "creationDirectory": directory,
        "creationTotal": total,
    })
}

/// 浅看新发现的作品只是这位作者的语境：不进领域样本数、管理页统计、语料作品列表和创作作品目录；
/// 按运行、按作品引用读取它们不受影响；库里原有的作品即使同时出现在这次的卡片里，也只算一次。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn creator_context_works_stay_out_of_domain_wide_lists_and_statistics() {
    require_governance_phase();
    let db = isolated("preview_reads").await;
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let author = "context-author";
    let known = seed_work(
        &db,
        domain,
        "context-known-1",
        author,
        "语境作者",
        "已知作品",
        120,
    )
    .await;
    let other = seed_work(
        &db,
        domain,
        "context-other-1",
        "other-author",
        "其他作者",
        "其他作品",
        5,
    )
    .await;
    let (install, credential) = proof_station(&db).await;

    // 浅看之前：领域里就是这两篇研究作品。
    let before = domain_wide_reads(&db, domain).await;
    let mut research = vec![known.to_string(), other.to_string()];
    research.sort();
    assert_eq!(before["sampleCount"], 2, "{before}");
    assert_eq!(before["listed"], json!(research), "{before}");
    assert_eq!(before["creationDirectory"], json!(research), "{before}");
    // 另有一篇只在作者主页上出现过、带评论的作品：它的评论不能让管理页的评论数多出一条。
    let commented = seed_context_work_with_comment(&db, domain, "context-commented-1").await;

    // 浅看：作者资料 + 4 张主页卡片，其中一张是库里已有的作品。
    let uri = format!(
        "/api/local/creators/{}/preview",
        linggan_contracts::creator_discovery::creator_key("xhs", author)
    );
    let (status, started) = post_json(
        &db,
        &uri,
        json!({"domain": domain, "commandRef": Uuid::new_v4()}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{started}");
    let run: Uuid = serde_json::from_value(started["runRef"].clone()).unwrap();
    let cards: Vec<Value> = [
        (1, "context-card-1"),
        (2, "context-known-1"),
        (3, "context-card-3"),
        (4, "context-card-4"),
    ]
    .iter()
    .map(|(position, external)| {
        json!({
            "kind": "profile_discovery_card",
            "resultPosition": position,
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": external},
            "payload": {
                "title": format!("主页卡片 {position}"),
                "url": format!("https://www.xiaohongshu.com/explore/{external}?xsec_token=proof{position}")
            }
        })
    })
    .collect();
    for _ in 0..6 {
        let Some(spec) = next_task(&db, install, &credential).await else {
            break;
        };
        let kind = spec["capabilitiesRequested"][0]
            .as_str()
            .unwrap()
            .to_owned();
        let target = json!({"basis": "known_set", "authorExternalId": author});
        match kind.as_str() {
            "author_profile" => {
                let profile = json!({"kind": "author_profile", "sourceObject": {"platform": "xhs", "type": "author", "externalId": author}, "payload": {"userId": author, "nickname": "语境作者", "fans": 17}});
                deliver(&db, install, &spec, &kind, vec![profile], target).await;
            }
            "profile_discovery" => {
                deliver(&db, install, &spec, &kind, cards.clone(), target).await;
            }
            other => panic!("浅看只该要作者资料和主页卡片，却被要求 {other}"),
        }
        linggan_evidence::collection_run_progress::advance_runs(&db)
            .await
            .unwrap();
    }
    linggan_evidence::collection_run_progress::advance_runs(&db)
        .await
        .unwrap();
    assert_eq!(run_phase(&db, run).await, "ended");

    // 写入侧：4 篇只在作者主页上出现过的作品只有「作者语境」用途；库里原有、又出现在卡片里的那篇同时有两种用途。
    let kinds: Vec<(String, Vec<String>)> = sqlx::query_as(
        "SELECT c.content_external_id, array_agg(DISTINCT u.use_kind ORDER BY u.use_kind) \
         FROM linggan_material_domain_usage u JOIN linggan_material_content c ON c.public_ref=u.content_public_ref \
         WHERE u.domain_ref=$1 GROUP BY c.content_external_id ORDER BY c.content_external_id",
    )
    .bind(domain)
    .fetch_all(db.pool())
    .await
    .unwrap();
    let context = || vec!["creator_context".to_owned()];
    assert_eq!(
        kinds,
        vec![
            ("context-card-1".to_owned(), context()),
            ("context-card-3".to_owned(), context()),
            ("context-card-4".to_owned(), context()),
            ("context-commented-1".to_owned(), context()),
            (
                "context-known-1".to_owned(),
                vec!["creator_context".to_owned(), "domain_research".to_owned()]
            ),
            (
                "context-other-1".to_owned(),
                vec!["domain_research".to_owned()]
            ),
        ]
    );

    // 读取侧：整个领域的列表与统计一个字都没变——新卡片没有被算进去，已知作品也没有被算两次。
    let after = domain_wide_reads(&db, domain).await;
    assert_eq!(after, before, "浅看不得改变领域级的列表与统计");

    // 按运行读取不受影响：这次浅看的 4 张卡片仍能按运行取到；新作品也能按作品引用取到、算作在本领域内。
    let frozen: Vec<(String, Uuid)> = sqlx::query_as(
        "SELECT c.content_external_id,c.public_ref FROM collection_run_work w JOIN linggan_material_content c ON c.public_ref=w.content_public_ref WHERE w.run_ref=$1 ORDER BY w.ordinal",
    )
    .bind(run)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(frozen.len(), 4);
    let mut frozen_refs: Vec<String> = frozen.iter().map(|(_, work)| work.to_string()).collect();
    frozen_refs.sort();
    let (status, by_run) = get_json(
        &db,
        &format!("/api/local/work-resources?domain={domain}&collectionRunRef={run}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{by_run}");
    let mut run_listed: Vec<String> = by_run["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["identity"]["publicRef"].as_str().unwrap().to_owned())
        .collect();
    run_listed.sort();
    assert_eq!(
        run_listed, frozen_refs,
        "按运行读取仍列出这次浅看的全部卡片"
    );
    let new_work = frozen
        .iter()
        .find(|(external, _)| external == "context-card-1")
        .unwrap()
        .1;
    let (status, by_ref) = get_json(
        &db,
        &format!("/api/local/work-resources?domain={domain}&publicRefs={new_work}"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{by_ref}");
    assert_eq!(
        by_ref["items"][0]["identity"]["publicRef"],
        json!(new_work.to_string()),
        "按作品引用仍能读到浅看新发现的作品"
    );
    // 带筛选条件（读取通道「发现」）的领域列表走另一段取数：只剩库里原有、同时出现在卡片里的那一篇。
    let (status, by_lane) = get_json(
        &db,
        &format!("/api/local/work-resources?domain={domain}&lane=discovery"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{by_lane}");
    let discovered: Vec<String> = by_lane["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["identity"]["publicRef"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(discovered, vec![known.to_string()], "{by_lane}");
    let mut tx = db.pool().begin().await.unwrap();
    for work in [new_work, commented] {
        assert!(
            linggan_evidence::research_dossier::creation_work_in_domain(&mut tx, work, domain)
                .await
                .unwrap(),
            "按引用判断「是否在本领域」不过滤用途"
        );
    }
    tx.rollback().await.unwrap();

    // 精选研究（文字版）把三张新卡片的详情取回来之后，领域级读取仍然一个字没变：
    // 取回详情的作品还是作者语境，不会因此变成领域研究材料；详情通道的分类也不把它们算进领域。
    let new_cards: Vec<Uuid> = frozen
        .iter()
        .filter(|(external, _)| external.starts_with("context-card-"))
        .map(|(_, work)| *work)
        .collect();
    assert_eq!(new_cards.len(), 3);
    let (status, receipt) = post_json(
        &db,
        "/api/collection/runs",
        selected_study(Uuid::new_v4(), domain, run, &new_cards, "none"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let study: Uuid = serde_json::from_value(receipt["runRef"].clone()).unwrap();
    let asked = drive_text_only(&db, install, &credential, study, author).await;
    assert_eq!(asked.len(), 3, "三张新卡片各取一次详情：{asked:?}");
    assert_eq!(run_phase(&db, study).await, "ended");
    let after_study = domain_wide_reads(&db, domain).await;
    assert_eq!(
        after_study, before,
        "精选研究取回详情后，领域级的列表与统计仍不变"
    );
}

/// 把外部编号以 `prefix` 开头的作品批量标成只在作者主页上出现过（测试辅助，原因同 `make_creator_context_only`）。
async fn make_many_creator_context_only(db: &Database, domain: Uuid, prefix: &str) -> u64 {
    let mut tx = db.pool().begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *tx)
        .await
        .unwrap();
    let changed = sqlx::query("UPDATE linggan_material_domain_usage SET use_kind='creator_context' WHERE domain_ref=$1 AND content_public_ref IN (SELECT public_ref FROM linggan_material_content WHERE content_external_id LIKE $2)")
        .bind(domain)
        .bind(format!("{prefix}%"))
        .execute(&mut *tx)
        .await
        .unwrap()
        .rows_affected();
    tx.commit().await.unwrap();
    changed
}

/// 领域里有很多只在作者主页上出现过的作品，而且排在研究作品前面时，领域列表仍然列出研究作品：
/// 无筛选的列表先按页取最近的 51 篇再往下处理，语境作品若先占满这一页、事后才被过滤，整页会是空的。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn many_creator_context_works_do_not_crowd_research_works_out_of_the_domain_list() {
    let db = isolated("preview_crowd").await;
    let domain = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let mut research = Vec::new();
    for n in 1..=2 {
        let work = seed_work(
            &db,
            domain,
            &format!("z-research-{n}"),
            "research-author",
            "研究作者",
            &format!("研究作品 {n}"),
            10,
        )
        .await;
        research.push(work.to_string());
    }
    // 外部编号排在研究作品之前；合成包的观察时间都相同，列表按外部编号排，所以它们排在最前。
    for n in 0..60 {
        seed_work(
            &db,
            domain,
            &format!("a-crowd-{n:03}"),
            "crowd-author",
            "语境作者",
            &format!("语境作品 {n}"),
            1,
        )
        .await;
    }
    assert_eq!(
        make_many_creator_context_only(&db, domain, "a-crowd-").await,
        60
    );
    research.sort();
    let (status, list) = get_json(&db, &format!("/api/local/work-resources?domain={domain}")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let mut listed: Vec<String> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["identity"]["publicRef"].as_str().unwrap().to_owned())
        .collect();
    listed.sort();
    assert_eq!(listed, research, "60 篇语境作品不能把研究作品挤出领域列表");
}
