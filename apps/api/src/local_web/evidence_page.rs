use linggan_evidence::DiscoveryLibraryProjection;

pub(super) fn render_read_projection(
    base: &str,
    projection: &DiscoveryLibraryProjection,
    query_text: Option<&str>,
) -> String {
    let time_view = time_view_copy(projection.time_view);
    let input = format!(
        "<input name=\"q\" value=\"{}\" placeholder=\"仅检索 Linggan 已接纳的本地发现材料\" aria-label=\"本地 Evidence Library 搜索\">",
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
    html.replace(
        "READ MODEL NOT CONNECTED",
        "LOCAL DISCOVERY READ PROJECTION",
    )
    .replace(
        "LOCAL HOST / NO READ MODEL",
        "LOCAL HOST / ACCEPTED DISCOVERY",
    )
    .replace("SOURCE INCOMPLETE", "ACCEPTED DISCOVERY ONLY")
    .replace(
        "<b>UNKNOWN</b></div><div class=\"v7-stat\"><span>02 / COMMENTS",
        &format!("<b>{card_count}</b></div><div class=\"v7-stat\"><span>02 / COMMENTS"),
    )
    .replace(
        "<button class=\"v7-chip\" disabled><em>窗口</em> UNKNOWN</button>",
        &format!(
            "<button class=\"v7-chip\" disabled><em>{}</em> {}</button>",
            time_view.filter_label, time_view.value_label
        ),
    )
    .replace(
        "页面结构已就绪 · 材料读投影尚未接通",
        "只读取 Linggan 已接纳的 discovery 卡片；不触发平台采集",
    )
    .replace(
        "local presentation only · no material read",
        "local accepted discovery only · no platform read",
    )
    .replace(
        "<b>SOURCE_INCOMPLETE</b> · <span>NO QUERY AVAILABLE</span>",
        &format!(
            "<b>LOCAL READ ONLY</b> · <span>{}</span>",
            time_view.boundary_copy
        ),
    )
    .replace("NO ACCEPTED MATERIAL AVAILABLE", "ACCEPTED DISCOVERY CARDS")
}

fn strict_window_exclusion_markup(projection: &DiscoveryLibraryProjection) -> String {
    if projection.time_view == "latest_accepted_discovery"
        || projection.excluded_unknown_published_at == 0
    {
        return String::new();
    }
    format!(
        "<div class=\"v7-window-exclusion\" role=\"note\"><b>PUBLISHED_AT UNKNOWN</b><span>当前查询有 {} 个对象因来源发布时间未知而未进入此发布时间窗口；未用首次发现、观察或接收时间替代。</span></div>",
        projection.excluded_unknown_published_at
    )
}

struct TimeViewCopy {
    filter_label: &'static str,
    value_label: &'static str,
    boundary_copy: &'static str,
}

fn time_view_copy(time_view: &str) -> TimeViewCopy {
    match time_view {
        "latest_accepted_discovery" => TimeViewCopy {
            filter_label: "视角",
            value_label: "最新已接纳",
            boundary_copy: "VIEW = LATEST ACCEPTED DISCOVERY / PUBLISHED_AT MAY BE UNKNOWN",
        },
        "last_7_days" => TimeViewCopy {
            filter_label: "窗口",
            value_label: "7D",
            boundary_copy: "WINDOW = PUBLISHED_AT / 7D",
        },
        "last_30_days" => TimeViewCopy {
            filter_label: "窗口",
            value_label: "30D",
            boundary_copy: "WINDOW = PUBLISHED_AT / 30D",
        },
        _ => panic!("read projection must supply a supported time view"),
    }
}

fn no_cards_markup(projection: &DiscoveryLibraryProjection) -> String {
    if projection.time_view == "latest_accepted_discovery" {
        return "<div class=\"v7-results-empty\"><section class=\"v7-empty-panel\"><h2>当前视角没有可展示卡片</h2><p>本地 read projection 已接通，但本次查询没有匹配的已接纳 Discovery 卡片。</p><dl class=\"v7-empty-grid\"><div><dt>VIEW</dt><dd>最新已接纳不是发布时间窗口。</dd></div><div><dt>PUBLISHED_AT</dt><dd>未知发布时间不会被替代为首次发现、观察或接收时间。</dd></div><div><dt>不代表</dt><dd>不代表平台没有内容、采集失败或世界没有讨论。</dd></div></dl></section></div>".to_owned();
    }
    format!(
        "<div class=\"v7-results-empty\"><section class=\"v7-empty-panel\"><h2>当前窗口没有可展示卡片</h2><p>本地 read projection 已接通，但本次查询没有已知发布时间且落在当前窗口内的 Discovery 卡片。</p><dl class=\"v7-empty-grid\"><div><dt>WINDOW</dt><dd>只按来源可知的 published_at 过滤。</dd></div><div><dt>UNKNOWN TIME</dt><dd>当前查询候选中 {} 个对象因发布时间未知而未进入窗口。</dd></div><div><dt>不代表</dt><dd>不代表平台没有内容、采集失败或世界没有讨论。</dd></div></dl></section></div>",
        projection.excluded_unknown_published_at
    )
}

fn card_markup(card: &linggan_evidence::DiscoveryLibraryCard) -> String {
    let title = card.title.as_deref().unwrap_or("TITLE UNKNOWN");
    let creator = card
        .creator_display_name
        .as_deref()
        .unwrap_or("CREATOR UNKNOWN");
    let publication_state = match (&card.published_at, card.published_at_state) {
        (Some(published_at), "KNOWN") => format!(
            "来源发布时间：{} · 已知发布时间：{}",
            escape(card.published_at_source_text.as_deref().unwrap_or("SOURCE TEXT NOT RETAINED")),
            escape(published_at),
        ),
        (None, "UNKNOWN") => "<span class=\"v7-published-unknown\">PUBLISHED_AT UNKNOWN</span>来源页面未提供可验证发布时间；未用首次发现、观察或接收时间替代。".to_owned(),
        _ => panic!("published time state must agree with its value"),
    };
    let media = card.cover_local_asset_url.as_deref().map(|url| format!(
        "<img class=\"v7-media-local\" src=\"{}\" alt=\"Linggan 本地媒体副本\">", escape(url)
    )).unwrap_or_else(|| "<div class=\"v7-media-pending\" aria-label=\"MEDIA NOT ACQUIRED\">MEDIA<br>NOT ACQUIRED</div>".to_owned());
    format!(
        "<article class=\"v7-discovery-row\"><input class=\"v7-check\" type=\"checkbox\" disabled aria-label=\"未启用选择\">{}<div class=\"v7-discovery-main\"><div class=\"v7-discovery-title\">{}</div><div class=\"v7-discovery-meta\"><span>{}</span><span>{} / {}</span><span>POSITION #{}</span></div><div class=\"v7-discovery-boundary\"><b>ACCEPTED RUNTIME MATERIAL</b>{}</div></div><div class=\"v7-discovery-coverage\"><b>{}/{}</b><span>OBSERVED / QUOTA</span><small>{}</small></div></article>",
        media,
        escape(title),
        escape(creator),
        escape(&card.platform.to_uppercase()),
        escape(&card.platform_content_id),
        card.result_position,
        publication_state,
        card.coverage_visible_cards,
        card.coverage_maximum_quota,
        escape(&card.coverage_stopped_reason),
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
