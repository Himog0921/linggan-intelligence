//! The Tasks page's one generic, target-scoped single-collection entry.
//!
//! GET only reads existing targets, the current rule and a selected durable command receipt.
//! POST remains the existing target monitoring command; this view cannot issue a Task or Lease.

use linggan_evidence::{
    ManualObservationReceipt, ObservationTarget, list_targets,
    read_manual_observation_primary_domains, read_manual_observation_receipt,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use uuid::Uuid;

use super::collection::collection_control_rule_view::{
    MonitorRulePanel, MonitorRulePanelRead, MonitorRuleSelection, read_monitor_rule_panel,
};
use super::target_drawer::{KeywordArchiveRead, percent_encode_component};

const OPENER_ID: &str = "monitor-rule-single-collection";
const BODY_OPEN: &str = "<div class=\"c-body\">";

#[derive(Debug, Default, Deserialize)]
pub(super) struct TaskActionParams {
    pub domain: Option<String>,
    pub once: Option<String>,
    pub target: Option<String>,
    pub receipt: Option<String>,
    pub error: Option<String>,
}

impl TaskActionParams {
    pub fn nav(&self) -> Option<&str> {
        self.domain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

/// The caller permits this return scope only for the existing manual_observe command.
pub(super) fn return_path(
    target_ref: Uuid,
    domain: Option<&str>,
    receipt_ref: Option<Uuid>,
    error: Option<&str>,
) -> String {
    let mut path = action_path(domain);
    path.push_str(&format!("&target={target_ref}"));
    if let Some(receipt_ref) = receipt_ref {
        path.push_str(&format!("&receipt={receipt_ref}"));
    }
    if let Some(error) = error {
        path.push_str(&format!("&error={}", percent_encode_component(error)));
    }
    path
}

pub(super) async fn decorate(
    database: Option<&Database>,
    base: &str,
    params: &TaskActionParams,
) -> String {
    let overlay = if params.once.as_deref() == Some("1") {
        let content = match database {
            Some(database) => read_content(database, params).await,
            None => unavailable_content(params, "当前读不到观察目标与规则，请恢复本机数据库连接后重试。"),
        };
        modal(params, &content)
    } else {
        String::new()
    };
    decorate_document(base, params.nav(), &overlay)
}

fn decorate_document(base: &str, domain: Option<&str>, overlay: &str) -> String {
    let toolbar = format!(
        r#"<div class="c-toolbar" data-task-actions><div></div><div class="c-actions"><a id="{OPENER_ID}" class="c-btn-primary" href="{href}">＋ 单次采集</a></div></div>"#,
        href = escape(&action_path(domain)),
    );
    let document = base.replacen(BODY_OPEN, &format!("{toolbar}{BODY_OPEN}"), 1);
    super::target_drawer::attach_to_collection_document(&document, overlay)
}

fn page_path(domain: Option<&str>) -> String {
    domain.map_or_else(
        || "/collection/tasks".to_owned(),
        |domain| {
            format!(
                "/collection/tasks?domain={}",
                percent_encode_component(domain)
            )
        },
    )
}

fn action_path(domain: Option<&str>) -> String {
    let path = page_path(domain);
    let separator = if domain.is_some() { '&' } else { '?' };
    format!("{path}{separator}once=1")
}

fn modal(params: &TaskActionParams, content: &str) -> String {
    let return_url = page_path(params.nav());
    format!(
        r#"<div class="c-rule-overlay" data-monitor-rule-overlay>
          <section id="c-monitor-rule" class="c-rule-modal c-rule-modal-compact" role="dialog" aria-modal="true" aria-labelledby="c-rule-title" data-return-url="{return_url}" data-return-focus="{OPENER_ID}">
            <header class="c-rule-head"><div><h2 id="c-rule-title" tabindex="-1" data-monitor-rule-initial-focus>单次采集</h2><p>对已有观察目标人工观察一次。提交后由可用工位领取，自动巡查设置保持原样。</p></div><a class="c-btn-quiet c-rule-close" href="{close_url}" aria-label="关闭单次采集" data-monitor-rule-close>关闭</a></header>
            <div class="c-rule-body">{content}</div>
          </section>
        </div>"#,
        return_url = escape(&return_url),
        close_url = escape(&format!("{return_url}#{OPENER_ID}")),
    )
}

async fn read_content(database: &Database, params: &TaskActionParams) -> String {
    let targets = match list_targets(database, None, None, 200).await {
        Ok(targets) => targets,
        Err(_) => return unavailable_content(params, "观察目标列表本次读取失败；这不表示没有目标。请稍后重试。"),
    };
    let selected = match params.target.as_deref() {
        None => None,
        Some(value) => match Uuid::parse_str(value) {
            Ok(target_ref) => Some(target_ref),
            Err(_) => {
                return format!(
                    "{}{}",
                    selection_form(&targets, None, params),
                    notice("目标引用无法识别，请重新选择。")
                );
            }
        },
    };
    let panel = match selected {
        Some(target_ref) => match read_monitor_rule_panel(
            database,
            target_ref,
            MonitorRuleSelection::CurrentRule,
            None,
        )
        .await
        {
            Ok(MonitorRulePanelRead::Found(panel)) => Some(panel),
            Ok(MonitorRulePanelRead::TargetNotFound) => {
                return format!(
                    "{}{}",
                    selection_form(&targets, None, params),
                    notice("这个观察目标已无法找到，请重新选择。")
                );
            }
            Ok(MonitorRulePanelRead::SchemaUnavailable) | Err(_) => {
                return format!(
                    "{}{}",
                    selection_form(&targets, None, params),
                    notice("所选目标的规则当前读不到，暂不能提交。")
                );
            }
        },
        None => None,
    };
    let mut content = selection_form(&targets, panel.as_ref(), params);
    content.push_str(&form_feedback(params));
    if let Some(panel) = panel {
        content.push_str(
            &receipt_content(database, panel.target_ref, params.receipt.as_deref()).await,
        );
        let domains = read_manual_observation_primary_domains(database, panel.target_ref).await;
        let archive = if panel.target_kind == "keyword" {
            super::read_keyword_archive(database, panel.target_ref).await
        } else {
            KeywordArchiveRead::Complete
        };
        content.push_str(&command_form(
            &panel,
            domains.as_deref().ok(),
            archive,
            params,
        ));
    }
    content
}

fn selection_form(
    targets: &[ObservationTarget],
    panel: Option<&MonitorRulePanel>,
    params: &TaskActionParams,
) -> String {
    let selected = panel
        .map(|panel| panel.target_ref)
        .or_else(|| params.target.as_deref().and_then(|value| Uuid::parse_str(value).ok()));
    let mut options = String::from(r#"<option value="">请选择观察目标</option>"#);
    for target in targets {
        options.push_str(&target_option(
            target.target_ref,
            target.display_name.as_deref().unwrap_or(&target.identity_key),
            &target.target_kind,
            selected,
        ));
    }
    // A selected target is read independently; the bounded chooser must not erase it.
    if let Some(panel) = panel.filter(|panel| {
        !targets.iter().any(|target| target.target_ref == panel.target_ref)
    }) {
        options.push_str(&target_option(
            panel.target_ref,
            &panel.target_name,
            &panel.target_kind,
            selected,
        ));
    } else if let Some(target_ref) = selected
        .filter(|target_ref| {
            panel.is_none() && !targets.iter().any(|target| target.target_ref == *target_ref)
        })
    {
        options.push_str(&format!(r#"<option value="{target_ref}" selected>所选目标 · {target_ref}（当前读不到）</option>"#));
    }
    let empty = if targets.is_empty() && panel.is_none() {
        "当前没有可选择的观察目标，请先在观察目标页添加。"
    } else {
        "选择器显示最近 200 个观察目标。"
    };
    format!(
        r#"<section class="c-rule-section"><form class="c-rule-form" method="get" action="/collection/tasks"><input type="hidden" name="once" value="1">{domain}<div class="c-rule-grid"><label for="single-collection-target">观察目标<select id="single-collection-target" name="target" required>{options}</select></label><div class="c-rule-secondary-actions"><button class="c-btn-secondary" type="submit">查看采集范围</button></div></div><p class="c-rule-hint">{empty}</p></form></section>"#,
        domain = domain_field("domain", params.nav()),
    )
}

fn target_option(target_ref: Uuid, name: &str, kind: &str, selected: Option<Uuid>) -> String {
    format!(
        r#"<option value="{target_ref}"{selected}>{name} · {kind}</option>"#,
        selected = if selected == Some(target_ref) {
            " selected"
        } else {
            ""
        },
        name = escape(name),
        kind = if kind == "creator" { "创作者" } else { "关键词" },
    )
}

fn command_gate(
    panel: &MonitorRulePanel,
    domains: Option<&[String]>,
    archive: KeywordArchiveRead,
) -> Option<&'static str> {
    if panel.lifecycle_state == "dismissed" {
        return Some("这个目标已停止新增工作，当前不能申请单次采集。");
    }
    match domains {
        None => return Some("当前读不到目标的领域用途，暂不能提交。"),
        Some([_]) => {}
        Some([]) => return Some("目标没有唯一有效的主用途领域，请先在领域管理中检查归属与暂停状态。"),
        Some(_) => return Some("目标关联了多个有效主用途领域；现有人工观察命令无法选择其中一个，请先明确用途。"),
    }
    if panel.target_kind == "keyword" {
        return match archive {
            KeywordArchiveRead::Complete => None,
            KeywordArchiveRead::Unavailable => Some("关键词建档状态当前读不到，暂不能提交。"),
            KeywordArchiveRead::NotArchived | KeywordArchiveRead::DetailPending => {
                Some("关键词需先完成搜索面建档和待补详情，再申请一次巡查。")
            }
        };
    }
    None
}

fn command_form(
    panel: &MonitorRulePanel,
    domains: Option<&[String]>,
    archive: KeywordArchiveRead,
    params: &TaskActionParams,
) -> String {
    let blocked = command_gate(panel, domains, archive);
    let domain = match domains {
        Some([domain]) => domain.as_str(),
        _ => "当前无法确定",
    };
    let revision = panel.active_rule.as_ref().map_or(0, |rule| rule.revision);
    let rule = rule_summary(panel);
    let feedback = blocked.map(notice).unwrap_or_default();
    format!(
        r#"<section class="c-rule-section" data-single-collection-scope><div class="c-rule-section-head"><h3>采集范围</h3><span>人工观察一次</span></div><div class="c-rule-grid"><label>当前主用途领域<input type="text" value="{domain}" readonly aria-readonly="true" title="{domain}"></label><label>使用的现有规则<input type="text" value="{rule}" readonly aria-readonly="true" title="{rule}"></label></div><p class="c-rule-hint">提交时重新核对当前用途与授权；已有在途观察会复用其工单。多条规则时使用最早建立的在用规则。</p>{feedback}<form class="c-rule-form" method="post" action="/collection/targets/rules" data-monitor-rule-form data-readonly="{readonly}"><input type="hidden" name="target_ref" value="{target_ref}"><input type="hidden" name="expected_revision" value="{revision}"><input type="hidden" name="idempotency_key" value="{identity}"><input type="hidden" name="return_scope" value="tasks">{return_domain}<footer class="c-rule-actions"><a class="c-btn-quiet" href="{target_href}">查看目标</a><button class="c-btn-primary" type="submit" name="command_kind" value="manual_observe"{disabled}>提交一次采集</button></footer></form></section>"#,
        domain = escape(domain),
        rule = escape(&rule),
        readonly = if blocked.is_some() { "true" } else { "false" },
        disabled = if blocked.is_some() { " disabled" } else { "" },
        target_ref = panel.target_ref,
        identity = Uuid::new_v4(),
        return_domain = domain_field("return_domain", params.nav()),
        target_href = escape(&format!(
            "/collection/targets?drawer={}{}",
            panel.target_ref,
            params.nav().map_or_else(String::new, |domain| format!(
                "&domain={}",
                percent_encode_component(domain)
            ))
        )),
    )
}

fn rule_summary(panel: &MonitorRulePanel) -> String {
    let Some(rule) = panel.active_rule.as_ref() else {
        return "尚无巡查规则".to_owned();
    };
    let ranking = rule
        .ranking_key
        .as_deref()
        .map(ranking_label)
        .unwrap_or("主页目录");
    let mut detail = vec![format!("{ranking} · 版本 {}", rule.revision)];
    if let Some(rounds) = rule.scroll_rounds {
        detail.push(format!("下拉 {rounds} 次"));
    }
    if let Some(top) = rule.top_by_likes {
        detail.push(format!("取赞前 {top}"));
    }
    if let Some(days) = rule.published_within_days {
        detail.push(format!("近 {days} 天"));
    }
    detail.join(" · ")
}

fn ranking_label(ranking: &str) -> &str {
    match ranking {
        "most_liked" => "最多点赞",
        "most_collected" => "最多收藏",
        "most_commented" => "最多评论",
        "latest" => "最新排序",
        "comprehensive" => "综合排序",
        _ => "当前规则口径",
    }
}

async fn receipt_content(database: &Database, target_ref: Uuid, receipt: Option<&str>) -> String {
    let Some(receipt) = receipt else {
        return String::new();
    };
    let Ok(receipt_ref) = Uuid::parse_str(receipt) else {
        return notice("回执引用无法识别，不能据此确认提交结果。");
    };
    match read_manual_observation_receipt(database, target_ref, receipt_ref).await {
        Ok(Some(receipt)) => receipt_markup(&receipt),
        Ok(None) => notice("没有读到这次人工观察的回执，不能据此确认提交结果。"),
        Err(_) => notice("提交回执本次读不到，请稍后刷新核对；不要据此重复提交。"),
    }
}

fn receipt_markup(receipt: &ManualObservationReceipt) -> String {
    let recorded_work = receipt.work_order_ref.is_some()
        && matches!(receipt.outcome.as_str(), "applied" | "replay")
        && matches!(
            receipt.reason.as_str(),
            "manual_observe_created" | "manual_observe_reused"
        );
    let (title, meaning) = if recorded_work {
        match (receipt.outcome.as_str(), receipt.reason.as_str()) {
            ("replay", _) => ("已返回原申请结果", "相同申请已处理过。以下是原有工单引用，不代表已经执行或采回材料。"),
            (_, "manual_observe_reused") => ("已有工单覆盖这次申请", "已关联现有工单，没有重复创建。执行与材料结果仍以任务回执为准。"),
            _ => ("人工观察工单已建立", "工单进入统一队列后由可用工位领取；这里只确认申请结果，尚不能证明已经执行。"),
        }
    } else {
        ("本次申请尚未形成可确认的工单", command_problem(&receipt.reason))
    };
    let work = receipt
        .work_order_ref
        .map_or_else(|| "未记录工单".to_owned(), |value| value.to_string());
    format!(
        r#"<section class="c-rule-feedback{tone}" data-single-collection-receipt data-outcome="{outcome}"><div><b>{title}</b></div><p>{meaning}</p><dl><div><dt>命令回执</dt><dd>{receipt_ref}</dd></div><div><dt>工单引用</dt><dd>{work}</dd></div><div><dt>记录时间</dt><dd>{recorded_at}</dd></div></dl><p>原因：<code>{reason}</code></p></section>"#,
        tone = if recorded_work { " c-rule-feedback-ok" } else { "" },
        outcome = escape(&receipt.outcome),
        receipt_ref = receipt.receipt_ref,
        work = escape(&work),
        recorded_at = escape(&receipt.recorded_at),
        reason = escape(&receipt.reason),
    )
}

fn command_problem(code: &str) -> &'static str {
    match code {
        "stale_revision" | "monitor_rule_command_stale" => "规则版本已经变化。已保留所选目标，请核对当前规则后重新提交。",
        "identity_conflict" => "命令标识对应了不同内容，未覆盖原申请。请核对后重新提交。",
        "baseline_not_ready" | "keyword_archive_incomplete" => "关键词建档尚未完成，请先完成搜索面和待补详情。",
        "target_domain_unassigned" => "目标当前没有唯一有效的主用途领域，请检查领域归属。",
        "target_not_requestable" => "这个目标当前不允许新增观察工作。",
        "authorization_missing" | "authorization_expired_or_revoked" => "当前没有适用的有效授权，未创建新的采集工单。",
        "authorization_scope_mismatch" | "authorization_purpose_mismatch" => "当前授权没有覆盖这次观察的范围或用途。",
        "authorization_target_limit_reached" | "authorization_work_unit_limit_reached" => "本次观察超出授权额度，未创建新的采集工单。",
        "station_unavailable" | "station_not_accepting" | "installation_credential_missing" | "plugin_version_unsupported" | "installation_stale" | "capability_missing" => "工位当前未具备接活条件，请在工位管理中核对。",
        "account_unbound" | "account_binding_changed" | "account_binding_expired" | "account_eligibility_stale" | "account_needs_login" | "account_restricted" | "account_unknown" => "当前账号身份或资格未通过准入，请在工位管理中核对账号。",
        "risk_paused" | "installation_risk_cooldown" | "account_cooling" => "当前处于风险暂停或冷却中，请核对工位状态后再申请。",
        "account_busy" | "station_busy" | "station_daily_budget_reached" | "platform_concurrency_reached" | "capacity_unknown" => "当前执行容量未通过准入，请在工位管理中核对可用状态。",
        "need_already_satisfied" | "in_flight_work_covers_it" => "已有结果或在途工作覆盖了这次申请，请核对已有任务。",
        "read_model_not_connected" | "database_unavailable" | "schema_unavailable" => "当前无法读取或提交采集命令，请恢复本机服务后核对。",
        _ => "本次申请没有可确认的新工单。请核对回执原因与目标、工位状态。",
    }
}

fn form_feedback(params: &TaskActionParams) -> String {
    // A durable receipt is stronger than a redirect hint. In particular, replay may have
    // succeeded; its query hint must not introduce a contradictory "no work" message.
    if params.receipt.is_some() {
        return String::new();
    }
    params.error.as_deref().map_or_else(String::new, |error| notice(command_problem(error)))
}

fn unavailable_content(params: &TaskActionParams, message: &str) -> String {
    let selected = params.target.as_deref().and_then(|value| Uuid::parse_str(value).ok());
    let retained = selected.map_or_else(String::new, |target_ref| {
        format!(r#"<p class="c-rule-hint">已保留目标引用：<code>{target_ref}</code></p>"#)
    });
    format!("{}{retained}", notice(message))
}

fn domain_field(name: &str, domain: Option<&str>) -> String {
    domain.map_or_else(String::new, |domain| {
        format!(
            r#"<input type="hidden" name="{name}" value="{}">"#,
            escape(domain)
        )
    })
}

fn notice(message: &str) -> String {
    format!(
        r#"<div class="c-rule-feedback" role="status"><p>{}</p></div>"#,
        escape(message)
    )
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
#[path = "collection_task_actions_tests.rs"]
mod tests;
