//! 跨行业落库分流的真实链路证明：**采样口径与观察记录**。
//!
//! 这条路径此前没有任何集成测试：`insert_samples` 只在代码复用的意义上「顺带受益」，
//! 从没有人证明过外部领域的包真的会走到它、真的写出预期的行。本文件补上，并锁住三件
//! 最容易悄悄坏掉的语义：采样口径**整套一起换或一起保**、签名链接**只存平台返回的**、
//! 以及同一篇被再看到一次要**追加一条观察记录**而不是覆盖上一条。

#[path = "support/cross_industry_fixture.rs"]
mod cross_industry;
#[path = "support/material_fixture.rs"]
mod fixture;

use cross_industry::{
    discovery_card, discovery_card_at, search_coverage, submit_external_package,
    submit_package_for_target,
};
use fixture::proof_database;
use linggan_evidence::{CatalogDetailState, read_cross_industry_hits, read_keyword_hits};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// 关键词采集要把「这一批是按什么口径取回来的」和「上哪儿能看到原作」一起留下。
///
/// 口径此前一直是空的，于是同一个词的不同排序混作一堆；链接此前根本没存，于是既看不了
/// 原作也无法接着采详情。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_search_records_its_sampling_provenance_and_signed_link() {
    let database = proof_database("cross_industry_sampling_provenance").await;
    let signed = "https://www.xiaohongshu.com/search_result/note-cross-1?xsec_token=ABsigned";
    submit_external_package(
        &database,
        "考研自习::most_liked",
        "patrol",
        serde_json::json!({
            "query":"考研自习","ranking":"most_liked",
            "topByLikes":20,"scrollRounds":3,"publishedWithinDays":7
        }),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card(
            "note-cross-1",
            "二战考研半夜被自习室赶出来了",
            "2.9万",
            signed,
        )],
    )
    .await;

    let row = sqlx::query(
        "SELECT keyword,sort_order,scroll_rounds,requested_count,actual_count,source_url,\
                like_count,title \
         FROM cross_industry_sample WHERE content_external_id='note-cross-1'",
    )
    .fetch_one(database.pool())
    .await
    .expect("external-domain material lands in the cross-industry table");

    assert_eq!(
        row.get::<Option<String>, _>("keyword").as_deref(),
        Some("考研自习")
    );
    assert_eq!(
        row.get::<Option<String>, _>("sort_order").as_deref(),
        Some("most_liked")
    );
    assert_eq!(row.get::<Option<i32>, _>("scroll_rounds"), Some(3));
    assert_eq!(row.get::<Option<i32>, _>("requested_count"), Some(20));
    // 「实际取得数」取自插件如实报告的 coverage，不是本次写库条数。
    assert_eq!(row.get::<Option<i32>, _>("actual_count"), Some(1));
    assert_eq!(
        row.get::<Option<String>, _>("source_url").as_deref(),
        Some(signed)
    );
    // 顺带证明互动数的文本解析在跨行业这条路径上同样生效（此前只在发现层有证明）。
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(29_000));

    // 本领域证据侧一行都不该有：外部领域是参照物，不进证据库。
    let evidence_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_discovery_finding")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(evidence_rows, 0, "参照物不得混进证据侧");
}

