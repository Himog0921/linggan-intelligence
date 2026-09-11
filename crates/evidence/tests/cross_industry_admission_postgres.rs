//! 跨行业落库分流的真实链路证明。
//!
//! 这条路径此前没有任何集成测试：`insert_samples` 只在代码复用的意义上「顺带受益」，
//! 从没有人证明过外部领域的包真的会走到它、真的写出预期的行。本文件补上，并锁住两件
//! 最容易悄悄坏掉的语义：采样口径**整套一起换或一起保**，签名链接**只存平台返回的**。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_contracts::{parse_producer_attempt, parse_producer_submission};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, keyword_baseline_qualified, register_station,
    request_and_admit, start_producer_attempt, submit_producer_package,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// 迁移预置的外部领域「考研自习」。
const EXTERNAL_DOMAIN: &str = "00000000-0000-4000-8000-000000000002";

/// 造出一条完整的「外部领域关键词目标 → 工单 → 租约 → 任务 → 包」链路并提交。
///
/// 领域判定靠的正是这条链（`resolve_package_domain` 从包一路 JOIN 回观察目标），
/// 所以少任何一环，包都会被当作本领域材料写进证据侧——那恰恰是这套分流要防的事。
async fn submit_external_package(
    database: &Database,
    identity_key: &str,
    lane: &str,
    task_target: serde_json::Value,
    package_kind: &str,
    coverage: serde_json::Value,
    // 搜索面的完整性判据读的是**包的 checkpoint**（`surfaceReceipt.stopReason`），
    // 不是 coverage 里的 stoppedReason——两处都有 stop 字样，放错地方判据就悄悄落空。
    checkpoint: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> Uuid {
    // 生命周期一律留在默认的 pending_decision。关键词**不能**进入 archiving/archived
    // （`0042` 的 CHECK），建档与否是读取时从证据里查出来的；领域分流也不看生命周期。
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source,domain_ref) \
         VALUES ($1,'xhs','keyword',$2,$2,'manual',$3::uuid)",
    )
    .bind(target_ref)
    .bind(identity_key)
    .bind(EXTERNAL_DOMAIN)
    .execute(database.pool())
    .await
    .expect("external-domain target is stored");

    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request (request_ref,target_ref,lane,purpose,requested_by) \
         VALUES ($1,$2,$3,'cross industry sampling proof','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .bind(lane)
    .execute(database.pool())
    .await
    .expect("request fixture is stored");

    // `admitted` 必须挂在一份有效授权上（CHECK 强制两者同在），所以先签一份。
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units) \
         VALUES ($1,'xhs','keyword',$2,'cross industry sampling proof','person', \
                 scope_001_now()+interval '1 day', \
                 CASE WHEN $2='deep_archive' THEN ARRAY['keyword_archive','material_deepening'] \
                      ELSE ARRAY['keyword_patrol'] END, \
                 CASE WHEN $2='deep_archive' THEN ARRAY['immediate','batch'] \
                      ELSE ARRAY['immediate','scheduled'] END, \
                 200)",
    )
    .bind(authorization_ref)
    .bind(lane)
    .execute(database.pool())
    .await
    .expect("authorization fixture is stored");

    let decision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref) \
         VALUES ($1,$2,'admitted','cross_industry_sampling_proof',$3,$4)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("admission fixture is stored");

    let work_order_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_work_order (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions) \
         VALUES ($1,$2,$3,$4,200,'[\"maximum_quota\"]'::jsonb)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .execute(database.pool())
    .await
    .expect("work order fixture is stored");

    // 工位名在同一个证明库里必须唯一：一条用例会连着提交两轮（列表面 + 详情面）。
    let station_ref =
        register_station(database, &format!("跨行业采样证明工位 {identity_key}"), 200)
            .await
            .expect("station fixture is stored");
    let lease_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_work_order_lease (lease_ref,work_order_ref,station_ref,capture_identity,expires_at) \
         VALUES ($1,$2,$3,'{}'::jsonb,scope_001_now()+interval '1 hour')",
    )
    .bind(lease_ref)
    .bind(work_order_ref)
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("lease fixture is stored");

    let task_id = Uuid::new_v4();
    let producer_instance_id = Uuid::new_v4();
    let attempt_id = Uuid::new_v4();
    let installation_ref = Uuid::new_v4();
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"scheduled",
        "platform":"xhs","pageType":"search_results","target":task_target,
        "capabilitiesRequested":[package_kind],"maximumQuota":200,
        "commentLimit":"not_requested","acquireMedia":"not_requested",
        // scheduled 任务必须配服务端签发的风险策略，本机自发那套只属于 manual。
        "riskPolicy":"server_authorized_leased","stopConditions":["maximum_quota"]
    });
    // scheduled 任务只能由服务端派发产生——`create_producer_task` 会明确拒绝创建它们
    // （`ScheduledTaskNotServerIssued`）。夹具因此直接写入这一行，与它已经在直接造
    // 工单、租约、认领是同一层面的事。
    let task_spec = task.to_string();
    sqlx::query(
        "INSERT INTO linggan_runtime_task \
             (task_id,task_spec_hash,task_spec,source,platform,page_type) \
         VALUES ($1,encode(sha256(convert_to($2,'UTF8')),'hex'),$2::jsonb,'scheduled','xhs','search_results')",
    )
    .bind(task_id)
    .bind(&task_spec)
    .execute(database.pool())
    .await
    .expect("scheduled task fixture is stored");
    // 认领必须落在一个**真实的安装**上，而且停在 in_progress：提交时要重新锁定这份执行
    // 权限，只有锁得住才会写出 `COMPLETED_LIVE_STEP` 回执。夹具此前直接写 completed，
    // 回执因此是 NOT_APPLICABLE——那种包不足以证明任何一轮建档完成过。
    sqlx::query(
        "INSERT INTO plugin_installation \
             (installation_ref,install_key,station_ref,claim_kind,claimed_at,plugin_version) \
         VALUES ($1,$2,$3,'person',scope_001_now(),'0.8.48')",
    )
    .bind(installation_ref)
    .bind(producer_instance_id.to_string())
    .bind(station_ref)
    .execute(database.pool())
    .await
    .expect("installation fixture is stored");
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task \
             (lease_ref,task_id,sequence_no,execution_state,claimed_at,claimed_by_installation_ref) \
         VALUES ($1,$2,1,'in_progress',scope_001_now(),$3)",
    )
    .bind(lease_ref)
    .bind(task_id)
    .bind(installation_ref)
    .execute(database.pool())
    .await
    .expect("lease task fixture is stored");

    let attempt = serde_json::json!({
        "contractVersion":"linggan.producer.attempt.v1","producerInstanceId":producer_instance_id,
        "taskId":task_id,"attemptId":attempt_id
    });
    let attempt = parse_producer_attempt(&attempt.to_string()).expect("attempt validates");
    assert!(matches!(
        start_producer_attempt(database, &attempt).await,
        Ok(RuntimeAttemptOutcome::Started { .. })
    ));

    let package_ref = Uuid::new_v4();
    let submission = serde_json::json!({
        "contractVersion":"linggan.producer.capture-package.v1",
        "producerInstanceId":producer_instance_id,"taskId":task_id,"attemptId":attempt_id,
        "submissionId":Uuid::new_v4(),
        "capturePackage":{
            "contractVersion":"linggan.producer.capture-package.v1","packageRef":package_ref,
            "packageKind":package_kind,"platform":"xhs",
            "observedAt":"2026-09-11T02:00:00Z","capturedAt":"2026-09-11T02:00:00Z",
            "coverage":coverage,"checkpoint":checkpoint,"records":records
        }
    });
    let submission =
        parse_producer_submission(&submission.to_string()).expect("submission validates");
    let outcome = submit_producer_package(database, &submission).await;
    assert!(
        matches!(outcome, Ok(RuntimeSubmissionOutcome::Acknowledged { .. })),
        "cross-industry submission must be acknowledged: {outcome:?}"
    );
    package_ref
}

