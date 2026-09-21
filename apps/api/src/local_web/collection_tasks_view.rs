//! The bounded Task / Attempt / Package / Receipt read surface.
//!
//! This module intentionally renders only persisted local facts.  It must not
//! turn a queue claim into a completed task, hide a recovered page-start
//! failure, or manufacture a target name when a task has no target projection.

use linggan_evidence::{
    CollectionTaskExecution, CollectionTaskTimeline, DeliveryConclusion,
    DetailDeliveryReconciliation,
};

use super::collection::{
    READOUT_SLOT_END, READOUT_SLOT_START, context_readout, replace_bounded_slot,
};

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

pub fn render_tasks(
    base: &str,
    timeline: &CollectionTaskTimeline,
    delivery: Option<&[DetailDeliveryReconciliation]>,
) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();

    let body = if timeline.tasks.is_empty() {
        r#"<section class="c-task-empty">
              <h2>当前读取范围没有采集任务</h2>
              <p>任务投影读取成功，最近 100 条范围返回零项；这不表示历史从未存在，也不会触发任何平台访问。</p>
              <p class="c-control-none">当前没有任务，因此没有可核对的冻结 Work 引用。</p>
            </section>"#
            .to_owned()
    } else {
        let selected = task_inspector(&timeline.tasks[0]);
        format!(
            r#"<section class="c-tasks c-tasks-v4">
                  <p class="c-task-boundary">按最近状态读取最多 100 条本机任务。队列资格、Attempt、Package 与 Receipt 分列显示；读取本身不会领取、重试或新建任务。</p>
                  <div class="c-task-workspace">
                    <div class="c-task-ledger">
                      <div class="c-task-ledger-head"><span>任务</span><span>目标 / 来源</span><span>阶段</span><span>状态</span><span>回执</span><span>尝试</span><span>创建时间</span></div>
                      <div class="c-task-list">{rows}</div>
                    </div>
                    {selected}
                  </div>
                </section>"#,
            rows = timeline
                .tasks
                .iter()
                .enumerate()
                .map(|(index, task)| task_row(task, index == 0))
                .collect::<String>(),
        )
    };
    let body = format!("{body}{}", delivery_section(delivery));
    let rendered = format!("{}{body}{}", &base[..open], &base[close..]);
    replace_bounded_slot(
        &rendered,
        READOUT_SLOT_START,
        READOUT_SLOT_END,
        &task_readout(timeline, delivery),
    )
}