/// 没有口径的那一轮不能把已经记下的口径抹掉。
///
/// 详情面与评论面按已知作品去采，本就没有「按什么排序搜到的」这回事。它们更新同一行时
/// 如果逐列覆盖，就会把列表面记下的整套口径清空；如果逐列 COALESCE，又会把这一轮的
/// 关键词配上上一轮的下拉次数，拼出一份从未发生过的口径。**五列必须整套一起换或一起保。**
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_later_round_without_provenance_keeps_the_recorded_one_intact() {
    let database = proof_database("cross_industry_provenance_retained").await;
    let signed = "https://www.xiaohongshu.com/search_result/note-cross-2?xsec_token=ABfirst";
    submit_external_package(
        &database,
        "考研自习::comprehensive",
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"comprehensive","topByLikes":20,"scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card("note-cross-2", "初稿标题", "1万", signed)],
    )
    .await;

    // 同一篇笔记随后被详情面再次观察：详情不带任何采样口径。
    submit_external_package(
        &database,
        "考研自习::comprehensive::detail",
        "patrol",
        serde_json::json!({"contentExternalId":"note-cross-2"}),
        "content_detail",
        serde_json::json!({
            "target":{"basis":"known_set","contentExternalId":"note-cross-2"},
            "layers":[{
                "capability":"content_detail","observed":1,"attempted":1,"acquired":1,
                "verified":0,"failed":0,"notAttempted":0,"unknown":0,
                "stoppedReason":"known_set_complete"
            }]
        }),
        serde_json::Value::Null,
        vec![serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-cross-2"},
            "payload":{"noteId":"note-cross-2","title":"详情面补准的标题","likes":"1.2万"}
        })],
    )
    .await;

    let row = sqlx::query(
        "SELECT keyword,sort_order,scroll_rounds,requested_count,actual_count,source_url,title,like_count \
         FROM cross_industry_sample WHERE content_external_id='note-cross-2'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the sample row survives the second observation");

    // 口径整套原样保留，一列都不许被抹成空。
    assert_eq!(
        row.get::<Option<String>, _>("keyword").as_deref(),
        Some("考研自习")
    );
    assert_eq!(
        row.get::<Option<String>, _>("sort_order").as_deref(),
        Some("comprehensive")
    );
    assert_eq!(row.get::<Option<i32>, _>("scroll_rounds"), Some(3));
    assert_eq!(row.get::<Option<i32>, _>("requested_count"), Some(20));
    assert_eq!(row.get::<Option<i32>, _>("actual_count"), Some(1));
    // 列表面拿到的签名链接也要留住：详情面没有来源页签名，不该把它覆盖成空。
    assert_eq!(
        row.get::<Option<String>, _>("source_url").as_deref(),
        Some(signed)
    );
    // 作品自身的事实照常保旧补新。
    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("详情面补准的标题")
    );
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(12_000));
}

/// 同一篇笔记同时上了一个词的两个榜时，两次都要留痕。
///
/// 样本行只留得下最近一次的口径，所以「同时上了综合榜和点赞榜」这个信号必须另记——
/// 它正是识别真爆款的依据，而且不记就补不回来（同词同排序隔两天重合率只有 5%~20%）。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_note_seen_on_two_boards_of_one_keyword_is_recorded_on_both() {
    let database = proof_database("cross_industry_sample_lane").await;
    let signed = "https://www.xiaohongshu.com/search_result/note-two-boards?xsec_token=ABboards";
    for (identity, ranking) in [
        ("考研自习::most_liked", "most_liked"),
        ("考研自习::comprehensive", "comprehensive"),
    ] {
        submit_external_package(
            &database,
            identity,
            "patrol",
            serde_json::json!({"query":"考研自习","ranking":ranking,"scrollRounds":3}),
            "discovery_search",
            search_coverage("考研自习", 1),
            serde_json::Value::Null,
            vec![discovery_card(
                "note-two-boards",
                "两个榜都上了",
                "3.1万",
                signed,
            )],
        )
        .await;
    }

    // 材料层仍然只有一行：去重的是笔记，不是榜。
    let samples: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cross_industry_sample WHERE content_external_id='note-two-boards'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(samples, 1, "同一篇笔记在材料层只该有一行");

    let boards: Vec<String> = sqlx::query_scalar(
        "SELECT lane.sort_order FROM cross_industry_sample_lane lane \
         JOIN cross_industry_sample sample USING (sample_ref) \
         WHERE sample.content_external_id='note-two-boards' AND lane.keyword='考研自习' \
         ORDER BY lane.sort_order",
    )
    .fetch_all(database.pool())
    .await
    .expect("lane rows are readable");
    assert_eq!(
        boards,
        vec!["comprehensive".to_owned(), "most_liked".to_owned()],
        "两个榜都要留痕，而不是后一个把前一个盖掉"
    );

    // 样本行上的口径仍是最近一次看到它的那个维度——两者各司其职。
    let latest: Option<String> = sqlx::query_scalar(
        "SELECT sort_order FROM cross_industry_sample WHERE content_external_id='note-two-boards'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(latest.as_deref(), Some("comprehensive"));
}

