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
    readout: &'static str,
    surface: Option<PrimarySurface>,
}

/// The five names are product responsibilities, not five authorised runtime instances.
/// Every one of them stays disabled until its route is actually connected.
const PRIMARY_ENTRIES: [PrimaryEntry; 5] = [
    PrimaryEntry {
        zh: "雷达",
        readout: "RADAR · —",
        surface: None,
    },
    PrimaryEntry {
        zh: "主题图谱",
        readout: "TOPIC MAP · —",
        surface: None,
    },
    PrimaryEntry {
        zh: "语料",
        readout: "CORPUS · UNKNOWN",
        surface: Some(PrimarySurface::Corpus),
    },
    PrimaryEntry {
        zh: "洞察",
        readout: "INSIGHTS · —",
        surface: None,
    },
    PrimaryEntry {
        zh: "采集",
        readout: "COLLECTION · —",
        surface: Some(PrimarySurface::Collection),
    },
];

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
        let current = if entry.surface == Some(active) {
            " aria-current=\"page\""
        } else {
            ""
        };
        if !nav.is_empty() {
            nav.push_str("\n            ");
        }
        nav.push_str(&format!(
            "<button disabled aria-disabled=\"true\"{current}><b class=\"v7-nav-zh\">{zh}</b><span class=\"v7-nav-readout\">{readout}</span></button>",
            zh = entry.zh,
            readout = entry.readout,
        ));
    }

    format!(
        r#"<header class="v7-global-header">
        <div class="v7-global-row">
          <div class="v7-global-brand" aria-label="Linggan Intelligence">
            <div class="v7-li-mark">LI</div>
            <div class="v7-global-brand-copy"><div class="v7-global-brand-name">Linggan Intelligence</div><div class="v7-global-brand-sub">EDITORIAL INTELLIGENCE TERMINAL</div></div>
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
