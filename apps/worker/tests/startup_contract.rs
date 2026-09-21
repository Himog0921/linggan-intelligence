//! 巡检 worker 的启动与退出语义（COLLECTION-UPGRADE-001 · S4a/S4b · 验收行 T29/T30）。
//!
//! 这一组测的是**进程**而不是函数：判据落在退出码与启动日志上——正是 supervisor 和看日志的
//! 人真正能看到的那两样东西。改之前这两条路都是干净的退出（`main` 返回 `()` 就是退出码 0），
//! 对 launchd 是一次正常退出，对看日志的人来说这台机器什么都没说。
//!
//! 两条不需要数据库的用例默认就跑；第三条要一个真实 PostgreSQL（证明库的 public schema 里
//! 没有任何迁移），挂在证明套件的 `--ignored` 组里。

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use linggan_storage_postgres::{Database, testing::isolated_proof_schema};

const WORKER_BINARY: &str = env!("CARGO_BIN_EXE_linggan-worker");
const MEDIA_WORKER_BINARY: &str = env!("CARGO_BIN_EXE_linggan-media-worker");

/// 一个必然没人监听的本地端口：连接会被立刻拒绝，测试不必等连接超时。
const UNREACHABLE_DATABASE_URL: &str =
    "postgresql://startup_contract:startup_contract@127.0.0.1:1/startup_contract";

/// 起来一个 worker，边跑边收它说的话。
struct WorkerRun {
    child: Child,
    log: std::thread::JoinHandle<String>,
}

impl WorkerRun {
    fn start(database_url: Option<&str>) -> Self {
        Self::start_binary(WORKER_BINARY, database_url)
    }

    fn start_binary(binary: &str, database_url: Option<&str>) -> Self {
        let mut command = Command::new(binary);
        // 不继承开发机的环境：这些用例问的是「没给地址/给了一个连不上的地址时它怎么反应」，
        // 不是「这台机器现在怎么配置」。
        command
            .env_remove("LINGGAN_LOCAL_DATABASE_URL")
            .env_remove("LINGGAN_SUPPORT_DIR")
            .env_remove("LINGGAN_WORKER_DRAIN_ACK_PATH")
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        if let Some(database_url) = database_url {
            command.env("LINGGAN_LOCAL_DATABASE_URL", database_url);
        }
        let mut child = command.spawn().expect("the worker binary runs");
        let stdout = child.stdout.take().expect("stdout is piped");
        let log = std::thread::spawn(move || {
            let mut text = String::new();
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                text.push_str(&line);
                text.push('\n');
            }
            text
        });
        Self { child, log }
    }

    /// 让它跑一段时间，然后结束它，交回「跑完这段时间还活着吗」与它说过的话。
    fn observe_for(mut self, wait: Duration) -> (bool, String) {
        std::thread::sleep(wait);
        let alive = self
            .child
            .try_wait()
            .expect("the worker is inspectable")
            .is_none();
        let _ = self.child.kill();
        let _ = self.child.wait();
        let log = self.log.join().expect("the log reader joins");
        (alive, log)
    }
}

#[test]
fn missing_database_url_is_a_failure_not_an_empty_work_queue() {
    let output = Command::new(WORKER_BINARY)
        .env_remove("LINGGAN_LOCAL_DATABASE_URL")
        .env_remove("LINGGAN_SUPPORT_DIR")
        .env_remove("LINGGAN_WORKER_DRAIN_ACK_PATH")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("the worker binary runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "配置错误必须以失败退出：以成功退出等于告诉系统一切正常"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refusing to start"),
        "要说清是拒绝启动，而不是照常工作：{stderr}"
    );
    assert!(
        !String::from_utf8_lossy(&output.stdout).contains("patrol tick every"),
        "没有数据库就不该进入 tick"
    );
}

#[test]
fn unreachable_database_keeps_the_process_waiting_and_says_so() {
    // 观察窗口要长过一次判定的时间上限（10 秒）：没有那道上限时，连接池会自己重试 30 秒
    // 才把失败交回来，这台机器整整半分钟和一台空闲机器长得一模一样。
    let (alive, log) =
        WorkerRun::start(Some(UNREACHABLE_DATABASE_URL)).observe_for(Duration::from_secs(15));

    assert!(
        alive,
        "连不上数据库时要留在原地按退避等它回来，而不是退出（那样 supervisor 只会反复重启它）：{log}"
    );
    assert!(
        log.contains("not ready (database_unreachable"),
        "未就绪必须说出来，而不是安静地当空闲：{log}"
    );
    assert!(
        !log.contains("patrol tick every"),
        "未就绪期间不进入任何 tick 步骤：{log}"
    );

    // 人读的散文行之外，stdout 上还要有一行机器读的事件。白名单那条测试证明的是**定义**
    // （字段是闭集、每个字段都有生产者），这一条证明它真的走到了 supervisor 看的那条流上：
    // 事件一旦悄悄换成 stderr 或换掉字段名，白名单仍然是绿的。
    let events: Vec<serde_json::Value> = log
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let readiness_event = events
        .iter()
        .find(|event| event["event"] == linggan_evidence::EVENT_READINESS)
        .unwrap_or_else(|| panic!("未就绪时该有一行 readiness 事件：{log}"));
    assert_eq!(readiness_event["service"], linggan_evidence::SERVICE_WORKER);
    assert_eq!(readiness_event["outcome"], "database_unreachable");
    for event in &events {
        for key in event.as_object().expect("事件是一行 JSON 对象").keys() {
            assert!(
                linggan_evidence::EVENT_FIELD_WHITELIST.contains(&key.as_str()),
                "事件字段是闭集，`{key}` 不在白名单里：{log}"
            );
        }
    }
}

