//! 存量重排：把旧版本处理器做过的作业，按当前版本重新排一遍。
//!
//! 2026-09-17。为什么必须先抬版本号、为什么重排是追加而不是覆盖，见
//! `crates/evidence/src/material_processing.rs` 里 `processor_version_for_kind` 的说明。
//! 这组用例钉住的是重排本身容易出错的四件事：**只补旧版**（不重排没有新版本的那两类、
//! 也不碰没点名的类）、**幂等**（重复运行是 0 条而不是报错）、**同一个键上多个旧版本只补一条**
//! （两条源行会长出同一个唯一键）、**排出来的作业真的会被 worker 领走**（不是排完就没人管）。
//!
//! 版本号在用例里**刻意重写一遍**（`BASELINE`/`CURRENT`），不从生产代码导出：抬版本号是一次
//! 需要想清楚存量怎么办的改动，用例要能独立发现「有人改了版本号却没同步这组断言」。

#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{proof_database, submit_package};
use linggan_evidence::{
    admit_media_blob, claim_media_processing_work, ensure_media_processing_work,
    processor_version_for_kind, requeue_outdated_processor_jobs,
};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

/// 与生产代码逐字相同的两份版本号。`local-v2` 是 2026-09-17 抬上去的那一代：
/// Tesseract 语言表去掉 `chi_tra`、whisper 换 medium 并修掉子进程 PATH。
const BASELINE: &str = "local-v1";
const CURRENT: &str = "local-v2";
/// 重排出来的作业在事件流里的理由，与摄取路径的 `queued_for_local_processor` 分开。
const REQUEUE_REASON: &str = "requeued_after_processor_version_upgrade";

/// 重排的来源是「库里已经躺着的那条旧作业」。重排工具自己不造这种行——它们都是当初摄取时
/// 按旧版本准入的。所以这里绕过准入直接写一条旧版本作业，把「存量」这件事还原出来。
async fn insert_outdated_job(
    database: &Database,
    blob_sha256: &str,
    slot_key: &str,
    processor_kind: &str,
    processor_version: &str,
) -> Uuid {
    let job_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_media_processing_job \
         (job_ref,blob_sha256,slot_key,processor_kind,processor_version,input_scope) \
         VALUES($1,$2,$3,$4,$5,'full_blob')",
    )
    .bind(job_ref)
    .bind(blob_sha256)
    .bind(slot_key)
    .bind(processor_kind)
    .bind(processor_version)
    .execute(database.pool())
    .await
    .expect("an already-admitted style job row is insertable");
    job_ref
}

async fn jobs_of(database: &Database, processor_kind: &str, processor_version: &str) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT job_ref FROM linggan_media_processing_job \
         WHERE processor_kind=$1 AND processor_version=$2",
    )
    .bind(processor_kind)
    .bind(processor_version)
    .fetch_all(database.pool())
    .await
    .expect("jobs are readable")
}

async fn job_row(database: &Database, job_ref: Uuid) -> (String, Option<String>, String, String) {
    let row = sqlx::query(
        "SELECT blob_sha256,slot_key,processor_version,input_scope \
         FROM linggan_media_processing_job WHERE job_ref=$1",
    )
    .bind(job_ref)
    .fetch_one(database.pool())
    .await
    .expect("the job is readable");
    (
        row.get("blob_sha256"),
        row.get("slot_key"),
        row.get("processor_version"),
        row.get("input_scope"),
    )
}

async fn event_rows(database: &Database, job_ref: Uuid) -> Vec<(String, Option<String>)> {
    let rows = sqlx::query(
        "SELECT state,reason FROM linggan_media_processing_job_event WHERE job_ref=$1 \
         ORDER BY occurred_at",
    )
    .bind(job_ref)
    .fetch_all(database.pool())
    .await
    .expect("events are readable");
    rows.iter()
        .map(|row| (row.get("state"), row.get("reason")))
        .collect()
}

/// 这次重排真正新排出来的那一条：前后两次取当前版本作业，差集就是它。
/// 不能拿「不是那条旧的」来认——准入时本来就已经有一条当前版本的作业了。
async fn enqueued_by(
    database: &Database,
    processor_kind: &str,
    processor_version: &str,
    before: &[Uuid],
) -> Uuid {
    *jobs_of(database, processor_kind, processor_version)
        .await
        .iter()
        .find(|job_ref| !before.contains(job_ref))
        .expect("the requeue added exactly one job of this kind")
}

