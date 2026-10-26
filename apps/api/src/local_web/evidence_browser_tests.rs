//! WEB-REACT-001 阶段 D2：React 证据库页在真实 Axum + 隔离 PostgreSQL + 合成材料上的浏览器证明。
//! 默认忽略，由 `scripts/test-web-app-postgres.sh` 运行。只用合成作品、评论与媒体；不调用模型、不访问平台、
//! 不碰开发库或本机常驻服务。页面本身由已构建的 `apps/web/dist` 提供（`LINGGAN_WEB_DIST`）。
use super::evidence_fixture::{isolated, public_ref};
use super::evidence_seed::{PLAIN_WORKS, seed_dataset};
use super::*;
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;

/// 浏览器路径在库里留下的后果：采集请求、准入决定、工单、租约、模型调用、重新分层结果。
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    requests: i64,
    decisions: i64,
    work_orders: i64,
    leases: i64,
    model_invocations: i64,
    reading_rows: i64,
}

async fn effects(db: &Database) -> Effects {
    let count = |sql: &'static str| async move {
        sqlx::query_scalar::<_, i64>(sql)
            .fetch_one(db.pool())
            .await
            .unwrap()
    };
    Effects {
        requests: count("SELECT count(*) FROM collection_acquisition_request").await,
        decisions: count("SELECT count(*) FROM collection_admission_decision").await,
        work_orders: count("SELECT count(*) FROM collection_work_order").await,
        leases: count("SELECT count(*) FROM collection_work_order_lease").await,
        model_invocations: count("SELECT count(*) FROM linggan_model_invocation").await,
        reading_rows: count("SELECT count(*) FROM linggan_material_reading").await,
    }
}

/// 脚本报告里的一条写请求。
struct Write {
    path: String,
    status: u64,
    updated: Option<i64>,
}

fn writes(report: &Value) -> Vec<Write> {
    report["writes"]
        .as_array()
        .expect("the browser report lists its write requests")
        .iter()
        .map(|entry| Write {
            path: entry["path"].as_str().unwrap().to_owned(),
            status: entry["status"].as_u64().unwrap(),
            updated: entry["updated"].as_i64(),
        })
        .collect()
}

