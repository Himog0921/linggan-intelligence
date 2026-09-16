//! COLLECTION-001 · injecting stored observation targets into the Targets surface.
//!
//! The page is rendered synchronously as an honest empty state; this module replaces that
//! empty state once the read side actually has targets. It lives apart from `collection.rs`
//! because that file already exceeds the module size limit — growing it further would make a
//! known problem worse.
//!
//! What this view may claim is narrow: a row here proves the target was **stored**. It says
//! nothing about archiving, authorisation or any capture ever running (INV-36).

use linggan_evidence::{
    ArchiveCompleteness, ObservationTarget, ObservationTargetAvatar, PatrolReadState,
    TargetObservationSummary,
};
use serde_json::Value;
use std::collections::HashMap;

/// The marker `collection.rs` leaves in the Targets page so the read side can find either a
/// confirmed-empty or unreadable empty state. A successful non-empty list is stronger than
/// a failed count query and must replace either form without changing unrelated header facts.
const EMPTY_STATE_OPEN: &str = "<section class=\"c-empty";
const EMPTY_STATE_CLOSE: &str = "</section>";

/// Replace the provisional target state with the successful list result. An empty successful
/// result is also a fact: it must replace both "list unreadable" variants without claiming
/// that the entire unfiltered target collection is empty.
pub fn render_stored_targets(
    base: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: Option<&HashMap<String, ArchiveCompleteness>>,
    error: Option<&str>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    render_stored_targets_with_observation(
        base,
        targets,
        avatars,
        completeness,
        None,
        error,
        None,
        None,
        None,
        // 这条入口不读那三件事——读不到就如实是「读不到」，不假装查过。
        TargetListFacts::default(),
        list_context,
    )
}

pub fn render_stored_targets_with_observation(
    base: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: Option<&HashMap<String, ArchiveCompleteness>>,
    observation: Option<&HashMap<uuid::Uuid, TargetObservationSummary>>,
    error: Option<&str>,
    // 刚排进队列的那张工单前面还有几个。只喂给右下角那条回执。
    ahead: Option<i64>,
    // 打开了删除确认面板时，这里带着「会删掉什么、会留下什么」的真实数字。
    deletion: Option<&linggan_evidence::TargetDeletionPreview>,
    deletion_target: Option<uuid::Uuid>,
    facts: TargetListFacts<'_>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    if targets.is_empty() {
        return replace_target_state(
            base,
            r#"<section class="c-empty c-empty-known-view">
                 <div class="c-empty-rule"></div>
                 <h2>当前列表范围没有匹配的观察目标</h2>
                 <p>当前筛选下没有观察目标。可以切换上方分类查看其他目标；这不表示平台上没有可观察对象。</p>
                 <div class="c-empty-foot"></div>
               </section>"#,
        );
    }

    let creator_table = target_table(
        "creator",
        "创作者档案",
        targets,
        avatars,
        completeness,
        observation,
        // 创作者不看建档那两问：它的档案状态另有来源（`completeness`）。排队位置两类都要。
        TargetListFacts {
            keyword_archives: None,
            keyword_details_pending: None,
            ..facts
        },
        list_context,
    );
    let keyword_table = target_table(
        "keyword",
        "关键词观察",
        targets,
        avatars,
        completeness,
        observation,
        facts,
        list_context,
    );

    // Creator 与 keyword 的事实不同，但列表管理动作相同。选择由表格提供，主动作
    // 固定在顶栏；两者以 HTML form 属性关联，不能把行操作 form 嵌进一个外层 form。
    // 否则浏览器会纠正无效嵌套，导致后续复选框和批量提交失去共同的表单归属。
    let list = format!(
        r#"<section class="c-tg-workspace">
              {failure}
              <div class="c-tg-directory">{creator_table}{keyword_table}</div>
              <form id="target-batch-modal-form" class="c-tg-batch" method="post" action="/collection/targets/batch">
                <div class="c-tg-batch-overlay" data-target-batch-modal hidden>
                  <section id="target-batch-modal" class="c-tg-batch-dialog" role="dialog" aria-modal="true" aria-labelledby="target-batch-title">
                    <div><h2 id="target-batch-title">批量编辑观察目标</h2><p data-target-batch-count>已选择 0 个目标</p></div>
                    <label>分组<input name="group_name" type="text" maxlength="80" placeholder="例如：ADHD 主力" required/></label>
                    <p>保存后会为所选目标设置同一分组。备注将在具备独立事实字段后接入，当前不会把自由文本冒充为已有备注。</p>
                    <div class="c-tg-batch-dialog-actions"><button class="c-btn-secondary" type="button" data-target-batch-close>取消</button><button class="c-btn-primary" type="submit" name="action" value="set_group">保存分组</button></div>
                  </section>
                </div>
              </form>
              {deletion}
            </section>"#,
        deletion = match (deletion, deletion_target) {
            (Some(preview), Some(target_ref)) => {
                deletion_modal(preview, target_ref, error, list_context)
            }
            _ => String::new(),
        },
        failure = action_feedback_markup(error, ahead),
    );
    replace_target_state(base, &list)
}

/// 列表页为整批目标额外读到的三件事。
///
/// 它们都是**批量读**的结果，都可能读不到，而且都只影响一行怎么显示，不影响目标本身。
/// 合成一个参数不是为了少写几个字：这三项永远一起取、一起传，分开列出来时，任何一处
/// 少传一个就是一个静默错位的行状态。
#[derive(Clone, Copy, Default)]
pub struct TargetListFacts<'facts> {
    /// 已经把搜索面翻完的关键词目标。`None` 表示这一批没读出来——与 `completeness`
    /// 同一种表达：读不到就说读不到，不压成「没建过」。
    pub keyword_archives: Option<&'facts std::collections::HashSet<uuid::Uuid>>,
    /// 还有作品等着补详情的关键词目标。`None` 同样是「没读出来」，不是「都补齐了」。
    pub keyword_details_pending: Option<&'facts std::collections::HashSet<uuid::Uuid>>,
    /// 每个关键词命中了多少篇、补到了多少篇详情。`None` 是这一批没读出来。
    pub keyword_counts: Option<&'facts HashMap<uuid::Uuid, linggan_evidence::KeywordCatalogCounts>>,
}

/// 把两次批量查询合成这一行的建档状态。
///
/// 三件事分开表达：翻没翻完、详情补没补完、读没读到。任何一次读不到都必须说「读不到」
/// ——把它压成「没建过」会催人重做一次真实的平台访问，压成「已完成」则会把一个半成品
/// 底座推进巡检。
///
/// 抽屉也调它（`pub(super)`）：同一个词的建档态在列表行和抽屉里必须由同一段代码得出，
/// 各写一份迟早会各说各的。
pub(super) fn keyword_archive_read(
    keyword_archives: Option<&std::collections::HashSet<uuid::Uuid>>,
    keyword_details_pending: Option<&std::collections::HashSet<uuid::Uuid>>,
    target_ref: uuid::Uuid,
) -> super::target_drawer::KeywordArchiveRead {
    use super::target_drawer::KeywordArchiveRead;
    let Some(archived) = keyword_archives else {
        return if keyword_details_pending.is_some_and(|pending| pending.contains(&target_ref)) {
            KeywordArchiveRead::DetailPending
        } else {
            KeywordArchiveRead::Unavailable
        };
    };
    if !archived.contains(&target_ref) {
        return if keyword_details_pending.is_some_and(|pending| pending.contains(&target_ref)) {
            // Baseline coverage and a discovered material's detail completeness are independent.
            KeywordArchiveRead::DetailPending
        } else {
            KeywordArchiveRead::NotArchived
        };
    }
    match keyword_details_pending {
        Some(pending) if pending.contains(&target_ref) => KeywordArchiveRead::DetailPending,
        Some(_) => KeywordArchiveRead::Complete,
        // 详情那一问没读到，不能用 baseline 代替它，更不能把未知写成补齐。
        None => KeywordArchiveRead::Unavailable,
    }
}

fn target_table(
    kind: &str,
    title: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: Option<&HashMap<String, ArchiveCompleteness>>,
    observation: Option<&HashMap<uuid::Uuid, TargetObservationSummary>>,
    facts: TargetListFacts<'_>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let matching = targets
        .iter()
        .enumerate()
        .filter(|(_, target)| target.target_kind == kind)
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return String::new();
    }
    let is_creator = kind == "creator";
    let mut rows = String::new();
    for (index, target) in matching.iter().copied() {
        rows.push_str(&target_row(
            target,
            index,
            avatars.get(&target.target_ref),
            super::target_drawer::TargetArchiveRead::from_map(completeness, &target.identity_key),
            observation.and_then(|values| values.get(&target.target_ref)),
            facts,
            list_context,
        ));
    }
    let columns = if is_creator {
        r#"<span role="columnheader"><label class="c-tg-select-all"><input type="checkbox" data-target-select-all aria-label="选择全部创作者目标"/></label></span><span role="columnheader">编号</span><span role="columnheader">创作者</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">档案状态</span><span class="c-tg-head-num" role="columnheader">作品目录</span><span class="c-tg-head-num c-tg-head-end" role="columnheader">详情进度</span><span role="columnheader">巡查状态</span><span role="columnheader">最近新增</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
    } else {
        r#"<span role="columnheader"><label class="c-tg-select-all"><input type="checkbox" data-target-select-all aria-label="选择全部关键词目标"/></label></span><span role="columnheader">编号</span><span role="columnheader">关键词</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">档案状态</span><span class="c-tg-head-num" role="columnheader">命中作品</span><span class="c-tg-head-num c-tg-head-end" role="columnheader">详情进度</span><span role="columnheader">巡查状态</span><span role="columnheader">最近新增</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
    };
    let grid = if is_creator {
        "c-tg-creator-grid"
    } else {
        "c-tg-keyword-grid"
    };
    format!(
        r#"<section class="c-tg-kind" aria-labelledby="target-kind-{kind}">
              <div class="c-tg-kind-head"><h2 id="target-kind-{kind}">{title}</h2><span>{count} 项</span></div>
              <div class="c-tg-table-scroll" role="table" aria-label="{title}">
                <div class="c-tg-table-head {grid}" role="row">{columns}</div>
                <div class="c-tg-list" role="rowgroup">{rows}</div>
              </div>
            </section>"#,
        count = matching.len(),
    )
}

fn replace_target_state(base: &str, replacement: &str) -> String {
    let Some(open) = base.find(EMPTY_STATE_OPEN) else {
        return base.to_owned();
    };
    let Some(close_offset) = base[open..].find(EMPTY_STATE_CLOSE) else {
        return base.to_owned();
    };
    let close = open + close_offset + EMPTY_STATE_CLOSE.len();
    format!("{}{}{}", &base[..open], replacement, &base[close..])
}

