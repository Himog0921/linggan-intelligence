//! 关键词建档的真实链路证明：**先拿链接，再补详情**。
//!
//! 「建过档没有」对关键词是查出来的，不是存下来的状态（`0042` 的 CHECK 禁止关键词进入
//! `archiving`/`archived`）。本文件锁住那条判据本身，以及建档第二段——把链接换成详情
//! ——的推进条件。
//!
//! 这些用例刻意走产品代码自己的入口（`request_and_admit`、`advance_keyword_archive_detail`），
//! 不用 SQL 抄近路造工单：此前本仓库全部用例都抄近路，于是「建档请求在写工单那一步必然
//! 撞上 `0042` 的 CHECK」这个 100% 必现的缺陷被 14 条绿测试整齐漏掉。

#[path = "support/domain_fixture.rs"]
mod domain;
#[path = "support/material_fixture.rs"]
mod fixture;

use domain::{
    ADHD_DOMAIN, FIXTURE_QUOTA, PEER_DOMAIN, discovery_card, search_coverage,
    submit_package_for_domain, submit_package_for_target, submit_peer_domain_package,
};
use fixture::proof_database;
use linggan_evidence::{
    AccountEligibilityObservation, CatalogDetailState, CheckInOutcome, DETAIL_WINDOW_COMMENT_LIMIT,
    DETAIL_WINDOW_REPLY_EXPAND_LIMIT, DispatchDecision, InstallationCheckIn, KeywordDetailAdvance,
    MaterialExecutionKind, activate_installation_credential, advance_keyword_archive_detail,
    bind_observation_account, check_in_installation, decide_dispatch, keyword_baselines_qualified,
    keyword_targets_pending_detail, open_claim_window, read_keyword_hits, register_station,
    report_account_eligibility, request_and_admit, set_station_accepting,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一轮把搜索面翻到底、且没有一条材料被隔离的建档，应当被判为**已建档**。
///
/// 关键词没有建档生命周期（`0042` 的 CHECK 禁止它进入 `archiving`/`archived`），所以
/// 「建过档没有」是读取时从证据里查出来的，不是存下来的状态。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_that_scanned_its_surface_to_the_bottom_counts_as_archived() {
    let database = proof_database("keyword_archive_completes").await;
    let target_ref =
        submit_keyword_archive(&database, "考研自习::archive-ok", "bottom_confirmed", 1, 0).await;

    let qualified = keyword_baselines_qualified(&database, &[target_ref])
        .await
        .expect("baseline query runs")
        .contains(&target_ref);
    assert!(qualified, "翻到底且无隔离的一轮就是建档");
}

/// 历史巡查状态只允许修复**缺失**的搜索面。已经有合格搜索面的词即使尚欠详情，也必须
/// 从具体作品的详情补采继续；不能借「补档」把整张搜索面重新跑一遍。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn legacy_patrol_states_cannot_repeat_a_qualified_keyword_search_baseline() {
    for (lifecycle_state, identity_key) in [
        ("monitoring", "考研自习::repeat-baseline-monitoring"),
        ("paused", "考研自习::repeat-baseline-paused"),
    ] {
        let database = proof_database(&format!("keyword_archive_repeat_{lifecycle_state}")).await;
        let target_ref =
            submit_keyword_archive(&database, identity_key, "bottom_confirmed", 1, 0).await;
        sqlx::query(
            "UPDATE collection_observation_target SET lifecycle_state=$2 WHERE target_ref=$1",
        )
        .bind(target_ref)
        .bind(lifecycle_state)
        .execute(database.pool())
        .await
        .expect("the historical patrol state is stored");
        let before: i64 =
            sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
                .bind(target_ref)
                .fetch_one(database.pool())
                .await
                .expect("the existing archive work order is readable");

        let repeated = request_and_admit(
            &database,
            target_ref,
            "deep_archive",
            "不得重复第一阶段关键词建档",
            "person",
        )
        .await;
        assert!(
            matches!(
                repeated,
                Err(linggan_evidence::AcquisitionChainError::TargetNotRequestable { .. })
            ),
            "{lifecycle_state} + 合格搜索面不得重跑第一阶段，实际：{repeated:?}"
        );
        let after: i64 =
            sqlx::query_scalar("SELECT count(*) FROM collection_work_order WHERE target_ref=$1")
                .bind(target_ref)
                .fetch_one(database.pool())
                .await
                .expect("the work-order count is readable");
        assert_eq!(after, before, "拒绝不得新建重复搜索工单");
    }
}

/// 中途因为失败而停下的那一轮不算建档。
///
/// 把它算作完成，等于宣布一个没挖完的词已经建好档——之后所有基于它的判断都建立在一个
/// 不完整的底座上，而且没人看得出来。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_archive_that_stopped_on_failures_does_not_count() {
    let database = proof_database("keyword_archive_incomplete").await;
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::archive-failed",
        "bottom_confirmed",
        1,
        2,
    )
    .await;

    let qualified = keyword_baselines_qualified(&database, &[target_ref])
        .await
        .expect("baseline query runs")
        .contains(&target_ref);
    assert!(!qualified, "有失败的一轮不是建档，还得接着补");
}

/// 没翻到底、也没采满配额的那一轮不算建档，哪怕一条失败都没有。
///
/// 这条与「有失败就不算」分开测是必要的：完整性判据读的是包的 `checkpoint`，而
/// coverage 里另有一个名字很像的 `stoppedReason`。写这组用例时就踩过——停止原因放错了
/// 地方，判据整条落空，负例却因为**失败数非零**照样变红，看上去一切正常。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_archive_that_never_reached_the_bottom_does_not_count() {
    let database = proof_database("keyword_archive_not_bottom").await;
    // 一条失败都没有，但停在半路：既没 bottom_confirmed，也没采满 maximumQuota。
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::archive-partial",
        "time_budget_exhausted",
        1,
        0,
    )
    .await;

    let qualified = keyword_baselines_qualified(&database, &[target_ref])
        .await
        .expect("baseline query runs")
        .contains(&target_ref);
    assert!(!qualified, "没翻到底又没采满，就还不是建档");
}

/// 巡检拿回来的包再完整，也不算建档——建档是另一条通道上的事。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_complete_patrol_round_is_not_an_archive() {
    let database = proof_database("keyword_patrol_is_not_archive").await;
    submit_peer_domain_package(
        &database,
        "考研自习::patrol-only",
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","topByLikes":20,"scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card(
            "note-patrol-only",
            "巡检样本",
            "233",
            "https://www.xiaohongshu.com/search_result/note-patrol-only?xsec_token=ABpatrol",
        )],
    )
    .await;
    let target_ref: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
    )
    .bind("考研自习::patrol-only")
    .fetch_one(database.pool())
    .await
    .unwrap();

    let qualified = keyword_baselines_qualified(&database, &[target_ref])
        .await
        .expect("baseline query runs")
        .contains(&target_ref);
    assert!(!qualified, "巡检不是建档，哪怕这一轮本身很完整");
}

/// 造一轮关键词建档：发一张 deep_archive 工单并提交一个搜索包。
///
/// `failed` 非零时模拟「中途停下」的那一轮。
async fn submit_keyword_archive(
    database: &Database,
    identity_key: &str,
    stop_reason: &str,
    acquired: i64,
    failed: i64,
) -> Uuid {
    submit_keyword_archive_of(
        database,
        identity_key,
        "note-archive-1",
        stop_reason,
        acquired,
        failed,
    )
    .await
}

