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
/// 文字版不取媒体与评论，标准版仍然继续评论采集；浅看与精选运行的材料用途不进入领域研究名单。
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
                deliver(&db, install, &spec, &kind, vec![profile], target).await;
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
    // 材料用途：浅看与精选运行带来的材料只是「创作者语境」，不进入领域研究名单。
    let leaked: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_domain_usage usage JOIN collection_run_order relation ON relation.work_order_ref=usage.work_order_ref JOIN collection_run run ON run.run_ref=relation.run_ref WHERE run.source_kind IN ('creator_preview','creator_selected_works') AND usage.use_kind<>'creator_context'")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(leaked, 0, "浅看与精选运行的材料不得成为领域研究成员");
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