fn task_row(task: &CollectionTaskExecution, selected: bool) -> String {
    let state = task_state(task);
    let target = task
        .target_display_name
        .as_deref()
        .or(task.target_identity_key.as_deref())
        .unwrap_or("目标关联当前未知");
    let task_id = task.task_id.to_string();
    let task_ref = short_ref(&task_id);
    let sequence = task
        .sequence_no
        .map(|number| format!(" · 步骤 {number}"))
        .unwrap_or_default();
    let attempt = task.attempt_id.as_ref().map_or_else(
        || "尚未形成 Attempt".to_owned(),
        |attempt_id| {
            format!(
                "Attempt {} · {}",
                short_ref(&attempt_id.to_string()),
                task.attempt_started_at.as_deref().unwrap_or("时间未知")
            )
        },
    );
    let package = task.package_ref.as_ref().map_or_else(
        || "尚未形成 Package".to_owned(),
        |package_ref| {
            format!(
                "{} · Package {} · {}",
                task.package_kind.as_deref().unwrap_or("类型未知"),
                short_ref(&package_ref.to_string()),
                task.package_accepted_at
                    .as_deref()
                    .unwrap_or("接纳时间未知")
            )
        },
    );
    let receipt = task.receipt_ref.as_ref().map_or_else(
        || "尚未收到 Receipt".to_owned(),
        |receipt_ref| {
            format!(
                "Receipt {} · {} · {}",
                short_ref(&receipt_ref.to_string()),
                material_admission_words(task.material_admission.as_deref()),
                task.receipt_received_at
                    .as_deref()
                    .unwrap_or("接收时间未知")
            )
        },
    );
    let previous_failure = task.last_dispatch_failure_code.as_ref().map_or_else(
        || "没有已记录的启动失败".to_owned(),
        |code| {
            let when = task
                .last_dispatch_failure_at
                .as_deref()
                .unwrap_or("时间未知");
            format!(
                "此前启动失败：{code} · {when}；{}。",
                if task.receipt_ref.is_some() {
                    "后续回执已保留"
                } else {
                    "当前尚未有 Receipt"
                },
            )
        },
    );
    let receipt_state = receipt_column(task);
    let (effect_text, effect_code) = execution_effect_reading(task);
    let attempt_short = task.attempt_id.as_ref().map_or_else(
        || "—".to_owned(),
        |value| short_ref(&value.to_string()).to_owned(),
    );
    let meta = format!(
        "{} · {} · {}{}",
        source_label(&task.source),
        task.platform,
        task.page_type,
        sequence
    );
    format!(
        r#"<button type="button" class="c-task-row{selected_class}" data-task-row data-task-id="{task_id}" data-task-title="{target}" data-task-ref="{task_ref}" data-task-meta="{meta}" data-task-state="{state}" data-task-state-class="{state_class}" data-task-state-note="{state_note}" data-task-attempt="{attempt}" data-task-package="{package}" data-task-receipt="{receipt}" data-task-effect="{effect_text}" data-task-effect-code="{effect_code}" data-task-failure="{previous_failure}" data-task-created="{created}" data-task-capabilities="{capabilities}" aria-pressed="{pressed}">
              <span class="c-task-id">#{task_ref}</span>
              <span class="c-task-target"><b>{target}</b><small>{source}</small></span>
              <span class="c-task-stage">{capabilities}</span>
              <span class="c-task-state {state_class}"><b>{state}</b><small>{state_note}</small></span>
              <span class="c-task-receipt">{receipt_state}</span>
              <span class="c-task-attempt">{attempt_short}</span>
              <time>{created}</time>
            </button>"#,
        selected_class = if selected { " is-selected" } else { "" },
        pressed = if selected { "true" } else { "false" },
        source = escape(source_label(&task.source)),
        meta = escape(&meta),
        capabilities = escape(&task.capabilities),
        target = escape(target),
        task_ref = escape(task_ref),
        task_id = task.task_id,
        state = state.label,
        state_class = state.class_name,
        state_note = escape(state.note),
        attempt = escape(&attempt),
        package = escape(&package),
        receipt = escape(&receipt),
        effect_text = escape(effect_text),
        effect_code = escape(effect_code),
        previous_failure = escape(&previous_failure),
        receipt_state = escape(receipt_state),
        attempt_short = escape(&attempt_short),
        created = escape(&task.created_at),
    )
}