/// 同一篇被**再次看到**时，样本仍是一行，但观察记录要多出一条。
///
/// 样本行是 upsert 的，它只回答「这篇是什么、现在多少赞」。巡检一周跑一次，如果只有
/// 这一行，上一周读到的数就被这一周盖掉了——「被看到过几次、热度怎么变」从此无从查起，
/// 而且补不回来（平台不提供历史值）。所以每看到一次就追加一条。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn seeing_a_note_again_appends_an_observation_without_forking_the_sample() {
    let database = proof_database("cross_industry_sample_observation").await;
    let signed = "https://www.xiaohongshu.com/search_result/note-seen-twice?xsec_token=ABtwice";
    for (identity, likes, order) in [
        ("考研自习::week-1", "1.2万", 7),
        ("考研自习::week-2", "1.5万", 2),
    ] {
        submit_external_package(
            &database,
            identity,
            "patrol",
            serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":3}),
            "discovery_search",
            search_coverage("考研自习", 1),
            serde_json::Value::Null,
            vec![discovery_card_at(
                "note-seen-twice",
                "被看到两次",
                likes,
                signed,
                order,
            )],
        )
        .await;
    }

    let samples: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cross_industry_sample WHERE content_external_id='note-seen-twice'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(samples, 1, "一篇一行——再看到一次不该分裂出第二条样本");

    // 两次读数都在，而且各自挂在自己那一轮的包上。
    let readings: Vec<(Option<i64>, Option<i32>)> = sqlx::query_as(
        "SELECT observation.like_count, observation.discovery_order \
         FROM cross_industry_sample_observation observation \
         JOIN cross_industry_sample sample USING (sample_ref) \
         WHERE sample.content_external_id='note-seen-twice' \
         ORDER BY observation.like_count",
    )
    .fetch_all(database.pool())
    .await
    .expect("observation rows are readable");
    assert_eq!(
        readings,
        vec![(Some(12_000), Some(7)), (Some(15_000), Some(2))],
        "两次观察各自的点赞数与发现位次都要留着，后一次不覆盖前一次"
    );

    let packages: i64 = sqlx::query_scalar(
        "SELECT count(DISTINCT observation.package_ref) \
         FROM cross_industry_sample_observation observation \
         JOIN cross_industry_sample sample USING (sample_ref) \
         WHERE sample.content_external_id='note-seen-twice'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(packages, 2, "两条观察必须来自两轮不同的采集");

    // 榜单留痕由明细聚合出来，「被看到过几次」直接能读。
    let (times, first_seen_differs): (i64, bool) = sqlx::query_as(
        "SELECT lane.observed_times, lane.first_seen_at <= lane.last_observed_at \
         FROM cross_industry_sample_lane lane \
         JOIN cross_industry_sample sample USING (sample_ref) \
         WHERE sample.content_external_id='note-seen-twice'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the aggregated lane view is readable");
    assert_eq!(times, 2, "同一个榜上被看到两次就是两次");
    assert!(first_seen_differs);

    // 样本行上仍是最近一次读到的数——两者各司其职。
    let current: Option<i64> = sqlx::query_scalar(
        "SELECT like_count FROM cross_industry_sample WHERE content_external_id='note-seen-twice'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(current, Some(15_000));
}

/// 跨行业目标的检查器要看得见自己的作品。
///
/// 外部领域的材料按 `0044` 的隔离住在 `cross_industry_sample`，证据侧一条都没有。检查器
/// 此前只读证据侧，于是跨行业目标的作品页永远是「0 条可查证命中」——采回来两百多篇，
/// 界面上一篇都看不到，而且看不出是没采到还是读错了地方。
///
/// 这条用例同时锁住名次那一列的类型：`discovery_order` 是 integer，展示合同上是 bigint。
/// 不显式转换时**只有在真的有观察记录的目标上**才会解码失败——空库一路绿灯，线上一点就崩。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_cross_industry_target_can_read_its_own_hits() {
    let database = proof_database("cross_industry_hits_read").await;
    submit_external_package(
        &database,
        "考研自习::hits-read",
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card_at(
            "note-hits-read",
            "检查器要看得见我",
            "2.4万",
            "https://www.xiaohongshu.com/search_result/note-hits-read?xsec_token=ABhits",
            11,
        )],
    )
    .await;
    let target_ref: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
    )
    .bind("考研自习::hits-read")
    .fetch_one(database.pool())
    .await
    .unwrap();

    // 证据侧那条读法对它必然落空——材料不在那儿。
    let evidence_side = read_keyword_hits(&database, target_ref)
        .await
        .expect("the evidence-side read runs");
    assert_eq!(
        evidence_side.map(|projection| projection.works.len()),
        Some(0),
        "跨行业材料不该出现在证据侧；这里为 0 是对的，也正是检查器此前空白的原因"
    );

    let hits = read_cross_industry_hits(&database, target_ref)
        .await
        .expect("the cross-industry read runs")
        .expect("the projection exists");
    assert_eq!(hits.works.len(), 1);
    let work = &hits.works[0];
    assert_eq!(work.content_external_id, "note-hits-read");
    assert_eq!(work.title.as_deref(), Some("检查器要看得见我"));
    // 发现位次从 0 起计，展示成「第几名」时加一——这里也验证那一列真的解得出来。
    assert_eq!(work.match_position, Some(12));
    assert_eq!(work.detail_state, CatalogDetailState::Pending);
}