/// 让一条作业看起来「已经做过了」。`linggan_media_processing_work` 是那张**可变**的控制表
/// （没有只追加触发器，证据表才有），所以这不是在改证据——它是把生产库里的存量状态还原出来：
/// 那边 2243 条 `image_ocr` 全是 `completed`，可领的活只该是这次新排出来的。
async fn mark_work_completed(database: &Database, job_ref: Uuid) {
    let updated = sqlx::query(
        "UPDATE linggan_media_processing_work SET state='completed',completed_at=scope_001_now(), \
         updated_at=scope_001_now() WHERE job_ref=$1 AND state='pending'",
    )
    .bind(job_ref)
    .execute(database.pool())
    .await
    .expect("the mutable control row accepts a terminal state");
    assert_eq!(updated.rows_affected(), 1, "the job had a pending work row");
}

/// 造一张图：经既有媒体链产生 `thumbnail` 与 `image_ocr` 两条作业。
async fn seed_image(database: &Database, index: usize) -> (String, String) {
    let content_id = format!("note-requeue-image-{index}");
    let slot_key = format!("xhs:{content_id}:image:1");
    let observation_ref = Uuid::new_v4();
    let uri = format!("https://media.example/requeue-image-{index}.jpg");
    submit_package(
        database,
        "media_slots",
        serde_json::json!({"contentExternalId":content_id}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":observation_ref,
            "slot":{"role":"image","ordinal":1},
            "observation":{
                "externalUri":uri,"candidateUris":[uri],"observedAt":"2026-08-28T10:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
        }),
    )
    .await;
    let sha256 = format!("{:064x}", index);
    admit_media_blob(
        database,
        observation_ref,
        &sha256,
        "image/jpeg",
        4096,
        &format!("blobs/{}/{}", &sha256[..2], sha256),
    )
    .await
    .expect("an image materializes through the existing media chain");
    (sha256, slot_key)
}

/// 造一段视频：经既有媒体链产生 `thumbnail`、`audio_extract`、`asr`、`video_frame_ocr` 四条作业。
async fn seed_video(database: &Database, index: usize) -> (String, String) {
    let content_id = format!("note-requeue-video-{index}");
    let slot_key = format!("xhs:{content_id}:video:1");
    let observation_ref = Uuid::new_v4();
    let uri = format!("https://media.example/requeue-video-{index}.mp4");
    submit_package(
        database,
        "media_slots",
        serde_json::json!({"contentExternalId":content_id}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":observation_ref,
            "slot":{"role":"video","ordinal":1},
            "observation":{
                "externalUri":uri,"candidateUris":[uri],"observedAt":"2026-08-28T10:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
        }),
    )
    .await;
    let sha256 = format!("{:064x}", index);
    admit_media_blob(
        database,
        observation_ref,
        &sha256,
        "video/mp4",
        8192,
        &format!("blobs/{}/{}", &sha256[..2], sha256),
    )
    .await
    .expect("a video materializes through the existing media chain");
    (sha256, slot_key)
}

/// 造一条**只有旧版本作业**的媒体键——也就是生产库今天的形状。
///
/// 这里不能用 `seed_image`／`seed_video`：那两条路会经准入**当场**建出当前版本的
/// `image_ocr`／`asr`／`video_frame_ocr`，键上从此就有了一条 `local-v2`。而重排的判据正是
/// 「这个键上还没有当前版本」——于是它**正确地**一条都不排，用例却以为自己测到了存量。
/// 第一版就是这么写的，7 条断言在隔离库上全红（`left: 0, right: 1`）。
///
/// 所以刻意用一个**不派生任何处理器**的字节类型来种：`processors_for_mime` 只认 `image/`
/// 与 `video/`，别的一律返回空表。槽位、字节、素材照常建出来（下面的外键要靠它们），
/// 键上却只留着用例自己插进去的那条旧作业。字节类型与作业种类在这里不匹配是有意的：
/// 这一组用例量的是**重排的键运算**，不是媒体类型派发。
async fn seed_media_without_jobs(
    database: &Database,
    index: usize,
    slot_role: &str,
) -> (String, String) {
    let content_id = format!("note-requeue-legacy-{index}");
    let slot_key = format!("xhs:{content_id}:{slot_role}:1");
    let observation_ref = Uuid::new_v4();
    let uri = format!("https://media.example/requeue-legacy-{index}.bin");
    submit_package(
        database,
        "media_slots",
        serde_json::json!({"contentExternalId":content_id}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":observation_ref,
            "slot":{"role":slot_role,"ordinal":1},
            "observation":{
                "externalUri":uri,"candidateUris":[uri],"observedAt":"2026-08-28T10:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
        }),
    )
    .await;
    let sha256 = format!("{:064x}", index);
    admit_media_blob(
        database,
        observation_ref,
        &sha256,
        "application/octet-stream",
        4096,
        &format!("blobs/{}/{}", &sha256[..2], sha256),
    )
    .await
    .expect("a jobless media key materializes through the existing media chain");
    let jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_media_processing_job WHERE blob_sha256=$1",
    )
    .bind(&sha256)
    .fetch_one(database.pool())
    .await
    .expect("jobs are readable");
    assert_eq!(jobs, 0, "这条种子只该留下键，不该留下任何作业");
    (sha256, slot_key)
}

