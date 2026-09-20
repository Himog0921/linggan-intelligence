//! COLLECTION-001 · 执行工位页。
//!
//! 单独成文件，因为 `collection.rs` 已超模块行数上限，再长会让一个已知问题更糟。
//!
//! 这一页回答**准入的最后一问**——「现在有没有兼容工位、账号、预算与风险余量」。
//! 允许出现工程执行细节（工位、许可、心跳、额度），但**不允许用工程内部命名**：
//! 「准入第 5 问」「有界控制事实」「同一评估器」「lane 上限」这类词对读页面的人
//! 不承载任何含义，按 LANG-05 只有三类英文可以留（机器标识、合同字面取值、编号）。
//!
//! RUNTIME-STATION-TABLE-001（2026-09-13）把这一页从「四层系统叙述」改写成
//! **以工位为单位的管理台**。改写前，同一台机器的事实散在三处：控制资格块（接活
//! 开关、安装、凭据、账号资格）、工位行（名字、额度、能力矩阵）、底部「上一次
//! 问活」。要回答「这台机器现在能不能干活」，人得在页面上滚三个地方来回对照。
//! 现在一台机器一行，展开即管理。
//!
//! **这一页此前用写死的句子回答接通状态**：`调度器未接通`、`队列、租约、回执尚不
//! 存在`。三句写下时都是真的，之后三样全部接通，句子一个字没变——一个不会随系统
//! 状态改变的状态区块，等于一个永远不会响的警报器。现在没有一句接通状态是手写的。
//!
//! RUNTIME-STATION-V7-2-001（2026-09-20）按 v7.2 稿复刻**内容区**，并新增右侧
//! 「运行概览」抽屉。这一轮**只换表达，不换事实**：判定口径、四态区分、权限与
//! 八条写路由全部逐字不变，新增的四块内容全部来自这一页已经在读的读模型。
//!
//! 稿里抽屉中的三块（回传成功率、补采入口计数、失败原因 TOP 3）在本系统中没有
//! 对应事实——全库只有三个 `failure_code` 且没有聚合读模型，也没有 24 小时全局
//! 速率与逐时序列。Mog 于同日裁定换成真读得到的四块（规则策略 / 近 7 天执行结果 /
//! 补采与失败入口 / 积压与重试），稿中数值一个都不出现。

use super::collection::collection_control_surface_view::{RuntimeLaneControlView, RuntimeResourceView};
use super::collection_targets_view::{beijing_now_minutes, minutes_since_epoch, moment_without_year};
use linggan_evidence::{
    CONTROL_FRESHNESS_MINUTES, CapabilityState, RuntimeCapacityOverview, StationCapability,
    StationOverview, UnclaimedInstallation,
};
use std::collections::BTreeMap;
use uuid::Uuid;

/// 工位能力矩阵。缺一台的条目表示这台工位的能力读不到，与「没有能力」不是一回事。
pub type CapabilityMatrix = BTreeMap<Uuid, Vec<StationCapability>>;

/// 通道判定与工位控制事实。
///
/// 这两样此前由 `collection_control_surface_view` 单独渲染成页面顶部的一个区块，
/// 于是同一台工位在一页里出现两次、同一条通道有两个名字。现在它们作为**输入**
/// 进入这一页：通道判定进判断区，工位控制事实进工位表对应的那一行。
pub struct RuntimeControl<'a> {
    pub lanes: &'a [RuntimeLaneControlView],
    pub resources: &'a [RuntimeResourceView],
    /// 本机是否配置了账号身份摘要。没配置时认证身份不会上报，但明确的登录或限制
    /// 仍然会阻断接活——这两件事必须分开说，否则「不上报」会被读成「不检查」。
    pub account_observation_available: bool,
}

impl RuntimeControl<'_> {
    fn resource(&self, station_ref: Uuid) -> Option<&RuntimeResourceView> {
        self.resources
            .iter()
            .find(|resource| resource.station_ref == station_ref)
    }
}

/// 登记工位与认领安装都不消耗任何平台访问——它们只是在本地记下「这台机器是谁」。
/// 因此这一页允许出现真实可点的按钮，而会触发平台访问的控件仍然不存在。
const NO_PLATFORM_ACCESS_NOTE: &str =
    "登记工位、开认领窗口与认领安装都只写本地记录，不访问任何平台。";

/// 接单仪器面表头右侧那一格英文标记，稿里逐字如此。
///
/// 它是 **LANG-05 的已登记单点例外**。按 LANG-05 的判据（「这个英文词是不是一个可枚举
/// 的状态值、一个机器标识、或一个编号？」），`INSTALLATION_STATE` 三者都不是——它是对这
/// 一格的描述，属于该规则明令禁止的第四类「描述性标签」。用户（LANG-05 的决定者）于
/// 2026-09-20 明确要求保留，因此按**用户授权的例外**落地：作用范围仅此一格，不构成先例，
/// LANG-05 在其它任何位置继续有效。裁定见 `docs/design/lids/language-policy.md`。
///
/// 另记一条语义错配，留证不改：这个标记的字面意思是「安装状态」，而本块判的是**通道能
/// 不能接活**，两者不是同一件事。照用户指示逐字复刻。见 manifest §5d-5。
const INSTALLATION_STATE_MARK: &str = "INSTALLATION_STATE";

/// 一条安装「最近报到过」的判据。超过这个天数只在展开里作为历史列出，不进页头计数。
///
/// 页头此前把 7 条历史插件残留写成「未归位安装 7」。那 7 条全是同两台机器自己
/// 升级插件留下的旧安装，永远不会有人去认领；一个读起来像「有 7 台机器掉队了」
/// 的数字，会让人去找 7 台并不存在的机器。
pub const RECENT_INSTALLATION_DAYS: i64 = 7;

const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty c-empty-engineering\">";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// 判断「最近报到」时使用的当前时刻，按北京时间折成分钟。
///
/// 调用方取一次，然后**把同一个值**同时交给上下文行的计数和这一页的分组。两处各自
/// 取一次当前时间，在跨过第 7 天那一瞬间会数出两个不同的值：页头说 0 台待处理，
/// 页面下方却仍把那条安装归在「最近」里。
pub fn recent_installation_cutoff() -> i64 {
    beijing_now_minutes()
}

/// 一条安装是否在最近 `RECENT_INSTALLATION_DAYS` 天内第一次报到。
///
/// 认不出时间格式时算「最近」：把一条读不懂的记录悄悄折进历史，等于让它消失。
pub fn installation_is_recent(first_seen_at: &str, now_minutes: i64) -> bool {
    minutes_since_epoch(first_seen_at)
        .is_none_or(|seen| now_minutes - seen <= RECENT_INSTALLATION_DAYS * 24 * 60)
}

/// 渲染整页工作区，替换掉原来的空态。
///
/// 即使一台工位都没有也要替换：空态说的是「等工程」，而登记工位现在真的可以做了，
/// 继续显示「等工程」就是在说假话。
pub fn render_runtime(
    base: &str,
    overview: Option<&RuntimeCapacityOverview>,
    stations: &[StationOverview],
    unclaimed: &[UnclaimedInstallation],
    capabilities: &CapabilityMatrix,
    control: Option<&RuntimeControl<'_>>,
    now_minutes: i64,
    error: Option<&str>,
) -> String {
    render_runtime_with_roster(
        base,
        overview,
        Some((stations, unclaimed)),
        capabilities,
        control,
        now_minutes,
        error,
    )
}

/// 产能与心跳可以照常读到，而工位名册那一次查询失败。调用方用这个入口，而不是把
/// 那次失败转换成两个空切片——「读不到」与「没有」是两个说法，合并会让人去修一个
/// 没有坏的东西。
pub fn render_runtime_with_unreadable_roster(
    base: &str,
    overview: Option<&RuntimeCapacityOverview>,
    control: Option<&RuntimeControl<'_>>,
    now_minutes: i64,
    error: Option<&str>,
) -> String {
    render_runtime_with_roster(
        base,
        overview,
        None,
        &CapabilityMatrix::new(),
        control,
        now_minutes,
        error,
    )
}

fn render_runtime_with_roster(
    base: &str,
    overview: Option<&RuntimeCapacityOverview>,
    roster: Option<(&[StationOverview], &[UnclaimedInstallation])>,
    capabilities: &CapabilityMatrix,
    control: Option<&RuntimeControl<'_>>,
    now_minutes: i64,
    error: Option<&str>,
) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();

    let stations = roster.map(|(stations, _)| stations);
    // 通道判定在一页里只算一次，交给控制条、接单仪器面与抽屉三处共用。三处各自算
    // 一次，就是给「同一屏上同一件事有两个答案」留门——这一页此前正是这么坏的。
    let lanes = lane_rows(control, overview);
    let body = format!(
        r#"<div class="c-runtime">
              {deck}
              {verdict}
              {table}
              {running}
              {schedule}
              {footer}
              {drawer}
            </div>"#,
        deck = deck_markup(overview, lanes.as_ref()),
        verdict = verdict_markup(overview, stations, lanes.as_ref()),
        table = station_table_markup(roster, capabilities, control, now_minutes),
        running = running_markup(overview),
        schedule = schedule_markup(overview),
        footer = footer_markup(roster, now_minutes, error),
        drawer = runtime_drawer_markup(overview, lanes.as_ref(), capabilities, stations),
    );

    format!(
        "{before}{body}{after}",
        before = &base[..open],
        after = &base[close..],
    )
}

// ---------------------------------------------------------------------------
// 控制条：一屏之内先说清「现在能不能接活、几台在岗、几个在跑、排了几条」
// ---------------------------------------------------------------------------

/// 判定的四态在页面上的说法与语气。**控制条、接单仪器面与抽屉共用同一个词表**——
/// 三处各写一份，就是给「同一屏上同一件事有三个答案」留门。
///
/// 返回的第二个值是语气，不是颜色：它决定这一块用哪一档强调，具体取值住在样式表里。
fn verdict_state(rows: Option<&Vec<LaneRow>>) -> (&'static str, &'static str) {
    let Some(rows) = rows else {
        return ("读不到", "unknown");
    };
    match verdict_of(rows) {
        Verdict::All => ("能接活", "ok"),
        Verdict::Some => ("部分接不了活", "partial"),
        Verdict::None => ("接不了活", "blocked"),
    }
}