/// **被隔离的那一轮不算「详情已取得」。**
///
/// 包的身份与任务声明对不上时，记录判为 `quarantined`、材料整段不写——样本行一个字段都
/// 没变；但租约任务仍然被标成 `completed`（完成说的是这一步执行过了，不是材料合格）。
/// 只看 `completed` 就会对一篇什么都没有的笔记说「详情已取得」，而那一篇从此没人再看它。
/// 2026-09-10 的 120 条整包隔离正是这个形状。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_quarantined_detail_round_is_not_reported_as_collected() {
    let database = proof_database("cross_industry_hits_quarantine").await;
    submit_external_package(
        &database,
        "考研自习::hits-quarantine",
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card(
            "note-quarantined",
            "详情会被隔离",
            "9000",
            "https://www.xiaohongshu.com/search_result/note-quarantined?xsec_token=ABq",
        )],
    )
    .await;
    let target_ref: Uuid = sqlx::query_scalar(
        "SELECT target_ref FROM collection_observation_target WHERE identity_key=$1",
    )
    .bind("考研自习::hits-quarantine")
    .fetch_one(database.pool())
    .await
    .unwrap();

    // 详情那一轮**挂在同一个目标底下**——判据是按 `target_ref` 关联两轮的，换个目标
    // 就静默落空，测试会为错误的原因变绿。
    submit_package_for_target(
        &database,
        target_ref,
        "考研自习::hits-quarantine-detail",
        "patrol",
        serde_json::json!({"contentExternalId":"note-quarantined"}),
        "content_detail",
        serde_json::json!({
            "target":{"basis":"known_set","contentExternalId":"note-quarantined"},
            "layers":[{
                "capability":"content_detail","observed":1,"attempted":1,"acquired":1,
                "verified":0,"failed":0,"notAttempted":0,"unknown":0,
                "stoppedReason":"known_set_complete"
            }]
        }),
        serde_json::Value::Null,
        vec![serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-somebody-else"},
            "payload":{"noteId":"note-somebody-else","title":"这不是要的那一篇","likes":"1"}
        })],
    )
    .await;

    // 前提：这一轮确实被隔离了，材料一个字段都没进去。
    let quarantined: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_record_disposition \
         WHERE disposition='quarantined'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(quarantined > 0, "前提不成立：这一轮本该被判为隔离");

    let hits = read_cross_industry_hits(&database, target_ref)
        .await
        .expect("the cross-industry read runs")
        .expect("the projection exists");
    let work = hits
        .works
        .iter()
        .find(|work| work.content_external_id == "note-quarantined")
        .expect("被隔离的那一篇仍然在命中列表里——隔离的是那一轮材料，不是这篇笔记");
    assert_eq!(
        work.detail_state,
        CatalogDetailState::Pending,
        "材料被隔离、一个字段都没写进来，就不能显示成「详情已取得」"
    );
}