fn task_inspector(task: &CollectionTaskExecution) -> String {
    let state = task_state(task);
    let target = task
        .target_display_name
        .as_deref()
        .or(task.target_identity_key.as_deref())
        .unwrap_or("目标关联当前未知");
    let task_id = task.task_id.to_string();
    let task_ref = short_ref(&task_id);
    let attempt = task.attempt_id.as_ref().map_or_else(
        || "尚未形成 Attempt".to_owned(),
        |attempt_id| {
            format!(
                "Attempt {} · {}",
                short_ref(&attempt_id.to_string()),
                task.attempt_started_at.as_deref().unwrap_or("时间未知")
            )
        },
    );
    let package = task.package_ref.as_ref().map_or_else(
        || "尚未形成 Package".to_owned(),
        |package_ref| {
            format!(
                "{} · Package {} · {}",
                task.package_kind.as_deref().unwrap_or("类型未知"),
                short_ref(&package_ref.to_string()),
                task.package_accepted_at
                    .as_deref()
                    .unwrap_or("接纳时间未知")
            )
        },
    );
    let receipt = task.receipt_ref.as_ref().map_or_else(
        || "尚未收到 Receipt".to_owned(),
        |receipt_ref| {
            format!(
                "Receipt {} · {} · {}",
                short_ref(&receipt_ref.to_string()),
                material_admission_words(task.material_admission.as_deref()),
                task.receipt_received_at
                    .as_deref()
                    .unwrap_or("接收时间未知")
            )
        },
    );
    let (effect_text, effect_code) = execution_effect_reading(task);
    let failure = task.last_dispatch_failure_code.as_ref().map_or_else(
        || "没有已记录的启动失败。".to_owned(),
        |code| {
            format!(
                "此前启动失败：{} · {}；{}。",
                code,
                task.last_dispatch_failure_at
                    .as_deref()
                    .unwrap_or("时间未知"),
                if task.receipt_ref.is_some() {
                    "后续回执已保留"
                } else {
                    "当前尚未有 Receipt"
                }
            )
        },
    );
    format!(
        r#"<aside class="c-task-inspector" data-task-inspector aria-live="polite">
             <div class="c-task-inspector-head"><span data-task-inspector-ref>任务 #{task_ref}</span><h2 data-task-inspector-title>{target}</h2><p data-task-inspector-meta>{source} · {platform} · {page_type}</p></div>
             <nav class="c-task-inspector-tabs" aria-label="任务详情" role="tablist"><button type="button" class="is-active" id="task-tab-overview" data-task-tab="overview" role="tab" aria-selected="true" aria-controls="task-panel-overview" tabindex="0">概览</button><button type="button" id="task-tab-attempt" data-task-tab="attempt" role="tab" aria-selected="false" aria-controls="task-panel-attempt" tabindex="-1">Attempt</button><button type="button" id="task-tab-package" data-task-tab="package" role="tab" aria-selected="false" aria-controls="task-panel-package" tabindex="-1">Package</button><button type="button" id="task-tab-receipt" data-task-tab="receipt" role="tab" aria-selected="false" aria-controls="task-panel-receipt" tabindex="-1">Receipt</button><button type="button" id="task-tab-frozen" data-task-tab="frozen" role="tab" aria-selected="false" aria-controls="task-panel-frozen" tabindex="-1">冻结资源</button></nav>
             <div class="c-task-inspector-body">
               <section class="c-task-panel is-active" id="task-panel-overview" data-task-panel="overview" role="tabpanel" aria-labelledby="task-tab-overview"><div class="c-state-line {state_class}" data-task-inspector-state-view><span><i></i><b data-task-inspector-state>{state}</b></span><span data-task-inspector-state-note>{state_note}</span></div><dl><div><dt>阶段</dt><dd data-task-inspector-capabilities>{capabilities}</dd></div><div><dt>创建时间</dt><dd data-task-inspector-created>{created}</dd></div><div><dt>历史</dt><dd data-task-inspector-failure>{failure}</dd></div></dl></section>
               <section class="c-task-panel" id="task-panel-attempt" data-task-panel="attempt" role="tabpanel" aria-labelledby="task-tab-attempt" hidden><h3>Attempt</h3><p data-task-inspector-attempt>{attempt}</p></section>
               <section class="c-task-panel" id="task-panel-package" data-task-panel="package" role="tabpanel" aria-labelledby="task-tab-package" hidden><h3>Package</h3><p data-task-inspector-package>{package}</p></section>
               <section class="c-task-panel" id="task-panel-receipt" data-task-panel="receipt" role="tabpanel" aria-labelledby="task-tab-receipt" hidden><h3>Receipt</h3><p data-task-inspector-receipt>{receipt}</p><p><span data-task-inspector-effect>{effect_text}</span><code data-task-inspector-effect-code>{effect_code}</code></p></section>
               <section class="c-task-panel" id="task-panel-frozen" data-task-panel="frozen" role="tabpanel" aria-labelledby="task-tab-frozen" hidden><div data-task-inspector-frozen><!-- frozen-work:start --><p class="c-control-none">该任务的冻结 Work 投影正在读取。</p><!-- frozen-work:end --></div></section>
             </div>
           </aside>"#,
        task_ref = escape(task_ref),
        target = escape(target),
        source = escape(source_label(&task.source)),
        platform = escape(&task.platform),
        page_type = escape(&task.page_type),
        state = state.label,
        state_class = state.class_name,
        state_note = escape(state.note),
        capabilities = escape(&task.capabilities),
        created = escape(&task.created_at),
        failure = escape(&failure),
        attempt = escape(&attempt),
        package = escape(&package),
        receipt = escape(&receipt),
        effect_text = escape(effect_text),
        effect_code = escape(effect_code),
    )
}

fn task_readout(
    timeline: &CollectionTaskTimeline,
    delivery: Option<&[DetailDeliveryReconciliation]>,
) -> String {
    let active = timeline.active_count.to_string();
    let expired = timeline.expired_lease_count.to_string();
    // 交付这两个读数只数结论，不数会话自己记的状态：有回执的 delivery_pending 会话
    // 已经算「已交付」，不再进「待交付」；读不到冻结通道的进「恢复待核实」。
    // 读不到投影时写 UNKNOWN，不写 0——「没读」不是「没有」。
    let (awaiting, unverified, scope) = match delivery {
        Some(sessions) => (
            sessions
                .iter()
                .filter(|session| session.conclusion == DeliveryConclusion::AwaitingDelivery)
                .count()
                .to_string(),
            sessions
                .iter()
                .filter(|session| session.conclusion == DeliveryConclusion::RecoveryUnverified)
                .count()
                .to_string(),
            "最近 100 个已消费导航的详情页会话",
        ),
        None => (
            "UNKNOWN".to_owned(),
            "UNKNOWN".to_owned(),
            "当前交付对账投影尚未读取",
        ),
    };
    context_readout(&[
        (&active, "待完成", "最近 100 条任务投影的待完成数"),
        (&expired, "租约失效", "最近 100 条任务投影的失效租约数"),
        (&awaiting, "待交付", scope),
        (&unverified, "恢复待核实", scope),
    ])
}

