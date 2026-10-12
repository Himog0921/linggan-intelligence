use super::*;

fn panel(kind: &str) -> MonitorRulePanel {
    MonitorRulePanel {
        target_ref: Uuid::from_u128(1),
        target_name: "待核对目标".to_owned(),
        target_kind: kind.to_owned(),
        identity_key: "known-target".to_owned(),
        lifecycle_state: "paused".to_owned(),
        active_rule: None,
        receipt: None,
        editing_slot: None,
        available_rankings: Vec::new(),
    }
}

#[test]
fn decoration_preserves_the_complete_task_surface_and_script_order() {
    let original = r#"<div class="c-body"><!-- collection-body:start --><div data-task-inspector>账本、冻结资源</div><section data-delivery-reconciliation>对账</section><!-- collection-body:end --></div><script src="/assets/collection-workspace.js"></script>"#;
    let params = TaskActionParams {
        once: Some("1".to_owned()),
        ..Default::default()
    };
    let overlay = modal(&params, "选择与回执");
    let rendered = decorate_document(original, None, &overlay);
    assert!(rendered.contains(original.split("<script").next().unwrap()));
    assert_eq!(rendered.matches("data-task-actions").count(), 1);
    assert!(rendered.find("c-monitor-rule").unwrap() < rendered.find("<script").unwrap());
    assert!(rendered.contains("data-monitor-rule-initial-focus"));
    assert!(rendered.contains("data-return-url=\"/collection/tasks\""));
    assert!(rendered.contains(&format!("data-return-focus=\"{OPENER_ID}\"")));
}

#[test]
fn command_uses_existing_route_and_keeps_navigation_separate_from_purpose() {
    let params = TaskActionParams {
        domain: Some("other\"&<domain>".to_owned()),
        ..Default::default()
    };
    let html = command_form(
        &panel("creator"),
        Some(&["实际主用途".to_owned()]),
        KeywordArchiveRead::Complete,
        &params,
    );
    assert!(html.contains("action=\"/collection/targets/rules\""));
    assert!(html.contains("name=\"command_kind\" value=\"manual_observe\""));
    assert!(html.contains("name=\"expected_revision\" value=\"0\""));
    assert!(html.contains("name=\"idempotency_key\""));
    assert!(html.contains("name=\"return_scope\" value=\"tasks\""));
    assert!(html.contains("value=\"实际主用途\" readonly"));
    assert!(html.contains("value=\"other&quot;&amp;&lt;domain&gt;\""));
    assert!(!html.contains("name=\"domain_ref\""));
    assert!(!html.contains("value=\"manual_observe\" disabled"));
}

#[test]
fn keyword_unknown_and_incomplete_archives_never_offer_an_enabled_submission() {
    let domains = ["领域".to_owned()];
    for archive in [
        KeywordArchiveRead::Unavailable,
        KeywordArchiveRead::NotArchived,
        KeywordArchiveRead::DetailPending,
    ] {
        let html = command_form(
            &panel("keyword"),
            Some(&domains),
            archive,
            &TaskActionParams::default(),
        );
        assert!(html.contains("value=\"manual_observe\" disabled"));
    }
    assert!(
        command_gate(
            &panel("keyword"),
            Some(&domains),
            KeywordArchiveRead::Complete
        )
        .is_none()
    );
    assert!(command_gate(&panel("creator"), None, KeywordArchiveRead::Complete).is_some());
    assert!(command_gate(&panel("creator"), Some(&[]), KeywordArchiveRead::Complete).is_some());
    assert!(
        command_gate(
            &panel("creator"),
            Some(&["一".to_owned(), "二".to_owned()]),
            KeywordArchiveRead::Complete
        )
        .is_some()
    );
    let mut dismissed = panel("creator");
    dismissed.lifecycle_state = "dismissed".to_owned();
    assert!(command_gate(&dismissed, Some(&domains), KeywordArchiveRead::Complete).is_some());
}