/// 同上一轮建档，但这一轮采回来的是**指定的那一篇**。
///
/// 样本是按作品身份去重的，所以同一个 `externalId` 采几轮都只落同一行样本。要造出两篇
/// 互不相同的参照物，只能在包里提交两个不同的 `externalId`——换目标、换关键词、再采一轮
/// 都换不出第二行来。
async fn submit_keyword_archive_of(
    database: &Database,
    identity_key: &str,
    note_external_id: &str,
    stop_reason: &str,
    acquired: i64,
    failed: i64,
) -> Uuid {
    submit_keyword_archive_of_url(
        database,
        identity_key,
        note_external_id,
        stop_reason,
        acquired,
        failed,
        &format!(
            "https://www.xiaohongshu.com/search_result/{note_external_id}?xsec_token=ABarchive"
        ),
    )
    .await
}

/// 同上一轮建档，但这一轮平台返回的是**指定的那一条**链接。
///
/// 同一篇作品的样本行按身份去重（`domain_ref + platform + content_external_id`），所以再采
/// 一轮更新的是同一行的来源地址——「平台这次给的入口变了没有」正是这条链路上真实发生的事，
/// 而它在夹具里只能靠换这一条链接来表达。
async fn submit_keyword_archive_of_url(
    database: &Database,
    identity_key: &str,
    note_external_id: &str,
    stop_reason: &str,
    acquired: i64,
    failed: i64,
    url: &str,
) -> Uuid {
    let (coverage, checkpoint, records) =
        keyword_archive_round(note_external_id, stop_reason, acquired, failed, url);
    submit_peer_domain_package(
        database,
        identity_key,
        "deep_archive",
        // 建档口径：不限时间、不设取前 N。
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":10}),
        "discovery_search",
        coverage,
        checkpoint,
        records,
    )
    .await;
    let target_ref: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
    )
    .bind(identity_key)
    .fetch_one(database.pool())
    .await
    .expect("the archived target exists");
    // This helper represents a completed accepted search round. Its synthetic producer chain
    // does not run the normal final-task transition, so close the unscoped search WorkOrders it
    // seeded; otherwise dispatch proofs can be blocked by a stale discovery task while testing
    // the later material-detail WorkOrder.
    sqlx::query(
        "UPDATE collection_work_order work_order SET queue_state='completed' \
         WHERE work_order.target_ref=$1 AND work_order.queue_state='queued' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                          WHERE scope.work_order_ref=work_order.work_order_ref)",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the submitted search round is terminal in the proof fixture");
    target_ref
}

