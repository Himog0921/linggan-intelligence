//! Collection Workspace — the continuous observation execution layer.
//!
//! Five sub-surfaces, frozen in name and order by the V4 Gold Master:
//! Targets / Operations / Attention / Tasks / Runtime.
//!
//! Every surface here is structurally complete and factually empty. Collection's domain
//! objects (ObservationTarget, plan, capture task, attempt, observation event, worker)
//! do not exist in this project yet, and the only authorised platform access is the single
//! first canary spec. So each surface states what it cannot show and why, and never
//! substitutes a zero, a percentage, or a prototype figure for the missing fact.

use super::shell::{PrimarySurface, global_header};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Targets,
    Operations,
    Attention,
    Tasks,
    Runtime,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperationsMode {
    Now,
    Trace,
    Review,
}

impl OperationsMode {
    pub fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("trace") => Self::Trace,
            Some("review") => Self::Review,
            _ => Self::Now,
        }
    }

    fn slug(self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::Trace => "trace",
            Self::Review => "review",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Now => "NOW",
            Self::Trace => "TRACE",
            Self::Review => "REVIEW",
        }
    }
}

struct SectionMeta {
    slug: &'static str,
    index: &'static str,
    zh: &'static str,
    eyebrow: &'static str,
    title: &'static str,
}

const SECTIONS: [(Section, SectionMeta); 5] = [
    (
        Section::Targets,
        SectionMeta {
            slug: "targets",
            index: "01",
            zh: "观察目标",
            eyebrow: "COLLECTION / OBSERVATION TARGETS",
            title: "观察目标",
        },
    ),
    (
        Section::Operations,
        SectionMeta {
            slug: "operations",
            index: "02",
            zh: "运行态",
            eyebrow: "COLLECTION / OBSERVATION OPERATIONS",
            title: "运行态",
        },
    ),
    (
        Section::Attention,
        SectionMeta {
            slug: "attention",
            index: "03",
            zh: "待处理",
            eyebrow: "COLLECTION / ATTENTION QUEUE",
            title: "待处理",
        },
    ),
    (
        Section::Tasks,
        SectionMeta {
            slug: "tasks",
            index: "04",
            zh: "执行任务",
            eyebrow: "COLLECTION / EXECUTION TASKS",
            title: "执行任务",
        },
    ),
    (
        Section::Runtime,
        SectionMeta {
            slug: "runtime",
            index: "05",
            zh: "执行运行时",
            eyebrow: "COLLECTION / RUNTIME",
            title: "执行运行时",
        },
    ),
];

fn meta(section: Section) -> &'static SectionMeta {
    &SECTIONS
        .iter()
        .find(|(candidate, _)| *candidate == section)
        .expect("every section has metadata")
        .1
}

/// The rail. Unlike the Corpus rail, every entry here is a real connected route, so these
/// are links rather than disabled buttons.
fn rail(active: Section) -> String {
    let mut items = String::new();
    for (section, entry) in SECTIONS.iter() {
        let current = if *section == active {
            " aria-current=\"page\""
        } else {
            ""
        };
        items.push_str(&format!(
            "<a class=\"v7-side-nav\" href=\"/collection/{slug}\"{current}><i>{index}</i><span>{zh}</span></a>",
            slug = entry.slug,
            index = entry.index,
            zh = entry.zh,
        ));
    }
    format!(
        r#"<aside class="v7-side" aria-label="采集导航">
          <div class="v7-nav-label" data-readout="COLLECTION">采集与观察</div>
          {items}
          <div class="v7-side-foot"><span class="v7-side-dot"></span>NO OBSERVATION TARGETS<br><span class="v7-mono">no acquisition authorisation chain · scheduler not connected</span></div>
        </aside>"#
    )
}

/// Readouts carry only figures this surface owns. System-wide counters live in the context
/// row and are not repeated here — the prototype printed several of them three times.
fn readout(entries: &[(&str, &str)]) -> String {
    let mut cells = String::new();
    for (value, label) in entries {
        let unknown = if *value == "UNKNOWN" {
            " class=\"c-unknown\""
        } else {
            ""
        };
        cells.push_str(&format!(
            "<div{unknown}><b>{value}</b><span>{label}</span></div>"
        ));
    }
    format!("<div class=\"c-readout\">{cells}</div>")
}

