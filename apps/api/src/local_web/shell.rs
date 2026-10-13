//! The single global intelligence header. Both `/corpus/evidence` and `/collection/*`
//! render it from here so the product never grows a second header implementation.

/// Which first-level product responsibility the current page belongs to.
/// Only one may be `aria-current` at a time.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PrimarySurface {
    Corpus,
    Collection,
    Topic,
    Settings,
    Ops,
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

/// The six names are product responsibilities, not six authorised runtime instances.
/// An entry only becomes navigable once `href` names a route this binary actually serves;
/// every other one stays disabled.
const PRIMARY_ENTRIES: [PrimaryEntry; 6] = [
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
        state: "暂定研究",
        surface: Some(PrimarySurface::Topic),
        href: Some("/topics"),
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
        // 这个状态词曾经写死为「尚未接通」。采集接通之后它一个字没变，于是全站页头
        // 都在说一句已经不成立的话。现在由调用方按真实事实覆盖，见 `global_header`。
        technical_key: "COLLECTION",
        state: "状态未知",
        surface: Some(PrimarySurface::Collection),
        href: Some("/collection"),
    },
    // OPS-MIGRATION-001 D2：Mog 2026-10-13 定为最后一个一级入口。新入口按 LANG-05 不再带英文
    // 代号；辅助行只有一个状态词：切换（D6）之前这里只能看。
    PrimaryEntry {
        zh: "运营",
        technical_key: "",
        state: "只读",
        surface: Some(PrimarySurface::Ops),
        href: Some("/ops"),
    },
];