/// 控制条。
///
/// 两个动作都落在真实存在的地方，**没有一个是死链**：
///
/// 三个动作：
///
/// - 「新增工位」开登记弹窗。登记本来就住在本页，另开一个页面只会多一次跳转；
/// - 「运行概览」开右侧抽屉。**开抽屉的入口只有这一个**。第二轮曾把入口留在贴页边的
///   固定标签上，那片标签虽然滚动可达，却悬在内容区的两条线**之外**，读起来不像这一行
///   的动作。代价如实记着：按钮会随正文滚走，滚到页面下方时够不着——由用户确认接受；
/// - 第三个链到观察目标页。稿里这个位置写的是「查看观察轨迹」，而本系统的一级导航
///   里没有「观察轨迹」这一项——照抄会得到一条点了没反应的按钮，那比没有按钮更糟。
///
/// 「新增工位」是 `<button>` 而不是 `<a>`。`.c-btn` 不声明 `display` 与 `align-items`，
/// 居中靠的是原生 `<button>` 自带的那份；锚点拿不到，文字会吊在盒子顶边之外（实测文本
/// 上方留白 −1px、下方 22.5px）。这一行里只有它是锚点，所以按元素类型修，不去动
/// `.c-btn` 这条贯穿全站的基础规则。
///
/// 四个读数与下方的接单仪器面、今日运行出**同一批数**：这里算一遍、那里算一遍，
/// 迟早会算出两个数。
fn deck_markup(
    overview: Option<&RuntimeCapacityOverview>,
    lanes: Option<&Vec<LaneRow>>,
) -> String {
    let (state, tone) = verdict_state(lanes);
    let staffed = overview.map_or_else(
        || "读不到".to_owned(),
        |overview| {
            format!(
                "{staffed}/{registered}",
                staffed = overview.staffed_stations,
                registered = overview.registered_stations,
            )
        },
    );
    // 整块读不到时给「—」而不是 0：0 是「已确认为零」，— 是「此刻答不出」。
    let backlog_pick = |pick: fn(&linggan_evidence::DispatchLaneBacklog) -> i64| {
        overview
            .map(|overview| &overview.dispatch_backlog)
            .filter(|lanes| !lanes.is_empty())
            .map_or_else(
                || "—".to_owned(),
                |lanes| lanes.iter().map(pick).sum::<i64>().to_string(),
            )
    };
    let scheduled = overview.map_or_else(
        || "读不到".to_owned(),
        |overview| overview.monitor_rule_schedules.len().to_string(),
    );

    // 先落成具名变量再进数组：直接写 `.as_str()` 会在同一条语句末尾被丢弃，
    // 数组借的是已经死掉的值。
    let leased = backlog_pick(|lane| lane.leased_work_orders);
    // 语气按四态原样带上，不压成一个「是不是 ok」的布尔：压了之后 `读不到` 就只能
    // 和 `接不了活` 落在同一档，而「答不出」与「坏了」的处置完全不同。
    let readouts = [
        (state, "接单状态", tone),
        (staffed.as_str(), "在岗工位", ""),
        (leased.as_str(), "正在执行", ""),
        (scheduled.as_str(), "自动排程", ""),
    ];
    let cells: String = readouts
        .iter()
        .map(|(value, label, tone)| {
            format!(
                r#"<div class="c-deck-readout"{state}><b>{value}</b><span>{label}</span></div>"#,
                state = if tone.is_empty() {
                    String::new()
                } else {
                    format!(r#" data-state="{tone}""#)
                },
                value = escape(value),
                label = escape(label),
            )
        })
        .collect();

    format!(
        r#"<section class="c-deck">
              <div class="c-deck-id">
                <span class="c-deck-badge">{badge}</span>
                <div class="c-deck-name">工位控制</div>
              </div>
              <div class="c-deck-readouts">{cells}</div>
              <div class="c-deck-actions">
                <button class="c-btn c-btn-primary" type="button" data-station-register-open aria-haspopup="dialog" aria-expanded="false" aria-controls="station-register">新增工位</button>
                <button class="c-btn c-btn-quiet" type="button" data-runtime-drawer-toggle aria-controls="c-runtime-drawer" aria-expanded="false">运行概览</button>
                <a class="c-btn c-btn-quiet" href="/collection/targets">去观察目标页</a>
              </div>
            </section>"#,
        // 「实时」是一个关于新鲜度的说法，只有这一页真的从库里读到了才敢说。
        badge = if overview.is_some() { "实时" } else { "读不到" },
    )
}

// ---------------------------------------------------------------------------
// 第一层：能不能接活，缺哪一样
// ---------------------------------------------------------------------------

/// 第一层是一个**判断**，不是一串读数。
///
/// 数字（2/2 在岗、55/200 篇）是支撑判断的证据，不是主角；把它们当主角，读的人得
/// 自己在脑子里做一次判定，而那次判定服务端已经做过了。
///
/// 判断的来源只有一个：能读到通道判定就用它，读不到才退回产能概览。**两者都渲染
/// 会让同一件事在一屏里有两个答案**——改写前顶部说「三条通道可接活」、下面说
/// 「两条通道可接活」，同一个 `deep_archive` 一处叫「基线建档」、一处叫「批量建档」。
///
/// v7.2 把这一块从「一段散文加几个格子」改成**仪器面**：深色表头一句话给出判定与
/// 阻塞数量，左边逐条列出通道，右边四个读数。改的是外壳，判定与文案一个字没动。
fn verdict_markup(
    overview: Option<&RuntimeCapacityOverview>,
    stations: Option<&[StationOverview]>,
    lanes: Option<&Vec<LaneRow>>,
) -> String {
    let Some(rows) = lanes else {
        // 读不到与「没有」是两个不同的说法，绝不能合并。
        return format!(
            r#"<section class="c-instr c-verdict-unknown">
                <div class="c-instr-head">
                  <span class="c-instr-badge">读不到</span>
                  <strong class="c-instr-state">读不到</strong>
                  <span class="c-instr-code">通道数未知</span>
                  <span class="c-instr-en">{mark}</span>
                </div>
                <div class="c-instr-body">
                  <div class="c-instr-issues">
                    <p class="c-instr-note">现在读不到「能不能接活」的判定。这不表示系统接不了活，只表示这一页此刻答不出——两者的处置不同。</p>
                  </div>
                </div>
              </section>"#,
            mark = INSTALLATION_STATE_MARK,
        );
    };
    let verdict = verdict_of(rows);
    let (state, tone) = verdict_state(Some(rows));
    // 阻塞与「可排队」必须分开数：可排队的通道缺的是工位，不是坏了。压成一个数，
    // 读的人会去修一条没有坏的通道。
    let blocked = rows
        .iter()
        .filter(|row| !row.available && !row.queueable)
        .count();
    let queueable = rows.iter().filter(|row| !row.available && row.queueable).count();
    let (badge, badge_tone) = if blocked > 0 {
        (format!("{blocked} 项阻塞"), " c-instr-badge-bad")
    } else if queueable > 0 {
        (format!("{queueable} 项等工位"), "")
    } else {
        ("全部通畅".to_owned(), "")
    };
    // 表头那一行（徽章 + 状态词）已经把判定说完了；再铺一句同义的散文只是重复一遍，
    // 「一条都派不出」这一档尤其如此——它那句「下面逐条说明卡在哪一样」，下面本来就
    // 逐条写着。用户 2026-09-20 指着这一句要删，删的是**复述判定**的这一类。
    //
    // 另外两档留着，理由不同：它们不复述判定，说的是**该怎么读**这个判定——「派得出
    // 任务」不等于「有活在跑」、几种堵塞不能合并成一件事。表头那三个字装不下这层意思。
    let why = match verdict {
        Verdict::All => Some("下面每条通道现在都派得出任务。这不代表现在有活在跑，只代表申请到这一步不会被挡下。"),
        Verdict::Some => Some("还有通道派得出任务，下面这几条现在派不出。它们卡住的原因各不相同，不要当成同一件事。"),
        Verdict::None => None,
    };

    format!(
        r#"<section class="c-instr c-verdict-{tone}">
              <div class="c-instr-head">
                <span class="c-instr-badge{badge_tone}">{badge}</span>
                <strong class="c-instr-state">{state}</strong>
                <span class="c-instr-code">{count} 条通道</span>
                <span class="c-instr-en">{mark}</span>
              </div>
              <div class="c-instr-body">
                <div class="c-instr-issues">
                  {note}{lanes}
                </div>
                {readouts}
              </div>
            </section>"#,
        count = rows.len(),
        note = why.map_or_else(String::new, |why| {
            format!(r#"<p class="c-instr-note">{}</p>"#, escape(why))
        }),
        lanes = lane_markup(rows),
        readouts = factor_markup(overview, stations),
        mark = INSTALLATION_STATE_MARK,
    )
}

enum Verdict {
    All,
    Some,
    None,
}

/// 一条通道在页面上的样子。两个来源折成同一种行，页面只认这一种。
struct LaneRow {
    name: String,
    needs: &'static str,
    available: bool,
    /// 准入允许入队、但还没有工位能领——这既不是「能接活」也不是「坏了」。
    queueable: bool,
    reason: Option<String>,
    reason_code: Option<String>,
}

fn verdict_of(rows: &[LaneRow]) -> Verdict {
    if rows.iter().all(|row| row.available) {
        Verdict::All
    } else if rows.iter().any(|row| row.available) {
        Verdict::Some
    } else {
        Verdict::None
    }
}

/// 通道该怎么读：这条通道要去干什么。
///
/// 这句话写在页面上，而不是留给人去猜 `deep_archive` 是什么意思。
///
/// **对象类型参与这句话**：关键词巡检查的是搜索命中，创作者巡检查的是作品清单。
/// 两者都写成「查一遍作品清单」，就是把一个关键词说成有作品清单的对象。
fn lane_needs(target_kind: &str, lane: &str) -> &'static str {
    match (target_kind, lane) {
        (_, "deep_archive") => "按作者档案建一份有界作品清单",
        ("keyword", "patrol") => "查一遍搜索结果有没有新命中",
        (_, "patrol") => "查一遍作品清单有没有新作品",
        _ => "这条通道的用途尚未登记说明",
    }
}

fn lane_rows(
    control: Option<&RuntimeControl<'_>>,
    overview: Option<&RuntimeCapacityOverview>,
) -> Option<Vec<LaneRow>> {
    if let Some(control) = control.filter(|control| !control.lanes.is_empty()) {
        return Some(
            control
                .lanes
                .iter()
                .map(|lane| LaneRow {
                    name: lane.label.to_owned(),
                    needs: lane_needs(lane.target_kind, lane.lane),
                    available: lane.available,
                    queueable: lane.queueable,
                    reason: lane.reason.clone(),
                    reason_code: lane.reason_code.map(str::to_owned),
                })
                .collect(),
        );
    }
    // 通道判定读不到时退回产能概览。它的粒度更粗（不分创作者与关键词），但它与
    // 上面的判断出自同一次读取，不会自相矛盾。
    let overview = overview?;
    Some(
        overview
            .lanes
            .iter()
            .map(|lane| LaneRow {
                name: lane.zh.to_owned(),
                needs: lane_needs("", lane.lane),
                available: lane.is_available(),
                queueable: false,
                reason: lane.blocking_reason(),
                reason_code: None,
            })
            .collect(),
    )
}

/// 每条通道分别判定，并逐条说明它卡在哪一样。
///
/// 合并成一次或压成一句「资源不足」，会让「创作者还能跑、关键词停了」这种真实情况
/// 消失——两条通道可能因为完全不同的原因停下（一条缺能力、一条额度满），而它们的
/// 处置差着四个子系统。
///
/// 每一行说的是「这条通道要去干什么」加「现在为什么不行」。原文稿这一行只有
/// 「通道名 · 插件心跳超时」，后面半句从哪来、前半句去哪了，读的人无从判断。
/// 一条通道现在的状态：样式类与说法。
///
/// **主页面与右侧抽屉共用这一个函数。** 两处各写一份判定，就是给「同一屏上同一条
/// 通道有两个说法」留门——这一页此前正是这么坏的（顶部说三条可接活、下面说两条）。
fn lane_state(row: &LaneRow) -> (&'static str, &'static str) {
    if row.available {
        ("c-lane-ok", "可接活")
    } else if row.queueable {
        ("c-lane-queueable", "可排队，等工位")
    } else {
        ("c-lane-blocked", "接不了")
    }
}

/// 状态在机器侧的字面取值。它只用于标记，不参与说话。
fn capacity_state(row: &LaneRow) -> &'static str {
    if row.available {
        "available"
    } else if row.queueable {
        "queueable"
    } else {
        "blocked"
    }
}

/// 原因码是数据合同里的字面取值，按 LANG-05 第 2 类可以作为中文旁边的小字保留；
/// 它旁边永远有一句中文，不单独承担含义。
fn reason_code_markup(row: &LaneRow) -> String {
    row.reason_code
        .as_deref()
        .map_or_else(String::new, |code| {
            format!(
                r#"<code class="c-lane-code">{code}</code>"#,
                code = escape(code)
            )
        })
}