fn empty_state(heading: &str, body: &str, notes: &[(&str, &str)]) -> String {
    let mut grid = String::new();
    for (term, description) in notes {
        grid.push_str(&format!("<div><dt>{term}</dt><dd>{description}</dd></div>"));
    }
    format!(
        r#"<section class="c-empty">
              <div class="c-empty-rule"></div>
              <h2>{heading}</h2>
              <p>{body}</p>
              <dl class="c-empty-grid">{grid}</dl>
              <div class="c-empty-foot"></div>
            </section>"#
    )
}

const AUTHORISATION_NOTE: &str = "创建观察目标会消耗真实平台访问。项目已冻结「申请 → 授权 → 准入 → 工单」四段分责，其中任何一段都还没有实现，因此这个动作现在不存在，而不是点了没反应。";

fn targets_body(drawer: Option<&str>) -> String {
    let empty = empty_state(
        "还没有观察目标",
        "这里将列出长期观察的创作者与关键词。当前没有任何观察目标，原因不是列表为空，而是建立观察目标所需的采集授权链尚未存在。",
        &[
            (
                "目标类型",
                "只有创作者与关键词两类。首次深度建档是每个目标都会经历的生命周期能力，不是第三种类型。",
            ),
            (
                "当前授权",
                "项目目前只批准了一次性的首个 canary：小红书 · 关键词 ADHD · 综合排序 · 最多 20 张实际可见搜索卡片。它不是持续观察授权。",
            ),
            (
                "不代表",
                "不代表平台上没有值得观察的对象，也不代表已有目标被删除或采集失败。",
            ),
        ],
    );
    format!("{empty}{}", drawer_markup(drawer))
}

/// The drawer opens from the URL, so a refresh or a shared link lands on the same object.
/// No target exists yet, so any identifier resolves to an honest not-found panel — the
/// shell geometry is real and verifiable, the content is not invented.
fn drawer_markup(drawer: Option<&str>) -> String {
    let Some(raw) = drawer else {
        return String::new();
    };
    let identifier = escape(raw);
    format!(
        r#"<aside class="c-drawer" id="c-drawer" aria-label="观察目标工作区">
          <div class="c-drawer-head">
            <div class="c-drawer-top">
              <div>
                <div class="c-drawer-kind">TARGET WORKSPACE</div>
                <div class="c-drawer-name">未找到该观察目标</div>
                <div class="c-drawer-id">#{identifier}</div>
              </div>
              <div class="c-drawer-controls">
                <button class="c-btn-quiet" id="c-drawer-wide" type="button">WIDE ↔</button>
                <a class="c-btn-quiet" href="/collection/targets">CLOSE ×</a>
              </div>
            </div>
            <div class="c-drawer-tabs" role="tablist">
              <button type="button" class="c-on" data-panel="overview">概览</button>
              <button type="button" data-panel="baseline">建档基线</button>
              <button type="button" data-panel="patrol">巡逻策略</button>
              <button type="button" data-panel="evidence">证据</button>
              <button type="button" data-panel="trace">观察史</button>
            </div>
          </div>
          <div class="c-drawer-body">
            <div class="c-drawer-panel" data-panel="overview">
              {overview}
            </div>
            <div class="c-drawer-panel" data-panel="baseline" hidden>{baseline}</div>
            <div class="c-drawer-panel" data-panel="patrol" hidden>{patrol}</div>
            <div class="c-drawer-panel" data-panel="evidence" hidden>{evidence}</div>
            <div class="c-drawer-panel" data-panel="trace" hidden>{trace}</div>
          </div>
        </aside>"#,
        overview = drawer_panel_overview(),
        baseline = drawer_panel_baseline(),
        patrol = drawer_panel_patrol(),
        evidence = drawer_panel_evidence(),
        trace = drawer_panel_trace(),
    )
}

fn drawer_panel_overview() -> String {
    empty_state(
        "这个标识没有对应的观察目标",
        "抽屉按地址栏里的标识打开，因此刷新和分享都会回到同一个对象。当前系统里还没有任何观察目标，所以这个标识无法解析。",
        &[
            (
                "创作者概览",
                "将显示身份读数与作品生命周期分布：正常、超基线、爆发三档加滚动中位数。",
            ),
            (
                "关键词概览",
                "将显示搜索环境读数与结果景观，并始终声明它是观察样本、不是平台全量世界。",
            ),
            ("不代表", "不代表该对象在平台上不存在，也不代表它曾被删除。"),
        ],
    )
}

