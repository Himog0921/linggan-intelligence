//! The single global intelligence header. Both `/corpus/evidence` and `/collection/*`
//! render it from here so the product never grows a second header implementation.

/// Which first-level product responsibility the current page belongs to.
/// Only one may be `aria-current` at a time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PrimarySurface {
    Corpus,
    Collection,
}

struct PrimaryEntry {
    zh: &'static str,
    technical_key: &'static str,
    state: &'static str,
    surface: Option<PrimarySurface>,
    /// The connected entry route. `None` means the responsibility has no served route yet,
    /// so the entry stays a disabled button rather than becoming a dead link.
    href: Option<&'static str>,
}

/// The five names are product responsibilities, not five authorised runtime instances.
/// An entry only becomes navigable once `href` names a route this binary actually serves;
/// every other one stays disabled.
const PRIMARY_ENTRIES: [PrimaryEntry; 5] = [
    PrimaryEntry {
        zh: "雷达",
        technical_key: "RADAR",
        state: "尚未接通",
        surface: None,
        href: None,
    },
    PrimaryEntry {
        zh: "主题图谱",
        technical_key: "TOPIC MAP",
        state: "尚未接通",
        surface: None,
        href: None,
    },
    PrimaryEntry {
        zh: "语料",
        technical_key: "CORPUS",
        state: "未知",
        surface: Some(PrimarySurface::Corpus),
        href: Some("/corpus"),
    },
    PrimaryEntry {
        zh: "洞察",
        technical_key: "INSIGHTS",
        state: "尚未接通",
        surface: None,
        href: None,
    },
    PrimaryEntry {
        zh: "采集",
        technical_key: "COLLECTION",
        state: "尚未接通",
        surface: Some(PrimarySurface::Collection),
        href: Some("/collection"),
    },
];

/// Renders one primary entry. A connected route becomes a real link so an operator can move
/// between served surfaces; a responsibility with no served route stays a disabled button
/// rather than a dead link. Both forms keep the same label markup and `aria-current`.
fn primary_entry_markup(entry: &PrimaryEntry, active: PrimarySurface) -> String {
    let current = if entry.surface == Some(active) {
        " aria-current=\"page\""
    } else {
        ""
    };
    let label = format!(
        "<b class=\"v7-nav-zh\">{zh}</b><span class=\"v7-nav-readout\"><small class=\"v7-tech-key\">{technical_key}</small><span class=\"v7-nav-state\">{state}</span></span>",
        zh = entry.zh,
        technical_key = entry.technical_key,
        state = entry.state,
    );

    match entry.href {
        Some(href) => format!("<a href=\"{href}\"{current}>{label}</a>"),
        None => format!("<button disabled aria-disabled=\"true\"{current}>{label}</button>"),
    }
}

/// The local web host keeps Chinese as the user-facing semantic layer. The original
/// static runtime codes survive as smaller technical annotations, so no state, timestamp,
/// route, query, or action meaning is silently changed by a display-only migration.
fn chinese_first(zh: &str, technical_key: &str) -> String {
    format!(
        "<span class=\"v7-status-main\">{zh}</span><small class=\"v7-tech-key\">{technical_key}</small>"
    )
}

fn localize_context_markup(markup: &str) -> String {
    let mut localized = markup.to_owned();

    // Full codes must precede short tokens. The markup is authored only by the two
    // first-party renderers that use this shell; this helper never sees source material.
    for (technical_key, zh) in [
        ("SCHEDULER NOT CONNECTED", "调度器未接通"),
        ("READ MODEL NOT CONNECTED", "读模型未接通"),
        ("NO OBSERVATION TARGETS", "暂无观察目标"),
        ("SOURCE INCOMPLETE", "来源信息不完整"),
        ("UTC+08", "中国标准时间"),
        ("UNKNOWN", "未知"),
    ] {
        localized = localized.replace(technical_key, &chinese_first(zh, technical_key));
    }

    localized
}

fn localize_boundary_label(label: &str) -> String {
    let chinese = match label {
        "LOCAL HOST / NO READ MODEL" => "本机服务 / 读模型未接通",
        "LOCAL HOST / ACCEPTED DISCOVERY" => "本机服务 / 已接纳发现",
        "LOCAL HOST / NO COLLECTION RUNTIME" => "本机服务 / 采集运行时未接通",
        _ => "本机服务状态",
    };

    chinese_first(chinese, label)
}

