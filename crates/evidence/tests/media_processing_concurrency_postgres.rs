//! 媒体处理的并发上限：由库里的策略决定，不由「有几个进程在跑」决定。
//!
//! 2026-09-16 线上校准。设计上并发是 1——worker 内部逐条串行（`media_worker.rs` 的
//! `for _ in 0..MAX_JOBS_PER_TICK` 每条都 `.await`），launchd 也只托管一个进程。实际是 6：
//! 另外 5 个是从已被删除的 worktree 遗留的孤儿进程，跑着目录都不存在的旧二进制，各自持有到
//! 生产库的连接并持续认领工单，三天里每个累计约 40 分钟 CPU。
//!
//! **代码里每一处看过去都是对的，上限却形同虚设**：因为「同时最多几条」此前只由进程数这一个
//! 没人管的量决定，而进程数会被任何一次「换个目录再起一个」改变。这些用例把上限钉在策略表上。
//!
//! ## 这些用例为什么不赛跑
//!
//! 第一版写成「两个 `tokio::join!` 的认领者，断言只有一个拿到」。它绿着——但把闸锁摘掉之后
//! **它照样绿**。原因是 `#[tokio::test]` 默认单线程运行时，两个 future 在同一个线程上交替，
//! 先跑的那个早已把事务提交完，后跑的那个根本没和它撞上。那条断言测的不是锁，是「这件事在
//! 这台机器上跑得够快」。
//!
//! 所以下面改用**等待**而不是**抢先**：让一条连接把闸锁拿住不放，然后看认领会不会停在它上面。
//! 锁没放掉之前等待方不可能完成，这是确定的；而赛跑只在时序凑巧时才红。
//!
//! ## 这组用例必须串行跑
//!
//! 咨询锁按 **database** 生效，不按 schema——`isolated_proof_schema` 给每个用例一套独立的表，
//! 但锁的命名空间是共用的。`claiming_waits_on_the_shared_gate_lock` 等的是「有人正等在这把
//! 锁上」，若另一个用例同时也在等同一把锁，它会满足在一个不相干的事件上。目前由
//! `scripts/test-local-001-discovery-postgres.sh` 里的 `RUST_TEST_THREADS=1` 挡住；
//! 若将来有人并行跑这组用例，先解决这件事，别指望它还能测到原来的东西。
//! （摘掉闸锁的突变验证在并行下**仍然会红**——那三个用例断言的是计数谓词与 INNER JOIN，
//! 本就不依赖锁——所以这里说的是语义变弱，不是会假绿。）

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{proof_database, submit_package};
use linggan_evidence::{
    ClaimGateReadiness, admit_media_blob, claim_media_processing_work,
    ensure_media_processing_work, read_claim_gate_readiness,
};
use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 认领闸用的咨询锁名。与 `material_processing.rs` 里那一处必须逐字相同——
/// 这里刻意重写一遍而不是从生产代码导出常量：用例要能独立发现「那把锁换了名字」。
const GATE_LOCK_NAME: &str = "linggan_media_processing_claim_gate";

/// 造 `count` 张图。每张经既有媒体链产生一条 `image_ocr` 与一条 `thumbnail` 作业。
async fn seed_images(database: &Database, count: usize) {
    for index in 0..count {
        let content_id = format!("note-concurrency-{index}");
        let observation_ref = Uuid::new_v4();
        let uri = format!("https://media.example/concurrency-{index}.jpg");
        submit_package(
            database,
            "media_slots",
            serde_json::json!({"contentExternalId":content_id}),
            serde_json::json!({
                "kind":"media_slot",
                "slotKey":format!("xhs:{content_id}:image:1"),
                "observationRef":observation_ref,
                "slot":{"role":"image","ordinal":1},
                "observation":{
                    "externalUri":uri,"candidateUris":[uri],"observedAt":"2026-08-28T10:00:00Z"
                },
                "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
            }),
        )
        .await;
        let sha256 = format!("{index:064x}");
        admit_media_blob(
            database,
            observation_ref,
            &sha256,
            "image/jpeg",
            4,
            &format!("blobs/{}/{}", &sha256[..2], sha256),
        )
        .await
        .expect("an image materializes through the existing media chain");
    }
}

async fn seed_runnable_images(database: &Database, count: usize) {
    seed_images(database, count).await;
    ensure_media_processing_work(database)
        .await
        .expect("every job becomes runnable work");
}

