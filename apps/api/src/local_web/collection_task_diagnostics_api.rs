//! Read-only migration of the bounded legacy Task/Attempt/Package/Receipt inspector.
//! Domain readers own state and frozen resources; this adapter emits no HTML and performs no
//! admission, dispatch, recovery, acknowledgement or external access.
use super::collection::collection_control_surface_view::{
    CollectionTaskControlRead, FrozenWorkView, read_task_control_surface,
};
use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_evidence::{
    CollectionTaskExecution, DeliveryConclusion, DetailDeliveryReconciliation,
    read_collection_task_timeline, read_detail_delivery_reconciliation,
};
use serde_json::{Value, json};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new().route("/api/collection/workspace/task-diagnostics", get(read))
}
async fn read(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "collection_task_diagnostics_unavailable",
        );
    };
    let (timeline, delivery, frozen) = tokio::join!(
        read_collection_task_timeline(db, 100),
        read_detail_delivery_reconciliation(db, 100),
        read_task_control_surface(db, 100)
    );
    let timeline = match timeline {
        Ok(v) => v,
        Err(_) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "collection_task_diagnostics_unavailable",
            );
        }
    };
    let delivery = delivery.ok();
    let awaiting = delivery.as_ref().map(|v| {
        v.iter()
            .filter(|s| s.conclusion == DeliveryConclusion::AwaitingDelivery)
            .count()
    });
    let unverified = delivery.as_ref().map(|v| {
        v.iter()
            .filter(|s| s.conclusion == DeliveryConclusion::RecoveryUnverified)
            .count()
    });
    let frozen = match frozen {
        Ok(CollectionTaskControlRead::Ready(v)) => Some(v),
        _ => None,
    };
    Json(json!({"limit":100,"active":timeline.active_count,"expired":timeline.expired_lease_count,"awaiting":awaiting,"unverified":unverified,"tasks":timeline.tasks.iter().map(task).collect::<Vec<_>>(),"delivery":delivery.map(|v|v.iter().map(delivery_row).collect::<Vec<_>>()),"frozen":frozen.map(|v|v.iter().map(frozen_row).collect::<Vec<_>>())})).into_response()
}
fn task(t: &CollectionTaskExecution) -> Value {
    let state = task_state(t);
    let (effect, effect_code) = execution_effect_reading(t);
    json!({"taskId":t.task_id,"target":t.target_display_name.as_deref().or(t.target_identity_key.as_deref()).unwrap_or("目标关联当前未知"),"source":source_label(&t.source),"platform":t.platform,"pageType":t.page_type,"capabilities":t.capabilities,"domains":t.domain_usage,"created":t.created_at,"sequence":t.sequence_no,"state":state.label,"stateClass":state.class_name,"stateNote":state.note,"attemptId":t.attempt_id,"attemptStarted":t.attempt_started_at,"packageRef":t.package_ref,"packageKind":t.package_kind,"packageAccepted":t.package_accepted_at,"receiptRef":t.receipt_ref,"receiptReceived":t.receipt_received_at,"receiptLabel":receipt_column(t),"admission":material_admission_words(t.material_admission.as_deref()),"effect":effect,"effectCode":effect_code,"failure":t.last_dispatch_failure_code,"failureAt":t.last_dispatch_failure_at})
}
fn delivery_row(s: &DetailDeliveryReconciliation) -> Value {
    let (label, slug, note, class) = match s.conclusion {
        DeliveryConclusion::Delivered => (
            "已交付",
            "delivered",
            "冻结通道的包都已拿到回执",
            "c-task-state-ok",
        ),
        DeliveryConclusion::AwaitingDelivery => (
            "待交付",
            "awaiting_delivery",
            "还有通道没有回执",
            "c-task-state-wait",
        ),
        DeliveryConclusion::RecoveryUnverified => (
            "恢复待核实",
            "recovery_unverified",
            "读不到这个会话的冻结通道",
            "c-task-state-warn",
        ),
        DeliveryConclusion::Closed => (
            "已终结",
            "closed",
            "会话已终结，不再接收进度",
            "c-task-state-wait",
        ),
    };
    json!({"sessionRef":s.session_ref,"target":s.target_display_name.as_deref().unwrap_or("目标关联当前未知"),"content":s.content_external_id.as_deref().unwrap_or("内容引用未知"),"platform":s.platform.as_deref().unwrap_or("平台未知"),"label":label,"slug":slug,"note":note,"class":class,"prepared":s.prepared_lanes,"delivered":s.delivered_lanes,"observed":s.last_observed_at,"closed":s.closed_at,"closedReason":if s.conclusion==DeliveryConclusion::Closed {Some(closed_reason_words(s))}else{None},"reasonCode":s.stop_reason.as_deref().unwrap_or(&s.state)})
}
fn frozen_row(w: &FrozenWorkView) -> Value {
    json!({"workRef":w.work_order_ref,"taskId":w.task_id,"target":w.target_name,"lane":match w.lane.as_str(){"deep_archive"=>"基线建档","patrol"=>"巡检",_=>"未知 lane"},"created":w.created_at,"leaseRef":w.lease_ref,"leaseLabel":match w.lease_state.as_str(){"live"=>"Lease 有效","released"=>"Lease 已结束","expired"=>"Lease 已过期",_=>"尚无 Lease"},"taskState":w.task_state,"resources":[resource("工位",w.station_ref,w.station_is_current),resource("安装",w.installation_ref,w.installation_is_current),resource("观察账号",w.account_ref,w.account_is_current),resource("资格回执",w.eligibility_ref,None),resource("规则版本",w.monitor_rule_revision_ref,w.rule_is_current)]})
}
fn resource(label: &str, id: Option<uuid::Uuid>, current: Option<bool>) -> Value {
    json!({"label":label,"ref":id,"state":match current{Some(true)=>"当前一致",Some(false)=>"当前已变化",None=>"当时未冻结"}})
}

