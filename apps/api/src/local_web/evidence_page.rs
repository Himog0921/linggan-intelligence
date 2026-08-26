use linggan_evidence::DiscoveryLibraryProjection;

pub(super) fn render_read_projection(
    base: &str,
    projection: &DiscoveryLibraryProjection,
    query_text: Option<&str>,
) -> String {
    let time_view = time_view_copy(projection.time_view);
    let input = format!(
        "<input name=\"q\" value=\"{}\" placeholder=\"仅检索 Linggan 已接纳的本地发现材料\" aria-label=\"本地证据库检索\">",
        escape(query_text.unwrap_or_default())
    );
    let results = if projection.cards.is_empty() {
        no_cards_markup(projection)
    } else {
        format!(
            "{}{}",
            strict_window_exclusion_markup(projection),
            projection
                .cards
                .iter()
                .map(card_markup)
                .collect::<Vec<_>>()
                .join("")
        )
    };
    let card_count = projection.cards.len();
    let html = replace_slot(base, "EVIDENCE_SEARCH_INPUT", &input);
    let html = replace_slot(&html, "EVIDENCE_RESULTS", &results);
    let html = replace_slot(
        &html,
        "EVIDENCE_RESULTS_HEAD",
        "<div class=\"v7-results-head\"><div class=\"v7-results-left\"><input class=\"v7-check\" type=\"checkbox\" disabled aria-label=\"选择全部材料\"><span>已接纳的发现卡片 <span class=\"v7-tech-key\">ACCEPTED DISCOVERY</span></span></div><div>本机发现读取投影 <span class=\"v7-tech-key\">LOCAL DISCOVERY READ PROJECTION</span></div></div>",
    );
    let html = replace_slot(
        &html,
        "EVIDENCE_READ_STATUS",
        "<span class=\"v7-zh-status\">仅显示已接纳的发现材料</span><span class=\"v7-tech-key\">ACCEPTED DISCOVERY ONLY</span><br><span class=\"v7-zh-status\">不触发平台采集</span><span class=\"v7-tech-key\">LOCAL READ ONLY</span>",
    );
    let html = replace_slot(
        &html,
        "EVIDENCE_QUERY_LINE",
        &format!(
            "<div class=\"v7-query-line\"><div>只读取 Linggan 已接纳的发现卡片；不触发平台采集 <span class=\"v7-tech-key\">DISCOVERY ONLY</span></div><div><b>本机只读 <span class=\"v7-tech-key\">LOCAL READ ONLY</span></b> · <span>{}</span></div></div>",
            time_view.boundary_copy
        ),
    );
    let html = replace_slot(
        &html,
        "EVIDENCE_HEADER_BOUNDARY",
        "本机服务 / 已接纳发现材料 <span class=\"v7-tech-key\">LOCAL HOST / ACCEPTED DISCOVERY</span>",
    );
    let html = replace_slot(
        &html,
        "EVIDENCE_HEADER_META_STATE",
        "<span class=\"v7-query-meta\">已接纳发现材料 <span class=\"v7-tech-key\">ACCEPTED DISCOVERY</span></span>",
    );
    html.replace(
        "<span class=\"v7-kpi\"><em>内容</em><b>未知 <span class=\"v7-tech-key\">UNKNOWN</span></b></span>",
        &format!("<span class=\"v7-kpi\"><em>内容</em><b>{card_count}</b></span>"),
    )
    .replace(
        "<button class=\"v7-chip\" disabled><em>窗口</em> 未知 <span class=\"v7-tech-key\">UNKNOWN</span></button>",
        &format!(
            "<button class=\"v7-chip\" disabled><em>{}</em> <span class=\"v7-zh-value\">{}</span>{}</button>",
            time_view.filter_label, time_view.value_label, time_view.technical_key
        ),
    )
}

/// Renders an explicit read failure without re-labeling it as source incompleteness. A failed
/// local read or invalid local query tells us nothing about whether material exists upstream.
pub(super) fn render_read_unavailable(base: &str) -> String {
    render_read_failure(
        base,
        "本机发现材料读取暂时不可用",
        "READ_PROJECTION_UNAVAILABLE",
        "当前未读取任何材料；没有显示旧系统或远程数据。",
    )
}

/// Renders a local-query validation failure without treating it as an empty library or a source
/// coverage finding. The page read does not acquire, search, or supplement platform material.
pub(super) fn render_query_invalid(base: &str) -> String {
    render_read_failure(
        base,
        "当前查询参数无效",
        "LOCAL_QUERY_INVALID",
        "当前未读取任何材料，也未触发平台搜索或补采。",
    )
}

