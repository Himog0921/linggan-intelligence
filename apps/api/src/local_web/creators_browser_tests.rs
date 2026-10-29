//! WEB-REACT-001 阶段 D1：React 创作者页（语料）在真实 Axum + 隔离 PostgreSQL + 合成材料上的证明，
//! 以及语料外壳只读接口对真实数据库的读取证明。全部默认忽略，由 `scripts/test-web-app-postgres.sh` 运行。
//! 只用合成作者与作品；不调用模型、不访问平台、不碰开发库或本机常驻服务。
use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use std::{fs, path::Path as ProofPath};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;

const FIRST_DOMAIN_AUTHORS: usize = 27;

/// 一篇合成作品，归到某个领域的主研究用途下。`likes` 随作者序号变化，便于核对爆款标准与排序。
pub(super) async fn seed_work(
    db: &Database,
    domain: Uuid,
    external: &str,
    author: &str,
    name: &str,
    title: &str,
    likes: i64,
) -> Uuid {
    let package = fixture::submit_package(
        db,
        "content_detail",
        json!({"contentExternalId": external}),
        json!({
            "kind": "content_detail",
            "sourceObject": {"platform": "xhs", "type": "content", "externalId": external},
            "payload": {
                "title": title,
                "bodyText": format!("合成正文：{title}。先做两分钟，再决定要不要继续。"),
                "authorId": author,
                "authorName": name,
                "likedCount": likes
            }
        }),
    )
    .await;
    let work: Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(external)
    .fetch_one(db.pool())
    .await
    .unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) VALUES($1,$2,$3,'primary','legacy_domain_migration',$4)")
        .bind(Uuid::new_v4())
        .bind(work)
        .bind(domain)
        .bind(package)
        .execute(db.pool())
        .await
        .unwrap();
    work
}

fn author_id(index: usize) -> String {
    format!("author-{index:02}")
}

/// 两个领域：第一个领域 27 位作者（超过一页 25 条，翻页有内容），第二个领域 2 位。
async fn seed(db: &Database) -> (Uuid, Uuid, Vec<Uuid>) {
    let first = linggan_evidence::observation_domain::create_observation_domain_with_details(
        db,
        "合成领域一",
        Some("合成领域说明：用于创作者页的真实链路证明。"),
        Some("合成研究目标：看清创作者与作品的关系。"),
    )
    .await
    .expect("first domain")
    .domain_ref;
    let second = linggan_evidence::observation_domain::create_observation_domain_with_details(
        db,
        "合成领域二",
        None,
        None,
    )
    .await
    .expect("second domain")
    .domain_ref;
    let mut works = Vec::new();
    for index in 1..=FIRST_DOMAIN_AUTHORS {
        works.push(
            seed_work(
                db,
                first,
                &format!("creators-proof-a-{index:02}"),
                &author_id(index),
                &format!("合成作者 {index:02}"),
                &format!("合成作品 {index:02}"),
                (index as i64) * 100,
            )
            .await,
        );
    }
    for index in 1..=2 {
        seed_work(
            db,
            second,
            &format!("creators-proof-b-{index:02}"),
            &format!("author-b{index}"),
            &format!("第二领域作者 {index}"),
            &format!("第二领域作品 {index}"),
            50 * index as i64,
        )
        .await;
    }
    // 第 2 位作者已在第一个领域的观察名单里：用于展示「已加入」与采集状态词「观察中」。
    linggan_evidence::register_discovered_creator(
        db,
        first,
        &linggan_contracts::creator_discovery::creator_key("xhs", &author_id(2)),
        "primary",
    )
    .await
    .expect("seed one observation target");
    (first, second, works)
}

pub(super) async fn count(db: &Database, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(db.pool()).await.unwrap()
}

/// 浏览器路径里本不该发生的副作用：采集请求、工单、模型调用。
pub(super) async fn forbidden_side_effects(db: &Database) -> (i64, i64, i64) {
    (
        count(db, "SELECT count(*) FROM collection_acquisition_request").await,
        count(db, "SELECT count(*) FROM collection_work_order").await,
        count(db, "SELECT count(*) FROM linggan_model_invocation").await,
    )
}

pub(super) async fn isolated(prefix: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("isolated proof URL supplied");
    let schema = format!("{prefix}_{}", Uuid::new_v4().simple());
    linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        &schema,
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .expect("full migrations apply to isolated schema")
}

pub(super) async fn get_json(db: &Database, uri: &str) -> (StatusCode, Value) {
    let response = app_with_database(db.clone())
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::HOST, "127.0.0.1:3000")
                .body(Body::empty())
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