/// 上一次动作的回执必须看得见。跳转回来却什么都不说，会让人以为动作成功了——那比
/// 「点了没反应」更糟，因为它会让人以为系统里正在跑一件其实没跑的事。
fn action_feedback_markup(error: Option<&str>, ahead: Option<i64>) -> String {
    let Some(code) = error else {
        return String::new();
    };
    let (class, heading, explanation) = match code {
        "domain_name_required" => (
            "c-src-feedback c-src-feedback-warn",
            "新领域还没起名",
            "目标没有建立。选了「新建一个领域」就要给它一个名字——领域名是后面所有材料归属的依据，不能留空。",
        ),
        "domain_name_taken" => (
            "c-src-feedback c-src-feedback-warn",
            "这个领域已经有了",
            "目标没有建立，也没有新建重名领域。想归进已有的那个领域，直接在下拉里选它；这里不静默复用同名领域，否则你以为新开了一个领域，实际把目标归进了另一个已经在用的领域。",
        ),
        "domain_create_failed" => (
            "c-src-feedback c-src-feedback-warn",
            "领域没有建成",
            "目标也没有建立。这次什么都没有改动，可以重试；如果反复失败，说明领域这部分功能当前不可用，而不是你填错了。",
        ),
        "target_domain_unassigned" => (
            "c-src-feedback c-src-feedback-warn",
            "这个目标还没归属领域",
            "采集没有开始。领域决定这个目标采回来的材料写进本行业证据库还是跨行业参照语料，没有它就只能靠推断，而推断错会让参照物混进证据。请先给这个目标指定领域，再发起采集——不是等状态流转，也不是服务出了问题。",
        ),
        "target_domain_required" => (
            "c-src-feedback c-src-feedback-warn",
            "还没选领域",
            "新建观察目标前要先选定它属于哪个领域。领域决定这个目标采回来的材料进本行业证据库还是跨行业参照语料——选错会让参照物混进证据，而且之后任何读证据的地方都不会再提醒你，所以这一项不做推断。",
        ),
        "archive_requested" => (
            "c-src-feedback c-src-feedback-ok",
            "建档已入队",
            "已记录这次建立档案请求。系统会先取回作品链接（创作者取主页目录，关键词按排序翻搜索面），再逐篇补齐详情；目录和详情只会在接纳真实回执后更新。执行要等一个空闲工位，可能需要几分钟，不用再点一次。",
        ),
        "keyword_detail_requested" => (
            "c-src-feedback c-src-feedback-ok",
            "已排入详情补采",
            "这个词的链接已经拿到，正在按点赞从高到低逐篇补详情：正文、发布时间、前 30 条评论与 2 层回复，并把图片与视频文件一并取回（其中的文字与语音随后识别）。一次补三篇，补完之后调度会自己接着补下一批。执行要等一个空闲工位。",
        ),
        "keyword_detail_complete" => (
            "c-src-feedback c-src-feedback-ok",
            "详情已补齐",
            "这个词当前拿到的作品都取过详情了。下一步是开始每周巡检，看它每周新增什么。",
        ),
        "monitor_rule_retired" => (
            "c-src-feedback c-src-feedback-ok",
            "这条规则已停用",
            "它不再排期。已经签发过的工单与采回来的材料仍然留着——删掉会让那些材料说不清是按什么口径取回来的。想重新用它，按同一个排序再存一条规则即可。",
        ),
        "monitor_rule_is_the_last_one" => (
            "c-src-feedback c-src-feedback-warn",
            "这是最后一条规则",
            "没有停用。观察中的目标必须有一条规则，否则调度不知道该按什么口径去看它。要完全停下来，用这一行的观察开关停止观察；那是另一个决定，不该由「停用一条规则」顺带完成。",
        ),
        "monitor_rule_paused" => (
            "c-src-feedback c-src-feedback-ok",
            "这条规则已暂停",
            "只停了这一条口径，这个目标的其他规则照自己的周期继续跑。人工观察仍可申请。",
        ),
        "monitor_rule_resumed" => (
            "c-src-feedback c-src-feedback-ok",
            "这条规则已启用",
            "它重新排期了；尚未据此声称已经派出任务。",
        ),
        "monitor_rule_saved" => (
            "c-src-feedback c-src-feedback-ok",
            "规则已保存",
            "只写入了一版规则，没有据此声称采集已开始或完成。",
        ),
        "monitor_rule_command_stale" => (
            "c-src-feedback c-src-feedback-warn",
            "页面上的版本已经旧了",
            "这一条规则在你打开这一页之后被改过（可能是另一个标签页，也可能是一次巡检推进了它的版本）。**什么都没有改动**。刷新一下看当前的样子再点。",
        ),
        "monitor_rule_command_replayed" => (
            "c-src-feedback c-src-feedback-ok",
            "这条命令已经处理过了",
            "重复提交没有再改一次；显示的是原来那次的耐久结果。",
        ),
        "monitor_rule_command_rejected" => (
            "c-src-feedback c-src-feedback-warn",
            "这次命令没有被接受",
            "规则一动没动。原因记在这条命令的回执里；从这一行的「管理巡查」进去可以看到逐字说明。",
        ),
        "monitor_rule_unknown" => (
            "c-src-feedback c-src-feedback-warn",
            "没找到这条规则",
            "它可能已经被停用了。刷新一下看当前还有哪几条。",
        ),
        "monitor_rule_retire_failed" => (
            "c-src-feedback c-src-feedback-warn",
            "规则没有停用",
            "这次什么都没有改动，可以重试。反复失败说明规则管理当前不可用，而不是你点错了。",
        ),
        "archive_merge" => (
            "c-src-feedback c-src-feedback-warn",
            "未重复提交",
            "已有相同建档任务等待处理或执行中。已打开建档状态；本次没有创建第二个任务。",
        ),
        "target_deleted" => (
            "c-src-feedback c-src-feedback-ok",
            "观察目标已删除",
            "它的采集申请、工单与租约一并清掉了。已经采到的作品、详情与评论仍在语料库里，也仍然属于这个博主——作者归属来自作品自己，不依赖观察目标是否存在。",
        ),
        "target_delete_blocked" => (
            "c-src-failure",
            "没有完成",
            "这个目标下已有必须保留的材料或人工结论。没有做任何删除；在有独立保留方案前，不能为了删掉观察决定而把它们一起抹掉。",
        ),
        "target_delete_name_mismatch" => (
            "c-src-failure",
            "没有完成",
            "输入的名字与目标名字不一致，没有删除任何东西。",
        ),
        "target_delete_failed" | "target_delete_invalid" | "target_delete_missing" => (
            "c-src-failure",
            "没有完成",
            "这次删除没有生效，目标与它的记录都没有变化。可以再试一次。",
        ),
        "patrol_paused" => (
            "c-src-feedback c-src-feedback-ok",
            "已停止观察",
            "不再排新的巡检；已经在跑的会跑完。语料与档案全部保留，随时可以恢复。",
        ),
        "patrol_resumed" => (
            "c-src-feedback c-src-feedback-ok",
            "已恢复观察",
            "会按现有规则重新排巡检。",
        ),
        "patrol_toggle_partial" => (
            "c-src-feedback c-src-feedback-warn",
            "有规则没有翻过来",
            "这个开关一次翻这个目标的全部规则，其中至少有一条在你打开这一页之后被改过（另一个标签页，或者一次巡检推进了它的版本），那一条没有改动。这一行的状态显示的是真实情况：只要还有一条在跑，它就仍然是「巡查中」。刷新后从检查器的规则台逐条确认。",
        ),
        "patrol_toggle_no_rule" => (
            "c-src-failure",
            "没有完成",
            "这个目标还没有生效的观察规则，没有可以开关的东西。先设置一条规则。",
        ),
        "patrol_toggle_failed" | "patrol_toggle_invalid" => (
            "c-src-failure",
            "没有完成",
            "这次开关没有生效，巡检状态没有变化。",
        ),
        // 成功回执与失败回执一样必须说出来：跳转回来却什么都不说，人会以为没生效而再点一次。
        "material_retirement_done" => (
            "c-src-feedback c-src-feedback-ok",
            "已记下这几篇不存在了",
            "它们仍留在作品目录里——博主当时确实发过——只是不再计入待补齐。作品若日后恢复并采到详情，会自动按已有详情算。",
        ),
        "material_retirement_empty" => (
            "c-src-failure",
            "没有完成",
            "一篇都没有勾选。没有勾选不等于全部确认，这次没有写下任何判断。",
        ),
        "material_retirement_none" => (
            "c-src-feedback c-src-feedback-warn",
            "没有新的判断",
            "勾选的作品此前已经确认过，或它们已经取到了详情。没有重复写入第二条记录。",
        ),
        "material_retirement_failed" => (
            "c-src-failure",
            "没有完成",
            "这次判断没有写进去。作品目录与待补齐都没有变化，可以再试一次。",
        ),
        "material_retirement_invalid" => (
            "c-src-failure",
            "没有完成",
            "这次提交没有说明是哪个观察目标，因此没有写下任何判断。",
        ),
        "identity_unrecognised" => (
            "c-src-failure",
            "没有完成",
            "认不出这是谁。创作者请粘主页链接（里面带平台 ID），关键词直接写词就行。",
        ),
        "store_failed" => (
            "c-src-failure",
            "没有完成",
            "没有保存成功。这个目标可能已经在观察列表里了。",
        ),
        "archive_not_requestable" => (
            "c-src-failure",
            "没有完成",
            "现在不能重复提交建档或补采。目标可能已有同类任务在进行，或当前状态不允许再次发起。",
        ),
        "archive_in_progress" => (
            "c-src-failure",
            "没有完成",
            "已有一批作品正在补齐，请先查看当前进度。",
        ),
        "archive_nothing_to_continue" => (
            "c-src-failure",
            "没有完成",
            "当前作品目录没有待补详情，无需重复发起。",
        ),
        "archive_refuse" => (
            "c-src-failure",
            "没有完成",
            "当前没有覆盖本次范围的采集授权，请完成授权后再试。",
        ),
        "archive_defer" => (
            "c-src-failure",
            "没有完成",
            "当前暂无可用采集能力，或今天的采集额度已用完；稍后可以重试。",
        ),
        "archive_lease_failed" => (
            "c-src-failure",
            "没有完成",
            "建档已完成准备，但暂时没有可用执行资源；稍后可以重试。",
        ),
        "archive_authorization_below_200" => (
            "c-src-failure",
            "没有完成",
            "当前采集授权不足以支持前 200 篇作品的有界建档。本次没有缩小范围后静默开始。",
        ),
        "archive_unavailable" => (
            "c-src-failure",
            "没有完成",
            "这次请求没有送达采集链路，没有发起任何采集，目标状态也没有改动。可以稍后重试。",
        ),
        "keyword_archive_target_missing" => (
            "c-src-failure",
            "目标已不存在",
            "没有建立档案，也没有创建新的采集任务。这个观察目标已被删除或当前地址指向了旧记录；刷新列表后再继续，而不是重试这次请求。",
        ),
        "keyword_archive_incomplete" => (
            "c-src-feedback c-src-feedback-warn",
            "关键词建档尚未完成",
            "没有开启或恢复巡查。请先完成搜索面的有界建档，并补齐已发现作品的详情；这两段都由接纳的真实回执证明，不能用已提交任务或空状态代替。",
        ),
        "keyword_patrol_archive_unreadable" => (
            "c-src-failure",
            "无法读取关键词建档状态",
            "没有开启或恢复巡查。未知不等于已完成；等建档读取恢复后，系统才会判断这个关键词是否可以进入巡查。",
        ),
        // 关键词的「历史建档」只在**开始观察之前**被采集准入接受。这是一条状态前置，
        // 不是暂时故障——写「稍后重试」等于让人反复做一件永远不会成的事。此前它落进
        // `archive_unavailable`，那句话正好是「可以稍后重试」。
        "keyword_archive_not_requestable" => (
            "c-src-failure",
            "没有完成",
            "采集准入按这个词当前所处的状态拒绝了这次请求，因此这次没有发起采集，目标状态也没有改动。准入的前置是状态：历史建档只在开始观察之前接受；详情补采在这个词停止观察之后就不再接受。**重试不会改变结果**——这一条是状态前置，不是暂缺资源。",
        ),
        // 建档态读不出来时**没有可猜的默认动作**：当成没建过会重做一次真实的平台访问，
        // 当成建好了会把半成品底座推进巡检。两件都不做，并且把原因说出来——此前它落进
        // 一句「上一次动作没有完成」，人只看到失败，看不到「为什么不能替你决定」。
        "keyword_archive_unreadable" => (
            "c-src-failure",
            "没有完成",
            "读不到这个词的建档状态，因此这次没有发起采集（建档与补详情要看它才能定）。目标状态没有改动，可以稍后重试。",
        ),
        "batch_nothing_selected" => ("c-src-failure", "没有完成", "没有选中任何观察目标。"),
        "batch_unknown_action" => ("c-src-failure", "没有完成", "当前不支持这个操作。"),
        "batch_failed" => (
            "c-src-failure",
            "没有完成",
            "这次操作没有完成，也没有改动任何观察目标。",
        ),
        "monitoring_toggle_failed" => ("c-src-failure", "没有完成", "巡查开关没有切换成功。"),
        "read_model_not_connected" => (
            "c-src-failure",
            "没有完成",
            "当前无法读取目标状态，这次没有产生任何改动。",
        ),
        _ => ("c-src-failure", "没有完成", "上一次动作没有完成。"),
    };
    // 成功回执做成右下角角标，3 秒后自己消失：它只是确认「刚才那下生效了」，看过即可，
    // 常驻在列表顶上会把内容一直往下顶。
    //
    // **失败与警示不自动消失**——没成功的事必须让人看清楚，自动收走等于把坏消息藏起来。
    // 消失由 CSS 动画完成，不依赖 JS：脚本没跑起来时，一个永不消失的浮层比横幅更糟。
    if class == "c-src-feedback c-src-feedback-ok" {
        // 排队位置只在这条右下角回执里说一次。**不在每一行旁边常驻一句描述**：那行字
        // 挤在操作按钮之间，窄一点就和按钮叠在一起，而它回答的本来就是「我刚点的那下
        // 什么时候轮到」——看过即可。
        let queued = match ahead {
            Some(ahead) if ahead > 0 => format!("前面还有 {ahead} 个在排队。"),
            Some(_) => "它就排在最前面。".to_owned(),
            None => String::new(),
        };
        return format!(
            r#"<p class="c-tg-toast" role="status"><b>{heading}</b>{explanation}{queued}</p>"#,
            heading = heading,
            explanation = escape(explanation),
        );
    }
    format!(
        r#"<p class="{class}" role="status"><b>{heading}</b>{explanation}</p>"#,
        class = class,
        heading = heading,
        explanation = escape(explanation),
    )
}

