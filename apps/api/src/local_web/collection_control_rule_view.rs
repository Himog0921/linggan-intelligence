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
                automatic_enabled: false,
                fixed_interval_seconds: DEFAULT_INTERVAL_SECONDS.to_string(),
                surface_key: default_surface.to_owned(),
                ranking_key: default_ranking.to_owned(),
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
              <input type="hidden" name="ranking_key" value="{ranking_key}">
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
                  <p class="c-rule-hint">每次运行固定执行：作者主页资格核验 + 最近 30 条作品观察。历史建档完整度不会阻断后续观察。</p>
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
        interval = interval_select(
            "fixed_interval_seconds",
            "观察间隔（仅可选择下列 5 档）",
            &form.fixed_interval_seconds,
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
            lifecycle_state: "archived".to_owned(),
            active_rule: Some(ActiveMonitorRule {
                rule_revision_ref: Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                revision: 3,
                automatic_enabled: true,
                fixed_interval_seconds: Some(86_400),
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