fn image_ocr() -> Vec<String> {
    vec!["image_ocr".to_owned()]
}

async fn leased_count(database: &Database, processor_kind: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM linggan_media_processing_work work \
         JOIN linggan_media_processing_job job USING(job_ref) \
         WHERE work.state='leased' AND job.processor_kind=$1",
    )
    .bind(processor_kind)
    .fetch_one(database.pool())
    .await
    .expect("leased work is readable")
}

/// 认领必须停在闸锁上。
///
/// 这是「数在途」与「领一条」之间没有缝的全部依据：两件事在同一把锁下，才有「我数到 0，
/// 到我把状态改成 leased 为止，不会有第二个人也数到 0」。少了这把锁，`FOR UPDATE ...
/// SKIP LOCKED` 反而会让两个人各拿一条不同的——它保证的是不抢同一条，不是不超过上限。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn claiming_waits_on_the_shared_gate_lock() {
    let database = proof_database("media_concurrency_gate_lock").await;
    seed_runnable_images(&database, 3).await;

    // 占住闸锁，直到本用例主动放掉。另开一条连接，不能占用池里剩下的认领与轮询位置。
    let mut blocker = database
        .pool()
        .begin()
        .await
        .expect("a blocker transaction");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(GATE_LOCK_NAME)
        .execute(&mut *blocker)
        .await
        .expect("the gate lock is acquirable");

    let claimer = tokio::spawn({
        let database = database.clone();
        async move { claim_media_processing_work(&database, Uuid::new_v4(), &image_ocr()).await }
    });

    // 等「有人正等在这把锁上」出现。等的是条件，不是固定时长：认领不走锁时它永远不出现，
    // 用例就会红，而不是碰运气。
    let mut waiting = 0_i64;
    for _ in 0..100 {
        waiting = sqlx::query_scalar(
            "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
        )
        .fetch_one(database.pool())
        .await
        .expect("lock state is readable");
        if waiting > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        waiting > 0,
        "认领没有停在闸锁上：它根本没走这把锁，「数在途」与「领一条」之间就还留着缝"
    );
    assert!(!claimer.is_finished(), "锁还被占着，认领不该已经完成");

    blocker.rollback().await.expect("the blocker releases");

    let claimed = claimer
        .await
        .expect("the claim task joins")
        .expect("the claim attempt answers");
    assert!(claimed.is_some(), "锁放掉之后认领应当正常完成");
}

/// 名额满了就不再发第二条租约。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_full_processor_kind_issues_no_second_lease() {
    let database = proof_database("media_concurrency_ceiling").await;
    seed_runnable_images(&database, 8).await;

    let first = claim_media_processing_work(&database, Uuid::new_v4(), &image_ocr())
        .await
        .expect("a claim attempt answers");
    assert!(first.is_some(), "空的时候应当领得到");
    assert_eq!(leased_count(&database, "image_ocr").await, 1);

    // 名额被别人占着时，后来者拿不到——不是排队，是这一次就没有。
    let second = claim_media_processing_work(&database, Uuid::new_v4(), &image_ocr())
        .await
        .expect("a claim attempt answers");
    assert!(second.is_none(), "上限已满时不该发出第二条 image_ocr 租约");
    assert_eq!(
        leased_count(&database, "image_ocr").await,
        1,
        "库里实际处于 leased 的 image_ocr 只能有一条"
    );

    // 别的 processor_kind 名额独立，不受 image_ocr 占满的影响。
    let thumbnail =
        claim_media_processing_work(&database, Uuid::new_v4(), &["thumbnail".to_owned()])
            .await
            .expect("a claim attempt answers");
    assert!(
        thumbnail.is_some(),
        "thumbnail 自己的名额是空的，不该被 image_ocr 挡住"
    );
}

