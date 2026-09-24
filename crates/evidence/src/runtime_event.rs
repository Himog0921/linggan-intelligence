//! 运行事件：一行一个 JSON 对象。
//!
//! 诊断为什么要有形状？因为「现在是不是每一轮都在失败」此前只能靠读散文日志猜：每行都是
//! 一句给人看的话，没有 `service`、没有 revision、没有一次 tick 的关联号，也没法过滤。
//! 结构化事件不是把日志变漂亮，是让「哪一个进程、哪个 revision、哪一轮 tick、哪一步、
//! 为什么、花了多久」成为可判定的字段。
//!
//! 两条硬边界，都由类型而不是约定保证：
//!
//! 1. **字段是闭集**。事件只有一张固定字段表（[`EVENT_FIELD_WHITELIST`]），结构体里没有
//!    「随手塞一个 map」的入口，也没有只写在白名单里、实际没人写的字段。诊断样例与字段
//!    白名单因此是可核对的，不是承诺。
//! 2. **理由是受限码**。`reason` 与 `error_class` 只接受已经长得像受限码的东西（小写字母、
//!    数字、下划线），其余一律记 `unclassified`：报文、连接串、选择器串、页面文本都过不了
//!    这道门。这不是美化，是防止「为了排查把原文打出来」——那正是这条链路最容易出的事故，
//!    而**把散文按字符挑成一个像码的词**会以更隐蔽的方式犯同一个错（见 `bounded_code`）。
//!
//! 时间用的是**进程时钟**（`ts`），与就绪判定的 `checked_at`（数据库时钟）是两个时钟，
//! 如实分开标注：数据库在本机容器里时两者同源，但不把「进程说的」冒充「数据库说的」。
//! 事件不访问数据库——一行日志不该多一次往返。

use std::{
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use uuid::Uuid;

/// 事件类型。闭集：新的类型先在这里登记，字段表与白名单同步更新。
pub const EVENT_STARTUP: &str = "startup";
pub const EVENT_READINESS: &str = "readiness";
pub const EVENT_TICK: &str = "tick";
pub const EVENT_TICK_STEP: &str = "tick_step";

/// 谁在说话。取值对应 `scripts/runtime/install.sh` 装的三个 launchd 服务。
pub const SERVICE_WORKER: &str = "worker";
pub const SERVICE_MEDIA_WORKER: &str = "media-worker";

/// 事件行允许出现的顶层字段。测试据此断言，文档（runbook「运行诊断」）据此给白名单。
///
/// **逐条引用只到 tick 号为止。** 目标、工单、租约、任务、尝试、提交这些引用住在数据库里
/// （`collection_scheduler_target_decision` 与工单链），事件只带能进入那条链的入口。把引用
/// 抄进日志会多出一份会漂移的副本，也会让「什么算敏感」在第二个地方重新判一遍。
///
/// 每个条目都必须有生产者：`every_field_the_event_can_carry_is_whitelisted_and_used` 断言
/// 一个写满所有 setter 的事件与这张表**逐字相等**。留一个永远不出现的条目，会让人以为某个
/// 字段「有时候在」。
pub const EVENT_FIELD_WHITELIST: &[&str] = &[
    "ts",
    "service",
    "revision",
    "event",
    "tickRef",
    "stepKey",
    "outcome",
    "reason",
    "errorClass",
    "durationMs",
    "considered",
    "produced",
    "skipped",
];

/// 进程所属部署的 revision：`sync.sh` 写、`launch.sh` 导出路径、进程读。
///
/// 读不到就是 `unknown`——不知道就写不知道，不拿别的东西冒充。这个值只读一次：一次部署
/// 期间它不会变，而每个事件都去读一遍文件是没有意义的 I/O。
pub fn runtime_revision() -> &'static str {
    static REVISION: OnceLock<String> = OnceLock::new();
    REVISION.get_or_init(|| {
        revision_from_identity_path(
            std::env::var("LINGGAN_RUNTIME_IDENTITY_PATH")
                .ok()
                .as_deref(),
        )
    })
}