fn lane_markup(rows: &[LaneRow]) -> String {
    let rendered: String = rows
        .iter()
        .map(|row| {
            let (state_class, label) = lane_state(row);
            // 通得了的通道写它要去干什么，通不了的写它为什么不行——两句话都来自
            // 服务端，页面这一层不补写。
            let detail = row.reason.as_deref().unwrap_or(row.needs);
            format!(
                r#"<div class="c-instr-issue" data-capacity-state="{capacity_state}">
                    <i class="c-lane-mark" aria-hidden="true"></i>
                    <span class="c-lane-name"><b>{name}</b> · {detail}</span>
                    <em class="c-lane-state {state_class}">{label}</em>
                    {code}
                  </div>"#,
                capacity_state = capacity_state(row),
                name = escape(&row.name),
                detail = escape(detail),
                code = reason_code_markup(row),
            )
        })
        .collect();
    format!(r#"<div class="c-instr-issue-list">{rendered}</div>"#)
}

/// 第二层：判定用到的几样资源各自的当前值。
///
/// **账号那一项单独说明**：服务端判定的是风险、工位、能力、预算四样，账号不在其中，
/// 它按每台工位单独判定。四项里有一项从未参与判定，却显示成四项齐备，就是用视觉
/// 便利改写资格；所以第 4 格的限定语写「每台单独判定」，并把值指向工位表的「账号」列。
fn factor_markup(
    overview: Option<&RuntimeCapacityOverview>,
    stations: Option<&[StationOverview]>,
) -> String {
    let Some(overview) = overview else {
        return String::new();
    };
    let station_value = format!(
        "{staffed}/{registered}",
        staffed = overview.staffed_stations,
        registered = overview.registered_stations,
    );
    // 额度按工位计（Mog 于 2026-08-27 确认）。多台工位时不加总成一个数——各自的额度
    // 是各自的边界，加总出来的「400」不对应任何一个真实上限。
    let budget_value = match stations {
        None => "读不到".to_owned(),
        Some([]) => "—".to_owned(),
        Some([single]) => format!(
            "{used}/{quota}",
            used = single.daily_notes_used,
            quota = single.daily_work_quota
        ),
        Some(many) => format!("{} 台各计", many.len()),
    };
    let risk_value = match overview.risk_pauses.len() {
        0 => "无".to_owned(),
        count => format!("{count} 条生效"),
    };

    // 有风险暂停生效时，那一格整块强调：四条读数里它是唯一说明「为什么现在不动」的。
    let cells: [(bool, &str, String, &str); 4] = [
        (false, "在岗工位", station_value, "在岗 / 已登记"),
        (false, "今日预算", budget_value, "今天已采 / 每台每日上限"),
        (
            !overview.risk_pauses.is_empty(),
            "风险余量",
            risk_value,
            "生效中的风险暂停",
        ),
        (
            false,
            "观察账号",
            "每台单独判定".to_owned(),
            "见下面工位表的「账号」列",
        ),
    ];

    // 三行的次序是「读数 → 限定语 → 名称」，名称落在最后。这与控制条上那四个读数是
    // 同一个次序；两处都是读数卡，读的人不该为了同一件事换两次读法。
    let rendered: String = cells
        .iter()
        .map(|(active, label, value, note)| {
            format!(
                r#"<div class="c-readout"{active}>
                    <b>{value}</b>
                    <span>{note}</span>
                    <small>{label}</small>
                  </div>"#,
                active = if *active { " data-active" } else { "" },
                label = escape(label),
                value = escape(value),
                note = escape(note),
            )
        })
        .collect();

    // 只返回四格本身。仪器面正文是五列栅格（左列通道 + 四格读数），四格必须是它的直接
    // 孩子；再套一层包装，四格会掉进同一格里挤成一条。底下那句说明句按用户 2026-09-20
    // 的指示删除——它没有独占信息，第 4 格自己的限定语已经把同一件事说过了。
    rendered
}

// ---------------------------------------------------------------------------
// 第二层：工位总表。一台机器一行，展开即管理
// ---------------------------------------------------------------------------

/// 表头。列名全部是中文，且每一列都能被一行里的值直接回答。
const STATION_COLUMNS: &str = r#"<span role="columnheader">编号</span><span role="columnheader">工位</span><span role="columnheader">状态</span><span role="columnheader">插件</span><span role="columnheader">最后报到</span><span class="c-tg-head-num" role="columnheader">今日采集</span><span role="columnheader">接活</span><span role="columnheader">账号</span><span role="columnheader">能力</span><span role="columnheader">上次问活</span><span role="columnheader">明细</span>"#;

fn station_table_markup(
    roster: Option<(&[StationOverview], &[UnclaimedInstallation])>,
    capabilities: &CapabilityMatrix,
    control: Option<&RuntimeControl<'_>>,
    now_minutes: i64,
) -> String {
    let Some((stations, _)) = roster else {
        return r#"<section class="c-sect c-stn">
              <div class="c-sect-head"><span class="c-sect-badge soft">读不到</span><h2>工位</h2><div class="c-sect-meta">台数未知</div></div>
              <p class="c-stn-empty">工位列表当前读不到。这不表示没有登记工位，也不表示它们离线——两者的处置不同。</p>
            </section>"#
            .to_owned();
    };
    if stations.is_empty() {
        return r#"<section class="c-sect c-stn">
              <div class="c-sect-head"><span class="c-sect-badge ink">0 台</span><h2>工位</h2><div class="c-sect-meta">合计 · 0</div></div>
              <p class="c-stn-empty">还没有登记任何工位。这不是「等工程」——登记一台是你现在就能做的事，页面下方就是入口。</p>
            </section>"#
            .to_owned();
    }
    let rows: String = stations
        .iter()
        .enumerate()
        .map(|(index, station)| {
            station_entry(
                index,
                station,
                capabilities.get(&station.station_ref),
                control.and_then(|control| control.resource(station.station_ref)),
                control.is_none_or(|control| control.account_observation_available),
                now_minutes,
            )
        })
        .collect();
    // 表头右侧只报「几台在岗 / 共几台」，不报「几台有问题」——问题台的尺寸由接单
    // 仪器面回答，同屏两处说同一件事迟早会说出两个数。
    let staffed = stations
        .iter()
        .filter(|station| station.active_plugin_version.is_some())
        .count();
    format!(
        r#"<section class="c-sect c-stn">
              <div class="c-sect-head">
                <span class="c-sect-badge ink">台</span>
                <h2>工位</h2>
                <div class="c-sect-meta">{staffed} / {count} 在岗 · 合计 {count}</div>
              </div>
              <div class="c-tg-table-scroll" role="table" aria-label="工位">
                <div class="c-tg-table-head c-tg-station-grid" role="row">{STATION_COLUMNS}</div>
                <div class="c-stn-list" role="rowgroup">{rows}</div>
              </div>
            </section>"#,
        count = stations.len(),
    )
}

/// 一台工位：一行读完，展开管理。
///
/// 行本身是 `<summary>`，因此行里不放表单——`<form>` 不是 `<summary>` 的合法内容，
/// 而且放进去会让一次点击既提交又折叠。更名、停用、接活开关与账号确认全部在展开的
/// 面板里，它们都是低频且其中两项不可逆，多一次点击是对的方向。
fn station_entry(
    index: usize,
    station: &StationOverview,
    capabilities: Option<&Vec<StationCapability>>,
    control: Option<&RuntimeResourceView>,
    account_observation_available: bool,
    now_minutes: i64,
) -> String {
    let (state_label, state_tone) = station_state(station, now_minutes);
    let plugin = station.active_plugin_version.as_deref().unwrap_or("—");
    let last_seen = station
        .active_last_seen_at
        .as_deref()
        .map_or_else(|| "尚未报到".to_owned(), |at| moment_without_year(at).to_owned());
    // 额度是这一行唯一能告诉人「今天还能干多少活」的数。用尽时它也是准入拒绝的来源。
    let quota = i64::from(station.daily_work_quota);
    let exhausted = station.daily_notes_used >= quota;
    let quota_tone = if exhausted { " c-tg-warn" } else { "" };
    // 12 格是粗读刻度，不是数据：真实读数永远是旁边那串「已用/上限」。所以格子数与
    // 上限之间不存在任何「一格代表几篇」的约定，上限换了也不用改这里。上限为 0 时
    // 一格不亮——不猜一个比例出来。
    let lit = if quota > 0 {
        (station.daily_notes_used.clamp(0, quota) * 12 + quota - 1) / quota
    } else {
        0
    };
    let usage_track: String = (0..12)
        .map(|segment| {
            if segment < lit {
                r#"<i class="c-usage-seg on"></i>"#
            } else {
                r#"<i class="c-usage-seg"></i>"#
            }
        })
        .collect();
    // 换过几次插件要看得见。内容工作台正是让这个数字隐身，才变成 11 台僵尸工位。
    let history = match station.superseded_count {
        0 => "首次安装".to_owned(),
        count => format!("换过 {count} 次插件"),
    };
    let (dispatch_label, dispatch_hint) = station_dispatch_answer(station);
    let caps = capability_counts(capabilities);

    format!(
        r#"<details class="c-stn-entry" data-station-entry="{station_ref}">
              <summary class="c-tg-item c-tg-station-grid" role="row">
                <div class="c-tg-number" role="cell"><span class="c-tg-index">{index:02}</span></div>
                <div class="c-tg-object" role="cell"><div class="c-tg-object-text"><span class="c-tg-title">{name}</span><span class="c-tg-meta">{history}</span></div></div>
                <div class="c-tg-cell" role="cell"><span class="c-tg-truth {state_tone}">{state}</span></div>
                <div class="c-tg-cell c-stn-mono" role="cell">{plugin}</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last_seen}</time>
                <div class="c-tg-cell c-tg-number-value{quota_tone}" role="cell"><span class="c-usage"><span class="c-usage-value">{used}/{quota_text}</span><span class="c-usage-track" aria-hidden="true">{usage_track}</span></span></div>
                <div class="c-tg-cell" role="cell">{accepting}</div>
                <div class="c-tg-cell" role="cell">{account}</div>
                <div class="c-tg-cell" role="cell">{caps_summary}</div>
                <div class="c-tg-cell" role="cell" title="{dispatch_title}">{dispatch}</div>
                <div class="c-tg-cell c-stn-more" role="cell">明细</div>
              </summary>
              <div class="c-stn-panel">
                {actions}
                {account_panel}
                {dispatch_panel}
                {matrix}
              </div>
            </details>"#,
        station_ref = station.station_ref,
        index = index + 1,
        name = escape(&station.display_name),
        history = escape(&history),
        state = state_label,
        plugin = escape(plugin),
        last_seen = escape(&last_seen),
        used = station.daily_notes_used,
        quota_text = station.daily_work_quota,
        usage_track = usage_track,
        accepting = accepting_cell(control),
        account = account_cell(control),
        caps_summary = caps.summary_markup(),
        dispatch = escape(dispatch_label),
        dispatch_title = escape(&format!("{dispatch_label} · {dispatch_hint}")),
        actions = station_actions(station),
        account_panel = account_panel(control, account_observation_available),
        dispatch_panel = dispatch_panel(station, dispatch_label, &dispatch_hint),
        matrix = capability_matrix_markup(capabilities),
    )
}

/// 「状态」一列：在岗 / 失联 / 空缺，三件不同的事。
///
/// **「有插件认领这台工位」与「这台工位现在还连着」是两回事。** 改写前这一列只分前者，
/// 于是 2026-09-13 那次插件后台进程睡了 2 小时 26 分时，页面首屏已经正确地说「接不了活 ·
/// installation_stale」，这一列却还写着「在岗」——同一屏上两个互相矛盾的答案，而那个让人
/// 放心的那个在离眼睛更近的地方。
///
/// 阈值直接用准入判定的那一个常量（`CONTROL_FRESHNESS_MINUTES`），不另写一个数：
/// 页面与判定各写一个 20，就是给「以后有人只改了一处」留了门。
///
/// 读不到报到时间时说「在岗」但不给肯定语气——我们知道有插件认领了它，只是判断不了新鲜度；
/// 编一个「失联」出来和编一个「在岗」一样，都是拿未知冒充已知。
fn station_state(station: &StationOverview, now_minutes: i64) -> (&'static str, &'static str) {
    if station.active_plugin_version.is_none() {
        // 没有在岗安装不等于工位坏了，只是现在没插件连着它。
        return ("空缺", "c-tg-neutral");
    }
    let Some(seen) = station
        .active_last_seen_at
        .as_deref()
        .and_then(minutes_since_epoch)
    else {
        return ("在岗", "c-tg-neutral");
    };
    if now_minutes - seen > i64::from(CONTROL_FRESHNESS_MINUTES) {
        ("失联", "c-tg-warn")
    } else {
        ("在岗", "c-tg-ok")
    }
}

/// 「接活」一列。读不到控制事实时说读不到，不写成「已暂停」——那会让人去开一个
/// 本来就没关的开关。
fn accepting_cell(control: Option<&RuntimeResourceView>) -> String {
    control.map_or_else(
        || r#"<span class="c-tg-truth c-tg-neutral">读不到</span>"#.to_owned(),
        |resource| {
            if resource.accepting_tasks {
                r#"<span class="c-tg-truth c-tg-ok">自动</span>"#.to_owned()
            } else {
                r#"<span class="c-tg-truth c-tg-warn">已暂停</span>"#.to_owned()
            }
        },
    )
}

