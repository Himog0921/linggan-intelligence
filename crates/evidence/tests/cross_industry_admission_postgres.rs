//! 跨行业落库分流的真实链路证明。
//!
//! 这条路径此前没有任何集成测试：`insert_samples` 只在代码复用的意义上「顺带受益」，
//! 从没有人证明过外部领域的包真的会走到它、真的写出预期的行。本文件补上，并锁住两件
//! 最容易悄悄坏掉的语义：采样口径**整套一起换或一起保**，签名链接**只存平台返回的**。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::proof_database;
use linggan_contracts::{
    parse_producer_attempt, parse_producer_submission, parse_producer_task_spec,
};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome, create_producer_task,
    register_station, start_producer_attempt, submit_producer_package,
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
    task_target: serde_json::Value,
    package_kind: &str,
    coverage: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> Uuid {
    // 生命周期留在默认的 pending_decision：`monitoring` 另有「必须绑定监控规则」的约束，
    // 而领域分流只看目标归属哪个领域，与它处在哪个生命周期无关。
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
         VALUES ($1,$2,'patrol','cross industry sampling proof','person')",
    )
    .bind(request_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("request fixture is stored");

    // `admitted` 必须挂在一份有效授权上（CHECK 强制两者同在），所以先签一份。
    let authorization_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_authorization \
             (authorization_ref,platform,target_kind,lane,purpose,granted_by,expires_at, \
              allowed_task_templates,allowed_dispatch_lanes,max_work_units) \
         VALUES ($1,'xhs','keyword','patrol','cross industry sampling proof','person', \
                 scope_001_now()+interval '1 day',ARRAY['keyword_patrol'],ARRAY['scheduled'],200)",
    )
    .bind(authorization_ref)
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
         VALUES ($1,$2,$3,'patrol',200,'[\"maximum_quota\"]'::jsonb)",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
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
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"manual",
        "platform":"xhs","pageType":"search_results","target":task_target,
        "capabilitiesRequested":[package_kind],"maximumQuota":200,
        "commentLimit":"not_requested","acquireMedia":"not_requested",
        "riskPolicy":"local_trusted_user_initiated","stopConditions":["maximum_quota"]
    });
    let task = parse_producer_task_spec(&task.to_string()).expect("task validates");
    assert!(matches!(
        create_producer_task(database, &task).await,
        Ok(RuntimeTaskOutcome::Created { .. })
    ));
    sqlx::query(
        "INSERT INTO collection_work_order_lease_task (lease_ref,task_id,sequence_no,execution_state,claimed_at,completed_at) \
         VALUES ($1,$2,1,'completed',scope_001_now(),scope_001_now())",
    )
    .bind(lease_ref)
    .bind(task_id)
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
            "coverage":coverage,"records":records
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
        serde_json::json!({
            "query":"考研自习","ranking":"most_liked",
            "topByLikes":20,"scrollRounds":3,"publishedWithinDays":7
        }),
        "discovery_search",
        search_coverage("考研自习", 1),
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
        serde_json::json!({"query":"考研自习","ranking":"comprehensive","topByLikes":20,"scrollRounds":3}),
        "discovery_search",
        search_coverage("考研自习", 1),
        vec![discovery_card("note-cross-2", "初稿标题", "1万", signed)],
    )
    .await;

    // 同一篇笔记随后被详情面再次观察：详情不带任何采样口径。
    submit_external_package(
        &database,
        "考研自习::comprehensive::detail",
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