/// 同一个目标上的**下一轮**建档：目标是既有的，不能再建一次。
///
/// 「平台这次给的是另一条链接」只可能发生在**同一个目标**上。`submit_peer_domain_package`
/// 每一轮都从建目标开始，而 `collection_observation_target` 上「平台+类型+身份」是唯一的：
/// 拿它再采一轮，撞的是目标身份，不是链接——换一个目标则更糟，那不是「同一篇又采到一次」，
/// 而是另一篇，按 `target_ref` 关联的判据会静默落空、让用例因为错误的原因变绿。
///
/// `station_key` 只用来拼证明工位的显示名：`execution_station` 有一条「同名工位只能有一个」
/// 的唯一索引（未退休者为限），而这一路每一轮都要登记一次工位。
async fn submit_keyword_archive_round(
    database: &Database,
    target_ref: Uuid,
    station_key: &str,
    note_external_id: &str,
    stop_reason: &str,
    acquired: i64,
    failed: i64,
    url: &str,
) {
    let (coverage, checkpoint, records) =
        keyword_archive_round(note_external_id, stop_reason, acquired, failed, url);
    submit_package_for_target(
        database,
        target_ref,
        station_key,
        "deep_archive",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":10}),
        FIXTURE_QUOTA,
        "discovery_search",
        coverage,
        checkpoint,
        records,
    )
    .await;
    sqlx::query(
        "UPDATE collection_work_order work_order SET queue_state='completed' \
         WHERE work_order.target_ref=$1 AND work_order.queue_state='queued' \
           AND NOT EXISTS (SELECT 1 FROM collection_work_order_material_target scope \
                          WHERE scope.work_order_ref=work_order.work_order_ref)",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the submitted search round is terminal in the proof fixture");
}

/// 一轮关键词建档的包体：覆盖率（含停在什么地方）、检查点与发现记录。
///
/// 抽出来只为一件事：把「第一个目标」和「同一个目标的下一轮」分开以后，两边的包体仍然
/// 逐字相同。两边各写一份，任何一边漂了，比的就是两个不同的包。
fn keyword_archive_round(
    note_external_id: &str,
    stop_reason: &str,
    acquired: i64,
    failed: i64,
    url: &str,
) -> (serde_json::Value, serde_json::Value, Vec<serde_json::Value>) {
    let mut coverage = search_coverage("考研自习", acquired);
    coverage["layers"][0]["failed"] = serde_json::json!(failed);
    coverage["layers"][0]["observed"] = serde_json::json!(acquired + failed);
    coverage["layers"][0]["attempted"] = serde_json::json!(acquired + failed);
    coverage["layers"][0]["stoppedReason"] = serde_json::json!(stop_reason);
    (
        coverage,
        serde_json::json!({"surfaceReceipt":{"stopReason":stop_reason}}),
        vec![discovery_card(note_external_id, "建档样本", "1.4万", url)],
    )
}

/// 界面上「这一篇此刻能不能取详情、不能时欠的是什么」——走的是页面用的那个读取口。
///
/// 断言不在这里另拼一句 SQL 去近似它：另拼的句子永远会对，而页面上那一行可以同时在说别的
/// 话。读同一个投影，才是在断言用户看得见的那件事。
async fn hit_display_state(
    database: &Database,
    target_ref: Uuid,
    content_external_id: &str,
) -> (CatalogDetailState, Option<MaterialExecutionKind>) {
    let projection = read_keyword_hits(
        database,
        target_ref,
        Uuid::parse_str(PEER_DOMAIN).expect("the fixture Domain is a UUID"),
    )
    .await
    .expect("the Domain-scoped keyword hit list stays readable")
    .expect("the selected Domain has a discovery surface");
    let work = projection
        .works
        .iter()
        .find(|work| work.content_external_id == content_external_id)
        .unwrap_or_else(|| {
            panic!(
                "{content_external_id} 应当在这张榜上；实际是 {:?}",
                projection
                    .works
                    .iter()
                    .map(|work| work.content_external_id.as_str())
                    .collect::<Vec<_>>()
            )
        });
    (
        work.detail_state,
        work.execution_state.as_ref().map(|state| state.kind),
    )
}

struct DetailStation {
    install_key: String,
    credential: Option<String>,
}

/// Create an idle detail-capable station whose plugin is authenticated and bound to an account.
async fn ready_detail_station(
    database: &Database,
    label: &str,
    capabilities: serde_json::Value,
) -> DetailStation {
    let station_ref = register_station(database, label, 200)
        .await
        .expect("station is registered");
    open_claim_window(database, station_ref, 1)
        .await
        .expect("station claim window opens");
    let install_key = Uuid::new_v4().to_string();
    let outcome = check_in_installation(
        database,
        &InstallationCheckIn {
            install_key: &install_key,
            installation_credential: None,
            plugin_version: "0.8.48",
            browser_label: Some(label),
            capabilities,
            selector_health: None,
        },
    )
    .await
    .expect("installation checks in");
    let CheckInOutcome::Claimed {
        installation_ref,
        credential,
        ..
    } = outcome
    else {
        panic!("an open claim window must claim the installation; got {outcome:?}");
    };
    let credential = credential.map(|issued| {
        (
            issued.credential_ref,
            issued.raw_credential.expose_once().to_owned(),
        )
    });
    let credential = if let Some((credential_ref, raw)) = credential {
        activate_installation_credential(database, installation_ref, credential_ref, &raw)
            .await
            .expect("the proof installation activates its pending credential");
        Some(raw)
    } else {
        None
    };
    set_station_accepting(database, station_ref, true, "person")
        .await
        .expect("person enables station acceptance");
    let receipt = report_account_eligibility(
        database,
        installation_ref,
        credential
            .as_deref()
            .expect("the proof installation holds a credential"),
        AccountEligibilityObservation::Authenticated {
            raw_platform_account_id: "detail-dispatch-account",
        },
        Some(&[7_u8; 32]),
    )
    .await
    .expect("account eligibility is reported");
    bind_observation_account(
        database,
        receipt.account_ref.expect("account ref exists"),
        installation_ref,
        "person",
    )
    .await
    .expect("person binds the account");
    DetailStation {
        install_key,
        credential,
    }
}

async fn submit_home_domain_keyword_archive(database: &Database, identity_key: &str) -> Uuid {
    let mut coverage = search_coverage("学不进去", 4);
    coverage["layers"][0]["stoppedReason"] = serde_json::json!("bottom_confirmed");
    coverage["target"]["query"] = serde_json::json!("学不进去");
    submit_package_for_domain(
        database,
        ADHD_DOMAIN,
        identity_key,
        "deep_archive",
        serde_json::json!({"query":"学不进去","ranking":"most_liked","scrollRounds":10}),
        FIXTURE_QUOTA,
        "discovery_search",
        coverage,
        serde_json::json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        (1..=4)
            .map(|index| {
                discovery_card(
                    &format!("note-home-{index}"),
                    &format!("本领域建档样本 {index}"),
                    &format!("{}", index * 1000),
                    &format!("https://www.xiaohongshu.com/search_result/note-home-{index}?xsec_token=ABhome"),
                )
            })
            .collect(),
    )
    .await;
    sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key=$1")
        .bind(identity_key)
        .fetch_one(database.pool())
        .await
        .expect("the ADHD target exists")
}

/// 建档拿到链接之后，必须接着把详情补上。
///
/// 列表面只给得出标题、封面和点赞；正文、发布时间、评论都在详情页里。**博主的渐进式
/// 建档一直是两段（先目录、后逐篇），关键词此前只有第一段**，建完档的词停在一堆链接上，
/// 而链接本身回答不了这个词长什么样。
///
/// 去重发生在进语料库那一刻：包里采回多少条就留多少条，落库是 upsert，所以从样本表挑
/// 出来的候选天然一篇一条。这里连采两轮同一篇，候选仍然只有一份。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_archived_keyword_advances_from_links_to_details() {
    let database = proof_database("keyword_detail_advance").await;
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::detail-advance",
        "bottom_confirmed",
        1,
        0,
    )
    .await;

    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    let KeywordDetailAdvance::Queued {
        work_order_ref,
        works,
    } = advance
    else {
        panic!("翻到底的建档之后应当排出详情补采，实际是 {advance:?}");
    };
    assert_eq!(works, 1, "这一轮只采回一篇，就只该补一篇的详情");

    // 该 Domain 的目标材料使用规范 Content 身份进入冻结范围。
    let scoped: Vec<String> = sqlx::query_scalar(
        "SELECT content.content_external_id \
         FROM collection_work_order_material_target scope \
         JOIN linggan_material_content content ON content.public_ref=scope.content_public_ref \
         WHERE scope.work_order_ref=$1 ORDER BY scope.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("the canonical material scope is readable");
    assert_eq!(scoped, vec!["note-archive-1".to_owned()]);
    let evidence_scope: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_material_target WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(evidence_scope, 1);

    // 详情读的窗口冻在规范材料作用域上：详情 + 前 30 条评论 + 2 层回复。
    let policy: Vec<(i32, i32)> = sqlx::query_as(
        "SELECT scope.comment_limit,scope.reply_expand_limit \
         FROM collection_work_order_material_target scope \
         WHERE scope.work_order_ref=$1 ORDER BY scope.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("the frozen Domain material policy is readable");
    assert_eq!(
        policy,
        vec![(
            DETAIL_WINDOW_COMMENT_LIMIT,
            DETAIL_WINDOW_REPLY_EXPAND_LIMIT
        )],
        "peer Domain 的详情补采也必须冻上评论与回复的窗口"
    );

    // 已经排上的那一篇不会被第二次排进来——重复补采同一篇不产生新事实，只多花一次
    // 平台访问。
    let again = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the second advance runs");
    assert!(
        matches!(
            again,
            KeywordDetailAdvance::Skipped("detail_batch_in_flight")
        ),
        "在途的批次要如实说成在途，而不是「没有可继续的」：{again:?}"
    );
}

/// 已发现的材料仍欠详情时，baseline 覆盖不完整不能把它冻死。该覆盖事实继续为 false，
/// 但 detail continuation 必须独立可达；这覆盖历史任务已经切到 monitoring 的同一形状。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_incomplete_keyword_baseline_can_still_continue_discovered_details() {
    let database = proof_database("keyword_detail_continues_incomplete_baseline").await;
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::detail-after-incomplete-baseline",
        "time_budget_exhausted",
        1,
        0,
    )
    .await;
    assert!(
        !keyword_baselines_qualified(&database, &[target_ref])
            .await
            .unwrap()
            .contains(&target_ref),
        "前置：这一轮搜索覆盖仍然不完整"
    );
    assert!(
        keyword_targets_pending_detail(&database, &[target_ref])
            .await
            .unwrap()
            .contains(&target_ref),
        "前置：已发现作品仍缺详情"
    );
    assert!(
        matches!(
            advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
                .await
                .unwrap(),
            KeywordDetailAdvance::Queued { works: 1, .. }
        ),
        "详情 work 的可达性不得依赖 baseline 是否完整"
    );
}

