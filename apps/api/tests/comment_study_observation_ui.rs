#[test]
fn comment_study_overview_exposes_observation_chart_table_and_ranges() {
    let page = include_str!("../src/local_web/comment_study.html");

    assert!(page.contains("id=\"study-observation-title\">评论观察量"));
    for days in [7, 28, 56] {
        assert!(
            page.contains(&format!("data-series-days=\"{days}\"")),
            "missing observation range {days}"
        );
    }
    assert!(page.contains("data-series-view=\"chart\""));
    assert!(page.contains("data-series-view=\"table\""));
    for heading in [
        "首次观察评论",
        "进入研究评论",
        "接纳信号",
        "覆盖作品",
        "观察覆盖",
    ] {
        assert!(page.contains(heading), "missing observation field {heading}");
    }
}

#[test]
fn comment_observation_copy_keeps_missing_coverage_distinct_from_no_user_voice() {
    let page = include_str!("../src/local_web/comment_study.html");

    assert!(page.contains("没有观察记录不等于用户没有表达"));
    assert!(page.contains("有观察记录"));
    assert!(page.contains("部分观察"));
    assert!(page.contains("无观察记录"));
    assert!(!page.contains("需求趋势"));
}

#[test]
fn observation_range_switches_are_read_only_and_do_not_fake_date_drilldown() {
    let page = include_str!("../src/local_web/comment_study.html");

    assert!(page.contains("data-series-days"));
    assert!(page.contains("data-series-view"));
    assert!(!page.contains("data-observation-date"));
    assert!(!page.contains("post('observation"));
}