/// 判定本身是纯的：给一个路径，要么读出可信的 revision，要么 `unknown`。
fn revision_from_identity_path(path: Option<&str>) -> String {
    let Some(path) = path else {
        return "unknown".to_owned();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return "unknown".to_owned();
    };
    let Ok(identity) = serde_json::from_str::<RuntimeIdentity>(&raw) else {
        return "unknown".to_owned();
    };
    let revision = identity.revision.trim();
    // 只接受可能是 revision 的形状（十六进制、分支名），其余一律不写进日志。
    let acceptable = !revision.is_empty()
        && revision.len() <= 64
        && revision
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if acceptable {
        revision.to_owned()
    } else {
        "unknown".to_owned()
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeIdentity {
    revision: String,
}

/// 一行运行事件。构造时只给「谁、什么事件」，其余字段按需附加。
#[derive(Debug, Clone)]
pub struct RuntimeEvent {
    line: EventLine,
}

impl RuntimeEvent {
    pub fn new(service: &'static str, event: &'static str) -> Self {
        Self {
            line: EventLine {
                ts: utc_rfc3339(SystemTime::now()),
                service,
                revision: runtime_revision(),
                event,
                tick_ref: None,
                step_key: None,
                outcome: None,
                reason: None,
                error_class: None,
                duration_ms: None,
                considered: None,
                produced: None,
                skipped: None,
            },
        }
    }

    /// 这一轮 tick（也就是 `collection_scheduler_run.scheduler_run_ref`）。
    pub fn with_tick(mut self, tick_ref: Uuid) -> Self {
        self.line.tick_ref = Some(tick_ref.to_string());
        self
    }

    pub fn with_step(mut self, step_key: &'static str) -> Self {
        self.line.step_key = Some(step_key);
        self
    }

    pub fn with_outcome(mut self, outcome: &'static str) -> Self {
        self.line.outcome = Some(outcome);
        self
    }

    /// 受限原因码。不是 `[a-z0-9_]` 的东西在这里就被挡掉。
    pub fn with_reason(mut self, reason: &str) -> Self {
        self.line.reason = Some(bounded_code(reason));
        self
    }

    /// 受限错误分类码（如 SQLSTATE 类别）。
    pub fn with_error_class(mut self, error_class: &str) -> Self {
        self.line.error_class = Some(bounded_code(error_class));
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.line.duration_ms = Some(i64::try_from(duration.as_millis()).unwrap_or(i64::MAX));
        self
    }

    /// 这一步数过的计数。没数的（不统计的那一项）留 `None`：0 是一个事实，不能拿来冒充没数。
    pub fn with_counts(
        mut self,
        considered: Option<i64>,
        produced: Option<i64>,
        skipped: Option<i64>,
    ) -> Self {
        self.line.considered = considered.map(|value| value.max(0));
        self.line.produced = produced.map(|value| value.max(0));
        self.line.skipped = skipped.map(|value| value.max(0));
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.line)
            .unwrap_or_else(|_| "{\"event\":\"event_unserializable\"}".to_owned())
    }

    /// 写到标准输出。服务日志由 launchd 收走；事件不另开文件、不另建通道。
    pub fn emit(&self) {
        println!("{}", self.to_json());
    }
}

#[derive(Debug, Clone, Serialize)]
struct EventLine {
    ts: String,
    service: &'static str,
    revision: &'static str,
    event: &'static str,
    #[serde(rename = "tickRef", skip_serializing_if = "Option::is_none")]
    tick_ref: Option<String>,
    #[serde(rename = "stepKey", skip_serializing_if = "Option::is_none")]
    step_key: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(rename = "errorClass", skip_serializing_if = "Option::is_none")]
    error_class: Option<String>,
    #[serde(rename = "durationMs", skip_serializing_if = "Option::is_none")]
    duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    considered: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    produced: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    skipped: Option<i64>,
}