/// 语料外壳接口读真实数据库：领域列表与作品数、对地址里领域的解析、采集状态词的三种取值。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn corpus_shell_api_reads_domains_selection_and_collection_state() {
    let db = isolated("corpus_shell").await;
    let first = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域一",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    let second = linggan_evidence::observation_domain::create_observation_domain_with_details(
        &db,
        "合成领域二",
        None,
        None,
    )
    .await
    .unwrap()
    .domain_ref;
    seed_work(
        &db,
        first,
        "shell-proof-1",
        "author-01",
        "合成作者 01",
        "合成作品 01",
        100,
    )
    .await;
    seed_work(
        &db,
        first,
        "shell-proof-2",
        "author-02",
        "合成作者 02",
        "合成作品 02",
        200,
    )
    .await;

    // 还没有任何观察目标：读到的是零个，不是读不到。
    let (status, shell) = get_json(&db, &format!("/api/local/corpus/shell?domain={first}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(shell["collectionState"], "无观察目标");
    assert_eq!(shell["selectedDomain"], json!(first));
    // 完整迁移会预置几个默认领域；这里只核对本次播种的两个，按编号找。
    let domains = shell["domains"].as_array().unwrap();
    assert!(domains.len() >= 2);
    let find = |reference: Uuid| {
        domains
            .iter()
            .find(|domain| domain["domainRef"] == json!(reference))
            .expect("seeded domain is listed")
    };
    assert_eq!(find(first)["name"], "合成领域一");
    assert_eq!(find(first)["status"], "active");
    assert_eq!(find(first)["sampleCount"], 2);
    assert_eq!(find(second)["sampleCount"], 0);

    // 缺省、无效、不存在的领域都不选任何领域，也不报错。
    for uri in [
        "/api/local/corpus/shell".to_owned(),
        "/api/local/corpus/shell?domain=not-a-domain".to_owned(),
        format!("/api/local/corpus/shell?domain={}", Uuid::new_v4()),
    ] {
        let (status, shell) = get_json(&db, &uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert_eq!(shell["selectedDomain"], Value::Null, "{uri}");
    }

    // 有了观察目标：观察中；暂停的领域显示为 paused。
    linggan_evidence::register_discovered_creator(
        &db,
        first,
        &linggan_contracts::creator_discovery::creator_key("xhs", "author-01"),
        "primary",
    )
    .await
    .unwrap();
    sqlx::query("UPDATE observation_domain SET status='paused' WHERE domain_ref=$1")
        .bind(second)
        .execute(db.pool())
        .await
        .unwrap();
    let (_, shell) = get_json(&db, &format!("/api/local/corpus/shell?domain={second}")).await;
    assert_eq!(shell["collectionState"], "观察中");
    assert_eq!(shell["selectedDomain"], json!(second));
    let domains = shell["domains"].as_array().unwrap();
    let find = |reference: Uuid| {
        domains
            .iter()
            .find(|domain| domain["domainRef"] == json!(reference))
            .expect("seeded domain is listed")
    };
    assert_eq!(find(second)["status"], "paused");
    assert_eq!(find(first)["targetCount"], 1);

    // 读接口没有任何写入副作用。
    assert_eq!(forbidden_side_effects(&db).await, (0, 0, 0));
}

/// 浏览器真实链路：打开 → 筛选 / 翻页 → 抽屉 → 真实调整判断 → 发现设置 → 切换领域 →
/// 证据库往返 → 加入观察（确认、未确认重试）→ 四个视口无横向溢出。
// 多线程：浏览器子进程运行期间，服务器任务必须能在其他线程上继续响应。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL + built apps/web/dist + Node/Chromium; run by scripts/test-web-app-postgres.sh"]
async fn creators_page_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE"),
        "浏览器证明只允许合成凭据"
    );
    let db = isolated("creators").await;
    let (first, second, works) = seed(&db).await;
    // 一份已冻结的主页浅看样本（作者 08 的 3 张卡片）：直接落库，不经派发，所以不产生采集请求或工单。
    // 顺序与真实流程一致：先放作品，再冻结范围（库里的触发器不许往已冻结的范围里再放作品）。
    let preview_run = Uuid::new_v4();
    sqlx::query("INSERT INTO collection_run(run_ref,idempotency_key,request_hash,domain_ref,source_kind,name,search_query,work_limit,comment_mode) VALUES($1,$2,repeat('0',64),$3,'creator_preview','主页浅看 · 合成作者 08',$4,30,'none')")
        .bind(preview_run)
        .bind(Uuid::new_v4())
        .bind(first)
        .bind(author_id(8))
        .execute(db.pool())
        .await
        .unwrap();
    for (ordinal, work) in works[7..10].iter().enumerate() {
        sqlx::query(
            "INSERT INTO collection_run_work(run_ref,content_public_ref,ordinal) VALUES($1,$2,$3)",
        )
        .bind(preview_run)
        .bind(work)
        .bind(ordinal as i32 + 1)
        .execute(db.pool())
        .await
        .unwrap();
    }
    sqlx::query("UPDATE collection_run SET phase='ended',scope_frozen_at=scope_001_now(),ended_at=scope_001_now(),discovery_reason='surface_ended' WHERE run_ref=$1")
        .bind(preview_run)
        .execute(db.pool())
        .await
        .unwrap();
    let before = forbidden_side_effects(&db).await;
    assert_eq!(before, (0, 0, 0), "播种本身不创建采集请求、工单或模型调用");

    let application = app_with_database(db.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, application).await.unwrap() });
    let directory = std::env::var("WEB_APP_PROOF_DIR").expect("evidence directory is supplied");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        ProofPath::new(&directory).join("fixture.json"),
        json!({
            "origin": origin,
            "domainOne": first,
            "domainTwo": second,
            "authorTotal": FIRST_DOMAIN_AUTHORS,
            "registeredAuthor": "合成作者 02",
            "correctedAuthor": "合成作者 03",
            "correctedWork": works[2],
            "joinedAuthor": "合成作者 05",
            "retriedAuthor": "合成作者 06",
            "previewAuthor": "合成作者 08",
            "previewTitles": ["合成作品 08", "合成作品 09", "合成作品 10"],
        })
        .to_string(),
    )
    .unwrap();
    let node = std::env::var("WEB_APP_PROOF_NODE").expect("node binary is supplied");
    let script =
        ProofPath::new(env!("CARGO_MANIFEST_DIR")).join("../web/test/e2e/creators-real-chain.mjs");
    let result = std::process::Command::new(node)
        .arg(script)
        .env("WEB_APP_PROOF_DIR", &directory)
        .status();
    server.abort();
    let _ = server.await;

    // ── 浏览器之后，回数据库核对真实后果 ─────────────────────────────────────
    // 加入观察：每位作者恰好一个手动登记的观察目标、一条与第一个领域的主研究关系，且没有采集或模型副作用。
    for author in [author_id(5), author_id(6)] {
        let targets: i64 = sqlx::query_scalar("SELECT count(*) FROM collection_observation_target WHERE platform='xhs' AND target_kind='creator' AND identity_key=$1 AND source='manual'")
            .bind(&author)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(targets, 1, "{author} 应恰好有一个手动登记的观察目标");
        let relations: i64 = sqlx::query_scalar("SELECT count(*) FROM observation_domain_target relation JOIN collection_observation_target target USING(target_ref) WHERE target.identity_key=$1 AND relation.domain_ref=$2 AND relation.role='primary'")
            .bind(&author)
            .bind(first)
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(
            relations, 1,
            "{author} 与第一个领域应恰好一条主研究关系（重试没有重复登记）"
        );
    }
    assert_eq!(
        forbidden_side_effects(&db).await,
        before,
        "浏览器路径不得产生采集请求、工单或模型调用"
    );
    // 调整判断：人工覆盖写在那篇作品上，引用了当前的支持片段，且是人工身份。
    let manual: Value = sqlx::query_scalar("SELECT manual_overrides FROM linggan_creator_work_override WHERE domain_ref=$1 AND work_public_ref=$2")
        .bind(first)
        .bind(works[2])
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(manual["relevance"]["value"]["value"], "related");
    assert_eq!(manual["relevance"]["actor"], "local_user");
    assert!(
        !manual["relevance"]["supportFragmentIds"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    // 发现设置：点赞标准存下，版本号加一。
    let (threshold, revision): (Option<i64>, i64) = sqlx::query_as("SELECT like_threshold,revision FROM linggan_creator_discovery_threshold WHERE domain_ref=$1 AND platform='xhs'")
        .bind(first)
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!((threshold, revision), (Some(100), 1));
    // 精选研究：页面发出的请求在库里恰好建出一个精选运行——只含所选的两篇、指向那份浅看、文字版、没有排工单。
    let studies: Vec<(Uuid, i32, String, String)> = sqlx::query_as("SELECT run_ref,work_limit,comment_mode,phase FROM collection_run WHERE source_kind='creator_selected_works'")
        .fetch_all(db.pool())
        .await
        .unwrap();
    assert_eq!(studies.len(), 1, "页面确认一次，库里恰好一个精选运行");
    let (study, limit, mode, phase) = &studies[0];
    assert_eq!(
        (*limit, mode.as_str(), phase.as_str()),
        (2, "none", "materials")
    );
    let chosen: Vec<Uuid> = sqlx::query_scalar(
        "SELECT content_public_ref FROM collection_run_work WHERE run_ref=$1 ORDER BY ordinal",
    )
    .bind(study)
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        chosen,
        vec![works[7], works[9]],
        "取消了中间那篇，只剩第一篇与第三篇"
    );
    let frozen: i64 =
        sqlx::query_scalar("SELECT count(*) FROM collection_run_work WHERE run_ref=$1")
            .bind(preview_run)
            .fetch_one(db.pool())
            .await
            .unwrap();
    assert_eq!(frozen, 3, "精选研究没有改动浅看冻结的样本");
    assert!(
        result.expect("browser subprocess starts").success(),
        "React creators browser proof failed; see the evidence directory"
    );
}