fn search_coverage(query: &str, acquired: i64) -> serde_json::Value {
    serde_json::json!({
        "target":{"basis":"current_visible_surface","surface":"target_driven_surface","query":query},
        "layers":[{
            "capability":"discovery_search","observed":acquired,"attempted":acquired,
            "acquired":acquired,"verified":0,"failed":0,"notAttempted":0,"unknown":0,
            "stoppedReason":"surface_read_complete"
        }]
    })
}

fn discovery_card(external_id: &str, title: &str, likes: &str, url: &str) -> serde_json::Value {
    serde_json::json!({
        "kind":"discovery_card","resultPosition":1,
        "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
        "payload":{"noteId":external_id,"title":title,"likes":likes,"url":url}
    })
}

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

    // 另一个真实存在的外部领域（迁移预置的「自闭症干预」）。
    let wrong_domain = sqlx::query(
        "INSERT INTO cross_industry_sample_lane (sample_ref,domain_ref,keyword,sort_order) \
         VALUES ($1,'00000000-0000-4000-8000-000000000003','考研自习','most_liked')",
    )
    .bind(sample_ref)
    .execute(database.pool())
    .await;
    assert!(
        wrong_domain.is_err(),
        "榜单留痕不得声称一个不属于该样本的领域"
    );
}

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