/// 没有采样口径的那一轮不该在任何榜上留痕。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_detail_round_without_provenance_joins_no_board() {
    let database = proof_database("cross_industry_lane_detail_only").await;
    submit_external_package(
        &database,
        "考研自习::detail-only",
        "patrol",
        serde_json::json!({"contentExternalId":"note-detail-only"}),
        "content_detail",
        serde_json::json!({
            "target":{"basis":"known_set","contentExternalId":"note-detail-only"},
            "layers":[{
                "capability":"content_detail","observed":1,"attempted":1,"acquired":1,
                "verified":0,"failed":0,"notAttempted":0,"unknown":0,
                "stoppedReason":"known_set_complete"
            }]
        }),
        serde_json::Value::Null,
        vec![serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-detail-only"},
            "payload":{"noteId":"note-detail-only","title":"只有详情","likes":"12"}
        })],
    )
    .await;

    let lanes: i64 = sqlx::query_scalar("SELECT count(*) FROM cross_industry_sample_lane")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(lanes, 0, "详情面不是从某个榜上看到这篇的，不该留榜单痕迹");
}

/// 榜单留痕的领域必须与样本一致——由数据库拒绝，不指望每个读取点都记得加条件。
///
/// 「按关键词查这张表」是主要读法，而两个领域完全可能用同一个词。复合外键把这件事
/// 变成写不进去，而不是读出来才发现混了。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_board_row_cannot_claim_a_domain_its_sample_does_not_belong_to() {
    let database = proof_database("cross_industry_lane_domain_guard").await;
    submit_external_package(
        &database,
        "考研自习::most_liked",
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card(
            "note-domain-guard",
            "领域守卫",
            "88",
            "https://www.xiaohongshu.com/search_result/note-domain-guard?xsec_token=ABguard",
        )],
    )
    .await;

    let sample_ref: Uuid = sqlx::query_scalar(
        "SELECT sample_ref FROM cross_industry_sample WHERE content_external_id='note-domain-guard'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the sample exists");

    let package_ref: Uuid = sqlx::query_scalar(
        "SELECT package_ref FROM cross_industry_sample_observation WHERE sample_ref=$1",
    )
    .bind(sample_ref)
    .fetch_one(database.pool())
    .await
    .expect("the observation exists");

    // 另一个真实存在的外部领域（迁移预置的「自闭症干预」）。
    let wrong_domain = sqlx::query(
        "INSERT INTO cross_industry_sample_observation \
             (observation_ref,sample_ref,domain_ref,package_ref,keyword,sort_order) \
         VALUES (gen_random_uuid(),$1,'00000000-0000-4000-8000-000000000003',$2, \
                 '考研自习','most_liked')",
    )
    .bind(sample_ref)
    .bind(package_ref)
    .execute(database.pool())
    .await;
    assert!(
        wrong_domain.is_err(),
        "榜单留痕不得声称一个不属于该样本的领域"
    );
}

