use linggan_evidence::DiscoveryLibraryProjection;

pub(super) fn render_read_projection(
    base: &str,
    projection: &DiscoveryLibraryProjection,
    query_text: Option<&str>,
) -> String {
    let window_label = window_label(projection.window);
    let input = format!(
        "<input name=\"q\" value=\"{}\" placeholder=\"仅检索 Linggan 已接纳的本地发现材料\" aria-label=\"本地 Evidence Library 搜索\">",
        escape(query_text.unwrap_or_default())
    );
    let results = if projection.cards.is_empty() {
        no_cards_markup(projection)
    } else {
        projection
            .cards
            .iter()
            .map(card_markup)
            .collect::<Vec<_>>()
            .join("")
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
        "<em>WINDOW:</em> UNKNOWN",
        &format!("<em>WINDOW:</em> {window_label}"),
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
        &format!("<b>LOCAL READ ONLY</b> · <span>WINDOW = PUBLISHED_AT / {window_label}</span>"),
    )
    .replace("NO ACCEPTED MATERIAL AVAILABLE", "ACCEPTED DISCOVERY CARDS")
}

fn window_label(window: &str) -> &'static str {
    match window {
        "last_7_days" => "7D",
        "last_30_days" => "30D",
        _ => panic!("read projection must supply a supported published-at window"),
    }
}

fn no_cards_markup(projection: &DiscoveryLibraryProjection) -> String {
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
    let source_time = card
        .published_at_source_text
        .as_deref()
        .unwrap_or("SOURCE TIME UNKNOWN");
    let coverage_detail = match (
        card.coverage_discovered_cards,
        card.coverage_emitted_cards,
        card.coverage_failed_cards,
        card.coverage_not_attempted_cards,
    ) {
        (Some(discovered), Some(emitted), Some(failed), Some(not_attempted)) => format!(
            "发现 {discovered} · 交付 {emitted} · 读取失败 {failed} · 当前页未读取 {not_attempted}"
        ),
        _ => "处理明细 UNKNOWN（该包早于当前 Coverage 合同）".to_owned(),
    };
    format!(
        "<article class=\"v7-discovery-row\"><input class=\"v7-check\" type=\"checkbox\" disabled aria-label=\"未启用选择\"><div class=\"v7-media-pending\" aria-label=\"MEDIA NOT ACQUIRED\">MEDIA<br>NOT ACQUIRED</div><div class=\"v7-discovery-main\"><div class=\"v7-discovery-title\">{}</div><div class=\"v7-discovery-meta\"><span>{}</span><span>XHS / {}</span><span>POSITION #{}</span></div><div class=\"v7-discovery-boundary\"><b>DISCOVERY ONLY</b>来源发布时间：{} · 已知发布时间：{} · {}</div></div><div class=\"v7-discovery-coverage\"><b>{}/{}</b><span>VISIBLE / QUOTA</span><small>{}</small></div></article>",
        escape(title),
        escape(creator),
        escape(&card.platform_content_id),
        card.result_position,
        escape(source_time),
        escape(&card.published_at),
        escape(&coverage_detail),
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
