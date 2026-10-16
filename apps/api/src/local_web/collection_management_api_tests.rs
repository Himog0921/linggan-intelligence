use super::*;

#[test]
fn content_workbench_domain_options_preserve_the_react_selector_contract() {
    let first = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
    let second = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
    let domains = vec![
        ObservationDomain {
            domain_ref: first,
            name: "亲子养育".to_owned(),
            description: Some("不应泄漏到选择器".to_owned()),
            research_goal: Some("也不应泄漏".to_owned()),
            status: "active".to_owned(),
            sample_count: Some(12),
            target_count: 4,
            primary_target_count: 3,
            reference_target_count: 1,
        },
        ObservationDomain {
            domain_ref: second,
            name: "职场成长".to_owned(),
            description: None,
            research_goal: None,
            status: "paused".to_owned(),
            sample_count: None,
            target_count: 0,
            primary_target_count: 0,
            reference_target_count: 0,
        },
    ];

    assert_eq!(
        content_workbench_domain_options(&domains),
        serde_json::json!({
            "items": [
                {"domainRef": first, "name": "亲子养育"},
                {"domainRef": second, "name": "职场成长"}
            ]
        })
    );
}