/// 一行观察目标。creator 与 keyword 只共享对象身份和巡查时间；档案、作品分布与详情
/// 只属于 creator。当前读模型没有「最近新增/数据更新」计数时直接写尚未取得，不用 0
/// 冒充已巡查后的零变化。
fn target_row(
    target: &ObservationTarget,
    index: usize,
    avatar: Option<&ObservationTargetAvatar>,
    archive: super::target_drawer::TargetArchiveRead<'_>,
    observation: Option<&TargetObservationSummary>,
    facts: TargetListFacts<'_>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let keyword_archive = keyword_archive_read(
        facts.keyword_archives,
        facts.keyword_details_pending,
        target.target_ref,
    );

    let is_creator = target.target_kind == "creator";
    let name = target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key);
    let opener_href = list_context.drawer_href(target.target_ref, &[], None);
    let opener_id = format!("target-{}", target.target_ref);

    let opener_label = if is_creator {
        format!("打开{name}的创作者档案")
    } else {
        format!("打开{name}的关键词观察")
    };
    let shared_start = format!(
        r#"<label class="c-tg-select" role="cell"><input type="checkbox" name="target_ref" value="{target_ref}" form="target-batch-modal-form" data-target-select aria-label="选择{opener_label}"/></label>
            <div class="c-tg-number" role="cell"><span class="c-tg-index">{index:03}</span></div>
            <div class="c-tg-object" role="cell">{avatar}<div class="c-tg-object-text"><a id="{opener_id}" class="c-tg-title c-tg-object-link" data-row-opener data-drawer-trigger="{target_ref}" href="{opener_href}" aria-label="{opener_label}">{name}</a>{identity}</div></div>
            <div class="c-tg-cell c-tg-platform" role="cell">{platform}</div>
            <div class="c-tg-cell c-tg-group" role="cell" title="{group}">{group}</div>"#,
        index = index + 1,
        avatar = if is_creator {
            avatar_markup(avatar, name)
        } else {
            String::new()
        },
        name = escape(name),
        identity = if is_creator {
            format!(
                r#"<span class="c-tg-meta" title="{}">{}</span>"#,
                escape(&identity_display(target)),
                escape(&identity_display(target))
            )
        } else {
            String::new()
        },
        platform = escape(platform_label(&target.platform)),
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
        target_ref = target.target_ref,
        opener_label = escape(&opener_label),
    );
    let last = moment_without_year(
        target
            .last_patrol_succeeded_at
            .as_deref()
            .unwrap_or("尚未巡查"),
    );
    // 下次巡查写成「还有多久」：人在这一列判断的是要等多久，不是那一刻的钟点。
    let next = if target.monitoring_enabled {
        relative_moment(target.next_patrol_at.as_deref(), beijing_now_minutes())
    } else {
        "—".to_owned()
    };
    let cells = if is_creator {
        format!(
            r#"<div class="c-tg-cell" role="cell">{archive_state}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{works}</div>
                <div class="c-tg-cell c-tg-number-value c-tg-cell-end" role="cell">{details}</div>
                <div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-change" role="cell">{recent_change}</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell" data-row-no-open>{actions}{secondary}</div>"#,
            archive_state = archive_state(target, archive),
            works = archive_count(archive),
            details = detail_count(archive),
            patrol = patrol_state(target, observation),
            recent_change = creator_recent_change(observation),
            last = escape(last),
            next = escape(&next),
            actions = row_action(target, true, archive, keyword_archive, list_context),
            secondary = row_secondary_actions(target, list_context),
        )
    } else {
        format!(
            r#"<div class="c-tg-cell" role="cell">{rule}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{hits}</div>
                <div class="c-tg-cell c-tg-number-value c-tg-cell-end" role="cell">{details}</div>
                <div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-change" role="cell">{recent_change}</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell" data-row-no-open>{actions}{secondary}</div>"#,
            rule = keyword_archive_state(keyword_archive),
            patrol = patrol_state(target, observation),
            hits = keyword_hit_count(facts.keyword_counts, target.target_ref),
            details = keyword_detail_progress(facts.keyword_counts, target.target_ref),
            recent_change = keyword_recent_change(observation),
            last = escape(last),
            next = escape(&next),
            actions = row_action(target, false, archive, keyword_archive, list_context),
            secondary = row_secondary_actions(target, list_context),
        )
    };
    let grid = if is_creator {
        "c-tg-creator-grid"
    } else {
        "c-tg-keyword-grid"
    };
    format!(
        r#"<article class="c-tg-item {grid}" role="row" data-target-row>
              {shared_start}{cells}
            </article>"#,
    )
}

fn avatar_markup(avatar: Option<&ObservationTargetAvatar>, name: &str) -> String {
    match avatar.unwrap_or(&ObservationTargetAvatar::NotObserved) {
        ObservationTargetAvatar::Local { local_asset_path } => format!(
            r#"<img class="c-tg-avatar" src="{}" alt="" width="40" height="40" referrerpolicy="no-referrer" />"#,
            escape(local_asset_path),
        ),
        ObservationTargetAvatar::Pending => avatar_initial(name, "头像正在准备"),
        ObservationTargetAvatar::Unavailable => avatar_initial(name, "头像当前不可用"),
        ObservationTargetAvatar::NotObserved => avatar_initial(name, "尚未取得头像"),
    }
}

fn avatar_initial(name: &str, title: &str) -> String {
    let initial = name
        .chars()
        .find(|character| !character.is_whitespace())
        .unwrap_or('创');
    format!(
        r#"<span class="c-tg-avatar c-tg-avatar-state" title="{}" aria-hidden="true">{}</span>"#,
        escape(title),
        escape(&initial.to_string())
    )
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "xhs" => "小红书",
        other => other,
    }
}