fn drawer_panel_baseline() -> String {
    empty_state(
        "没有建档基线",
        "首次深度建档记录的是「第一次到底观察了什么、边界在哪里」。没有目标，也就没有基线。",
        &[
            (
                "创作者",
                "主页探查 → 作品清单 → 身份解析 → 详情 → 评论 → 媒体 → 转录。",
            ),
            (
                "关键词",
                "搜索环境探查 → 生成条件 → 结果身份 → 详情与评论 → 作者观察。",
            ),
            (
                "生成条件",
                "关键词基线必须保存 query、排序、窗口、观察时刻、结果深度与捕获条件。",
            ),
        ],
    )
}

fn drawer_panel_patrol() -> String {
    empty_state(
        "没有巡逻策略",
        "巡逻策略回答「为什么这个对象被这样观察」。策略变更会扩大来源、深度或成本时必须形成新版本并重新检查授权，这套版本化机制尚未实现。",
        &[
            (
                "创作者默认",
                "新作品轻量扫描；相对历史与速度触发自适应观察；历史锚点低频复访。",
            ),
            (
                "关键词默认",
                "同一查询环境对比基线；爆发自适应加深；旧内容重回高位记为变化。",
            ),
            (
                "候选",
                "作者候选与新词候选只能被提议，永远不自动升级为长期观察目标。",
            ),
        ],
    )
}

fn drawer_panel_evidence() -> String {
    empty_state(
        "没有关联证据",
        "证据面优先显示原始事实，不用 AI 摘要替换原声。当前没有目标，也没有任何已接纳材料与之关联。",
        &[
            ("来源", "证据由全局事实层持有，采集侧不再造第二份原文。"),
            ("已接纳材料", "现有的已接纳材料在语料 / 证据库中查看。"),
            ("不代表", "不代表相关材料不存在。"),
        ],
    )
}

fn drawer_panel_trace() -> String {
    empty_state(
        "没有观察史",
        "观察史是这一个对象的历史，既不是运行态的实时流，也不是执行运行时的技术日志。",
        &[
            ("包含", "发现、变化、状态与异常这类有业务含义的事件。"),
            ("不包含", "心跳、租约、选择器重试、HTTP 状态、堆栈。"),
            ("当前", "没有对象，因此没有事件。"),
        ],
    )
}

fn operations_body(mode: OperationsMode) -> String {
    match mode {
        OperationsMode::Now => {
            let stages = [
                ("01", "目标接入", "Target Intake"),
                ("02", "首次建档", "Baseline"),
                ("03", "日常巡逻", "Patrol"),
                ("04", "触发式深采", "Event Deepening"),
                ("05", "事实资产保留", "Evidence Retain"),
                ("06", "恢复与重排", "Recovery"),
            ];
            let mut flow = String::new();
            for (no, zh, en) in stages {
                flow.push_str(&format!(
                    r#"<div class="c-flow-stage">
                    <div class="c-flow-no">{no}</div>
                    <div class="c-flow-name"><b>{zh}</b><span>{en}</span></div>
                    <div class="c-flow-metrics"><div><b>UNKNOWN</b><span>阶段计数</span></div></div>
                  </div>"#
                ));
            }
            format!(
                r#"<div class="c-now">
              <div class="c-now-main">
                <div class="c-conclusion">
                  <div class="c-conclusion-label">系统结论 / SYSTEM CONCLUSION</div>
                  <div class="c-conclusion-state">UNKNOWN</div>
                  <p>调度器未接通，没有「最近一轮观察」可供判断。这里将来只显示能说明时间窗、样本与判定规则的结论，不显示无出处的系统判断。</p>
                </div>
                <div class="c-flow"><p class="c-flow-note">进入、处理中、完成与异常四项计数各自独立，将在调度接通后分别显示。当前每个阶段都还没有可判断的运行，因此统一为未知，而不是零。</p>{flow}</div>
              </div>
              {stream}
            </div>"#,
                stream = stream_markup(),
            )
        }
        OperationsMode::Trace => empty_state(
            "没有可回放的观察历史",
            "观察轨迹是可检索、可回放的语义历史，与右侧实时流的区别在于时间跨度，不在于内容层级。两者都不承载技术日志。",
            &[
                ("事件类型", "观察、发现、变化、状态、异常。"),
                (
                    "为什么是空的",
                    "还没有任何观察目标产生过事件；调度器也未接通。",
                ),
                ("不代表", "不代表历史被清空。"),
            ],
        ),
        OperationsMode::Review => empty_state(
            "没有可复盘的周期",
            "周期复盘回答过去一段时间观察了多少、发现了什么、哪些变化重要，以及最要紧的一件事——观察体系哪里还有盲区。",
            &[
                ("重要变化", "需要跨时间可比的观察记录，目前不存在。"),
                (
                    "观察盲区",
                    "哪些地方我们还不能声称「看到了世界」。这需要真实的捕获面记录来支撑。",
                ),
                ("不代表", "不代表这段时间没有变化发生。"),
            ],
        ),
    }
}