/// **本领域的关键词也要能补详情。**
///
/// 关键词不只有外部领域那一种。本领域的关键词（ADHD 底下的「a娃」就是）采回来的材料按
/// `0044` 的隔离写进证据侧，跨行业样本表里一条都没有。此前候选只查跨行业那张表，于是
/// 本领域关键词永远「没有待补详情的」，补详情的入口对它们根本不出现——活只做了一半，
/// 而界面上看不出少了什么：它显示的是「查看结果」，像是已经做完了。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_home_domain_keyword_also_advances_to_details() {
    let database = proof_database("keyword_detail_home_domain").await;
    let target_ref = submit_home_domain_keyword_archive(&database, "ADHD::home-detail").await;

    // 批量判据要认得出它——列表页的「补采缺口」按钮就是由这一问决定的。
    let pending = keyword_targets_pending_detail(&database, &[target_ref])
        .await
        .expect("pending detail query runs");
    assert!(
        pending.contains(&target_ref),
        "本领域关键词的待补详情必须数得出来，否则界面上永远不会出现补详情的入口"
    );

    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    let KeywordDetailAdvance::Queued {
        work_order_ref,
        works,
    } = advance
    else {
        panic!("本领域关键词建完档之后同样该排出详情补采，实际是 {advance:?}");
    };
    assert_eq!(works, 3, "一批补三篇");

    // **按点赞从高到低挑**——与跨行业那一侧以及回执文案（「正在按点赞从高到低逐篇补」）
    // 一致。此前本领域这条按 `content_public_ref`（近似随机的 UUID）取，页面上写着一回事、
    // 实际做的是另一回事，而且看不出来。
    let picked: Vec<String> = sqlx::query_scalar(
        "SELECT content.content_external_id \
         FROM collection_work_order_material_target scope \
         JOIN linggan_material_content content ON content.public_ref=scope.content_public_ref \
         WHERE scope.work_order_ref=$1 ORDER BY scope.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("the evidence-side scope is readable");
    assert_eq!(
        picked,
        vec![
            "note-home-4".to_owned(),
            "note-home-3".to_owned(),
            "note-home-2".to_owned()
        ],
        "该先补点赞最高的三篇"
    );

    // 作用域落在所有 Domain 共用的规范材料表。
    let evidence_scope: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_material_target WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(evidence_scope, 3);

    // 三篇都按同一个窗口冻：正文 + 前 30 条评论 + 2 层回复 + 媒体。
    //
    // 三个媒体布尔**一起断言，不只断言 `acquire_media`**：关键词这条路此前三项全是 false
    // （2026-09-16 起与创作者观察同口径，Issue #296），只盯一个的话，另外两列被人改回去
    // 这条断言照样绿。
    let policy: Vec<(i32, i32, bool, bool, bool)> = sqlx::query_as(
        "SELECT scope.comment_limit,scope.reply_expand_limit, \
                scope.acquire_media,scope.allow_ocr,scope.allow_asr \
         FROM collection_work_order_material_target scope \
         WHERE scope.work_order_ref=$1 ORDER BY scope.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("the evidence-side policy is readable");
    assert_eq!(
        policy,
        vec![
            (
                DETAIL_WINDOW_COMMENT_LIMIT,
                DETAIL_WINDOW_REPLY_EXPAND_LIMIT,
                true,
                true,
                true
            );
            3
        ],
        "证据侧的详情补采与创作者观察同口径：正文 + 评论 + 回复 + 媒体三项授权"
    );
}

/// 基线停在**失败**上时，已经发现的那一篇仍然要能补详情。
///
/// 此前这里断言的是反面（`Skipped("archive_round_not_complete")`），用的就是这套夹具：它
/// 踩中那道闸门靠的是判据开头「这一轮有没有失败」（`failed=0`），而这里塞了 2 条失败。
/// 闸门已拆，且拆得有理由——它还读派发时冻进任务说明书的 `expectedCount`，早于该字段的
/// 一轮永远没有它，`COALESCE(...,2147483647)` 于是恒不成立：采满了也说没采满，详情永久
/// 排不进队列，而且一声不吭。
///
/// 覆盖不完整这件事没有被抹掉，只是不再连坐已经取得的原料：列表页的「建过档没有」照旧
/// 由关键词建档合格判据回答（`keyword_baselines_qualified`）。这里钉的是详情不再问它。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_archive_that_stopped_on_failures_still_continues_its_details() {
    let database = proof_database("keyword_detail_after_failed_baseline").await;
    let target_ref =
        submit_keyword_archive(&database, "考研自习::detail-early", "surface_error", 1, 2).await;

    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    assert!(
        matches!(advance, KeywordDetailAdvance::Queued { works: 1, .. }),
        "覆盖不完整不能连坐已经发现的那一篇：{advance:?}"
    );
}

/// 关键词的建档请求必须真的能走完准入、写出工单。
///
/// 这条用例补的是一个方法上的缺口：本文件其它用例都直接拼 SQL 造工单与租约，**从未走过
/// `request_and_admit` 这条真实链路**，于是没有一条能发现「建档请求在写工单那一步必然
/// 撞上 `0042` 的 CHECK」——`write_work_order` 里那条把目标推进 `archiving` 的 UPDATE
/// 原本不分目标类型，而关键词被禁止进入那个状态，整个事务因此回滚，页面只显示一句
/// 「上一次动作没有完成」。100% 必现，却被 14 条绿测试全部漏掉。
///
/// 所以这里不用夹具抄近路：建目标、签授权，然后调用产品代码自己的入口。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_archive_request_passes_admission_and_writes_a_work_order() {
    let database = proof_database("keyword_archive_admission").await;
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword','考研自习::most_liked','考研自习','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is stored");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES ($1::uuid,$2,'primary')",
    )
    .bind(PEER_DOMAIN)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target has an explicit Domain relation");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,max_works_per_target) \
         VALUES ($1,'xhs','keyword','deep_archive','关键词历史建档','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_archive','material_deepening'],ARRAY['immediate','batch'],200,200)",
    )
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .expect("keyword deep-archive authorization is granted");

    let outcome = request_and_admit(
        &database,
        target_ref,
        "deep_archive",
        "从观察目标页发起关键词历史建档",
        "person",
    )
    .await
    .expect("the archive request survives admission");
    assert!(
        outcome.work_order_ref.is_some(),
        "关键词建档请求必须写出工单，实际结论：{:?} / {}",
        outcome.outcome,
        outcome.reason_code
    );

    // 生命周期一步都没动：关键词不进 archiving，建过没建过从证据里查。
    let state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state, "pending_decision", "关键词不得被推进到建档状态");

    let lane: String =
        sqlx::query_scalar("SELECT lane FROM collection_work_order WHERE target_ref=$1")
            .bind(target_ref)
            .fetch_one(database.pool())
            .await
            .expect("the work order exists");
    assert_eq!(lane, "deep_archive");
}

/// 同一个请求对创作者仍然要推进到「建档中」——这条链没有被上面的修改弄坏。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_creator_archive_request_still_advances_the_lifecycle() {
    let database = proof_database("creator_archive_admission").await;
    let target_ref = Uuid::new_v4();
    // 本行业创作者，领域写实：采集准入会拒绝未归属领域的目标。
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','creator','69aad16e000000003201b172','木可可','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is stored");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES ($1::uuid,$2,'primary')",
    )
    .bind(ADHD_DOMAIN)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the creator belongs to the ADHD Domain");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,max_works_per_target) \
         VALUES ($1,'xhs','creator','deep_archive','创作者建档','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['creator_archive','material_deepening'],ARRAY['immediate','batch'],200,200)",
    )
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .expect("creator deep-archive authorization is granted");

    request_and_admit(
        &database,
        target_ref,
        "deep_archive",
        "创作者建档",
        "person",
    )
    .await
    .expect("the archive request survives admission");

    let state: String = sqlx::query_scalar(
        "SELECT lifecycle_state FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(state, "archiving", "创作者建档仍然推进生命周期");
}