/// **详情取到了，就得有一条说得出口的事实——而且正文真的要存下来。**
///
/// 此前跨行业的详情落库只把标题、作者、封面、互动数 upsert 回样本行，**正文直接丢掉**：
/// 「补详情」除了刷新一遍数字什么也没留下，而正文正是详情这一轮存在的全部理由。
/// 「详情已取得」也没有任何一行事实支撑，只能从运行任务的状态反推。`0079` 两件一起解决。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_completed_detail_round_records_the_body_and_the_fact_that_it_arrived() {
    let database = proof_database("cross_industry_detail_fact").await;
    let target_ref = seed_board_hit(&database, "考研自习::detail-fact", "note-detail-fact").await;

    submit_package_for_target(
        &database,
        target_ref,
        "考研自习::detail-fact-detail",
        "patrol",
        serde_json::json!({"contentExternalId":"note-detail-fact"}),
        "content_detail",
        detail_coverage("note-detail-fact"),
        serde_json::Value::Null,
        vec![serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-detail-fact"},
            "payload":{
                "noteId":"note-detail-fact",
                "title":"自习室蹲了一个月",
                "bodyText":"每天七点到馆，靠窗第三排最容易坚持。",
                "publishedAtText":"3天前",
                "likes":"24000"
            }
        })],
    )
    .await;

    let detail: (Option<String>, String, Option<String>) = sqlx::query_as(
        "SELECT detail.body_text,detail.body_state,detail.published_at_source_text \
         FROM cross_industry_sample_detail detail \
         JOIN cross_industry_sample sample USING(sample_ref) \
         WHERE sample.content_external_id=$1",
    )
    .bind("note-detail-fact")
    .fetch_one(database.pool())
    .await
    .expect("详情落库必须留下一行事实——没有它，「详情已取得」只能靠猜运行任务的状态");
    assert_eq!(
        detail.0.as_deref(),
        Some("每天七点到馆，靠窗第三排最容易坚持。"),
        "正文必须真的存下来；此前它被丢掉，补详情等于白采一轮"
    );
    assert_eq!(detail.1, "KNOWN");
    assert_eq!(
        detail.2.as_deref(),
        Some("3天前"),
        "平台给的相对时间原样保留，不压成一个它从没给过的绝对时刻"
    );

    let hits = read_cross_industry_hits(&database, target_ref)
        .await
        .expect("the cross-industry read runs")
        .expect("the projection exists");
    let work = hits
        .works
        .iter()
        .find(|work| work.content_external_id == "note-detail-fact")
        .expect("the sample is a hit of this target");
    assert_eq!(
        work.detail_state,
        CatalogDetailState::Complete,
        "材料真的进来了，就该显示成已取得"
    );

    // **反方向同样要守住：取到了就不该再排进补详情。**
    //
    // 此前只断言了「还没取到的仍在待补清单里」。少了这一半，那条排除条件被漏写、取反或
    // 关联到错误的列时，一篇已经取到详情的样本会被永远重复排队——每一轮都真的去平台再采
    // 一遍同一篇，而它不产生任何新事实，只多花一次平台访问额度，界面上也看不出异常。
    let pending = linggan_evidence::keyword_targets_pending_detail(&database, &[target_ref])
        .await
        .expect("the pending-detail read runs");
    assert!(
        !pending.contains(&target_ref),
        "这个关键词唯一的一篇已经取到详情，它不该还留在待补清单里"
    );
}

/// **试过没成功不等于取到了。**
///
/// 补详情的待办判据此前认 `execution_state IN ('completed','unavailable','blocked')`：
/// 后两个是「这一篇采不到」，与取到了相反。结果是**失败一次的笔记永久从待补清单里消失**，
/// 界面上这个词看起来已经补齐了。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_blocked_detail_attempt_leaves_the_sample_pending() {
    let database = proof_database("cross_industry_detail_blocked").await;
    let target_ref = seed_board_hit(&database, "考研自习::detail-blocked", "note-blocked").await;

    // 一次真的受阻的详情尝试：任务停在 `blocked`（`0047` 的 CHECK 要求它既没认领也没完成），
    // 没有任何包、没有任何材料。工单已结束、租约已归还，所以它不算「还在飞」。
    //
    // 这一条造的正是旧判据会认的形状：`task_spec` 里要的能力是 `content_detail`、
    // `{target,contentExternalId}` 对得上这一篇、状态在它认的那三个里。
    blocked_detail_attempt(&database, target_ref, "note-blocked").await;

    let pending = linggan_evidence::keyword_targets_pending_detail(&database, &[target_ref])
        .await
        .expect("the pending-detail read runs");
    assert!(
        pending.contains(&target_ref),
        "受阻过的那一篇必须还在待补清单里——它一个字都没采到"
    );
}