fn archive_state(
    target: &ObservationTarget,
    archive: super::target_drawer::TargetArchiveRead<'_>,
) -> String {
    use super::target_drawer::TargetArchiveRead;
    let (tone, label) = match archive {
        TargetArchiveRead::Unavailable => ("neutral", "档案暂不可读"),
        TargetArchiveRead::Known(Some(value)) if value.has_actionable_problems() => {
            ("warn", "档案有问题")
        }
        TargetArchiveRead::Known(Some(value)) if value.work_in_progress => ("warn", "建档中"),
        TargetArchiveRead::Known(Some(value))
            if value.directory_baseline
                == linggan_evidence::ArchiveDirectoryBaseline::HistoricalDirectory =>
        {
            ("ok", "已建立")
        }
        TargetArchiveRead::Known(Some(value)) if value.requires_directory_rebuild() => {
            ("warn", "异常")
        }
        TargetArchiveRead::Known(None) => ("neutral", "尚未建立"),
        TargetArchiveRead::Known(Some(value)) if value.is_untouched() => ("neutral", "尚未建立"),
        TargetArchiveRead::Known(Some(value))
            if value.works_listed > 0 && value.pending_details > 0 =>
        {
            ("warn", "详情有缺口")
        }
        TargetArchiveRead::Known(Some(value))
            if value.works_listed > 0
                && matches!(
                    target.lifecycle_state.as_str(),
                    "archived" | "monitoring" | "paused"
                ) =>
        {
            ("ok", "档案已建立")
        }
        TargetArchiveRead::Known(Some(value)) if value.started || value.attempted => {
            ("warn", "档案有问题")
        }
        TargetArchiveRead::Known(Some(value)) if value.works_listed > 0 => ("warn", "详情有缺口"),
        TargetArchiveRead::Known(Some(_)) => ("neutral", "等待作品目录"),
    };
    format!(r#"<span class="c-tg-truth c-tg-{tone}">{label}</span>"#)
}

fn patrol_state(
    target: &ObservationTarget,
    observation: Option<&TargetObservationSummary>,
) -> String {
    let (tone, label) = match observation.map(|summary| summary.patrol_state) {
        Some(PatrolReadState::Disabled) => ("neutral", "已暂停"),
        Some(PatrolReadState::Waiting) => ("neutral", "等待巡查"),
        Some(PatrolReadState::Running) => ("info", "巡查中"),
        Some(PatrolReadState::Normal) => ("ok", "正常"),
        Some(PatrolReadState::Blocked) => ("warn", "受阻"),
        Some(PatrolReadState::Unavailable) | None => {
            super::target_drawer::lifecycle_patrol_copy(target)
        }
    };
    format!(r#"<span class="c-tg-truth c-tg-{tone}">{label}</span>"#)
}

fn creator_recent_change(observation: Option<&TargetObservationSummary>) -> String {
    match observation.and_then(|summary| summary.latest_new) {
        Some(0) => "无新增".to_owned(),
        Some(count) => format!("+{count} 新发现"),
        None => "尚无成功巡查".to_owned(),
    }
}

fn keyword_hits(observation: Option<&TargetObservationSummary>) -> String {
    observation
        .and_then(|summary| summary.latest_hits)
        .map(|count| count.to_string())
        .unwrap_or_else(|| "尚无成功巡查".to_owned())
}

fn keyword_recent_change(observation: Option<&TargetObservationSummary>) -> String {
    match observation.and_then(|summary| summary.latest_new) {
        Some(count) => format!("+{count}"),
        None => "尚无成功巡查".to_owned(),
    }
}

/// 「停止观察」与「删除」这两个动作，跟主动作放在同一格里。
///
/// 停止观察 ≠ 删除：前者是「先不看了」——语料、档案、历史全部保留，只是不再排新的巡检，
/// 随时能开回来；后者是「不要了」，且不可逆。两者形状不同（开关 vs 危险动作），
/// 不能长得像同一个按钮。
fn row_secondary_actions(
    target: &ObservationTarget,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    // 开关不带可见文字：状态由滑块位置与颜色表达（右+信号色=观察中，左+墨色=已停止）。
    // 但没有可见文字就必须有可读标签，否则读屏软件只会念出一个「按钮」。
    let (enable, pressed, action_label) = if target.monitoring_enabled {
        ("false", "true", "正在观察，点击停止观察")
    } else {
        ("true", "false", "已停止观察，点击恢复观察")
    };
    let focus_id = format!("target-{}", target.target_ref);
    let fields = list_context.return_fields(None, None, Some(&focus_id));
    let delete_href = list_context.delete_href(target.target_ref);
    format!(
        r#"<form class="c-tg-toggle" method="post" action="/collection/targets/patrol-toggle">{fields}<input type="hidden" name="enable" value="{enable}"/><button class="c-tg-switch" type="submit" name="row_target_ref" value="{target_ref}" aria-pressed="{pressed}" aria-label="{action_label}" title="{action_label}"></button></form>
           <a class="c-tg-act c-tg-act-danger" href="{delete_href}">删除</a>"#,
        target_ref = target.target_ref,
    )
}

/// 列表里的巡查时刻去掉年份。
///
/// 这一列要回答的是「上次是什么时候」，而同一屏里的年份几乎总是同一个，却稳定占掉
/// 五个字符的宽度——在一张十三列的表里，那正是把「2026-09-08 23:14」挤成
/// 「2026-09-08 23:…」的最后一根稻草。完整时刻仍在目标详情里给出。
///
/// **只裁剪，不另写格式**：全项目的人可读时间由 `linggan_human_moment()` 统一产出，
/// 这里从那一种格式上切掉前缀。自己拼一个 `to_char` 出来就是全项目的第二种时间写法，
/// 治理检查也正是为此设的。
pub(super) fn moment_without_year(value: &str) -> &str {
    // 只认 `YYYY-MM-DD ...` 这一种形状；认不出就原样返回（"尚未巡查" 这类文案）。
    let bytes = value.as_bytes();
    if bytes.len() >= 5 && bytes[0..4].iter().all(u8::is_ascii_digit) && bytes[4] == b'-' {
        &value[5..]
    } else {
        value
    }
}

/// 「下次巡查」写成还有多久，而不是一个绝对时刻。
///
/// 人在这一列要判断的是「还要等多久」，而不是「那一刻的钟点是几点」——后者他还得自己
/// 跟当前时间做一次减法。上次巡查保留绝对时刻：那是一个已经发生的事实，可能要拿去跟
/// 别的记录对时间。
///
/// 逾期单独说。把逾期显示成「0 分钟后」会把一个真的出问题的状态说成正常。
fn relative_moment(value: Option<&str>, now_minutes: i64) -> String {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "尚未安排".to_owned();
    };
    let Some(target_minutes) = minutes_since_epoch(raw) else {
        // 认不出来就原样显示。编一个「还有多久」出来会被当成事实。
        return raw.to_owned();
    };
    let delta = target_minutes - now_minutes;
    match delta {
        d if d <= -60 => format!("已逾期约 {} 小时", (-d + 30) / 60),
        d if d < 0 => format!("已逾期约 {} 分钟", -d),
        d if d < 60 => format!("约 {d} 分钟后"),
        d if d < 60 * 48 => format!("约 {} 小时后", (d + 30) / 60),
        d => format!("约 {} 天后", (d + 720) / 1440),
    }
}

/// 把 `YYYY-MM-DD HH:MM` 折成分钟数。只认这一种格式——全项目的人可读时间都由
/// `linggan_human_moment()` 统一给出，认别的写法等于给第二种格式留后门。
/// 当前时刻，按北京时间折成分钟。
///
/// 加 8 小时而不是查时区库：全项目的会话时区已经固定为 Asia/Shanghai，页面上的时间也
/// 都由 `linggan_human_moment()` 按这个时区给出。这里换一个时区就会跟页面对不上。
pub(super) fn beijing_now_minutes() -> i64 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    i64::try_from(seconds / 60).unwrap_or(0) + 8 * 60
}

pub(super) fn minutes_since_epoch(value: &str) -> Option<i64> {
    let bytes = value.as_bytes();
    if bytes.len() < 16 {
        return None;
    }
    let number = |from: usize, to: usize| value.get(from..to)?.parse::<i64>().ok();
    let (year, month, day) = (number(0, 4)?, number(5, 7)?, number(8, 10)?);
    let (hour, minute) = (number(11, 13)?, number(14, 16)?);
    // civil-from-days（Howard Hinnant）：不引日期库也能把年月日折成天数。
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 1440 + hour * 60 + minute)
}

/// 删除确认面板：把将要发生的事用真实数字摆出来，而不是一句「确定删除吗」。
///
/// 要求把名字原样打一遍——不可逆的操作，点两下太容易了。
///
/// 「会保留」那一栏同样重要：人担心的正是「删了目标，采到的东西是不是也没了」。
/// 答案是不会，而且原因写在这里——作者归属推自 append-only 事实，不依赖这个目标。
fn deletion_modal(
    preview: &linggan_evidence::TargetDeletionPreview,
    target_ref: uuid::Uuid,
    error: Option<&str>,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    let blocked =
        preview.blocking_cross_industry_samples > 0 || preview.blocking_material_retirements > 0;
    let note = if blocked {
        let mut reasons = Vec::new();
        if preview.blocking_cross_industry_samples > 0 {
            reasons.push(format!(
                "{} 条跨行业样本",
                preview.blocking_cross_industry_samples
            ));
        }
        if preview.blocking_material_retirements > 0 {
            reasons.push(format!(
                "{} 条已确认的作品失效结论",
                preview.blocking_material_retirements
            ));
        }
        format!(
            r#"<p class="c-tg-delete-blocked">这个目标关联 {reasons}。它们是已经留下的材料或人工结论，删除目标不能把它们一起抹掉；在有独立保留方案前，这里不提供删除。</p>"#,
            reasons = escape(&reasons.join("、")),
        )
    } else {
        String::new()
    };
    let mismatch = if error == Some("target_delete_name_mismatch") {
        r#"<p class="c-tg-delete-blocked">名字对不上，没有删除任何东西。请原样输入上面那个名字。</p>"#
    } else {
        ""
    };
    let confirm = if blocked {
        String::new()
    } else {
        let focus_id = format!("target-{target_ref}");
        let fields = list_context.return_fields(None, None, Some(&focus_id));
        format!(
            r#"<form method="post" action="/collection/targets/delete">
                 <input type="hidden" name="row_target_ref" value="{target_ref}"/>
                 {fields}
                 <label>输入「{name}」以确认<input name="confirm_name" required autocomplete="off"/></label>
                 <button class="c-btn-primary c-tg-delete-go" type="submit">彻底删除</button>
               </form>"#,
            name = escape(&preview.confirmation_name),
        )
    };
    format!(
        r#"<div class="c-tg-batch-overlay">
             <section class="c-tg-batch-dialog c-tg-delete" role="dialog" aria-modal="true" aria-labelledby="target-delete-title">
               <h2 id="target-delete-title">彻底删除「{name}」</h2>
               {mismatch}
               {note}
               <div class="c-tg-delete-cols">
                 <div><b>会删掉</b><ul>
                   <li>这个观察目标本身与它的 {rules} 个规则版本</li>
                   <li>{requests} 次采集申请与准入决定</li>
                   <li>{orders} 张采集工单、{leases} 份租约、{tasks} 个执行任务</li>
                 </ul></div>
                 <div><b>不会删掉</b><ul>
                   <li>{works} 篇作品与 {details} 份详情，以及它们的评论</li>
                   <li>它们仍然属于这个博主：作者归属来自作品自己，不依赖观察目标是否存在</li>
                   <li>采集包与回执——数据库层禁止删除已经发生的事实</li>
                 </ul></div>
               </div>
               {confirm}
               <a class="c-btn-quiet" href="{cancel_href}">取消</a>
             </section>
           </div>"#,
        name = escape(&preview.confirmation_name),
        rules = preview.rule_revisions,
        requests = preview.requests,
        orders = preview.work_orders,
        leases = preview.leases,
        tasks = preview.lease_tasks,
        works = preview.retained_works,
        details = preview.retained_details,
        cancel_href = list_context.list_href(Some(&format!("target-{target_ref}"))),
    )
}