/// The one dark surface in the product. Kept from the V4 Gold Master on Mog's decision of
/// 2026-08-26 and recorded as a long-term LIDS exception. It shows that nothing is arriving
/// rather than inventing events on a timer, and its pause control would only ever pause the
/// view — never the real scheduler.
fn stream_markup() -> String {
    r#"<aside class="c-stream" aria-label="实时观察流">
              <div class="c-stream-toolbar">
                <div class="c-stream-id"><span class="c-stream-dot"></span><b>LIVE OBSERVATION</b></div>
                <div class="c-stream-filters">
                  <button type="button" class="c-on" disabled aria-disabled="true">ALL</button>
                  <button type="button" disabled aria-disabled="true">DISCOVER</button>
                  <button type="button" disabled aria-disabled="true">CHANGE</button>
                  <button type="button" disabled aria-disabled="true">EXCEPTION</button>
                </div>
              </div>
              <div class="c-stream-feed">
                <div class="c-stream-empty">
                  <b>NOT CONNECTED</b>
                  <p>没有事件到达。调度器未接通，也没有任何观察目标会产生事件。这里不会用计时器伪造事件来证明系统在运行。</p>
                  <dl>
                    <div><dt>会出现什么</dt><dd>发现、变化、状态与异常这类有业务含义的观察事件，低层事件先聚合再出现。</dd></div>
                    <div><dt>不会出现什么</dt><dd>心跳、租约、选择器重试、HTTP 状态与堆栈；它们只属于执行运行时。</dd></div>
                    <div><dt>暂停的含义</dt><dd>暂停只停止画面跟随，永远不会暂停真实的采集调度。</dd></div>
                  </dl>
                </div>
              </div>
              <div class="c-stream-readout">
                <div><b>UNKNOWN</b><span>EVENTS / 1H</span></div>
                <div><b>UNKNOWN</b><span>RETAINED</span></div>
                <div><b>UNKNOWN</b><span>EXCEPTIONS</span></div>
              </div>
              <div class="c-stream-foot"><span>SEMANTIC EVENTS ONLY</span><span>SCHEDULER NOT CONNECTED</span></div>
            </aside>"#
        .to_owned()
}

fn attention_body() -> String {
    empty_state(
        "没有待处理事项",
        "这里只保留真正需要人介入的问题。当前没有事项，是因为还没有任何观察在运行——不是因为已确认零故障。",
        &[
            (
                "会包含什么",
                "巡逻中断、建档不完整、执行运行时故障、输出数据问题。每一条都必须说明影响边界与已保留的证据。",
            ),
            (
                "为什么不是 0",
                "0 只用于已确认为零的数值。当前没有可判断的运行，因此是未知，不是零。",
            ),
            ("不代表", "不代表系统健康，也不代表没有数据丢失。"),
        ],
    )
}

fn tasks_body() -> String {
    empty_state(
        "没有执行任务",
        "任务是纯执行视角：一次具体的采集目标。观察对象是长期身份，任务只是它的某一次执行，两者不能互相冒充。",
        &[
            (
                "任务预算不是覆盖率",
                "预算 100、观察到 67，说明的是这次执行的边界，不能写成覆盖率 67%。",
            ),
            (
                "部分完成仍然有效",
                "任务部分完成而数据可用，是一等状态；已保留的材料不因任务未完成而隐藏或归零。",
            ),
            ("当前", "没有观察目标，也就没有任何任务被创建。"),
        ],
    )
}

fn runtime_body() -> String {
    empty_state(
        "执行运行时未接通",
        "这是唯一允许出现工程执行细节的页面：执行工位、队列、租约、心跳与回执。这些细节不会反向进入观察目标与观察史。",
        &[
            (
                "当前状态",
                "调度器未接通。插件侧已有「任务规格 → 尝试 → 浏览器待发件箱 → 本地回执」这条链，但它目前只服务受控的手动 discovery。",
            ),
            (
                "这一页给谁看",
                "排查故障时使用。日常判断「哪里出了问题、影响了哪些观察对象」应该看待处理。",
            ),
            ("不代表", "不代表执行工位全部离线，也不代表队列为空。"),
        ],
    )
}

fn body(section: Section, mode: OperationsMode, drawer: Option<&str>) -> String {
    match section {
        Section::Targets => targets_body(drawer),
        Section::Operations => operations_body(mode),
        Section::Attention => attention_body(),
        Section::Tasks => tasks_body(),
        Section::Runtime => runtime_body(),
    }
}

