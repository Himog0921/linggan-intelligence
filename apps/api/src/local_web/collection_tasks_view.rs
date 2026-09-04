//! The bounded Task / Attempt / Package / Receipt read surface.
//!
//! This module intentionally renders only persisted local facts.  It must not
//! turn a queue claim into a completed task, hide a recovered page-start
//! failure, or manufacture a target name when a task has no target projection.

use linggan_evidence::{CollectionTaskExecution, CollectionTaskTimeline};

use super::collection::{
    READOUT_SLOT_END, READOUT_SLOT_START, context_readout, replace_bounded_slot,
};

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

pub fn render_tasks(base: &str, timeline: &CollectionTaskTimeline) -> String {
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
    let rendered = format!("{}{body}{}", &base[..open], &base[close..]);
    replace_bounded_slot(
        &rendered,
        READOUT_SLOT_START,
        READOUT_SLOT_END,
        &task_readout(timeline),
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
            let admission = task.material_admission.as_deref().unwrap_or("UNKNOWN");
            format!(
                "Receipt {} · {} · {}",
                short_ref(&receipt_ref.to_string()),
                admission,
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
    let receipt_state = task.material_admission.as_deref().unwrap_or("—");
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
        r#"<button type="button" class="c-task-row{selected_class}" data-task-row data-task-id="{task_id}" data-task-title="{target}" data-task-ref="{task_ref}" data-task-meta="{meta}" data-task-state="{state}" data-task-state-class="{state_class}" data-task-state-note="{state_note}" data-task-attempt="{attempt}" data-task-package="{package}" data-task-receipt="{receipt}" data-task-effect="{effect_text}" data-task-failure="{previous_failure}" data-task-created="{created}" data-task-capabilities="{capabilities}" aria-pressed="{pressed}">
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
        effect_text = escape(
            task.execution_effect
                .as_deref()
                .unwrap_or("没有执行 effect")
        ),
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
                task.material_admission.as_deref().unwrap_or("UNKNOWN"),
                task.receipt_received_at
                    .as_deref()
                    .unwrap_or("接收时间未知")
            )
        },
    );
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
               <section class="c-task-panel" id="task-panel-receipt" data-task-panel="receipt" role="tabpanel" aria-labelledby="task-tab-receipt" hidden><h3>Receipt</h3><p data-task-inspector-receipt>{receipt}</p><p data-task-inspector-effect>{effect}</p></section>
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
        effect = escape(
            task.execution_effect
                .as_deref()
                .unwrap_or("没有执行 effect")
        ),
    )
}

fn task_readout(timeline: &CollectionTaskTimeline) -> String {
    let active = timeline.active_count.to_string();
    let expired = timeline.expired_lease_count.to_string();
    context_readout(&[
        (&active, "待完成", "最近 100 条任务投影的待完成数"),
        (&expired, "租约失效", "最近 100 条任务投影的失效租约数"),
    ])
}

struct TaskStateView {
    label: &'static str,
    class_name: &'static str,
    note: &'static str,
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
        let html = render_tasks(&base, &timeline);
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
        assert_eq!(html.matches("class=\"v7-kpi\"").count(), 2);
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
}