/// 「账号」一列。绑定状态是这一列的主语；账号资格的机器取值留在展开里。
fn account_cell(control: Option<&RuntimeResourceView>) -> String {
    let Some(resource) = control else {
        return r#"<span class="c-tg-truth c-tg-neutral">读不到</span>"#.to_owned();
    };
    let (tone, label) = account_binding_label(&resource.binding_state);
    format!(r#"<span class="c-tg-truth {tone}">{label}</span>"#)
}

/// 绑定状态的中文与语气。
///
/// 「尚无观察账号」用中性语气而不是警告：没有绑定账号的工位不是坏了，它只是还没被
/// 指派观察身份。需要人动手的只有「待确认」与「已变化」两种。
fn account_binding_label(state: &str) -> (&'static str, &'static str) {
    match state {
        "current" => ("c-tg-ok", "已确认"),
        "unconfirmed" => ("c-tg-warn", "待确认"),
        "changed" => ("c-tg-warn", "已变化"),
        "bound_without_eligibility" => ("c-tg-warn", "缺资格信号"),
        _ => ("c-tg-neutral", "未绑定"),
    }
}

/// 每台工位的四个真实动作。它们都只写本地记录，不访问任何平台。
fn station_actions(station: &StationOverview) -> String {
    let window = if station.claim_window_open {
        "认领窗口开着：新装的插件会自动归到这台工位。"
    } else {
        "认领窗口已关：新装的插件会停在下面的待认领里等你指认。"
    };
    format!(
        r#"<div class="c-stn-block">
              <h3>这台工位</h3>
              <form class="c-stn-form" method="post" action="/collection/runtime/stations/name" data-station-name-form>
                <input type="hidden" name="station_ref" value="{station_ref}" />
                <label for="station-name-{station_ref}">工位名称</label>
                <input id="station-name-{station_ref}" name="display_name" required maxlength="60" value="{name}" />
                <button class="c-btn-quiet" type="submit">更名</button>
              </form>
              <p class="c-stn-note">{window}</p>
              {window_form}
              <form class="c-stn-form" method="post" action="/collection/runtime/retire">
                <input type="hidden" name="station_ref" value="{station_ref}" />
                <button class="c-tg-act c-tg-act-danger" type="submit">停用这台工位</button>
              </form>
            </div>"#,
        station_ref = station.station_ref,
        name = escape(&station.display_name),
        window = escape(window),
        window_form = claim_window_form(station),
    )
}

/// 认领窗口的开关。
///
/// 改写前它住在一个叫「开发期工具」的折叠块里——一个用户界面上不该出现的名字。它其实
/// 是一台工位自己的属性（新装的插件要不要自动归到我这里），因此现在住在那台工位下面。
fn claim_window_form(station: &StationOverview) -> String {
    if station.claim_window_open {
        return format!(
            r#"<form class="c-stn-form" method="post" action="/collection/runtime/close-window">
                 <input type="hidden" name="station_ref" value="{station_ref}" />
                 <button class="c-btn-quiet" type="submit">现在就关掉认领窗口</button>
               </form>"#,
            station_ref = station.station_ref,
        );
    }
    format!(
        r#"<form class="c-stn-form" method="post" action="/collection/runtime/claim-window">
             <input type="hidden" name="station_ref" value="{station_ref}" />
             <label for="claim-hours-{station_ref}">开一个认领窗口</label>
             <select id="claim-hours-{station_ref}" name="valid_for_hours">
               <option value="24">24 小时</option>
               <option value="8">8 小时</option>
               <option value="1">1 小时</option>
             </select>
             <button class="c-btn-quiet" type="submit">开窗口</button>
           </form>"#,
        station_ref = station.station_ref,
    )
}

/// 展开里的账号与安装明细。
///
/// 凭据只显示有效性，账号身份摘要不上报——这两条是既有的数据边界，本次不改，
/// 只把它们从页面顶部搬到它们真正归属的那台工位下面。
fn account_panel(
    control: Option<&RuntimeResourceView>,
    account_observation_available: bool,
) -> String {
    let Some(resource) = control else {
        return r#"<div class="c-stn-block"><h3>接活与账号</h3><p class="c-stn-note">这台工位的接活开关与账号绑定当前读不到。这不表示它没有绑定账号。</p></div>"#.to_owned();
    };
    let toggle = format!(
        r#"<form class="c-stn-form" method="post" action="/collection/runtime/accepting" data-station-accepting-form>
             <input type="hidden" name="station_ref" value="{station_ref}">
             <button class="c-btn-quiet" type="submit" name="accepting" value="{next}">{label}</button>
           </form>"#,
        station_ref = resource.station_ref,
        next = !resource.accepting_tasks,
        label = if resource.accepting_tasks {
            "暂停这台工位接活"
        } else {
            "恢复这台工位自动接活"
        },
    );
    let confirm = match (resource.installation_ref, resource.account_ref) {
        (Some(installation_ref), Some(account_ref))
            if matches!(resource.binding_state.as_str(), "unconfirmed" | "changed") =>
        {
            format!(
                r#"<form class="c-stn-form" method="post" action="/collection/runtime/account-bindings" data-account-binding-form>
                     <input type="hidden" name="installation_ref" value="{installation_ref}">
                     <input type="hidden" name="account_ref" value="{account_ref}">
                     <button class="c-btn-quiet" type="submit">确认这个观察账号</button>
                   </form>"#,
            )
        }
        _ => String::new(),
    };
    // 账号资格的机器取值是数据合同里的字面取值，按 LANG-05 第 2 类允许，且它旁边
    // 永远有一句中文说明现在该怎么办。
    let eligibility = resource
        .eligibility_reason_code
        .as_deref()
        .unwrap_or("account_unknown");
    let observed = resource
        .eligibility_observed_at
        .as_deref()
        .map_or_else(|| "从未观察过".to_owned(), |at| moment_without_year(at).to_owned());
    let busy = if resource.account_has_live_lease {
        "这个账号已经有一份活在跑。"
    } else {
        "这个账号当前没有活在跑。"
    };
    format!(
        r#"<div class="c-stn-block">
              <h3>接活与账号</h3>
              <dl class="c-stn-facts">
                <div><dt>插件安装</dt><dd>{installation}</dd></div>
                <div><dt>服务端凭据</dt><dd>{credential}</dd></div>
                <div><dt>账号绑定</dt><dd>{binding}</dd></div>
                <div><dt>最后一次账号观察</dt><dd>{observed}</dd></div>
              </dl>
              <p class="c-stn-note">{busy}账号资格尚未形成结论时按关闭处理，但不会仅因为「很久没观察」就拦住接活（{eligibility}）。</p>
              <p class="c-stn-note">{digest}</p>
              {toggle}{confirm}
            </div>"#,
        installation = short_ref(resource.installation_ref),
        credential = if resource.has_valid_credential {
            "有效"
        } else {
            "缺失或已失效"
        },
        binding = account_binding_label(&resource.binding_state).1,
        observed = escape(&observed),
        busy = escape(busy),
        eligibility = escape(eligibility),
        digest = escape(if account_observation_available {
            "未观察不阻断首单；只有人工确认会替换或结束绑定。"
        } else {
            "本机没有配置账号身份摘要：认证身份不会上报，但明确的登录或限制仍然会阻断接活。"
        }),
    )
}

fn dispatch_panel(station: &StationOverview, label: &str, hint: &str) -> String {
    let at = station
        .last_dispatch_answer_at
        .as_deref()
        .map_or_else(|| "—".to_owned(), |at| moment_without_year(at).to_owned());
    format!(
        r#"<div class="c-stn-block">
              <h3>上次问活</h3>
              <dl class="c-stn-facts">
                <div><dt>时间</dt><dd>{at}</dd></div>
                <div><dt>服务端的回答</dt><dd>{label}</dd></div>
              </dl>
              <p class="c-stn-note">{hint}</p>
            </div>"#,
        at = escape(&at),
        label = escape(label),
        hint = escape(hint),
    )
}

fn short_ref(value: Option<Uuid>) -> String {
    value.map_or_else(
        || "—".to_owned(),
        |value| value.to_string().chars().take(8).collect(),
    )
}

// ---------------------------------------------------------------------------
// 能力：一格摘要，展开看明细
// ---------------------------------------------------------------------------

/// 能力矩阵的摘要计数。
///
/// 三个数不能互相替代：**未验证不是降级**（声明支持但窗口内没派过活，我们没有证据
/// 说它坏了），**不支持不是降级**（在岗插件没声明这一项），**读不到不是没有**。
struct CapabilityCounts {
    readable: bool,
    ready: usize,
    degraded: usize,
    unverified: usize,
    undeclared: usize,
}

impl CapabilityCounts {
    fn summary_markup(&self) -> String {
        if !self.readable {
            return r#"<span class="c-tg-truth c-tg-neutral">读不到</span>"#.to_owned();
        }
        if self.ready + self.degraded + self.unverified + self.undeclared == 0 {
            return r#"<span class="c-tg-truth c-tg-neutral">尚无记录</span>"#.to_owned();
        }
        // 降级是唯一需要有人去处理的一档，因此它决定这一格的语气。
        let tone = if self.degraded > 0 {
            "c-tg-warn"
        } else {
            "c-tg-ok"
        };
        let mut parts = vec![format!("{} 就绪", self.ready)];
        if self.degraded > 0 {
            parts.push(format!("{} 降级", self.degraded));
        }
        if self.unverified > 0 {
            parts.push(format!("{} 未验证", self.unverified));
        }
        format!(
            r#"<span class="c-tg-truth {tone}">{text}</span>"#,
            text = escape(&parts.join(" · ")),
        )
    }
}

fn capability_counts(capabilities: Option<&Vec<StationCapability>>) -> CapabilityCounts {
    let Some(rows) = capabilities else {
        return CapabilityCounts {
            readable: false,
            ready: 0,
            degraded: 0,
            unverified: 0,
            undeclared: 0,
        };
    };
    let mut counts = CapabilityCounts {
        readable: true,
        ready: 0,
        degraded: 0,
        unverified: 0,
        undeclared: 0,
    };
    for row in rows {
        match row.state() {
            CapabilityState::Ready => counts.ready += 1,
            CapabilityState::Degraded => counts.degraded += 1,
            CapabilityState::Unverified => counts.unverified += 1,
            CapabilityState::NotDeclared => counts.undeclared += 1,
        }
    }
    counts
}

/// 能力的中文名。机器名是合同里的字面取值，中文名是给人读的——两者并存，中文在前。
///
/// 未登记的能力名原样显示机器名：编一个中文名出来，等于假装我们知道它是什么。
fn capability_label(capability: &str) -> &str {
    match capability {
        "discovery_search" => "搜索发现",
        "profile_discovery" => "主页发现",
        "author_profile" => "作者档案",
        "content_detail" => "内容详情",
        "comments" => "评论",
        "replies" => "评论回复",
        "media_slots" => "媒体",
        "xhs.pageAccess" => "页面访问",
        other => other,
    }
}

/// 能力矩阵。
///
/// 这一格回答的是「这台工位这项能力，最近还跑得成吗」。它有三个必须分开的判断，
/// 合并任意两个都会让这一格开始说假话：
///
/// - **未验证不是降级。** 声明支持但窗口内没派过活，我们没有证据说它坏了。写成降级
///   会让人去修一台没坏的机器；写成就绪则是拿没验证过的东西当已验证。
/// - **执行失败不是能力缺陷。** 页面超时、标签页丢失是这一次没成，不是这项干不了。
///   只有插件明确回 `capability_not_executable_here` 才是能力级降级。超时次数单独显示，
///   因为连续超时值得看见，但它不该让「作者档案」这项显示成坏的。
/// - **读不到不是没有。** 整块读不到时说读不到，不渲染成一排「不支持」。
fn capability_matrix_markup(capabilities: Option<&Vec<StationCapability>>) -> String {
    let Some(rows) = capabilities else {
        return r#"<div class="c-stn-block"><h3>能力</h3><p class="c-stn-note">能力矩阵当前读不到。这不表示这台工位没有能力，只表示这一项此刻答不出。</p></div>"#
            .to_owned();
    };
    if rows.is_empty() {
        return r#"<div class="c-stn-block"><h3>能力</h3><p class="c-stn-note">这台工位还没有能力记录：在岗插件没有声明能力，近 7 天也没有产出或失败。</p></div>"#
            .to_owned();
    }

    let mut cells = String::new();
    for row in rows {
        let (state_word, state_class, detail) = match row.state() {
            CapabilityState::Ready => (
                "就绪",
                "c-cap-ready",
                match row.last_success_at.as_deref() {
                    Some(at) => format!("{} 次 · 最后 {}", row.successes, moment_without_year(at)),
                    None => format!("{} 次", row.successes),
                },
            ),
            CapabilityState::Degraded => (
                "降级",
                "c-cap-degraded",
                match row.last_failure_at.as_deref() {
                    Some(at) => format!("插件回报跑不了 · 最后 {}", moment_without_year(at)),
                    None => "插件回报跑不了".to_owned(),
                },
            ),
            // 没有证据时说没有证据。这一格的存在本身就是为了不让它被读成「正常」。
            CapabilityState::Unverified => (
                "未验证",
                "c-cap-unverified",
                "近 7 天没派过这种活".to_owned(),
            ),
            CapabilityState::NotDeclared => (
                "不支持",
                "c-cap-undeclared",
                "在岗插件没有声明这一项".to_owned(),
            ),
        };
        // 执行失败与能力状态分开表达：它是一条附注，不是这一格的判定。
        let hiccups = if row.execution_failures > 0 {
            format!(
                r#"<em class="c-cap-hiccup">另有 {} 次执行失败（超时或标签页不可用），不是能力问题</em>"#,
                row.execution_failures
            )
        } else {
            String::new()
        };
        // 没有登记中文名时，机器名已经当了标题，不再重复印一遍。
        let label = capability_label(&row.capability);
        let machine = if label == row.capability {
            String::new()
        } else {
            format!("<code>{}</code>", escape(&row.capability))
        };
        cells.push_str(&format!(
            r#"<div class="c-cap {state_class}">
                  <b>{label}</b>
                  <span class="c-cap-state">{state_word}</span>
                  <span class="c-cap-detail">{detail}</span>
                  {machine}
                  {hiccups}
                </div>"#,
            label = escape(label),
            detail = escape(&detail),
        ));
    }

    format!(
        r#"<div class="c-stn-block c-stn-block-wide">
              <h3>能力<span>近 7 天</span></h3>
              <div class="c-caps-grid">{cells}</div>
            </div>"#
    )
}

// ---------------------------------------------------------------------------
// 上次问活：工位来要活时，服务端实际给出的回答
// ---------------------------------------------------------------------------

/// 这一列回答的是「现在为什么不动」。
///
/// 2026-09-08 那一整天，「等待 5 单」一直显示得好好的，服务端每 5 分钟也都明确回答了
/// 「被拦住了」——但那个回答只发给插件，页面上一个字也没有。人于是只能看着一个不动的
/// 数字，猜不出原因。
fn station_dispatch_answer(station: &StationOverview) -> (&'static str, String) {
    let Some(code) = station.last_dispatch_answer_code.as_deref() else {
        // 从未问过与「问了但没活」是两件事。前者说明这台工位根本没来敲过门，
        // 该查的是插件那一侧，不是队列。
        return (
            "从未问过活",
            "这台工位还没有向服务端要过任务。先确认插件已安装，并且已经归位到这台工位。".to_owned(),
        );
    };
    dispatch_answer_label(code, station.last_dispatch_answer_reason.as_deref())
}

/// 每一条派发回答的中文说明：这是什么事，你该做什么。
///
/// 写成一张表而不是一长串分支：它本来就是一份词表，不是控制流。新增一条原因码时，
/// 这里加一行即可，读的人也一眼看得出总共认得几条。
const DISPATCH_ANSWER_EXPLANATIONS: &[(&str, &str, &str)] = &[
    ("dispatch", "派出了一单", "工位正在执行它；这是正常状态。"),
    (
        "nothing_waiting",
        "队列里没有活",
        "这不是故障。要立刻采一次，去观察目标页发起人工观察。",
    ),
    (
        "installation_not_claimed",
        "插件还没归位",
        "在这一页把它认领到一台工位上，或者开一个认领窗口。",
    ),
    (
        "risk_paused",
        "风险暂停生效中",
        "有人按下了刹车。解除之前不会派出任何活。",
    ),
    (
        "daily_quota_reached",
        "今天额度用完了",
        "工位没有离线，明天窗口重置后自然恢复。",
    ),
    (
        "station_daily_budget_reached",
        "今天额度用完了",
        "工位没有离线，明天窗口重置后自然恢复。",
    ),
    (
        "execution_locator_unavailable",
        "缺少可用的签名链接",
        "这篇作品当前没有带签名的已接纳发现链接，已进入冷却重试。",
    ),
    (
        "station_not_accepting",
        "被人暂停了接活",
        "在这一行展开，把「恢复这台工位自动接活」点开。",
    ),
    (
        "station_unavailable",
        "工位或安装已不在岗",
        "确认那台工位还在、插件还连着。",
    ),
    (
        "installation_credential_missing",
        "在岗安装没有有效凭据",
        "让插件重新报到一次，服务端会补发凭据。",
    ),
    (
        "plugin_version_unsupported",
        "插件版本过低",
        "升级这台机器上的插件到当前最低版本以上。",
    ),
    (
        "installation_stale",
        "插件超过 20 分钟没报到",
        "按失联处理。确认那台机器的浏览器还开着、插件还启用着。",
    ),
    (
        "capability_missing",
        "缺这条活要的能力",
        "在这一行展开看「能力」缺哪一项；升级插件或换一台工位。",
    ),
    (
        "account_unbound",
        "还没绑定观察账号",
        "在这一行展开，把这个安装绑定到一个平台账号上。",
    ),
    (
        "account_binding_changed",
        "账号绑定与工单对不上",
        "这一单是按旧绑定发出的；重新发起一次即可。",
    ),
    (
        "account_binding_expired",
        "账号绑定超过 30 天没确认",
        "重新确认一次这个安装绑定的是哪个账号。",
    ),
    (
        "account_eligibility_stale",
        "账号观察已过诊断窗口",
        "这是历史兼容状态；当前不会仅因观察时间阻断接活。实际登录、限制或绑定问题仍会阻断。",
    ),
    (
        "account_cooling",
        "账号正在冷却",
        "平台恢复后，在正常平台页面形成新的账号观察再恢复接活。",
    ),
    (
        "account_needs_login",
        "账号需要重新登录",
        "在这台机器的浏览器里重新登录平台账号。",
    ),
    (
        "account_restricted",
        "账号被平台限制",
        "先在浏览器里处理平台的验证或限制，再让它接活。",
    ),
    (
        "account_unknown",
        "账号资格未知",
        "尚未形成可判定的账号事实，按关闭处理；页面暂时读不到标记不会覆盖已有状态。",
    ),
    (
        "account_busy",
        "这个账号已有活在跑",
        "等它跑完自然轮到下一单。",
    ),
    (
        "station_busy",
        "这台工位已有活在跑",
        "等当前这一份跑完；未观察账号的首单也只允许这一份并发。",
    ),
    (
        "platform_concurrency_reached",
        "平台并发已满",
        "等任一执行许可释放后自然恢复。",
    ),
    (
        "rule_revision_changed",
        "规则改过了",
        "这一单是按旧规则发出的，不会再执行；新规则会自己排下一单。",
    ),
    (
        "monitoring_paused",
        "这个目标的自动观察被暂停了",
        "去观察目标页把它的巡查重新打开。",
    ),
    (
        "authorization_expired_or_revoked",
        "这一单的授权已撤销或过期",
        "工单已终结，不会再重试。需要的话在观察目标页重新发起一次。",
    ),
    (
        "target_not_requestable",
        "这个观察目标已被弃置",
        "弃置的目标不能再发起采集。要用它就先恢复这个目标。",
    ),
];

/// 把一条派发回答翻成「这是什么事」加「你该做什么」。
///
/// 词表里没有的取值原样显示机器取值，并明说它还没有说明——**编一句出来等于假装我们
/// 知道它是什么**。
fn dispatch_answer_label(code: &str, reason: Option<&str>) -> (&'static str, String) {
    if let Some((_, label, hint)) = DISPATCH_ANSWER_EXPLANATIONS
        .iter()
        .find(|(key, _, _)| *key == code)
    {
        return ((*label), (*hint).to_owned());
    }
    if code == "capacity_unknown" {
        return (
            "原因还没有中文说明",
            reason.map_or_else(
                || "服务端只说了「资格未知」，没有给出更具体的原因。".to_owned(),
                |reason| format!("服务端给出的原因码是 {reason}。"),
            ),
        );
    }
    (
        "这条回答还没有中文说明",
        format!("服务端给出的判定是 {code}。"),
    )
}

// ---------------------------------------------------------------------------
// 第三层：今天在跑什么
// ---------------------------------------------------------------------------

/// 六个分组压成一组读数。
///
/// 改写前这里是六个并列区块——正在执行的许可、平台并发、调度通道、上次问活、巡查、
/// 规则排程——其中五个在正常日子里全是 0，占掉半屏在说「现在没事发生」。上次问活
/// 已经进了工位表那一行；剩下的压成一排读数，真有任务在跑时再把它们逐条列出来。
///
/// 「读不到」与 0 分开写：`—` 表示这一项此刻答不出，`0` 表示已确认为零。
fn running_markup(overview: Option<&RuntimeCapacityOverview>) -> String {
    let Some(overview) = overview else {
        return String::new();
    };
    let backlog_readable = !overview.dispatch_backlog.is_empty();
    let sum = |pick: fn(&linggan_evidence::DispatchLaneBacklog) -> i64| {
        if backlog_readable {
            overview.dispatch_backlog.iter().map(pick).sum::<i64>().to_string()
        } else {
            "—".to_owned()
        }
    };
    let platform = overview.platform_dispatch.first().map_or_else(
        || "—".to_owned(),
        |capacity| {
            format!(
                "{live}/{cap}",
                live = capacity.live_leases,
                cap = capacity.concurrent_cap
            )
        },
    );
    let patrol = &overview.patrol;
    let readouts = [
        ("正在执行", sum(|lane| lane.leased_work_orders), "已经派给工位、还没跑完"),
        ("排队等待", sum(|lane| lane.queued_work_orders), "已经生成、还没有工位领走"),
        ("失败重试中", sum(|lane| lane.retry_cooling_work_orders), "上一次没跑成，正在等下一次"),
        ("同时最多能跑", platform, "小红书同一时刻允许的并发上限"),
        (
            "开着自动观察",
            format!("{}/{}", patrol.monitoring_targets, patrol.total_targets),
            "观察目标里有几个在自动巡查",
        ),
        (
            "到点没跑的",
            patrol.overdue_rules.to_string(),
            "已经过了计划时间还没派出去",
        ),
    ];
    // 六格等宽：读数本身是主角（大字号、等宽对齐），标签和注解各降一级。格子数少于
    // 六时最后一格留白，不拉伸已有的格子——被拉伸的格子会读成「它更重要」。
    let cells: String = readouts
        .iter()
        .map(|(label, value, note)| {
            format!(
                r#"<div class="c-op"><dt>{label}</dt><dd>{value}</dd><p>{note}</p></div>"#,
                label = escape(label),
                value = escape(value),
                note = escape(note),
            )
        })
        .collect();

    // 「已展开成任务」与「只是许可」必须分开：许可不是活。
    let live = if overview.live_leases.is_empty() {
        String::new()
    } else {
        let rows: String = overview
            .live_leases
            .iter()
            .map(|lease| {
                let stage = if lease.has_task {
                    "已展开成任务"
                } else {
                    "只是许可，还没展开成任务"
                };
                format!(
                    r#"<div class="c-run-row">
                        <div class="c-run-name"><b>{target}</b><span>{station}</span></div>
                        <div class="c-run-meta"><span>{stage}</span><span>开始 {started}</span><span>预计 {units} 单元</span><span>到期 {expires}</span></div>
                      </div>"#,
                    target = escape(&lease.target_label),
                    station = escape(&lease.station_name),
                    stage = escape(stage),
                    started = escape(moment_without_year(&lease.started_at)),
                    units = lease.estimated_work_units,
                    expires = escape(moment_without_year(&lease.expires_at)),
                )
            })
            .collect();
        format!(r#"<div class="c-run-list"><h3 class="c-run-list-head">正在执行的任务</h3>{rows}</div>"#)
    };

    // 「在跑但没活可派」与「根本没在跑」处置完全不同，页面必须把这个区别说出来，
    // 而不是把两者都写成「未接通」——那会让人去修一个没有坏的东西。
    let note = if patrol.silent_because_nothing_to_patrol() {
        "没有任何目标开着自动观察，因此系统即使正常运行也不会有动静。运行状态见页面顶部。"
    } else {
        "到期判据与调度自己用的是同一个式子。运行状态见页面顶部；这里是它留下的痕迹。"
    };
    format!(
        r#"<section class="c-sect c-run">
              <div class="c-sect-head">
                <span class="c-sect-badge ink">今日</span>
                <h2>今天在跑什么</h2>
                <div class="c-sect-meta">最近一次派出 {dispatched}</div>
              </div>
              <dl class="c-ops">{cells}</dl>
              {live}
              <p class="c-stn-note">{note}</p>
            </section>"#,
        // 从未派出与派出过是两个事实，前者不是 0 也不是未知，是「从未」。
        dispatched = escape(
            patrol
                .last_dispatched_at
                .as_deref()
                .map_or_else(|| "从未".to_owned(), |at| moment_without_year(at).to_owned())
                .as_str()
        ),
        note = escape(note),
    )
}

// ---------------------------------------------------------------------------
// 第四层：自动观察按什么节奏跑
// ---------------------------------------------------------------------------

fn schedule_markup(overview: Option<&RuntimeCapacityOverview>) -> String {
    let Some(overview) = overview else {
        return String::new();
    };
    if overview.monitor_rule_schedules.is_empty() {
        return r#"<section class="c-sect c-run">
              <div class="c-sect-head">
                <span class="c-sect-badge ink">0 条</span>
                <h2>自动观察排程</h2>
                <div class="c-sect-meta">没有开着的规则</div>
              </div>
              <p class="c-stn-empty">当前没有开着的自动观察规则。</p>
            </section>"#
            .to_owned();
    }
    // 「上次开工」是这条规则最近一次真的动过的时刻，取代了稿里那一列「状态」：状态
    // 已经由「最近一单」回答，而开工时刻在此之前从未上过页面——逾期排查要的正是
    // 「计划 18:50、18:55 开了工、之后没有回音」这种断点，缺的就是中间这一列。
    let rows: String = overview
        .monitor_rule_schedules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            format!(
                r#"<div class="c-tg-item c-sched-grid" role="row">
                    <div class="c-tg-cell c-tg-index" role="cell">{index:02}</div>
                    <div class="c-tg-object" role="cell"><div class="c-tg-object-text"><span class="c-tg-title">{target}</span></div></div>
                    <div class="c-tg-cell" role="cell">每 {interval}</div>
                    <div class="c-tg-cell" role="cell">{state}</div>
                    <time class="c-tg-cell c-tg-time" role="cell">{started}</time>
                    <time class="c-tg-cell c-tg-time" role="cell">{planned}</time>
                    <time class="c-tg-cell c-tg-time" role="cell">{receipt}</time>
                    <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                    <div class="c-tg-cell c-sched-act" role="cell"><a href="/collection/targets">观察目标</a></div>
                  </div>"#,
                index = index + 1,
                target = escape(&rule.target_label),
                interval = escape(&interval_label(rule.interval_seconds)),
                state = escape(work_order_state_label(rule.latest_work_order_state.as_deref())),
                started = escape(&moment_or_never(rule.last_attempt_started_at.as_deref())),
                planned = escape(&moment_or_never(rule.last_scheduled_for.as_deref())),
                receipt = escape(&moment_or_never(rule.last_receipt_at.as_deref())),
                next = escape(&moment_or_never(rule.next_run_at.as_deref())),
            )
        })
        .collect();
    format!(
        r#"<section class="c-sect c-run">
              <div class="c-sect-head">
                <span class="c-sect-badge ink">排程</span>
                <h2>自动观察排程</h2>
                <div class="c-sect-meta">{count} 条在跑</div>
              </div>
              <div class="c-tg-table-scroll" role="table" aria-label="自动观察排程">
                <div class="c-tg-table-head c-sched-grid" role="row"><span role="columnheader">序</span><span role="columnheader">目标</span><span role="columnheader">间隔</span><span role="columnheader">最近一单</span><span role="columnheader">上次开工</span><span role="columnheader">上次计划</span><span role="columnheader">上次拿回</span><span role="columnheader">下次</span><span role="columnheader">操作</span></div>
                <div class="c-stn-list" role="rowgroup">{rows}</div>
              </div>
            </section>"#,
        count = overview.monitor_rule_schedules.len(),
    )
}

