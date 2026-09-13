// Target-scoped monitoring-rule read and render surface.
//
// This module owns no scheduling or admission decision. It reads the append-only active rule
// and durable command receipt written by `linggan_evidence`, then renders one server-owned
// modal outside the target list's batch form. A saved rule is never described as collection
// success; only a later Work/Lease/Task/Receipt chain may make that claim.
use super::super::target_drawer::TargetListContext;
use linggan_contracts::PRODUCER_TASK_SPEC_VERSION;
use linggan_evidence::collection_control_schema_is_ready;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

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
    /// 目标的身份键。关键词是 `{词}::{排序}`——新建规则时的排序默认值从它来，
    /// 否则一个身份写着「最多点赞」的目标会默认配上综合排序，界面与身份各说各话。
    pub identity_key: String,
    pub lifecycle_state: String,
    pub active_rule: Option<ActiveMonitorRule>,
    pub receipt: Option<MonitorRuleReceiptView>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMonitorRule {
    pub rule_revision_ref: Uuid,
    pub revision: i32,
    pub automatic_enabled: bool,
    pub fixed_interval_seconds: Option<i32>,
    pub fallback_interval_seconds: i32,
    pub surface_key: String,
    pub ranking_key: Option<String>,
    /// 采样口径。只有关键词搜索面才有；创作者主页恒为 `None`。
    pub scroll_rounds: Option<i32>,
    pub top_by_likes: Option<i32>,
    pub published_within_days: Option<i32>,
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

/// Raw strings are retained only in the in-memory failed form. The route handler is
/// responsible for parsing the closed set before it calls the evidence command owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFormState {
    pub expected_revision: i32,
    pub idempotency_key: Uuid,
    pub automatic_enabled: bool,
    pub fixed_interval_seconds: String,
    pub surface_key: String,
    pub ranking_key: String,
    /// 采样口径，保持原始输入。解析与范围判定交给 handler，与间隔同一处置——
    /// 失败时把人填过的字面值原样送回表单，不要悄悄换成一个能通过校验的数。
    pub scroll_rounds: String,
    pub top_by_likes: String,
    pub published_within_days: String,
    pub task_contract_version: String,
}

impl MonitorRuleFormState {
    pub fn from_panel(panel: &MonitorRulePanel) -> Self {
        let default_surface = if panel.target_kind == "keyword" {
            "keyword_search"
        } else {
            "creator_profile"
        };
        // 默认排序从目标身份里取，而不是一律给综合排序：身份写着「最多点赞」的目标
        // 默认配上综合排序，等于界面与身份各说各话。身份里读不出来才回落——回落到
        // 综合排序是既有行为，不在这一步改它（规格说跨行业不采综合，那要在建目标时解决）。
        let default_ranking = if panel.target_kind == "keyword" {
            panel
                .identity_key
                .rsplit_once("::")
                .map_or("comprehensive", |(_, ranking)| ranking)
        } else {
            ""
        };
        // 关键词面的默认口径就是规格里那套统一采集动作：下拉 3 次、取点赞前 20。
        // 发布时间默认不限——写死一周会让本领域那条一直在采全部时间的关键词说假话。
        let (default_scroll, default_top) = if panel.target_kind == "keyword" {
            ("3", "20")
        } else {
            ("", "")
        };
        let Some(rule) = panel.active_rule.as_ref() else {
            return Self {
                expected_revision: 0,
                idempotency_key: Uuid::new_v4(),
                automatic_enabled: false,
                fixed_interval_seconds: DEFAULT_INTERVAL_SECONDS.to_string(),
                surface_key: default_surface.to_owned(),
                ranking_key: default_ranking.to_owned(),
                scroll_rounds: default_scroll.to_owned(),
                top_by_likes: default_top.to_owned(),
                published_within_days: String::new(),
                task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
            };
        };
        Self {
            expected_revision: rule.revision,
            idempotency_key: Uuid::new_v4(),
            // The current surface has one schedule meaning: an all-day fixed
            // interval.  A historical dynamic/window revision remains readable
            // in the immutable rule table, but opening it must not silently
            // retain a second scheduling language through hidden inputs.
            automatic_enabled: rule.automatic_enabled,
            fixed_interval_seconds: rule
                .fixed_interval_seconds
                .unwrap_or(rule.fallback_interval_seconds)
                .to_string(),
            surface_key: rule.surface_key.clone(),
            ranking_key: rule.ranking_key.clone().unwrap_or_default(),
            // 空表示「没记录过口径」，不是 0。既有规则由 0046 补过，新规则从上面的默认来。
            scroll_rounds: rule.scroll_rounds.map(|v| v.to_string()).unwrap_or_default(),
            top_by_likes: rule.top_by_likes.map(|v| v.to_string()).unwrap_or_default(),
            published_within_days: rule
                .published_within_days
                .map(|v| v.to_string())
                .unwrap_or_default(),
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
        // 预填用最早建的那条在用规则。一个目标可以有几条口径（`0076`），面板本身一次只
        // 编辑一条——存的时候按排序决定落到哪条口径上，选一个新排序就是新开一条。
        // 单规则目标（博主永远如此）与从前完全一样。检查器的规则台负责把几条都列出来。
        "SELECT target.target_ref,target.target_kind,target.identity_key, \
                COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key) AS target_name, \
                target.lifecycle_state, \
                (SELECT rule.active_revision_ref FROM collection_monitor_rule rule \
                  WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
                  ORDER BY rule.created_at,rule.rule_ref LIMIT 1) AS active_monitor_rule_revision_ref \
         FROM collection_observation_target target WHERE target.target_ref=$1",
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
        identity_key: target.try_get("identity_key")?,
        target_kind: target.try_get("target_kind")?,
        lifecycle_state: target.try_get("lifecycle_state")?,
        active_rule,
        receipt,
    }))
}