/// 闸门必须**说得出**它为什么领不到活，不能只表现为「领不到」。
///
/// 「未迁移」与「未登记」这两种状态下，`claim` 都返回 `Ok(None)`——与「这一轮没有活」逐字
/// 相同，worker 一行日志都不打，进程看着完全健康却永远空转。这正是本包要消灭的失败形态
/// （195 条 asr 就是零日志藏出来的），所以这两种状态各钉一条断言，并要求它们**彼此可区分**：
/// 「迁移没跑」和「这个处理器没登记」要改的地方完全不同。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn the_claim_gate_reports_what_it_cannot_claim() {
    let database = proof_database("media_concurrency_readiness").await;

    // 登记齐了就不吭声。
    assert_eq!(
        read_claim_gate_readiness(&database, &image_ocr())
            .await
            .expect("the readiness is readable"),
        ClaimGateReadiness::Ready
    );

    // 启用了却没登记——「新增一种处理器、两边都忘了改」当场的样子。要点名报出来，
    // 而不是笼统地说「有问题」。
    assert_eq!(
        read_claim_gate_readiness(
            &database,
            &["image_ocr".to_owned(), "future_kind".to_owned()]
        )
        .await
        .expect("the readiness is readable"),
        ClaimGateReadiness::Unregistered(vec!["future_kind".to_owned()])
    );

    // 闸门表整个不在（`0088` 未应用）是另一回事，要说得出是「迁移没跑」。
    // 这个用例拥有一套独立的 schema，所以删得。
    sqlx::query("DROP TABLE linggan_media_processing_concurrency")
        .execute(database.pool())
        .await
        .expect("the policy table can be withdrawn in this isolated schema");
    assert_eq!(
        read_claim_gate_readiness(&database, &image_ocr())
            .await
            .expect("the readiness is readable"),
        ClaimGateReadiness::NotMigrated
    );
}

/// 上限表里没登记的 processor_kind，一条都认领不到。
///
/// 这是有意的 Closed World（与 SCOPE-001 一致）：新增一种处理器却忘了登记上限时，它应该停下来
/// **被人看见**（上面那条用例管的就是「被看见」），而不是没有上限地跑起来。判据走 INNER JOIN，
/// 所以删掉登记行，认领必须当场归零。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_unregistered_processor_kind_is_never_claimable() {
    let database = proof_database("media_concurrency_closed_world").await;
    seed_runnable_images(&database, 3).await;

    // 前提：登记着的时候领得到。否则下面那条断言会因为别的原因变绿。
    let before = claim_media_processing_work(&database, Uuid::new_v4(), &image_ocr())
        .await
        .expect("a claim attempt answers");
    assert!(before.is_some(), "登记着的时候应当领得到");

    sqlx::query(
        "DELETE FROM linggan_media_processing_concurrency WHERE processor_kind='image_ocr'",
    )
    .execute(database.pool())
    .await
    .expect("the policy row can be withdrawn");
    // 把刚领走的那条还回去，否则「领不到」可能只是因为它已经在别人手里。
    sqlx::query(
        "UPDATE linggan_media_processing_work SET state='pending',worker_instance_ref=NULL, \
           lease_expires_at=NULL WHERE state='leased'",
    )
    .execute(database.pool())
    .await
    .expect("the lease is returned");

    let after = claim_media_processing_work(&database, Uuid::new_v4(), &image_ocr())
        .await
        .expect("a claim attempt answers");
    assert!(
        after.is_none(),
        "上限表里没有登记的处理器，一条都不该被认领"
    );
}

/// 新增 processor_kind 时不会漏登记上限。
///
/// 判据取自 `linggan_media_processing_job.processor_kind` 的 **CHECK 约束本身**，不抄一份常量
/// 清单——抄一份的话，将来加了一种处理器、两边都忘了改，这个用例照样绿，那就等于没设。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn every_processor_kind_the_schema_allows_is_registered_for_concurrency() {
    let database = proof_database("media_concurrency_coverage").await;
    let definition: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint \
         WHERE conrelid='linggan_media_processing_job'::regclass AND contype='c' \
           AND pg_get_constraintdef(oid) LIKE '%processor_kind%'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the processor_kind constraint is readable");

    // 约束写成一个 `ARRAY['thumbnail'::text, ...]` 字面量；引号之间的就是允许的取值。
    let allowed = definition
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert!(
        allowed.contains(&"image_ocr".to_owned()),
        "约束解析失败，取不到已知取值：{definition}"
    );

    let registered: Vec<String> = sqlx::query_scalar(
        "SELECT processor_kind FROM linggan_media_processing_concurrency ORDER BY processor_kind",
    )
    .fetch_all(database.pool())
    .await
    .expect("the concurrency policy is readable");

    for kind in &allowed {
        assert!(
            registered.contains(kind),
            "schema 允许 `{kind}`，并发上限表里却没有登记，它会认领不到任何活"
        );
    }
    for kind in &registered {
        assert!(
            allowed.contains(kind),
            "上限表登记了 `{kind}`，schema 里却没有这种处理器——多半是拼错了，永远不会生效"
        );
    }
}
