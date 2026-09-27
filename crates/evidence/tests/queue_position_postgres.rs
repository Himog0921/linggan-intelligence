//! 「我刚才那个请求，现在排在第几」的真实证明。
//!
//! 这个数会被人拿来判断还要等多久，所以它必须用派发自己的那把尺子量，而且只能在**同一
//! 条派发通道内**比较：immediate 与 scheduled、batch 各有各的队列、各自独立出队，
//! 「在 batch 里排第 1」与「在 immediate 里排第 51」之间没有先后关系。

#[path = "support/domain_fixture.rs"]
mod domain;
#[path = "support/material_fixture.rs"]
mod fixture;

use domain::ADHD_DOMAIN;
use fixture::proof_database;
use linggan_evidence::read_target_queue_positions;
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 造一张排队中的工单。`created_at` 由调用顺序决定，同一目标最近创建的那张才是人刚点的。
async fn queue_work_order(database: &Database, target_ref: Uuid, dispatch_lane: &str) -> Uuid {
    let request_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_acquisition_request \
             (request_ref,target_ref,domain_ref,observation_role,lane,purpose,requested_by) \
         SELECT $1,$2,relation.domain_ref,relation.role,'patrol','queue position proof','person' \
           FROM observation_domain_target relation \
          WHERE relation.target_ref=$2 AND relation.role='primary'",
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
         VALUES ($1,'xhs','keyword','patrol','queue position proof','person', \
                 scope_001_now()+interval '1 day', \
                 ARRAY['keyword_patrol'],ARRAY['immediate','scheduled','batch'],200)",
    )
    .bind(authorization_ref)
    .execute(database.pool())
    .await
    .expect("authorization fixture is stored");
    let decision_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_admission_decision \
             (decision_ref,request_ref,outcome,reason_code,authorization_ref,target_ref) \
         VALUES ($1,$2,'admitted','within_authorization',$3,$4)",
    )
    .bind(decision_ref)
    .bind(request_ref)
    .bind(authorization_ref)
    .bind(target_ref)
    .execute(database.pool())
    .await
    .expect("decision fixture is stored");
    let work_order_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_work_order \
             (work_order_ref,decision_ref,target_ref,lane,max_works,stop_conditions, \
              dispatch_lane,queue_state,scheduled_for,dedupe_key,retry_not_before_at) \
         VALUES ($1,$2,$3,'patrol',20,'[\"maximum_quota\"]'::jsonb,$4,'queued', \
                 scope_001_now(),$1::text,scope_001_now())",
    )
    .bind(work_order_ref)
    .bind(decision_ref)
    .bind(target_ref)
    .bind(dispatch_lane)
    .execute(database.pool())
    .await
    .expect("work order fixture is stored");
    work_order_ref
}

async fn keyword_target(database: &Database, identity_key: &str) -> Uuid {
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
             (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword',$2,$2,'manual')",
    )
    .bind(target_ref)
    .bind(identity_key)
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
    .expect("target fixture has an explicit Domain relation");
    target_ref
}

/// 名次只在同一条通道内可比，而且问的是**人刚点的那一张**。
///
/// 这里的目标在 batch 通道里是当时唯一一张（排第 1），随后人又点了一次，落在 immediate
/// 通道、前面堵着 50 张别人的工单。跨通道取最小值会报出「下一个就是它」——人刚点的那下
/// 其实还要等 50 张，而这正是这个数字本来要消除的那种沉默。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_position_is_counted_inside_one_lane_for_the_most_recent_request() {
    let database = proof_database("queue_position_per_lane").await;
    let mine = keyword_target(&database, "考研自习::queue-mine").await;

    // 先有一张 batch 的：它在自己那条通道里排第一。
    queue_work_order(&database, mine, "batch").await;
    // 别人的 50 张 immediate 先排着。
    for index in 0..50 {
        let other = keyword_target(&database, &format!("考研自习::queue-other-{index}")).await;
        queue_work_order(&database, other, "immediate").await;
    }
    // 然后人点了一次，落在 immediate 的第 51 位。
    queue_work_order(&database, mine, "immediate").await;

    let positions = read_target_queue_positions(&database, &[mine])
        .await
        .expect("queue positions are readable");
    let position = positions.get(&mine).expect("the target is queued");
    assert_eq!(
        position.ahead, 50,
        "报的必须是人刚点的那张在它自己通道里的名次，不是它在最空那条通道里的名次"
    );
    assert_eq!(position.ready, 2, "这个目标确实有两张现在就能派的工单");
    assert_eq!(position.waiting_for_time, 0);
}

/// 排在最前面时前面就是 0 个——这是真话，界面自己决定怎么说。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn the_only_queued_order_has_nobody_ahead() {
    let database = proof_database("queue_position_front").await;
    let mine = keyword_target(&database, "考研自习::queue-front").await;
    queue_work_order(&database, mine, "immediate").await;

    let positions = read_target_queue_positions(&database, &[mine])
        .await
        .expect("queue positions are readable");
    let position = positions.get(&mine).expect("the target is queued");
    assert_eq!(position.ahead, 0);
    assert_eq!(position.ready, 1);
}

/// 还没到点的工单不占当前队列位置，但也不能当作「没有排队」。
///
/// 它等的是时间，不是工位；混进 `ahead` 会让人看到一个不会随工位空闲而变小的数字。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_order_waiting_for_its_time_is_counted_separately() {
    let database = proof_database("queue_position_scheduled").await;
    let mine = keyword_target(&database, "考研自习::queue-later").await;
    let work_order_ref = queue_work_order(&database, mine, "scheduled").await;
    sqlx::query(
        "UPDATE collection_work_order SET scheduled_for=scope_001_now()+interval '1 hour' \
         WHERE work_order_ref=$1",
    )
    .bind(work_order_ref)
    .execute(database.pool())
    .await
    .expect("the order is pushed into the future");

    let positions = read_target_queue_positions(&database, &[mine])
        .await
        .expect("queue positions are readable");
    let position = positions.get(&mine).expect("the target has queued work");
    assert_eq!(position.ready, 0, "没到点的不算现在就能派");
    assert_eq!(position.waiting_for_time, 1);
    assert_eq!(
        position.ahead, 0,
        "没有一张现在就能派的工单时，名次不成立，不该拿它当「前面还有几个」"
    );
}

/// 队列里没有它，就不该出现在返回里——「没有排队」与「读不到」是两件事，
/// 后者由 `Err` 表达，不能靠一个空条目冒充。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_target_without_queued_work_is_absent_rather_than_zero() {
    let database = proof_database("queue_position_absent").await;
    let mine = keyword_target(&database, "考研自习::queue-none").await;

    let positions = read_target_queue_positions(&database, &[mine])
        .await
        .expect("queue positions are readable");
    assert!(positions.get(&mine).is_none());
}