/// **一篇被另一个关键词先看到，这个关键词照样得数到它。**
///
/// 样本行上的 `target_ref` 只在第一次插入时写定
/// （`target_ref = COALESCE(cross_industry_sample.target_ref, EXCLUDED.target_ref)`）。
/// 读取侧按它划范围，于是第二个关键词永远看不到这一篇、也永远不会给它补详情——而两个词
/// 各自的观察记录明明都在。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_note_first_seen_by_another_keyword_still_counts_for_this_one() {
    let database = proof_database("cross_industry_shared_sample").await;
    let first_ref = seed_board_hit(&database, "考研自习::shared-first", "note-shared").await;
    let second_ref = seed_board_hit(&database, "图书馆打卡::shared-second", "note-shared").await;

    // 前提：确实是同一篇样本，而样本行只认第一个目标。
    let owner: (Uuid, i64) = sqlx::query_as(
        "SELECT sample.target_ref,count(*) OVER () FROM cross_industry_sample sample \
         WHERE sample.content_external_id=$1",
    )
    .bind("note-shared")
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(owner.1, 1, "同一个领域同一篇只有一行样本");
    assert_eq!(owner.0, first_ref, "样本行上记的是最早看到它的那个关键词");

    for (label, target_ref) in [("先看到的", first_ref), ("后看到的", second_ref)] {
        let hits = read_cross_industry_hits(&database, target_ref)
            .await
            .expect("the cross-industry read runs")
            .expect("the projection exists");
        assert!(
            hits.works
                .iter()
                .any(|work| work.content_external_id == "note-shared"),
            "{label}那个关键词也在自己的榜上看到过这一篇，命中列表必须有它"
        );
    }
    let counts = linggan_evidence::read_keyword_catalog_counts(&database, &[first_ref, second_ref])
        .await
        .expect("the catalog counts read runs");
    for (label, target_ref) in [("先看到的", first_ref), ("后看到的", second_ref)] {
        assert_eq!(
            counts.get(&target_ref).map(|counts| counts.works),
            Some(1),
            "{label}那个关键词的命中计数必须算上这一篇"
        );
    }
    let pending =
        linggan_evidence::keyword_targets_pending_detail(&database, &[first_ref, second_ref])
            .await
            .expect("the pending-detail read runs");
    assert!(
        pending.contains(&second_ref),
        "后看到的那个关键词也该能给这一篇补详情"
    );
}

/// **跨行业目标的「最近命中」不能永远是「读不到」。**
///
/// 列表那一列走 `linggan_material_discovery_finding`，而外部领域的材料按 `0044` 的隔离一条
/// 都不进证据侧。于是每一个跨行业目标的最近命中都是「读不到」、巡查状态永远停在「等待
/// 首轮」——哪怕它刚采回来两百篇。而这一列上「读不到」与「采了一篇都没有」长得一样。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_cross_industry_target_reports_its_latest_patrol_hits() {
    let database = proof_database("cross_industry_latest_hits").await;
    let target_ref = seed_board_hit(&database, "考研自习::latest-hits", "note-latest-hits").await;
    sqlx::query(
        "UPDATE collection_observation_target SET monitoring_enabled=true WHERE target_ref=$1",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target is monitoring");

    let targets = linggan_evidence::list_targets(&database, None, None, 50)
        .await
        .expect("the target list reads");
    let summaries = linggan_evidence::read_target_observation_summaries(&database, &targets)
        .await
        .expect("the observation summary reads");
    let summary = summaries
        .get(&target_ref)
        .expect("the target has a summary row");
    assert_eq!(
        summary.latest_hits,
        Some(1),
        "这一轮真的命中了一篇，不能报「读不到」"
    );
    assert_eq!(
        summary.latest_new,
        Some(1),
        "第一次看到它，就是这一轮的新增"
    );
    assert_eq!(
        summary.patrol_state,
        linggan_evidence::PatrolReadState::Normal,
        "有过一轮合格巡查，状态不该停在「等待首轮」"
    );
}