/// 「从未」不是 0，也不是未知。
fn moment_or_never(value: Option<&str>) -> String {
    value.map_or_else(
        || "从未".to_owned(),
        |value| moment_without_year(value).to_owned(),
    )
}

// ---------------------------------------------------------------------------
// 页尾：登记一台工位，以及还没归位的插件安装
// ---------------------------------------------------------------------------

fn footer_markup(
    roster: Option<(&[StationOverview], &[UnclaimedInstallation])>,
    now_minutes: i64,
    error: Option<&str>,
) -> String {
    let (stations, unclaimed): (&[StationOverview], &[UnclaimedInstallation]) =
        roster.unwrap_or((&[], &[]));
    // 登记表单住在一个居中的弹窗里，由控制条上的「新增工位」打开。它曾经是页尾一行里的
    // 一个块（`#station-register`）加一条 `href="#station-register"` 的锚点；用户
    // 2026-09-20 指示改成弹窗。表单本身一个字段没动。
    //
    // **登记失败时弹窗必须自己开着**。一次失败的 POST 会整页重渲染：如果窗还是关的，用户
    // 看到的就是「页面回到了原样、什么也没说」，而错误块恰好就在这个关着的窗里。所以
    // `error` 有值时在这里就不输出 `hidden`。
    format!(
        r#"<section class="c-stn-footer">
              <div class="c-stn-overlay" data-station-register-overlay{open}>
                <section class="c-stn-modal" id="station-register" role="dialog" aria-modal="true" aria-labelledby="c-station-register-title">
                  <header class="c-stn-modal-head">
                    <h3 id="c-station-register-title">登记一台工位</h3>
                    <button class="c-btn-quiet c-stn-modal-close" type="button" data-station-register-close>关闭</button>
                  </header>
                  {failure}
                  <form class="c-stn-form" method="post" action="/collection/runtime/stations">
                    <label for="station-name">名字要能让你一眼认出是哪台机器</label>
                    <input id="station-name" name="display_name" required maxlength="60" value="本机 Chrome" />
                    <button class="c-btn-primary" type="submit">登记</button>
                  </form>
                  <p class="c-stn-note">{note}</p>
                </section>
              </div>
              {unclaimed}
            </section>"#,
        open = if error.is_some() { "" } else { " hidden" },
        failure = failure_markup(error),
        note = escape(NO_PLATFORM_ACCESS_NOTE),
        unclaimed = unclaimed_markup(stations, unclaimed, now_minutes),
    )
}

/// 还没归位的插件安装。
///
/// 默认折叠，且**近期与历史分开数**：一台机器每升级一次插件就留下一条旧安装，
/// 它们永远不会被认领。把它们和「今天刚装好、正等你指认」的那一条混在一个数字里，
/// 这个数字就只会往上涨，再也不代表任何要处理的事。
fn unclaimed_markup(
    stations: &[StationOverview],
    unclaimed: &[UnclaimedInstallation],
    now_minutes: i64,
) -> String {
    if unclaimed.is_empty() {
        return String::new();
    }
    let now = now_minutes;
    let (recent, historical): (Vec<_>, Vec<_>) = unclaimed
        .iter()
        .partition(|installation| installation_is_recent(&installation.first_seen_at, now));

    let recent_rows = if recent.is_empty() {
        r#"<p class="c-stn-note">最近 7 天没有新的插件安装在等你指认。</p>"#.to_owned()
    } else {
        recent
            .iter()
            .map(|installation| unclaimed_row(installation, stations))
            .collect::<String>()
    };
    let historical_rows = if historical.is_empty() {
        String::new()
    } else {
        format!(
            r#"<p class="c-stn-note">下面 {count} 条是更早以前的安装。同一台机器每升级一次插件就会留下一条，它们不会被派活，通常也不需要认领。</p>{rows}"#,
            count = historical.len(),
            rows = historical
                .iter()
                .map(|installation| unclaimed_row(installation, stations))
                .collect::<String>(),
        )
    };

    format!(
        r#"<details class="c-stn-block c-stn-unclaimed">
              <summary><b>还没归位的插件安装</b><span>{recent_count} 条最近报到 · 共 {total} 条</span></summary>
              <p class="c-stn-note">这些插件报到了，但还没人说它们是哪台工位。它们不会被派活。认领窗口开着时新安装会自动归位，关着就停在这里等你指认。</p>
              {recent_rows}
              {historical_rows}
            </details>"#,
        recent_count = recent.len(),
        total = unclaimed.len(),
    )
}

fn unclaimed_row(installation: &UnclaimedInstallation, stations: &[StationOverview]) -> String {
    let browser = installation
        .browser_label
        .as_deref()
        .unwrap_or("未知浏览器");
    // 没有已登记的工位时不给认领控件：没有可认领到的去处，给了也是点了没反应。
    let claim = if stations.is_empty() {
        r#"<span class="c-tg-truth c-tg-neutral">待认领</span>"#.to_owned()
    } else {
        let options: String = stations
            .iter()
            .map(|station| {
                format!(
                    r#"<option value="{value}">{name}</option>"#,
                    value = station.station_ref,
                    name = escape(&station.display_name),
                )
            })
            .collect();
        format!(
            r#"<form class="c-stn-form" method="post" action="/collection/runtime/claims">
                  <input type="hidden" name="installation_ref" value="{installation}" />
                  <select name="station_ref" aria-label="认领到哪台工位">{options}</select>
                  <button class="c-btn-quiet" type="submit">认领</button>
                </form>"#,
            installation = installation.installation_ref,
        )
    };
    format!(
        r#"<div class="c-stn-unclaimed-row">
                <div class="c-tg-object-text"><span class="c-tg-title">插件 {version}</span><span class="c-tg-meta">{browser} · 首次报到 {first_seen}</span></div>
                {claim}
              </div>"#,
        version = escape(&installation.plugin_version),
        browser = escape(browser),
        first_seen = escape(moment_without_year(&installation.first_seen_at)),
    )
}