/// 「交付对账」小节：一个已授权详情页会话的包，到服务端了没有。
///
/// 它只读会话、冻结通道身份与回执三张现有事实，不接纳、不重投、不清理本地 outbox，
/// 也不重开页面。`None` 表示这次读取失败——那时要写「读不到」，不能写得像「没有待交付」。
fn delivery_section(delivery: Option<&[DetailDeliveryReconciliation]>) -> String {
    let rows = match delivery {
        None => r#"<p class="c-control-none">交付对账本次读取失败。这不表示没有待交付，也不表示本地包丢了；请恢复本机数据库读取后重试。</p>"#
            .to_owned(),
        Some([]) => r#"<p class="c-control-none">最近 100 个已消费导航的详情页会话里没有可对账的行。这不表示历史上没有会话，也不触发任何平台访问。</p>"#
            .to_owned(),
        Some(sessions) => sessions.iter().map(delivery_row).collect::<String>(),
    };
    format!(
        r#"<section class="c-task-delivery" data-delivery-reconciliation>
             <h2>交付对账</h2>
             <p class="c-task-boundary">按最近进度读取最多 100 个已消费导航的详情页会话，逐条核对冻结通道的包有没有拿到服务端回执。已有回执的会话不再算待交付；读不到冻结通道的会话只说「恢复待核实」和最后观察时间——包没到不等于没采到，等得久也不等于采不到。</p>
             <div class="c-task-delivery-list">{rows}</div>
           </section>"#
    )
}

fn delivery_row(session: &DetailDeliveryReconciliation) -> String {
    let (label, slug, note) = match session.conclusion {
        DeliveryConclusion::Delivered => (
            "已交付",
            "delivered",
            "冻结通道的包都已拿到回执",
        ),
        DeliveryConclusion::AwaitingDelivery => ("待交付", "awaiting_delivery", "还有通道没有回执"),
        DeliveryConclusion::RecoveryUnverified => (
            "恢复待核实",
            "recovery_unverified",
            "读不到这个会话的冻结通道",
        ),
        DeliveryConclusion::Closed => ("已终结", "closed", "会话已终结，不再接收进度"),
    };
    let state_class = match session.conclusion {
        DeliveryConclusion::Delivered => "c-task-state-ok",
        DeliveryConclusion::AwaitingDelivery => "c-task-state-wait",
        DeliveryConclusion::RecoveryUnverified => "c-task-state-warn",
        DeliveryConclusion::Closed => "c-task-state-wait",
    };
    let target = session
        .target_display_name
        .as_deref()
        .unwrap_or("目标关联当前未知");
    let content = session.content_external_id.as_deref().unwrap_or("内容引用未知");
    let platform = session.platform.as_deref().unwrap_or("平台未知");
    let lanes = if session.prepared_lanes == 0 {
        "没有可核对的冻结通道".to_owned()
    } else {
        format!(
            "{} / {} 通道已有回执",
            session.delivered_lanes, session.prepared_lanes
        )
    };
    let when = match (session.conclusion, session.closed_at.as_deref()) {
        (DeliveryConclusion::Closed, Some(closed)) => {
            format!("终结于 {closed} · 最后观察 {}", session.last_observed_at)
        }
        _ => format!("最后观察 {}", session.last_observed_at),
    };
    // 终结原因和会话状态都是机器码：中文先说话，机器码降为 Mono 旁注（LANG-05）。
    let closed_reason = (session.conclusion == DeliveryConclusion::Closed).then(|| {
        format!(
            "{}<code>{}</code>",
            escape(closed_reason_words(session)),
            escape(session.stop_reason.as_deref().unwrap_or(&session.state)),
        )
    });
    format!(
        r#"<div class="c-task-delivery-row" data-delivery-session="{session_ref}" data-delivery-conclusion="{slug}">
             <span class="c-task-delivery-target"><b>{target}</b><small>{content} · {platform}</small></span>
             <span class="c-task-state {state_class}"><b>{label}</b><small>{note}</small></span>
             <span class="c-task-delivery-lanes">{lanes}{closed_reason}</span>
             <time>{when}</time>
           </div>"#,
        session_ref = session.session_ref,
        slug = slug,
        target = escape(target),
        content = escape(content),
        platform = escape(platform),
        state_class = state_class,
        label = label,
        note = note,
        lanes = escape(&lanes),
        closed_reason = closed_reason
            .map(|reason| format!("<small>{reason}</small>"))
            .unwrap_or_default(),
        when = escape(&when),
    )
}

