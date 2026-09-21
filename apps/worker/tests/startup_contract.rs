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