/// 没归属领域的目标不得开始采集。
///
/// 目标本身允许先无领域地存在：浏览器上报（`collection_target_intake`）与插件推送
/// （`sync_target_from_author_profile`）都发现得到目标却不知道领域，让它们建候选是对的。
/// 但读取侧对未归属目标一律回落成本领域，**一旦真开始采，材料就会按那个猜测写进证据侧**
/// ——2026-09-11 的 413 条误写正是这条路径。所以闸设在「要花平台访问额度之前」。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_target_without_a_domain_cannot_start_acquiring() {
    let database = proof_database("acquisition_requires_domain").await;
    let target_ref = Uuid::new_v4();
    // 刻意不建 Domain relation：模拟尚未归属的浏览器/插件候选目标。
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword','数学思维::most_liked','数学思维','plugin_push')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("candidate target is stored");
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,max_works_per_target) \
         VALUES ($1,'xhs','keyword','deep_archive','领域闸门证明','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_archive','material_deepening'],ARRAY['immediate','batch'],200,200)",
    )
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .expect("authorization is granted");

    let refused = request_and_admit(
        &database,
        target_ref,
        "deep_archive",
        "领域闸门证明",
        "person",
    )
    .await;
    assert!(
        matches!(
            refused,
            Err(linggan_evidence::AcquisitionChainError::TargetDomainUnassigned)
        ),
        "未归属领域的目标不该开始采集，实际：{refused:?}"
    );

    // 授权是齐的——挡住它的必须是领域这一项，不是别的条件顺带拦下的。
    let requests: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_acquisition_request WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(requests, 0, "连请求都不该落下，它还没到该被审的那一步");

    // 指定领域之后，同一个请求就该通过——证明闸挡的确实只是「没归属」。
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES ($1::uuid,$2,'primary')",
    )
    .bind(PEER_DOMAIN)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the Domain relation is assigned");
    let admitted = request_and_admit(
        &database,
        target_ref,
        "deep_archive",
        "领域闸门证明",
        "person",
    )
    .await
    .expect("the same request now survives admission");
    assert!(
        admitted.work_order_ref.is_some(),
        "补上领域之后应当放行，实际：{:?} / {}",
        admitted.outcome,
        admitted.reason_code
    );
}

/// 详情补采的工单必须真的能被一个工位领走，并且拿到一个**打得开的入口**。
///
/// 这条用例补的是一处方法上的缺口：上面那两条只证明到「工单写出来了、作用域表对了」。
/// 而这一段链路上有三个判断从来只认证据侧的表——派发挑候选时问「这张工单有没有冻结
/// 具体作品」、发租时把作用域展开成逐篇任务、派发前查带签名的执行入口。三处任何一处
/// 漏掉跨行业那一侧，工单都会安静地永远派不出去，或者派出去一个打不开的裸链接。
///
/// 本仓库已经栽过五次同一个形状：活做了，被某个判断无声挡住，而夹具的形状恰好绕开了
/// 真实形状。所以这里一路跑到工位真的拿到任务为止。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_detail_batch_is_claimable_and_carries_a_signed_entry_point() {
    let database = proof_database("keyword_detail_dispatch").await;
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::detail-dispatch",
        "bottom_confirmed",
        1,
        0,
    )
    .await;
    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    assert!(
        matches!(advance, KeywordDetailAdvance::Queued { .. }),
        "前置：详情补采要先排出一张工单，实际是 {advance:?}"
    );

    // **先钉住反例**：只会补详情的工位领不走这张工单。作用域行冻着 30 条评论与 2 层回复，
    // 一个读不了评论的工位接下来只会把评论永远欠着，而回执已经告诉人它在补——活看着有人
    // 接，结果少两样。换一个具备这三样能力的工位才该领走。
    let detail_only = ready_detail_station(
        &database,
        "只会补详情的工位",
        serde_json::json!(["content_detail"]),
    )
    .await;
    let refused = decide_dispatch(
        &database,
        &detail_only.install_key,
        detail_only
            .credential
            .as_deref()
            .expect("the proof installation holds a credential"),
    )
    .await
    .expect("dispatch decides");
    assert!(
        matches!(
            refused,
            DispatchDecision::ControlBlocked { ref reason_code }
                if reason_code == "capability_missing"
        ),
        "只有 content_detail 的工位不得领走一张冻了评论与回复的工单，\
         而它必须是因为能力不够被挡下的（不是没活、也不是别的理由），实际是 {refused:?}"
    );

    let installation = ready_detail_station(
        &database,
        "详情补采证明工位",
        serde_json::json!(["content_detail", "media_slots", "comments", "replies"]),
    )
    .await;
    let decision = decide_dispatch(
        &database,
        &installation.install_key,
        installation
            .credential
            .as_deref()
            .expect("the proof installation holds a credential"),
    )
    .await
    .expect("dispatch decides");
    let DispatchDecision::Dispatch {
        task_spec,
        execution_source_url,
        page_session_plan,
        lease_ref,
        ..
    } = decision
    else {
        panic!("具备详情能力的空闲工位必须能领走这张工单，实际是 {decision:?}");
    };

    // 一条任务只请求一个能力（任务规格合同），所以这里只能是这一串里的第一步。**工位够不够格
    // 是另一回事**：上面那段已经钉住「只有 content_detail 的工位被挡住」，说明门禁读的是
    // 作用域行推出来的三个能力，而不是「派出去的那一条任务写了什么」。把这两件事混成一条
    // 断言，就会得出一个永远不可能成立的期望。
    assert_eq!(
        task_spec["capabilitiesRequested"],
        serde_json::json!(["content_detail"])
    );
    assert_eq!(
        task_spec
            .pointer("/target/contentExternalId")
            .and_then(serde_json::Value::as_str),
        Some("note-archive-1")
    );
    // 冻结的窗口要真的展开成任务，而不是只体现在同一页计划里：同一篇材料冻结了媒体、30 条
    // 评论和 2 层回复，这一单就该排出四步，且每一步都只请求一种能力。
    let steps: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT task.task_spec->'capabilitiesRequested'->>0, task.task_spec->>'commentLimit', \
                (task.task_spec->>'maximumQuota')::integer \
         FROM collection_work_order_lease_task step \
         JOIN linggan_runtime_task task USING(task_id) \
         WHERE step.lease_ref=$1 ORDER BY step.sequence_no",
    )
    .bind(lease_ref)
    .fetch_all(database.pool())
    .await
    .expect("the leased steps are readable");
    assert_eq!(
        steps,
        vec![
            ("content_detail".to_owned(), "not_requested".to_owned(), 1),
            ("media_slots".to_owned(), "not_requested".to_owned(), 1),
            (
                "comments".to_owned(),
                DETAIL_WINDOW_COMMENT_LIMIT.to_string(),
                DETAIL_WINDOW_COMMENT_LIMIT
            ),
            (
                "replies".to_owned(),
                DETAIL_WINDOW_COMMENT_LIMIT.to_string(),
                DETAIL_WINDOW_COMMENT_LIMIT
            ),
        ],
        "统一详情补采要按作用域行排出「详情 + 媒体 + 前 30 条评论 + 2 层回复」四步"
    );
    // 入口必须是列表面当时平台返回的那条带签名的链接，不能退化成裸 `/explore/{id}`。
    assert_eq!(
        execution_source_url.as_deref(),
        Some("https://www.xiaohongshu.com/search_result/note-archive-1?xsec_token=ABarchive"),
        "详情任务必须带上跨行业样本记下的签名链接"
    );
    // 同一次开页的服务范围从统一的材料 scope 读取，确保插件按冻结授权读评论与回复。
    let plan = page_session_plan.expect("跨行业的详情任务也要带上同页计划");
    assert_eq!(plan["contractVersion"], "linggan.detail-page-session.v1");
    assert_eq!(plan["contentExternalId"], "note-archive-1");
    assert_eq!(
        plan["lanes"],
        serde_json::json!(["content_detail", "media_slots", "comments", "replies"])
    );
    assert_eq!(plan["commentLimit"], DETAIL_WINDOW_COMMENT_LIMIT);
    assert_eq!(plan["replyExpandLimit"], DETAIL_WINDOW_REPLY_EXPAND_LIMIT);
}