/// 受限码：小写字母、数字与下划线，最长 32 字符。
///
/// 全仓只此一处把任意字符串变成受限码：事件行用它，账本的 `reason`/`error_class` 列也用它。
/// 两份实现会漂移，而漂移的那一份就是漏出去的那一份。
///
/// **不把散文过滤成码。** 输入里只要出现字母、数字、下划线之外的任何字符——空格、冒号、
/// 斜杠、问号、`=`、中文——就整条记作 `unclassified`。按字符挑挑拣拣会从 `failed to fetch
/// https://x.test/a?token=secret` 里挑出 `failedtofetchhttpsxtestatoken`：它长得像一个原因码，
/// 却谁也不曾这么说过，而且把那段文字里的词原样带进了落盘的事实里。要么它本来就是受限形状，
/// 要么我们如实说「没分类」。
///
/// 大写折成小写（数据库给的 SQLSTATE 是 `42P01` 这种形状，而账本与事件的词表一律小写）；
/// 超过 32 字符**按前缀截断**（账本列写死 `≤32`）：受限标识真的会超长（如
/// `0098_scheduler_tick_steps_and_readiness`），截出来的是那个标识的前缀而不是编出来的词，
/// 完整的那份在心跳的 `readiness_detail` 里另存。
pub(crate) fn bounded_code(value: &str) -> String {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return "unclassified".to_owned();
    }
    value
        .chars()
        .take(32)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// 进程时钟的 UTC 时刻，RFC3339 形状。
fn utc_rfc3339(at: SystemTime) -> String {
    let seconds = at
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| i64::try_from(since_epoch.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0);
    let days = seconds.div_euclid(86_400);
    let time_of_day = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        time_of_day / 3_600,
        (time_of_day % 3_600) / 60,
        time_of_day % 60
    )
}