#[test]
fn media_worker_without_a_database_url_is_a_failure_too() {
    let output = Command::new(MEDIA_WORKER_BINARY)
        .env_remove("LINGGAN_LOCAL_DATABASE_URL")
        .env_remove("LINGGAN_SUPPORT_DIR")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("the media worker binary runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "两个 worker 对同一个配置错误必须给出同一个答复"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("refusing to start"),
        "要说清是拒绝启动，而不是照常工作"
    );
}

#[test]
fn unreachable_database_keeps_the_media_worker_retrying_instead_of_exiting() {
    // 媒体 worker 以前在这条路上「打印一行然后 return」——退出码 0，对 launchd 是一次干净退出。
    // 现在它留在原地退避重试，并且**十秒内**就把这件事说出来（连接池自己的耐心是 30 秒）。
    let (alive, log) = WorkerRun::start_binary(MEDIA_WORKER_BINARY, Some(UNREACHABLE_DATABASE_URL))
        .observe_for(Duration::from_secs(15));

    assert!(
        alive,
        "连不上数据库时要留在原地按退避等它回来，而不是退出（那样 supervisor 只会反复重启它）：{log}"
    );
    assert!(
        log.contains("started; checking whether the local database is reachable"),
        "开场就要说一声，不能前 30 秒一片空白：{log}"
    );
    assert!(
        log.contains("retrying with backoff instead of exiting"),
        "连不上必须说出来，而不是安静地当空闲：{log}"
    );
    assert!(
        !log.contains("local processors"),
        "没接上数据库就不该进入 tick：{log}"
    );
    assert!(
        !log.contains("postgresql://") && !log.contains("startup_contract:startup_contract"),
        "连不上时打印的错误不得带上连接串（那里面有密码）：{log}"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn reachable_but_unmigrated_database_is_classified_not_masked() {
    // 证明库的 public schema 里没有任何迁移——每个用例各自在独立 schema 里建自己的库——
    // 所以台账表不存在。这正是最容易被糊过去的一类：连得上，但没人知道迁移跑到哪了。
    let database_url =
        std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");

    let (alive, log) = WorkerRun::start(Some(&database_url)).observe_for(Duration::from_secs(3));

    assert!(alive, "可接活的判据不成立时也要留在原地等：{log}");
    assert!(
        log.contains("not ready (migration_ledger_unreadable"),
        "连得上、台账不在，就该报这一分类，而不是 database_unreachable 或什么都不说：{log}"
    );
    assert!(
        !log.contains("patrol tick every"),
        "未就绪期间不进入任何 tick 步骤：{log}"
    );
}

/// 未就绪的分类必须落在心跳里，而不是只留在日志上。
///
/// `0098` 给心跳加 `readiness_state` / `readiness_detail` / `readiness_checked_at` 三列，
/// 理由写在迁移头上：日志与 `/health` 都是**即时**视图，重启即丢，而「这台机器从上周起就
/// 一直没迁移完」要在重启之后仍然查得出来。列注释也写着同一件事：anything other than
/// ready means this machine must not claim work。
///
/// 而 worker 此前只在**就绪**那一支写心跳：未就绪的一支打完日志就退避重试。于是那五类
/// 「不能接活」写得进、查得出，却没人写——库里那一行永远停在 `ready` 或 `unknown`，
/// 恰恰是操作者最需要的那条事实缺失。
///
/// 这条用例问的是**调用点**：起一个真的 worker 进程，让它面对一个「连得上、但接不了活」
/// 的库（迁移全部应用，台账里唯独不登记 `0034`），然后不读日志、只读心跳那一行。
///
/// 夹具的边界说清楚：写入逻辑跑在生产代码上，这条用例只提供落点。真表由 `0020` 建、
/// `0098` 加列，同一条 UPDATE 落进**真表**已由 `linggan-evidence` 的
/// `readiness_is_a_heartbeat_column_that_does_not_touch_the_tick_columns` 在完整迁移链上
/// 证明过；这里要证的是另一件事——未就绪分支真的会调用它。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn an_unready_worker_leaves_its_classification_in_the_heartbeat() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let schema = "worker_readiness_landing";
    isolated_proof_schema(
        &url,
        schema,
        &migrations_missing("0034_collection_control_closure"),
    )
    .await
    .expect("the fixture schema is built from the real migration directory");

    // 子进程只拿得到一个连接串，所以 schema 走 libpq 的 `options`——它与
    // `Database::connect_within_schema` 写进 startup 包的是同一个参数。
    let (alive, log) = WorkerRun::start(Some(&format!("{url}?options=-csearch_path%3D{schema}")))
        .observe_for(Duration::from_secs(3));
    assert!(alive, "接不了活时要留在原地等它变好，而不是退出：{log}");
    assert!(
        log.contains("not ready (migrations_not_applied"),
        "台账缺一个被要求的 migration id，就该报这一分类：{log}"
    );

    let database = Database::connect_within_schema(&url, schema)
        .await
        .expect("the isolated schema is reachable");
    let (state, detail, checked_at): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT readiness_state,readiness_detail,readiness_checked_at::text \
         FROM collection_scheduler_heartbeat WHERE scheduler_key='patrol'",
    )
    .fetch_one(database.pool())
    .await
    .expect("the worker left a heartbeat row behind");
    assert_eq!(
        state, "migrations_not_applied",
        "未就绪的分类要落在心跳里；只留在日志上等于重启就丢"
    );
    assert_eq!(
        detail.as_deref(),
        Some("0034_collection_control_closure"),
        "原因要是限制过的那个标识，不是一句散文"
    );
    assert!(checked_at.is_some(), "判定时刻来自数据库时钟");
}

