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
fn legacy_redirects_keep_their_location_and_context_preserves_domain_and_error() {
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

fn document(body: &str) -> String {
    format!("<main class=\"c-page\">{BODY_START}{body}<!-- collection-body:end --></main>")
}

#[test]
fn every_existing_management_action_keeps_its_fields_and_gets_one_return_value() {
    let actions = [
        "/collection/runtime/stations",
        "/collection/runtime/stations/name",
        "/collection/runtime/claim-window",
        "/collection/runtime/close-window",
        "/collection/runtime/retire",
        "/collection/runtime/claims",
        "/collection/runtime/accepting",
        "/collection/runtime/account-bindings",
        "/collection/domains/save",
        "/collection/domains/target",
    ];
    let forms = actions.iter().map(|action| format!("<form method=\"post\" action=\"{action}\"><input name=\"station_ref\" value=\"station-1\"><button>保存</button></form>")).collect::<String>();
    let rendered = decorate(&document(&forms), Some("/collection/tasks?domain=all#r1"), None);
    assert_eq!(rendered.matches("name=\"return_to\"").count(), actions.len());
    assert_eq!(
        rendered.matches("name=\"station_ref\" value=\"station-1\"").count(),
        actions.len()
    );
    for action in actions {
        assert!(rendered.contains(&format!("action=\"{action}\"")));
    }
    assert!(rendered.contains("返回采集任务"));
}

#[test]
fn decoration_is_idempotent_and_a_replaced_error_draft_gets_context() {
    let source = document(
        "<form action='/collection/domains/save'><input name='name' value='草稿'></form>",
    );
    let first = decorate(&source, Some("/collection/tasks#r1"), None);
    assert_eq!(decorate(&first, Some("/collection/tasks#r1"), None), first);
    let with_draft = first.replace("<!-- collection-body:end -->", "<form action=\"/collection/domains/save\"><input name=\"name\" value=\"失败草稿\"></form><!-- collection-body:end -->");
    let second = decorate(&with_draft, Some("/collection/tasks#r1"), None);
    assert_eq!(second.matches(RETURN_MARKER).count(), 1);
    assert_eq!(second.matches("name=\"return_to\"").count(), 2);
    assert!(second.contains("value=\"失败草稿\""));
}

#[test]
fn only_management_navigation_is_rewritten_and_markup_is_escaped_once() {
    let source = document(r#"<a href="/collection/runtime?domain=all&amp;return_to=old">工位</a><a href='/collection/domains#settings'>领域</a><a href="/collection/targets?domain=other">该领域目标</a><a href="https://example.com">外部</a><form action="/collection/targets/new"><input name="name" value="目标"></form>"#);
    let rendered = decorate(&source, Some("/collection/tasks?domain=family&limit=20#r1"), None);
    assert!(rendered.contains("/collection/runtime?domain=all&amp;return_to=%2Fcollection%2Ftasks%3Fdomain%3Dfamily%26limit%3D20%23r1"));
    assert!(rendered.contains("/collection/domains?return_to=%2Fcollection%2Ftasks%3Fdomain%3Dfamily%26limit%3D20%23r1#settings"));
    assert!(rendered.contains("href=\"/collection/targets?domain=other\""));
    assert!(rendered.contains("href=\"https://example.com\""));
    assert!(!rendered.contains("name=\"return_to\""));
    assert!(!rendered.contains("&amp;amp;"));
    assert_eq!(
        decorate(
            &rendered,
            Some("/collection/tasks?domain=family&limit=20#r1"),
            None
        ),
        rendered
    );
}

#[test]
fn unreadable_runtime_keeps_its_original_failure_and_global_scope_explanation() {
    let source = format!(
        "<title>执行工位 · 采集</title>{}",
        document("<p>工位列表当前读不到。</p>")
    );
    let rendered = decorate(&source, None, Some("family"));
    assert!(rendered.contains("工位列表当前读不到。"));
    assert!(rendered.contains("工位是全局共享资源"));
    assert!(rendered.contains("href=\"/collection/targets?domain=family\""));
}