fn render_read_failure(
    base: &str,
    primary_copy: &str,
    technical_code: &str,
    detail: &str,
) -> String {
    let status = format!(
        "<span class=\"v7-zh-status\">{primary_copy}</span><span class=\"v7-tech-key\">{technical_code}</span><br><span class=\"v7-zh-status\">{detail}</span>"
    );
    let query_line = format!(
        "<div class=\"v7-query-line\"><div>{primary_copy} <span class=\"v7-tech-key\">{technical_code}</span></div><div><b>当前未读取材料 <span class=\"v7-tech-key\">NO MATERIAL READ</span></b> · <span>{detail}</span></div></div>"
    );
    let results_head = format!(
        "<div class=\"v7-results-head\"><div class=\"v7-results-left\"><input class=\"v7-check\" type=\"checkbox\" disabled aria-label=\"选择全部材料\"><span>当前未读取材料 <span class=\"v7-tech-key\">NO MATERIAL READ</span></span></div><div>{primary_copy} <span class=\"v7-tech-key\">{technical_code}</span></div></div>"
    );
    let results = format!(
        "<div class=\"v7-results-empty\"><section class=\"v7-empty-panel\"><h2>{primary_copy} <span class=\"v7-tech-key\">{technical_code}</span></h2><p>{detail}</p><dl class=\"v7-empty-grid\"><div><dt>材料读取 <span class=\"v7-tech-key\">NO MATERIAL READ</span></dt><dd>当前未读取任何材料；这不表示材料不存在或本地库为空。</dd></div><div><dt>未发生</dt><dd>没有显示旧系统或远程数据，也没有触发平台搜索或补采。</dd></div><div><dt>当前不能判断</dt><dd>不能据此判断采集是否失败、覆盖是否为零，或平台是否没有内容。</dd></div></dl></section></div>"
    );
    let html = replace_slot(base, "EVIDENCE_READ_STATUS", &status);
    let html = replace_slot(&html, "EVIDENCE_QUERY_LINE", &query_line);
    let html = replace_slot(&html, "EVIDENCE_RESULTS_HEAD", &results_head);
    let html = replace_slot(&html, "EVIDENCE_RESULTS", &results);
    let html = replace_slot(
        &html,
        "EVIDENCE_HEADER_BOUNDARY",
        &format!("本机服务 / {primary_copy} <span class=\"v7-tech-key\">{technical_code}</span>"),
    );
    let html = replace_slot(
        &html,
        "EVIDENCE_HEADER_META_STATE",
        &format!(
            "<span class=\"v7-query-meta\">{primary_copy} <span class=\"v7-tech-key\">{technical_code}</span></span><span>当前未读取材料，来源状态未知 <span class=\"v7-tech-key\">{technical_code}</span></span>"
        ),
    );
    html.replace(
        "来源信息尚未完整接通 <span class=\"v7-tech-key\">SOURCE INCOMPLETE</span>",
        &format!(
            "当前未读取材料，来源状态未知 <span class=\"v7-tech-key\">{technical_code}</span>"
        ),
    )
}

fn strict_window_exclusion_markup(projection: &DiscoveryLibraryProjection) -> String {
    if projection.time_view == "latest_accepted_discovery"
        || projection.excluded_unknown_published_at == 0
    {
        return String::new();
    }
    format!(
        "<div class=\"v7-window-exclusion\" role=\"note\"><b>发布时间未知 <span class=\"v7-tech-key\">PUBLISHED_AT UNKNOWN</span></b><span>当前查询有 {} 个对象因来源发布时间未知而未进入此发布时间窗口；未用首次发现、观察或接收时间替代。</span></div>",
        projection.excluded_unknown_published_at
    )
}

struct TimeViewCopy {
    filter_label: &'static str,
    value_label: &'static str,
    boundary_copy: &'static str,
    technical_key: &'static str,
}

fn time_view_copy(time_view: &str) -> TimeViewCopy {
    match time_view {
        "latest_accepted_discovery" => TimeViewCopy {
            filter_label: "视角",
            value_label: "最新已接纳",
            boundary_copy: "当前显示最新已接纳的发现卡片；其中部分卡片的发布时间仍可能未知。",
            technical_key: "<span class=\"v7-tech-key\">LATEST ACCEPTED DISCOVERY · PUBLISHED_AT UNKNOWN</span>",
        },
        "last_7_days" => TimeViewCopy {
            filter_label: "窗口",
            value_label: "近 7 天",
            boundary_copy: "只按来源可验证的发布时间筛选。",
            technical_key: "<span class=\"v7-tech-key\">PUBLISHED_AT / 7D</span>",
        },
        "last_30_days" => TimeViewCopy {
            filter_label: "窗口",
            value_label: "近 30 天",
            boundary_copy: "只按来源可验证的发布时间筛选。",
            technical_key: "<span class=\"v7-tech-key\">PUBLISHED_AT / 30D</span>",
        },
        _ => panic!("read projection must supply a supported time view"),
    }
}