/// 一个外部领域关键词目标 + 它从榜上命中的一篇样本。
async fn seed_board_hit(database: &Database, identity_key: &str, external_id: &str) -> Uuid {
    submit_external_package(
        database,
        identity_key,
        "patrol",
        serde_json::json!({"query":"考研自习","ranking":"most_liked","scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        serde_json::Value::Null,
        vec![discovery_card(
            external_id,
            "自习室蹲了一个月",
            "2.4万",
            &format!("https://www.xiaohongshu.com/search_result/{external_id}?xsec_token=ABseed"),
        )],
    )
    .await;
    sqlx::query_scalar("SELECT target_ref FROM collection_observation_target WHERE identity_key=$1")
        .bind(identity_key)
        .fetch_one(database.pool())
        .await
        .expect("the seeded target is readable")
}

/// 一轮详情采集的覆盖度。
fn detail_coverage(external_id: &str) -> serde_json::Value {
    serde_json::json!({
        "target":{"basis":"known_set","contentExternalId":external_id},
        "layers":[{
            "capability":"content_detail","observed":1,"attempted":1,"acquired":1,
            "verified":0,"failed":0,"notAttempted":0,"unknown":0,
            "stoppedReason":"known_set_complete"
        }]
    })
}

/// 一次受阻的详情尝试：有任务、有工单，但一个包都没有。
///
/// 工单置为已结束、租约已归还——否则它会落进「还排在一张活着的工单上」那一支，测试就会
/// 因为错误的原因变绿。
async fn blocked_detail_attempt(database: &Database, target_ref: Uuid, external_id: &str) {
    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref,target_ref,lane,purpose,requested_by) \
         VALUES ($1,$2,'patrol','blocked detail proof','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("request is stored");
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units) \
         VALUES ($1,'xhs','keyword','patrol','blocked detail proof','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_patrol'],ARRAY['immediate','scheduled'],200)",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("authorization is stored");
    let decision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref) \
         VALUES ($1,$2,'admitted','blocked_detail_proof',$3,$4)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("admission is stored");
    let work_order_ref = Uuid::new_v4();
    sqlx::query(
        // `queue_state` 一旦离开 `legacy`／`cancelled`，`0036` 就要求它有排期时刻。
        "INSERT INTO collection_work_order \
             (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions, \
              queue_state,scheduled_for) \
         VALUES ($1,$2,$3,'patrol',1,'[\"maximum_quota\"]'::jsonb,'completed',scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("work order is stored");
    let station_ref = linggan_evidence::register_station(
        database,
        &format!("受阻详情证明工位 {external_id}"),
        200,
    )
    .await
    .expect("station is stored");
    let lease_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_work_order_lease \
             (lease_ref,work_order_ref,station_ref,capture_identity,expires_at,released_at, \
              release_reason) \
         VALUES ($1,$2,$3,'{}'::jsonb,scope_001_now()+interval '1 hour',scope_001_now(),'partial')",
    )
    .bind(lease_ref)
    .bind(work_order_ref)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("lease is stored");
    let task_id = Uuid::new_v4();
    let task_spec = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"scheduled",
        "platform":"xhs","pageType":"content_detail",
        "target":{"contentExternalId":external_id},
        "capabilitiesRequested":["content_detail"],"maximumQuota":1,
        "commentLimit":"not_requested","acquireMedia":"not_requested",
        "riskPolicy":"server_authorized_leased","stopConditions":["maximum_quota"]
    })
    .to_string();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
             (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,encode(sha256(convert_to($2,'UTF8')),'hex'),$2::jsonb,'scheduled','xhs', \
                 'content_detail')",
    )
    .bind(task_id)
    .bind(&task_spec)
    .execute(database.pool())
    .await
    .expect("task is stored");
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task \
             (lease_ref,task_id,sequence_no,execution_state) \
         VALUES ($1,$2,1,'blocked')",
    )
    .bind(lease_ref)
    .bind(task_id)
    .execute(database.pool())
    .await
    .expect("the blocked attempt is recorded");
}