/// Renders the 128px global header: the 78px global row plus the 50px context row.
///
/// `crumb` and `meta` are already-escaped markup owned by the calling page. The context
/// row's first column is intentionally empty: it continues the local rail's width so the
/// instrument mesh reads as one vertical field.
pub fn global_header(
    active: PrimarySurface,
    boundary_label: &str,
    crumb: &str,
    meta: &str,
) -> String {
    let mut nav = String::new();
    for entry in PRIMARY_ENTRIES {
        if !nav.is_empty() {
            nav.push_str("\n            ");
        }
        nav.push_str(&primary_entry_markup(&entry, active));
    }

    let boundary_label = localize_boundary_label(boundary_label);
    let crumb = localize_context_markup(crumb);
    let meta = localize_context_markup(meta);

    format!(
        r#"<header class="v7-global-header">
        <div class="v7-global-row">
          <div class="v7-global-brand" aria-label="Linggan Intelligence">
            <div class="v7-li-mark">LI</div>
            <div class="v7-global-brand-copy"><div class="v7-global-brand-name">Linggan Intelligence</div><div class="v7-global-brand-sub"><span class="v7-brand-sub-zh">领域情报工作台</span><small class="v7-tech-key">EDITORIAL INTELLIGENCE</small></div></div>
          </div>
          <nav class="v7-primary-nav" aria-label="一级导航">
            {nav}
          </nav>
          <div class="v7-global-flex" aria-hidden="true"></div>
          <div class="v7-global-system">
            <div class="v7-system-boundary">{boundary_label}</div>
            <button class="v7-global-command" disabled aria-disabled="true"><span>&gt; 输入命令</span><kbd>/</kbd></button>
          </div>
        </div>
        <div class="v7-context-row">
          <div aria-hidden="true"></div>
          <div class="v7-context-main">
            <div class="v7-context-crumb">{crumb}</div>
            <div class="v7-context-meta">{meta}</div>
          </div>
        </div>
      </header>"#
    )
}

#[cfg(test)]
mod tests {
    use super::{PrimarySurface, global_header};

    #[test]
    fn shared_shell_gives_chinese_the_primary_meaning_on_both_served_surfaces() {
        let corpus = global_header(
            PrimarySurface::Corpus,
            "LOCAL HOST / NO READ MODEL",
            "语料 <span class=\"v7-slash\">/</span> <b>证据库</b>",
            "<span class=\"v7-kpi\"><em>内容</em><b>UNKNOWN</b></span><span>READ MODEL NOT CONNECTED</span><span>SOURCE INCOMPLETE</span><span>UTC+08</span>",
        );
        let collection = global_header(
            PrimarySurface::Collection,
            "LOCAL HOST / NO COLLECTION RUNTIME",
            "采集 <span class=\"v7-slash\">/</span> <b>生产流</b> <span class=\"v7-context-current\">NOW</span>",
            "<span class=\"v7-kpi\"><em>巡逻中断</em><b>UNKNOWN</b></span><span>SCHEDULER NOT CONNECTED</span><span>NO OBSERVATION TARGETS</span><span>UTC+08</span>",
        );

        for (name, header, required_chinese) in [
            (
                "corpus",
                &corpus,
                ["读模型未接通", "来源信息不完整", "中国标准时间"],
            ),
            (
                "collection",
                &collection,
                ["调度器未接通", "暂无观察目标", "中国标准时间"],
            ),
        ] {
            for chinese in required_chinese {
                assert!(
                    header.contains(chinese),
                    "{name} shell must show Chinese primary meaning: {chinese}"
                );
            }
            assert!(
                header.contains("v7-tech-key"),
                "{name} shell must keep technical annotations subordinate"
            );
            assert!(
                header.contains("领域情报工作台"),
                "{name} shell must give the brand descriptor a Chinese primary label"
            );
        }

        for (code, chinese) in [
            ("RADAR", "雷达"),
            ("TOPIC MAP", "主题图谱"),
            ("CORPUS", "语料"),
            ("INSIGHTS", "洞察"),
            ("COLLECTION", "采集"),
            ("UNKNOWN", "未知"),
            ("UTC+08", "中国标准时间"),
        ] {
            assert!(
                corpus.contains(code),
                "technical annotation missing: {code}"
            );
            assert!(
                corpus.contains(chinese),
                "Chinese primary label missing: {chinese}"
            );
        }
    }
}