/// 终结原因的中文说法。认不出的码照实写「未记录」，不猜一个更好听的原因。
fn closed_reason_words(session: &DetailDeliveryReconciliation) -> &'static str {
    match session.stop_reason.as_deref() {
        Some("navigation_state_unknown") => "导航状态未知后终结",
        Some("owner_unavailable") => "执行工位不可用而终结",
        Some("page_unavailable") => "页面不可读而终结",
        Some("risk_stop") => "平台风控停止",
        Some("delivery_terminal") => "交付已明确终止",
        Some(_) => "停止原因未记录",
        None if session.state == "finished" => "会话正常终结",
        None => "终结原因未记录",
    }
}

struct TaskStateView {
    label: &'static str,
    class_name: &'static str,
    note: &'static str,
}

/// 拿到回执只说明服务端收到了这次提交；材料有没有通过接纳是另一条结论。
/// 两者都不写回执那一格是「未收到」，也不把没记录的接纳结论写成「未接纳」。
fn receipt_column(task: &CollectionTaskExecution) -> &'static str {
    match (task.receipt_ref.is_some(), task.material_admission.as_deref()) {
        (false, _) => "—",
        (true, Some("ACCEPTED")) => "已接纳",
        (true, _) => "接纳结论未记录",
    }
}

fn material_admission_words(admission: Option<&str>) -> &'static str {
    match admission {
        Some("ACCEPTED") => "材料已接纳",
        Some("UNKNOWN") => "接纳结论未记录",
        // 合同目前只有 UNKNOWN / ACCEPTED 两个取值；出现别的码照实说「未记录」，
        // 不把一个认不出的码当成功。
        Some(_) => "接纳结论未记录",
        None => "接纳结论未记录",
    }
}

/// 执行 effect 的中文说法，第二项是机器码旁注（可能为空）。执行权的结论只在有回执时
/// 才存在：没有回执时写「还没有执行结论」，不能写成「这一步不涉及执行权」。
fn execution_effect_reading(task: &CollectionTaskExecution) -> (&'static str, &str) {
    match task.execution_effect.as_deref() {
        Some("COMPLETED_LIVE_STEP") => ("材料已保存，执行权仍在", "COMPLETED_LIVE_STEP"),
        Some("LOST_AUTHORITY") => ("材料已保存，原执行权已失效", "LOST_AUTHORITY"),
        Some("NOT_APPLICABLE") => ("这一步不涉及执行权", "NOT_APPLICABLE"),
        Some("UNKNOWN") => ("执行结论未记录", "UNKNOWN"),
        Some(other) => ("执行结论未记录", other),
        None => ("尚未收到 Receipt，没有执行结论", ""),
    }
}

fn task_state(task: &CollectionTaskExecution) -> TaskStateView {
    if task.material_admission.as_deref() == Some("ACCEPTED") {
        return TaskStateView {
            label: "已接纳",
            class_name: "c-task-state-ok",
            note: "Package 与 Receipt 已保存",
        };
    }
    let (label, class_name, note) = match task.queue_state.as_deref() {
        Some("in_progress" | "pending") if task.has_live_lease == Some(false) => (
            "租约已失效",
            "c-task-state-warn",
            "租约已经到期；当前没有执行权",
        ),
        Some("in_progress") => ("执行中", "c-task-state-live", "任务已被独占领取"),
        Some("pending") if task.last_dispatch_failure_code.is_some() => (
            "等待重试",
            "c-task-state-warn",
            "上次页面启动未形成 Attempt",
        ),
        Some("pending") => ("等待派发", "c-task-state-wait", "尚未领取"),
        Some("unavailable") => (
            "页面暂不可读",
            "c-task-state-warn",
            "平台当前未提供该作品；未生成 Attempt、Package 或详情",
        ),
        Some("blocked") => (
            "详情读取受阻",
            "c-task-state-warn",
            "连续读取失败后已停止自动重试；未生成 Attempt、Package、Receipt 或详情",
        ),
        Some("completed") => (
            "步骤完成",
            "c-task-state-ok",
            "队列步骤已关闭，Receipt 读不到",
        ),
        _ if task.attempt_id.is_some() => (
            "等待回执",
            "c-task-state-warn",
            "已有 Attempt，尚未收到 Receipt",
        ),
        _ => ("尚未启动", "c-task-state-wait", "本机任务已保存"),
    };
    TaskStateView {
        label,
        class_name,
        note,
    }
}