/// 浏览器真实链路：打开与外壳 → 未选领域 → 三种排版与续读 → 检索与筛选 → 阅读主序 → 媒体与查看器 → 评论 →
/// 研究底稿（预览、导出、重新分层、批量弹窗）→ 立即复观测 → 评论深采 → 入站链接 → 评论深采交接 → 存储 →
/// 四个视口 → 负例。
// 多线程：浏览器子进程运行期间，服务器任务必须能在其他线程上继续响应。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL + built apps/web/dist + Node/Chromium; run by scripts/test-web-app-postgres.sh"]
async fn evidence_page_real_chain_browser_proof() {
    assert_eq!(std::env::var("WEB_APP_BROWSER_PROOF").as_deref(), Ok("1"));
    assert_eq!(
        std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref(),
        Ok("SYNTHETIC-NOT-EVIDENCE"),
        "浏览器证明只允许合成凭据"
    );
    let db = isolated("ev_page").await;
    let data = seed_dataset(&db).await;
    let before = effects(&db).await;
    assert_eq!(
        (before.leases, before.model_invocations),
        (0, 0),
        "播种本身不形成租约，也不调用模型"
    );
    // 第一个领域里的全部作品：49 篇普通作品加 8 篇各有一种状态的作品。
    let total_works: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT content_public_ref) FROM linggan_material_domain_usage WHERE domain_ref=$1",
    )
    .bind(data.domain_one)
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(total_works as usize, PLAIN_WORKS + 8);

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
            "domainOne": data.domain_one,
            "domainTwo": data.domain_two,
            "rich": data.rich,
            "video": data.video,
            "risk": data.risk,
            "partial": data.partial,
            "cleaned": data.cleaned,
            "restricted": data.restricted,
            "reobserve": data.reobserve,
            "offPage": data.off_page,
            "plain": public_ref(&db, "ev-plain-00").await,
            "collectionRun": Uuid::new_v4(),
            "totalWorks": total_works,
            "pageSize": 50,
        })
        .to_string(),
    )
    .unwrap();
    let node = std::env::var("WEB_APP_PROOF_NODE").expect("node binary is supplied");
    let run = |script: &str| {
        std::process::Command::new(&node)
            .arg(
                ProofPath::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("../web/test/e2e/{script}")),
            )
            .env("WEB_APP_PROOF_DIR", &directory)
            .status()
            .expect("the browser script starts")
            .success()
    };
    let page_proof = run("evidence-real-chain.mjs");
    // 插件入口：最小的 MV3 测试扩展用 chrome.tabs.create 打开页面（真实插件打开证据库的方式）。它只读页面，
    // 不产生任何库内后果；被来源闸门拒绝就是失败——不放宽闸门。
    let plugin_entry = page_proof && run("evidence-plugin-entry.mjs");
    server.abort();
    let _ = server.await;
    assert!(
        page_proof,
        "浏览器证明失败；证据与 report.json 在 {directory}"
    );
    assert!(
        plugin_entry,
        "插件入口证明失败；见 {directory}/plugin-entry.json"
    );

    // ── 浏览器之后，回数据库核对真实后果 ─────────────────────────────────────
    let report: Value = serde_json::from_slice(
        &fs::read(ProofPath::new(&directory).join("report.json")).expect("browser report"),
    )
    .unwrap();
    assert_eq!(report["finished"], true);
    assert_eq!(report["pageErrors"], json!([]));
    assert_eq!(report["blockedRequests"], json!([]), "没有外域请求");
    let writes = writes(&report);
    let reobserve = writes
        .iter()
        .filter(|write| write.path.ends_with("/reobserve"))
        .count();
    let runs = writes
        .iter()
        .filter(|write| write.path == "/api/collection/runs")
        .count();
    assert_eq!(reobserve + runs, writes.len(), "页面只提交复观测与评论深采");
    assert!(writes.iter().all(|write| write.status == 200));
    // 脚本里的写动作是固定的：复观测两次（先建工单，再合并）；重新分层五次（底稿里两次、批量弹窗里三篇）；
    // 评论深采两次（富作品尽量采全、视频作品指定 40 条）。
    assert_eq!((reobserve, runs), (2, 2));
    // 评论深采与采集页同一条合同：每次一个只含该作品的单次采集（富作品尽量采全；视频作品采到 40 条就停）。
    // 建立时只冻结范围，采集请求、工单与租约要等调度推进才形成，所以下面的计数不受它影响。
    let runs: Vec<(Uuid, String, Option<i32>, String, i32)> = sqlx::query_as(
        "SELECT w.content_public_ref,r.comment_mode,r.comment_target,r.source_kind,r.work_limit \
         FROM collection_run r JOIN collection_run_work w USING(run_ref) ORDER BY r.comment_mode",
    )
    .fetch_all(db.pool())
    .await
    .unwrap();
    assert_eq!(
        runs,
        vec![
            (
                data.rich,
                "all_public".to_owned(),
                None,
                "selected_works".to_owned(),
                1
            ),
            (
                data.video,
                "up_to".to_owned(),
                Some(40),
                "selected_works".to_owned(),
                1
            ),
        ]
    );
    let after = effects(&db).await;
    let reobserved = reobserve as i64;
    assert_eq!(
        after.requests - before.requests,
        reobserved,
        "每次确认一条采集请求"
    );
    assert_eq!(
        after.decisions - before.decisions,
        reobserved,
        "每次确认一条准入决定"
    );
    assert_eq!(
        after.work_orders - before.work_orders,
        1,
        "两次提交落在同一张尚未被认领的工单上"
    );
    assert_eq!(after.leases, before.leases, "请求时从不形成租约");
    assert_eq!(
        after.model_invocations, before.model_invocations,
        "预览 AI 输入、导出、读取都不调用模型"
    );
    assert_eq!(
        after.reading_rows, before.reading_rows,
        "read-only browsing and collection requests do not rewrite canonical text"
    );
}
