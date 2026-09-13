//! 关键词建档的真实链路证明：**先拿链接，再补详情**。
//!
//! 「建过档没有」对关键词是查出来的，不是存下来的状态（`0042` 的 CHECK 禁止关键词进入
//! `archiving`/`archived`）。本文件锁住那条判据本身，以及建档第二段——把链接换成详情
//! ——的推进条件。
//!
//! 这些用例刻意走产品代码自己的入口（`request_and_admit`、`advance_keyword_archive_detail`），
//! 不用 SQL 抄近路造工单：此前本仓库全部用例都抄近路，于是「建档请求在写工单那一步必然
//! 撞上 `0042` 的 CHECK」这个 100% 必现的缺陷被 14 条绿测试整齐漏掉。

#[path = "support/cross_industry_fixture.rs"]
mod cross_industry;
#[path = "support/material_fixture.rs"]
mod fixture;

use cross_industry::{
    EXTERNAL_DOMAIN, HOME_DOMAIN, discovery_card, search_coverage, submit_external_package,
    submit_package_in_domain,
};
use fixture::proof_database;
use linggan_evidence::{
    AccountEligibilityObservation, CheckInOutcome, DispatchDecision, InstallationCheckIn,
    KeywordDetailAdvance, activate_installation_credential, advance_keyword_archive_detail,
    bind_observation_account, check_in_installation, decide_dispatch, keyword_baseline_qualified,
    keyword_targets_pending_detail, open_claim_window, register_station,
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

    let mut tx = database.pool().begin().await.unwrap();
    let qualified = keyword_baseline_qualified(&mut tx, target_ref)
        .await
        .expect("baseline query runs");
    tx.commit().await.unwrap();
    assert!(qualified, "翻到底且无隔离的一轮就是建档");
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

    let mut tx = database.pool().begin().await.unwrap();
    let qualified = keyword_baseline_qualified(&mut tx, target_ref)
        .await
        .expect("baseline query runs");
    tx.commit().await.unwrap();
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

    let mut tx = database.pool().begin().await.unwrap();
    let qualified = keyword_baseline_qualified(&mut tx, target_ref)
        .await
        .expect("baseline query runs");
    tx.commit().await.unwrap();
    assert!(!qualified, "没翻到底又没采满，就还不是建档");
}

/// 巡检拿回来的包再完整，也不算建档——建档是另一条通道上的事。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_complete_patrol_round_is_not_an_archive() {
    let database = proof_database("keyword_patrol_is_not_archive").await;
    submit_external_package(
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

    let mut tx = database.pool().begin().await.unwrap();
    let qualified = keyword_baseline_qualified(&mut tx, target_ref)
        .await
        .expect("baseline query runs");
    tx.commit().await.unwrap();
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
    let mut coverage = search_coverage("考研自习", acquired);
    coverage["layers"][0]["failed"] = serde_json::json!(failed);
    coverage["layers"][0]["observed"] = serde_json::json!(acquired + failed);
    coverage["layers"][0]["attempted"] = serde_json::json!(acquired + failed);
    coverage["layers"][0]["stoppedReason"] = serde_json::json!(stop_reason);
    submit_external_package(
        database,
        identity_key,
        "deep_archive",
        // 建档口径：不限时间、不设取前 N。
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":10}),
        "discovery_search",
        coverage,
        serde_json::json!({"surfaceReceipt":{"stopReason":stop_reason}}),
        vec![discovery_card(
            "note-archive-1",
            "建档样本",
            "1.4万",
            "https://www.xiaohongshu.com/search_result/note-archive-1?xsec_token=ABarchive",
        )],
    )
    .await;
    sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key=$1")
        .bind(identity_key)
        .fetch_one(database.pool())
        .await
        .expect("the archived target exists")
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

    // 作用域落在跨行业样本上，而不是证据侧那张表——参照物不进证据库。
    let scoped: Vec<String> = sqlx::query_scalar(
        "SELECT sample.content_external_id \
         FROM collection_work_order_cross_industry_target scope \
         JOIN cross_industry_sample sample USING(sample_ref) \
         WHERE scope.work_order_ref=$1 ORDER BY scope.ordinal",
    )
    .bind(work_order_ref)
    .fetch_all(database.pool())
    .await
    .expect("the cross-industry scope is readable");
    assert_eq!(scoped, vec!["note-archive-1".to_owned()]);
    let evidence_scope: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_material_target WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(evidence_scope, 0, "跨行业参照物不得写进证据侧的作用域表");

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

    // 作用域落在**证据侧**那张表，不是跨行业那张——本领域的材料不进跨行业语料。
    let evidence_scope: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_material_target WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(evidence_scope, 3);
    let cross_scope: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM collection_work_order_cross_industry_target \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(cross_scope, 0, "本领域的作品不得写进跨行业作用域表");
}