fn kinds(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// 准入这条路自己就得用当前版本：不这么做，新采的字节会一进来就是「旧版」，
/// 于是「重排」会把它再跑一遍，而库里从此同时躺着两代同名结果。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn admission_uses_the_registered_version_per_kind() {
    let database = proof_database("media_requeue_admission_versions").await;
    seed_image(&database, 1).await;
    seed_video(&database, 2).await;

    // 一次问全表，而不是逐个种类问——逐种类问的话，**漏掉一个种类**（比如将来新增了
    // 第六种处理器）不会有任何断言变红，这组断言会安静地少守一格。
    let rows = sqlx::query(
        "SELECT processor_kind,processor_version FROM linggan_media_processing_job \
         ORDER BY processor_kind,processor_version",
    )
    .fetch_all(database.pool())
    .await
    .expect("jobs are readable");
    let versions: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.get("processor_kind"), row.get("processor_version")))
        .collect();
    assert_eq!(
        versions,
        vec![
            // 一张图 + 一段视频：两个 `thumbnail`、一个 `image_ocr`，视频那三条各一个。
            ("asr".to_owned(), CURRENT.to_owned()),
            ("audio_extract".to_owned(), BASELINE.to_owned()),
            ("image_ocr".to_owned(), CURRENT.to_owned()),
            ("thumbnail".to_owned(), BASELINE.to_owned()),
            ("thumbnail".to_owned(), BASELINE.to_owned()),
            ("video_frame_ocr".to_owned(), CURRENT.to_owned()),
        ],
        "准入这条路必须按 `processor_version_for_kind` 逐类取版本：行为变过的三类抬到 {CURRENT}，\
         没变的两类**不许**跟着抬——抬了就会凭空多出一轮谁也没要求过的重跑"
    );
}

