use super::*;

#[test]
fn exact_daily_routes_keep_the_original_query_and_fragment() {
    for location in [
        "/collection/targets",
        "/collection/tasks?domain=all&limit=20#task-7",
        "/collection/targets?domain=family&filter=creator&q=%E5%AD%A6%E4%B9%A0%20%E6%96%B9%E6%B3%95#target-1",
    ] {
        assert_eq!(
            ManagementContext::new(Some(location), None).return_to(),
            location
        );
    }
}

#[test]
fn foreign_pseudo_paths_controls_and_unbounded_locations_are_rejected() {
    for location in [
        "https://example.com/collection/tasks",
        "//example.com/collection/tasks",
        "/\\example.com",
        "/collection/tasks/../runtime",
        "/collection/tasks-extra",
        "/collection/tasks;path=x",
        "/collection/%74asks",
        "/collection/tasks%2f../runtime",
        " /collection/tasks",
        "/collection/tasks?x=\\evil",
        "/collection/tasks?x=%5c",
        "/collection/tasks?x=%0d%0aLocation:evil",
        "/collection/tasks?x=%C2%80",
        "/collection/tasks?x=%FF",
        "/collection/tasks#\n",
        "/collection/tasks?x=%GG",
        "/collection/tasks?x=%",
    ] {
        assert_eq!(valid_return_to(Some(location)), None, "{location:?}");
    }
    let long = format!("/collection/tasks?q={}", "a".repeat(MAX_RETURN_BYTES));
    assert_eq!(valid_return_to(Some(&long)), None);
}

#[test]
fn missing_or_rejected_context_has_a_domain_aware_daily_fallback() {
    assert_eq!(
        ManagementContext::new(None, Some("all")).return_to(),
        "/collection/targets?domain=all"
    );
    assert_eq!(
        ManagementContext::new(Some("//evil"), Some("family")).return_to(),
        "/collection/targets?domain=family"
    );
    assert_eq!(
        ManagementContext::new(None, None).return_to(),
        "/collection/targets"
    );
}

#[test]
fn navigation_preserves_location_context_domain_and_error() {
    let base = "/collection/domains?domain=family#settings";
    assert_eq!(redirect_path(base, None, None), base);
    assert_eq!(redirect_path(base, None, Some("https://evil")), base);
    assert_eq!(
        redirect_path(
            base,
            Some("save_rejected"),
            Some("/collection/tasks?domain=all#task-7")
        ),
        "/collection/domains?domain=family&error=save_rejected&return_to=%2Fcollection%2Ftasks%3Fdomain%3Dall%23task-7#settings"
    );
}
