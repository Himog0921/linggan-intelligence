//! Collection Workspace — the continuous observation execution layer.
//!
//! Five sub-surfaces. The V4 Gold Master froze the set; DESIGN-006 reordered and renamed
//! them so the row reads by urgency rather than by pipeline stage:
//! 待处理 / 观察目标 / 生产流 / 采集任务 / 执行工位.
//! The slugs behind them are unchanged and remain the URL contract.
//!
//! Every surface here is structurally complete and factually empty. Collection's domain
//! objects (ObservationTarget, plan, capture task, attempt, observation event, worker)
//! do not exist in this project yet, and the only authorised platform access is the single
//! first canary spec. So each surface states what it cannot show and why, and never
//! substitutes a zero, a percentage, or a prototype figure for the missing fact.

use super::shell::{PrimarySurface, global_header};
use linggan_evidence::TargetCounts;

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
            Self::Now => "当前",
            Self::Trace => "追溯",
            Self::Review => "复核",
        }
    }
}

struct SectionMeta {
    slug: &'static str,
    index: &'static str,
    zh: &'static str,
    title: &'static str,
}

/// DESIGN-006 · order follows urgency, not the pipeline's own stages. Attention is the only
/// surface whose contents expire: the other four read the same today and next week. The
/// Collection Control pattern already required `NEEDS ATTENTION` first; the pipeline order
/// this list used to carry was tidy but never the order anyone reads it in.
///
/// The names avoid sharing a word stem. The three that used to (运行态 / 执行任务 /
/// 执行运行时) were page labels, not domain terms — `domain-language.md` defines none of
/// them, so they were free to change. It does define 工位, which is why 执行工位 keeps it.
///
/// Slugs are untouched: they are the URL contract the browser plugin and bookmarks hold.
const SECTIONS: [(Section, SectionMeta); 5] = [
    (
        Section::Attention,
        SectionMeta {
            slug: "attention",
            index: "01",
            zh: "待处理",
            title: "待处理",
        },
    ),
    (
        Section::Targets,
        SectionMeta {
            slug: "targets",
            index: "02",
            zh: "观察目标",
            title: "观察目标",
        },
    ),
    (
        Section::Operations,
        SectionMeta {
            slug: "operations",
            index: "03",
            zh: "生产流",
            title: "生产流",
        },
    ),
    (
        Section::Tasks,
        SectionMeta {
            slug: "tasks",
            index: "04",
            zh: "采集任务",
            title: "采集任务",
        },
    ),
    (
        Section::Runtime,
        SectionMeta {
            slug: "runtime",
            index: "05",
            zh: "执行工位",
            title: "执行工位",
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
fn rail(active: Section, state: Option<&SurfaceState>) -> String {
    // 导轨底部此前写死 `NO OBSERVATION TARGETS · scheduler not connected`，两句都已不成立。
    let foot = match state {
        Some(state) if state.total_targets > 0 => format!(
            "观察目标 {total}<br><span class=\"v7-mono\">巡检开着 {monitoring} · 调度心跳读不到</span>",
            total = state.total_targets,
            monitoring = state.monitoring_targets,
        ),
        Some(_) => "暂无观察目标<br><span class=\"v7-mono\">没有可巡检的对象 · 调度心跳读不到</span>".to_owned(),
        None => "NO OBSERVATION TARGETS<br><span class=\"v7-mono\">no acquisition authorisation chain · scheduler not connected</span>".to_owned(),
    };
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
          <div class="v7-side-foot"><span class="v7-side-dot"></span>{foot}</div>
        </aside>"#
    )
}

/// Readouts carry only figures this surface owns — the prototype printed several of the
/// system-wide ones three times over. Under the DESIGN-003 header reclaim they no longer
/// get a title block of their own: they join the system state in the context row. Each
/// reading stays a separate `.v7-kpi` and they are never summed, because two unknowns do
/// not add up to a known total.
fn readout(entries: &[(&str, &str)]) -> String {
    let mut cells = String::new();
    for (value, label) in entries {
        cells.push_str(&format!(
            "<span class=\"v7-kpi\"><em>{label}</em><b>{value}</b></span>"
        ));
    }
    cells
}

/// DESIGN-006 · Collection has five empty surfaces but only three kinds of empty, and only
/// one of them is the reader's to act on. Rendering all five at equal weight produced five
/// honest reports that together answered everything except "so what do I do".
#[derive(Clone, Copy)]
enum Empty<'a> {
    /// The chain is stopped on a decision only a person can make. Carries that one action.
    AwaitingYou { action: &'a str },
    /// Nothing here until something is connected. Reading it changes nothing, and saying so
    /// is more useful than letting the reader hunt for an action that does not exist.
    AwaitingEngineering { note: &'a str },
    /// Empty only because its upstream is. The shortest of the three: it states the fact and
    /// points at whatever is actually stopped, instead of re-deriving the whole chain.
    Upstream {
        because: &'a str,
        pointer: &'a str,
        href: &'a str,
    },
    /// Not one of the three. Local emptiness inside a drawer panel, where the surface-level
    /// question "is this mine to act on" has already been answered by the surface around it.
    Plain,
}

fn empty_state(kind: Empty<'_>, heading: &str, body: &str, notes: &[(&str, &str)]) -> String {
    if let Empty::Upstream {
        because,
        pointer,
        href,
    } = kind
    {
        return format!(
            r#"<section class="c-empty c-empty-upstream">
              <div class="c-empty-rule"></div>
              <h2>{heading}</h2>
              <p>{because}</p>
              <a class="c-empty-pointer" href="{href}">{pointer} <i aria-hidden="true">→</i></a>
            </section>"#
        );
    }

    let mut grid = String::new();
    for (term, description) in notes {
        grid.push_str(&format!("<div><dt>{term}</dt><dd>{description}</dd></div>"));
    }
    let (variant, tail) = match kind {
        Empty::AwaitingYou { action } => (
            " c-empty-you",
            format!(r#"<p class="c-empty-action"><b>等你</b>{action}</p>"#),
        ),
        Empty::AwaitingEngineering { note } => (
            " c-empty-engineering",
            format!(r#"<p class="c-empty-note">{note}</p>"#),
        ),
        Empty::Plain => ("", String::new()),
        Empty::Upstream { .. } => unreachable!("handled above"),
    };

    format!(
        r#"<section class="c-empty{variant}">
              <div class="c-empty-rule"></div>
              <h2>{heading}</h2>
              <p>{body}</p>
              <dl class="c-empty-grid">{grid}</dl>
              {tail}
              <div class="c-empty-foot"></div>
            </section>"#
    )
}

fn targets_body(drawer: Option<&str>) -> String {
    let empty = empty_state(
        // The only surface in Collection whose emptiness has a human in front of it.
        Empty::AwaitingYou {
            action: "：用上方的「加入观察」把第一个创作者或关键词放进来。创作者直接粘主页链接即可，平台 ID 会自动认出来。",
        },
        "还没有观察目标",
        "这里将列出长期观察的创作者与关键词。加入观察只写本机记录，不访问任何平台，也不会让任何采集开始——两者是分开的两步。",
        &[
            (
                "目标类型",
                "只有创作者与关键词两类。首次深度建档是每个目标都会经历的生命周期能力，不是第三种类型。",
            ),
            (
                "加进来之后",
                "目标先停在「待决」。真正开始采集要另走一遍申请 → 授权 → 准入 → 工单 → 租约，最后还要人开闸——加入观察本身不消耗任何平台访问。",
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
        Empty::Plain,
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
        Empty::Plain,
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
        Empty::Plain,
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
        Empty::Plain,
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
        Empty::Plain,
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
            // DESIGN-006 · the six stage names are the domain model and carry information;
            // a counter reading UNKNOWN six times over carries none. The stage counts do not
            // exist yet — the scheduler is not connected — so the cells are not rendered
            // rather than filled with a placeholder. Nothing is hidden: the reason sits in
            // the context row as SCHEDULER NOT CONNECTED, and the note below says so again
            // once. Six repetitions of UNKNOWN would only teach the eye to skip UNKNOWN,
            // which is the one word here that must never become invisible.
            let mut flow = String::new();
            for (no, zh, en) in stages {
                flow.push_str(&format!(
                    r#"<div class="c-flow-stage">
                    <div class="c-flow-no">{no}</div>
                    <div class="c-flow-name"><b>{zh}</b><span>{en}</span></div>
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
                <div class="c-flow"><p class="c-flow-note">下面是观察生产的六个阶段，顺序固定。每个阶段的进入、处理中、完成与异常四项计数各自独立，将在调度接通后显示——现在还没有任何一次运行可供计数，所以这里不放数字，而不是放一个零或六个未知。</p>{flow}</div>
              </div>
              {stream}
            </div>"#,
                stream = stream_markup(),
            )
        }
        OperationsMode::Trace => empty_state(
            Empty::AwaitingEngineering {
                note: "这一栏不需要你做任何事：历史要等调度接通、产生过观察之后才会有内容。",
            },
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
            Empty::AwaitingEngineering {
                note: "这一栏不需要你做任何事：复盘需要跨时间可比的观察记录，那要等真实观察积累起来。",
            },
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
              <div class="c-stream-foot"><span>SEMANTIC EVENTS ONLY</span><span>SCHEDULER NOT CONNECTED</span></div>
            </aside>"#
        .to_owned()
}

/// The default landing surface. It is empty because nothing upstream is running yet, so it
/// says that in one line and points at the step that is actually stopped — a reader who
/// arrives here should leave knowing where the chain broke, not having read three columns
/// about a queue that cannot have contents.
fn attention_body() -> String {
    empty_state(
        Empty::Upstream {
            because: "没有需要你处理的事。这不是「已确认零故障」——而是还没有任何观察在运行，因此还不可能产生需要处理的问题。真正卡住的是上一环：",
            pointer: "观察目标 · 采集授权链尚未建立",
            href: "/collection/targets",
        },
        "没有待处理事项",
        "",
        &[],
    )
}

fn tasks_body() -> String {
    empty_state(
        Empty::Upstream {
            because: "没有采集任务。任务是一次具体执行，只能由观察目标产生——当前没有观察目标，因此不可能有任务。真正卡住的是上一环：",
            pointer: "观察目标 · 采集授权链尚未建立",
            href: "/collection/targets",
        },
        "没有采集任务",
        "",
        &[],
    )
}

fn runtime_body() -> String {
    empty_state(
        Empty::AwaitingEngineering {
            note: "这一栏不需要你做任何事：调度器接通是工程实现，不是等你决定。",
        },
        "执行工位未接通",
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

/// 上下文行、系统边界与一级导航状态词要说的真话。
///
/// 这四处此前是写死的字符串：`调度器未接通`、`暂无观察目标`、`采集 尚未接通`、
/// `本机服务 / 采集运行时未接通`。写下时都成立，之后采集接通而字符串一个字没变。
/// **一个不会随系统状态改变的状态词，等于一个永远不会响的警报器。**
///
/// 只有执行工位页读得到这些事实，因此只有它传 `Some`。其余子面继续用静态词——
/// 读不到事实时保留原话，绝不因为读不到就宣布已接通。
pub struct SurfaceState {
    /// 已登记但当前没有在岗插件的工位数。DESIGN-006：每个面至少有一个读数回答
    /// 「这里有没有出问题」，因为没人因为「有 30 台」而行动，只因为「3 台停了」而行动。
    pub vacant_stations: i64,
    /// 报到了却没归位的插件安装数。它们不会被派活。
    pub unclaimed_installations: i64,
    pub total_targets: i64,
    pub monitoring_targets: i64,
}

/// Labels are Chinese because `.v7-kpi em` is the Sans reading slot the Corpus surface set
/// the standard for. An English label here renders at 11px Sans beside the 9px Mono English
/// system words in the same row, so two English labels end up at two different weights and
/// sizes. `PARTIAL` keeps its English form wherever it means the LIDS validity state; this
/// count is the number of partially completed tasks, which is why it can read in Chinese.
/// DESIGN-006 · at least one reading per surface answers "is anything wrong here", because
/// a total never makes anyone act: nobody moves because there are 30 targets, they move
/// because 3 stopped. This is the Collection Control pattern's own Failed-first rule applied
/// to the context row. Attention needs no change — it is already nothing but exceptions.
///
/// 部分完成 stays alongside 失败 rather than folded into it: a partially completed task whose
/// data is usable is a first-class state here, not a failure.
fn head_readout(section: Section, state: Option<&SurfaceState>) -> String {
    match (section, state) {
        // 读得到真实事实时，两个读数都回答「这里有没有出问题」：空缺的工位和没归位的
        // 安装，都是会让活派不出去的东西。此前这里是两个写死的 UNKNOWN，而页面正下方
        // 就写着这台工位「在岗」——同一屏给出两个互相矛盾的答案。
        (Section::Runtime, Some(state)) => readout(&[
            (&state.vacant_stations.to_string(), "空缺工位"),
            (&state.unclaimed_installations.to_string(), "未归位安装"),
        ]),
        (Section::Attention, _) => readout(&[("UNKNOWN", "待处理"), ("UNKNOWN", "数据缺失")]),
        (Section::Targets, _) => readout(&[("UNKNOWN", "巡逻中断"), ("UNKNOWN", "建档未完成")]),
        (Section::Operations, _) => readout(&[("UNKNOWN", "异常阶段"), ("UNKNOWN", "最近一轮")]),
        (Section::Tasks, _) => readout(&[("UNKNOWN", "失败"), ("UNKNOWN", "部分完成")]),
        (Section::Runtime, None) => readout(&[("UNKNOWN", "空缺工位"), ("UNKNOWN", "未归位安装")]),
    }
}

/// 上下文行右侧的系统状态词。
///
/// **「调度器未接通」是这一组里唯一一句系统读不到答案的话。** 没有任何进程心跳记录，
/// 因此页面既不能说它在跑，也不能说它没在跑——只能如实说心跳读不到。编一个「运行中」
/// 出来是撒谎，继续写「未接通」同样是撒谎，而且是更难被发现的那种。
fn system_words(state: Option<&SurfaceState>) -> String {
    let Some(state) = state else {
        return "<span class=\"v7-query-meta\">SCHEDULER NOT CONNECTED</span>\
                <span>NO OBSERVATION TARGETS</span><span>UTC+08</span>"
            .to_owned();
    };
    // 系统状态词这一槽位只放状态，不放计数——计数是页面自己的读数，属于左边的
    // `.v7-kpi`（LIDS 页头收回条）。数字混进 9px Mono 大写的系统词里两头都不像。
    //
    // 观察目标与巡检合成一个词，而不是各占一格：这一行在 1280 下本来就挤，而三种情况
    // 互斥，分两格只是把同一件事说两遍。词的强度依次递进，读的人一眼知道卡在哪一层。
    let observation = match (state.total_targets, state.monitoring_targets) {
        (0, _) => "NO OBSERVATION TARGETS",
        (_, 0) => "PATROL OFF",
        _ => "PATROL ARMED",
    };
    format!(
        "<span class=\"v7-query-meta\">SCHEDULER HEARTBEAT UNREADABLE</span>\
         <span>{observation}</span><span>UTC+08</span>"
    )
}

/// Targets and Operations carry their own second bar. Filter tabs stay disabled: with no
/// targets there is nothing to filter, and pretending otherwise would be a fake control.
/// 观察目标的筛选。
///
/// 这些页签此前一律 disabled，理由是「没有目标就没有可筛的」。但目标现在真的能建了，
/// 继续禁用就变成了「有东西却不让看」。改为真链接：筛选只改读取范围，不消耗任何平台访问。
fn target_filter_tabs(active: Option<&str>, counts: Option<&TargetCounts>) -> String {
    // 计数跟着标签走（内容工作台：`全部来源 88 / 博主 84 / 关键词 4`），而不是在列表
    // 上方再摆一排带计数的按钮——那会让同一组筛选在一屏里出现两遍。
    let labels: Vec<(&str, String)> = match counts {
        Some(counts) => vec![
            ("", format!("全部来源 {}", counts.total)),
            ("creator", format!("创作者 {}", counts.creator)),
            ("keyword", format!("关键词 {}", counts.keyword)),
            ("archiving", format!("建档中 {}", counts.archiving)),
            ("monitoring", format!("巡逻中 {}", counts.monitoring)),
        ],
        // 数不出来时不写 0——0 看起来像一个已知结论，而事实是没读到。
        None => vec![
            ("", "全部来源".to_owned()),
            ("creator", "创作者".to_owned()),
            ("keyword", "关键词".to_owned()),
            ("archiving", "建档中".to_owned()),
            ("monitoring", "巡逻中".to_owned()),
        ],
    };
    labels
        .iter()
        .map(|(value, label)| {
            let current = active.unwrap_or("");
            let class = if current == *value {
                " class=\"c-on\""
            } else {
                ""
            };
            let href = if value.is_empty() {
                "/collection/targets".to_owned()
            } else {
                format!("/collection/targets?filter={value}")
            };
            format!(r#"<a{class} href="{href}">{label}</a>"#)
        })
        .collect()
}

fn second_bar(
    section: Section,
    mode: OperationsMode,
    filter: Option<&str>,
    counts: Option<&TargetCounts>,
) -> String {
    match section {
        Section::Targets => format!(
            r#"<div class="c-toolbar">
          <div class="c-tabs c-tg-views">{target_filters}</div>
          <div class="c-actions c-tg-toolbar">
            <a class="c-btn-quiet" href="/collection/targets?sort=last">排序 / 最近观察 ↓</a>
            <form class="c-target-add" method="post" action="/collection/targets/new">
              <select name="target_kind" aria-label="目标类型">
                <option value="creator">创作者</option>
                <option value="keyword">关键词</option>
              </select>
              <input name="identity" required maxlength="120"
                     placeholder="创作者主页链接或 ID／关键词" />
              <button class="c-btn-primary" type="submit">＋ 新建目标</button>
            </form>
          </div>
        </div>"#,
            target_filters = target_filter_tabs(filter, counts),
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

pub fn render(
    section: Section,
    mode: OperationsMode,
    drawer: Option<&str>,
    filter: Option<&str>,
    counts: Option<&TargetCounts>,
    state: Option<&SurfaceState>,
) -> String {
    let entry = meta(section);
    // DESIGN-003 header reclaim: this surface's own counts ride in the context row next to
    // the system state, so the page can start at its content instead of restating its name.
    let meta_row = format!(
        "{counts}<i class=\"v7-vr\" aria-hidden=\"true\"></i>{system}",
        counts = head_readout(section, state),
        system = system_words(state),
    );
    // 系统边界那个位置在 shell.css 里是**警告样式**（琥珀底 + 警告圆点），因此它只能
    // 放真正值得警惕的事。「采集运行时未接通」曾经合格，接通后成了假话；换成「只写本地
    // 记录」则是把一句正常状态塞进警告框。这台服务真正的边界是：它不访问任何平台。
    let boundary = if state.is_some() {
        "LOCAL HOST / NO PLATFORM ACCESS"
    } else {
        "LOCAL HOST / NO COLLECTION RUNTIME"
    };
    // 一级导航里「采集」的状态词同理：读得到才敢改，读不到保留原话。
    let collection_state = state.map(|state| {
        if state.total_targets > 0 {
            "观察中"
        } else {
            "无观察目标"
        }
    });
    let header = global_header(
        PrimarySurface::Collection,
        boundary,
        &crumb(section, mode),
        &meta_row,
        collection_state,
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
        <main class="c-page" aria-labelledby="page-title">
          <h1 class="v7-sr-only" id="page-title">{title}</h1>
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
        rail = rail(section, state),
        second_bar = second_bar(section, mode, filter, counts),
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