/// 只补旧版：没点名的类一条不动；点了名但本来就没有新版本的那两类，一条也不排。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeue_covers_only_the_outdated_jobs_of_the_named_kinds() {
    let database = proof_database("media_requeue_scope").await;
    let (image_sha, image_slot) = seed_media_without_jobs(&database, 1, "image").await;
    let (video_sha, video_slot) = seed_media_without_jobs(&database, 2, "video").await;
    insert_outdated_job(&database, &image_sha, &image_slot, "image_ocr", BASELINE).await;
    insert_outdated_job(&database, &video_sha, &video_slot, "asr", BASELINE).await;
    insert_outdated_job(&database, &video_sha, &video_slot, "video_frame_ocr", BASELINE).await;
    // 行为没变的两类也各放一条旧作业：要点名的类里有一类「本来就没有新版本」，
    // 那一条要在点了名之后**一条都不多**，而不是靠它压根不存在来蒙混过关。
    insert_outdated_job(&database, &image_sha, &image_slot, "thumbnail", BASELINE).await;
    insert_outdated_job(&database, &video_sha, &video_slot, "audio_extract", BASELINE).await;

    // 点名的这一类：旧的那条补出一条同键的新版本作业，旧的一条不动。
    let summary = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("the named kind is registered");
    assert_eq!(summary.len(), 1);
    assert_eq!(summary[0].processor_kind, "image_ocr");
    assert_eq!(summary[0].target_version, CURRENT);
    assert_eq!(summary[0].enqueued_jobs, 1);
    assert_eq!(jobs_of(&database, "image_ocr", CURRENT).await.len(), 1);
    assert_eq!(jobs_of(&database, "image_ocr", BASELINE).await.len(), 1);

    // 没点名的三类：一条不动。
    for (kind, version) in [
        ("asr", BASELINE),
        ("asr", CURRENT),
        ("video_frame_ocr", BASELINE),
        ("video_frame_ocr", CURRENT),
    ] {
        let expected = usize::from(version == BASELINE);
        assert_eq!(
            jobs_of(&database, kind, version).await.len(),
            expected,
            "{kind} @{version}"
        );
    }

    // 转录与视频帧文字：这一次一起排，各自补出一条。
    let summary =
        requeue_outdated_processor_jobs(&database, &kinds(&["asr", "video_frame_ocr"]), false)
            .await
            .expect("both kinds are registered");
    assert_eq!(summary[0].enqueued_jobs, 1);
    assert_eq!(summary[1].enqueued_jobs, 1);
    assert_eq!(jobs_of(&database, "asr", CURRENT).await.len(), 1);
    assert_eq!(jobs_of(&database, "video_frame_ocr", CURRENT).await.len(), 1);
    assert_eq!(jobs_of(&database, "asr", BASELINE).await.len(), 1);

    // 行为没变的两类：目标版本就是它们已经躺着的版本，点了名也一条都排不出来。
    let summary =
        requeue_outdated_processor_jobs(&database, &kinds(&["thumbnail", "audio_extract"]), false)
            .await
            .expect("both kinds are registered");
    assert_eq!(summary[0].target_version, BASELINE);
    assert_eq!(summary[1].target_version, BASELINE);
    assert_eq!(summary[0].enqueued_jobs, 0);
    assert_eq!(summary[1].enqueued_jobs, 0);
    assert_eq!(jobs_of(&database, "thumbnail", BASELINE).await.len(), 1);
    assert_eq!(jobs_of(&database, "audio_extract", BASELINE).await.len(), 1);
}

/// 补出来的作业与旧的那条同键、只差版本号，并且带上一条与摄取路径分开记的 `pending` 事件。
/// 读路径取作业的最后一条事件当状态（`material_media_read.rs`），少了它界面会显示成没有状态。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeue_preserves_the_job_keys_and_writes_one_pending_event() {
    let database = proof_database("media_requeue_event").await;
    let (sha256, slot_key) = seed_media_without_jobs(&database, 1, "image").await;
    let outdated =
        insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", BASELINE).await;

    let before = jobs_of(&database, "image_ocr", CURRENT).await;
    requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("the named kind is registered");
    let enqueued = enqueued_by(&database, "image_ocr", CURRENT, &before).await;

    let (blob, slot, version, scope) = job_row(&database, enqueued).await;
    assert_eq!(blob, sha256);
    assert_eq!(slot.as_deref(), Some(slot_key.as_str()));
    assert_eq!(version, CURRENT);
    assert_eq!(scope, "full_blob");
    assert_eq!(
        event_rows(&database, enqueued).await,
        vec![("pending".to_owned(), Some(REQUEUE_REASON.to_owned()))]
    );
    // 旧那条作业原样留着：重排是追加，不是替换。
    let (_, _, old_version, _) = job_row(&database, outdated).await;
    assert_eq!(old_version, BASELINE);
}

/// 幂等：同一件事跑第二遍是 0 条，不是报错——生产上这条工具要能安全地重跑。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeue_is_idempotent() {
    let database = proof_database("media_requeue_idempotent").await;
    let (sha256, slot_key) = seed_media_without_jobs(&database, 1, "image").await;
    insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", BASELINE).await;

    let first = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("the named kind is registered");
    let second = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("a second run is not an error");
    assert_eq!(first[0].enqueued_jobs, 1);
    assert_eq!(second[0].enqueued_jobs, 0);
    assert_eq!(jobs_of(&database, "image_ocr", CURRENT).await.len(), 1);
    assert_eq!(jobs_of(&database, "image_ocr", BASELINE).await.len(), 1);
}