#[test]
fn durable_receipt_has_exact_work_ref_and_replay_of_rejection_is_not_success() {
    let work_ref = Uuid::from_u128(20);
    let mut receipt = ManualObservationReceipt {
        receipt_ref: Uuid::from_u128(10),
        outcome: "applied".to_owned(),
        reason: "manual_observe_created".to_owned(),
        work_order_ref: Some(work_ref),
        recorded_at: "2026-10-12 10:00".to_owned(),
    };
    let html = receipt_markup(&receipt);
    assert!(html.contains(&work_ref.to_string()));
    assert!(html.contains("尚不能证明已经执行"));
    receipt.outcome = "replay".to_owned();
    receipt.reason = "authorization_missing".to_owned();
    receipt.work_order_ref = None;
    let html = receipt_markup(&receipt);
    assert!(!html.contains("c-rule-feedback-ok"));
    assert!(html.contains("当前没有适用的有效授权"));
    assert!(html.contains("未记录工单"));
}

#[test]
fn return_path_keeps_selection_and_encodes_only_local_query_components() {
    let target = Uuid::from_u128(1);
    let receipt = Uuid::from_u128(2);
    let path = return_path(
        target,
        Some("other&once=0"),
        Some(receipt),
        Some("stale_revision"),
    );
    assert!(path.starts_with("/collection/tasks?domain=other%26once%3D0&once=1"));
    assert!(path.contains(&format!("&target={target}&receipt={receipt}")));
    assert!(path.ends_with("&error=stale_revision"));
}

#[test]
fn a_selected_target_outside_the_readable_chooser_survives_an_error() {
    let target_ref = Uuid::from_u128(3);
    let params = TaskActionParams {
        once: Some("1".to_owned()),
        target: Some(target_ref.to_string()),
        error: Some("database_unavailable".to_owned()),
        ..Default::default()
    };
    let html = selection_form(&[], None, &params);
    assert!(html.contains(&format!("value=\"{target_ref}\" selected")));
    assert!(html.contains("当前读不到"));
    assert!(!html.contains("manual_observe"));
    assert!(unavailable_content(&params, "当前读不到").contains(&target_ref.to_string()));
    assert!(!unavailable_content(&params, "当前读不到").contains("method=\"post\""));
}

#[test]
fn a_redirect_hint_cannot_override_a_durable_replay_receipt() {
    let mut params = TaskActionParams {
        receipt: Some(Uuid::from_u128(2).to_string()),
        error: Some("monitor_rule_command_replayed".to_owned()),
        ..Default::default()
    };
    assert!(form_feedback(&params).is_empty());
    params.receipt = None;
    params.error = Some("database_unavailable".to_owned());
    assert!(form_feedback(&params).contains("当前无法读取或提交采集命令"));
}

#[test]
fn the_manual_form_carries_the_read_revision_without_a_rule_draft() {
    use super::super::collection::collection_control_rule_view::ActiveMonitorRule;

    let mut target = panel("keyword");
    target.active_rule = Some(ActiveMonitorRule {
        rule_revision_ref: Uuid::from_u128(7),
        revision: 7,
        automatic_enabled: false,
        creator_follow_details: false,
        fixed_interval_seconds: Some(86_400),
        fallback_interval_seconds: 86_400,
        surface_key: "keyword_search".to_owned(),
        ranking_key: Some("most_liked".to_owned()),
        scroll_rounds: Some(3),
        top_by_likes: Some(20),
        published_within_days: Some(30),
        task_contract_version: linggan_contracts::PRODUCER_TASK_SPEC_VERSION.to_owned(),
        created_at: "2026-10-12 10:00".to_owned(),
    });
    let html = command_form(
        &target,
        Some(&["实际主用途".to_owned()]),
        KeywordArchiveRead::Complete,
        &TaskActionParams::default(),
    );
    assert!(html.contains("name=\"expected_revision\" value=\"7\""));
    assert!(html.contains("最多点赞 · 版本 7 · 下拉 3 次 · 取赞前 20 · 近 30 天"));
    assert!(!html.contains("name=\"automatic_enabled\""));
    assert!(!html.contains("name=\"rule_slot\""));
    let identity = html
        .split("name=\"idempotency_key\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    assert!(Uuid::parse_str(identity).is_ok());
}
