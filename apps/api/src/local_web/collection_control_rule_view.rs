// Target-scoped monitoring-rule read and render surface.
//
// This module owns no scheduling or admission decision. It reads the append-only active rule
// and durable command receipt written by `linggan_evidence`, then renders one server-owned
// modal outside the target list's batch form. A saved rule is never described as collection
// success; only a later Work/Lease/Task/Receipt chain may make that claim.
use super::super::target_drawer::TargetListContext;
use linggan_contracts::PRODUCER_TASK_SPEC_VERSION;
use linggan_evidence::{MonitorRuleMode, collection_control_schema_is_ready};
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

const COLLECTION_SCRIPT: &str = r#"<script src="/assets/collection-workspace.js"></script>"#;
const DEFAULT_INTERVAL_SECONDS: i32 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorRulePanelRead {
    Found(MonitorRulePanel),
    TargetNotFound,
    SchemaUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRulePanel {
    pub target_ref: Uuid,
    pub target_name: String,
    pub target_kind: String,
    pub lifecycle_state: String,
    pub active_rule: Option<ActiveMonitorRule>,
    pub receipt: Option<MonitorRuleReceiptView>,
    pub dynamic_cadence: DynamicCadenceView,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMonitorRule {
    pub rule_revision_ref: Uuid,
    pub revision: i32,
    pub mode: MonitorRuleMode,
    pub automatic_enabled: bool,
    pub run_on_weekdays: bool,
    pub run_on_weekends: bool,
    pub all_day: bool,
    pub window_start_minute: Option<i16>,
    pub window_end_minute: Option<i16>,
    pub fixed_interval_seconds: Option<i32>,
    pub fallback_interval_seconds: i32,
    pub surface_key: String,
    pub ranking_key: Option<String>,
    pub task_contract_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleReceiptView {
    pub receipt_ref: Uuid,
    pub command_kind: String,
    pub expected_revision: i32,
    pub outcome: String,
    pub reason_code: String,
    pub applied_rule_revision_ref: Option<Uuid>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DynamicCadenceView {
    Available {
        interval_seconds: i32,
        qualified_rounds: usize,
    },
    Unavailable {
        reason_code: &'static str,
    },
}

impl Default for DynamicCadenceView {
    fn default() -> Self {
        Self::Unavailable {
            reason_code: "comparable_rounds_not_projected",
        }
    }
}

/// Raw strings are retained only in the in-memory failed form. The route handler is
/// responsible for parsing the closed set before it calls the evidence command owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFormState {
    pub expected_revision: i32,
    pub idempotency_key: Uuid,
    pub mode: String,
    pub automatic_enabled: bool,
    pub run_on_weekdays: bool,
    pub run_on_weekends: bool,
    pub all_day: bool,
    pub window_start: String,
    pub window_end: String,
    pub fixed_interval_seconds: String,
    pub fallback_interval_seconds: String,
    pub surface_key: String,
    pub ranking_key: String,
    pub task_contract_version: String,
}

impl MonitorRuleFormState {
    pub fn from_panel(panel: &MonitorRulePanel) -> Self {
        let default_surface = if panel.target_kind == "keyword" {
            "keyword_search"
        } else {
            "creator_profile"
        };
        let default_ranking = if panel.target_kind == "keyword" {
            "comprehensive"
        } else {
            ""
        };
        let Some(rule) = panel.active_rule.as_ref() else {
            return Self {
                expected_revision: 0,
                idempotency_key: Uuid::new_v4(),
                mode: "fixed".to_owned(),
                automatic_enabled: false,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: true,
                window_start: String::new(),
                window_end: String::new(),
                fixed_interval_seconds: DEFAULT_INTERVAL_SECONDS.to_string(),
                fallback_interval_seconds: DEFAULT_INTERVAL_SECONDS.to_string(),
                surface_key: default_surface.to_owned(),
                ranking_key: default_ranking.to_owned(),
                task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
            };
        };
        Self {
            expected_revision: rule.revision,
            idempotency_key: Uuid::new_v4(),
            mode: rule.mode.as_str().to_owned(),
            automatic_enabled: rule.automatic_enabled,
            run_on_weekdays: rule.run_on_weekdays,
            run_on_weekends: rule.run_on_weekends,
            all_day: rule.all_day,
            window_start: minute_as_time(rule.window_start_minute),
            window_end: minute_as_time(rule.window_end_minute),
            fixed_interval_seconds: rule
                .fixed_interval_seconds
                .map_or_else(String::new, |value| value.to_string()),
            fallback_interval_seconds: rule.fallback_interval_seconds.to_string(),
            surface_key: rule.surface_key.clone(),
            ranking_key: rule.ranking_key.clone().unwrap_or_default(),
            task_contract_version: rule.task_contract_version.clone(),
        }
    }

    /// A rejected/stale/conflicting durable command owns its old idempotency key. Rendering
    /// the retained values with a new key lets the person correct and submit a new command
    /// instead of converting that correction into an identity conflict.
    pub fn with_new_command_identity(mut self) -> Self {
        self.idempotency_key = Uuid::new_v4();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFormError {
    pub code: &'static str,
    pub fields: Vec<MonitorRuleFieldError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFieldError {
    pub field: &'static str,
    pub message: &'static str,
}

/// Read only fields safe for the local UI. The query deliberately never selects credential
/// hashes, account digests, raw account identity, Cookie, HTML, or platform error text.
pub async fn read_monitor_rule_panel(
    database: &Database,
    target_ref: Uuid,
    receipt_ref: Option<Uuid>,
) -> Result<MonitorRulePanelRead, sqlx::Error> {
    if !collection_control_schema_is_ready(database).await? {
        return Ok(MonitorRulePanelRead::SchemaUnavailable);
    }
    let target = sqlx::query(
        "SELECT target_ref,target_kind,COALESCE(NULLIF(btrim(display_name),''),identity_key) AS target_name, \
                lifecycle_state,active_monitor_rule_revision_ref \
         FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?;
    let Some(target) = target else {
        return Ok(MonitorRulePanelRead::TargetNotFound);
    };
    let active_rule_ref: Option<Uuid> = target.try_get("active_monitor_rule_revision_ref")?;
    let active_rule = if let Some(rule_ref) = active_rule_ref {
        read_active_rule(database, target_ref, rule_ref).await?
    } else {
        None
    };
    let receipt = read_receipt(database, target_ref, receipt_ref).await?;
    Ok(MonitorRulePanelRead::Found(MonitorRulePanel {
        target_ref: target.try_get("target_ref")?,
        target_name: target.try_get("target_name")?,
        target_kind: target.try_get("target_kind")?,
        lifecycle_state: target.try_get("lifecycle_state")?,
        active_rule,
        receipt,
        // The current schema stores rule inputs and per-target decisions, but no durable
        // qualified-comparable-round projection. The UI therefore cannot recompute or imply
        // cadence availability. The scheduler may still use the explicit fixed fallback.
        dynamic_cadence: DynamicCadenceView::default(),
    }))
}

async fn read_active_rule(
    database: &Database,
    target_ref: Uuid,
    rule_ref: Uuid,
) -> Result<Option<ActiveMonitorRule>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT rule_revision_ref,revision,mode,automatic_enabled,run_on_weekdays, \
                run_on_weekends,all_day,window_start_minute,window_end_minute, \
                fixed_interval_seconds,fallback_interval_seconds,surface_key,ranking_key, \
                task_contract_version,created_at::text AS created_at \
         FROM collection_monitor_rule_revision \
         WHERE target_ref=$1 AND rule_revision_ref=$2",
    )
    .bind(target_ref)
    .bind(rule_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        let mode: String = row.try_get("mode")?;
        Ok(ActiveMonitorRule {
            rule_revision_ref: row.try_get("rule_revision_ref")?,
            revision: row.try_get("revision")?,
            mode: parse_mode(&mode),
            automatic_enabled: row.try_get("automatic_enabled")?,
            run_on_weekdays: row.try_get("run_on_weekdays")?,
            run_on_weekends: row.try_get("run_on_weekends")?,
            all_day: row.try_get("all_day")?,
            window_start_minute: row.try_get("window_start_minute")?,
            window_end_minute: row.try_get("window_end_minute")?,
            fixed_interval_seconds: row.try_get("fixed_interval_seconds")?,
            fallback_interval_seconds: row.try_get("fallback_interval_seconds")?,
            surface_key: row.try_get("surface_key")?,
            ranking_key: row.try_get("ranking_key")?,
            task_contract_version: row.try_get("task_contract_version")?,
            created_at: row.try_get("created_at")?,
        })
    })
    .transpose()
}

async fn read_receipt(
    database: &Database,
    target_ref: Uuid,
    receipt_ref: Option<Uuid>,
) -> Result<Option<MonitorRuleReceiptView>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT command_receipt_ref,command_kind,expected_revision,outcome,reason_code, \
                applied_rule_revision_ref,recorded_at::text AS recorded_at \
         FROM collection_monitor_rule_command_receipt \
         WHERE target_ref=$1 AND ($2::uuid IS NULL OR command_receipt_ref=$2) \
         ORDER BY recorded_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .bind(receipt_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(MonitorRuleReceiptView {
            receipt_ref: row.try_get("command_receipt_ref")?,
            command_kind: row.try_get("command_kind")?,
            expected_revision: row.try_get("expected_revision")?,
            outcome: row.try_get("outcome")?,
            reason_code: row.try_get("reason_code")?,
            applied_rule_revision_ref: row.try_get("applied_rule_revision_ref")?,
            recorded_at: row.try_get("recorded_at")?,
        })
    })
    .transpose()
}

fn parse_mode(value: &str) -> MonitorRuleMode {
    match value {
        "manual_only" => MonitorRuleMode::ManualOnly,
        "dynamic" => MonitorRuleMode::Dynamic,
        _ => MonitorRuleMode::Fixed,
    }
}

pub fn attach_to_collection_document(document: &str, overlay: &str) -> String {
    if overlay.is_empty() {
        return document.to_owned();
    }
    let Some(script_at) = document.rfind(COLLECTION_SCRIPT) else {
        return document.to_owned();
    };
    format!(
        "{}{}{}",
        &document[..script_at],
        overlay,
        &document[script_at..]
    )
}

pub fn render_monitor_rule_modal(
    panel: &MonitorRulePanel,
    form: &MonitorRuleFormState,
    error: Option<&MonitorRuleFormError>,
    list_context: TargetListContext<'_>,
) -> String {
    let opener_id = format!("monitor-rule-{}", panel.target_ref);
    let return_url = list_context.list_href(None);
    let close_href = list_context.list_href(Some(&opener_id));
    let return_filter = list_context.filter.unwrap_or_default();
    let return_sort = list_context.sort.unwrap_or_default();
    let disabled = panel.lifecycle_state == "dismissed";
    let revision = panel.active_rule.as_ref().map_or_else(
        || "尚无规则".to_owned(),
        |rule| format!("REV {}", rule.revision),
    );
    format!(
        r#"<div class="c-rule-overlay" data-monitor-rule-overlay>
          <section id="c-monitor-rule" class="c-rule-modal" role="dialog" aria-modal="true"
                   aria-labelledby="c-rule-title" data-return-url="{return_url}"
                   data-return-focus="{opener_id}">
            <header class="c-rule-head">
              <div>
                <p class="c-rule-context">{kind} · {revision}</p>
                <h2 id="c-rule-title" tabindex="-1" data-monitor-rule-initial-focus>监控规则 · {name}</h2>
                <p>保存只写入一版规则，不代表采集已经开始。暂停只停止未来自动调度，人工观察仍可申请。</p>
              </div>
              <a class="c-btn-quiet c-rule-close" href="{close_href}" aria-label="关闭监控规则" data-monitor-rule-close>关闭</a>
            </header>
            {feedback}
            {dismissed}
            <form class="c-rule-form" method="post" action="/collection/targets/rules" data-monitor-rule-form data-readonly="{readonly}">
              <input type="hidden" name="target_ref" value="{target_ref}">
              <input type="hidden" name="expected_revision" value="{expected_revision}">
              <input type="hidden" name="idempotency_key" value="{idempotency_key}">
              <input type="hidden" name="surface_key" value="{surface_key}">
              <input type="hidden" name="ranking_key" value="{ranking_key}">
              <input type="hidden" name="task_contract_version" value="{task_contract_version}">
              <input type="hidden" name="return_filter" value="{return_filter}">
              <input type="hidden" name="return_sort" value="{return_sort}">
              <div class="c-rule-body">
                {errors}
                <section class="c-rule-section" aria-labelledby="c-rule-mode-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-mode-title">运行方式</h3>
                    <span>时区固定为 Asia/Shanghai</span>
                  </div>
                  <label class="c-rule-switch">
                    <input type="checkbox" name="automatic_enabled" value="true"{automatic_checked}{disabled_attr}>
                    <span><b>自动巡检</b><small>关掉后规则仍保留，人工观察不受影响。</small></span>
                  </label>
                  <fieldset class="c-rule-options">
                    <legend>频率模式</legend>
                    {mode_options}
                  </fieldset>
                  {dynamic}
                </section>
                <section class="c-rule-section" aria-labelledby="c-rule-calendar-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-calendar-title">日历与时间窗</h3>
                    <span>首版不接受跨午夜窗口</span>
                  </div>
                  <div class="c-rule-checks">
                    {checkbox_weekdays}
                    {checkbox_weekends}
                    {checkbox_all_day}
                  </div>
                  <div class="c-rule-grid c-rule-window" data-monitor-rule-window>
                    {time_start}
                    {time_end}
                  </div>
                </section>
                <section class="c-rule-section" aria-labelledby="c-rule-cadence-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-cadence-title">间隔</h3>
                    <span>闭集范围 6 小时至 7 天</span>
                  </div>
                  <div class="c-rule-grid">
                    {fixed_interval}
                    {fallback_interval}
                  </div>
                </section>
              </div>
              <footer class="c-rule-actions">
                <div class="c-rule-secondary-actions">
                  <button class="c-btn-quiet" type="submit" name="command_kind" value="manual_observe"{disabled_attr}>人工观察一次</button>
                  {pause_or_resume}
                </div>
                <button class="c-btn-primary" type="submit" name="command_kind" value="save_rule"{disabled_attr}>保存新版本</button>
              </footer>
            </form>
          </section>
        </div>"#,
        kind = if panel.target_kind == "keyword" {
            "关键词"
        } else {
            "创作者"
        },
        revision = escape(&revision),
        name = escape(&panel.target_name),
        return_url = return_url,
        close_href = close_href,
        opener_id = escape(&opener_id),
        target_ref = panel.target_ref,
        expected_revision = form.expected_revision,
        idempotency_key = form.idempotency_key,
        surface_key = escape(&form.surface_key),
        ranking_key = escape(&form.ranking_key),
        task_contract_version = escape(&form.task_contract_version),
        return_filter = escape(return_filter),
        return_sort = escape(return_sort),
        feedback = feedback_markup(panel.receipt.as_ref()),
        dismissed = dismissed_markup(disabled),
        errors = errors_markup(error),
        automatic_checked = checked(form.automatic_enabled),
        disabled_attr = if disabled { " disabled" } else { "" },
        readonly = disabled,
        mode_options = mode_options(form, disabled),
        dynamic = dynamic_markup(&panel.dynamic_cadence, form),
        checkbox_weekdays = checkbox("run_on_weekdays", "工作日", form.run_on_weekdays, disabled),
        checkbox_weekends = checkbox("run_on_weekends", "周末", form.run_on_weekends, disabled),
        checkbox_all_day = checkbox("all_day", "全天", form.all_day, disabled),
        time_start = time_input(
            "window_start",
            "开始时间",
            &form.window_start,
            form.all_day || disabled
        ),
        time_end = time_input(
            "window_end",
            "结束时间",
            &form.window_end,
            form.all_day || disabled
        ),
        fixed_interval = interval_select(
            "fixed_interval_seconds",
            "固定间隔",
            &form.fixed_interval_seconds,
            form.mode != "fixed" || disabled,
        ),
        fallback_interval = interval_select(
            "fallback_interval_seconds",
            "动态不可用时兜底",
            &form.fallback_interval_seconds,
            disabled,
        ),
        pause_or_resume = pause_or_resume(panel, disabled),
    )
}

pub fn render_monitor_rule_unavailable(
    target_ref: Uuid,
    state: MonitorRuleUnavailableState,
    list_context: TargetListContext<'_>,
) -> String {
    let opener_id = format!("monitor-rule-{target_ref}");
    let return_url = list_context.list_href(None);
    let close_href = list_context.list_href(Some(&opener_id));
    let (title, body, code) = match state {
        MonitorRuleUnavailableState::TargetNotFound => (
            "没有找到这个观察目标",
            "这个本机标识没有对应目标；它不证明平台上的对象不存在，也不证明目标曾被删除。",
            "TARGET_NOT_FOUND",
        ),
        MonitorRuleUnavailableState::SchemaUnavailable => (
            "监控规则当前不可读",
            "规则表尚未可用。这次没有保存任何规则，也没有创建采集工作。",
            "RULE_SCHEMA_UNAVAILABLE",
        ),
        MonitorRuleUnavailableState::ReadUnavailable => (
            "监控规则读取失败",
            "目标列表仍可读取，但这个目标的规则状态当前未知。刷新会重新读取，不会自动重试或创建工作。",
            "RULE_READ_UNAVAILABLE",
        ),
    };
    format!(
        r#"<div class="c-rule-overlay" data-monitor-rule-overlay>
          <section id="c-monitor-rule" class="c-rule-modal c-rule-modal-compact" role="dialog"
                   aria-modal="true" aria-labelledby="c-rule-title" data-return-url="{return_url}"
                   data-return-focus="{opener_id}">
            <header class="c-rule-head">
              <div>
                <p class="c-rule-context">{code}</p>
                <h2 id="c-rule-title" tabindex="-1" data-monitor-rule-initial-focus>{title}</h2>
                <p>{body}</p>
              </div>
              <a class="c-btn-quiet c-rule-close" href="{close_href}" aria-label="关闭监控规则" data-monitor-rule-close>关闭</a>
            </header>
          </section>
        </div>"#,
        return_url = return_url,
        opener_id = escape(&opener_id),
        close_href = close_href,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRuleUnavailableState {
    TargetNotFound,
    SchemaUnavailable,
    ReadUnavailable,
}

fn mode_options(form: &MonitorRuleFormState, disabled: bool) -> String {
    [
        (
            "manual_only",
            "仅人工",
            "不进入自动调度；规则与历史仍保留。",
        ),
        ("fixed", "固定间隔", "按明确间隔进入调度候选。"),
        (
            "dynamic",
            "动态间隔",
            "仅在可比轮次合格时计算，否则使用兜底值。",
        ),
    ]
    .iter()
    .map(|(value, title, note)| {
        format!(
            r#"<label class="c-rule-option">
                 <input type="radio" name="mode" value="{value}"{checked}{disabled}>
                 <span><b>{title}</b><small>{note}</small></span>
               </label>"#,
            checked = checked(form.mode == *value),
            disabled = if disabled { " disabled" } else { "" },
        )
    })
    .collect()
}

fn dynamic_markup(dynamic: &DynamicCadenceView, form: &MonitorRuleFormState) -> String {
    let (state, body, interval) = match dynamic {
        DynamicCadenceView::Available {
            interval_seconds,
            qualified_rounds,
        } => (
            "动态间隔可用",
            format!("由 {qualified_rounds} 轮合格、可比观察计算。"),
            interval_label(*interval_seconds),
        ),
        DynamicCadenceView::Unavailable { .. } => (
            "动态间隔不可用",
            "当前没有可审计的合格可比轮次投影；不会用作者价值、语料内容或推测值代替。"
                .to_owned(),
            "DYNAMIC_UNAVAILABLE".to_owned(),
        ),
    };
    let hidden = if form.mode == "dynamic" {
        ""
    } else {
        " hidden"
    };
    let reason = match dynamic {
        DynamicCadenceView::Available { .. } => String::new(),
        DynamicCadenceView::Unavailable { reason_code } => format!(
            r#"<code data-control-reason>{}</code>"#,
            escape(reason_code),
        ),
    };
    format!(
        r#"<div class="c-rule-dynamic" data-monitor-dynamic{hidden}>
             <div><b>{state}</b><span>{interval}</span></div>
             <p>{body} 当前兜底值为 {fallback}。</p>{reason}
           </div>"#,
        fallback = escape(&interval_label(parse_interval(
            &form.fallback_interval_seconds
        ))),
    )
}

fn feedback_markup(receipt: Option<&MonitorRuleReceiptView>) -> String {
    let Some(receipt) = receipt else {
        return String::new();
    };
    let (tone, title) = match receipt.outcome.as_str() {
        "applied" => ("c-rule-feedback-ok", "命令已应用"),
        "replay" => ("c-rule-feedback-info", "已返回原命令结果"),
        "stale_revision" => ("c-rule-feedback-warn", "规则版本已经变化"),
        "identity_conflict" => ("c-rule-feedback-bad", "命令标识发生冲突"),
        _ => ("c-rule-feedback-bad", "命令未应用"),
    };
    let applied = receipt.applied_rule_revision_ref.map_or_else(
        || "未产生规则版本".to_owned(),
        |value| format!("规则 {}", short_ref(value)),
    );
    format!(
        r#"<section class="c-rule-feedback {tone}" data-monitor-rule-receipt data-outcome="{outcome}">
             <div><b>{title}</b><span data-monitor-rule-outcome>{reason}</span></div>
             <p>{meaning}</p>
             <dl>
               <div><dt>回执</dt><dd>{receipt_ref}</dd></div>
               <div><dt>记录时间</dt><dd>{recorded_at}</dd></div>
               <div><dt>结果</dt><dd>{applied}</dd></div>
             </dl>
           </section>"#,
        outcome = escape(&receipt.outcome),
        reason = escape(&receipt.reason_code),
        meaning = escape(receipt_meaning(&receipt.outcome, &receipt.reason_code)),
        receipt_ref = receipt.receipt_ref,
        recorded_at = escape(&receipt.recorded_at),
        applied = escape(&applied),
    )
}

fn receipt_meaning(outcome: &str, reason: &str) -> &'static str {
    match (outcome, reason) {
        ("applied", "rule_saved") => "规则已保存。没有据此声称采集已开始或完成。",
        ("applied", "monitor_paused") => "未来自动调度已暂停；人工观察仍可申请。",
        ("applied", "monitor_resumed") => "自动调度已恢复；尚未据此声称已经派出任务。",
        ("applied", "monitor_stopped") => "目标已停止新增工作，既有历史仍保留。",
        ("applied", "manual_observe_created") => "人工观察请求已建立；这不是执行或采集回执。",
        ("applied", "manual_observe_reused") => "已有工作覆盖这次人工观察，没有重复创建。",
        ("replay", _) => "相同命令已处理过；这里显示的是耐久重放结果。",
        ("stale_revision", _) => "页面基于旧版本提交，没有覆盖当前规则。请核对当前值后重试。",
        ("identity_conflict", _) => "同一命令标识携带了不同内容，没有覆盖原命令。",
        (_, "baseline_not_ready") => "创作者基线尚未满足自动巡检资格；关键词不使用这条基线。",
        (_, "invalid_interval") => "固定或兜底间隔不在 6 小时至 7 天的闭集范围内。",
        (_, "invalid_schedule") => "日历或时间窗不符合首版规则；跨午夜窗口不会静默改写。",
        (_, "target_not_requestable") => "这个目标当前不允许新增观察工作。",
        _ => "命令没有改变当前规则，也没有创建采集事实。",
    }
}

fn errors_markup(error: Option<&MonitorRuleFormError>) -> String {
    let Some(error) = error else {
        return String::new();
    };
    let items = error
        .fields
        .iter()
        .map(|field| {
            format!(
                r##"<li><a href="#{field}">{message}</a></li>"##,
                field = escape(field.field),
                message = escape(field.message),
            )
        })
        .collect::<String>();
    format!(
        r#"<div class="c-rule-errors" role="alert" data-monitor-rule-error data-error-code="{code}">
             <b>这次没有保存</b>
             <p>{summary}</p>
             <ul>{items}</ul>
           </div>"#,
        code = escape(error.code),
        summary = escape(error_summary(error.code)),
    )
}

fn error_summary(code: &str) -> &'static str {
    match code {
        "stale_revision" => "规则版本已变化；下面保留了你提交的值，没有覆盖当前版本。",
        "identity_conflict" => "命令标识冲突；原命令仍保留，下面的值尚未应用。",
        "baseline_not_ready" => "创作者基线尚未满足自动巡检资格。你可以先保存为仅人工。",
        "read_model_not_connected" => "本机规则读写当前不可用；没有写入规则或采集工作。",
        _ => "请修正标出的字段后再提交；当前规则没有改变。",
    }
}

fn dismissed_markup(disabled: bool) -> String {
    if !disabled {
        return String::new();
    }
    r#"<p class="c-rule-disabled" data-control-reason="target_not_requestable"><b>目标已停止</b>历史与规则仍可查看，但不能保存、恢复或发起新的观察。</p>"#.to_owned()
}

fn pause_or_resume(panel: &MonitorRulePanel, disabled: bool) -> String {
    let Some(rule) = panel.active_rule.as_ref() else {
        return String::new();
    };
    let (kind, label) = if rule.automatic_enabled {
        ("pause", "暂停未来自动调度")
    } else {
        ("resume", "恢复自动调度")
    };
    format!(
        r#"<button class="c-btn-quiet" type="submit" name="command_kind" value="{kind}"{disabled}>{label}</button>"#,
        disabled = if disabled { " disabled" } else { "" },
    )
}

fn checkbox(name: &str, label: &str, value: bool, disabled: bool) -> String {
    format!(
        r#"<label class="c-rule-check"><input type="checkbox" name="{name}" value="true"{checked}{disabled}><span>{label}</span></label>"#,
        checked = checked(value),
        disabled = if disabled { " disabled" } else { "" },
    )
}

fn time_input(name: &str, label: &str, value: &str, disabled: bool) -> String {
    format!(
        r#"<label for="{name}"><span>{label}</span><input id="{name}" type="time" name="{name}" value="{value}"{disabled}></label>"#,
        value = escape(value),
        disabled = if disabled { " disabled" } else { "" },
    )
}

fn interval_select(name: &str, label: &str, selected: &str, disabled: bool) -> String {
    let options = [
        (21_600, "6 小时"),
        (43_200, "12 小时"),
        (86_400, "24 小时"),
        (172_800, "2 天"),
        (604_800, "7 天"),
    ]
    .iter()
    .map(|(value, text)| {
        format!(
            r#"<option value="{value}"{selected}>{text}</option>"#,
            selected = if selected == value.to_string() {
                " selected"
            } else {
                ""
            },
        )
    })
    .collect::<String>();
    format!(
        r#"<label for="{name}"><span>{label}</span><select id="{name}" name="{name}"{disabled}>{options}</select></label>"#,
        disabled = if disabled { " disabled" } else { "" },
    )
}

fn checked(value: bool) -> &'static str {
    if value { " checked" } else { "" }
}

fn minute_as_time(value: Option<i16>) -> String {
    value.map_or_else(String::new, |value| {
        format!("{:02}:{:02}", value / 60, value % 60)
    })
}

fn parse_interval(value: &str) -> i32 {
    value.parse().unwrap_or(DEFAULT_INTERVAL_SECONDS)
}

fn interval_label(seconds: i32) -> String {
    match seconds {
        21_600 => "6 小时".to_owned(),
        43_200 => "12 小时".to_owned(),
        86_400 => "24 小时".to_owned(),
        172_800 => "2 天".to_owned(),
        604_800 => "7 天".to_owned(),
        value if value % 86_400 == 0 => format!("{} 天", value / 86_400),
        value => format!("{} 小时", value / 3_600),
    }
}

fn short_ref(value: Uuid) -> String {
    value.to_string().chars().take(8).collect()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> MonitorRulePanel {
        MonitorRulePanel {
            target_ref: Uuid::parse_str("d87b17da-f93c-42de-8051-8b21b2c1e90d").unwrap(),
            target_name: "边界测试 <创作者>".to_owned(),
            target_kind: "creator".to_owned(),
            lifecycle_state: "archived".to_owned(),
            active_rule: Some(ActiveMonitorRule {
                rule_revision_ref: Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                revision: 3,
                mode: MonitorRuleMode::Dynamic,
                automatic_enabled: true,
                run_on_weekdays: true,
                run_on_weekends: true,
                all_day: false,
                window_start_minute: Some(480),
                window_end_minute: Some(1320),
                fixed_interval_seconds: None,
                fallback_interval_seconds: 86_400,
                surface_key: "creator_profile".to_owned(),
                ranking_key: None,
                task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
                created_at: "2026-09-04 09:00:00+08".to_owned(),
            }),
            receipt: Some(MonitorRuleReceiptView {
                receipt_ref: Uuid::parse_str("92dbfd51-9678-4781-a489-c00805576852").unwrap(),
                command_kind: "save_rule".to_owned(),
                expected_revision: 2,
                outcome: "applied".to_owned(),
                reason_code: "rule_saved".to_owned(),
                applied_rule_revision_ref: Some(
                    Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                ),
                recorded_at: "2026-09-04 09:00:00+08".to_owned(),
            }),
            dynamic_cadence: DynamicCadenceView::default(),
        }
    }

    #[test]
    fn modal_is_a_sibling_form_with_stable_contract_selectors() {
        let panel = panel();
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = render_monitor_rule_modal(
            &panel,
            &form,
            None,
            TargetListContext {
                filter: Some("creator"),
                sort: Some("last"),
            },
        );
        for marker in [
            "data-monitor-rule-overlay",
            "data-monitor-rule-form",
            "data-monitor-rule-receipt",
            "data-monitor-rule-outcome",
            "data-monitor-dynamic",
            "data-control-reason",
            "name=\"expected_revision\"",
            "name=\"idempotency_key\"",
            "name=\"return_filter\" value=\"creator\"",
            "name=\"return_sort\" value=\"last\"",
            "DYNAMIC_UNAVAILABLE",
            "当前兜底值为 24 小时",
        ] {
            assert!(html.contains(marker), "missing {marker}");
        }
        assert!(html.contains("边界测试 &lt;创作者&gt;"));
        assert!(!html.contains("采集成功"));
        assert!(!html.contains("Cookie"));
        assert!(!html.contains("account identity"));
    }

    #[test]
    fn rejected_form_retains_values_but_rotates_command_identity() {
        let panel = panel();
        let original = MonitorRuleFormState::from_panel(&panel);
        let original_key = original.idempotency_key;
        let mut retained = original.with_new_command_identity();
        retained.mode = "dynamic".to_owned();
        retained.window_start = "22:00".to_owned();
        retained.window_end = "06:00".to_owned();
        let html = render_monitor_rule_modal(
            &panel,
            &retained,
            Some(&MonitorRuleFormError {
                code: "invalid_schedule",
                fields: vec![MonitorRuleFieldError {
                    field: "window_end",
                    message: "结束时间必须晚于开始时间；首版不接受跨午夜。",
                }],
            }),
            TargetListContext::default(),
        );
        assert_ne!(original_key, retained.idempotency_key);
        assert!(html.contains("value=\"22:00\""));
        assert!(html.contains("value=\"06:00\""));
        assert!(html.contains("data-error-code=\"invalid_schedule\""));
        assert!(html.contains("href=\"#window_end\""));
    }

    #[test]
    fn attach_places_modal_before_the_behavior_script_and_after_the_list_form() {
        let document = format!("<form id=\"list\"></form>{COLLECTION_SCRIPT}</body></html>");
        let attached = attach_to_collection_document(&document, "<aside>modal</aside>");
        let form_end = attached.find("</form>").unwrap();
        let modal = attached.find("<aside>modal</aside>").unwrap();
        let script = attached.find(COLLECTION_SCRIPT).unwrap();
        assert!(form_end < modal && modal < script);
    }

    #[test]
    fn unavailable_modal_keeps_unknown_distinct_from_empty_or_failure() {
        let html = render_monitor_rule_unavailable(
            Uuid::new_v4(),
            MonitorRuleUnavailableState::ReadUnavailable,
            TargetListContext::default(),
        );
        assert!(html.contains("规则状态当前未知"));
        assert!(!html.contains("没有监控规则"));
        assert!(!html.contains("采集失败"));
    }
}