/// 同一个键上躺着两个旧版本（抬过不止一次版本时会出现）：只补一条。
/// 两条源行会长出**同一个唯一键**，一条语句里插两遍同键的行会当场违反唯一约束、整句失败。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeue_adds_one_job_when_one_key_holds_several_older_versions() {
    let database = proof_database("media_requeue_two_older_versions").await;
    let (sha256, slot_key) = seed_media_without_jobs(&database, 1, "image").await;
    insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", "local-v0").await;
    insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", BASELINE).await;

    let summary = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("two older versions on one key do not break the statement");
    assert_eq!(summary[0].enqueued_jobs, 1);
    assert_eq!(jobs_of(&database, "image_ocr", CURRENT).await.len(), 1);
    // 两个旧版本都原样留着。
    assert_eq!(jobs_of(&database, "image_ocr", BASELINE).await.len(), 1);
    assert_eq!(jobs_of(&database, "image_ocr", "local-v0").await.len(), 1);
}

/// 排出来只是「插了作业」；它真的会被 worker 领走才算数。可认领的 work 行由 worker 每 tick 的
/// `ensure_media_processing_work` 投影，认领仍走 `claim_media_processing_work`（含 0088 的并发闸）。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeued_jobs_become_claimable_work() {
    let database = proof_database("media_requeue_claimable").await;
    let (sha256, slot_key) = seed_media_without_jobs(&database, 1, "image").await;
    let outdated =
        insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", BASELINE).await;
    ensure_media_processing_work(&database)
        .await
        .expect("every job becomes runnable work");
    // 存量在库里的样子：那两条都已经做过了，可领的活只该是重排出来的那条。
    for job_ref in jobs_of(&database, "image_ocr", CURRENT).await {
        mark_work_completed(&database, job_ref).await;
    }
    mark_work_completed(&database, outdated).await;
    let before = jobs_of(&database, "image_ocr", CURRENT).await;

    let summary = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("the named kind is registered");
    assert_eq!(summary[0].enqueued_jobs, 1);
    let enqueued = enqueued_by(&database, "image_ocr", CURRENT, &before).await;
    ensure_media_processing_work(&database)
        .await
        .expect("the requeued job becomes runnable work");

    let claim = claim_media_processing_work(&database, Uuid::new_v4(), &kinds(&["image_ocr"]))
        .await
        .expect("the claim is readable")
        .expect("the requeued job is claimable");
    assert_eq!(claim.job_ref, enqueued);
    assert_eq!(claim.processor_kind, "image_ocr");
    assert_eq!(claim.processor_version, CURRENT);
    assert_eq!(claim.blob_sha256, sha256);
}

/// 预演是「真的做一遍再撤回」：报的条数与真做一致，库里一条不留。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn dry_run_reports_what_a_real_run_would_do_and_leaves_nothing_behind() {
    let database = proof_database("media_requeue_dry_run").await;
    let (sha256, slot_key) = seed_media_without_jobs(&database, 1, "image").await;
    insert_outdated_job(&database, &sha256, &slot_key, "image_ocr", BASELINE).await;

    let preview = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), true)
        .await
        .expect("a dry run is readable");
    assert_eq!(preview[0].enqueued_jobs, 1);
    assert_eq!(jobs_of(&database, "image_ocr", CURRENT).await.len(), 0);

    let real = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr"]), false)
        .await
        .expect("the named kind is registered");
    assert_eq!(real[0].enqueued_jobs, preview[0].enqueued_jobs);
    assert_eq!(jobs_of(&database, "image_ocr", CURRENT).await.len(), 1);
}

/// 名字打错必须当场报错。安静地排 0 条会被读成「这一类没有旧作业」——正是本次重排要消灭的
/// 失败形态（196 条 asr 里 195 条静默失败就是这么藏了三天）。顺带钉住版本表本身。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn requeue_refuses_a_kind_that_has_no_registered_version() {
    let database = proof_database("media_requeue_unregistered").await;

    assert_eq!(processor_version_for_kind("image_ocr"), Some(CURRENT));
    assert_eq!(processor_version_for_kind("video_frame_ocr"), Some(CURRENT));
    assert_eq!(processor_version_for_kind("asr"), Some(CURRENT));
    assert_eq!(processor_version_for_kind("thumbnail"), Some(BASELINE));
    assert_eq!(processor_version_for_kind("audio_extract"), Some(BASELINE));
    assert_eq!(processor_version_for_kind("image_ocr_v2"), None);

    let error = requeue_outdated_processor_jobs(&database, &kinds(&["image_ocr_ocr"]), true)
        .await
        .expect_err("an unregistered processor kind is refused");
    assert!(
        error.to_string().contains("image_ocr_ocr"),
        "报错要点出是哪个名字：{error}"
    );
}
