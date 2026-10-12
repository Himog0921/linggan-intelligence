//! User-facing explanations for existing manual-observation command outcomes.

pub(super) fn command_problem(code: &str) -> &'static str {
    match code {
        "stale_revision" | "monitor_rule_command_stale" => {
            "规则版本已经变化。已保留所选目标，请核对当前规则后重新提交。"
        }
        "identity_conflict" => "命令标识对应了不同内容，未覆盖原申请。请核对后重新提交。",
        "baseline_not_ready" | "keyword_archive_incomplete" => {
            "关键词建档尚未完成，请先完成搜索面和待补详情。"
        }
        "target_domain_unassigned" => "目标当前没有唯一有效的主用途领域，请检查领域归属。",
        "target_not_requestable" => "这个目标当前不允许新增观察工作。",
        "authorization_missing" | "authorization_expired_or_revoked" => {
            "当前没有适用的有效授权，未创建新的采集工单。"
        }
        "authorization_scope_mismatch" | "authorization_purpose_mismatch" => {
            "当前授权没有覆盖这次观察的范围或用途。"
        }
        "authorization_target_limit_reached" | "authorization_work_unit_limit_reached" => {
            "本次观察超出授权额度，未创建新的采集工单。"
        }
        "station_unavailable"
        | "station_not_accepting"
        | "installation_credential_missing"
        | "plugin_version_unsupported"
        | "installation_stale"
        | "capability_missing" => "工位当前未具备接活条件，请在工位管理中核对。",
        "account_unbound"
        | "account_binding_changed"
        | "account_binding_expired"
        | "account_eligibility_stale"
        | "account_needs_login"
        | "account_restricted"
        | "account_unknown" => "当前账号身份或资格未通过准入，请在工位管理中核对账号。",
        "risk_paused" | "installation_risk_cooldown" | "account_cooling" => {
            "当前处于风险暂停或冷却中，请核对工位状态后再申请。"
        }
        "account_busy"
        | "station_busy"
        | "station_daily_budget_reached"
        | "platform_concurrency_reached"
        | "capacity_unknown" => "当前执行容量未通过准入，请在工位管理中核对可用状态。",
        "need_already_satisfied" | "in_flight_work_covers_it" => {
            "已有结果或在途工作覆盖了这次申请，请核对已有任务。"
        }
        "read_model_not_connected" | "database_unavailable" | "schema_unavailable" => {
            "当前无法读取或提交采集命令，请恢复本机服务后核对。"
        }
        _ => "本次申请没有可确认的新工单。请核对回执原因与目标、工位状态。",
    }
}
