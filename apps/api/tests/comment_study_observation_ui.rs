#[test]
fn comment_study_overview_exposes_observation_chart_table_and_ranges() {
    let page = include_str!("../src/local_web/comment_study.html");
    let script = include_str!("../src/local_web/comment_study.js");
    assert!(page.contains("<script src=\"/assets/comment-study.js\"></script>"));

    // 三个范围按钮由模板生成（源码里是 data-series-days="${days}" 配 [7,28,56] 的取值集合），
    // 写死的 data-series-days="7" 本来就不会出现在文件里，断言要对着模板写。
    let compact: String = script
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();

    assert!(script.contains("id=\\\"study-observation-title\\\" class=\\\"v7-sr-only\\\""));
    assert!(
        compact.contains("data-series-days=\"${days}\""),
        "观察量必须提供 7 / 28 / 56 三个尾窗范围按钮"
    );
    assert!(
        compact.contains("[7,28,56]"),
        "范围集合固定为尾部 7 / 28 / 56 天"
    );
    assert!(script.contains("data-series-view=\"chart\""));
    assert!(script.contains("data-series-view=\"table\""));
    for heading in [
        "首次观察评论",
        "进入研究评论",
        "接纳信号",
        "覆盖作品",
        "观察覆盖",
    ] {
        assert!(
            script.contains(heading),
            "missing observation field {heading}"
        );
    }
}

#[test]
fn comment_observation_copy_keeps_missing_coverage_distinct_from_no_user_voice() {
    let script = include_str!("../src/local_web/comment_study.js");

    assert!(script.contains("没有观察记录不等于用户没有表达"));
    assert!(script.contains("有观察记录"));
    assert!(script.contains("部分观察"));
    assert!(script.contains("无观察记录"));
    assert!(!script.contains("需求趋势"));
}

#[test]
fn observation_range_switches_are_read_only_and_do_not_fake_date_drilldown() {
    let script = include_str!("../src/local_web/comment_study.js");

    assert!(script.contains("data-series-days"));
    assert!(script.contains("data-series-view"));
    assert!(!script.contains("data-observation-date"));
    assert!(!script.contains("post('observation"));
}

#[test]
fn borderless_overview_reuses_real_observation_series_and_keeps_lower_sections() {
    let script = include_str!("../src/local_web/comment_study.js");
    let css = include_str!("../src/local_web/comment_study.css");
    let read = include_str!("../../../crates/intelligence/src/comment_study_read.rs");
    assert!(script.contains("study-overview-hero"));
    assert!(script.contains("study-metric-pair"));
    assert!(script.contains("study-metric-delta"));
    assert!(script.contains("昨日首次观察"));
    assert!(script.contains("昨日进入"));
    assert!(script.contains("昨日写入"));
    assert!(script.contains("昨日建档"));
    assert!(script.contains("yesterdayNewProblemCount"));
    assert!(script.contains("newObservedCommentCount"));
    assert!(script.contains("studiedCommentCount"));
    assert!(script.contains("acceptedSignalCount"));
    assert!(css.contains("COMMENT-OVERVIEW-V2-428"));
    assert!(css.contains(".study-series-coverage-tick"));
    assert!(css.contains(".study-series-signal-bar"));
    assert!(read.contains("\"yesterdayNewProblemCount\":yesterday_new_problem_count"));
    assert!(read.contains("AT TIME ZONE 'Asia/Shanghai'"));
    for heading in ["近期获得新依据的问题","研究与归并进度","用户提到的办法与经历","原声预览","尚未看清的部分"] {
        assert!(script.contains(heading));
    }
}

#[test]
fn observation_coverage_is_attached_to_signal_axis_not_separate_row() {
    let script = include_str!("../src/local_web/comment_study.js");
    assert!(script.contains("covY=signalBase+8"));
    assert!(script.contains("class=\"study-series-coverage-tick\""));
    assert!(script.contains("无观察记录不等于用户没有表达"));
    assert!(script.contains("id=\"study-observation-title\" class=\"v7-sr-only\""));
}