async fn read_active_rule(
    database: &Database,
    target_ref: Uuid,
    rule_ref: Uuid,
) -> Result<Option<ActiveMonitorRule>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT rule_revision_ref,revision,automatic_enabled,fixed_interval_seconds, \
                fallback_interval_seconds,surface_key,ranking_key, \
                scroll_rounds,top_by_likes,published_within_days, \
                task_contract_version,created_at::text AS created_at \
         FROM collection_monitor_rule_revision \
         WHERE target_ref=$1 AND rule_revision_ref=$2",
    )
    .bind(target_ref)
    .bind(rule_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(ActiveMonitorRule {
            rule_revision_ref: row.try_get("rule_revision_ref")?,
            revision: row.try_get("revision")?,
            automatic_enabled: row.try_get("automatic_enabled")?,
            fixed_interval_seconds: row.try_get("fixed_interval_seconds")?,
            fallback_interval_seconds: row.try_get("fallback_interval_seconds")?,
            surface_key: row.try_get("surface_key")?,
            ranking_key: row.try_get("ranking_key")?,
            scroll_rounds: row.try_get("scroll_rounds")?,
            top_by_likes: row.try_get("top_by_likes")?,
            published_within_days: row.try_get("published_within_days")?,
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

#[cfg(test)]
const COLLECTION_SCRIPT: &str = r#"<script src="/assets/collection-workspace.js"></script>"#;

#[cfg(test)]
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
              {ranking_hidden}
              <input type="hidden" name="task_contract_version" value="{task_contract_version}">
              <input type="hidden" name="return_filter" value="{return_filter}">
              <input type="hidden" name="return_sort" value="{return_sort}">
              <div class="c-rule-body">
                {errors}
                <section class="c-rule-section" aria-labelledby="c-rule-cadence-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-cadence-title">自动观察</h3>
                    <span>Asia/Shanghai · 从启用时刻开始计时</span>
                  </div>
                  <label class="c-rule-switch">
                    <input type="checkbox" name="automatic_enabled" value="true"{automatic_checked}{disabled_attr}>
                    <span><b>按规则自动观察</b><small>关闭后不再产生未来定时工单；人工观察仍可申请。</small></span>
                  </label>
                  <div class="c-rule-grid">
                    {interval}
                  </div>
                  <p class="c-rule-hint">{cadence_hint}</p>
                </section>
                {sampling}
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
        task_contract_version = escape(&form.task_contract_version),
        return_filter = escape(return_filter),
        return_sort = escape(return_sort),
        feedback = feedback_markup(panel.receipt.as_ref()),
        dismissed = dismissed_markup(disabled),
        errors = errors_markup(error),
        automatic_checked = checked(form.automatic_enabled),
        disabled_attr = if disabled { " disabled" } else { "" },
        readonly = disabled,
        interval = interval_select(
            "fixed_interval_seconds",
            "观察间隔（仅可选择下列 5 档）",
            &form.fixed_interval_seconds,
            disabled,
        ),
        pause_or_resume = pause_or_resume(panel, disabled),
        sampling = sampling_policy_section(form, disabled),
        // 关键词的排序移进了采样口径那一节，不再藏在 hidden 里；创作者没有那一节，
        // 它的 ranking 仍按原样透传（恒为空）。少一个隐藏输入就少一处能和界面说法不一致的地方。
        // 排序对两种目标都由 hidden 传回：关键词的它只读展示在采样口径里，创作者的它
        // 恒为空。表单不带这一项，保存就会把已有的排序清掉。
        ranking_hidden = format!(
            r#"<input type="hidden" name="ranking_key" value="{}">"#,
            escape(&form.ranking_key)
        ),
        cadence_hint = if form.surface_key.trim() == "keyword_search" {
            "每次运行按下面的采样口径搜一轮。关键词观察不做作者资格核验——那是创作者档案的事。"
        } else {
            "每次运行固定执行：作者主页资格核验 + 最近 30 条作品观察。历史建档完整度不会阻断后续观察。"
        },
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
        ("applied", "manual_observe_created") => {
            "人工观察工单已入队，尚未被插件认领；这不是执行或采集回执。"
        }
        ("applied", "manual_observe_reused") => "已有工作覆盖这次人工观察，没有重复创建。",
        ("replay", _) => "相同命令已处理过；这里显示的是耐久重放结果。",
        ("stale_revision", _) => "页面基于旧版本提交，没有覆盖当前规则。请核对当前值后重试。",
        ("identity_conflict", _) => "同一命令标识携带了不同内容，没有覆盖原命令。",
        (_, "baseline_not_ready") => "这是旧版本记录的历史原因；当前规则不再以建档完整度作为自动观察门槛。",
        (_, "invalid_interval") => "观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。",
        (_, "invalid_schedule") => "当前版本只接受固定、全天的单一观察间隔。",
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
        "baseline_not_ready" => "这是旧版本记录的历史原因；当前规则不再以建档完整度作为自动观察门槛。",
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

/// 关键词观察的采样口径。
///
/// 这一节回答的是「这一轮的 20 篇是怎么来的」：按什么排序、翻了几次页、从中取几篇、
/// 只要多新的内容。没有它，采回来的数字无法复核——而复核不了的数字不该拿去比较。
///
/// 只对关键词目标渲染。创作者主页没有排序也没有「取前 N」可言，给它一组输入框，
/// 等于请人填一份不存在的事实。
fn sampling_policy_section(form: &MonitorRuleFormState, disabled: bool) -> String {
    if form.surface_key.trim() != "keyword_search" {
        return String::new();
    }
    let disabled_attr = if disabled { " disabled" } else { "" };
    // 平台只提供这四档发布时间。此前这里是个任意天数的输入框——填 3 天，平台只能给
    // 「一周内」，那是个做不到的承诺。**取值仍是天数**：存的是「我想要多新的内容」这个
    // 意图，天数是跨平台通用的表达；档位是各平台的执行细节，由插件按自己的能力兑现，
    // 并把实际生效的那一档回写进回执。
    const PUBLISH_WINDOWS: [(&str, &str); 4] =
        [("", "不限"), ("1", "一天内"), ("7", "一周内"), ("180", "半年内")];
    let current_window = form.published_within_days.trim();
    let mut publish_windows = PUBLISH_WINDOWS
        .iter()
        .map(|(value, text)| {
            format!(
                r#"<option value="{value}"{selected}>{text}</option>"#,
                selected = if current_window == *value {
                    " selected"
                } else {
                    ""
                },
            )
        })
        .collect::<String>();
    // 库里存着一个不在四档里的天数时，多给一个选项把它显示出来。
    // 否则浏览器会默认选中第一项「不限」，**打开规则再保存一次就把那个值悄悄改没了**——
    // 界面不该在人没动过某一项的情况下改掉它。
    if !current_window.is_empty()
        && !PUBLISH_WINDOWS
            .iter()
            .any(|(value, _)| *value == current_window)
    {
        publish_windows.push_str(&format!(
            r#"<option value="{value}" selected>保持当前 · {value} 天（非平台档位）</option>"#,
            value = escape(current_window),
        ));
    }
    // 排序**只读**：它是目标身份的一部分（`{词}::{排序}`，「同一个词的两种排序是两个
    // 观察面」）。做成可编辑会让规则里的排序与身份脱节——列表还叫「考研自习（最多点赞）」，
    // 实际却按别的排序采。要换排序就建一个新的观察目标，那本来就是另一个观察面。
    let ranking_label = [
        ("most_liked", "最多点赞"),
        ("most_collected", "最多收藏"),
        ("most_commented", "最多评论"),
        ("latest", "最新"),
        ("comprehensive", "综合排序"),
    ]
    .iter()
    .find(|(value, _)| *value == form.ranking_key.trim())
    .map_or_else(
        // 认不出的排序显示原文：那是一个机器标识而不是我们的描述性标签，
        // 把它藏起来会让人看不出这个目标到底在按什么采。
        || escape(form.ranking_key.trim()),
        |(_, text)| (*text).to_owned(),
    );
    format!(
        r#"<section class="c-rule-section" aria-labelledby="c-rule-sampling-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-sampling-title">采样口径</h3>
                    <span>决定每一轮怎么取样，随样本一起留痕</span>
                  </div>
                  <div class="c-rule-grid">
                    <label><span>排序依据</span><output>{ranking_label}</output></label>
                    <label for="scroll_rounds"><span>下拉刷新次数</span><input id="scroll_rounds" name="scroll_rounds" type="number" min="0" max="20" value="{scroll_rounds}"{disabled_attr}></label>
                    <label for="top_by_likes"><span>取点赞前几篇</span><input id="top_by_likes" name="top_by_likes" type="number" min="1" max="200" value="{top_by_likes}"{disabled_attr}></label>
                    <label for="published_within_days"><span>只要多新的内容</span><select id="published_within_days" name="published_within_days"{disabled_attr}>{publish_windows}</select></label>
                  </div>
                  <p class="c-rule-hint">排序属于这个观察目标的身份，不在这里改——同一个词的两种排序是两个观察面。要按别的排序采，建一个新的关键词目标。</p>
                  <p class="c-rule-hint">按下拉次数控制，不按条数控制——页面每次加载出多少条不由我们决定，只有「拉了几次」是能说准的事实。时间范围只有这四档，因为平台就只给这四档；选了之后，回执里记的是页面上<b>实际生效</b>的那一档，不是这里选的值。同一批样本的点赞数不可跨时间比较。综合排序掺入个性化推荐，采回来的是平台认为这个账号会喜欢的内容，不是这个领域客观最好的内容。</p>
                </section>"#,
        scroll_rounds = escape(&form.scroll_rounds),
        ranking_label = ranking_label,
        top_by_likes = escape(&form.top_by_likes),
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
            identity_key: "creator-boundary".to_owned(),
            lifecycle_state: "archived".to_owned(),
            active_rule: Some(ActiveMonitorRule {
                rule_revision_ref: Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                revision: 3,
                automatic_enabled: true,
                fixed_interval_seconds: Some(86_400),
                fallback_interval_seconds: 86_400,
                surface_key: "creator_profile".to_owned(),
                ranking_key: None,
                scroll_rounds: None,
                top_by_likes: None,
                published_within_days: None,
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
                domain: None,
            },
        );
        for marker in [
            "data-monitor-rule-overlay",
            "data-monitor-rule-form",
            "data-monitor-rule-receipt",
            "data-monitor-rule-outcome",
            "name=\"expected_revision\"",
            "name=\"idempotency_key\"",
            "name=\"return_filter\" value=\"creator\"",
            "name=\"return_sort\" value=\"last\"",
            "观察间隔（仅可选择下列 5 档）",
            "最近 30 条作品观察",
        ] {
            assert!(html.contains(marker), "missing {marker}");
        }
        assert!(html.contains("边界测试 &lt;创作者&gt;"));
        assert!(!html.contains("动态间隔"));
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
        retained.fixed_interval_seconds = "1".to_owned();
        let html = render_monitor_rule_modal(
            &panel,
            &retained,
            Some(&MonitorRuleFormError {
                code: "invalid_interval",
                fields: vec![MonitorRuleFieldError {
                    field: "fixed_interval_seconds",
                    message: "观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。",
                }],
            }),
            TargetListContext::default(),
        );
        assert_ne!(original_key, retained.idempotency_key);
        assert!(html.contains("data-error-code=\"invalid_interval\""));
        assert!(html.contains("href=\"#fixed_interval_seconds\""));
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

#[cfg(test)]
mod sampling_policy_tests {
    use super::*;

    fn keyword_panel(identity_key: &str) -> MonitorRulePanel {
        MonitorRulePanel {
            target_ref: Uuid::nil(),
            target_name: "考研自习".to_owned(),
            target_kind: "keyword".to_owned(),
            identity_key: identity_key.to_owned(),
            lifecycle_state: "paused".to_owned(),
            active_rule: None,
            receipt: None,
        }
    }

    /// 身份写着「最多点赞」的目标，默认不该配上综合排序——那会让界面与身份各说各话。
    #[test]
    fn a_new_rule_defaults_to_the_ranking_in_the_target_identity() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::most_liked"));
        assert_eq!(form.ranking_key, "most_liked");
        // 规格里那套统一采集动作就是关键词面的默认口径。
        assert_eq!(form.scroll_rounds, "3");
        assert_eq!(form.top_by_likes, "20");
        // 发布时间默认不限：写死一周会让一直在采全部时间的关键词说假话。
        assert_eq!(form.published_within_days, "");
    }

    /// 创作者主页没有排序也没有取样口径可言，一项都不该预填。
    #[test]
    fn a_creator_rule_carries_no_sampling_policy() {
        let mut panel = keyword_panel("creator-1");
        panel.target_kind = "creator".to_owned();
        let form = MonitorRuleFormState::from_panel(&panel);
        assert_eq!(form.ranking_key, "");
        assert_eq!(form.scroll_rounds, "");
        assert_eq!(form.top_by_likes, "");
        assert!(sampling_policy_section(&form, false).is_empty());
    }

    /// 时间范围只给平台真正支持的四档：此前是个任意天数输入框，填 3 天平台只能给
    /// 「一周内」，那是个做不到的承诺。
    #[test]
    fn the_publish_window_offers_only_what_the_platform_supports() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::latest"));
        let html = sampling_policy_section(&form, false);
        for (value, text) in [("", "不限"), ("1", "一天内"), ("7", "一周内"), ("180", "半年内")] {
            assert!(html.contains(&format!(r#"<option value="{value}""#)));
            assert!(html.contains(text));
        }
        assert!(!html.contains(r#"name="published_within_days" type="number""#));
    }

    /// 库里存着非四档的天数时要显示出来。浏览器会默认选中第一项「不限」，
    /// 打开规则再保存一次就把那个值悄悄改没了——界面不该改掉人没动过的东西。
    #[test]
    fn a_non_standard_window_is_shown_instead_of_being_silently_dropped() {
        let mut form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::latest"));
        form.published_within_days = "3".to_owned();
        let html = sampling_policy_section(&form, false);
        assert!(html.contains(r#"<option value="3" selected>"#));
        assert!(html.contains("非平台档位"));
        // 「不限」不该同时被选中。
        assert!(!html.contains(r#"<option value="" selected>"#));
    }

    /// 关键词面渲染出那一节：三项可编辑，排序只读。
    #[test]
    fn the_keyword_surface_renders_every_sampling_input() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::latest"));
        let html = sampling_policy_section(&form, false);
        for field in ["scroll_rounds", "top_by_likes", "published_within_days"] {
            assert!(html.contains(&format!(r#"name="{field}""#)), "{field} 缺失");
        }
        // 排序只读展示，中文。
        assert!(html.contains("<output>最新</output>"));
    }

    /// 排序不可在规则里改：它是目标身份的一部分（`{词}::{排序}`）。做成可编辑会让规则
    /// 与身份脱节——列表还叫「考研自习（最多点赞）」，实际却按别的排序采。
    #[test]
    fn the_ranking_is_read_only_because_it_belongs_to_the_target_identity() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::most_liked"));
        let html = sampling_policy_section(&form, false);
        assert!(!html.contains(r#"<select id="ranking_key""#));
        assert!(html.contains("<output>最多点赞</output>"));
        assert!(html.contains("排序属于这个观察目标的身份"));
    }
}