/// 天数 → 公历年月日（Howard Hinnant 的 `civil_from_days`，以 1970-01-01 为第 0 天）。
///
/// 自己算而不引依赖：仓库没有日期库，而这里只需要一个方向、一种历法。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event() -> RuntimeEvent {
        RuntimeEvent::new(SERVICE_WORKER, EVENT_TICK)
    }

    fn keys_of(json: &str) -> Vec<String> {
        serde_json::from_str::<serde_json::Value>(json)
            .expect("an event line is JSON")
            .as_object()
            .expect("an event line is an object")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn an_event_carries_only_whitelisted_fields() {
        let json = event()
            .with_tick(Uuid::nil())
            .with_step("patrol")
            .with_outcome("failed")
            .with_reason("not_due")
            .with_error_class("sqlstate_42P01")
            .with_duration(Duration::from_millis(12))
            .with_counts(Some(3), Some(1), Some(2))
            .to_json();
        for key in keys_of(&json) {
            assert!(
                EVENT_FIELD_WHITELIST.contains(&key.as_str()),
                "event line leaked a field outside the whitelist: {key}"
            );
        }
    }

    /// 白名单本身就是全部字段：少一个（字段没进白名单）或多一个（白名单留着没人写的字段）
    /// 都要拦下来——一个永远不出现的白名单条目会让人以为某个字段「有时候在」。
    #[test]
    fn every_field_the_event_can_carry_is_whitelisted_and_used() {
        let json = event()
            .with_tick(Uuid::nil())
            .with_step("patrol")
            .with_outcome("failed")
            .with_reason("not_due")
            .with_error_class("sqlstate_42p01")
            .with_duration(Duration::from_millis(12))
            .with_counts(Some(3), Some(1), Some(2))
            .to_json();
        let mut keys = keys_of(&json);
        keys.sort();
        let mut whitelist: Vec<String> = EVENT_FIELD_WHITELIST
            .iter()
            .map(|key| (*key).to_owned())
            .collect();
        whitelist.sort();
        assert_eq!(keys, whitelist);
    }

    /// 没数的计数不能变成 0：0 是「数过了，是零」，缺席是「这一步不数它」。
    #[test]
    fn unreported_counts_are_absent_rather_than_zero() {
        let json = event().with_counts(None, Some(2), None).to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["produced"], 2);
        assert!(value.get("considered").is_none());
        assert!(value.get("skipped").is_none());
    }

    #[test]
    fn a_minimal_event_still_says_who_and_what() {
        let json = event().to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["service"], SERVICE_WORKER);
        assert_eq!(value["event"], EVENT_TICK);
        assert!(value["ts"].as_str().unwrap().ends_with('Z'));
        assert!(value["revision"].is_string());
        assert!(value.get("tickRef").is_none());
    }

    #[test]
    fn reasons_are_bounded_codes_not_text() {
        let json = event()
            .with_reason("failed to fetch https://x.test/a?token=secret")
            .to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let reason = value["reason"].as_str().unwrap();
        assert!(
            reason
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
            "a reason must stay a bounded code, got {reason}"
        );
        assert!(reason.len() <= 32);
        assert!(!reason.contains("token"));
        assert!(!reason.contains("https"));
    }

    /// 「不把散文过滤成码」的另一面：**本来就受限形状**的东西必须原样留着。一条迁移 id 与
    /// 一句错误报文只差一个空格——前者不能因为长了几个字符就被丢成「没分类」。
    #[test]
    fn a_long_restricted_identifier_keeps_its_prefix_instead_of_vanishing() {
        assert_eq!(
            bounded_code("0098_scheduler_tick_steps_and_readiness"),
            "0098_scheduler_tick_steps_and_re",
            "超长的受限标识留前缀：完整的那份在心跳的 detail 里"
        );
        assert_eq!(
            bounded_code("42P01"),
            "42p01",
            "数据库给的 SQLSTATE 是大写，词表一律小写"
        );
        // 边界：正好 32 个字符原样通过；33 个字符切掉最后一个。
        assert_eq!(bounded_code(&"a".repeat(32)), "a".repeat(32));
        assert_eq!(bounded_code(&"a".repeat(33)), "a".repeat(32));
    }

    #[test]
    fn an_empty_reason_is_unclassified_rather_than_blank() {
        let json = event().with_reason("东京 · 页面").to_json();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["reason"], "unclassified");
    }

    #[test]
    fn process_clock_is_rendered_as_utc_seconds() {
        assert_eq!(utc_rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
        assert_eq!(
            utc_rfc3339(UNIX_EPOCH + Duration::from_secs(1_000_000_000)),
            "2001-09-09T01:46:40Z"
        );
        // 闰年 2 月 29 日与跨世纪：都走同一条整数算法，不靠时区数据库。
        assert_eq!(
            utc_rfc3339(UNIX_EPOCH + Duration::from_secs(951_782_400)),
            "2000-02-29T00:00:00Z"
        );
    }

    #[test]
    fn revision_is_read_from_the_deployment_identity_file() {
        let directory = std::env::temp_dir().join(format!("linggan-identity-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&directory).unwrap();
        let identity = directory.join("runtime-identity.json");
        std::fs::write(
            &identity,
            "{\"revision\":\"abc1234\",\"builtAt\":\"2026-09-21T00:00:00Z\",\"migrationHead\":\"0098\"}",
        )
        .unwrap();
        assert_eq!(
            revision_from_identity_path(Some(identity.to_str().unwrap())),
            "abc1234"
        );

        // 读不到的三种形态都必须说「不知道」，不能拿别的东西冒充部署标识。
        assert_eq!(revision_from_identity_path(None), "unknown");
        assert_eq!(
            revision_from_identity_path(Some("/nonexistent/linggan-identity.json")),
            "unknown"
        );
        std::fs::write(&identity, "{\"revision\":\"deadbeef token=secret\"}").unwrap();
        assert_eq!(
            revision_from_identity_path(Some(identity.to_str().unwrap())),
            "unknown"
        );
        std::fs::remove_dir_all(&directory).ok();
    }
}