/// 缺入口停下的一篇，后来真的拿到了新签名地址：经准入建立**后继资格**，旧的停止事实留着。
///
/// 这条用例钉的是「停止不是封禁」。只把停下来的那一篇挡在候选之外，会让它从「等一个更好的
/// 输入」变成「永久出局」——平台再给一条新链接也没人接。所以输入真的变了以后，准入要为这
/// 个缺口交还当前执行资格，并指回停过的那一条。
///
/// 「旧行原样留着」与「当前行接替」必须同时成立，缺一半都会说谎：只开新行不指回前驱，
/// 「这一次为什么又能跑了」就查不到了；改写旧行，则「当时确实没有可用的入口」这条历史事实
/// 被抹掉，看起来像它一直都有入口、只是没人跑。
///
/// **但不许换 epoch。** 换一条地址既不是故障修复证明也不是受控重新准入，让它开一个新 epoch
/// 就等于让「刷新一次签名 token」把跨工单的失败预算清零——同一篇作品于是可以靠不断换链接
/// 无限重试。所以断言里除了「当前资格回到 eligible」，还有一条同样重要的反面：`retry_epoch`
/// 仍是 0，而且同一时刻只有一条可执行资格。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_new_signed_address_after_a_stop_opens_a_successor_eligibility_through_admission() {
    let database = proof_database("keyword_detail_input_successor").await;
    let target_ref = submit_keyword_archive(
        &database,
        "考研自习::input-successor",
        "bottom_confirmed",
        1,
        0,
    )
    .await;

    // 前置：这一篇此刻打得开，所以详情补采为它排得出工单，作用域冻住它。
    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    // 停止发生在**这一张**工单上。不能用「这个目标最新的一张工单」代替：同一篇的每一轮
    // 建档都会各自留下一张工单，先按时间再按算子挑出来的那一张不一定是详情这张，
    // 于是断言会对着另一张说「它没被取消」——红的理由与被测行为无关。
    let KeywordDetailAdvance::Queued {
        works: 1,
        work_order_ref: detail_order,
        ..
    } = advance
    else {
        panic!("前置不成立：这一篇要先真的排进一张详情工单，实际是 {advance:?}");
    };

    // 平台这一轮返回的是一条**没有签名**的链接。这一篇还在作用域里，但此刻没有可用入口。
    submit_keyword_archive_round(
        &database,
        target_ref,
        "考研自习::input-successor#unsigned",
        "note-archive-1",
        "bottom_confirmed",
        1,
        0,
        "https://www.xiaohongshu.com/explore/note-archive-1",
    )
    .await;

    let station = ready_detail_station(
        &database,
        "缺输入后继证明工位",
        serde_json::json!(["content_detail", "media_slots", "comments", "replies"]),
    )
    .await;
    let decision = decide_dispatch(
        &database,
        &station.install_key,
        station
            .credential
            .as_deref()
            .expect("the proof installation holds a credential"),
    )
    .await
    .expect("dispatch decides");
    assert!(
        matches!(decision, DispatchDecision::NothingWaiting),
        "没有可用入口的那一篇不该派给工位，实际是 {decision:?}"
    );

    // 停下的这一篇在榜上要说出**它欠的是什么**：不是「读失败」（它一次页面都没打开过），也
    // 不是「待采集」（那等于什么都不知道）。界面读的就是这一个值。
    assert_eq!(
        hit_display_state(&database, target_ref, "note-archive-1").await,
        (
            CatalogDetailState::Pending,
            Some(MaterialExecutionKind::InputBlocked)
        ),
        "缺输入停下的作品，行上说的是「输入不可执行」"
    );

    let stopped: (String, Option<String>, i32) = sqlx::query_as(
        "SELECT state,input_fingerprint,retry_epoch \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_kind='material_content' \
           AND capability='content_detail'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the stopped range is on the eligibility ledger");
    assert_eq!(stopped.0, "input_blocked", "停下这件事要记在台账上");
    assert_eq!(
        stopped.1, None,
        "「当初连一条地址都没有」是一条真实观察到的事实，不是一个待补的默认值"
    );
    assert_eq!(stopped.2, 0, "第一次停不增加 epoch");

    let work_order_state: String =
        sqlx::query_scalar("SELECT queue_state FROM collection_work_order WHERE work_order_ref=$1")
            .bind(detail_order)
            .fetch_one(database.pool())
            .await
            .expect("the detail work order stays readable");
    assert_eq!(
        work_order_state, "cancelled",
        "一篇都没取回：既不能记成完成，也不能留在队列里每一轮空转一次"
    );

    // 平台又给了一条**新的**带签名链接。走产品自己的入口推进，而不是靠人再点一次。
    submit_keyword_archive_round(
        &database,
        target_ref,
        "考研自习::input-successor#fresh",
        "note-archive-1",
        "bottom_confirmed",
        1,
        0,
        "https://www.xiaohongshu.com/search_result/note-archive-1?xsec_token=ABfresh",
    )
    .await;
    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs again");
    assert!(
        matches!(advance, KeywordDetailAdvance::Queued { works: 1, .. }),
        "新地址到了，这一篇要重新排得上；实际是 {advance:?}"
    );

    // 停止行还留在台账上，但它已经**不是此刻的答案**：界面读到的必须是「可执行」。把停止行
    // 当成永久封禁，正是这次要修的那种缺陷的形状——从前是每一轮都重认一次「输入又变了」，
    // 反过来就成了永远不再排。两种都错。
    assert_eq!(
        hit_display_state(&database, target_ref, "note-archive-1").await,
        (
            CatalogDetailState::Pending,
            Some(MaterialExecutionKind::Executable)
        ),
        "新地址到了之后这一篇读到的就是可执行：停止行是历史，不是永久封禁"
    );

    // 停止行与当前行**同时存在**，各自说各自的事实：一条说「当初确实没有入口」，一条说
    // 「此刻这条地址可以执行」。所以按 state 取行，不按「第几行」——顺序在这里没有意义。
    let rows: Vec<(Uuid, i32, String, Option<Uuid>, Option<String>)> = sqlx::query_as(
        "SELECT eligibility_ref,retry_epoch,state,successor_eligibility_ref,input_fingerprint \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_kind='material_content' \
           AND capability='content_detail'",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await
    .expect("the ledger stays readable");
    assert_eq!(
        rows.len(),
        2,
        "留一条停止事实、再有一条当前资格；不是把旧行改写成「它一直都有地址」，\
         也不是同一时刻挂着两条可执行资格：{rows:?}"
    );
    let stopped_row = rows
        .iter()
        .find(|(_, _, state, _, _)| state == "input_blocked")
        .expect("the stop stays on the ledger");
    assert_eq!(stopped_row.1, 0, "停止事实仍记在它发生的那个 epoch 上");
    assert_eq!(
        stopped_row.4, None,
        "旧行的输入指纹保持「当时没有」，不随后来的发现被回填"
    );
    assert_eq!(
        stopped_row.3, None,
        "前驱不该被改写；「谁接替了它」写在接替的那一条上"
    );
    let live_row = rows
        .iter()
        .find(|(_, _, state, _, _)| state == "eligible")
        .expect("the reopened input leaves a currently executable eligibility");
    assert_eq!(
        live_row.1, 0,
        "换地址不换 epoch：换一条签名地址不是故障修复证明，不能靠它把失败预算清零"
    );
    assert!(
        live_row.4.is_some(),
        "当前行要带着这一次真正解析出来的地址指纹，否则下一轮无从判断输入有没有再变"
    );
    assert_eq!(
        live_row.3,
        Some(stopped_row.0),
        "接替的那一条要指回被它接替的停止行，「这一次为什么又能跑了」才查得到"
    );
    // 平台又给**同一条**地址：这不是一次新的执行机会，不该再多出一条当前资格。
    //
    // 要证明这一条，得让准入真的再走到判断那一步。上一轮排出的工单还在途时，产品入口会直接以
    // 「批次在途」作答（那也是一种正确），根本到不了这里——于是断言会永远绿，测的却不是幂等。
    // 所以先让这一篇**再失去一次地址**，把那张工单结束掉，再拿同一条地址推进。
    //
    // 为什么这条判据是必须的：停止行的指纹是空的，因此它**永远满足**「与此刻的指纹不同」——
    // 同一个缺口每准入一次都会被重新认成「输入又变了一次」。防重不能靠「这条前驱交接过了没
    // 有」，只能靠写入那一步：与当前行冲突时就地更新，写进去的还是同一份输入。少了它，同一篇
    // 会同时挂着两条可执行资格，被两个 scheduler 各派一张工单。
    submit_keyword_archive_round(
        &database,
        target_ref,
        "考研自习::input-successor#unsigned-again",
        "note-archive-1",
        "bottom_confirmed",
        1,
        0,
        "https://www.xiaohongshu.com/explore/note-archive-1",
    )
    .await;
    let decision = decide_dispatch(
        &database,
        &station.install_key,
        station
            .credential
            .as_deref()
            .expect("the proof installation holds a credential"),
    )
    .await
    .expect("dispatch decides");
    assert!(
        matches!(decision, DispatchDecision::NothingWaiting),
        "这一段的前置：地址又被收走时这一篇不该被派出去，实际是 {decision:?}"
    );
    submit_keyword_archive_round(
        &database,
        target_ref,
        "考研自习::input-successor#fresh-again",
        "note-archive-1",
        "bottom_confirmed",
        1,
        0,
        "https://www.xiaohongshu.com/search_result/note-archive-1?xsec_token=ABfresh",
    )
    .await;
    let advance_again =
        advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
            .await
            .expect("a second advance still answers");
    assert!(
        matches!(advance_again, KeywordDetailAdvance::Queued { works: 1, .. }),
        "同一条地址回来了，这一篇仍然排得上（它本来就可执行）；实际是 {advance_again:?}"
    );
    // 两条各自的「只此一条」：停止事实只有一个通道一条，当前资格每轮准入之后也只有一条。
    //
    // 后一条是有回填的：把 `ON CONFLICT` 从写入里去掉，这一句所在的用例立刻红（23505 唯一键
    // 冲突）。**前一条没有**——把身份键的 `NULLS NOT DISTINCT` 去掉，本用例照样全绿，因为
    // 「再停一次」这条路在它之前就被挡住了：发租时 `blocked_object_refs_in_transaction` 会把
    // 输入未变的停止成员排除在任务之外，同一篇根本不会再产生一个 `pending` 任务去触发第二次
    // 停止。所以这一句只守住「四通道各一条、不多不少」，不构成 `NULLS NOT DISTINCT` 的证明。
    let (stops_after, live_after): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE state='input_blocked'), \
                count(*) FILTER (WHERE state<>'input_blocked') \
         FROM collection_execution_input_eligibility \
         WHERE target_ref=$1 AND object_kind='material_content' \
           AND capability='content_detail'",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("the ledger stays readable");
    assert_eq!(
        stops_after, 1,
        "又停了一次，留的仍是同一条停止事实：缺地址这件事不该每轮攒一行"
    );
    assert_eq!(
        live_after, 1,
        "同一条地址再准入一次，不得再多一条当前资格——否则每跑一次 tick 就多一次「新的执行机会」，\
         同一篇作品会被并行派发"
    );
}