/// Renders one primary entry. A connected route becomes a real link so an operator can move
/// between served surfaces; a responsibility with no served route stays a disabled button
/// rather than a dead link. Both forms keep the same label markup and `aria-current`.
fn primary_entry_markup(
    entry: &PrimaryEntry,
    active: PrimarySurface,
    collection_state: Option<&str>,
) -> String {
    let current = if entry.surface == Some(active) {
        " aria-current=\"page\""
    } else {
        ""
    };
    // 一个责任的状态词只有它自己的页面读得到真实事实。读得到时用真的，读不到时保留
    // 明确的未知——**绝不允许因为读不到就宣布「已接通」「未接通」或「为空」**。
    let state = match (entry.surface, collection_state) {
        (Some(PrimarySurface::Collection), Some(state)) => state,
        _ => entry.state,
    };
    let technical_key = if entry.technical_key.is_empty() {
        String::new()
    } else {
        format!(
            "<small class=\"v7-tech-key\">{}</small>",
            entry.technical_key
        )
    };
    let label = format!(
        "<b class=\"v7-nav-zh\">{zh}</b><span class=\"v7-nav-readout\">{technical_key}<span class=\"v7-nav-state\">{state}</span></span>",
        zh = entry.zh,
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
    for (technical_key, zh) in CONTEXT_CODES {
        localized = localized.replace(technical_key, &chinese_first(zh, technical_key));
    }

    localized
}

/// 上下文行里允许出现的技术码及其中文主表达。
const CONTEXT_CODES: [(&str, &str); 13] = [
    // 一条码不得包含另一条码。顺序在这里救不了：先替换长码会生成含短码的 markup，
    // 后一轮再替换一次，得到嵌套的 `调度心跳读不到 SCHEDULER HEARTBEAT 未知 UNKNOWN`。
    // 由 `no_code_contains_another_code` 自动执行，不靠人记得。
    ("SCHEDULER HEARTBEAT UNREADABLE", "调度心跳读不到"),
    ("SCHEDULER NOT CONNECTED", "调度器未接通"),
    ("SCHEDULER RUNNING", "调度运行中"),
    ("SCHEDULER STALE", "调度心跳已过期"),
    ("READ MODEL NOT CONNECTED", "读模型未接通"),
    ("COLLECTION STATE UNAVAILABLE", "采集状态读不到"),
    ("NO OBSERVATION TARGETS", "暂无观察目标"),
    ("PATROL ARMED", "巡检已开启"),
    ("PATROL OFF", "有观察目标但巡检未开"),
    ("SOURCE INCOMPLETE", "来源信息不完整"),
    ("SOURCE UNREADABLE", "来源读不到"),
    ("UTC+08", "中国标准时间"),
    ("UNKNOWN", "未知"),
];

fn localize_boundary_label(label: &str) -> String {
    match label {
        "LOCAL HOST / NO READ MODEL" => "读模型未接通".to_owned(),
        "LOCAL HOST / ACCEPTED DISCOVERY" => "已接纳发现".to_owned(),
        "LOCAL HOST / NO COLLECTION RUNTIME" => "采集运行时未接通".to_owned(),
        "LOCAL HOST / COLLECTION STATE UNKNOWN" => "采集状态未知".to_owned(),
        "LOCAL HOST / NO PLATFORM ACCESS" => "本机不访问平台".to_owned(),
        _ => "本机服务状态".to_owned(),
    }
}

/// Renders the 128px global header: the 78px global row plus the 50px context row.
///
/// `crumb` and `meta` are already-escaped markup owned by the calling page. The context
/// row's first column is intentionally empty: it continues the local rail's width so the
/// instrument mesh reads as one vertical field.
/// `collection_state` 让采集页用真实事实覆盖一级导航里那个静态状态词。
/// 传 `None` 保留“状态未知”——读不到事实时不许宣布已接通或未接通。
pub fn global_header(
    active: PrimarySurface,
    boundary_label: &str,
    crumb: &str,
    meta: &str,
    collection_state: Option<&str>,
) -> String {
    let mut nav = String::new();
    for entry in PRIMARY_ENTRIES {
        if !nav.is_empty() {
            nav.push_str("\n            ");
        }
        nav.push_str(&primary_entry_markup(&entry, active, collection_state));
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
            <details class="v7-personal-menu"><summary class="v7-global-command">个人菜单</summary><nav aria-label="个人菜单"><a href="/settings">设置</a></nav></details>
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

/// 语料模块的二级导航。
///
/// 当前领域由这里统一带上。领域由整个语料模块承载，任何一个子页都无权在自己的链接里
/// 把它丢掉——此前证据库与评论研究各写一份侧栏，评论研究那份不带领域，于是「选了外部
/// 领域、点一下评论研究就掉回本领域」。导航只有一处生成，这类漏写才不会随着下一个子页
/// 再发生一次。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CorpusPage {
    Creators,
    Evidence,
    Comments,
}

struct CorpusEntry {
    ordinal: &'static str,
    label: &'static str,
    /// `None` 表示这项还没有真正提供的路由，保持禁用而不是变成死链。
    href: Option<&'static str>,
    page: Option<CorpusPage>,
}

const CORPUS_ENTRIES: [CorpusEntry; 3] = [
    CorpusEntry {
        ordinal: "01",
        label: "证据库",
        href: Some("/corpus/evidence"),
        page: Some(CorpusPage::Evidence),
    },
    CorpusEntry {
        ordinal: "02",
        label: "评论研究",
        href: Some("/corpus/comments"),
        page: Some(CorpusPage::Comments),
    },
    CorpusEntry {
        ordinal: "03",
        label: "创作者",
        href: Some("/corpus/creators"),
        page: Some(CorpusPage::Creators),
    },
];

/// `domain_ref` 传 `None` 时链接保持裸路径（尚未选择领域时不能传入虚假的读取范围）。
/// 传了就每一条都带上：本领域同样带，
/// 这样导航行为不因当前是哪个领域而不同。
pub fn corpus_side_nav(active: CorpusPage, domain_ref: Option<&str>, foot: &str) -> String {
    let mut items = String::new();
    for entry in CORPUS_ENTRIES {
        if !items.is_empty() {
            items.push('\n');
        }
        let badge = if matches!(entry.page, Some(CorpusPage::Evidence)) {
            r#"<b class="ev-rail-count" id="ev-rail-count" hidden></b>"#
        } else {
            ""
        };
        match entry.href {
            None => items.push_str(&format!(
                r#"<span class="v7-side-nav" aria-disabled="true"><i>{ordinal}</i><span>{label}</span></span>"#,
                ordinal = entry.ordinal,
                label = entry.label,
            )),
            Some(href) => {
                let href = match domain_ref {
                    Some(domain_ref) => format!("{href}?domain={domain_ref}"),
                    None => href.to_owned(),
                };
                let current = if entry.page == Some(active) {
                    r#" aria-current="page""#
                } else {
                    ""
                };
                items.push_str(&format!(
                    r#"<a class="v7-side-nav" href="{href}"{current}><i>{ordinal}</i><span>{label}</span>{badge}</a>"#,
                    ordinal = entry.ordinal,
                    label = entry.label,
                ));
            }
        }
    }
    format!(
        r#"<aside class="v7-side" aria-label="语料导航">
{items}
<div class="v7-side-foot">{foot}</div></aside>"#
    )
}

/// 运营模块的二级导航（OPS-MIGRATION-001 D2）。写法与语料侧栏一致；业务选择由这里统一带上，
/// 切页时不丢。总览自 D3 起开放。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OpsPage {
    Overview,
    Publications,
    Accounts,
    Daily,
    ImportExport,
}

const OPS_ENTRIES: [(&str, &str, Option<&str>, Option<OpsPage>); 5] = [
    ("01", "总览", Some("/ops/overview"), Some(OpsPage::Overview)),
    (
        "02",
        "发布记录",
        Some("/ops/publications"),
        Some(OpsPage::Publications),
    ),
    (
        "03",
        "账号管理",
        Some("/ops/accounts"),
        Some(OpsPage::Accounts),
    ),
    ("04", "每日数据", Some("/ops/daily"), Some(OpsPage::Daily)),
    (
        "05",
        "导入导出",
        Some("/ops/io"),
        Some(OpsPage::ImportExport),
    ),
];

/// `business` 是已校验的业务选择（`all` 或领域编号）；`foot` 是调用方已转义的 markup。
pub fn ops_side_nav(active: OpsPage, business: Option<&str>, foot: &str) -> String {
    let mut items = String::new();
    for (ordinal, label, href, page) in OPS_ENTRIES {
        if !items.is_empty() {
            items.push('\n');
        }
        match href {
            None => items.push_str(&format!(
                r#"<span class="v7-side-nav" aria-disabled="true"><i>{ordinal}</i><span>{label}</span></span>"#,
            )),
            Some(href) => {
                let href = match business {
                    Some(business) => format!("{href}?domain={business}"),
                    None => href.to_owned(),
                };
                let current = if page == Some(active) {
                    r#" aria-current="page""#
                } else {
                    ""
                };
                items.push_str(&format!(
                    r#"<a class="v7-side-nav" href="{href}"{current}><i>{ordinal}</i><span>{label}</span></a>"#,
                ));
            }
        }
    }
    format!(
        r#"<aside class="v7-side" aria-label="运营导航">
{items}
<div class="v7-side-foot">{foot}</div></aside>"#
    )
}

#[cfg(test)]
mod code_table_tests {
    use super::CONTEXT_CODES;

    #[test]
    fn no_code_contains_another_code() {
        // 替换是逐条顺序执行的：一条码若包含另一条，先替换的那条会生成含短码的 markup，
        // 后一轮把它再替换一次，产出嵌套的技术键。这不是排序能解决的，只能不允许包含。
        for (outer, _) in CONTEXT_CODES {
            for (inner, _) in CONTEXT_CODES {
                if outer != inner {
                    assert!(
                        !outer.contains(inner),
                        "「{outer}」包含「{inner}」，第二轮替换会嵌套技术键"
                    );
                }
            }
        }
    }
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
            None,
        );
        let collection = global_header(
            PrimarySurface::Collection,
            "LOCAL HOST / NO COLLECTION RUNTIME",
            "采集 <span class=\"v7-slash\">/</span> <b>生产流</b> <span class=\"v7-context-current\">NOW</span>",
            "<span class=\"v7-kpi\"><em>巡逻中断</em><b>UNKNOWN</b></span><span>SCHEDULER NOT CONNECTED</span><span>NO OBSERVATION TARGETS</span><span>UTC+08</span>",
            None,
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