/// 还没翻完搜索面的词不该跳到补详情这一步。
///
/// 在一个没挖完的底座上补详情，补出来的是一份看上去完整、其实少了一截的档案，而且
/// 没人看得出来少的是哪一截。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_unfinished_keyword_archive_does_not_jump_to_details() {
    let database = proof_database("keyword_detail_needs_baseline").await;
    let target_ref =
        submit_keyword_archive(&database, "考研自习::detail-early", "surface_error", 1, 2).await;

    let advance = advance_keyword_archive_detail(&database, target_ref, "建档补详情", "person")
        .await
        .expect("the detail advance runs");
    assert!(
        matches!(
            advance,
            KeywordDetailAdvance::Skipped("archive_round_not_complete")
        ),
        "第一段没完成时要说第一段没完成：{advance:?}"
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
             (target_ref,platform,target_kind,identity_key,display_name,source,domain_ref) \
         VALUES ($1,'xhs','keyword','考研自习::most_liked','考研自习','manual',$2::uuid)",
    )
    .bind(target_ref)
    .bind(EXTERNAL_DOMAIN)
    .execute(database.pool())
    .await
    .expect("target is stored");
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
             (target_ref,platform,target_kind,identity_key,display_name,source,domain_ref) \
         VALUES ($1,'xhs','creator','69aad16e000000003201b172','木可可','manual', \
                 '00000000-0000-4000-8000-000000000001')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("target is stored");
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
    // 刻意不写 domain_ref：模拟浏览器上报或插件推送建出来的候选目标。
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
    sqlx::query("UPDATE collection_observation_target SET domain_ref=$2::uuid WHERE target_ref=$1")
        .bind(target_ref)
        .bind(EXTERNAL_DOMAIN)
        .execute(database.pool())
        .await
        .expect("domain is assigned");
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

    let installation = ready_detail_station(&database, "详情补采证明工位").await;
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
        ..
    } = decision
    else {
        panic!("具备详情能力的空闲工位必须能领走这张工单，实际是 {decision:?}");
    };

    assert_eq!(
        task_spec
            .pointer("/capabilitiesRequested/0")
            .and_then(serde_json::Value::as_str),
        Some("content_detail"),
        "跨行业作用域要展开成逐篇详情任务，而不是再翻一次发现面"
    );
    assert_eq!(
        task_spec
            .pointer("/target/contentExternalId")
            .and_then(serde_json::Value::as_str),
        Some("note-archive-1")
    );
    // 入口必须是列表面当时平台返回的那条带签名的链接。裸 `/explore/{id}` 打不开，
    // 而这条链接在证据侧根本不存在——它只在跨行业样本表上。
    assert_eq!(
        execution_source_url.as_deref(),
        Some("https://www.xiaohongshu.com/search_result/note-archive-1?xsec_token=ABarchive"),
        "详情任务必须带上跨行业样本记下的签名链接"
    );
}

struct DetailStation {
    install_key: String,
    credential: Option<String>,
}

/// 一个只会补详情的工位：认领窗口开着、接活开关打开、账号已绑定、能力**只有**
/// `content_detail`。
///
/// 这四项缺任何一项，派发都会以一个**别的**理由拒绝，用例就会为错误的原因变红或变绿。
async fn ready_detail_station(database: &Database, label: &str) -> DetailStation {
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
            // **只有详情能力，故意不给 `discovery_search`。**
            //
            // 派发是按「这张工单有没有冻结具体作品」挑能力要求的：冻结了就要
            // `content_detail`，没冻结就要 `discovery_search`。若那个判断只认证据侧
            // 那张表、把跨行业作用域看成空，这张工单就会去要 `discovery_search`，
            // 这个工位于是被排除，活永远派不出去。一个同时具备两种能力的工位两边都
            // 合格，测不出这件事。
            capabilities: serde_json::json!(["content_detail"]),
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
        panic!("开着的认领窗口必须认领这次安装；实际是 {outcome:?}");
    };
    let credential = credential.map(|issued| {
        let raw = issued.raw_credential.expose_once().to_owned();
        (issued.credential_ref, raw)
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

/// 造一轮**本领域**关键词建档：材料走证据侧，跨行业样本表里一条都不该有。
async fn submit_home_domain_keyword_archive(database: &Database, identity_key: &str) -> Uuid {
    let mut coverage = search_coverage("学不进去", 4);
    coverage["layers"][0]["stoppedReason"] = serde_json::json!("bottom_confirmed");
    coverage["target"]["query"] = serde_json::json!("学不进去");
    submit_package_in_domain(
        database,
        HOME_DOMAIN,
        identity_key,
        "deep_archive",
        serde_json::json!({"query":"学不进去","ranking":"most_liked","scrollRounds":10}),
        "discovery_search",
        coverage,
        serde_json::json!({"surfaceReceipt":{"stopReason":"bottom_confirmed"}}),
        // 四篇、点赞各不相同：一批只补三篇，于是这一批挑的是哪三篇本身就是判据。
        (1..=4)
            .map(|index| {
                discovery_card(
                    &format!("note-home-{index}"),
                    &format!("本领域建档样本 {index}"),
                    &format!("{}", index * 1000),
                    &format!(
                        "https://www.xiaohongshu.com/search_result/note-home-{index}?xsec_token=ABhome"
                    ),
                )
            })
            .collect(),
    )
    .await;
    sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key=$1")
        .bind(identity_key)
        .fetch_one(database.pool())
        .await
        .expect("the home-domain target exists")
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
             (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state, \
              domain_ref) \
         VALUES ($1,'xhs','keyword','数学思维::sched','数学思维','manual','pending_decision', \
                 $2::uuid)",
    )
    .bind(target_ref)
    .bind(HOME_DOMAIN)
    .execute(database.pool())
    .await
    .expect("target fixture is stored");
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
                surface_key: "keyword_search".to_owned(),
                ranking_key: Some("most_liked".to_owned()),
                scroll_rounds: Some(3),
                top_by_likes: Some(20),
                published_within_days: Some(7),
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
         VALUES (gen_random_uuid(),'xhs','keyword','patrol','定时巡检','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_patrol'],ARRAY['immediate','scheduled'],200,200)",
    )
    .execute(database.pool())
    .await
    .expect("authorization is granted");
    sqlx::query("UPDATE collection_observation_target SET domain_ref=NULL WHERE target_ref=$1")
        .bind(target_ref)
        .execute(database.pool())
        .await
        .expect("the target is left without an observation domain");
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