/// Frozen material scopes reject comment limits that authorize replies without comments.
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_material_scope_cannot_authorize_replies_without_comments() {
    let database = proof_database("keyword_material_scope_policy").await;
    let mut orders = Vec::new();
    for (identity, content_id) in [
        ("考研自习::policy-a", "note-policy-a"),
        ("考研自习::policy-b", "note-policy-b"),
        ("考研自习::policy-c", "note-policy-c"),
    ] {
        let target_ref =
            submit_keyword_archive_of(&database, identity, content_id, "bottom_confirmed", 1, 0)
                .await;
        let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
            .await
            .expect("the detail advance runs");
        let KeywordDetailAdvance::Queued { work_order_ref, .. } = advance else {
            panic!("a missing detail should create its bounded WorkOrder: {advance:?}");
        };
        orders.push(work_order_ref);
    }

    let mut contents = Vec::new();
    for work_order_ref in &orders {
        let content_ref: Uuid = sqlx::query_scalar(
            "SELECT content_public_ref FROM collection_work_order_material_target \
             WHERE work_order_ref=$1",
        )
        .bind(work_order_ref)
        .fetch_one(database.pool())
        .await
        .expect("each WorkOrder freezes one canonical Content");
        contents.push(content_ref);
    }

    sqlx::query(
        "INSERT INTO collection_work_order_material_target \
             (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit) \
         VALUES ($1,$2,2,$3,$4)",
    )
    .bind(orders[0])
    .bind(contents[1])
    .bind(DETAIL_WINDOW_COMMENT_LIMIT)
    .bind(DETAIL_WINDOW_REPLY_EXPAND_LIMIT)
    .execute(database.pool())
    .await
    .expect("a valid second scope row is accepted");

    for (work_order_ref, content_ref, comment_limit, reply_expand_limit, constraint) in [
        (
            orders[1],
            contents[0],
            0,
            2,
            "collection_work_order_material_target_reply_requires_comment_ch",
        ),
        (
            orders[2],
            contents[1],
            31,
            0,
            "collection_work_order_material_target_comment_limit_check",
        ),
    ] {
        let refusal = sqlx::query(
            "INSERT INTO collection_work_order_material_target \
                 (work_order_ref,content_public_ref,ordinal,comment_limit,reply_expand_limit) \
             VALUES ($1,$2,2,$3,$4)",
        )
        .bind(work_order_ref)
        .bind(content_ref)
        .bind(comment_limit)
        .bind(reply_expand_limit)
        .execute(database.pool())
        .await
        .expect_err("the invalid material-scope row must be rejected");
        assert!(
            refusal.to_string().contains(constraint),
            "expected {constraint} to reject the scope, got {refusal}"
        );
    }
}