pub(super) fn closed_reason_words(session: &DetailDeliveryReconciliation) -> &'static str {
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

pub(super) struct TaskStateView {
    pub(super) label: &'static str,
    pub(super) class_name: &'static str,
    pub(super) note: &'static str,
}

/// 拿到回执只说明服务端收到了这次提交；材料有没有通过接纳是另一条结论。
/// 两者都不写回执那一格是「未收到」，也不把没记录的接纳结论写成「未接纳」。
pub(super) fn receipt_column(task: &CollectionTaskExecution) -> &'static str {
    match (
        task.receipt_ref.is_some(),
        task.material_admission.as_deref(),
    ) {
        (false, _) => "—",
        (true, Some("ACCEPTED")) => "已接纳",
        (true, _) => "接纳结论未记录",
    }
}

pub(super) fn material_admission_words(admission: Option<&str>) -> &'static str {
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
pub(super) fn execution_effect_reading(task: &CollectionTaskExecution) -> (&'static str, &str) {
    match task.execution_effect.as_deref() {
        Some("COMPLETED_LIVE_STEP") => ("材料已保存，执行权仍在", "COMPLETED_LIVE_STEP"),
        Some("LOST_AUTHORITY") => ("材料已保存，原执行权已失效", "LOST_AUTHORITY"),
        Some("NOT_APPLICABLE") => ("这一步不涉及执行权", "NOT_APPLICABLE"),
        Some("UNKNOWN") => ("执行结论未记录", "UNKNOWN"),
        Some(other) => ("执行结论未记录", other),
        None => ("尚未收到 Receipt，没有执行结论", ""),
    }
}

pub(super) fn task_state(task: &CollectionTaskExecution) -> TaskStateView {
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

pub(super) fn source_label(source: &str) -> &str {
    match source {
        "scheduled" => "调度下发",
        "manual" => "本机交付",
        _ => source,
    }
}

pub(super) fn short_ref(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn task() -> CollectionTaskExecution {
        CollectionTaskExecution {
            task_id: uuid::Uuid::parse_str("23d29f84-baee-4ed6-896b-0b2e5b8f7412").unwrap(),
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
            domain_usage: "考研自习 · 主研究 · 当前已暂停；效率学习 · 参照 · 当前运行中".to_owned(),
            attempt_id: Some(uuid::Uuid::new_v4()),
            attempt_started_at: Some("2026-09-02T16:57:08Z".to_owned()),
            package_kind: Some("author_profile".to_owned()),
            package_ref: Some(uuid::Uuid::new_v4()),
            package_accepted_at: Some("2026-09-02T16:57:09Z".to_owned()),
            receipt_ref: Some(uuid::Uuid::new_v4()),
            receipt_received_at: Some("2026-09-02T16:57:09Z".to_owned()),
            execution_effect: Some("COMPLETED_LIVE_STEP".to_owned()),
            material_admission: Some("ACCEPTED".to_owned()),
            last_dispatch_failure_code: Some("page_timeout".to_owned()),
            last_dispatch_failure_at: Some("2026-09-02T16:56:00Z".to_owned()),
        }
    }

    #[test]
    fn accepted_material_and_lost_authority_are_distinct_with_recovered_failure_retained() {
        let mut t = task();
        t.material_admission = Some("ACCEPTED".into());
        t.execution_effect = Some("LOST_AUTHORITY".into());
        t.last_dispatch_failure_code = Some("page_start_failed".into());
        let v = super::task(&t);
        assert_eq!(v["state"], "已接纳");
        assert_eq!(v["effect"], "材料已保存，原执行权已失效");
        assert_eq!(v["failure"], "page_start_failed");
    }
    #[test]
    fn expired_historical_in_progress_never_claims_live_execution() {
        let mut t = task();
        t.material_admission = None;
        t.receipt_ref = None;
        t.queue_state = Some("in_progress".into());
        t.has_live_lease = Some(false);
        let v = super::task(&t);
        assert_eq!(v["state"], "租约已失效");
        assert_eq!(v["receiptLabel"], "—");
    }
    #[test]
    fn a_receipt_without_admission_does_not_invent_acceptance() {
        let mut t = task();
        t.material_admission = None;
        t.receipt_ref = Some(uuid::Uuid::nil());
        let v = super::task(&t);
        assert_eq!(v["receiptLabel"], "接纳结论未记录");
        assert_ne!(v["state"], "已接纳");
    }
}
