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
        list_context,
    );
    let keyword_table = target_table(
        "keyword",
        "关键词观察",
        targets,
        avatars,
        completeness,
        observation,
        list_context,
    );

    // Creator 与 keyword 的事实不同，但列表管理动作相同。选择由表格提供，主动作
    // 固定在顶栏；两者以 HTML form 属性关联，不能把行操作 form 嵌进一个外层 form。
    // 否则浏览器会纠正无效嵌套，导致后续复选框和批量提交失去共同的表单归属。
    let list = format!(
        r#"<section class="c-tg-workspace">
              {failure}
              <div class="c-tg-list-head">
                <span>{count} 个观察目标</span>
                <span class="c-tg-list-hint">勾选后可在顶部批量编辑；点击一行查看详情</span>
              </div>
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
            </section>"#,
        count = targets.len(),
        failure = action_feedback_markup(error),
    );
    replace_target_state(base, &list)
}

fn target_table(
    kind: &str,
    title: &str,
    targets: &[ObservationTarget],
    avatars: &HashMap<uuid::Uuid, ObservationTargetAvatar>,
    completeness: Option<&HashMap<String, ArchiveCompleteness>>,
    observation: Option<&HashMap<uuid::Uuid, TargetObservationSummary>>,
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
            list_context,
        ));
    }
    let columns = if is_creator {
        r#"<span role="columnheader"><label class="c-tg-select-all"><input type="checkbox" data-target-select-all aria-label="选择全部创作者目标"/></label></span><span role="columnheader">编号</span><span role="columnheader">创作者</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">档案状态</span><span role="columnheader">作品目录</span><span role="columnheader">详情进度</span><span role="columnheader">巡查状态</span><span role="columnheader">最近变化</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
    } else {
        r#"<span role="columnheader"><label class="c-tg-select-all"><input type="checkbox" data-target-select-all aria-label="选择全部关键词目标"/></label></span><span role="columnheader">编号</span><span role="columnheader">关键词</span><span role="columnheader">平台</span><span role="columnheader">分组</span><span role="columnheader">规则</span><span role="columnheader">最近命中</span><span role="columnheader">最近新增</span><span role="columnheader">巡查状态</span><span role="columnheader">数据更新</span><span role="columnheader">上次巡查</span><span role="columnheader">下次巡查</span><span role="columnheader">操作</span>"#
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
fn action_feedback_markup(error: Option<&str>) -> String {
    let Some(code) = error else {
        return String::new();
    };
    let (class, heading, explanation) = match code {
        "archive_requested" => (
            "c-src-feedback c-src-feedback-ok",
            "建档已入队",
            "已记录这次建立档案请求。系统会先获取主页作品链接（最多 200 篇或主页实际结束），再逐篇补齐详情；目录和详情只会在接纳真实回执后更新。",
        ),
        "archive_merge" => (
            "c-src-feedback c-src-feedback-warn",
            "未重复提交",
            "已有相同建档任务等待处理或执行中。已打开建档状态；本次没有创建第二个任务。",
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
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
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
    let last = target
        .last_patrol_succeeded_at
        .as_deref()
        .unwrap_or("尚未巡查");
    let next = if target.monitoring_enabled {
        target.next_patrol_at.as_deref().unwrap_or("待排定")
    } else {
        "—"
    };
    let cells = if is_creator {
        format!(
            r#"<div class="c-tg-cell" role="cell">{archive_state}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{works}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{details}</div>
                <div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-change" role="cell">{recent_change}</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell" data-row-no-open>{actions}</div>"#,
            archive_state = archive_state(target, archive),
            works = archive_count(archive),
            details = detail_count(archive),
            patrol = patrol_state(target, observation),
            recent_change = creator_recent_change(observation),
            last = escape(last),
            next = escape(next),
            actions = row_action(target, true, archive, list_context),
        )
    } else {
        format!(
            r#"<div class="c-tg-cell c-tg-rule" role="cell">{rule}</div>
                <div class="c-tg-cell c-tg-number-value" role="cell">{hits}</div>
                <div class="c-tg-cell c-tg-change" role="cell">{recent_change}</div>
                <div class="c-tg-cell" role="cell">{patrol}</div>
                <div class="c-tg-cell c-tg-unknown" role="cell">尚未取得</div>
                <time class="c-tg-cell c-tg-time" role="cell">{last}</time>
                <time class="c-tg-cell c-tg-time" role="cell">{next}</time>
                <div class="c-tg-actions" role="cell" data-row-no-open>{actions}</div>"#,
            rule = keyword_rule(target),
            patrol = patrol_state(target, observation),
            hits = keyword_hits(observation),
            recent_change = keyword_recent_change(observation),
            last = escape(last),
            next = escape(next),
            actions = row_action(target, false, archive, list_context),
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
        TargetArchiveRead::Known(Some(value)) if value.work_in_progress => ("warn", "建档中"),
        TargetArchiveRead::Known(Some(value)) if value.quarantined > 0 => ("warn", "档案有问题"),
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
            if value.works_listed > 0 && value.details_captured < value.works_listed =>
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

fn keyword_rule(target: &ObservationTarget) -> String {
    target
        .identity_key
        .split_once("::")
        .map(|(_, ranking)| match ranking {
            "comprehensive" => "综合排序",
            "latest" => "最新排序",
            other => other,
        })
        .unwrap_or("当前规则")
        .to_owned()
}

fn archive_count(archive: super::target_drawer::TargetArchiveRead<'_>) -> String {
    match archive {
        super::target_drawer::TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        super::target_drawer::TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory())
            .map(|value| {
                if value.directory_baseline
                    == linggan_evidence::ArchiveDirectoryBaseline::HistoricalDirectory
                {
                    format!("{} 篇 · 边界未知", value.works_listed)
                } else {
                    format!("{} 篇", value.works_listed)
                }
            })
            .unwrap_or_else(|| "—".to_owned()),
    }
}

fn detail_count(archive: super::target_drawer::TargetArchiveRead<'_>) -> String {
    match archive {
        super::target_drawer::TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        super::target_drawer::TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory() && value.works_listed > 0)
            .map(|value| {
                let missing = (value.works_listed - value.details_captured).max(0);
                format!(
                    "{} / {} · 缺 {}",
                    value.details_captured, value.works_listed, missing
                )
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
    list_context: super::target_drawer::TargetListContext<'_>,
) -> String {
    use super::target_drawer::TargetPrimaryAction;
    let action = super::target_drawer::target_primary_action(target, is_creator, archive);
    match action {
        TargetPrimaryAction::ViewKeyword => {
            let drawer_href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看结果</a>"#)
        }
        TargetPrimaryAction::ViewCreator => {
            let drawer_href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">查看档案</a>"#)
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
            format!(r#"<a class="c-btn-secondary c-tg-btn" href="{drawer_href}">{label}</a>"#)
        }
        TargetPrimaryAction::OpenPatrol(label) => {
            let opener_id = format!("monitor-rule-{}", target.target_ref);
            let rule_href = list_context.monitor_rule_href(target.target_ref, &opener_id);
            format!(
                r#"<a id="{opener_id}" class="c-btn-secondary c-tg-btn" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">{label}</a>"#,
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
                r#"<form method="post" action="/collection/targets/archive">{fields}<button class="c-btn-primary c-tg-btn" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
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
    fn archive_progress_failures_use_business_language() {
        assert!(
            action_feedback_markup(Some("archive_in_progress")).contains("已有一批作品正在补齐")
        );
        assert!(
            action_feedback_markup(Some("archive_nothing_to_continue"))
                .contains("当前作品目录没有待补详情")
        );
        assert!(action_feedback_markup(Some("archive_requested")).contains("建档已入队"));
        assert!(action_feedback_markup(Some("archive_merge")).contains("未重复提交"));
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
        assert!(html.contains("41 / 41 · 缺 0"));
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
        assert!(html.contains(r#"作品目录</span><span role="columnheader">详情进度"#));
        assert!(html.contains(r#"最近命中</span><span role="columnheader">最近新增"#));
        assert!(html.contains("打开作者的创作者档案"));
        assert!(html.contains("打开关键词的关键词观察"));
        assert!(!html.contains("关键词档案"));
        assert_eq!(html.matches("c-tg-actions").count(), 2);
        assert_eq!(html.matches("c-tg-btn").count(), 2);
        assert_eq!(html.matches("data-monitor-rule-trigger").count(), 1);
        assert_eq!(html.matches(">建立档案</button>").count(), 1);
        assert_eq!(html.matches(">设置巡查</a>").count(), 1);
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
                quarantined: 0,
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
        assert!(html.contains("5 / 12 · 缺 7"));
        assert!(html.contains("补采缺口"));
        assert!(html.contains("2026-09-04 08:30"));
        assert!(html.contains("2026-09-05 09:00"));
        assert!(!html.contains("2026-09-04 09:00"));
        assert!(!html.contains("ARCHIVE HEALTH"));
        assert!(!html.contains('%'));
    }
}