// ---------------------------------------------------------------------------
// 右侧抽屉：运行概览
// ---------------------------------------------------------------------------

/// 右侧「运行概览」抽屉。
///
/// 外观、位置与开合方式照 v7.2 稿：固定在右侧、贴边一个把手、`Esc` 可关。**里面换成
/// 这一页真读得到的四块。** 稿里那三块——回传成功率 98.6%、补采入口 26 条、失败原因
/// TOP 3——在本系统里没有对应事实：全库只有三个 `failure_code` 且没有聚合读模型，
/// 也没有 24 小时全局速率与逐时序列。编一个出来，等于把「不知道」画成「知道」。
///
/// 与稿子有意的第二处不同：**默认关闭**。稿子为了展示把它画成开着的，但那是一个覆盖层，
/// 第一帧就压在被覆盖的内容上、且没有脚本时永远关不掉。开合方式、把手位置与动效都
/// 与稿子一致，只是初始态取关。关着的时候整个抽屉带 `inert`，屏幕外的链接不进 Tab 序列。
///
/// 四块与主页面**同源**：通道判定只算一次传进来，队列与风险暂停取自同一次读取。
/// 抽屉里不会出现和上面读数打架的第二套数。
fn runtime_drawer_markup(
    overview: Option<&RuntimeCapacityOverview>,
    lanes: Option<&Vec<LaneRow>>,
    capabilities: &CapabilityMatrix,
    stations: Option<&[StationOverview]>,
) -> String {
    // 「派过」与「成了」是两个事实，并排放在最底下。只报派出不报拿回，正是让一个
    // 一直派得出去、什么都拿不回来的系统看起来在正常工作的那种写法。
    let (dispatched, succeeded) = overview.map_or_else(
        || ("读不到".to_owned(), "读不到".to_owned()),
        |overview| {
            (
                moment_or_never(overview.patrol.last_dispatched_at.as_deref()),
                moment_or_never(overview.patrol.last_succeeded_at.as_deref()),
            )
        },
    );
    format!(
        r#"<aside class="c-rdrawer" id="c-runtime-drawer" aria-label="运行概览" inert>
              <div class="c-rdrawer-head">
                <span class="c-rdrawer-title">运行概览</span>
                <button class="c-rdrawer-close" type="button" data-runtime-drawer-close aria-label="关闭运行概览">×</button>
              </div>
              <div class="c-rdrawer-body">
                {policy}
                {results}
                {routes}
                {backlog}
              </div>
              <p class="c-rdrawer-foot">最近一次派出 {dispatched} · 最近一次拿回 {succeeded}</p>
            </aside>"#,
        policy = drawer_policy_markup(lanes),
        results = drawer_results_markup(capabilities, stations),
        routes = drawer_routes_markup(),
        backlog = drawer_backlog_markup(overview),
    )
}

/// 抽屉里一块的固定外壳。四块共用，免得四段各自长出一个表头。
fn drawer_section(title: &str, body: &str) -> String {
    format!(
        r#"<section class="c-rd-sec"><h3 class="c-rd-sec-head">{title}</h3>{body}</section>"#,
        title = escape(title),
    )
}

/// 第一块：规则策略。
///
/// 逐条列出通道**要去干什么**加它现在的判定。上面那块仪器面在通道被挡住时写的是
/// 原因，于是「这条通道本来是干什么的」在那一行被挤掉了；这里把它补回来。状态词与
/// 判定码都走同一对函数（`lane_state` / `reason_code_markup`），两处不可能给出两个答案。
fn drawer_policy_markup(lanes: Option<&Vec<LaneRow>>) -> String {
    let Some(rows) = lanes else {
        return drawer_section(
            "规则策略",
            r#"<p class="c-rd-note">通道判定此刻读不到。这不表示没有通道，只表示这一页答不出——两者的处置不同。</p>"#,
        );
    };
    let open = rows.iter().filter(|row| row.available).count();
    let cards: String = rows
        .iter()
        .map(|row| {
            let (state_class, label) = lane_state(row);
            format!(
                r#"<div class="c-rd-rule">
                    <div class="c-rd-rule-head"><b>{name}</b><em class="c-lane-state {state_class}">{label}</em>{code}</div>
                    <p>{needs}</p>
                  </div>"#,
                name = escape(&row.name),
                needs = escape(row.needs),
                code = reason_code_markup(row),
            )
        })
        .collect();
    drawer_section(
        "规则策略",
        &format!(
            r#"<p class="c-rd-meta">共 {total} 条通道 · {open} 条现在能接活</p>{cards}"#,
            total = rows.len(),
        ),
    )
}

/// 第二块：近 7 天执行结果（深色卡）。
///
/// 窗口不是页面自己划的：能力读数本来就按 7 天窗口取（`CAPABILITY_WINDOW_DAYS`），
/// 这里只是把各台工位的计数加起来。三类数**分开放，不合成一个「失败数」**——
/// 「插件说这项在这里跑不了」是能力问题，「这一次超时了」不是，合成一个会让一次网络
/// 抖动看起来像能力缺陷。
///
/// **只读到一部分工位时先说清几台有读数**：把三台的和当成全系统的和，是这一页最容易
/// 犯、也最难被看出来的那种错。整个窗口一条记录都没有时说「没有留下记录」，不显示 0。
fn drawer_results_markup(
    capabilities: &CapabilityMatrix,
    stations: Option<&[StationOverview]>,
) -> String {
    let (mut read, mut picked, mut capability_failures, mut execution_failures) = (0usize, 0i64, 0i64, 0i64);
    for rows in capabilities.values() {
        read += 1;
        for row in rows {
            picked += row.successes;
            capability_failures += row.capability_failures;
            execution_failures += row.execution_failures;
        }
    }
    let coverage = stations.map_or_else(
        || format!("{read} 台有读数，工位总数此刻读不到"),
        |stations| format!("{read} / {} 台工位有读数", stations.len()),
    );
    let body = if read == 0 {
        r#"<p class="c-rd-note">这一轮没有任何一台工位的能力读数是读到的。这不表示它们没干过活，只表示现在答不出。</p>"#
            .to_owned()
    } else if picked + capability_failures + execution_failures == 0 {
        // 0 是「已确认为零」，这里要说的正是这个：窗口内确实一条记录都没有。
        r#"<p class="c-rd-note">这 7 天里，有读数的工位没有留下任何执行记录。</p>"#.to_owned()
    } else {
        format!(
            r#"<dl class="c-rd-figures">
                <div><dt>拿回的采集包</dt><dd>{picked}</dd><p>这 7 天从这些工位收上来的包</p></div>
                <div><dt>插件说这项跑不了</dt><dd>{capability_failures}</dd><p>工位明确回答这项在这里干不了，是能力问题</p></div>
                <div><dt>这一次没跑成</dt><dd>{execution_failures}</dd><p>超时、标签页不可用等，能力本身没问题</p></div>
              </dl>"#
        )
    };
    drawer_section(
        "近 7 天执行结果",
        &format!(
            r#"<div class="c-rd-card"><p class="c-rd-card-meta">{coverage}</p>{body}</div>"#,
            coverage = escape(&coverage),
        ),
    )
}

/// 第三块：补采与失败入口。
///
/// 两个都是真链接，落到真存在的地方，指向哪就说哪：补采缺口在观察目标页的目标详情里
/// （那里有「补采缺口」这个操作），上一次为什么没成在任务页（工单时间线上带着
/// `last_dispatch_failure_code`）。稿里这一块挂着「26 条待检查」这类计数，一个都不出现：
/// 那两个页面各自的筛选口径不在这条链路上，报一个数出来只能是编的。
fn drawer_routes_markup() -> String {
    drawer_section(
        "补采与失败入口",
        r#"<div class="c-rd-routes">
              <a class="c-rd-route" href="/collection/targets"><b>补采缺口</b><span>在观察目标页打开目标详情，能看到缺了多少、点一下排入补采</span></a>
              <a class="c-rd-route" href="/collection/tasks"><b>失败与重试</b><span>在任务页看每张工单停在哪一步、上一次为什么没成</span></a>
            </div>"#,
    )
}

/// 第四块：积压与重试。
///
/// 三个总数已经在上面「今天在跑什么」里，这里**不重报**，只补上面没有的两件：
/// 排在最前面的那一单从什么时候开始等，以及生效中的风险暂停各自是什么。后者是
/// 「为什么没有在重试」的答案——上面那四格只说有几条暂停，不说暂停的理由。
fn drawer_backlog_markup(overview: Option<&RuntimeCapacityOverview>) -> String {
    let Some(overview) = overview else {
        return drawer_section(
            "积压与重试",
            r#"<p class="c-rd-note">这一轮没有读到队列事实，因此答不出积压与重试的现状。</p>"#,
        );
    };
    // 「最久的一单还在等」比「一共积压几张」更能说明等了多久：一张等了两天的单和
    // 五张刚排进来的单，处置完全不同。取原始时刻比大小（`YYYY-MM-DD HH:MM` 的字典序
    // 与时间序一致），再裁掉年份显示。
    let oldest = overview
        .dispatch_backlog
        .iter()
        .filter_map(|lane| lane.oldest_ready_at.as_deref())
        .min()
        .map_or_else(
            || "没有在等的一单".to_owned(),
            |at| moment_without_year(at).to_owned(),
        );
    let pauses = if overview.risk_pauses.is_empty() {
        r#"<p class="c-rd-note">当前没有生效中的风险暂停。</p>"#.to_owned()
    } else {
        let rows: String = overview
            .risk_pauses
            .iter()
            .map(|pause| {
                format!(
                    r#"<div class="c-rd-pause">
                        <b>{reason}</b>
                        <span>由 {by} 于 {at} 暂停</span>
                      </div>"#,
                    reason = escape(&pause.reason),
                    by = escape(&pause.paused_by),
                    at = escape(moment_without_year(&pause.paused_at)),
                )
            })
            .collect();
        format!(r#"<div class="c-rd-pauses">{rows}</div>"#)
    };
    drawer_section(
        "积压与重试",
        &format!(
            r#"<p class="c-rd-meta">最久在等的一单 · <b>{oldest}</b></p>{pauses}"#,
            oldest = escape(&oldest),
        ),
    )
}

/// 上一次动作失败时说明原因。
///
/// 失败必须看得见。跳转回来却什么都不说，会让人以为动作成功了——那比「点了没反应」更糟，
/// 因为它会让人以为系统里有一台并不存在的工位。
fn failure_markup(error: Option<&str>) -> String {
    let Some(code) = error else {
        return String::new();
    };
    let explanation = match code {
        "station_rejected" => {
            "工位没有登记成功。最常见的原因是名字与某台在册工位重复——名字要能让你一眼认出是哪台机器，因此不允许重名。"
        }
        "station_name_rejected" => "工位名称没有更新。名字不能为空，且不能与另一台在册工位重复。",
        "claim_window_rejected" => "认领窗口没有打开。该工位可能已被停用。",
        "close_window_rejected" => "认领窗口没有关闭。它可能已经自己过期了。",
        "retire_rejected" => "工位没有停用成功。它可能已经处于停用状态。",
        "claim_rejected" => "认领没有成功。这台工位可能已有在岗安装，或该安装已被取代。",
        _ => "上一次动作没有完成。",
    };
    format!(
        r#"<p class="c-stn-failure"><b>没有完成</b>{explanation}</p>"#,
        explanation = escape(explanation),
    )
}

fn work_order_state_label(value: Option<&str>) -> &'static str {
    match value {
        Some("queued") => "等待领取",
        Some("leased") => "执行中",
        Some("completed") => "已完成",
        Some("cancelled") => "已取消",
        Some("legacy") => "历史状态",
        _ => "还没排过",
    }
}

