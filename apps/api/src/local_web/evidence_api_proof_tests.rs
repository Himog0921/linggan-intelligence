//! WEB-REACT-001 阶段 D2：证据库页面依赖的接口行为，在隔离 PostgreSQL 上用真实调用证明。
//! 默认全部忽略，由 `scripts/test-web-app-postgres.sh` 运行。只用合成作品；不调用模型、不访问平台。
use super::evidence_fixture::{
    Work, count, create_domain, get, isolated, now_ms, post, public_ref, seed_work, send,
};
use super::evidence_seed::{Dataset, seed_dataset};
use super::*;
use axum::http::StatusCode;
use std::{fs, path::Path as ProofPath};
use uuid::Uuid;

const DAY_MS: i64 = 86_400_000;

fn snippet(text: &str) -> String {
    text.chars().take(160).collect()
}

/// DR-1：旧页的三个控件，在真实读库上能不能给出有效结果。
/// 每条记录写下「请求、状态、原因」，供计划文档的 DR-1 裁定引用。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn dead_controls_never_return_results_by_construction() {
    let db = isolated("ev_dead").await;
    let domain = create_domain(&db, "合成领域一").await;
    let now = now_ms();
    for (external, title, published) in [
        (
            "dr1-recent",
            "合成关键词 两天前发布",
            Some(now - 2 * DAY_MS),
        ),
        (
            "dr1-month",
            "合成关键词 二十天前发布",
            Some(now - 20 * DAY_MS),
        ),
        ("dr1-unknown", "合成关键词 发布时间未知", None),
    ] {
        let mut work = Work::new(external, title);
        work.published_ms = published;
        seed_work(&db, domain, &work).await;
    }
    let app = app_with_database(db.clone());
    let base = format!("/api/local/work-resources?domain={domain}");
    let mut record = Vec::new();
    let mut probe = |label: &str, reply: &super::evidence_fixture::Reply, uri: &str| {
        let items = reply
            .json
            .get("items")
            .and_then(Value::as_array)
            .map(Vec::len);
        record.push(json!({
            "control": label, "request": format!("GET {uri}"),
            "status": reply.status.as_u16(), "items": items,
            "queryScope": reply.json.get("queryScope"),
            "reason": snippet(&reply.text),
        }));
    };

    // 对照：同一批数据，缺省读取与检索都有结果，三篇作品都可读。
    let uri = base.clone();
    let reply = get(&app, &uri).await;
    probe("baseline", &reply, &uri);
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["items"].as_array().unwrap().len(), 3);
    // 两天前发布的作品确实带着可采用的来源发布时间（不是因为缺数据才查不到）。
    let recent = reply.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["display"]["title"] == "合成关键词 两天前发布")
        .expect("recent work is listed");
    assert_eq!(recent["display"]["publishedAtState"], "KNOWN", "{recent}");
    let uri = format!("{base}&q=合成关键词");
    let reply = get(&app, &uri).await;
    probe("search with text", &reply, &uri);
    assert_eq!(reply.status, StatusCode::OK, "{}", reply.text);
    assert_eq!(reply.json["items"].as_array().unwrap().len(), 3);
    let uri = format!("{base}&sort=latest_discovery");
    let reply = get(&app, &uri).await;
    probe("sort latest_discovery", &reply, &uri);
    assert_eq!(reply.status, StatusCode::OK);

    // 排序「相关度」：有检索词与没有检索词都是 400。
    for (label, uri) in [
        (
            "sort relevance without text",
            format!("{base}&sort=relevance"),
        ),
        (
            "sort relevance with text",
            format!("{base}&sort=relevance&q=合成关键词"),
        ),
    ] {
        let reply = get(&app, &uri).await;
        probe(label, &reply, &uri);
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{uri}: {}",
            reply.text
        );
        assert_eq!(
            reply.json["code"], "invalid_material_query_cursor_or_sort",
            "{uri}: {}",
            reply.text
        );
    }

    // 读取窗口「近 7 天／近 30 天来源发布时间」：200 但恒为空，即使有作品落在窗口内。
    for window in ["last_7_days", "last_30_days"] {
        let uri = format!("{base}&window={window}");
        let reply = get(&app, &uri).await;
        probe(&format!("window {window}"), &reply, &uri);
        assert_eq!(reply.status, StatusCode::OK, "{uri}: {}", reply.text);
        assert_eq!(
            reply.json["items"].as_array().map(Vec::len),
            Some(0),
            "{uri} 恒为空，即使两天前发布的作品在窗口内：{}",
            reply.text
        );
        assert_eq!(
            reply.json["queryScope"], "accepted_typed_material_strict_published_time",
            "{uri}"
        );
        // 带检索词也一样。
        let uri = format!("{base}&window={window}&q=合成关键词");
        let reply = get(&app, &uri).await;
        probe(&format!("window {window} with text"), &reply, &uri);
        assert_eq!(
            reply.json["items"].as_array().map(Vec::len),
            Some(0),
            "{uri}"
        );
    }

    // 通道状态「旧 OCR 已停用」（RETIRED）：400，后端枚举里没有这个值；合法值（对照）是 200。
    for (label, uri, expected) in [
        (
            "laneState RETIRED",
            format!("{base}&laneState=RETIRED"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "laneState RETIRED with lane=ocr",
            format!("{base}&lane=ocr&laneState=RETIRED"),
            StatusCode::BAD_REQUEST,
        ),
        (
            "laneState PARTIAL (valid control)",
            format!("{base}&laneState=PARTIAL"),
            StatusCode::OK,
        ),
    ] {
        let reply = get(&app, &uri).await;
        probe(label, &reply, &uri);
        assert_eq!(reply.status, expected, "{uri}: {}", reply.text);
    }

    // 这些只读探测不产生任何写入。
    assert_eq!(
        count(&db, "SELECT count(*) FROM collection_acquisition_request").await,
        0
    );
    assert_eq!(
        count(&db, "SELECT count(*) FROM collection_work_order").await,
        0
    );

    let report = json!({"proof": "DR-1 dead controls", "calls": record});
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    if let Ok(directory) = std::env::var("WEB_APP_PROOF_DIR") {
        fs::create_dir_all(&directory).unwrap();
        fs::write(
            ProofPath::new(&directory).join("dr1-dead-controls.json"),
            serde_json::to_string_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}

async fn reading_rows(db: &Database) -> i64 {
    count(db, "SELECT count(*) FROM linggan_material_reading").await
}

async fn reobservation_counts(db: &Database) -> [i64; 4] {
    [
        count(db, "SELECT count(*) FROM collection_acquisition_request").await,
        count(db, "SELECT count(*) FROM collection_admission_decision").await,
        count(db, "SELECT count(*) FROM collection_work_order").await,
        count(db, "SELECT count(*) FROM collection_work_order_lease").await,
    ]
}

/// 立即复观测：每次点击留下一条请求与准入决定；同一范围只有一张尚未被认领的工单（第二次合并到它）；
/// 请求时从不形成租约，状态地址也恒为空。
async fn reobservation_merges_and_never_leases(db: &Database, app: &Router, data: &Dataset) {
    let uri = format!(
        "/api/local/work-resources/{}/reobserve?domain={}",
        data.reobserve, data.domain_one
    );
    let before = reobservation_counts(db).await;
    let first = post(app, &uri, None).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    let operation = &first.json["operation"];
    assert_eq!(operation["admission"], "ADMITTED");
    assert_eq!(operation["execution"], "QUEUED");
    assert!(operation["leaseRef"].is_null() && first.json["statusUrl"].is_null());
    assert_eq!(operation["tasks"], json!([]));
    let order = operation["workOrderRef"].clone();
    assert!(order.is_string());
    let after_first = reobservation_counts(db).await;
    assert_eq!(
        after_first,
        [before[0] + 1, before[1] + 1, before[2] + 1, before[3]],
        "一次点击：一条请求、一条准入决定、一张新工单、没有租约"
    );
    let second = post(app, &uri, None).await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text);
    assert_eq!(second.json["operation"]["admission"], "MERGE");
    assert_eq!(
        second.json["operation"]["workOrderRef"], order,
        "合并到同一张工单"
    );
    assert_eq!(
        reobservation_counts(db).await,
        [
            after_first[0] + 1,
            after_first[1] + 1,
            after_first[2],
            after_first[3]
        ],
        "再点一次：多一条请求与决定，工单仍是同一张，没有租约"
    );
}

/// 被拒的写入不改库：没有关联授权、领域不匹配、请求体多余字段、缺少本机 Host。
async fn rejected_writes_change_nothing(db: &Database, app: &Router, data: &Dataset) {
    let domain = data.domain_one;
    let plain = public_ref(db, "ev-plain-00").await;
    let before = (reobservation_counts(db).await, reading_rows(db).await);
    let reobserve = |work: Uuid, scope: Uuid| {
        format!("/api/local/work-resources/{work}/reobserve?domain={scope}")
    };
    let unlinked = post(app, &reobserve(plain, domain), None).await;
    assert_eq!(unlinked.status, StatusCode::CONFLICT, "{}", unlinked.text);
    assert_eq!(
        unlinked.json["code"],
        "reobservation_authorization_not_linked"
    );
    let other_domain = post(app, &reobserve(data.reobserve, data.domain_two), None).await;
    assert_eq!(
        other_domain.status,
        StatusCode::NOT_FOUND,
        "{}",
        other_domain.text
    );
    assert_eq!(other_domain.json["code"], "material_not_found");
    let base = format!("/api/local/work-resources/{}/research-dossier", data.rich);
    let dossier = get(app, &format!("{base}?domain={domain}")).await;
    let image = dossier.json["dossier"]["materials"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "image")
        .unwrap();
    let review = format!("{base}/materials/{}/review", image["ref"].as_str().unwrap());
    let body = json!({"commandRef":Uuid::new_v4(),"unitRef":image["ref"],"sourceVersion":image["sourceVersion"],"expectedCurrentRevisionRef":image["currentRevisionRef"],"action":"edit","editedBlocks":image["blocks"],"actor":"synthetic","reason":"test boundary","verified":false,"resolvedIssueKeys":[]});
    let wrong_scope = post(
        app,
        &format!("{review}?domain={}", data.domain_two),
        Some(body.clone()),
    )
    .await;
    assert_eq!(
        wrong_scope.status,
        StatusCode::NOT_FOUND,
        "{}",
        wrong_scope.text
    );
    let mut extra = body;
    extra["extra"] = json!(1);
    let invalid = post(app, &format!("{review}?domain={domain}"), Some(extra)).await;
    assert!(invalid.status.is_client_error());
    let headless = send(
        app.clone(),
        "POST",
        &reobserve(data.reobserve, domain),
        &[],
        None,
    )
    .await;
    assert_eq!(headless.status, StatusCode::FORBIDDEN);
    assert_eq!(
        (reobservation_counts(db).await, reading_rows(db).await),
        before,
        "被拒的写入不改库"
    );
}

/// 两条写路径在数据库里的真实后果（隔离库，合成数据）。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "isolated PostgreSQL proof; run by scripts/test-web-app-postgres.sh"]
async fn write_paths_leave_exactly_the_expected_database_records() {
    let db = isolated("ev_writes").await;
    let data = seed_dataset(&db).await;
    let app = app_with_database(db.clone());
    reobservation_merges_and_never_leases(&db, &app, &data).await;
    rejected_writes_change_nothing(&db, &app, &data).await;
}