fn source_label(source: &str) -> &str {
    match source {
        "scheduled" => "调度下发",
        "manual" => "本机交付",
        _ => source,
    }
}

fn short_ref(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn task() -> CollectionTaskExecution {
        CollectionTaskExecution {
            task_id: Uuid::parse_str("23d29f84-baee-4ed6-896b-0b2e5b8f7412").unwrap(),
            source: "scheduled".to_owned(),
            platform: "xhs".to_owned(),
            page_type: "profile".to_owned(),
            capabilities: "author_profile".to_owned(),
            created_at: "2026-09-02T16:57:00Z".to_owned(),
            sequence_no: Some(1),
            queue_state: Some("completed".to_owned()),
            claimed_at: Some("2026-09-02T16:57:06Z".to_owned()),
            has_live_lease: Some(false),
            target_display_name: Some("<真实目标>".to_owned()),
            target_identity_key: Some("creator-001".to_owned()),
            attempt_id: Some(Uuid::new_v4()),
            attempt_started_at: Some("2026-09-02T16:57:08Z".to_owned()),
            package_kind: Some("author_profile".to_owned()),
            package_ref: Some(Uuid::new_v4()),
            package_accepted_at: Some("2026-09-02T16:57:09Z".to_owned()),
            receipt_ref: Some(Uuid::new_v4()),
            receipt_received_at: Some("2026-09-02T16:57:09Z".to_owned()),
            execution_effect: Some("COMPLETED_LIVE_STEP".to_owned()),
            material_admission: Some("ACCEPTED".to_owned()),
            last_dispatch_failure_code: Some("page_timeout".to_owned()),
            last_dispatch_failure_at: Some("2026-09-02T16:56:00Z".to_owned()),
        }
    }

    #[test]
    fn renders_receipt_and_preserves_recovered_dispatch_failure() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let timeline = CollectionTaskTimeline {
            tasks: vec![task()],
            accepted_count: 1,
            active_count: 0,
            expired_lease_count: 0,
        };
        let html = render_tasks(&base, &timeline, None);
        assert!(html.contains("已接纳"));
        assert!(html.contains("page_timeout"));
        assert!(html.contains("后续回执已保留"));
        assert!(html.contains("&lt;真实目标&gt;"));
        assert!(!html.contains("<真实目标>"));
        assert!(!html.contains("采集任务当前未知"));
    }

    #[test]
    fn waiting_task_does_not_claim_completion() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut waiting = task();
        waiting.queue_state = Some("pending".to_owned());
        waiting.attempt_id = None;
        waiting.receipt_ref = None;
        waiting.material_admission = None;
        waiting.last_dispatch_failure_code = None;
        waiting.has_live_lease = Some(true);
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: vec![waiting],
                accepted_count: 0,
                active_count: 1,
                expired_lease_count: 0,
            },
            None,
        );
        assert!(html.contains("等待派发"));
        assert!(html.contains("data-task-state-class=\"c-task-state-wait\""));
        assert!(html.contains("class=\"c-state-line c-task-state-wait\""));
        assert!(html.contains("尚未形成 Attempt"));
        assert!(!html.contains("Package 与 Receipt 已保存"));
    }

    #[test]
    fn expired_lease_is_not_rendered_as_a_live_execution() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut expired = task();
        expired.queue_state = Some("in_progress".to_owned());
        expired.attempt_id = None;
        expired.receipt_ref = None;
        expired.material_admission = None;
        expired.has_live_lease = Some(false);
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: vec![expired],
                accepted_count: 0,
                active_count: 0,
                expired_lease_count: 1,
            },
            None,
        );
        assert!(html.contains("租约已失效"));
        assert!(html.contains("data-task-state-class=\"c-task-state-warn\""));
        assert!(html.contains("class=\"c-state-line c-task-state-warn\""));
        assert!(html.contains("租约已经到期；当前没有执行权"));
        assert!(!html.contains("任务已被独占领取"));
    }

    #[test]
    fn an_empty_successful_timeline_closes_the_frozen_work_read() {
        let base = format!(
            "<!-- collection-readout:start --><div>old readout</div><!-- collection-readout:end -->{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}"
        );
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: Vec::new(),
                accepted_count: 0,
                active_count: 0,
                expired_lease_count: 0,
            },
            None,
        );

        assert!(html.contains("当前没有任务，因此没有可核对的冻结 Work 引用"));
        assert!(!html.contains("冻结资源正在读取"));
        assert!(!html.contains("<!-- frozen-work:start -->"));
    }

    #[test]
    fn v4_task_ledger_and_inspector_preserve_the_execution_chain() {
        let base = format!(
            "<!-- collection-readout:start --><div>old readout</div><!-- collection-readout:end -->{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}"
        );
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: vec![task()],
                accepted_count: 1,
                active_count: 0,
                expired_lease_count: 0,
            },
            None,
        );

        assert!(html.contains("c-task-workspace"));
        assert!(html.contains("data-task-row"));
        assert!(html.contains("data-task-inspector"));
        for tab in ["overview", "attempt", "package", "receipt", "frozen"] {
            assert!(html.contains(&format!("data-task-tab=\"{tab}\"")));
            assert!(html.contains(&format!("data-task-panel=\"{tab}\"")));
            assert!(html.contains(&format!("aria-controls=\"task-panel-{tab}\"")));
            assert!(html.contains(&format!("aria-labelledby=\"task-tab-{tab}\"")));
        }
        assert!(html.contains("role=\"tablist\""));
        assert!(html.contains("role=\"tab\""));
        assert!(html.contains("role=\"tabpanel\""));
        assert!(html.contains(&format!("data-task-id=\"{}\"", task().task_id)));
        assert!(html.contains("<!-- frozen-work:start -->"));
        assert!(!html.contains("old readout"));
        assert_eq!(html.matches("class=\"v7-kpi\"").count(), 4);
        assert!(!html.contains("c-readout-strip"));
        let script = include_str!("collection_workspace.js");
        for key in ["ArrowRight", "ArrowLeft", "Home", "End"] {
            assert!(script.contains(key));
        }
        assert!(script.contains("candidate.setAttribute(\"aria-selected\""));
        assert!(script.contains("panel.hidden = !selected"));
        assert!(script.contains("row.dataset.taskStateClass"));
        assert!(script.contains("stateView.classList.remove"));
    }

    #[test]
    fn a_failed_delivery_read_is_unknown_not_zero() {
        let base = format!(
            "<!-- collection-readout:start --><div>old readout</div><!-- collection-readout:end -->{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}"
        );
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: vec![task()],
                accepted_count: 1,
                active_count: 0,
                expired_lease_count: 0,
            },
            None,
        );

        assert!(html.contains("交付对账本次读取失败"));
        assert!(!html.contains("没有可对账的行"));
        // 读不到投影时两个读数都是 UNKNOWN（屏幕上显示「未知」），不是 0。
        assert!(html.contains("<em>待交付</em><b aria-hidden=\"true\">UNKNOWN</b>"));
        assert!(html.contains("<em>恢复待核实</em><b aria-hidden=\"true\">UNKNOWN</b>"));
    }

    #[test]
    fn a_receipt_with_lost_authority_is_not_rendered_as_a_plain_success() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut task = task();
        task.execution_effect = Some("LOST_AUTHORITY".to_owned());
        let html = render_tasks(
            &base,
            &CollectionTaskTimeline {
                tasks: vec![task],
                accepted_count: 1,
                active_count: 0,
                expired_lease_count: 0,
            },
            None,
        );

        assert!(html.contains("材料已保存，原执行权已失效"));
        assert!(html.contains("data-task-inspector-effect-code>LOST_AUTHORITY</code>"));
        // 机器码只是旁注，不再顶替中文出现在回执行里。
        assert!(!html.contains("data-task-effect=\"LOST_AUTHORITY\""));
    }

    /// 会话说自己在等交付，但冻结通道全部拿到回执时，结论是已交付——页面不能继续
    /// 把这一行留在「待交付」里，也不能把 `delivery_pending` 这个机器状态当结论。
    #[test]
    fn a_delivery_pending_session_with_every_lane_delivered_reads_as_delivered() {
        let html = render_tasks(
            &readout_base(),
            &timeline_with(vec![task()]),
            Some(&[session("delivery_pending", None, 4, 4, None)]),
        );

        assert!(html.contains("data-delivery-conclusion=\"delivered\""));
        assert!(html.contains("4 / 4 通道已有回执"));
        assert!(!html.contains("data-delivery-conclusion=\"awaiting_delivery\""));
        assert!(html.contains("<em>待交付</em><b aria-hidden=\"true\">0</b>"));
    }

    /// 冻结通道读到一部分时才是待交付；读不到任何通道时不猜「没采到」，只说
    /// 恢复待核实并附最后观察时间。
    #[test]
    fn partial_lanes_await_delivery_and_missing_lanes_stay_unverified() {
        let html = render_tasks(
            &readout_base(),
            &timeline_with(vec![task()]),
            Some(&[
                session("delivery_pending", None, 4, 1, None),
                session("delivery_pending", None, 0, 0, None),
            ]),
        );

        assert!(html.contains("data-delivery-conclusion=\"awaiting_delivery\""));
        assert!(html.contains("1 / 4 通道已有回执"));
        assert!(html.contains("data-delivery-conclusion=\"recovery_unverified\""));
        assert!(html.contains("没有可核对的冻结通道"));
        assert!(html.contains("<em>待交付</em><b aria-hidden=\"true\">1</b>"));
        assert!(html.contains("<em>恢复待核实</em><b aria-hidden=\"true\">1</b>"));
    }

    /// 终结压过一切：已终结的会话不再渲染成待交付（哪怕它还欠着三个通道），并且五种
    /// 终结原因各自说成中文，机器码只作 Mono 旁注。
    #[test]
    fn a_closed_session_reports_its_terminal_reason_in_chinese() {
        for (reason, words) in [
            ("navigation_state_unknown", "导航状态未知后终结"),
            ("owner_unavailable", "执行工位不可用而终结"),
            ("page_unavailable", "页面不可读而终结"),
            ("risk_stop", "平台风控停止"),
            ("delivery_terminal", "交付已明确终止"),
        ] {
            let html = render_tasks(
                &readout_base(),
                &timeline_with(vec![task()]),
                Some(&[session(
                    "stopped",
                    Some(reason),
                    4,
                    1,
                    Some("2026-09-21T09:00:00Z"),
                )]),
            );

            assert!(
                html.contains("data-delivery-conclusion=\"closed\""),
                "{reason} must read as a closed session"
            );
            assert!(html.contains(words), "{reason} must speak Chinese");
            assert!(html.contains(&format!("<code>{reason}</code>")));
            assert!(html.contains("终结于 2026-09-21T09:00:00Z"));
            assert!(!html.contains("data-delivery-conclusion=\"awaiting_delivery\""));
            assert!(html.contains("<em>待交付</em><b aria-hidden=\"true\">0</b>"));
        }
    }

    fn readout_base() -> String {
        format!(
            "<!-- collection-readout:start --><div>old readout</div><!-- collection-readout:end -->{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}"
        )
    }

    fn timeline_with(tasks: Vec<CollectionTaskExecution>) -> CollectionTaskTimeline {
        CollectionTaskTimeline {
            active_count: tasks.len() as i64,
            accepted_count: 0,
            expired_lease_count: 0,
            tasks,
        }
    }

    fn session(
        state: &str,
        stop_reason: Option<&str>,
        prepared_lanes: i64,
        delivered_lanes: i64,
        closed_at: Option<&str>,
    ) -> DetailDeliveryReconciliation {
        let conclusion = match state {
            "finished" | "stopped" => DeliveryConclusion::Closed,
            _ if prepared_lanes == 0 => DeliveryConclusion::RecoveryUnverified,
            _ if delivered_lanes == prepared_lanes => DeliveryConclusion::Delivered,
            _ => DeliveryConclusion::AwaitingDelivery,
        };
        DetailDeliveryReconciliation {
            session_ref: Uuid::new_v4(),
            conclusion,
            state: state.to_owned(),
            stop_reason: stop_reason.map(str::to_owned),
            platform: Some("xhs".to_owned()),
            content_external_id: Some("note-001".to_owned()),
            target_display_name: Some("<详情目标>".to_owned()),
            prepared_lanes,
            delivered_lanes,
            last_observed_at: "2026-09-21T08:30:00Z".to_owned(),
            closed_at: closed_at.map(str::to_owned),
        }
    }
}