/// **准入直接报错时，调度要如实说出是哪一种。**
///
/// 此前调度对准入的直接报错只有一句 `Err(_) => "target_not_requestable"`：一个字符串吞掉了
/// 「目标还没归属领域」「schema 没装」「目标不存在」「授权额度不够」全部情况。界面只写
/// 「目标不可请求」，而真实原因是没有领域——排查时为此多花了两轮。
///
/// **一个压平的原因码比没有原因码更坏：它看起来是个答案。**
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_scheduler_tick_names_the_actual_admission_failure() {
    let database = proof_database("scheduler_failure_reason").await;
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) \
         VALUES ($1,'xhs','creator','creator-scheduler-reason','调度原因样本','manual','pending_decision')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target fixture is stored");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES ($1::uuid,$2,'primary')",
    )
    .bind(ADHD_DOMAIN)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target is assigned before saving a rule");
    // 规则得先立起来（保存规则本身要求目标有领域），之后再把领域摘掉——模拟历史上那些
    // 先被发现、领域还没定的目标。
    let saved = linggan_evidence::apply_monitor_rule_command(
        &database,
        &linggan_evidence::MonitorRuleCommand {
            target_ref,
            expected_revision: 0,
            idempotency_key: Uuid::new_v4(),
            kind: linggan_evidence::MonitorCommandKind::SaveRule,
            actor: linggan_evidence::MonitorCommandActor::Person,
            source: "targets_ui",
            slot_key: None,
            draft: Some(linggan_evidence::MonitorRuleDraft {
                mode: linggan_evidence::MonitorRuleMode::Fixed,
                automatic_enabled: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start_minute: None,
                window_end_minute: None,
                fixed_interval_seconds: Some(86_400),
                fallback_interval_seconds: 86_400,
                surface_key: "creator_patrol".to_owned(),
                ranking_key: None,
                scroll_rounds: None,
                top_by_likes: None,
                published_within_days: None,
                task_contract_version: "linggan.producer.task-spec.v1".to_owned(),
            }),
        },
    )
    .await
    .expect("the rule command runs");
    assert_eq!(
        saved.outcome,
        linggan_evidence::MonitorCommandOutcomeKind::Applied
    );
    // 授权签齐，这样挡住它的只剩领域那一道闸——否则测试会因为「没有授权」而变绿，
    // 证明不了任何关于原因码的事。
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units,max_works_per_target) \
         VALUES (gen_random_uuid(),'xhs','creator','patrol','定时巡检','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['creator_patrol'],ARRAY['immediate','scheduled'],200,200)",
    )
    .execute(database.pool())
    .await
    .expect("authorization is granted");
    sqlx::query("DELETE FROM observation_domain_target WHERE target_ref=$1")
        .bind(target_ref)
        .execute(database.pool())
        .await
        .expect("the target is left without an observation Domain");
    sqlx::query(
        "UPDATE collection_monitor_rule \
         SET monitor_next_run_at=scope_001_now()-interval '1 second' WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the rule is due");

    linggan_evidence::run_due_patrols(&database)
        .await
        .expect("the scheduler tick completes");
    let reason: String = sqlx::query_scalar(
        "SELECT reason_code FROM collection_scheduler_target_decision \
         WHERE target_ref=$1 ORDER BY decided_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .expect("a per-target decision is durable");
    assert_eq!(
        reason, "target_domain_unassigned",
        "挡住它的是「还没说清属于哪个领域」，不是一句笼统的「目标不可请求」"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn recovery_phase_skips_keyword_candidate_scan() {
    const SCHEMA: &str = "keyword_recovery_scan";
    if let Ok(target_ref) = std::env::var("KEYWORD_RECOVERY_SCAN_CHILD") {
        let database = Database::connect_within_schema(
            &std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap(),
            SCHEMA,
        )
        .await
        .unwrap();
        let summary = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            linggan_evidence::run_keyword_archive_details(&database, "proof"),
        )
        .await
        .expect("recovery must not wait for the locked candidate table")
        .unwrap();
        assert!(summary.queued.is_empty());
        assert!(summary.skipped.contains(&(
            Uuid::parse_str(&target_ref).unwrap(),
            "collection_upgrade_recovery_only".to_owned(),
        )));
        return;
    }
    let database = proof_database(SCHEMA).await;
    let target_ref =
        submit_keyword_archive(&database, "recovery-scan-proof", "bottom_confirmed", 1, 0).await;
    let mut blocker = database.pool().begin().await.unwrap();
    sqlx::query("LOCK TABLE linggan_material_discovery_finding IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--ignored",
            "--exact",
            "recovery_phase_skips_keyword_candidate_scan",
        ])
        .env("LINGGAN_COLLECTION_UPGRADE_PHASE", "recovery")
        .env("KEYWORD_RECOVERY_SCAN_CHILD", target_ref.to_string())
        .output()
        .unwrap();
    blocker.rollback().await.unwrap();
    assert!(
        result.status.success(),
        "{} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_unsigned_work_cannot_borrow_another_works_signed_locator() {
    let database = proof_database("keyword_locator_correlation").await;
    submit_home_domain_keyword_archive(&database, "unrelated-home-signed").await;
    submit_keyword_archive_of(
        &database,
        "unrelated-sample-signed",
        "signed-sample",
        "bottom_confirmed",
        1,
        0,
    )
    .await;
    let mut borrowed = Vec::new();
    for (domain, identity) in [
        (ADHD_DOMAIN, "unsigned-home"),
        (PEER_DOMAIN, "unsigned-sample"),
    ] {
        let (coverage, checkpoint, records) = keyword_archive_round(
            identity,
            "bottom_confirmed",
            1,
            0,
            &format!("https://www.xiaohongshu.com/explore/{identity}"),
        );
        submit_package_for_domain(
            &database,
            domain,
            identity,
            "deep_archive",
            serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":10}),
            FIXTURE_QUOTA,
            "discovery_search",
            coverage,
            checkpoint,
            records,
        )
        .await;
        let target_ref: Uuid = sqlx::query_scalar(
            "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
        )
        .bind(identity)
        .fetch_one(database.pool())
        .await
        .unwrap();
        let pending = keyword_targets_pending_detail(&database, &[target_ref])
            .await
            .unwrap();
        if pending.contains(&target_ref) {
            borrowed.push(identity);
            continue;
        }
        assert!(!matches!(
            advance_keyword_archive_detail(&database, target_ref, "proof", "agent")
                .await
                .unwrap(),
            KeywordDetailAdvance::Queued { .. }
        ));
    }
    assert!(
        borrowed.is_empty(),
        "unsigned works borrowed unrelated locators: {borrowed:?}"
    );
}