/// 把真实迁移目录按文件名顺序拼成一段 SQL，并在台账里登记**除** `missing` 之外的每一条。
///
/// 不手抄一份迁移清单：抄一份就会漂移，而漂移的夹具绿的没有意义。台账在生产上由
/// `scripts/local-runtime.sh migrate` 写；这里由夹具代笔，因为探针读的就是台账，
/// 而「台账说这一条没跑」正是要造出来的那个状态。
///
/// 登记用的 `sha256` 是占位值：探针只按 `migration_id` 判在不在，不比对内容——内容校验在
/// 应用迁移那一步，不在判定能不能接活的这一步。
fn migrations_missing(missing: &str) -> String {
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../database/migrations");
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(&directory)
        .expect("the migrations directory is readable")
        .map(|entry| entry.expect("the directory entry is readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "sql"))
        .collect();
    paths.sort();
    assert!(paths.len() > 90, "迁移目录看起来不完整");

    let mut sql = String::from(
        "CREATE TABLE linggan_local_schema_migration (\n\
           migration_id text PRIMARY KEY,\n\
           migration_sha256 text NOT NULL CHECK (migration_sha256 ~ '^[0-9a-f]{64}$'),\n\
           applied_at timestamptz NOT NULL DEFAULT clock_timestamp()\n\
         );\n",
    );
    let mut registered = Vec::new();
    for path in &paths {
        sql.push_str(&std::fs::read_to_string(path).expect("the migration is readable"));
        sql.push('\n');
        let id = path
            .file_stem()
            .expect("a migration file has a stem")
            .to_string_lossy()
            .into_owned();
        if id != missing {
            registered.push(id);
        }
    }
    let rows: Vec<String> = registered
        .iter()
        .map(|id| format!("('{}', repeat('0', 64))", id.replace('\'', "''")))
        .collect();
    sql.push_str(&format!(
        "INSERT INTO linggan_local_schema_migration (migration_id, migration_sha256) VALUES\n{};\n",
        rows.join(",\n")
    ));
    sql
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn sigterm_drains_even_while_the_collection_tick_is_blocked() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let schema = "worker_tick_drain";
    let database = isolated_proof_schema(&url, schema, &migrations_missing("none"))
        .await
        .expect("the complete isolated schema is built");
    let mut blocker = database.pool().begin().await.unwrap();
    sqlx::query("LOCK TABLE collection_scheduler_run IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let ack_dir =
        std::env::temp_dir().join(format!("linggan-drain-proof-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&ack_dir).unwrap();
    struct ProcessGuard(Child, std::path::PathBuf);
    impl Drop for ProcessGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
            let _ = std::fs::remove_dir_all(&self.1);
        }
    }
    let ack = ack_dir.join("ack");
    let mut worker = ProcessGuard(
        Command::new(WORKER_BINARY)
            .env(
                "LINGGAN_LOCAL_DATABASE_URL",
                format!("{url}?options=-csearch_path%3D{schema}"),
            )
            .env_remove("LINGGAN_SUPPORT_DIR")
            .env("LINGGAN_WORKER_DRAIN_ACK_PATH", &ack)
            .env("LINGGAN_COLLECTION_UPGRADE_PHASE", "recovery")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
        ack_dir,
    );
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_locks waiting \
                 WHERE waiting.relation='collection_scheduler_run'::regclass \
                   AND NOT waiting.granted)",
            )
            .fetch_one(database.pool())
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the real worker reaches the blocked tick insert");
    assert!(
        Command::new("kill")
            .args(["-TERM", &worker.0.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    let status = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            if let Some(status) = worker.0.try_wait().unwrap() {
                break status;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("SIGTERM must drain while the database lock remains held");
    assert!(status.success());
    let receipt = std::fs::read_to_string(&ack).expect("the worker confirms drain");
    assert!(receipt.contains(&format!("pid={}\nstate=drained\n", worker.0.id())));
    blocker.rollback().await.unwrap();
}
