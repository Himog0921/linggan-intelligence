//! Domain 采集链路的共用夹具。
//!
//! Domain 用途靠一整条链（包 → 任务 → 租约 → WorkOrder → Domain relation），少任何一环，
//! 已接纳材料就没有可追溯的用途。所以造一次真实的链路比 mock 便宜：
//! 抄近路的夹具正是此前让「建档请求必然撞 CHECK」这个 100% 必现的缺陷躲过 14 条绿测试
//! 的原因。
//!
//! 两个证明文件共用它：抽样与观察记录一个，关键词建档与详情补采一个。

use linggan_contracts::{parse_producer_attempt, parse_producer_submission};
use linggan_evidence::{
    RuntimeAttemptOutcome, RuntimeSubmissionOutcome, register_station, start_producer_attempt,
    submit_producer_package,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 迁移预置的 peer Domain「考研自习」。
pub const PEER_DOMAIN: &str = "00000000-0000-4000-8000-000000000002";

/// 迁移预置的本领域（ADHD）。本领域只有一个，由 `0041` 预置。
pub const ADHD_DOMAIN: &str = "00000000-0000-4000-8000-000000000001";

/// 夹具默认的单子篇数上限（授权给的额度）。
///
/// 它和规则口径的「取赞前 N」是两个数：额度 200 配口径 20 时两者取小仍是 20，看不出区别；
/// 要证明记录读的是哪一个，就得有一条额度更小的链路，见 `submit_peer_domain_package_with_quota`。
pub const FIXTURE_QUOTA: i32 = 200;

/// 造出一条完整的「peer Domain 关键词目标 → 工单 → 租约 → 任务 → 包」链路并提交。
///
/// Domain 使用从 WorkOrder 冻结的用途推导，所以少任何一环，材料就没有可追溯的用途。
pub async fn submit_peer_domain_package(
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
    submit_package_for_domain(
        database,
        PEER_DOMAIN,
        identity_key,
        lane,
        task_target,
        FIXTURE_QUOTA,
        package_kind,
        coverage,
        checkpoint,
        records,
    )
    .await
}

/// 同一条链路，但这一单的篇数上限由调用方给。
///
/// 「规则口径要 20」与「这一单能拿回 8」是两个数。上限不小于口径时，从哪个数推都得出同一个
/// 答案，缺陷因此藏得住；只有上限更小的这一条链路才问得清记录里记的到底是哪一个。
pub async fn submit_peer_domain_package_with_quota(
    database: &Database,
    identity_key: &str,
    lane: &str,
    task_target: serde_json::Value,
    quota: i32,
    package_kind: &str,
    coverage: serde_json::Value,
    checkpoint: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> Uuid {
    submit_package_for_domain(
        database,
        PEER_DOMAIN,
        identity_key,
        lane,
        task_target,
        quota,
        package_kind,
        coverage,
        checkpoint,
        records,
    )
    .await
}

/// 同一条链路，Domain 可选。每个 Domain 都走同一套采集和材料接纳。
#[allow(clippy::too_many_arguments)]
pub async fn submit_package_for_domain(
    database: &Database,
    domain_ref: &str,
    identity_key: &str,
    lane: &str,
    task_target: serde_json::Value,
    quota: i32,
    package_kind: &str,
    coverage: serde_json::Value,
    checkpoint: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> Uuid {
    // 生命周期一律留在默认的 pending_decision。关键词**不能**进入 archiving/archived
    // （`0042` 的 CHECK），建档与否是读取时从证据里查出来的；领域分流也不看生命周期。
    let target_ref = Uuid::new_v4();
    create_target(database, target_ref, domain_ref, identity_key).await;
    submit_package_for_target(
        database,
        target_ref,
        identity_key,
        lane,
        task_target,
        quota,
        package_kind,
        coverage,
        checkpoint,
        records,
    )
    .await
}

async fn create_target(
    database: &Database,
    target_ref: Uuid,
    domain_ref: &str,
    identity_key: &str,
) {
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword',$2,$2,'manual')",
    )
    .bind(target_ref)
    .bind(identity_key)
    .execute(database.pool())
    .await
    .expect("the target is stored");
    sqlx::query(
        "INSERT INTO observation_domain_target(domain_ref,target_ref,role) \
         VALUES ($1::uuid,$2,'primary')",
    )
    .bind(domain_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("the target has an explicit Domain relation");
}

/// 同一条链路，但挂在**已有的观察目标**上。
///
/// 「第二轮采集」与「第二个目标」是两件事。一篇笔记的详情必须在同一个目标底下取，否则
/// 任何按 `target_ref` 关联两轮的判据都会静默落空——而测试会因此为错误的原因变绿。
#[allow(clippy::too_many_arguments)]
pub async fn submit_package_for_target(
    database: &Database,
    target_ref: Uuid,
    identity_key: &str,
    lane: &str,
    task_target: serde_json::Value,
    quota: i32,
    package_kind: &str,
    coverage: serde_json::Value,
    checkpoint: serde_json::Value,
    records: Vec<serde_json::Value>,
) -> Uuid {
    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) \
         SELECT $1,$2,relation.domain_ref,relation.role,$3,'domain sampling proof','person' \
           FROM observation_domain_target relation \
          WHERE relation.target_ref=$2 AND relation.role='primary'",
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
         VALUES ($1,'xhs','keyword',$2,'domain sampling proof','person', \
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
         VALUES ($1,$2,'admitted','domain_sampling_proof',$3,$4)",
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
        "INSERT INTO collection_work_order \
             (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions,dispatch_lane,queue_state,scheduled_for) \
         VALUES ($1,$2,$3,$4,$5,'[\"maximum_quota\"]'::jsonb,'batch','queued',scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(lane)
    .bind(quota)
    .execute(database.pool())
    .await
    .expect("work order fixture is stored");
    sqlx::query(
        "INSERT INTO collection_work_order_domain_usage \
             (work_order_ref,request_ref,domain_ref,role,basis_kind) \
             SELECT $1,request.request_ref,request.domain_ref,request.observation_role,'admitted' \
           FROM collection_acquisition_request request WHERE request.request_ref=$2",
    )
    .bind(work_order_ref)
    .bind(request_ref)
    .execute(database.pool())
    .await
    .expect("the fixture WorkOrder has a frozen Domain purpose");

    // 工位名在同一个证明库里必须唯一：一条用例会连着提交两轮（列表面 + 详情面）。
    let station_ref =
        register_station(database, &format!("Domain采样证明工位 {identity_key}"), 200)
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
    // 说明书里「这一轮该拿回多少」，按派发侧同一条算法：口径取前 N 与这一单篇数上限取小；
    // 没设取前 N（建档、详情这类）就是上限本身。夹具手写说明书也得照这条算法写，差这个数，
    // 「记录读的是冻结值」这条断言就测不到真东西。
    let expected_count = match task_target
        .get("topByLikes")
        .and_then(serde_json::Value::as_i64)
        .and_then(|top_by_likes| i32::try_from(top_by_likes).ok())
    {
        Some(top_by_likes) => top_by_likes.min(quota),
        None => quota,
    };
    let task = serde_json::json!({
        "contractVersion":"linggan.producer.task-spec.v1","taskId":task_id,"source":"scheduled",
        "platform":"xhs","pageType":"search_results","target":task_target,
        "capabilitiesRequested":[package_kind],"maximumQuota":quota,
        "expectedCount":expected_count,
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
        "domain submission must be acknowledged: {outcome:?}"
    );
    package_ref
}

pub fn search_coverage(query: &str, acquired: i64) -> serde_json::Value {
    serde_json::json!({
        "target":{"basis":"current_visible_surface","surface":"target_driven_surface","query":query},
        "layers":[{
            "capability":"discovery_search","observed":acquired,"attempted":acquired,
            "acquired":acquired,"verified":0,"failed":0,"notAttempted":0,"unknown":0,
            "stoppedReason":"surface_read_complete"
        }]
    })
}

pub fn discovery_card(external_id: &str, title: &str, likes: &str, url: &str) -> serde_json::Value {
    discovery_card_at(external_id, title, likes, url, 0)
}

/// 带上插件报告的发现位次（`_discoveryOrder`，从 0 起计），与线上包里实际出现的字段同名。
pub fn discovery_card_at(
    external_id: &str,
    title: &str,
    likes: &str,
    url: &str,
    discovery_order: i64,
) -> serde_json::Value {
    serde_json::json!({
        "kind":"discovery_card","resultPosition":1,
        "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
        "payload":{
            "noteId":external_id,"title":title,"likes":likes,"url":url,
            "_discoveryOrder":discovery_order
        }
    })
}