fn head_readout(section: Section) -> String {
    match section {
        Section::Targets => readout(&[("UNKNOWN", "TARGETS"), ("UNKNOWN", "BASELINING")]),
        Section::Operations => readout(&[("UNKNOWN", "RUNNING"), ("UNKNOWN", "RETAINED / 24H")]),
        Section::Attention => readout(&[("UNKNOWN", "OPEN"), ("UNKNOWN", "DATA LOSS")]),
        Section::Tasks => readout(&[("UNKNOWN", "ACTIVE"), ("UNKNOWN", "PARTIAL")]),
        Section::Runtime => readout(&[("UNKNOWN", "WORKERS"), ("UNKNOWN", "QUEUE")]),
    }
}

/// Targets and Operations carry their own second bar. Filter tabs stay disabled: with no
/// targets there is nothing to filter, and pretending otherwise would be a fake control.
fn second_bar(section: Section, mode: OperationsMode) -> String {
    match section {
        Section::Targets => format!(
            r#"<div class="c-toolbar">
          <div class="c-tabs">
            <button type="button" class="c-on" disabled aria-disabled="true">全部 <small>UNKNOWN</small></button>
            <button type="button" disabled aria-disabled="true">创作者 <small>UNKNOWN</small></button>
            <button type="button" disabled aria-disabled="true">关键词 <small>UNKNOWN</small></button>
            <button type="button" disabled aria-disabled="true">建档中 <small>UNKNOWN</small></button>
            <button type="button" disabled aria-disabled="true">巡逻中 <small>UNKNOWN</small></button>
          </div>
          <div class="c-actions">
            <span class="c-gate" title="{note}">需要采集授权</span>
            <button class="c-btn" type="button" disabled aria-disabled="true">＋ 新建观察目标</button>
          </div>
        </div>"#,
            note = AUTHORISATION_NOTE,
        ),
        Section::Operations => {
            let mut tabs = String::new();
            for candidate in [
                OperationsMode::Now,
                OperationsMode::Trace,
                OperationsMode::Review,
            ] {
                let current = if candidate == mode {
                    " aria-current=\"page\""
                } else {
                    ""
                };
                tabs.push_str(&format!(
                    "<a href=\"/collection/operations?mode={slug}\"{current}>{label}</a>",
                    slug = candidate.slug(),
                    label = candidate.label(),
                ));
            }
            format!(
                r#"<div class="c-modebar">
              <div class="c-tabs">{tabs}</div>
              <div class="c-mode-meta">SCHEDULER NOT CONNECTED</div>
            </div>"#
            )
        }
        Section::Attention | Section::Tasks | Section::Runtime => String::new(),
    }
}

fn crumb(section: Section, mode: OperationsMode) -> String {
    let entry = meta(section);
    let tail = if section == Section::Operations {
        format!(
            " <span class=\"v7-slash\">/</span> <span class=\"v7-context-current\">{}</span>",
            mode.label()
        )
    } else {
        String::new()
    };
    format!(
        "采集 <span class=\"v7-slash\">/</span> <b>{zh}</b>{tail}",
        zh = entry.zh,
    )
}

pub fn render(section: Section, mode: OperationsMode, drawer: Option<&str>) -> String {
    let entry = meta(section);
    let header = global_header(
        PrimarySurface::Collection,
        "LOCAL HOST / NO COLLECTION RUNTIME",
        &crumb(section, mode),
        "<span class=\"v7-query-meta\">SCHEDULER NOT CONNECTED</span><span>NO OBSERVATION TARGETS</span><span>UTC+08</span>",
    );
    format!(
        r#"<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>{title} · 采集 · Linggan Intelligence</title>
    <link rel="stylesheet" href="/assets/collection-workspace.css">
  </head>
  <body>
    <div class="v7-app">
      {header}
      <div class="v7-shell">
        {rail}
        <main class="c-page" aria-label="{title}">
          <div class="c-head">
            <div>
              <div class="c-eyebrow">{eyebrow}</div>
              <h1 class="c-title">{title}</h1>
            </div>
            {readout}
          </div>
          {second_bar}
          <div class="c-body">{body}</div>
        </main>
      </div>
    </div>
    <script src="/assets/collection-workspace.js"></script>
  </body>
</html>
"#,
        title = entry.title,
        eyebrow = entry.eyebrow,
        rail = rail(section),
        readout = head_readout(section),
        second_bar = second_bar(section, mode),
        body = body(section, mode, drawer),
    )
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}