/// 关键词目标当前按什么排序采。
///
/// 从右边切：身份键是 `{词}::{排序}`，词本身可能含 `::`，而排序不含。
///
/// 五种排序都给中文——描述性标签必须只用中文（LIDS-LANG-001 · LANG-05），而且弹窗里
/// 已经是「最多点赞」，列表再显示 `most_liked` 就是同一件事两种说法。认不出的值保留
/// 原文：那是一个机器标识，藏起来会让人看不出这个目标到底在按什么采。
/// 关键词的档案状态。与创作者那一列同一套说法：**这个词的底座建好了没有**。
///
/// 这一列此前显示的是「最多点赞」这类排序口径——排序现在属于规则（`0076`），一个词可以有
/// 好几条，摆在列表上既不完整也不再是这一行的身份。而「建没建档」是这一行真正该先回答的：
/// 新建关键词默认采最多点赞前 200 篇，那就是它的档案。
fn keyword_archive_state(archive: super::target_drawer::KeywordArchiveRead) -> String {
    use super::target_drawer::KeywordArchiveRead;
    let (tone, label) = match archive {
        KeywordArchiveRead::NotArchived => ("neutral", "尚未建立"),
        KeywordArchiveRead::DetailPending => ("info", "建档中"),
        KeywordArchiveRead::Complete => ("ok", "档案已建立"),
        // 读不到就说读不到。压成「尚未建立」会催人重跑一次真实的平台访问。
        KeywordArchiveRead::Unavailable => ("neutral", "读不到"),
    };
    format!(r#"<span class="c-tg-truth c-tg-{tone}">{label}</span>"#)
}

/// 这个词命中了多少篇作品。
///
/// **「查到了，这个目标一篇都没有」与「没查到」是两件事。** 计数是批量读的：读成功时
/// 返回一张表，一个还没采过的目标根本不会出现在表里——那是 0，不是读不到。此前这两种
/// 情况都落在同一个 `None` 上，于是刚建好的关键词显示「读不到」，看着像坏了。
fn keyword_hit_count(
    counts: Option<&HashMap<uuid::Uuid, linggan_evidence::KeywordCatalogCounts>>,
    target_ref: uuid::Uuid,
) -> String {
    match counts {
        None => "读不到".to_owned(),
        Some(counts) => counts
            .get(&target_ref)
            .map_or(0, |counts| counts.works)
            .to_string(),
    }
}

/// 详情补到哪儿了。与创作者的「详情进度」同一种写法：`已取得 / 命中`。
fn keyword_detail_progress(
    counts: Option<&HashMap<uuid::Uuid, linggan_evidence::KeywordCatalogCounts>>,
    target_ref: uuid::Uuid,
) -> String {
    let Some(counts) = counts else {
        return "读不到".to_owned();
    };
    // 一篇都还没命中时不写 `0 / 0`——那读起来像「采过了，一篇都没有」。
    match counts.get(&target_ref) {
        None => "—".to_owned(),
        Some(counts) if counts.works == 0 => "—".to_owned(),
        Some(counts) => format!("{} / {}", counts.details, counts.works),
    }
}

fn archive_count(archive: super::target_drawer::TargetArchiveRead<'_>) -> String {
    match archive {
        super::target_drawer::TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        super::target_drawer::TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory())
            // The table column is count-only by the approved page display
            // contract, so canonical and historical directories scan alike.
            .map(|value| format!("{} 篇", value.works_listed))
            .unwrap_or_else(|| "—".to_owned()),
    }
}

fn detail_count(archive: super::target_drawer::TargetArchiveRead<'_>) -> String {
    match archive {
        super::target_drawer::TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        super::target_drawer::TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory() && value.works_listed > 0)
            .map(|value| {
                // 缺口读投影，不在这里相减：已确认失效的作品有详情之外的第三种去向，
                // 减法算不出来它。
                //
                // 缺 0 不显示——每一行都挂一个「缺 0」是纯噪音。但**缺口大于 0 时必须
                // 显示**：木可可的 `10 / 13` 里那 3 篇是已确认失效、不是缺失，读的人
                // 若自己做 13−10 会得出「缺 3」这个错误结论。这个数正是为此而投影的。
                if value.pending_details == 0 {
                    format!("{} / {}", value.details_captured, value.works_listed)
                } else {
                    format!(
                        "{} / {} · 缺 {}",
                        value.details_captured, value.works_listed, value.pending_details
                    )
                }
            })
            .unwrap_or_else(|| "—".to_owned()),
    }
}

/// 每行只提供一个主要动作。详情抽屉和巡查规则只是导航；建档/补采仍走同一条受控
/// 请求入口，真实结果由后端 durable receipt 决定，按钮本身不声称已经执行。
fn row_action(
    target: &ObservationTarget,
    is_creator: bool,
    archive: super::target_drawer::TargetArchiveRead<'_>,
    keyword_archive: super::target_drawer::KeywordArchiveRead,
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    use super::target_drawer::TargetPrimaryAction;
    let action =
        super::target_drawer::target_primary_action(target, is_creator, archive, keyword_archive);
    match action {
        TargetPrimaryAction::ViewKeyword => {
            let drawer_href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-tg-act" href="{drawer_href}">查看结果</a>"#)
        }
        TargetPrimaryAction::ViewCreator => {
            let drawer_href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-tg-act" href="{drawer_href}">查看档案</a>"#)
        }
        TargetPrimaryAction::ViewArchiveProgress
        | TargetPrimaryAction::ViewArchiveProblems
        | TargetPrimaryAction::ViewArchiveUnavailable => {
            let (label, fragment) = match action {
                TargetPrimaryAction::ViewArchiveProgress => ("查看任务", "archive-task"),
                TargetPrimaryAction::ViewArchiveProblems => ("处理异常", "archive-problems"),
                TargetPrimaryAction::ViewArchiveUnavailable => ("查看档案", "target-archive"),
                _ => unreachable!(),
            };
            let drawer_href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "overview")],
                Some(fragment),
            );
            format!(r#"<a class="c-tg-act" href="{drawer_href}">{label}</a>"#)
        }
        TargetPrimaryAction::OpenPatrol(label) => {
            let opener_id = format!("monitor-rule-{}", target.target_ref);
            let rule_href = list_context.monitor_rule_href(target.target_ref, &opener_id);
            format!(
                r#"<a id="{opener_id}" class="c-tg-act" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">{label}</a>"#,
                target_ref = target.target_ref,
            )
        }
        action @ (TargetPrimaryAction::EstablishArchive
        | TargetPrimaryAction::RebuildDirectory
        | TargetPrimaryAction::ContinueArchive) => {
            let label = match action {
                TargetPrimaryAction::EstablishArchive => "建立档案",
                TargetPrimaryAction::RebuildDirectory => "处理异常",
                TargetPrimaryAction::ContinueArchive => "补采缺口",
                _ => unreachable!(),
            };
            let focus_id = format!("target-{}", target.target_ref);
            let fields = list_context.return_fields(None, None, Some(&focus_id));
            format!(
                r#"<form method="post" action="/collection/targets/archive">{fields}<button class="c-tg-act" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
                target_ref = target.target_ref,
            )
        }
    }
}

/// 小红书号优先，采不到才退回平台 ID——小红书号是人能对上的那个。
fn identity_display(target: &ObservationTarget) -> String {
    fact_text(target.identity_facts.as_ref(), "redId")
        .unwrap_or_else(|| target.identity_key.clone())
}