fn no_cards_markup(projection: &DiscoveryLibraryProjection) -> String {
    if projection.time_view == "latest_accepted_discovery" {
        return "<div class=\"v7-results-empty\"><section class=\"v7-empty-panel\"><h2>当前视角没有可展示卡片</h2><p>本机发现读取投影已接通，但本次查询没有匹配的已接纳发现卡片。</p><dl class=\"v7-empty-grid\"><div><dt>当前视角 <span class=\"v7-tech-key\">VIEW</span></dt><dd>最新已接纳不是发布时间窗口。</dd></div><div><dt>发布时间 <span class=\"v7-tech-key\">PUBLISHED_AT</span></dt><dd>未知发布时间不会被替代为首次发现、观察或接收时间。</dd></div><div><dt>不代表</dt><dd>不代表平台没有内容、采集失败或世界没有讨论。</dd></div></dl></section></div>".to_owned();
    }
    format!(
        "<div class=\"v7-results-empty\"><section class=\"v7-empty-panel\"><h2>当前窗口没有可展示卡片</h2><p>本机发现读取投影已接通，但本次查询没有已知发布时间且落在当前窗口内的发现卡片。</p><dl class=\"v7-empty-grid\"><div><dt>发布时间窗口 <span class=\"v7-tech-key\">WINDOW</span></dt><dd>只按来源可知的发布时间过滤。</dd></div><div><dt>未知发布时间 <span class=\"v7-tech-key\">UNKNOWN TIME</span></dt><dd>当前查询候选中 {} 个对象因发布时间未知而未进入窗口。</dd></div><div><dt>不代表</dt><dd>不代表平台没有内容、采集失败或世界没有讨论。</dd></div></dl></section></div>",
        projection.excluded_unknown_published_at
    )
}

fn card_markup(card: &linggan_evidence::DiscoveryLibraryCard) -> String {
    let title = card.title.as_deref().unwrap_or("标题未知");
    let creator = card.creator_display_name.as_deref().unwrap_or("创作者未知");
    let publication_state = match (&card.published_at, card.published_at_state) {
        (Some(published_at), "KNOWN") => format!(
            "来源发布时间原文：{} · 归一化发布时间：{}",
            escape(card.published_at_source_text.as_deref().unwrap_or("来源时间原文未保留")),
            escape(published_at),
        ),
        (None, "UNKNOWN") => "<span class=\"v7-published-unknown\">发布时间未知 <span class=\"v7-tech-key\">PUBLISHED_AT UNKNOWN</span></span>来源页面未提供可验证发布时间；未用首次发现、观察或接收时间替代。".to_owned(),
        _ => panic!("published time state must agree with its value"),
    };
    let media = card.cover_local_asset_url.as_deref().map(|url| format!(
        "<img class=\"v7-media-local\" src=\"{}\" alt=\"Linggan 本地媒体副本\">", escape(url)
    )).unwrap_or_else(|| "<div class=\"v7-media-pending\" aria-label=\"媒体尚未采集（MEDIA NOT ACQUIRED）\">媒体<br>尚未采集<span class=\"v7-tech-key\">MEDIA NOT ACQUIRED</span></div>".to_owned());
    format!(
        "<article class=\"v7-discovery-row\"><input class=\"v7-check\" type=\"checkbox\" disabled aria-label=\"未启用选择\">{}<div class=\"v7-discovery-main\"><div class=\"v7-discovery-title\">{}</div><div class=\"v7-discovery-meta\"><span>{}</span><span>{} / {}</span><span>搜索位置 #{}</span><span class=\"v7-tech-key\">POSITION #{}</span></div><div class=\"v7-discovery-boundary\"><b>已接纳的本机发现材料 <span class=\"v7-tech-key\">ACCEPTED RUNTIME MATERIAL</span></b>{}</div></div><div class=\"v7-discovery-coverage\"><b>{}/{}</b><span>观察到 / 配额 <span class=\"v7-tech-key\">OBSERVED / QUOTA</span></span><small>{}</small></div></article>",
        media,
        escape(title),
        escape(creator),
        escape(&card.platform.to_uppercase()),
        escape(&card.platform_content_id),
        card.result_position,
        card.result_position,
        publication_state,
        card.coverage_visible_cards,
        card.coverage_maximum_quota,
        stopped_reason_markup(&card.coverage_stopped_reason),
    )
}

/// Keeps the persisted stop-reason code intact while making its present meaning readable without
/// requiring an operator to understand a producer enum. Unknown future codes deliberately remain
/// unknown instead of being guessed as a successful or complete collection outcome.
pub(super) fn stopped_reason_markup(raw_reason: &str) -> String {
    let meaning = match raw_reason {
        "quota_reached" | "maximum_quota" => "已达到本次配额",
        "surface_ended" => "当前页面内容已结束",
        "surface_read_complete" => "当前页面读取完成",
        "risk_control" => "平台风险控制导致停止",
        "manual_stop" => "用户手动停止",
        "unknown" => "停止原因未知",
        _ => "停止原因未归类",
    };

    format!(
        "停止原因：{meaning} <span class=\"v7-tech-key\">{}</span>",
        escape(raw_reason),
    )
}

fn replace_slot(base: &str, name: &str, replacement: &str) -> String {
    let start = format!("<!-- {name}_START -->");
    let end = format!("<!-- {name}_END -->");
    let before = base
        .split_once(&start)
        .expect("base page has a slot start")
        .0;
    let after = base.split_once(&end).expect("base page has a slot end").1;
    format!("{before}{start}{replacement}{end}{after}")
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
