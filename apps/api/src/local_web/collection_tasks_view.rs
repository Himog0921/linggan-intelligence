//! The bounded Task / Attempt / Package / Receipt read surface.
//!
//! This module intentionally renders only persisted local facts.  It must not
//! turn a queue claim into a completed task, hide a recovered page-start
//! failure, or manufacture a target name when a task has no target projection.

use linggan_evidence::{CollectionTaskExecution, CollectionTaskTimeline};

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
            </section>"#
            .to_owned()
    } else {
        format!(
            r#"<section class="c-tasks">
                  <div class="c-task-summary">
                    <div><b>{total}</b><span>最近任务</span></div>
                    <div><b>{accepted}</b><span>已接纳</span></div>
                    <div><b>{active}</b><span>待完成</span></div>
                    <div><b>{expired}</b><span>租约失效</span></div>
                  </div>
                  <p class="c-task-boundary">按最近状态读取最多 100 条本机任务。队列资格、Attempt、Package 与 Receipt 分列显示；读取本身不会领取、重试或新建任务。</p>
                  <div class="c-task-list">{rows}</div>
                </section>"#,
            total = timeline.tasks.len(),
            accepted = timeline.accepted_count,
            active = timeline.active_count,
            expired = timeline.expired_lease_count,
            rows = timeline.tasks.iter().map(task_row).collect::<String>(),
        )
    };
    format!("{}{body}{}", &base[..open], &base[close..])
}

fn task_row(task: &CollectionTaskExecution) -> String {
    let (state, state_class, state_note) = task_state(task);
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
    let previous_failure =
        task.last_dispatch_failure_code
            .as_ref()
            .map_or_else(String::new, |code| {
                let when = task
                    .last_dispatch_failure_at
                    .as_deref()
                    .unwrap_or("时间未知");
                format!(
                    r#"<p class="c-task-history">此前启动失败：<code>{}</code> · {}；{}。</p>"#,
                    escape(code),
                    escape(when),
                    if task.receipt_ref.is_some() {
                        "后续回执已保留"
                    } else {
                        "当前尚未有 Receipt"
                    },
                )
            });
    let effect = task
        .execution_effect
        .as_deref()
        .map_or_else(String::new, |effect| {
            format!(r#"<span class="c-task-effect">{}</span>"#, escape(effect))
        });

    format!(
        r#"<article class="c-task-row">
              <div class="c-task-head">
                <div>
                  <p class="c-task-kicker">{source} · {platform} · {page_type}{sequence}</p>
                  <h2>{capabilities}</h2>
                  <p class="c-task-target">目标：<b>{target}</b> <code>{task_ref}</code></p>
                </div>
                <div class="c-task-state {state_class}"><b>{state}</b><span>{state_note}</span></div>
              </div>
              <dl class="c-task-facts">
                <div><dt>Attempt</dt><dd>{attempt}</dd></div>
                <div><dt>Package</dt><dd>{package}</dd></div>
                <div><dt>Receipt</dt><dd>{receipt} {effect}</dd></div>
              </dl>
              {previous_failure}
              <p class="c-task-created">创建于 {created}</p>
            </article>"#,
        source = escape(source_label(&task.source)),
        platform = escape(&task.platform),
        page_type = escape(&task.page_type),
        sequence = sequence,
        capabilities = escape(&task.capabilities),
        target = escape(target),
        task_ref = escape(task_ref),
        state = state,
        state_class = state_class,
        state_note = escape(state_note),
        attempt = escape(&attempt),
        package = escape(&package),
        receipt = escape(&receipt),
        effect = effect,
        previous_failure = previous_failure,
        created = escape(&task.created_at),
    )
}

fn task_state(task: &CollectionTaskExecution) -> (&'static str, &'static str, &'static str) {
    if task.material_admission.as_deref() == Some("ACCEPTED") {
        return ("已接纳", "c-task-state-ok", "Package 与 Receipt 已保存");
    }
    match task.queue_state.as_deref() {
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
        assert!(html.contains("租约已经到期；当前没有执行权"));
        assert!(!html.contains("任务已被独占领取"));
    }
}