fn fact_text(facts: Option<&Value>, key: &str) -> Option<String> {
    facts
        .and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
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
    use super::super::target_drawer::TargetListContext;
    use super::*;
    use uuid::Uuid;

    fn target(kind: &str, name: Option<&str>) -> ObservationTarget {
        ObservationTarget {
            target_ref: Uuid::new_v4(),
            platform: "xhs".to_owned(),
            target_kind: kind.to_owned(),
            identity_key: "5ebe6d21".to_owned(),
            display_name: name.map(str::to_owned),
            identity_facts: None,
            source: "plugin_push".to_owned(),
            lifecycle_state: "pending_decision".to_owned(),
            first_stored_at: "2026-08-26T20:00:00+08".to_owned(),
            monitoring_enabled: false,
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
            domain_name: None,
            domain_is_own: None,
        }
    }

    #[test]
    fn a_successful_empty_list_replaces_each_provisional_unreadable_state() {
        for provisional in ["观察目标列表暂时读不到", "目标计数与列表当前都不可读"]
        {
            let base = format!(
                "before<section class=\"c-empty c-empty-engineering\">{provisional}</section>after"
            );
            let html = render_stored_targets(
                &base,
                &[],
                &HashMap::new(),
                Some(&HashMap::new()),
                None,
                TargetListContext::default(),
            );
            assert!(html.contains("当前列表范围没有匹配的观察目标"));
            assert!(html.contains("当前筛选下没有观察目标"));
            assert!(!html.contains(provisional));
        }
    }

    #[test]
    fn stored_targets_never_claim_more_than_being_stored() {
        let base = format!("before{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}after");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("孩悦"))],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("孩悦"));
        // 存下目标不等于已经建立档案或跑过巡查。
        assert!(html.contains("尚未建立"));
        assert!(html.contains("未开启巡查"));
        assert!(html.contains("建立档案"));
        assert!(!html.contains("档案已建立"));
        assert!(!html.contains("巡查中"));
    }

    #[test]
    fn a_successful_list_replaces_an_unreadable_count_empty_state() {
        let base =
            "before<section class=\"c-empty c-empty-engineering\">count unreadable</section>after";
        let html = render_stored_targets(
            base,
            &[target("creator", Some("真实目标"))],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("真实目标"));
        assert!(!html.contains("count unreadable"));
        assert_eq!(html.matches("c-tg-workspace").count(), 1);
    }

    #[test]
    fn a_target_without_a_name_shows_its_identity_rather_than_an_invented_one() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(
            &base,
            &[target("keyword", None)],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("5ebe6d21"));
        assert!(!html.contains("未命名"));
    }

    #[test]
    fn target_text_is_escaped() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let html = render_stored_targets(
            &base,
            &[target("creator", Some("<script>x</script>"))],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>x"));
    }

    #[test]
    fn stored_target_does_not_render_a_remote_identity_avatar() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("真实创作者"));
        creator.identity_facts = Some(serde_json::json!({
            "avatar": "https://sns-avatar-qc.xhscdn.com/remote-avatar.jpg",
            "redId": "creator-001"
        }));

        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("creator-001"));
        assert!(!html.contains("remote-avatar.jpg"));
        assert!(!html.contains("<img"));
    }

    #[test]
    fn stored_target_renders_only_a_qualified_local_avatar_asset() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("本机头像作者"));
        let mut avatars = HashMap::new();
        avatars.insert(
            creator.target_ref,
            ObservationTargetAvatar::Local {
                local_asset_path: "/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &avatars,
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("/api/local/media/11111111-1111-4111-8111-111111111111/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
        assert!(html.contains("<img class=\"c-tg-avatar\""));
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }

    #[test]
    fn target_opener_keeps_the_filtered_list_context_and_has_a_focus_return_anchor() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("筛选内作者"));
        let html = render_stored_targets(
            &base,
            std::slice::from_ref(&creator),
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext {
                filter: Some("creator"),
                sort: Some("last"),
                domain: None,
            },
        );
        assert!(html.contains(&format!("id=\"target-{}\"", creator.target_ref)));
        assert!(html.contains(&format!("data-drawer-trigger=\"{}\"", creator.target_ref)));
        assert!(html.contains(&format!(
            "href=\"/collection/targets?filter=creator&amp;sort=last&amp;drawer={}\"",
            creator.target_ref
        )));
        assert!(html.contains(r#"name="return_filter" value="creator""#));
        assert!(html.contains(r#"name="return_sort" value="last""#));
        assert!(html.contains(&format!(
            r#"name="return_focus" value="target-{}""#,
            creator.target_ref
        )));
        assert!(html.contains(r#"<div class="c-tg-object" role="cell">"#));
        assert!(html.contains("data-row-opener"));
        assert!(!html.contains("c-tg-row-open"));
    }

    #[test]
    fn destructive_and_toggle_actions_keep_the_closed_list_context() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("上下文作者"));
        let domain = "11111111-1111-4111-8111-111111111111";
        let context = TargetListContext {
            filter: Some("creator"),
            sort: Some("last"),
            domain: Some(domain),
        };
        let preview = linggan_evidence::TargetDeletionPreview {
            confirmation_name: "上下文作者".to_owned(),
            target_kind: "creator".to_owned(),
            work_orders: 1,
            leases: 0,
            lease_tasks: 0,
            rule_revisions: 1,
            requests: 1,
            retained_works: 2,
            retained_details: 1,
            blocking_cross_industry_samples: 0,
            blocking_material_retirements: 0,
        };
        let html = render_stored_targets_with_observation(
            &base,
            std::slice::from_ref(&creator),
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            None,
            None,
            Some(&preview),
            Some(creator.target_ref),
            TargetListFacts::default(),
            context,
        );
        let target_ref = creator.target_ref;
        assert!(html.contains(&format!(
            "href=\"/collection/targets?domain={domain}&amp;filter=creator&amp;sort=last&amp;delete={target_ref}\""
        )));
        assert!(html.contains(&format!(r#"name="return_domain" value="{domain}""#)));
        assert!(html.contains(r#"name="return_filter" value="creator""#));
        assert!(html.contains(r#"name="return_sort" value="last""#));
        assert!(html.contains(&format!(
            r#"name="return_focus" value="target-{target_ref}""#
        )));
        assert!(html.contains(&format!(
            "href=\"/collection/targets?domain={domain}&amp;filter=creator&amp;sort=last#target-{target_ref}\""
        )));
    }

    #[test]
    fn unreadable_archive_state_never_becomes_an_unbuilt_archive_or_write_action() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("读取失败作者"));
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            None,
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("档案暂不可读"));
        assert_eq!(html.matches("当前读不到").count(), 2);
        assert!(html.contains(">查看档案</a>"));
        assert!(!html.contains(">建立档案</button>"));
        assert!(!html.contains(">继续完善</button>"));
        assert!(!html.contains(r#"action="/collection/targets/archive""#));
    }

    #[test]
    fn quarantined_archive_opens_its_real_problem_section_without_a_write_action() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("待处理作者"));
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: true,
                work_in_progress: false,
                author_profile_captures: 0,
                works_listed: 0,
                details_captured: 0,
                retired_works: 0,
                pending_details: 0,
                quarantined: 1,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("档案有问题"));
        assert!(html.contains(">处理异常</a>"));
        assert!(html.contains("#archive-problems"));
        assert!(!html.contains(">建立档案</button>"));
        assert!(!html.contains(">继续完善</button>"));
    }

    #[test]
    fn blocked_detail_is_a_visible_archive_problem_not_a_missing_page_or_completed_detail() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("读取受阻作者"));
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: false,
                work_in_progress: true,
                works_listed: 8,
                details_captured: 5,
                blocked_details: 1,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("档案有问题"));
        assert!(html.contains(">处理异常</a>"));
        assert!(html.contains("#archive-problems"));
        assert!(!html.contains("页面暂不可读"));
        assert!(!html.contains(">继续完善</button>"));
    }

    #[test]
    fn archive_progress_failures_use_business_language() {
        assert!(
            action_feedback_markup(Some("archive_in_progress"), None)
                .contains("已有一批作品正在补齐")
        );
        assert!(
            action_feedback_markup(Some("archive_nothing_to_continue"), None)
                .contains("当前作品目录没有待补详情")
        );
        assert!(action_feedback_markup(Some("archive_requested"), None).contains("建档已入队"));
        assert!(action_feedback_markup(Some("archive_merge"), None).contains("未重复提交"));
    }

    #[test]
    fn partial_historical_directory_is_not_rendered_as_a_detail_denominator() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("目录待重建作者"));
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: true,
                author_profile_captures: 1,
                works_listed: 31,
                details_captured: 27,
                directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::RebuildRequired,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("异常"));
        assert!(html.contains(">处理异常</button>"));
        assert!(!html.contains("31 篇"));
        assert!(!html.contains("27 / 31"));
    }

    #[test]
    fn completed_historical_directory_remains_visible_and_actionable() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("已有目录作者"));
        creator.lifecycle_state = "monitoring".to_owned();
        creator.monitoring_enabled = true;
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: true,
                works_listed: 41,
                details_captured: 41,
                directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::HistoricalDirectory,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("已有目录"));
        assert!(html.contains("41 篇"));
        assert!(
            !html.contains("41 篇 · 边界未知"),
            "the works-directory column is a count-only field"
        );
        // 缺口为 0 时不挂「缺 0」：每行都挂一个零值是纯噪音。
        assert!(html.contains("41 / 41"));
        assert!(!html.contains("缺 0"));
        assert!(html.contains(">查看档案</a>"));
        assert!(!html.contains("建立标准目录"));
    }

    #[test]
    fn pending_legacy_archive_opens_status_without_offering_a_duplicate_request() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("待处理作者"));
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                work_in_progress: true,
                directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::Building,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("建档中"));
        assert!(html.contains(">查看任务</a>"));
        assert!(!html.contains(">建立档案</button>"));
    }

    #[test]
    fn non_live_unusable_archive_never_pairs_an_error_state_with_view_archive() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("停滞建档作者"));
        creator.lifecycle_state = "monitoring".to_owned();
        creator.monitoring_enabled = true;
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: true,
                directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::Building,
                ..ArchiveCompleteness::default()
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("档案有问题"));
        assert!(html.contains(">处理异常</a>"));
        assert!(!html.contains(">查看档案</a>"));
    }

    #[test]
    fn creator_and_keyword_use_distinct_columns_and_one_primary_action_per_row() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let creator = target("creator", Some("作者"));
        let keyword = target("keyword", Some("关键词"));
        let html = render_stored_targets(
            &base,
            &[creator, keyword],
            &HashMap::new(),
            Some(&HashMap::new()),
            None,
            TargetListContext::default(),
        );
        assert!(html.contains("创作者档案"));
        assert!(html.contains("关键词观察"));
        assert!(html.contains("c-tg-creator-grid"));
        assert!(html.contains("c-tg-keyword-grid"));
        assert!(html.contains(
            r#"作品目录</span><span class="c-tg-head-num c-tg-head-end" role="columnheader">详情进度"#
        ));
        // 两张表的「最近新增」必须落在同一列位（第 10 位）。它们读的是同一个字段
        // `latest_new`——创作者那边此前叫「最近变化」、排在第 10 位，关键词这边叫
        // 「最近新增」、排在第 8 位：同一件事，两个名字，两个位置。现已统一。
        //
        // 关键词这两列此前叫「最近命中／数据更新」，后者还是写死的「尚未取得」——一整列
        // 纯装饰。现在与创作者同义：命中作品／详情进度，两边纵向对得上，读的也都是真数。
        assert!(html.contains(
            r#"命中作品</span><span class="c-tg-head-num c-tg-head-end" role="columnheader">详情进度"#
        ));
        // 第 8 列（详情进度）在两张表里都单独多留右内边距，一起左移。
        assert_eq!(html.matches("c-tg-head-end").count(), 2);
        assert_eq!(html.matches("c-tg-cell-end").count(), 2);
        assert!(html.contains(r#"巡查状态</span><span role="columnheader">最近新增"#));
        assert!(!html.contains("最近变化"));
        // 数字列的表头必须与右对齐的数字同侧，否则一列两端各站一边，看着就是错位。
        // 4 处：创作者表的 作品目录/详情进度，关键词表的 命中作品/详情进度。
        assert_eq!(html.matches("c-tg-head-num").count(), 4);
        assert!(html.contains("打开作者的创作者档案"));
        assert!(html.contains("打开关键词的关键词观察"));
        assert!(!html.contains("关键词档案"));
        assert_eq!(html.matches("c-tg-actions").count(), 2);
        // 三个行内控件现在共用一套尺寸（34px + 1px 墨线），靠颜色与形状区分而不是靠大小。
        //
        // 判据用**引号收尾的完整 class**，不用裸子串：`c-tg-act-danger` 自身就含
        // `c-tg-act`，按子串数会把一个删除按钮算成两次。
        assert_eq!(html.matches(r#"class="c-tg-act""#).count(), 2);
        assert_eq!(html.matches(r#"c-tg-act c-tg-act-danger""#).count(), 2);
        assert_eq!(html.matches("c-tg-switch").count(), 2);
        // 旧的三种尺寸（40px 描边按钮 / 28px 浅按钮 / 下划线文字）必须彻底消失。
        assert!(!html.contains("c-tg-btn-slim"));
        assert!(!html.contains("c-btn-quiet"));
        // 观察开关与删除必须每行各出现一次。区分不再靠尺寸（三者已统一成 34px），
        // 靠形状与颜色：开关是唯一带滑轨的，删除是唯一的红。
        assert_eq!(html.matches("c-tg-toggle").count(), 2);
        // 关键词档案读不到时不提供进入巡查的入口；未知不能被当成完成。
        assert_eq!(html.matches("data-monitor-rule-trigger").count(), 0);
        assert_eq!(html.matches(">建立档案</button>").count(), 1);
        assert_eq!(html.matches(">查看档案</a>").count(), 1);
        assert_eq!(html.matches("type=\"checkbox\"").count(), 4);
        assert!(html.contains("id=\"target-batch-modal-form\""));
        assert!(html.contains("form=\"target-batch-modal-form\" data-target-select"));
        assert!(!html.contains("data-target-batch-open"));
        assert!(html.contains("批量编辑观察目标"));
        assert!(html.contains("name=\"action\" value=\"set_group\""));
        assert!(!html.contains("/collection/targets/monitoring"));
        assert!(!html.contains("name=\"action\" value=\"monitor_on\""));
        assert!(!html.contains("name=\"action\" value=\"monitor_off\""));
    }

    #[test]
    fn creator_row_separates_archive_progress_and_successful_patrol_times() {
        let base = format!("{EMPTY_STATE_OPEN}empty{EMPTY_STATE_CLOSE}");
        let mut creator = target("creator", Some("档案进度作者"));
        creator.monitoring_enabled = true;
        creator.lifecycle_state = "monitoring".to_owned();
        creator.last_patrol_dispatched_at = Some("2026-09-04 09:00".to_owned());
        creator.last_patrol_succeeded_at = Some("2026-09-04 08:30".to_owned());
        creator.next_patrol_at = Some("2026-09-05 09:00".to_owned());
        let mut completeness = HashMap::new();
        completeness.insert(
            creator.identity_key.clone(),
            ArchiveCompleteness {
                started: true,
                attempted: true,
                work_in_progress: false,
                author_profile_captures: 1,
                works_listed: 12,
                details_captured: 5,
                retired_works: 0,
                pending_details: 7,
                quarantined: 0,
                blocked_details: 0,
                directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::Ready,
            },
        );
        let html = render_stored_targets(
            &base,
            &[creator],
            &HashMap::new(),
            Some(&completeness),
            None,
            TargetListContext::default(),
        );

        assert!(html.contains("详情有缺口"));
        assert!(html.contains("12 篇"));
        // 缺口大于 0 时必须显示：这一格的 12 篇里有已确认失效的，读的人不能靠
        // 12−5 自己算——那会把「已失效」误算成「缺失」。
        assert!(html.contains("5 / 12 · 缺 7"));
        assert!(html.contains("补采缺口"));
        // 上次巡查是已经发生的事实，保留绝对时刻——可能要拿去跟别的记录对时间。
        // 列表里的巡查时刻不带年份：同一屏里年份永远相同，却稳定占掉五个字符，
        // 正是把这一列挤到截断的最后一根稻草。完整时刻仍在目标详情里。
        assert!(html.contains("09-04 08:30"));
        assert!(!html.contains("2026-09-04 08:30"));
        // 下次巡查写成还有多久：人在这一列判断的是要等多久，而不是那一刻的钟点。
        assert!(!html.contains("2026-09-05 09:00"));
        // 断言「已逾期」而不是「已逾期 || 后」：夹具的下次巡查固定在 2026-09-05，时间
        // 只会往前走，这一格永远是逾期，判据因此是确定的。原先那个 `|| "后"` 是条一字
        // 逃生口——本文件别处文案就含「日后恢复」，任何这类文字落进渲染片段，断言就
        // 永久为真，这一列从此不再被测。
        assert!(html.contains("已逾期"));
        assert!(!html.contains("2026-09-04 09:00"));
        assert!(!html.contains("ARCHIVE HEALTH"));
        assert!(!html.contains('%'));
    }
}

#[cfg(test)]
mod keyword_archive_action_tests {
    use super::super::target_drawer::{KeywordArchiveRead, TargetArchiveRead, TargetPrimaryAction};
    use super::*;

    fn keyword(lifecycle: &str, monitoring: bool) -> ObservationTarget {
        ObservationTarget {
            target_ref: uuid::Uuid::new_v4(),
            platform: "xhs".to_owned(),
            target_kind: "keyword".to_owned(),
            identity_key: "考研自习::most_liked".to_owned(),
            display_name: Some("考研自习".to_owned()),
            identity_facts: None,
            source: "manual".to_owned(),
            lifecycle_state: lifecycle.to_owned(),
            first_stored_at: "2026-09-11T09:00:00+08".to_owned(),
            monitoring_enabled: monitoring,
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
            domain_name: None,
            domain_is_own: None,
        }
    }

    fn action(
        target: &ObservationTarget,
        keyword_archive: KeywordArchiveRead,
    ) -> TargetPrimaryAction {
        super::super::target_drawer::target_primary_action(
            target,
            false,
            TargetArchiveRead::Known(None),
            keyword_archive,
        )
    }

    /// 一个还没建过档的词，下一步是建档，不是直接开始每周看增量。
    ///
    /// 历史高赞是这个词的底座；没有底座就开始数「本周新增」，等于在一张空表上数新增。
    #[test]
    fn an_unarchived_keyword_is_asked_to_build_its_archive_first() {
        assert_eq!(
            action(
                &keyword("pending_decision", false),
                KeywordArchiveRead::NotArchived
            ),
            TargetPrimaryAction::EstablishArchive
        );
    }

    /// 链接拿到了、详情还没补，下一步是继续建档，不是开始巡检。
    ///
    /// 列表面只给得出标题、封面和点赞；正文、评论、发布时间都在详情页里。停在这一步
    /// 就开始每周看增量，等于拿一个只有半张脸的底座去比对新增。
    #[test]
    fn a_keyword_still_missing_details_is_asked_to_continue() {
        assert_eq!(
            action(
                &keyword("pending_decision", false),
                KeywordArchiveRead::DetailPending
            ),
            TargetPrimaryAction::ContinueArchive
        );
    }

    /// 链接和详情都齐了之后，下一步才是开始每周巡检。
    #[test]
    fn an_archived_keyword_is_offered_the_weekly_patrol() {
        assert_eq!(
            action(
                &keyword("pending_decision", false),
                KeywordArchiveRead::Complete
            ),
            TargetPrimaryAction::OpenPatrol("开始每周巡检")
        );
    }

    /// **读不到就不猜**：既不催人建档（可能已经建过），也不允许它进入巡查。
    ///
    /// 把「读不到」当成「没建过」，会让页面催人重做一件可能已经做完的事；而重做一次
    /// 关键词建档要真实访问平台。
    #[test]
    fn an_unreadable_archive_state_blocks_patrol_entry() {
        assert_eq!(
            action(
                &keyword("pending_decision", false),
                KeywordArchiveRead::Unavailable
            ),
            TargetPrimaryAction::ViewArchiveUnavailable
        );
    }

    /// **底座还没建起来，监控中也要给建档入口。**
    ///
    /// 行上写着「尚未建立」，就必须有一个动作能把它建起来。此前这一条把 `NotArchived`
    /// 也归进「监控中的词不再被问建档」，于是一个还没建过档就打开了观察开关的词，永远
    /// 只能靠先关掉观察再去点建档——那是界面逼出来的绕路。巡查按口径取的是每期增量，
    /// 历史那一整段只有建档拿得回来。
    #[test]
    fn a_monitored_keyword_without_an_archive_is_still_offered_it() {
        assert_eq!(
            action(
                &keyword("monitoring", true),
                KeywordArchiveRead::NotArchived
            ),
            TargetPrimaryAction::EstablishArchive
        );
    }

    /// 已在观察的历史关键词不被伪装成完成；只有完整建档才提供结果查看。
    #[test]
    fn a_monitored_keyword_with_a_finished_archive_is_not_asked_to_archive() {
        assert_eq!(
            action(&keyword("monitoring", true), KeywordArchiveRead::Complete),
            TargetPrimaryAction::ViewKeyword,
        );
        assert_eq!(
            action(
                &keyword("monitoring", true),
                KeywordArchiveRead::Unavailable
            ),
            TargetPrimaryAction::ViewArchiveUnavailable,
        );
    }

    /// **监控中的词，详情欠着时仍要够得着补详情。**
    ///
    /// 巡检每周带回来的新笔记同样只有链接。这一条不排在「查看结果」前面，补详情的入口
    /// 一进入监控就消失，这个词的面貌永远停在列表面那一层——正文、发布时间、评论一样
    /// 都没有，而界面上看不出少了什么。
    #[test]
    fn a_monitored_keyword_still_reaches_its_missing_details() {
        assert_eq!(
            action(
                &keyword("monitoring", true),
                KeywordArchiveRead::DetailPending
            ),
            TargetPrimaryAction::ContinueArchive
        );
    }

    /// 暂停巡查不等于放弃已经拿到的链接：补详情是人手动点的一次动作，仍该够得着。
    #[test]
    fn a_paused_keyword_can_still_finish_its_details() {
        assert_eq!(
            action(&keyword("paused", false), KeywordArchiveRead::DetailPending),
            TargetPrimaryAction::ContinueArchive
        );
    }

    /// 已弃用的目标只提供查看，不被任何采集动作打断。
    ///
    /// 钉死结果而不是只排除一个动作：`assert_ne!(_, ContinueArchive)` 放得过
    /// 「建立档案」和「开始每周巡检」，而那两个同样是在催一个已经被弃用的目标去采集。
    ///
    /// 这一条在 main 上是红的——弃用的关键词此前会落到建档三态的 match 里，拿到
    /// 「继续建档」或「开始每周巡检」按钮。
    #[test]
    fn a_dismissed_keyword_is_offered_only_a_read_only_view() {
        for archive in [
            KeywordArchiveRead::NotArchived,
            KeywordArchiveRead::DetailPending,
            KeywordArchiveRead::Complete,
            KeywordArchiveRead::Unavailable,
        ] {
            assert_eq!(
                action(&keyword("dismissed", false), archive),
                TargetPrimaryAction::ViewKeyword,
                "已弃用的目标不该被任何采集动作打断"
            );
        }
    }

    /// 暂停的关键词只有建档完成后才能恢复巡查。
    #[test]
    fn a_paused_keyword_without_an_archive_must_build_it_first() {
        assert_eq!(
            action(&keyword("paused", false), KeywordArchiveRead::NotArchived),
            TargetPrimaryAction::EstablishArchive
        );
    }
}

#[cfg(test)]
mod queue_toast_tests {
    use super::action_feedback_markup;
    use linggan_evidence::{DETAIL_WINDOW_COMMENT_LIMIT, DETAIL_WINDOW_REPLY_EXPAND_LIMIT};

    /// 回执说的范围必须就是真去读的范围。
    ///
    /// 关键词详情补采与创作者观察同口径：一次打开带回详情、前 30 条评论与 2 层回复
    /// （`DETAIL_WINDOW_*`），以及媒体文件。这句写着「只补正文与发布时间」的时候，人和系统
    /// 对同一次采集的理解差着两样东西——纸面窄、实际宽，人就不会去等评论，也不会去查它们
    /// 为什么没回来。
    ///
    /// 数字从窗口常量取，不写死：窗口改了而文案没改，这条就红。
    ///
    /// 媒体那一条**只能断言「有没有提」，不能像数字那样对着常量咬**——媒体是三个布尔授权，
    /// 没有可写进文案的量。所以它比上面两条弱：它挡得住「有人把媒体从文案里删掉」，挡不住
    /// 「有人把 `acquire_media` 改回 false 而文案没跟着改」。写明这个强度，免得后来人以为
    /// 它守得比实际多。
    #[test]
    fn the_keyword_detail_receipt_states_the_window_that_is_actually_read() {
        let markup = action_feedback_markup(Some("keyword_detail_requested"), Some(3));
        assert!(
            markup.contains(&format!("{DETAIL_WINDOW_COMMENT_LIMIT} 条评论")),
            "回执没有说这次会读回评论区：{markup}"
        );
        assert!(
            markup.contains(&format!("{DETAIL_WINDOW_REPLY_EXPAND_LIMIT} 层回复")),
            "回执没有说这次会展开回复：{markup}"
        );
        assert!(
            markup.contains("图片与视频"),
            "回执没有说这次会把媒体文件取回来——它就写在同一句里：{markup}"
        );
    }

    /// 排队位置只在右下角那条回执里说一次，不在行里常驻一句描述。
    ///
    /// 回执正文也不能再提"行里那句话"——那行字已经删了，留着就是一句谎话。
    ///
    /// 那行字此前挤在操作按钮之间，窄一点就和「补采缺口」叠在一起——一个用来消除焦虑的
    /// 提示，自己先变成了显示故障。
    #[test]
    fn the_success_receipt_says_how_many_are_ahead() {
        let markup = action_feedback_markup(Some("archive_requested"), Some(3));
        assert!(markup.contains("c-tg-toast"), "排队位置属于右下角那条回执");
        assert!(markup.contains("前面还有 3 个"));
    }

    /// 排在最前面时不说「前面还有 0 个」——那读起来像没在排队。
    #[test]
    fn the_front_of_the_queue_is_not_written_as_zero_ahead() {
        let markup = action_feedback_markup(Some("archive_requested"), Some(0));
        assert!(markup.contains("就排在最前面"));
        assert!(!markup.contains("0 个"));
    }

    /// 读不到位置就不说这句，其余回执照常。**不编一个数字**：人会拿它估还要等多久。
    #[test]
    fn an_unknown_position_stays_silent_without_losing_the_receipt() {
        let markup = action_feedback_markup(Some("archive_requested"), None);
        assert!(markup.contains("建档已入队"));
        assert!(!markup.contains("前面还有"));
        assert!(!markup.contains("排在最前面"));
    }

    /// 失败回执不是右下角角标，也不带排队位置——没成功的事不该自动消失。
    #[test]
    fn a_failure_is_not_turned_into_a_disappearing_toast() {
        let markup = action_feedback_markup(Some("archive_refuse"), Some(3));
        assert!(!markup.contains("c-tg-toast"));
        assert!(!markup.contains("前面还有"));
    }

    /// 行里不再有任何常驻的排队描述——样式与标记都不该留着。
    ///
    /// 这条断言第一版查错了常量（查的是 `COLLECTION_WORKSPACE_CSS`，而那条规则住在
    /// `TARGET_DRAWER_CSS`），于是样式还在、断言照绿。查的东西不对，断言再多也是装饰。
    ///
    /// 第二版想顺便查渲染标记，写成了「读这个文件自己的源码」——结果断言自己那行字
    /// 就命中了它要找的字符串，永远为真。自引用的检查不是检查。
    #[test]
    fn no_row_carries_a_standing_queue_sentence() {
        assert!(!crate::local_web::TARGET_DRAWER_CSS.contains("c-tg-queued"));
        for code in ["archive_requested", "keyword_detail_requested"] {
            assert!(
                !action_feedback_markup(Some(code), Some(3)).contains("按钮旁"),
                "{code} 的回执还在指着一句已经删掉的行内提示"
            );
        }
    }

    /// 回执要盖在抽屉之上——抽屉一开，点「建立档案」拿到的回执正好在它下面，最该看见
    /// 它的时候看不见。
    ///
    /// 断言的是**用了哪个层级 token**，不是某个数字。此前这里写死 `z-index:80`，与抽屉
    /// 同层，而抽屉在 DOM 里靠后——同一个 z-index 由 DOM 顺序裁决，回执就落到了抽屉下面。
    /// 数字写对一次没有意义，改一次 token 就会再错一次。
    ///
    /// 抽屉自己也是 `var(--lgi-z-drawer)`，所以这条不会因为「两边都用了 token」而假绿：
    /// 下面同时核对 token 阶梯本身。
    #[test]
    fn the_receipt_is_layered_above_the_drawer_it_must_be_read_over() {
        // 「回执盖过抽屉」是**两侧一起**成立的不变量。只钉回执那一侧会留下一个假绿窗口：
        // 把抽屉写死成 999（或换挂别的 token）会重新盖住回执，而只核对回执的断言照样绿。
        // 这个页面的抽屉就是 `.c-dw`（`target_drawer.rs` 渲染 `<aside id="c-drawer" class="c-dw">`），
        // 两条规则都在同一张 `TARGET_DRAWER_CSS` 里。
        let declaration = |selector: &str| -> String {
            let css = crate::local_web::TARGET_DRAWER_CSS;
            let rule = css
                .split(&format!("{selector}{{"))
                .nth(1)
                .unwrap_or_else(|| panic!("TARGET_DRAWER_CSS 里应当有一条 {selector} 规则"));
            rule.split('}').next().unwrap_or(rule).to_owned()
        };

        let receipt = declaration(".c-tg-toast");
        assert!(
            receipt.contains("z-index:var(--lgi-z-toast)"),
            "回执必须用 --lgi-z-toast 定层，写死数字会再次落在抽屉下面：\n  {}",
            receipt
        );
        let drawer = declaration(".c-dw");
        assert!(
            drawer.contains("z-index:var(--lgi-z-drawer)"),
            "抽屉也必须由 --lgi-z-drawer 定层：写死一个更大的数字、或改挂别的 token，都会重新盖住回执，\
             而只钉回执那一侧的断言不会变红：\n  {}",
            drawer
        );

        let level = |token: &str| -> i32 {
            let tokens = crate::local_web::LIDS_TOKENS;
            let declaration = tokens
                .split(&format!("{token}:"))
                .nth(1)
                .unwrap_or_else(|| panic!("LIDS_TOKENS 里没有 {token}"));
            declaration
                .split(';')
                .next()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or_else(|| panic!("{token} 的取值不是一个整数层级"))
        };
        assert!(
            level("--lgi-z-toast") > level("--lgi-z-drawer"),
            "回执的层级必须高于抽屉"
        );
    }
}

#[cfg(test)]
mod keyword_counts_tests {
    use super::{keyword_detail_progress, keyword_hit_count};
    use linggan_evidence::KeywordCatalogCounts;
    use std::collections::HashMap;

    /// **「查到了，这个目标一篇都没有」与「没查到」是两件事。**
    ///
    /// 计数是批量读的：读成功时返回一张表，一个还没采过的目标根本不会出现在表里——那是 0，
    /// 不是读不到。此前两种情况共用一个 `None`，于是刚建好的关键词显示「读不到」，
    /// 看着像坏了；而真读不到时又说不出来。
    #[test]
    fn a_target_absent_from_a_successful_read_has_zero_not_unknown() {
        let target_ref = uuid::Uuid::new_v4();
        let empty = HashMap::new();
        assert_eq!(keyword_hit_count(Some(&empty), target_ref), "0");
        assert_eq!(keyword_detail_progress(Some(&empty), target_ref), "—");
    }

    /// 读失败才说读不到。
    #[test]
    fn an_unreadable_count_says_so() {
        let target_ref = uuid::Uuid::new_v4();
        assert_eq!(keyword_hit_count(None, target_ref), "读不到");
        assert_eq!(keyword_detail_progress(None, target_ref), "读不到");
    }

    /// 有命中时按「已取得 / 命中」写，与创作者那一列同一种写法。
    #[test]
    fn a_counted_target_reads_as_details_over_hits() {
        let target_ref = uuid::Uuid::new_v4();
        let mut counts = HashMap::new();
        counts.insert(
            target_ref,
            KeywordCatalogCounts {
                works: 218,
                details: 60,
            },
        );
        assert_eq!(keyword_hit_count(Some(&counts), target_ref), "218");
        assert_eq!(
            keyword_detail_progress(Some(&counts), target_ref),
            "60 / 218"
        );
    }
}

#[cfg(test)]
mod keyword_archive_read_tests {
    use super::super::target_drawer::KeywordArchiveRead;
    use super::keyword_archive_read;
    use std::collections::HashSet;

    fn only(target: uuid::Uuid) -> HashSet<uuid::Uuid> {
        HashSet::from([target])
    }

    /// 翻完搜索面只是建档的上半段；详情还欠着的时候，状态必须说得出来。
    #[test]
    fn an_archived_keyword_with_pending_details_is_reported_as_pending() {
        let target = uuid::Uuid::new_v4();
        assert_eq!(
            keyword_archive_read(Some(&only(target)), Some(&only(target)), target),
            KeywordArchiveRead::DetailPending
        );
    }

    /// 两段都完成了才是完成。
    #[test]
    fn both_halves_done_is_complete() {
        let target = uuid::Uuid::new_v4();
        assert_eq!(
            keyword_archive_read(Some(&only(target)), Some(&HashSet::new()), target),
            KeywordArchiveRead::Complete
        );
    }

    /// 详情那一问读不到时，不谎称补齐，也不倒退成「没建过」。
    ///
    /// 说成补齐会把一个半成品底座推进巡检；说成没建过会催人重跑一次真实的平台访问。
    #[test]
    fn an_unreadable_detail_state_is_reported_as_unreadable() {
        let target = uuid::Uuid::new_v4();
        assert_eq!(
            keyword_archive_read(Some(&only(target)), None, target),
            KeywordArchiveRead::Unavailable
        );
    }

    /// 还没翻完的词，详情那一问的结果不影响结论。
    #[test]
    fn an_unarchived_keyword_stays_unarchived() {
        let target = uuid::Uuid::new_v4();
        for pending in [None, Some(&HashSet::new())] {
            assert_eq!(
                keyword_archive_read(Some(&HashSet::new()), pending, target),
                KeywordArchiveRead::NotArchived
            );
        }
    }
}

#[cfg(test)]
mod domain_gate_tests {
    use super::*;

    /// 领域必须由人选定，不能由「当前在看哪个领域」推断。
    ///
    /// 2026-09-11 真实发生：两个本想做跨行业参照的关键词在「全部领域」视图下建出来，
    /// 被静默归进本领域，413 条笔记直接写进了 ADHD 证据库，而跨行业语料表一条都没有。
    /// 参照物一旦混进证据，之后任何读证据的地方都不会再提醒。
    #[test]
    fn the_missing_domain_notice_explains_what_the_choice_decides() {
        let markup = action_feedback_markup(Some("target_domain_required"), None);
        assert!(markup.contains("还没选领域"));
        // 提示必须说清后果，而不只是「请选择」——不然人不知道为什么这一项不能省。
        assert!(markup.contains("证据库"));
        assert!(markup.contains("跨行业"));
    }

    /// 建档提示此前只说「主页作品链接」，那是创作者的说法；关键词翻的是搜索面。
    #[test]
    fn the_archive_notice_covers_both_kinds_of_target() {
        let markup = action_feedback_markup(Some("archive_requested"), None);
        assert!(markup.contains("创作者取主页目录"));
        assert!(markup.contains("关键词按排序翻搜索面"));
        // 执行要排队等空闲工位，实测等过 213 秒。不说这件事，人会以为点了没反应。
        assert!(markup.contains("空闲工位"));
    }
}

#[cfg(test)]
mod domain_unassigned_notice_tests {
    use super::*;

    /// 「缺领域」必须说清是缺领域，并给出下一步。
    ///
    /// 它此前在三条真实路径上被吞成别的意思：人工立即观察显示「数据库不可用」（人会去
    /// 查服务是不是挂了）、关键词建档显示「上一次动作没有完成」（信息量最低的兜底）、
    /// 创作者建档显示「当前状态不允许再次发起」（**会把人引向一个永远不会到来的状态
    /// 流转**）。三条都不是真的，而这一轮要解决的恰恰就是这类看不懂。
    #[test]
    fn the_unassigned_domain_notice_names_the_cause_and_the_next_step() {
        let markup = action_feedback_markup(Some("target_domain_unassigned"), None);
        assert!(markup.contains("还没归属领域"));
        assert!(markup.contains("采集没有开始"), "要先说清什么都没发生");
        assert!(markup.contains("先给这个目标指定领域"), "要给出下一步");
        // 明确排除两条错误的归因，免得再被读成状态问题或故障。
        assert!(markup.contains("不是等状态流转"));
        assert!(markup.contains("不是服务出了问题"));
    }
}
