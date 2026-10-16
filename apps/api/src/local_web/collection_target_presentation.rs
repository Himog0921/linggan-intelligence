//! Pure Collection target display decisions, retained while the server HTML is retired.
//! These helpers consume domain projections; they do not read, write or admit an action.
use linggan_evidence::{ArchiveCompleteness, ObservationTarget, TargetInspectorAction};
#[derive(Clone, Copy)]
pub(crate) enum TargetArchiveRead<'a> {
    Unavailable,
    Known(Option<&'a ArchiveCompleteness>),
}

impl<'a> TargetArchiveRead<'a> {
    pub(crate) fn from_map(
        completeness: Option<&'a std::collections::HashMap<String, ArchiveCompleteness>>,
        identity_key: &str,
    ) -> Self {
        match completeness {
            Some(completeness) => Self::Known(completeness.get(identity_key)),
            None => Self::Unavailable,
        }
    }

    pub(crate) fn value(self) -> Option<&'a ArchiveCompleteness> {
        match self {
            Self::Unavailable => None,
            Self::Known(value) => value,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TargetPrimaryAction {
    AssignDomain,
    EstablishArchive,
    RebuildDirectory,
    ContinueArchive,
    ViewArchiveProgress,
    ViewArchiveProblems,
    ViewArchiveUnavailable,
    OpenPatrol(&'static str),
    ViewCreator,
    ViewKeyword,
}

/// 一个关键词**建过档没有**。
///
/// 关键词不能进入 `archiving`/`archived`（`0042` 的 CHECK），所以这件事是查出来的，
/// 不是目标行上的一个状态字段。读不到时必须说「读不到」而不是当成「没建过」——后者会
/// 让页面催人去做一件可能已经做过的事。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeywordArchiveRead {
    /// 这一批目标的建档情况没读出来（查询失败或表还不在）。
    Unavailable,
    /// 还没有一轮把这个词的搜索面翻完。
    NotArchived,
    /// 链接拿到了，还有作品没取详情。**建档到这里只做了一半**：列表面给不出正文、
    /// 评论和发布时间，停在这一步的词还不是一个可用的底座。
    DetailPending,
    /// 该翻的翻完了，该补的详情也补完了。
    Complete,
}

pub(crate) fn target_primary_action(
    target: &ObservationTarget,
    is_creator: bool,
    archive: TargetArchiveRead<'_>,
    keyword_archive: KeywordArchiveRead,
) -> TargetPrimaryAction {
    // 已停止观察的目标始终只读。即使它来自历史插件写入、还没有领域，也不能借
    // “分配领域”重新打开写路径；恢复观察应走独立、显式的生命周期操作。
    if target.lifecycle_state == "dismissed" {
        return if is_creator {
            TargetPrimaryAction::ViewCreator
        } else {
            TargetPrimaryAction::ViewKeyword
        };
    }
    if target.domain_name.is_none() {
        return TargetPrimaryAction::AssignDomain;
    }
    if !is_creator {
        // 详情还欠着就先补详情，**排在「查看结果」之前**——与创作者那一路同一个次序，
        // 那边 `needs_details` 同样排在「查看档案」前面。
        //
        // 这一条不排前面，整个第二段就够不着：关键词一旦开始巡检就直接走「查看结果」，
        // 补详情的入口从此消失。而巡检每周带回来的新笔记同样只有链接——不补详情，这个词
        // 的面貌就永远停在列表面能看到的那一层（标题、封面、点赞），正文、发布时间、评论
        // 一样都没有。
        if keyword_archive == KeywordArchiveRead::DetailPending {
            return TargetPrimaryAction::ContinueArchive;
        }
        // **还没建过档的词，主操作就是建档——它是不是已经在按周巡查，不改这件事。**
        //
        // 这一条必须排在监控判断之前。巡查按规则口径取的是**每期增量**（取前 N，adhd 那
        // 条口径是 20），历史那一整段只有建档拿得回来；一个还没有底座的词，巡查得越久，
        // 手里越只剩最近几期的样本。而把建档让位给「查看结果」的后果不是少一个按钮：
        // 行上写着「尚未建立」，却没有任何入口能把它建起来——人只能先关掉观察再去点建档，
        // 那是界面逼出来的绕路。
        if keyword_archive == KeywordArchiveRead::NotArchived {
            return TargetPrimaryAction::EstablishArchive;
        }
        // Unknown is not complete.  Do not offer an action that can enter patrol until the
        // archive predicate becomes readable again.
        if keyword_archive == KeywordArchiveRead::Unavailable {
            return TargetPrimaryAction::ViewArchiveUnavailable;
        }
        if target.lifecycle_state == "paused" {
            return TargetPrimaryAction::OpenPatrol("恢复巡查");
        }
        if target.monitoring_enabled {
            return TargetPrimaryAction::ViewKeyword;
        }
        // 走到这里只剩完成态：两段都完成，才允许开始每周巡检。
        return match keyword_archive {
            KeywordArchiveRead::Complete => TargetPrimaryAction::OpenPatrol("开始每周巡检"),
            KeywordArchiveRead::Unavailable => TargetPrimaryAction::ViewArchiveUnavailable,
            // 前两态在上面按它们自己的次序返回了；保留这两臂是为了让这个 match 仍然穷尽
            // 四态——将来多一个建档态时，编译在这里就会停下来。
            KeywordArchiveRead::NotArchived => TargetPrimaryAction::EstablishArchive,
            KeywordArchiveRead::DetailPending => TargetPrimaryAction::ContinueArchive,
        };
    }
    let archive = match archive {
        TargetArchiveRead::Unavailable => return TargetPrimaryAction::ViewArchiveUnavailable,
        TargetArchiveRead::Known(archive) => archive,
    };
    if archive.is_some_and(|value| {
        value.directory_baseline == linggan_evidence::ArchiveDirectoryBaseline::NotStarted
    }) {
        // Patrol problems remain visible in coverage, but they do not replace the missing
        // homepage root or hide its explicit action.
        return TargetPrimaryAction::EstablishArchive;
    }
    if archive.is_some_and(ArchiveCompleteness::has_actionable_problems) {
        return TargetPrimaryAction::ViewArchiveProblems;
    }
    if archive.is_some_and(|value| value.work_in_progress) {
        return TargetPrimaryAction::ViewArchiveProgress;
    }
    let untouched = archive.is_none_or(ArchiveCompleteness::is_untouched);
    let needs_details =
        archive.is_some_and(|value| value.works_listed > 0 && value.pending_details > 0);
    if untouched {
        return TargetPrimaryAction::EstablishArchive;
    }
    if archive.is_some_and(ArchiveCompleteness::requires_directory_rebuild) {
        return TargetPrimaryAction::RebuildDirectory;
    }
    if needs_details {
        return TargetPrimaryAction::ContinueArchive;
    }
    if archive.is_some_and(|value| !value.has_displayable_directory()) {
        // A stored archive attempt without a current directory is neither a usable dossier nor
        // a live task.  Do not send the person to "查看档案" and make them infer the failure
        // from a contradictory status label; the next action is to inspect and handle it.
        return TargetPrimaryAction::ViewArchiveProblems;
    }
    let baseline_ready = matches!(
        target.lifecycle_state.as_str(),
        "archived" | "monitoring" | "paused"
    );
    if !baseline_ready {
        // A Work Order or even some rows do not prove that the bounded baseline completed.
        // Keep the user on the archive problem state; never offer patrol from equal counters alone.
        return TargetPrimaryAction::ViewArchiveProblems;
    }
    if !target.monitoring_enabled {
        return TargetPrimaryAction::OpenPatrol("开启巡查");
    }
    TargetPrimaryAction::ViewCreator
}

pub(crate) fn lifecycle_patrol_copy(target: &ObservationTarget) -> (&'static str, &'static str) {
    if target.monitoring_enabled && target.lifecycle_state != "monitoring" {
        return ("warn", "状态异常，未调度");
    }
    match target.lifecycle_state.as_str() {
        "paused" => ("warn", "巡查已暂停"),
        "dismissed" => ("neutral", "不再调度"),
        _ if target.monitoring_enabled => ("info", "巡查中"),
        _ => ("neutral", "未开启巡查"),
    }
}

/// 打开平台原页。只有创作者给得出链接——关键词没有一个「主页」。
pub(crate) fn inspector_action_copy(action: TargetInspectorAction) -> (&'static str, &'static str) {
    match action {
        TargetInspectorAction::NoActionHealthy => (
            "当前无需处理",
            "系统会按现有规则继续观察；需要查证时进入作品或巡查。",
        ),
        TargetInspectorAction::NoActionQueued => (
            "当前无需处理",
            "任务已经进入队列，系统会在取得真实回执后更新档案。",
        ),
        TargetInspectorAction::NoActionRunning => (
            "当前无需处理",
            "任务正在执行，系统会在取得真实回执后更新档案。",
        ),
        TargetInspectorAction::StartArchive => (
            "需要建立档案",
            "当前只有观察目标身份。建立档案会读取主页作品目录，最多 200 篇；页面提前结束则按实际结束。",
        ),
        TargetInspectorAction::RebuildDirectory => (
            "需要处理目录边界",
            "历史作品会保留；重新建立受当前标准证明的主页目录边界。",
        ),
        TargetInspectorAction::ContinueArchive => (
            "有详情缺口需要补采",
            "作品目录可用，但部分作品详情尚未取得。",
        ),
        TargetInspectorAction::HandleArchiveProblems => (
            "需要查看档案问题",
            "存在已知阻塞；页面不会把它写成自动处理中。",
        ),
        TargetInspectorAction::EnablePatrol => (
            "需要设置持续巡查",
            "当前档案已可用，但还没有生效的持续巡查规则。",
        ),
    }
}

pub(crate) fn required_action_copy(action: TargetPrimaryAction) -> (&'static str, &'static str) {
    match action {
        TargetPrimaryAction::AssignDomain => (
            "需要分配领域",
            "这个目标还没有关联研究领域。先选择领域再建立档案；作品材料仍走共享材料链。",
        ),
        TargetPrimaryAction::EstablishArchive => (
            "需要建立档案",
            "当前只有观察目标身份，还没有可核验的作品目录。",
        ),
        TargetPrimaryAction::RebuildDirectory => (
            "需要处理目录边界",
            "历史作品会保留；重新建立受当前标准证明的主页目录边界。",
        ),
        TargetPrimaryAction::ContinueArchive => (
            "有详情缺口需要补采",
            "作品目录可用，但部分作品详情尚未取得。",
        ),
        TargetPrimaryAction::ViewArchiveProgress => (
            "当前无需处理",
            "已有建档任务进入队列或执行；系统会在真实回执到达后更新档案。",
        ),
        TargetPrimaryAction::ViewArchiveProblems => (
            "需要查看档案问题",
            "存在已知阻塞或目录问题；页面不会把它写成自动处理中。",
        ),
        TargetPrimaryAction::ViewArchiveUnavailable => (
            "当前无法判断",
            "档案读取暂不可用；未知状态下不会发起新的写操作。",
        ),
        TargetPrimaryAction::OpenPatrol(_) => (
            "需要设置持续巡查",
            "当前档案已可用，但还没有生效的持续巡查规则。",
        ),
        TargetPrimaryAction::ViewCreator | TargetPrimaryAction::ViewKeyword => (
            "当前无需处理",
            "系统会按现有规则继续观察；需要查证时进入作品或巡查。",
        ),
    }
}

pub(crate) fn keyword_archive_read(
    keyword_archives: Option<&std::collections::HashSet<uuid::Uuid>>,
    keyword_details_pending: Option<&std::collections::HashSet<uuid::Uuid>>,
    target_ref: uuid::Uuid,
) -> KeywordArchiveRead {
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

#[cfg(test)]
mod keyword_archive_action_tests {
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
            domain_name: Some("ADHD".to_owned()),
        }
    }

    fn action(
        target: &ObservationTarget,
        keyword_archive: KeywordArchiveRead,
    ) -> TargetPrimaryAction {
        target_primary_action(
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
mod keyword_archive_read_tests {
    use super::KeywordArchiveRead;
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
