//! 任务规格的风险策略必须与来源配对——两个方向都要挡。
//!
//! 放在集成测试里而不是模块内，因为 `producer_runtime.rs` 是生产文件，受行数上限约束。

use linggan_contracts::{
    LOCAL_TRUSTED_RISK_POLICY, SERVER_LEASED_RISK_POLICY, parse_producer_task_spec,
};

fn spec(source: &str, risk_policy: &str) -> String {
    format!(
        r#"{{"contractVersion":"linggan.producer.task-spec.v1",
                "taskId":"aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa","source":"{source}",
                "platform":"xhs","pageType":"profile","target":{{"authorExternalId":"a"}},
                "capabilitiesRequested":["profile_discovery"],"maximumQuota":200,
                "commentLimit":"not_requested","acquireMedia":"not_requested",
                "riskPolicy":"{risk_policy}","stopConditions":["maximum_quota"]}}"#
    )
}

#[test]
fn each_source_accepts_only_its_own_risk_policy() {
    assert!(parse_producer_task_spec(&spec("manual", LOCAL_TRUSTED_RISK_POLICY)).is_ok());
    assert!(parse_producer_task_spec(&spec("scheduled", SERVER_LEASED_RISK_POLICY)).is_ok());
}

#[test]
fn a_manual_task_cannot_mint_a_server_authorized_policy() {
    // 若只放开「scheduled 可用服务端策略」而不禁止这一条，插件就能自签一份
    // 「服务端已授权」的任务，整条授权链被绕过。
    assert!(parse_producer_task_spec(&spec("manual", SERVER_LEASED_RISK_POLICY)).is_err());
}

#[test]
fn a_scheduled_task_cannot_claim_to_be_user_initiated() {
    // 反向同样要挡：服务端派发的任务谎称「人在键盘前点的」，会让追责链指向错误的人。
    assert!(parse_producer_task_spec(&spec("scheduled", LOCAL_TRUSTED_RISK_POLICY)).is_err());
}