fn interval_label(seconds: i32) -> String {
    match seconds {
        21_600 => "6 小时".to_owned(),
        43_200 => "12 小时".to_owned(),
        86_400 => "24 小时".to_owned(),
        172_800 => "2 天".to_owned(),
        604_800 => "7 天".to_owned(),
        _ => "未知间隔".to_owned(),
    }
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use linggan_contracts::Capacity;
    use linggan_evidence::{
        ActiveRiskPause, DispatchLaneBacklog, LaneVerdict, LiveLease, MonitorRuleSchedule,
        PatrolOutlook, PlatformDispatchCapacity,
    };
    use uuid::Uuid;

    /// 固定时刻，让「最近 / 更早」的分组在测试里是确定的。
    ///
    /// 从同一个解析函数算出来，而不是写一个魔数：写死的分钟数会在解析规则变动时
    /// 悄悄错位，而错位后测试仍然是绿的。
    fn test_now() -> i64 {
        minutes_since_epoch("2026-09-13 12:00").expect("固定时刻可解析")
    }

    fn base() -> String {
        format!("before{EMPTY_STATE_OPEN}这一栏不需要你做任何事{EMPTY_STATE_CLOSE}after")
    }

    fn station(active_version: Option<&str>, superseded: i64) -> StationOverview {
        StationOverview {
            station_ref: Uuid::new_v4(),
            display_name: "MacBook Chrome".to_owned(),
            daily_work_quota: 200,
            claim_window_open: false,
            active_plugin_version: active_version.map(str::to_owned),
            active_browser_label: Some("Chrome".to_owned()),
            active_last_seen_at: Some("2026-08-27 02:58".to_owned()),
            superseded_count: superseded,
            daily_notes_used: 0,
            last_dispatch_answer_at: None,
            last_dispatch_answer_code: None,
            last_dispatch_answer_reason: None,
        }
    }

    fn overview(lanes: Vec<LaneVerdict>) -> RuntimeCapacityOverview {
        RuntimeCapacityOverview {
            lanes,
            risk_pauses: Vec::new(),
            registered_stations: 1,
            staffed_stations: 1,
            patrol: PatrolOutlook {
                total_targets: 0,
                monitoring_targets: 0,
                due_now: 0,
                overdue_rules: 0,
                oldest_due_at: None,
                last_dispatched_at: None,
                last_succeeded_at: None,
            },
            live_leases: Vec::new(),
            platform_dispatch: Vec::new(),
            dispatch_backlog: Vec::new(),
            monitor_rule_schedules: Vec::new(),
        }
    }

    fn lane(zh: &'static str, capacity: Capacity) -> LaneVerdict {
        LaneVerdict {
            zh,
            lane: "deep_archive",
            needs: "作品清单",
            capacity,
        }
    }

    fn available() -> Capacity {
        Capacity::Available {
            station_ref: "s".to_owned(),
        }
    }

    fn resource(station_ref: Uuid, binding_state: &str) -> RuntimeResourceView {
        RuntimeResourceView {
            station_ref,
            station_name: "MacBook Chrome".to_owned(),
            accepting_tasks: true,
            installation_ref: Some(Uuid::new_v4()),
            plugin_version: Some("0.8.48".to_owned()),
            last_seen_at: Some("2026-09-13 17:50".to_owned()),
            has_valid_credential: true,
            account_ref: Some(Uuid::new_v4()),
            bound_account_ref: None,
            binding_state: binding_state.to_owned(),
            eligibility_ref: None,
            eligibility_state: None,
            eligibility_reason_code: None,
            eligibility_observed_at: None,
            account_has_live_lease: false,
        }
    }

    fn control_lane(label: &'static str, lane: &'static str, available: bool) -> RuntimeLaneControlView {
        RuntimeLaneControlView {
            label,
            target_kind: "creator",
            lane,
            queueable: false,
            available,
            reason_code: if available { None } else { Some("account_needs_login") },
            reason: if available {
                None
            } else {
                Some("观察账号需要重新登录。".to_owned())
            },
            station_ref: Some("6962b234-7ba2-40e7-aedf-69ee4583e1dd".to_owned()),
        }
    }

    fn render(
        overview: Option<&RuntimeCapacityOverview>,
        stations: &[StationOverview],
        unclaimed: &[UnclaimedInstallation],
        capabilities: &CapabilityMatrix,
        control: Option<&RuntimeControl<'_>>,
    ) -> String {
        render_runtime(
            &base(),
            overview,
            stations,
            unclaimed,
            capabilities,
            control,
            test_now(),
            None,
        )
    }

    // -----------------------------------------------------------------------
    // 一台工位一行：本轮改写的中心保证
    // -----------------------------------------------------------------------

    /// 改写前同一台机器的事实散在三处，人得滚三个地方来回对照。
    #[test]
    fn one_station_is_one_row_and_everything_about_it_hangs_off_that_row() {
        let station = station(Some("0.8.48"), 3);
        let resources = vec![resource(station.station_ref, "unconfirmed")];
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &resources,
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station.clone()],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );

        // 这台工位只占一行。
        assert_eq!(html.matches("c-tg-item c-tg-station-grid").count(), 1);
        assert_eq!(html.matches("data-station-entry").count(), 1);
        // 这一行里读得出状态、额度、接活、账号、能力、上次问活。
        for expected in ["在岗", "0/200", "自动", "待确认", "读不到", "从未问过活"] {
            assert!(html.contains(expected), "缺少 {expected}");
        }
        // 这台工位的四个动作都挂在这一行展开里，不在页面别处。
        for action in [
            "/collection/runtime/stations/name",
            "/collection/runtime/retire",
            "/collection/runtime/accepting",
            "/collection/runtime/account-bindings",
        ] {
            assert!(html.contains(action), "缺少动作 {action}");
        }
    }

    /// `<form>` 不是 `<summary>` 的合法内容，而且放进去会让一次点击既提交又折叠。
    #[test]
    fn the_clickable_row_never_contains_a_form() {
        let station = station(Some("0.8.48"), 0);
        let resources = vec![resource(station.station_ref, "unconfirmed")];
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &resources,
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        let start = html.find("<summary class=\"c-tg-item").expect("行是 summary");
        let end = html[start..].find("</summary>").expect("summary 闭合") + start;
        assert!(!html[start..end].contains("<form"));
        assert!(!html[start..end].contains("<button"));
    }

    /// 改写前顶部说三条通道、下面说两条，同一个 `deep_archive` 一处叫「基线建档」、
    /// 一处叫「批量建档」。页面只允许有一个答案。
    #[test]
    fn a_lane_is_never_named_twice_on_the_same_page() {
        let lanes = vec![
            control_lane("创作者基线", "deep_archive", true),
            control_lane("创作者巡检", "patrol", true),
        ];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![
                lane("批量建档", available()),
                lane("巡检", available()),
            ])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert_eq!(html.matches("data-capacity-state").count(), 2);
        assert!(html.contains("创作者基线"));
        // 产能概览那一套名字不再同时出现。
        assert!(!html.contains("批量建档"));
    }

    /// 通道判定读不到时才退回产能概览，且退回后仍然只有一套名字。
    #[test]
    fn the_capacity_overview_is_only_a_fallback_for_unreadable_lane_control() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert_eq!(html.matches("data-capacity-state").count(), 1);
        assert!(html.contains("基线建档"));
    }

    // -----------------------------------------------------------------------
    // 语言：页面不用工程内部命名说话
    // -----------------------------------------------------------------------

    /// LANG-05：描述性标签一律中文，工程内部命名不进界面。
    #[test]
    fn the_page_never_speaks_in_internal_engineering_names() {
        let station = station(Some("0.8.48"), 9);
        let resources = vec![resource(station.station_ref, "current")];
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &resources,
            account_observation_available: true,
        };
        let mut capacity = overview(vec![lane("基线建档", available())]);
        capacity.monitor_rule_schedules = vec![MonitorRuleSchedule {
            target_label: "木可可".to_owned(),
            interval_seconds: 86_400,
            last_scheduled_for: None,
            last_attempt_started_at: None,
            latest_work_order_state: None,
            next_run_at: None,
            last_receipt_at: None,
        }];
        let html = render(
            Some(&capacity),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        for forbidden in [
            "准入第 5 问",
            "控制资格",
            "有界控制事实",
            "同一评估器",
            "lane 上限",
            "WorkOrder",
            "Attempt",
            "Eligibility",
            "租约",
            "调度通道",
            "规则排程",
            "开发期工具",
        ] {
            assert!(!html.contains(forbidden), "内部术语泄漏：{forbidden}");
        }
    }

    /// 工位的机器标识不该整串糊在通道行上。改写前它在那里出现了三次。
    #[test]
    fn a_lane_row_never_prints_a_raw_station_identifier() {
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(!html.contains("6962b234-7ba2-40e7-aedf-69ee4583e1dd"));
    }

    // -----------------------------------------------------------------------
    // 状态诚实：读不到 ≠ 没有，未验证 ≠ 降级，许可 ≠ 活
    // -----------------------------------------------------------------------

    #[test]
    fn nothing_registered_still_offers_the_one_thing_the_person_can_do() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("还没有登记任何工位"));
        assert!(html.contains("登记一台工位"));
        // 原来的「等工程」空态已被替换掉，不是被包在新内容外面。
        assert!(!html.contains("这一栏不需要你做任何事"));
    }

    #[test]
    fn the_page_never_writes_a_connection_state_by_hand() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(Some("0.8.4"), 0)],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        for stale in [
            "调度器未接通",
            "尚未接上工位",
            "队列、租约、回执尚不存在",
            "等工程",
        ] {
            assert!(!html.contains(stale), "写死的状态句仍在：{stale}");
        }
    }

    #[test]
    fn a_blocked_lane_names_the_missing_resource() {
        let lanes = vec![control_lane("创作者巡检", "patrol", false)];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(html.contains("接不了活"));
        assert!(html.contains("观察账号需要重新登录"));
        assert!(html.contains("account_needs_login"));
        assert!(html.contains("data-capacity-state=\"blocked\""));
    }

    #[test]
    fn one_lane_running_is_not_reported_as_total_failure() {
        let lanes = vec![
            control_lane("创作者基线", "deep_archive", true),
            control_lane("创作者巡检", "patrol", false),
        ];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(html.contains("部分接不了活"));
        assert!(!html.contains(">接不了活<"));
    }

    #[test]
    fn unreadable_capacity_is_not_reported_as_no_capacity() {
        let html = render(None, &[], &[], &CapabilityMatrix::new(), None);
        assert!(html.contains("现在读不到「能不能接活」的判定"));
        assert!(html.contains("c-verdict-unknown"));
        assert!(!html.contains("c-verdict-blocked"));
    }

    #[test]
    fn unreadable_roster_is_not_reported_as_no_registered_stations() {
        let html = render_runtime_with_unreadable_roster(
            &base(),
            Some(&overview(vec![lane("基线建档", available())])),
            None,
            test_now(),
            None,
        );
        assert!(html.contains("工位列表当前读不到"));
        assert!(!html.contains("还没有登记任何工位"));
    }

    #[test]
    fn a_station_with_no_plugin_reads_as_vacant_not_broken() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(None, 0)],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("空缺"));
        assert!(html.contains("c-tg-neutral"));
    }

    #[test]
    fn many_reinstalls_stay_one_station_with_a_visible_count() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(Some("0.8.4"), 12)],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("换过 12 次插件"));
        assert_eq!(html.matches("data-station-entry").count(), 1);
    }

    #[test]
    fn a_lease_is_never_reported_as_work_in_progress() {
        let mut capacity = overview(vec![lane("基线建档", available())]);
        capacity.live_leases = vec![LiveLease {
            station_name: "MacBook Chrome".to_owned(),
            lane: "deep_archive".to_owned(),
            target_label: "木可可".to_owned(),
            started_at: "2026-09-13 10:00".to_owned(),
            expires_at: "2026-09-13 11:00".to_owned(),
            estimated_work_units: 20,
            has_task: false,
        }];
        let html = render(Some(&capacity), &[], &[], &CapabilityMatrix::new(), None);
        assert!(html.contains("只是许可，还没展开成任务"));
    }

    // -----------------------------------------------------------------------
    // 上次问活
    // -----------------------------------------------------------------------

    #[test]
    fn the_last_dispatch_answer_is_shown_in_chinese_instead_of_a_machine_code() {
        let mut station = station(Some("0.8.4"), 0);
        station.last_dispatch_answer_code = Some("installation_stale".to_owned());
        station.last_dispatch_answer_at = Some("2026-09-13 17:50".to_owned());
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("插件超过 20 分钟没报到"));
        assert!(html.contains("确认那台机器的浏览器还开着"));
        assert!(!html.contains(">installation_stale<"));
    }

    #[test]
    fn a_station_that_never_asked_is_not_reported_as_having_no_work() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(Some("0.8.4"), 0)],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("从未问过活"));
        assert!(!html.contains("队列里没有活"));
    }

    #[test]
    fn an_answer_with_no_chinese_explanation_says_so_instead_of_inventing_one() {
        let mut station = station(Some("0.8.4"), 0);
        station.last_dispatch_answer_code = Some("brand_new_reason".to_owned());
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("这条回答还没有中文说明"));
        assert!(html.contains("brand_new_reason"));
    }

    // -----------------------------------------------------------------------
    // 能力：摘要三态互不替代
    // -----------------------------------------------------------------------

    fn capability(name: &str, declared: bool, successes: i64, capability_failures: i64) -> StationCapability {
        StationCapability {
            capability: name.to_owned(),
            declared,
            successes,
            last_success_at: if successes > 0 {
                Some("2026-09-13 13:50".to_owned())
            } else {
                None
            },
            capability_failures,
            execution_failures: 0,
            last_failure_at: None,
        }
    }

    /// 「有插件认领这台工位」与「这台工位现在还连着」是两回事。
    ///
    /// 2026-09-13：插件后台进程睡了 2 小时 26 分，页面首屏已经正确地说「接不了活 ·
    /// installation_stale」，这一列却还写着「在岗」——同一屏上两个互相矛盾的答案。
    #[test]
    fn a_station_that_stopped_reporting_is_not_still_called_on_duty() {
        let now = test_now();
        let mut fresh = station(Some("0.8.49"), 0);
        fresh.active_last_seen_at = Some("2026-09-13 11:55".to_owned());
        let mut stale = station(Some("0.8.49"), 0);
        stale.display_name = "失联的那台".to_owned();
        stale.active_last_seen_at = Some("2026-09-13 09:30".to_owned());

        let html = render_runtime(
            &base(),
            Some(&overview(vec![lane("基线建档", available())])),
            &[fresh, stale],
            &[],
            &CapabilityMatrix::new(),
            None,
            now,
            None,
        );
        assert!(html.contains(">在岗<"), "还在报到的那台仍是在岗");
        assert!(html.contains(">失联<"), "两小时没报到的那台必须说失联");
        // 失联不是「坏了」也不是「空缺」：工位还在，只是现在连不上。
        assert!(!html.contains(">空缺<"));
    }

    /// 阈值必须与准入判定用同一个数，否则页面与判定会在边界上各说各话。
    #[test]
    fn the_page_uses_the_same_freshness_threshold_as_the_admission_check() {
        let now = test_now();
        let mut just_inside = station(Some("0.8.49"), 0);
        // 19 分钟前：还在 20 分钟窗口内。
        just_inside.active_last_seen_at = Some("2026-09-13 11:41".to_owned());
        let inside = render_runtime(
            &base(),
            Some(&overview(vec![lane("基线建档", available())])),
            &[just_inside],
            &[],
            &CapabilityMatrix::new(),
            None,
            now,
            None,
        );
        assert!(inside.contains(">在岗<"));

        let mut just_outside = station(Some("0.8.49"), 0);
        // 21 分钟前：越过 CONTROL_FRESHNESS_MINUTES。
        just_outside.active_last_seen_at = Some("2026-09-13 11:39".to_owned());
        let outside = render_runtime(
            &base(),
            Some(&overview(vec![lane("基线建档", available())])),
            &[just_outside],
            &[],
            &CapabilityMatrix::new(),
            None,
            now,
            None,
        );
        assert!(outside.contains(">失联<"));
        assert_eq!(CONTROL_FRESHNESS_MINUTES, 20);
    }

    /// 读不到报到时间时不许编：既不说「失联」，也不给「在岗」那个肯定语气。
    #[test]
    fn an_unreadable_heartbeat_is_not_rendered_as_either_answer() {
        let mut unknown = station(Some("0.8.49"), 0);
        unknown.active_last_seen_at = None;
        let html = render_runtime(
            &base(),
            Some(&overview(vec![lane("基线建档", available())])),
            &[unknown],
            &[],
            &CapabilityMatrix::new(),
            None,
            test_now(),
            None,
        );
        assert!(html.contains(">在岗<"));
        assert!(!html.contains(">失联<"));
        assert!(html.contains("尚未报到"));
    }

    /// 关键词没有作品清单。两条巡检通道写成同一句话，就是把一个关键词说成创作者。
    #[test]
    fn a_keyword_patrol_does_not_claim_to_read_a_work_directory() {
        let mut keyword = control_lane("关键词巡检", "patrol", true);
        keyword.target_kind = "keyword";
        let lanes = vec![control_lane("创作者巡检", "patrol", true), keyword];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &[],
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(html.contains("查一遍搜索结果有没有新命中"));
        assert!(html.contains("查一遍作品清单有没有新作品"));
    }

    /// 没有登记中文名的能力，机器名已经当了标题，不再重复印一遍。
    #[test]
    fn an_unlabelled_capability_prints_its_machine_name_once() {
        let station = station(Some("0.8.48"), 0);
        let mut capabilities = CapabilityMatrix::new();
        capabilities.insert(
            station.station_ref,
            vec![
                capability("batch_checkpoint", false, 0, 0),
                capability("author_profile", true, 3, 0),
            ],
        );
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &capabilities,
            None,
        );
        assert_eq!(html.matches("batch_checkpoint").count(), 1);
        // 有中文名的那一项仍然中文在前、机器名相邻。
        assert!(html.contains("作者档案"));
        assert!(html.contains("<code>author_profile</code>"));
    }

    #[test]
    fn the_capability_summary_keeps_unverified_degraded_and_unreadable_apart() {
        let station = station(Some("0.8.48"), 0);
        let mut capabilities = CapabilityMatrix::new();
        capabilities.insert(
            station.station_ref,
            vec![
                capability("author_profile", true, 17, 0),
                capability("comments", true, 0, 2),
                capability("xhs.pageAccess", true, 0, 0),
                capability("batch_checkpoint", false, 0, 0),
            ],
        );
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station.clone()],
            &[],
            &capabilities,
            None,
        );
        assert!(html.contains("1 就绪 · 1 降级 · 1 未验证"));
        assert!(html.contains("近 7 天没派过这种活"));
        assert!(html.contains("在岗插件没有声明这一项"));

        // 读不到整块 ≠ 一排「不支持」。
        let unreadable = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(unreadable.contains("能力矩阵当前读不到"));
        assert!(!unreadable.contains("不支持"));
    }

    #[test]
    fn an_execution_failure_does_not_turn_a_capability_into_a_defect() {
        let station = station(Some("0.8.48"), 0);
        let mut rows = vec![capability("content_detail", true, 316, 0)];
        rows[0].execution_failures = 26;
        let mut capabilities = CapabilityMatrix::new();
        capabilities.insert(station.station_ref, rows);
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &capabilities,
            None,
        );
        assert!(html.contains("1 就绪"));
        assert!(!html.contains("降级"));
        assert!(html.contains("另有 26 次执行失败"));
    }

    // -----------------------------------------------------------------------
    // 待认领安装：近期与历史分开数
    // -----------------------------------------------------------------------

    fn installation(version: &str, first_seen_at: &str) -> UnclaimedInstallation {
        UnclaimedInstallation {
            installation_ref: Uuid::new_v4(),
            plugin_version: version.to_owned(),
            browser_label: Some("Chrome".to_owned()),
            first_seen_at: first_seen_at.to_owned(),
        }
    }

    /// 一台机器每升级一次插件就留下一条旧安装。把它们和「今天刚装好、正等你指认」
    /// 的那一条混进一个数字，这个数字就只会往上涨，再也不代表任何要处理的事。
    #[test]
    fn historical_installations_are_counted_apart_from_the_recent_ones() {
        let now = minutes_since_epoch("2026-09-13 12:00").expect("固定时刻");
        assert!(installation_is_recent("2026-09-12 12:00", now));
        assert!(!installation_is_recent("2026-08-29 15:07", now));
        // 认不出时间格式时算「最近」：悄悄折进历史等于让它消失。
        assert!(installation_is_recent("时间未记录", now));
    }

    /// 分组必须用**传进来的那个时刻**，不是自己再读一次墙上时钟。
    ///
    /// 两处各取一次当前时间，在跨过第 7 天那一瞬间会数出两个不同的值：页头说
    /// 0 台待处理，页面下方却仍把那条安装归在「最近」里。
    #[test]
    fn the_recent_grouping_uses_the_instant_it_was_given() {
        let install = installation("0.8.48", "2026-09-13 10:00");
        let capacity = overview(vec![lane("基线建档", available())]);

        let at_noon = render_runtime(
            &base(),
            Some(&capacity),
            &[],
            std::slice::from_ref(&install),
            &CapabilityMatrix::new(),
            None,
            test_now(),
            None,
        );
        assert!(at_noon.contains("1 条最近报到"));

        // 同一条安装，站在一个月后的时刻看，它就是历史残留。
        let later = minutes_since_epoch("2026-10-13 12:00").expect("固定时刻可解析");
        let a_month_later = render_runtime(
            &base(),
            Some(&capacity),
            &[],
            std::slice::from_ref(&install),
            &CapabilityMatrix::new(),
            None,
            later,
            None,
        );
        assert!(a_month_later.contains("0 条最近报到"));
        assert!(a_month_later.contains("下面 1 条是更早以前的安装"));
    }

    #[test]
    fn an_unclaimed_install_is_listed_without_a_station() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[installation("0.5.0", "2026-08-29 15:07")],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("插件 0.5.0"));
        assert!(html.contains("还没归位的插件安装"));
        // 没有工位可认领到时不给认领控件：给了也是点了没反应。
        assert!(!html.contains("/collection/runtime/claims"));
        assert!(html.contains("待认领"));
    }

    #[test]
    fn an_unclaimed_install_can_be_claimed_once_a_station_exists() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(Some("0.8.48"), 0)],
            &[installation("0.8.48", "2026-09-13 10:00")],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("/collection/runtime/claims"));
    }

    // -----------------------------------------------------------------------
    // 账号：确认动作、排障信息与身份材料边界
    // -----------------------------------------------------------------------

    #[test]
    fn binding_action_is_only_shown_for_a_candidate_needing_confirmation() {
        let station = station(Some("0.8.48"), 0);
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let capacity = overview(vec![lane("基线建档", available())]);

        let needing = vec![resource(station.station_ref, "unconfirmed")];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &needing,
            account_observation_available: true,
        };
        let candidate = render(
            Some(&capacity),
            &[station.clone()],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(candidate.contains("data-account-binding-form"));
        assert!(candidate.contains("待确认"));

        let bound = vec![resource(station.station_ref, "bound_without_eligibility")];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &bound,
            account_observation_available: true,
        };
        let already_bound = render(
            Some(&capacity),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(!already_bound.contains("data-account-binding-form"));
        assert!(already_bound.contains("缺资格信号"));
    }

    #[test]
    fn the_last_observation_is_diagnosis_material_not_a_claim_deadline() {
        let station = station(Some("0.8.48"), 0);
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let mut rows = vec![resource(station.station_ref, "current")];
        rows[0].eligibility_observed_at = Some("2026-09-10 09:00".to_owned());
        rows[0].eligibility_reason_code = Some("authenticated".to_owned());
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &rows,
            account_observation_available: true,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station.clone()],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(html.contains("最后一次账号观察"));
        assert!(html.contains("不会仅因为「很久没观察」就拦住接活"));

        let never = vec![resource(station.station_ref, "current")];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &never,
            account_observation_available: true,
        };
        let never_observed = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(never_observed.contains("从未观察过"));
        assert!(never_observed.contains("account_unknown"));
    }

    #[test]
    fn a_missing_identity_key_is_stated_without_exposing_identity_material() {
        let station = station(Some("0.8.48"), 0);
        let lanes = vec![control_lane("创作者基线", "deep_archive", true)];
        let rows = vec![resource(station.station_ref, "current")];
        let control = RuntimeControl {
            lanes: &lanes,
            resources: &rows,
            account_observation_available: false,
        };
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            Some(&control),
        );
        assert!(html.contains("本机没有配置账号身份摘要"));
        assert!(html.contains("明确的登录或限制仍然会阻断接活"));
        for forbidden in ["credential_hash", "identity_digest", "Cookie", "payload"] {
            assert!(!html.contains(forbidden), "泄漏 {forbidden}");
        }
    }

    #[test]
    fn names_are_escaped() {
        let mut station = station(Some("0.8.48"), 0);
        station.display_name = "Mac mini <主机>".to_owned();
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("Mac mini &lt;主机&gt;"));
        assert!(!html.contains("Mac mini <主机>"));
    }

    // -----------------------------------------------------------------------
    // 今天在跑什么
    // -----------------------------------------------------------------------

    #[test]
    fn runtime_readouts_separate_unreadable_from_zero() {
        // 通道读不到时是 `—`，不是 0。
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("正在执行"));
        assert!(html.contains("<dd>—</dd>"));

        let mut capacity = overview(vec![lane("基线建档", available())]);
        capacity.dispatch_backlog = vec![DispatchLaneBacklog {
            dispatch_lane: "immediate".to_owned(),
            queued_work_orders: 0,
            leased_work_orders: 2,
            retry_cooling_work_orders: 1,
            oldest_ready_at: None,
            concurrent_cap: Some(3),
        }];
        capacity.platform_dispatch = vec![PlatformDispatchCapacity {
            platform: "xhs".to_owned(),
            concurrent_cap: 10,
            live_leases: 2,
        }];
        let html = render(Some(&capacity), &[], &[], &CapabilityMatrix::new(), None);
        assert!(html.contains("<dd>2</dd>"));
        assert!(html.contains("<dd>0</dd>"));
        assert!(html.contains("2/10"));
    }

    #[test]
    fn a_silent_scheduler_with_nothing_to_do_is_not_reported_as_broken() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("没有任何目标开着自动观察"));
        assert!(!html.contains("未接通"));
    }

    #[test]
    fn never_patrolled_reads_as_never_not_as_zero() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("最近一次派出 从未"));
    }

    #[test]
    fn an_active_risk_pause_is_counted_in_the_factor_row() {
        let mut capacity = overview(vec![lane("基线建档", available())]);
        capacity.risk_pauses = vec![ActiveRiskPause {
            platform: Some("xhs".to_owned()),
            lane: None,
            reason: "人工刹车".to_owned(),
            paused_by: "mog".to_owned(),
            paused_at: "2026-09-13 09:00".to_owned(),
        }];
        let html = render(Some(&capacity), &[], &[], &CapabilityMatrix::new(), None);
        assert!(html.contains("1 条生效"));
    }

    /// 账号不参与那四样的判定，页面必须说清它按每台工位单独判定，而不是给一个
    /// 指向页面上已经不存在的区块的路牌。
    #[test]
    fn the_account_factor_points_at_something_that_exists_on_the_page() {
        let html = render(
            Some(&overview(vec![lane("基线建档", available())])),
            &[station(Some("0.8.48"), 0)],
            &[],
            &CapabilityMatrix::new(),
            None,
        );
        assert!(html.contains("每台单独判定"));
        assert!(html.contains("见下面工位表的「账号」列"));
        assert!(!html.contains("见上方控制资格"));
    }

    #[test]
    fn runtime_scale_sections_render_only_persisted_scheduler_facts() {
        let mut capacity = overview(vec![lane("基线建档", available())]);
        capacity.monitor_rule_schedules = vec![MonitorRuleSchedule {
            target_label: "木可可".to_owned(),
            interval_seconds: 86_400,
            last_scheduled_for: Some("2026-09-12 18:50".to_owned()),
            last_attempt_started_at: Some("2026-09-12 18:55".to_owned()),
            latest_work_order_state: Some("completed".to_owned()),
            next_run_at: Some("2026-09-13 18:50".to_owned()),
            last_receipt_at: Some("2026-09-12 18:55".to_owned()),
        }];
        let html = render(Some(&capacity), &[], &[], &CapabilityMatrix::new(), None);
        assert!(html.contains("自动观察排程"));
        assert!(html.contains("木可可"));
        assert!(html.contains("每 24 小时"));
        assert!(html.contains("已完成"));
        // 年份裁掉，但时间仍是同一种格式。
        assert!(html.contains("09-13 18:50"));
    }

    #[test]
    fn a_failed_action_says_so_instead_of_looking_like_it_worked() {
        let render_with = |error: Option<&str>| {
            render_runtime(
                &base(),
                Some(&overview(vec![lane("基线建档", available())])),
                &[],
                &[],
                &CapabilityMatrix::new(),
                None,
                test_now(),
                error,
            )
        };

        let html = render_with(Some("station_rejected"));
        assert!(html.contains("没有完成"));
        assert!(html.contains("不允许重名"));
        // 错误块住在登记弹窗里，所以「说了没有完成」还不够——窗要是关着，用户看到的
        // 就只是「页面回到原样、什么都没说」，而原因恰好藏在一个看不见的窗里。这条
        // 断言钉住的是**窗必须自己开着**：失败后整页重渲染，没有任何脚本再替它开。
        assert!(
            html.contains(r#"<div class="c-stn-overlay" data-station-register-overlay>"#),
            "登记失败时弹窗必须渲染成开着的，否则错误信息不可见"
        );

        // 反过来也要钉住：没有失败时窗必须是关的，否则每次进页面都会弹一个空窗。
        let clean = render_with(None);
        assert!(
            clean.contains(r#"<div class="c-stn-overlay" data-station-register-overlay hidden>"#),
            "没有失败时弹窗必须是关着的"
        );
    }
}
