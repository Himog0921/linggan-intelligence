//! COLLECTION-001 · 观察目标的宽幅研究抽屉。
//!
//! 三个职责 tab（概览 / 作品 / 巡查）留在 Collection；作品正文、评论和媒体结果只在
//! Corpus。概览负责判断，作品负责查证和看表现，巡查负责时间变化。
//!
//! **用 URL 参数驱动，不用 JS**：`?drawer=<target_ref>&dtab=works&wview=list`。稿子那 142 行
//! 脚本换来的是「点击不刷新」，代价是刷新即丢状态、链接分享不过去。URL 版两样都不丢，
//! 而这一页的用途正是「打开一个目标细看，然后把它发给别人」。
//!
//! 稿子上有而系统里没有的读数，一律如实写「未采集」或「尚未接通」（Mog 于 2026-08-28
//! 选定方案 A），不填假数。

use linggan_evidence::{
    ArchiveCompleteness, BlockedMaterial, CatalogDetailState, CatalogSource,
    CreatorDirectoryProjection, CreatorLifecycleAssociation, CreatorLifecycleMetric,
    CreatorLifecyclePoint, CreatorLifecycleProjection, CreatorLifecycleStatus,
    CreatorLifecycleWindow, KeywordHitProjection, MaterialExecutionKind, ObservationTarget,
    ObservationTargetAvatar, TargetInspectorAction, TargetInspectorArchiveState,
    TargetInspectorCount, TargetInspectorExecutionState, TargetInspectorPatrolState,
    TargetInspectorProjection,
};

/// Collection 目标抽屉的三个职责。Evidence 已退回唯一的 Corpus 表面。退役或未知
/// 值统一归一到 Overview；读取守卫和渲染都只消费这个闭集，不能各自猜一次。
const TABS: &[(TargetDrawerTab, &str)] = &[
    (TargetDrawerTab::Overview, "概览"),
    (TargetDrawerTab::Baseline, "作品"),
    (TargetDrawerTab::Patrol, "巡查"),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetDrawerTab {
    Overview,
    Baseline,
    Patrol,
}

#[derive(Clone, Copy)]
pub(crate) enum TargetCatalogView<'a> {
    Creator(Option<&'a CreatorDirectoryProjection>),
    Keyword(Option<&'a KeywordHitProjection>),
    Unavailable,
}

impl TargetDrawerTab {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("archive" | "baseline" | "works") => Self::Baseline,
            Some("patrol") => Self::Patrol,
            // `baseline` remains a compatibility alias. Retired `evidence` / `trace` and any
            // future spelling open the safe default Overview and actually read it.
            None | Some("overview") | Some("evidence") | Some("trace") | Some(_) => Self::Overview,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Baseline => "works",
            Self::Patrol => "patrol",
        }
    }
}

/// Creator works have two ways to inspect the same target-scoped facts.  The view stays in the
/// URL so refresh/back/share preserve the operator's place.  Existing lifecycle links predate
/// `wview`; callers can opt into the compatibility path when any `life_*` query is present.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetWorksView {
    List,
    Performance,
}

impl TargetWorksView {
    pub fn parse(value: Option<&str>, has_legacy_lifecycle_query: bool) -> Self {
        match value {
            Some("performance") => Self::Performance,
            Some("list") => Self::List,
            None if has_legacy_lifecycle_query => Self::Performance,
            None | Some(_) => Self::List,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Performance => "performance",
        }
    }
}

/// The two performance readings are alternate views of one qualified point set, never two
/// independent reports. The state lives in the URL so a copied inspector link preserves the
/// question the operator was asking.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeChartView {
    Trend,
    Distribution,
}

impl LifeChartView {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("distribution") => Self::Distribution,
            None | Some("trend") | Some(_) => Self::Trend,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Trend => "trend",
            Self::Distribution => "distribution",
        }
    }
}

/// Trend aggregation is a local reading aid. It neither changes the lifecycle query nor creates
/// an additional persisted fact. “每 7 日” is deliberately described as a calendar-week bucket
/// rather than a new analytics metric.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifeTrendGrain {
    Month,
    Week,
}

impl LifeTrendGrain {
    pub fn parse(value: Option<&str>) -> Self {
        match value {
            Some("week") => Self::Week,
            None | Some("month") | Some(_) => Self::Month,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Month => "month",
            Self::Week => "week",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Month => "按月",
            Self::Week => "按周",
        }
    }

    fn density_unit(self) -> &'static str {
        match self {
            Self::Month => "活跃月",
            Self::Week => "活跃周",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TargetListContext<'a> {
    pub filter: Option<&'a str>,
    pub sort: Option<&'a str>,
    /// 当前观察领域。抽屉、规则、返回列表的地址都由这里重建，漏掉它就会在打开一个
    /// 目标细看之后把人送回别的领域——与本模块修过的那类缺陷同源。
    pub domain: Option<&'a str>,
}

impl<'a> TargetListContext<'a> {
    fn pairs(self) -> Vec<(&'static str, &'a str)> {
        let mut pairs = Vec::new();
        // 领域排在最前：它是这一页的观察对象，不是筛选条件之一。
        if let Some(domain) = self.domain.filter(|value| !value.is_empty()) {
            pairs.push(("domain", domain));
        }
        if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) = self.filter {
            pairs.push(("filter", filter));
        }
        if let Some(sort @ "last") = self.sort {
            pairs.push(("sort", sort));
        }
        pairs
    }

    pub fn list_href(self, focus_id: Option<&str>) -> String {
        let pairs = self.pairs();
        let mut href = "/collection/targets".to_owned();
        if !pairs.is_empty() {
            href.push('?');
            href.push_str(
                &pairs
                    .iter()
                    .map(|(key, value)| {
                        format!(
                            "{}={}",
                            percent_encode_component(key),
                            percent_encode_component(value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("&"),
            );
        }
        if let Some(focus_id) = focus_id {
            href.push('#');
            href.push_str(&percent_encode_component(focus_id));
        }
        escape(&href)
    }

    pub fn drawer_href(
        self,
        target_ref: uuid::Uuid,
        params: &[(&str, &str)],
        fragment: Option<&str>,
    ) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("drawer".to_owned(), target_ref.to_string()));
        pairs.extend(
            params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        let mut href = format!(
            "/collection/targets?{}",
            pairs
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        percent_encode_component(key),
                        percent_encode_component(value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&")
        );
        if let Some(fragment) = fragment {
            href.push('#');
            href.push_str(&percent_encode_component(fragment));
        }
        escape(&href)
    }

    /// Open the destructive-action confirmation while preserving the closed list context.
    /// The target itself is deliberately not a drawer state: deletion has its own modal and
    /// must not make a stale drawer look readable after the row is gone.
    pub fn delete_href(self, target_ref: uuid::Uuid) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("delete".to_owned(), target_ref.to_string()));
        escape(&format!(
            "/collection/targets?{}",
            pairs
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        percent_encode_component(key),
                        percent_encode_component(value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&")
        ))
    }

    pub fn domain_assignment_href(self, target_ref: uuid::Uuid) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("assign_domain".to_owned(), target_ref.to_string()));
        escape(&format!(
            "/collection/targets?{}",
            pairs
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        percent_encode_component(key),
                        percent_encode_component(value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&")
        ))
    }

    /// Open the target-scoped monitoring rule overlay without carrying drawer state into it.
    /// The list filter and sort remain in the URL so closing the overlay returns to the same
    /// working set. `opener_id` is a fragment only: it restores keyboard focus and never
    /// participates in target identity.
    pub fn monitor_rule_href(self, target_ref: uuid::Uuid, opener_id: &str) -> String {
        self.monitor_rule_slot_href(target_ref, None, opener_id)
    }

    /// 规则面板的链接，可以指定打开哪一条口径。
    ///
    /// `None` = 最早那条（博主永远只有一条，「管理巡查」的既有行为）；`Some("new")` = 新开
    /// 一条；`Some(口径)` = 规则台上那一条。此前只有按目标的那一种，于是「加一条规则」
    /// 与「编辑第二条」在 URL 上无法区分。
    pub fn monitor_rule_slot_href(
        self,
        target_ref: uuid::Uuid,
        slot: Option<&str>,
        opener_id: &str,
    ) -> String {
        let mut pairs = self
            .pairs()
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect::<Vec<_>>();
        pairs.push(("rule".to_owned(), target_ref.to_string()));
        if let Some(slot) = slot {
            pairs.push(("rule_slot".to_owned(), slot.to_owned()));
        }
        escape(&format!(
            "/collection/targets?{}#{}",
            pairs
                .iter()
                .map(|(key, value)| {
                    format!(
                        "{}={}",
                        percent_encode_component(key),
                        percent_encode_component(value)
                    )
                })
                .collect::<Vec<_>>()
                .join("&"),
            percent_encode_component(opener_id),
        ))
    }

    /// A POST may return only to this closed list context. These are individual, validated
    /// fields rather than an arbitrary return URL, so the action cannot become an open redirect.
    pub fn return_fields(
        self,
        drawer: Option<uuid::Uuid>,
        tab: Option<TargetDrawerTab>,
        focus_id: Option<&str>,
    ) -> String {
        let mut fields = self
            .pairs()
            .into_iter()
            .map(|(key, value)| {
                format!(
                    r#"<input type="hidden" name="return_{key}" value="{}"/>"#,
                    escape(value)
                )
            })
            .collect::<String>();
        if let Some(drawer) = drawer {
            fields.push_str(&format!(
                r#"<input type="hidden" name="return_drawer" value="{drawer}"/>"#
            ));
        }
        if let Some(tab) = tab {
            fields.push_str(&format!(
                r#"<input type="hidden" name="return_dtab" value="{}"/>"#,
                tab.as_str()
            ));
        }
        if let Some(focus_id) = focus_id {
            fields.push_str(&format!(
                r#"<input type="hidden" name="return_focus" value="{}"/>"#,
                escape(focus_id)
            ));
        }
        fields
    }

    /// Acquisition input must be a concrete Domain. The collection-wide sentinel is navigation
    /// context only and deliberately does not become a hidden Domain choice.
    pub fn acquisition_domain_field(self) -> String {
        let Some(domain_ref) = self
            .domain
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
        else {
            return String::new();
        };
        format!(r#"<input type="hidden" name="domain_ref" value="{domain_ref}"/>"#)
    }
}

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

    fn value(self) -> Option<&'a ArchiveCompleteness> {
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

#[derive(Clone, Copy)]
pub enum LifecycleView<'a> {
    Projection(&'a CreatorLifecycleProjection),
    ReadUnavailable {
        window: CreatorLifecycleWindow,
        metric: CreatorLifecycleMetric,
    },
    NotRead {
        window: CreatorLifecycleWindow,
        metric: CreatorLifecycleMetric,
    },
    QueryInvalid,
}

#[derive(Clone, Copy)]
pub enum TargetInspectorView<'a> {
    Projection(&'a TargetInspectorProjection),
    ReadUnavailable,
    NotRead,
}

/// Collection's base renderer owns the closing document and script tag. Mount the drawer before
/// that script so the behavior module can bind Escape on first parse; appending after `</html>`
/// produces invalid markup and makes the drawer invisible to the script at execution time.
pub fn attach_to_collection_document(document: &str, drawer: &str) -> String {
    if drawer.is_empty() {
        return document.to_owned();
    }
    const SCRIPT: &str = r#"<script src="/assets/collection-workspace.js"></script>"#;
    let Some(script_at) = document.rfind(SCRIPT) else {
        return document.to_owned();
    };
    format!(
        "{}{}{}",
        &document[..script_at],
        drawer,
        &document[script_at..]
    )
}

/// 渲染抽屉。`drawer` 为空时整块不渲染——没有选中目标时不该有一个空壳挂在那里。
pub fn render(
    target: Option<&ObservationTarget>,
    avatar: Option<&ObservationTargetAvatar>,
    completeness: Option<&std::collections::HashMap<String, ArchiveCompleteness>>,
    drawer: Option<&str>,
    active_tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    render_with_catalog(
        target,
        avatar,
        completeness,
        drawer,
        active_tab,
        lifecycle,
        TargetCatalogView::Unavailable,
        None,
        None,
        selected_work,
        list_context,
    )
}

pub fn render_with_catalog(
    target: Option<&ObservationTarget>,
    avatar: Option<&ObservationTargetAvatar>,
    completeness: Option<&std::collections::HashMap<String, ArchiveCompleteness>>,
    drawer: Option<&str>,
    active_tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    catalog: TargetCatalogView<'_>,
    catalog_query: Option<&str>,
    catalog_filter: Option<&str>,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    render_with_catalog_view(
        target,
        avatar,
        completeness,
        drawer,
        active_tab,
        lifecycle,
        TargetInspectorView::NotRead,
        TargetWorksView::List,
        catalog,
        catalog_query,
        catalog_filter,
        selected_work,
        // 这条旧入口不读巡检规则。`None` 是「没问」——界面据此说读不到，而不是谎称
        // 一条规则都没有，后者会让人再配一条。
        None,
        &[],
        list_context,
    )
}

/// 最内层入口：巡检规则清单从这里进来。外两层是旧入口，它们不读规则，传 `None`——
/// 那是「没问」，界面据此说读不到，而不是谎称一条规则都没有。
pub fn render_with_catalog_view(
    target: Option<&ObservationTarget>,
    avatar: Option<&ObservationTargetAvatar>,
    completeness: Option<&std::collections::HashMap<String, ArchiveCompleteness>>,
    drawer: Option<&str>,
    active_tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    inspector: TargetInspectorView<'_>,
    works_view: TargetWorksView,
    catalog: TargetCatalogView<'_>,
    catalog_query: Option<&str>,
    catalog_filter: Option<&str>,
    selected_work: Option<&str>,
    monitor_rules: Option<&[linggan_evidence::MonitorRuleSummary]>,
    // 当前还需要人来判断「是不是已经没了」的作品。空表示没有可判断的对象——
    // 那时不渲染任何确认入口，请人对空气做决定不是一个动作。
    retirable: &[BlockedMaterial],
    list_context: TargetListContext<'_>,
) -> String {
    render_with_catalog_view_with_chart(
        target,
        avatar,
        completeness,
        drawer,
        active_tab,
        lifecycle,
        inspector,
        works_view,
        LifeChartView::Trend,
        LifeTrendGrain::Month,
        catalog,
        catalog_query,
        catalog_filter,
        selected_work,
        monitor_rules,
        retirable,
        // 这条旧入口不读关键词的建档态。`Unavailable` 是「没问」——界面据此维持原本的
        // 入口，而不是把没问过说成「没建过」。接上四态的是下面那条带 chart 的入口。
        KeywordArchiveRead::Unavailable,
        list_context,
    )
}

/// Variant used by the collection route after it has decoded the bounded presentation state.
/// Keeping the older entry point above preserves callers that intentionally only need the
/// default trend reading.
pub fn render_with_catalog_view_with_chart(
    target: Option<&ObservationTarget>,
    avatar: Option<&ObservationTargetAvatar>,
    completeness: Option<&std::collections::HashMap<String, ArchiveCompleteness>>,
    drawer: Option<&str>,
    active_tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    inspector: TargetInspectorView<'_>,
    works_view: TargetWorksView,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    catalog: TargetCatalogView<'_>,
    catalog_query: Option<&str>,
    catalog_filter: Option<&str>,
    selected_work: Option<&str>,
    monitor_rules: Option<&[linggan_evidence::MonitorRuleSummary]>,
    retirable: &[BlockedMaterial],
    // 这个关键词建过档没有。列表行的主操作按它分派，抽屉按同一个判断给同一个答案——
    // 一个词在两个表面上各说各的，人就不知道该信哪一句。
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    let Some(drawer) = drawer else {
        return String::new();
    };
    // 找不到就如实说找不到，而不是静默关掉抽屉——链接失效与「我没点开」是两回事。
    //
    // 目标由调用方**独立查询**得到，不从当前列表里找：列表是筛过的，一个被筛掉的目标
    // 会让这里说「未找到」，而它其实好好地在库里——那是在撒谎。
    let Some(target) = target else {
        let return_focus = uuid::Uuid::parse_str(drawer)
            .ok()
            .map(|target_ref| format!("target-{target_ref}"));
        let return_href = list_context.list_href(return_focus.as_deref());
        let return_focus_attr = return_focus
            .as_deref()
            .map(|focus| format!(r#" data-return-focus="{focus}""#))
            .unwrap_or_default();
        return format!(
            r#"<aside id="c-drawer" class="c-dw" aria-labelledby="c-drawer-title" data-return-url="{return_url}"{return_focus_attr}>
                 <div class="c-dw-head">
                   <div class="c-dw-kicker">观察档案</div>
                   <div class="c-dw-title-row">
                     <div><h2 id="c-drawer-title" class="c-dw-title" tabindex="-1" data-drawer-initial-focus>未找到该观察目标</h2>
                       <div class="c-dw-meta">#{drawer}</div></div>
                     <div class="c-dw-actions"><a class="c-btn-secondary c-dw-close" href="{return_href}">{close_icon}<span>关闭</span></a></div>
                   </div>
                 </div>
                 <div class="c-dw-body"><p class="c-dw-empty">这个标识没有对应的观察目标。它可能已被删除，或链接来自另一台机器的库。</p></div>
               </aside>"#,
            drawer = escape(drawer),
            close_icon = close_icon(),
            return_url = list_context.list_href(None),
        );
    };

    let archive = TargetArchiveRead::from_map(completeness, &target.identity_key);
    let is_creator = target.target_kind == "creator";
    let tab = active_tab;
    let return_focus = format!("target-{}", target.target_ref);
    let return_href = list_context.list_href(Some(&return_focus));

    format!(
        r#"<aside id="c-drawer" class="c-dw" aria-labelledby="c-drawer-title" data-return-url="{return_url}" data-return-focus="{return_focus}">
             <div class="c-dw-head">
               <div class="c-dw-kicker">{workspace}</div>
               <div class="c-dw-title-row">
                 <div class="c-dw-identity">
                   {avatar}
                   <div class="c-dw-identity-copy">
                     <div class="c-dw-name-row"><h2 id="c-drawer-title" class="c-dw-title" tabindex="-1" data-drawer-initial-focus>{name}</h2>{watch_badge}</div>
                     <div class="c-dw-meta"><span>{platform}</span>{handle}<i class="c-dw-meta-dot" aria-hidden="true"></i><span>{group}</span></div>
                     {bio}
                   </div>
                 </div>
                 <div class="c-dw-actions">
                   {source_link}
                   <a class="c-btn-secondary c-dw-close" href="{return_href}">{close_icon}<span>关闭</span></a>
                 </div>
               </div>
             </div>
             <nav class="c-dw-tabs">{tabs}</nav>
             <div class="c-dw-body">{body}</div>
           </aside>"#,
        workspace = escape(if is_creator {
            "创作者档案"
        } else {
            "关键词观察"
        }),
        name = escape(display_name(target)),
        avatar = drawer_avatar_markup(avatar, display_name(target)),
        platform = escape(platform_label(&target.platform)),
        handle = identity_handle(target)
            .map(|handle| format!(" · {}", escape(handle)))
            .unwrap_or_default(),
        return_url = list_context.list_href(None),
        source_link = source_link(target, is_creator),
        close_icon = close_icon(),
        watch_badge = drawer_watch_badge(target),
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
        // Description facts may contain email addresses, tags and other unstructured profile
        // text. They are evidence, not drawer chrome: keeping them out of the header makes the
        // target identity and the three operational facts scannable at a glance.
        bio = "",
        tabs = tab_bar(
            target,
            tab,
            lifecycle,
            works_view,
            chart_view,
            trend_grain,
            selected_work,
            list_context
        ),
        body = body(
            retirable,
            target,
            archive,
            is_creator,
            tab,
            lifecycle,
            inspector,
            works_view,
            chart_view,
            trend_grain,
            catalog,
            catalog_query,
            catalog_filter,
            selected_work,
            monitor_rules,
            keyword_archive,
            list_context,
        ),
    )
}

fn display_name(target: &ObservationTarget) -> &str {
    target
        .display_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&target.identity_key)
}

fn identity_handle(target: &ObservationTarget) -> Option<&str> {
    target
        .identity_facts
        .as_ref()
        .and_then(|facts| facts.get("redId"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn drawer_avatar_markup(avatar: Option<&ObservationTargetAvatar>, name: &str) -> String {
    match avatar.unwrap_or(&ObservationTargetAvatar::NotObserved) {
        ObservationTargetAvatar::Local { local_asset_path } => format!(
            r#"<img class="c-dw-avatar" src="{}" alt="" width="48" height="48" referrerpolicy="no-referrer"/>"#,
            escape(local_asset_path),
        ),
        ObservationTargetAvatar::Pending
        | ObservationTargetAvatar::Unavailable
        | ObservationTargetAvatar::NotObserved => format!(
            r#"<span class="c-dw-avatar c-dw-avatar-placeholder" aria-hidden="true">{}</span>"#,
            escape(&name_initial(name))
        ),
    }
}

fn name_initial(name: &str) -> String {
    name.chars()
        .find(|character| !character.is_whitespace())
        .unwrap_or('创')
        .to_string()
}

fn platform_label(platform: &str) -> &str {
    match platform {
        "xhs" => "小红书",
        other => other,
    }
}

/// One kind-aware lifecycle vocabulary is shared by the target ledger and drawer. Keyword
/// targets never run creator baseline archiving, so a monitoring keyword must not inherit the
/// creator-only "baseline ready" claim merely because the state spelling is shared.
pub(crate) fn lifecycle_primary_copy(
    target_kind: &str,
    lifecycle_state: &str,
) -> (&'static str, &'static str) {
    match (target_kind, lifecycle_state) {
        ("creator", "pending_decision") => ("neutral", "尚未建档"),
        ("creator", "archiving") => ("warn", "建档待处理"),
        ("creator", "archived" | "monitoring" | "paused") => ("ok", "档案已建立"),
        ("creator", "dismissed") => ("neutral", "已停止观察"),
        ("keyword", "pending_decision") => ("neutral", "等待决定"),
        ("keyword", "monitoring") => ("ok", "规则已生效"),
        ("keyword", "paused") => ("warn", "规则已暂停"),
        ("keyword", "dismissed") => ("neutral", "已停止观察"),
        ("keyword", "archiving" | "archived") => ("warn", "状态与目标类型冲突"),
        _ => ("neutral", "状态未知"),
    }
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
fn source_link(target: &ObservationTarget, is_creator: bool) -> String {
    if !is_creator {
        return String::new();
    }
    let href = format!(
        "https://www.xiaohongshu.com/user/profile/{}",
        percent_encode_component(&target.identity_key)
    );
    format!(
        r#"<a class="c-btn-secondary c-dw-source" href="{}" target="_blank" rel="noreferrer"><span>打开小红书主页</span>{}</a>"#,
        escape(&href),
        external_link_icon(),
    )
}

fn close_icon() -> String {
    r#"<svg class="c-dw-action-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18"/></svg>"#.to_owned()
}

fn external_link_icon() -> String {
    r#"<svg class="c-dw-action-icon" viewBox="0 0 24 24" aria-hidden="true"><path d="M14 5h5v5M19 5l-9 9M19 14v5H5V5h5"/></svg>"#.to_owned()
}

/// Stable copy contract used by the archived/read-model tests. The compact reference header
/// intentionally exposes only the continuing-observation badge; full archive and patrol state
/// remains in the Overview body, where it can be read without being mistaken for identity.
#[cfg_attr(not(test), allow(dead_code))]
fn statusline(target: &ObservationTarget, completeness: TargetArchiveRead<'_>) -> String {
    let archive = if target.target_kind != "creator" {
        lifecycle_primary_copy(&target.target_kind, &target.lifecycle_state).1
    } else {
        match completeness {
            TargetArchiveRead::Unavailable => "档案状态暂时无法读取",
            TargetArchiveRead::Known(Some(value)) if value.has_actionable_problems() => {
                "档案有问题"
            }
            TargetArchiveRead::Known(Some(value)) if value.work_in_progress => "建档待处理",
            TargetArchiveRead::Known(Some(value)) if value.requires_directory_rebuild() => {
                "待建标准目录"
            }
            TargetArchiveRead::Known(None) => "尚未建档",
            TargetArchiveRead::Known(Some(value)) if value.is_untouched() => "尚未建档",
            TargetArchiveRead::Known(Some(value))
                if value.works_listed > 0 && value.pending_details > 0 =>
            {
                "档案待完善"
            }
            TargetArchiveRead::Known(Some(_))
                if matches!(
                    target.lifecycle_state.as_str(),
                    "archived" | "monitoring" | "paused"
                ) =>
            {
                "档案已建立"
            }
            TargetArchiveRead::Known(Some(_)) => "档案有问题",
        }
    };
    let (_, patrol) = lifecycle_patrol_copy(target);
    format!(
        "{archive} · {patrol} · {group}",
        group = escape(target.group_name.as_deref().unwrap_or("未分组")),
    )
}

/// This badge describes the target's configured continuing-observation state, not runtime work.
/// It deliberately does not borrow a scheduler, Work Order, Lease or Attempt state from the
/// read model: an enabled rule is not evidence that something is executing right now, and a
/// disabled rule alone cannot tell a paused target from one that was stopped or never enabled.
fn drawer_watch_badge(target: &ObservationTarget) -> String {
    let (state, label) = match target.lifecycle_state.as_str() {
        "dismissed" => ("stopped", "已停止观察"),
        "paused" => ("paused", "已暂停"),
        _ if target.monitoring_enabled => ("active", "观察中"),
        _ => ("inactive", "未开启观察"),
    };
    format!(
        r#"<span class="c-dw-watch-badge" data-state="{state}"><i aria-hidden="true"></i>{label}</span>"#,
    )
}

fn drawer_primary_action(
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    let action = target_primary_action(target, is_creator, archive, keyword_archive);
    match action {
        TargetPrimaryAction::AssignDomain => {
            let href = list_context.domain_assignment_href(target.target_ref);
            format!(r#"<a class="c-btn-primary" href="{href}">分配领域</a>"#)
        }
        TargetPrimaryAction::EstablishArchive
        | TargetPrimaryAction::RebuildDirectory
        | TargetPrimaryAction::ContinueArchive => {
            let label = archive_action_label(action);
            let gap_field = if action == TargetPrimaryAction::ContinueArchive {
                r#"<input type="hidden" name="archive_action" value="gaps">"#
            } else {
                ""
            };
            let fields = list_context.return_fields(
                Some(target.target_ref),
                Some(TargetDrawerTab::Baseline),
                Some("target-archive"),
            );
            let domain_field = list_context.acquisition_domain_field();
            format!(
                r#"<form class="c-dw-primary-form" method="post" action="/collection/targets/archive">{fields}{domain_field}{gap_field}<button class="c-btn-primary" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
                target_ref = target.target_ref,
            )
        }
        TargetPrimaryAction::OpenPatrol(label) => {
            let opener_id = format!("drawer-header-monitor-rule-{}", target.target_ref);
            let href = list_context.monitor_rule_href(target.target_ref, &opener_id);
            format!(
                r#"<a id="{opener_id}" class="c-btn-primary" data-monitor-rule-trigger="{target_ref}" href="{href}">{label}</a>"#,
                target_ref = target.target_ref,
            )
        }
        TargetPrimaryAction::ViewKeyword => {
            let href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-btn-secondary" href="{href}">查看结果</a>"#)
        }
        TargetPrimaryAction::ViewCreator => {
            let href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "works")],
                Some("target-works"),
            );
            format!(r#"<a class="c-btn-secondary" href="{href}">查看档案</a>"#)
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
            let href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "overview")],
                Some(fragment),
            );
            format!(r#"<a class="c-btn-secondary" href="{href}">{label}</a>"#)
        }
    }
}

fn tab_bar(
    target: &ObservationTarget,
    active: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    works_view: TargetWorksView,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    let lifecycle_state = lifecycle_query_state(lifecycle, chart_view, trend_grain, selected_work);
    TABS.iter()
        .map(|(key, label)| {
            let class = if *key == active { " c-dw-tab-on" } else { "" };
            let current = if *key == active {
                r#" aria-current="page""#
            } else {
                ""
            };
            let lifecycle_state = lifecycle_state
                .iter()
                .map(|(key, value)| (*key, value.as_str()))
                .collect::<Vec<_>>();
            let mut params = vec![("dtab", key.as_str())];
            if *key == TargetDrawerTab::Baseline && target.target_kind == "creator" {
                params.push(("wview", works_view.as_str()));
            }
            params.extend(lifecycle_state);
            let href = list_context.drawer_href(target.target_ref, &params, None);
            format!(r#"<a class="c-dw-tab{class}" href="{href}"{current}>{label}</a>"#,)
        })
        .collect()
}

fn body(
    retirable: &[BlockedMaterial],
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    tab: TargetDrawerTab,
    lifecycle: LifecycleView<'_>,
    inspector: TargetInspectorView<'_>,
    works_view: TargetWorksView,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    catalog: TargetCatalogView<'_>,
    catalog_query: Option<&str>,
    catalog_filter: Option<&str>,
    selected_work: Option<&str>,
    monitor_rules: Option<&[linggan_evidence::MonitorRuleSummary]>,
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    match tab {
        TargetDrawerTab::Baseline => works_tab(
            target,
            catalog,
            catalog_query,
            catalog_filter,
            lifecycle,
            works_view,
            chart_view,
            trend_grain,
            selected_work,
            list_context,
        ),
        TargetDrawerTab::Patrol => patrol_tab(target, inspector, monitor_rules, list_context),
        TargetDrawerTab::Overview => overview_tab(
            retirable,
            target,
            archive,
            is_creator,
            inspector,
            lifecycle,
            selected_work,
            keyword_archive,
            list_context,
        ),
    }
}

/// The work tab is the checkable result of the target's observation contract.
/// It contains only target-scoped, accepted discovery rows; opening a work
/// hands off to Corpus, which remains the owner of its full source material.
fn works_tab(
    target: &ObservationTarget,
    catalog: TargetCatalogView<'_>,
    query: Option<&str>,
    filter: Option<&str>,
    lifecycle: LifecycleView<'_>,
    works_view: TargetWorksView,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    if target.target_kind != "creator" {
        return works_list(target, catalog, query, filter, list_context);
    }
    let list_href = list_context.drawer_href(
        target.target_ref,
        &[("dtab", "works"), ("wview", "list")],
        Some("target-works"),
    );
    let mut performance_params = vec![("dtab", "works"), ("wview", "performance")];
    let lifecycle_params = lifecycle_query_state(lifecycle, chart_view, trend_grain, selected_work);
    let lifecycle_params = lifecycle_params
        .iter()
        .map(|(key, value)| (*key, value.as_str()))
        .collect::<Vec<_>>();
    performance_params.extend(lifecycle_params);
    let performance_href =
        list_context.drawer_href(target.target_ref, &performance_params, Some("target-works"));
    let tabs = format!(
        r#"<nav class="c-dw-view-tabs" aria-label="作品视图">
              <a class="c-dw-view-tab" href="{list_href}"{list_current}>列表</a>
              <a class="c-dw-view-tab" href="{performance_href}"{performance_current}>表现</a>
            </nav>"#,
        list_current = if works_view == TargetWorksView::List {
            r#" aria-current="page""#
        } else {
            ""
        },
        performance_current = if works_view == TargetWorksView::Performance {
            r#" aria-current="page""#
        } else {
            ""
        },
    );
    let content = match works_view {
        TargetWorksView::List => works_list(target, catalog, query, filter, list_context),
        TargetWorksView::Performance => lifecycle_overview(
            target,
            true,
            lifecycle,
            chart_view,
            trend_grain,
            selected_work,
            list_context,
        ),
    };
    format!(r#"{tabs}<div class="c-dw-view-body">{content}</div>"#)
}

fn works_list(
    target: &ObservationTarget,
    catalog: TargetCatalogView<'_>,
    query: Option<&str>,
    filter: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    let (works, read_error): (&[linggan_evidence::CatalogWork], bool) = match catalog {
        TargetCatalogView::Creator(Some(value)) => (&value.works, false),
        TargetCatalogView::Keyword(Some(value)) => (&value.works, false),
        TargetCatalogView::Creator(None)
        | TargetCatalogView::Keyword(None)
        | TargetCatalogView::Unavailable => (&[][..], true),
    };
    if read_error {
        return catalog_unavailable_state(target);
    }
    let normalized_query = query.map(str::trim).filter(|value| !value.is_empty());
    let selected_filter = match filter {
        Some("pending" | "complete" | "patrol_new") => filter.unwrap_or("all"),
        _ => "all",
    };
    let filtered = works
        .iter()
        .filter(|work| match selected_filter {
            "pending" => work.detail_state == CatalogDetailState::Pending,
            "complete" => work.detail_state == CatalogDetailState::Complete,
            "patrol_new" => work.source == CatalogSource::PatrolDiscovery,
            _ => true,
        })
        .filter(|work| {
            normalized_query.is_none_or(|needle| {
                let needle = needle.to_lowercase();
                work.content_external_id.to_lowercase().contains(&needle)
                    || work
                        .title
                        .as_deref()
                        .is_some_and(|title| title.to_lowercase().contains(&needle))
            })
        })
        .collect::<Vec<_>>();
    let completed = works
        .iter()
        .filter(|work| work.detail_state == CatalogDetailState::Complete)
        .count();
    let pending = works.len().saturating_sub(completed);
    let patrol_new = works
        .iter()
        .filter(|work| work.source == CatalogSource::PatrolDiscovery)
        .count();
    let is_creator = target.target_kind == "creator";
    let heading = if is_creator {
        "作品目录"
    } else {
        "命中作品"
    };
    let count_label = if is_creator { "作品" } else { "命中" };
    let mut hidden = format!(
        r#"<input type="hidden" name="drawer" value="{}"/>"#,
        target.target_ref
    );
    hidden.push_str(r#"<input type="hidden" name="wview" value="list"/>"#);
    for (key, value) in list_context.pairs() {
        hidden.push_str(&format!(
            r#"<input type="hidden" name="{key}" value="{}"/>"#,
            escape(value)
        ));
    }
    let rows = filtered
        .iter()
        .map(|work| {
            let title = work.title.as_deref().unwrap_or(match work.detail_state {
                // 详情已经取到、却还是没有标题：缺的是**字段覆盖度**（`0015` 的
                // `title_state='UNKNOWN'`），不是还没去取。说「待取得」会让人以为还有一次采集
                // 会把这个标题补回来，而这份材料其实已经在库里了。
                CatalogDetailState::Complete => "标题未收录",
                CatalogDetailState::Pending => "标题待取得",
            });
            let published = work.published_at.as_deref().unwrap_or("发布时间待取得");
            let source = match work.source {
                CatalogSource::InitialArchive => "初始建档",
                CatalogSource::PatrolDiscovery => "巡查新增",
            };
            // 欠详情的作品要说清它欠的是什么，而不是一律「待采集」：等执行权、退避冷却、输入
            // 缺失、预算用尽，在用户那里是四件不同的事（计划 §9.4）。判据只从领域读来的
            // `execution_state` 拿，页面不自己拼第二套资格，也不给「从未开始」编一句「试过没成」。
            let execution = work.execution_state.as_ref().map(|state| state.kind);
            let (detail, detail_state) = match (work.detail_state, execution) {
                (CatalogDetailState::Complete, _) => ("已完成", "complete"),
                (CatalogDetailState::Pending, Some(MaterialExecutionKind::RetryPending)) => {
                    ("延迟重试", "retry_pending")
                }
                (CatalogDetailState::Pending, Some(MaterialExecutionKind::InputBlocked)) => {
                    ("输入不可执行", "input_blocked")
                }
                (CatalogDetailState::Pending, Some(MaterialExecutionKind::BudgetExhausted)) => {
                    ("自动重试已停止", "budget_exhausted")
                }
                (CatalogDetailState::Pending, _) => ("待采集", "pending"),
            };
            let comments = work
                .comment_count
                .map(|value| value.to_string())
                .unwrap_or_else(|| "—".to_owned());
            let last = work.last_captured_at.as_deref().unwrap_or("—");
            let creator = work.creator_display_name.as_deref().unwrap_or("创作者待取得");
            let position = work.match_position.map(|value| value.to_string()).unwrap_or_else(|| "未提供".to_owned());
            let common = format!(
                r#"<td><div class="c-dw-catalog-work"><b>{title}</b><span>{id}</span></div></td><td>{published}</td>"#,
                title = escape(title),
                id = escape(&work.content_external_id),
                published = escape(published),
            );
            let corpus_href = match list_context
                .domain
                .and_then(|value| uuid::Uuid::parse_str(value).ok())
            {
                Some(domain_ref) => format!(
                    "/corpus/evidence?domain={domain_ref}&work={}",
                    work.public_ref
                ),
                None => format!("/corpus/evidence?work={}", work.public_ref),
            };
            let tail = format!(
                r#"<td><span class="c-dw-catalog-state" data-state="{detail_state}">{detail}</span></td><td>{media}</td><td>{comments}</td><td>{last}</td><td><a class="c-btn-secondary c-dw-work-open" href="{corpus_href}">查看</a></td>"#,
                detail = detail,
                media = work.media_state,
                comments = comments,
                last = escape(last),
                corpus_href = escape(&corpus_href),
            );
            if is_creator {
                format!(r#"<tr>{common}<td>{source}</td>{tail}</tr>"#)
            } else {
                format!(r#"<tr>{common}<td>{creator}</td><td>{position}</td>{tail}</tr>"#,
                    creator = escape(creator), position = escape(&position))
            }
        })
        .collect::<String>();
    let empty = if filtered.is_empty() {
        r#"<p class="c-dw-note">当前筛选没有匹配的作品。</p>"#
    } else {
        ""
    };
    format!(
        r#"<section class="c-dw-section c-dw-works" id="target-works">
             <div class="c-dw-section-head"><b>{heading}</b><span>{total} 条可查证{count_label}</span></div>
             <div class="c-dw-catalog-summary">
               <div><b>{total}</b><span>{count_label}目录</span></div>
               <div><b>{completed}</b><span>详情已取得</span></div>
               <div><b>{pending}</b><span>待取得详情</span></div>
               <div><b>{patrol_new}</b><span>巡查新增</span></div>
             </div>
             <form class="c-dw-catalog-filter" method="get" action="/collection/targets">{hidden}
               <input type="hidden" name="dtab" value="works"/>
               <input type="search" name="catalog_query" value="{query}" placeholder="搜索作品标题或作品 ID" aria-label="搜索作品标题或作品 ID"/>
               <span class="c-dw-field" data-drawn-select><select name="catalog_filter" aria-label="筛选作品">
                 <option value="all"{all}>全部</option><option value="pending"{pending_selected}>待采详情</option><option value="complete"{complete_selected}>详情已完成</option><option value="patrol_new"{patrol_selected}>巡查新增</option>
               </select></span><button class="c-tg-act" type="submit">筛选</button>
             </form>
             <div class="c-dw-catalog-table-wrap"><table class="c-dw-catalog-table"><thead><tr>{headers}</tr></thead><tbody>{rows}</tbody></table></div>{empty}
           </section>"#,
        heading = heading,
        headers = if is_creator {
            "<th>作品</th><th>发布时间</th><th>发现来源</th><th>详情</th><th>媒体处理</th><th>评论</th><th>最近采集</th><th>操作</th>"
        } else {
            "<th>作品</th><th>发布时间</th><th>创作者</th><th>命中位置</th><th>详情</th><th>媒体处理</th><th>评论</th><th>最近采集</th><th>操作</th>"
        },
        total = works.len(),
        count_label = count_label,
        completed = completed,
        pending = pending,
        patrol_new = patrol_new,
        hidden = hidden,
        query = escape(normalized_query.unwrap_or("")),
        all = if selected_filter == "all" {
            " selected"
        } else {
            ""
        },
        pending_selected = if selected_filter == "pending" {
            " selected"
        } else {
            ""
        },
        complete_selected = if selected_filter == "complete" {
            " selected"
        } else {
            ""
        },
        patrol_selected = if selected_filter == "patrol_new" {
            " selected"
        } else {
            ""
        },
        rows = rows,
        empty = empty,
    )
}

/// A catalogue failure is not a lifecycle fact. In particular, it must not reuse the retired
/// lifecycle empty shell: that made a failed work read look like an empty "作品生命周期" view.
fn catalog_unavailable_state(target: &ObservationTarget) -> String {
    let heading = if target.target_kind == "creator" {
        "作品目录"
    } else {
        "命中作品"
    };
    format!(
        r#"<section class="c-dw-section c-dw-works"><div class="c-dw-section-head"><b>{heading}</b><span>读取暂不可用</span></div><div class="c-dw-note"><b>当前读不到可核验的作品记录</b><p>这里不会用主表中的总数拼出一张作品表。恢复读取后，会显示每条真实记录。</p></div></section>"#,
        heading = heading,
    )
}

/// 概览只回答四件事：对象是谁、系统正在做什么、已经取得什么、是否需要人介入。
/// 作品逐条查证和表现分布都留在「作品」，避免抽屉再长成一个首页。
fn overview_tab(
    retirable: &[BlockedMaterial],
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    inspector: TargetInspectorView<'_>,
    _lifecycle: LifecycleView<'_>,
    _selected_work: Option<&str>,
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    match inspector {
        TargetInspectorView::Projection(projection)
            if projection.target_ref == target.target_ref =>
        {
            return inspector_overview(
                target,
                is_creator,
                projection,
                archive,
                keyword_archive,
                retirable,
                list_context,
            );
        }
        TargetInspectorView::ReadUnavailable | TargetInspectorView::Projection(_) => {
            if !is_creator {
                return keyword_inspector_unavailable_overview(
                    target,
                    archive,
                    keyword_archive,
                    list_context,
                );
            }
            return inspector_unavailable_overview(is_creator);
        }
        TargetInspectorView::NotRead => {}
    }
    if !is_creator {
        let action = target_primary_action(target, false, archive, keyword_archive);
        let (action_title, action_note) = required_action_copy(action);
        let action_control =
            required_action_control(target, archive, false, keyword_archive, list_context);
        return format!(
            r#"<section class="c-dw-section c-dw-now">
                  <div class="c-dw-section-head"><b>系统现在在做什么</b><span>关键词观察</span></div>
                  <dl class="c-dw-state-list"><div><dt>巡查</dt><dd>{patrol}</dd></div><div><dt>最近结果</dt><dd>{last}</dd></div></dl>
                </section>
                <section class="c-dw-section c-dw-decision" id="required-action">
                  <div class="c-dw-decision-copy"><span>是否需要处理</span><strong>{action_title}</strong><p>{action_note}</p></div>{action_control}
                </section>"#,
            patrol = escape(lifecycle_patrol_copy(target).1),
            last = escape(
                target
                    .last_patrol_succeeded_at
                    .as_deref()
                    .unwrap_or("尚未取得成功结果")
            ),
        );
    }
    let directory = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(Some(value)) if value.has_displayable_directory() => {
            value.works_listed.to_string()
        }
        TargetArchiveRead::Known(_) => "尚未建立".to_owned(),
    };
    let detail = match archive {
        TargetArchiveRead::Known(Some(value)) if value.has_displayable_directory() => {
            format!("{}/{}", value.details_captured, value.works_listed)
        }
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(_) => "尚未取得".to_owned(),
    };
    let action = target_primary_action(target, true, archive, keyword_archive);
    let (archive_state, execution_state) = current_system_copy(archive);
    let (_, patrol_state) = lifecycle_patrol_copy(target);
    let abnormal = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(Some(value)) => {
            let count = value.quarantined.saturating_add(value.blocked_details);
            if count == 0 {
                "无已知阻塞".to_owned()
            } else {
                format!("{count} 条待处理")
            }
        }
        TargetArchiveRead::Known(None) => "无已知阻塞".to_owned(),
    };
    let last = target
        .last_patrol_succeeded_at
        .as_deref()
        .unwrap_or("尚未巡查");
    let next = if target.monitoring_enabled {
        target.next_patrol_at.as_deref().unwrap_or("待排定")
    } else {
        "未开启"
    };
    let (action_title, action_note) = required_action_copy(action);
    let action_control =
        required_action_control(target, archive, true, keyword_archive, list_context);
    format!(
        r#"<section class="c-dw-section c-dw-now" id="archive-task">
              <div class="c-dw-section-head"><b>系统现在在做什么</b><span>状态分别判断</span></div>
              <dl class="c-dw-state-list">
                <div><dt>建档</dt><dd>{archive_state}</dd></div>
                <div><dt>执行</dt><dd>{execution_state}</dd></div>
                <div><dt>巡查</dt><dd>{patrol_state}</dd></div>
                <div><dt>异常</dt><dd>{abnormal}</dd></div>
              </dl>
            </section>
            <section class="c-dw-section c-dw-facts">
              <div class="c-dw-section-head"><b>已经取得什么</b><span>同一目标当前事实</span></div>
              <div class="c-dw-readouts c-dw-overview-readouts">
                <div><b>{directory}</b><span>作品目录</span></div>
                <div><b>{detail}</b><span>详情进度</span></div>
                <div><b>{last}</b><span>上次巡查</span></div>
                <div><b>{next}</b><span>下次巡查</span></div>
              </div>
            </section>
            <section class="c-dw-section c-dw-decision" id="archive-problems">
              <div class="c-dw-decision-copy"><span>是否需要处理</span><strong>{action_title}</strong><p>{action_note}</p></div>{action_control}
              {retire}
            </section>"#,
        retire = retirement_form(target, retirable, list_context),
        directory = directory,
        detail = detail,
        patrol_state = escape(patrol_state),
        abnormal = escape(&abnormal),
        last = escape(last),
        next = escape(next),
    )
}

fn inspector_unavailable_overview(is_creator: bool) -> String {
    format!(
        "{}\n            <section class=\"c-dw-section c-dw-decision\">\n              <div class=\"c-dw-decision-copy\"><span>是否需要处理</span><strong>当前无法判断</strong><p>读取恢复前不提供新的写操作，避免用未知状态触发重复任务。</p></div>\n            </section>",
        inspector_unavailable_notice(is_creator),
    )
}

/// 检查器读失败时，keyword 的巡查事实未知；但建档态由独立查询得出，不能把后者一并丢掉。
fn keyword_inspector_unavailable_overview(
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    let action = target_primary_action(target, false, archive, keyword_archive);
    let (action_title, action_note) = required_action_copy(action);
    let action_control =
        required_action_control(target, archive, false, keyword_archive, list_context);
    format!(
        r#"{notice}<section class="c-dw-section c-dw-decision" id="required-action">
              <div class="c-dw-decision-copy"><span>是否需要处理</span><strong>{action_title}</strong><p>{action_note}</p></div>{action_control}
            </section>"#,
        notice = inspector_unavailable_notice(false),
        action_title = action_title,
        action_note = action_note,
        action_control = action_control,
    )
}

fn inspector_unavailable_notice(is_creator: bool) -> String {
    let scope = if is_creator {
        "档案、执行与巡查"
    } else {
        "巡查与最近结果"
    };
    format!(
        r#"<section class="c-dw-section c-dw-now">
              <div class="c-dw-section-head"><b>系统现在在做什么</b><span>读取暂不可用</span></div>
              <div class="life-state"><b>目标状态暂时读不到</b><p>当前无法判断{scope}；这不表示没有任务、没有作品或运行正常。</p></div>
            </section>"#,
    )
}

/// 让人把「这几篇在平台上已经没了」这个结论写下来。
///
/// 只有在**确实还有待判断的作品**时才出现：没有可判断的对象却摆一个按钮，等于请人对空气
/// 做决定。列出的每一篇都带标题与平台 id——人是照着这些去平台上核对的，光给一个内部编号
/// 等于要求他凭记忆判断。
///
/// 它不删作品，也不抹掉那次读取失败：作品继续留在目录里（博主当时确实发过），失败记录
/// 继续留着（那次确实失败过）。写下的只是第三件事——有人看过，它没了。
fn retirement_form(
    target: &ObservationTarget,
    retirable: &[BlockedMaterial],
    list_context: TargetListContext<'_>,
) -> String {
    if retirable.is_empty() {
        return String::new();
    }
    let items = retirable
        .iter()
        .map(|material| {
            let title = material.title.as_deref().unwrap_or("未取得标题");
            format!(
                r#"<li><label><input type="checkbox" name="content_public_ref" value="{value}" checked/><b>{title}</b><span>{external}</span></label></li>"#,
                value = material.content_public_ref,
                title = escape(title),
                external = escape(&material.content_external_id),
            )
        })
        .collect::<String>();
    let fields = list_context.return_fields(
        Some(target.target_ref),
        Some(TargetDrawerTab::Overview),
        Some("archive-problems"),
    );
    format!(
        r#"<form class="c-dw-retire" method="post" action="/collection/targets/retire-materials">
             {fields}
             <input type="hidden" name="row_target_ref" value="{target_ref}"/>
             <b>这些作品连续读不到，等你判断</b>
             <p>确认之后它们仍留在作品目录里，只是不再计入待补齐——博主当时确实发过，抹掉分母会让「档案完成」建立在一个修饰过的数字上。判断错了可以重新采集，记录不会挡住它。</p>
             <ul>{items}</ul>
             <button class="c-btn-primary" type="submit">确认这些作品已失效</button>
           </form>"#,
        target_ref = target.target_ref,
    )
}

fn inspector_overview(
    target: &ObservationTarget,
    is_creator: bool,
    inspector: &TargetInspectorProjection,
    archive: TargetArchiveRead<'_>,
    keyword_archive: KeywordArchiveRead,
    retirable: &[BlockedMaterial],
    list_context: TargetListContext<'_>,
) -> String {
    let last = inspector
        .patrol
        .last_succeeded_at
        .as_deref()
        .unwrap_or("尚未取得成功结果");
    let next = inspector.patrol.next_run_at.as_deref().unwrap_or_else(|| {
        if inspector.patrol.state == TargetInspectorPatrolState::Disabled {
            "未开启"
        } else {
            "待排定"
        }
    });
    if !is_creator {
        // 关键词的主操作与列表行**同一个函数、同一组输入**：同一个词在两个表面上必须是
        // 同一个答案。照检查器投影渲染做不到这一点——关键词的档案态恒为 `NotApplicable`
        // （`archive_projection` 对非 creator 提前返回），于是每个关键词都得到「当前无需
        // 处理」，而列表行写着「建立档案」，补采缺口的入口在抽屉里根本不存在。
        let action = target_primary_action(target, false, archive, keyword_archive);
        let (action_title, action_note) = required_action_copy(action);
        let action_control =
            required_action_control(target, archive, false, keyword_archive, list_context);
        return format!(
            r#"<section class="c-dw-section c-dw-now">
                  <div class="c-dw-section-head"><b>系统现在在做什么</b><span>状态分别判断</span></div>
                  <dl class="c-dw-state-list"><div><dt>巡查</dt><dd>{patrol}</dd></div><div><dt>最近结果</dt><dd>{last}</dd></div></dl>
                </section>
                <section class="c-dw-section c-dw-facts">
                  <div class="c-dw-section-head"><b>已经取得什么</b><span>最近一次成功巡查</span></div>
                  <div class="c-dw-readouts c-dw-overview-readouts">
                    <div><b>{hits}</b><span>最近命中</span></div><div><b>{new}</b><span>其中新增</span></div>
                    <div><b>{last}</b><span>上次巡查</span></div><div><b>{next}</b><span>下次巡查</span></div>
                  </div>
                </section>
                <section class="c-dw-section c-dw-decision"><div class="c-dw-decision-copy"><span>是否需要处理</span><strong>{action_title}</strong><p>{action_note}</p></div>{action_control}</section>"#,
            patrol = inspector_patrol_copy(inspector.patrol.state),
            hits = inspector_count_copy(inspector.patrol.latest_hits),
            new = inspector_count_copy(inspector.patrol.latest_new),
            last = escape(last),
            next = escape(next),
        );
    }
    // Inspector 的投影只判断档案/执行/巡查，不拥有业务领域。未分配候选必须先走与列表、
    // 抽屉顶部相同的领域动作；否则这里会生成一个必然被准入拒绝的“建立档案”POST。
    let primary_action =
        target_primary_action(target, true, archive, KeywordArchiveRead::Unavailable);
    let (action_title, action_note, action_control) = if primary_action
        == TargetPrimaryAction::AssignDomain
        || (target.lifecycle_state == "dismissed"
            && primary_action == TargetPrimaryAction::ViewCreator)
    {
        let action = primary_action;
        let (title, note) = required_action_copy(action);
        (
            title,
            note,
            required_action_control(
                target,
                archive,
                true,
                KeywordArchiveRead::Unavailable,
                list_context,
            ),
        )
    } else {
        let action = inspector.required_action;
        let (title, note) = inspector_action_copy(action);
        (
            title,
            note,
            inspector_action_control(target, action, list_context),
        )
    };
    let directory = inspector_count_copy(inspector.coverage.directory_works);
    let detail = inspector_detail_copy(
        inspector.coverage.captured_details,
        inspector.coverage.directory_works,
    );
    let missing = inspector_count_copy(inspector.coverage.missing_details);
    let abnormal = inspector_abnormal_copy(
        inspector.coverage.quarantined_records,
        inspector.coverage.blocked_details,
    );
    format!(
        r#"<section class="c-dw-section c-dw-now" id="archive-task">
              <div class="c-dw-section-head"><b>系统现在在做什么</b><span>状态分别判断</span></div>
              <dl class="c-dw-state-list">
                <div><dt>建档</dt><dd>{archive_state}</dd></div>
                <div><dt>执行</dt><dd>{execution_state}</dd></div>
                <div><dt>巡查</dt><dd>{patrol_state}</dd></div>
                <div><dt>异常</dt><dd>{abnormal}</dd></div>
              </dl>
            </section>
            <section class="c-dw-section c-dw-facts">
              <div class="c-dw-section-head"><b>已经取得什么</b><span>同一时点的目标事实</span></div>
              <div class="c-dw-readouts c-dw-overview-readouts">
                <div><b>{directory}</b><span>作品目录</span></div>
                <div><b>{detail}</b><span>详情进度</span></div>
                <div><b>{missing}</b><span>待取得详情</span></div>
                <div><b>{latest_new}</b><span>最近巡查新增</span></div>
                <div><b>{last}</b><span>上次巡查</span></div>
                <div><b>{next}</b><span>下次巡查</span></div>
              </div>
            </section>
            <section class="c-dw-section c-dw-decision" id="archive-problems">
              <div class="c-dw-decision-copy"><span>是否需要处理</span><strong>{action_title}</strong><p>{action_note}</p></div>{action_control}
              {retire}
            </section>"#,
        retire = retirement_form(target, retirable, list_context),
        archive_state = inspector_archive_copy(inspector.archive.state),
        execution_state = inspector_execution_copy(inspector.execution.state),
        patrol_state = inspector_patrol_copy(inspector.patrol.state),
        latest_new = inspector_count_copy(inspector.patrol.latest_new),
        last = escape(last),
        next = escape(next),
    )
}

fn inspector_archive_copy(state: TargetInspectorArchiveState) -> &'static str {
    match state {
        TargetInspectorArchiveState::NotApplicable => "不适用",
        TargetInspectorArchiveState::NotStarted => "尚未建立",
        TargetInspectorArchiveState::Queued => "已排队",
        TargetInspectorArchiveState::Running => "正在执行",
        TargetInspectorArchiveState::Partial => "部分可用",
        TargetInspectorArchiveState::Blocked => "有待处理项",
        TargetInspectorArchiveState::Complete => "已建立",
        TargetInspectorArchiveState::Unavailable => "当前读不到",
    }
}

fn inspector_execution_copy(state: TargetInspectorExecutionState) -> &'static str {
    match state {
        TargetInspectorExecutionState::Idle => "当前无任务执行",
        TargetInspectorExecutionState::Queued => "已排队，等待调度",
        TargetInspectorExecutionState::AwaitingProducer => "已分配，等待执行工位",
        TargetInspectorExecutionState::Running => "正在执行",
        TargetInspectorExecutionState::Blocked => "执行已阻塞",
    }
}

fn inspector_patrol_copy(state: TargetInspectorPatrolState) -> &'static str {
    match state {
        TargetInspectorPatrolState::Disabled => "未开启",
        TargetInspectorPatrolState::Waiting => "已开启，等待首次结果",
        TargetInspectorPatrolState::Queued => "已排队",
        TargetInspectorPatrolState::AwaitingProducer => "等待执行工位",
        TargetInspectorPatrolState::Running => "正在巡查",
        TargetInspectorPatrolState::Normal => "运行正常",
        TargetInspectorPatrolState::Blocked => "巡查已阻塞",
    }
}

fn inspector_count_copy(value: TargetInspectorCount) -> String {
    match value {
        TargetInspectorCount::Known(value) => value.to_string(),
        TargetInspectorCount::Unknown => "尚未取得".to_owned(),
    }
}

fn inspector_detail_copy(captured: TargetInspectorCount, total: TargetInspectorCount) -> String {
    match (captured, total) {
        (TargetInspectorCount::Known(captured), TargetInspectorCount::Known(total)) => {
            format!("{captured}/{total}")
        }
        _ => "尚未取得".to_owned(),
    }
}

fn inspector_abnormal_copy(
    quarantined: TargetInspectorCount,
    blocked: TargetInspectorCount,
) -> String {
    match (quarantined, blocked) {
        (TargetInspectorCount::Known(quarantined), TargetInspectorCount::Known(blocked)) => {
            let total = quarantined.saturating_add(blocked);
            if total == 0 {
                "无已知阻塞".to_owned()
            } else {
                format!("{total} 条待处理")
            }
        }
        _ => "当前读不到".to_owned(),
    }
}

fn inspector_action_copy(action: TargetInspectorAction) -> (&'static str, &'static str) {
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

fn inspector_action_control(
    target: &ObservationTarget,
    action: TargetInspectorAction,
    list_context: TargetListContext<'_>,
) -> String {
    match action {
        TargetInspectorAction::StartArchive
        | TargetInspectorAction::RebuildDirectory
        | TargetInspectorAction::ContinueArchive => {
            let label = match action {
                TargetInspectorAction::StartArchive => "建立档案",
                TargetInspectorAction::RebuildDirectory => "处理异常",
                TargetInspectorAction::ContinueArchive => "补采缺口",
                _ => unreachable!(),
            };
            let gap_field = if action == TargetInspectorAction::ContinueArchive {
                r#"<input type="hidden" name="archive_action" value="gaps">"#
            } else {
                ""
            };
            let fields = list_context.return_fields(
                Some(target.target_ref),
                Some(TargetDrawerTab::Overview),
                Some("archive-problems"),
            );
            let domain_field = list_context.acquisition_domain_field();
            format!(
                r#"<form class="c-dw-primary-form" method="post" action="/collection/targets/archive">{fields}{domain_field}{gap_field}<button class="c-btn-primary" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
                target_ref = target.target_ref,
            )
        }
        TargetInspectorAction::EnablePatrol => {
            let opener_id = format!("drawer-overview-monitor-rule-{}", target.target_ref);
            let href = list_context.monitor_rule_href(target.target_ref, &opener_id);
            format!(
                r#"<a id="{opener_id}" class="c-btn-primary" data-monitor-rule-trigger="{target_ref}" href="{href}">开启巡查</a>"#,
                target_ref = target.target_ref,
            )
        }
        TargetInspectorAction::HandleArchiveProblems => {
            let href = list_context.drawer_href(
                target.target_ref,
                &[
                    ("dtab", "works"),
                    ("wview", "list"),
                    ("catalog_filter", "pending"),
                ],
                Some("target-works"),
            );
            format!(r#"<a class="c-btn-secondary" href="{href}">查看待取得作品</a>"#)
        }
        _ => String::new(),
    }
}

fn current_system_copy(archive: TargetArchiveRead<'_>) -> (&'static str, &'static str) {
    match archive {
        TargetArchiveRead::Unavailable => ("档案状态未知", "执行状态未知"),
        TargetArchiveRead::Known(None) => ("尚未建立", "当前无建档任务"),
        TargetArchiveRead::Known(Some(value)) if value.has_actionable_problems() => {
            ("档案有待处理项", "自动处理已停止")
        }
        TargetArchiveRead::Known(Some(value)) if value.work_in_progress => {
            ("档案正在完善", "已有任务，等待调度或执行")
        }
        TargetArchiveRead::Known(Some(value)) if value.requires_directory_rebuild() => {
            ("目录边界需要重建", "当前无建档任务")
        }
        TargetArchiveRead::Known(Some(value))
            if value.works_listed > 0 && value.pending_details > 0 =>
        {
            ("档案部分可用", "当前无建档任务")
        }
        TargetArchiveRead::Known(Some(value)) if value.has_displayable_directory() => {
            ("档案已建立", "当前无建档任务")
        }
        TargetArchiveRead::Known(Some(_)) => ("档案尚未建立", "当前无建档任务"),
    }
}

fn required_action_copy(action: TargetPrimaryAction) -> (&'static str, &'static str) {
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

fn required_action_control(
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    keyword_archive: KeywordArchiveRead,
    list_context: TargetListContext<'_>,
) -> String {
    match target_primary_action(target, is_creator, archive, keyword_archive) {
        TargetPrimaryAction::AssignDomain
        | TargetPrimaryAction::EstablishArchive
        | TargetPrimaryAction::RebuildDirectory
        | TargetPrimaryAction::ContinueArchive
        | TargetPrimaryAction::OpenPatrol(_) => {
            drawer_primary_action(target, archive, is_creator, keyword_archive, list_context)
        }
        _ => String::new(),
    }
}

fn lifecycle_query_state(
    lifecycle: LifecycleView<'_>,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
) -> Vec<(&'static str, String)> {
    let (window, metric, validate_selection) = match lifecycle {
        LifecycleView::Projection(projection) => (
            projection.window,
            projection.metric,
            Some(&projection.points),
        ),
        LifecycleView::ReadUnavailable { window, metric }
        | LifecycleView::NotRead { window, metric } => (window, metric, None),
        LifecycleView::QueryInvalid => return Vec::new(),
    };
    let selected = selected_work
        .and_then(|selected| uuid::Uuid::parse_str(selected).ok())
        .map(|selected| selected.to_string())
        .filter(|candidate| {
            validate_selection
                .map(|points| {
                    points
                        .iter()
                        .any(|point| point.work_public_ref.to_string() == candidate.as_str())
                })
                .unwrap_or(true)
        });
    let mut params = vec![
        ("life_window", window.as_str().to_owned()),
        ("life_metric", metric.as_str().to_owned()),
        ("life_chart", chart_view.as_str().to_owned()),
        ("life_grain", trend_grain.as_str().to_owned()),
    ];
    if let Some(selected) = selected {
        params.push(("life_work", selected));
    }
    params
}

fn lifecycle_overview(
    target: &ObservationTarget,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    if matches!(lifecycle, LifecycleView::QueryInvalid) {
        return lifecycle_state(
            "作品分布的查询条件无效",
            "地址里的时间范围或指标不受支持。页面没有改用默认条件，也没有把失败读取成空数据。",
        );
    }
    if !is_creator {
        return lifecycle_state(
            "不适用于关键词目标",
            "关键词按搜索命中持续观察，不拥有创作者作品档案，因此这里不显示创作者生命周期图。",
        );
    }
    let projection = match lifecycle {
        LifecycleView::Projection(projection) => projection,
        LifecycleView::ReadUnavailable { .. }
        | LifecycleView::NotRead { .. }
        | LifecycleView::QueryInvalid => {
            return lifecycle_state(
                "作品分布当前读不到",
                "当前读取状态未知，不表示创作者没有作品，也不表示互动数据为零。",
            );
        }
    };
    if projection.status == CreatorLifecycleStatus::NotApplicable {
        return lifecycle_state(
            "不适用于当前目标",
            "只有已确认身份的创作者目标才能生成作品生命周期。",
        );
    }

    let controls = lifecycle_controls(target, projection, chart_view, trend_grain, list_context);
    let has_points = !projection.points.is_empty();
    let summary = performance_summary(projection);
    let chart = if !has_points {
        lifecycle_state(
            "观察不足，暂时无法成图",
            "作品必须同时具备可确认的作者归属、真实发布时间和当前互动数据。未知值不会按零计算。",
        )
    } else {
        match chart_view {
            LifeChartView::Trend => performance_trend_chart(
                target,
                projection,
                trend_grain,
                selected_work,
                list_context,
            ),
            LifeChartView::Distribution => performance_distribution_chart(
                target,
                projection,
                chart_view,
                trend_grain,
                selected_work,
                list_context,
            ),
        }
    };
    let review = has_points
        .then(|| performance_review(projection, chart_view, trend_grain))
        .unwrap_or_default();
    let evidence = performance_evidence(target, projection, chart_view, trend_grain, list_context);
    let exclusions = lifecycle_exclusions(projection);
    let selected = selected_work
        .and_then(|selected| {
            projection
                .points
                .iter()
                .find(|point| point.work_public_ref.to_string() == selected)
        })
        .map(|point| selected_work_summary(point, projection.metric))
        .unwrap_or_default();

    format!(
        r#"<section class="c-dw-section life-panel" id="creator-lifecycle">
              <div class="life-performance-controls"><div>{controls}</div><p>数据截至 <time>{as_of}</time></p></div>
              {summary}
              <div class="life-performance-grid{empty_state}"><article class="life-trend-panel">{chart}</article>{review}</div>
              {evidence}{exclusions}{selected}
              <p class="life-boundary">这里比较的是该创作者自己的作品表现，不是“监控价值”评分。尚未建立内容分类，因此不按主题生成表现结论。</p>
            </section>"#,
        as_of = escape(&projection.as_of),
        empty_state = if has_points {
            ""
        } else {
            " life-performance-grid-empty"
        },
    )
}

/// The review surface aggregates only the already-qualified lifecycle points.  A bucket is not
/// a new persistence model and is never presented as a platform-wide total: it is merely a
/// readable way to see the same creator-scoped evidence set over time.
fn performance_buckets<'a>(
    points: &'a [CreatorLifecyclePoint],
    grain: LifeTrendGrain,
) -> Vec<(String, Vec<&'a CreatorLifecyclePoint>)> {
    let mut buckets: Vec<(String, Vec<&CreatorLifecyclePoint>)> = Vec::new();
    for point in points {
        let label = trend_bucket_label(&point.published_local_date, grain);
        if let Some((_, items)) = buckets.last_mut().filter(|(key, _)| *key == label) {
            items.push(point);
        } else {
            buckets.push((label, vec![point]));
        }
    }
    buckets
}

fn trend_bucket_label(published_local_date: &str, grain: LifeTrendGrain) -> String {
    match grain {
        LifeTrendGrain::Month => published_local_date
            .get(..7)
            .unwrap_or(published_local_date)
            .replace('-', "/"),
        LifeTrendGrain::Week => calendar_week_start(published_local_date)
            .unwrap_or_else(|| published_local_date.to_owned())
            .replace('-', "/"),
    }
}

/// The read model already supplies an Asia/Shanghai local calendar date. This small calendar
/// helper finds that date's Monday without treating the UTC timestamp as a second publication
/// fact or adding a timezone dependency to the UI adapter.
fn calendar_week_start(date: &str) -> Option<String> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<i64>().ok()?;
    let day = parts.next()?.parse::<i64>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let monday = days - (days + 3).rem_euclid(7);
    let (year, month, day) = civil_from_days(monday);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

// Howard Hinnant's public-domain civil calendar algorithms, expressed here only to bucket the
// already-qualified local date. Day zero is 1970-01-01, a Thursday; Monday therefore maps to 0.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    (year + i64::from(month <= 2), month, day)
}

fn median_f64(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    Some(if sorted.len() % 2 == 0 {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    })
}

fn compact_metric(value: f64) -> String {
    if value.abs() >= 10_000.0 {
        format!("{:.1}万", value / 10_000.0)
    } else {
        exact_metric(value)
    }
}

/// A compact label may save chart space, but it must never become the only representation of a
/// known value. Median and percentile values can be fractional; preserve that fraction while
/// grouping the integer part for the title/accessible alternative.
fn exact_metric(value: f64) -> String {
    let rounded = value.round();
    let raw = if (value - rounded).abs() < 0.000_001 {
        format!("{rounded:.0}")
    } else {
        let mut decimal = format!("{value:.2}");
        while decimal.ends_with('0') {
            decimal.pop();
        }
        if decimal.ends_with('.') {
            decimal.pop();
        }
        decimal
    };
    let (whole, fraction) = raw.split_once('.').unwrap_or((&raw, ""));
    let (sign, digits) = whole
        .strip_prefix('-')
        .map_or(("", whole), |digits| ("-", digits));
    let mut grouped_reversed = String::new();
    for (index, digit) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped_reversed.push(',');
        }
        grouped_reversed.push(digit);
    }
    let grouped = grouped_reversed.chars().rev().collect::<String>();
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}

fn performance_href(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
    fragment: Option<&str>,
) -> String {
    let mut params = vec![
        ("dtab", "works"),
        ("wview", "performance"),
        ("life_window", projection.window.as_str()),
        ("life_metric", projection.metric.as_str()),
        ("life_chart", chart_view.as_str()),
        ("life_grain", trend_grain.as_str()),
    ];
    if let Some(selected_work) = selected_work {
        params.push(("life_work", selected_work));
    }
    list_context.drawer_href(target.target_ref, &params, fragment)
}

fn performance_chart_switch(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    selected_work: Option<&str>,
    active_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    list_context: TargetListContext<'_>,
) -> String {
    [
        (LifeChartView::Trend, "趋势"),
        (LifeChartView::Distribution, "分布"),
    ]
    .into_iter()
    .map(|(view, label)| {
        let current = if view == active_view {
            r#" aria-current="page""#
        } else {
            ""
        };
        let href = performance_href(
            target,
            projection,
            view,
            trend_grain,
            selected_work,
            list_context,
            Some("creator-lifecycle"),
        );
        format!(r#"<a class="life-chart-switch-tab" href="{href}"{current}>{label}</a>"#)
    })
    .collect::<String>()
}

fn performance_grain_switch(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    [LifeTrendGrain::Month, LifeTrendGrain::Week]
        .into_iter()
        .map(|grain| {
            let current = if grain == trend_grain {
                r#" aria-current="page""#
            } else {
                ""
            };
            let href = performance_href(
                target,
                projection,
                LifeChartView::Trend,
                grain,
                selected_work,
                list_context,
                Some("creator-lifecycle"),
            );
            format!(r#"<a href="{href}"{current}>{}</a>"#, grain.label())
        })
        .collect::<String>()
}

fn performance_stat(value: String, label: &str, progress: Option<f64>, signal: bool) -> String {
    let progress = progress
        .map(|value| {
            format!(
                r#"<span class="life-stat-progress{}"><i style="--life-progress:{:.2}%"></i></span>"#,
                if signal { " life-stat-progress-signal" } else { "" },
                value.clamp(0.0, 100.0),
            )
        })
        .unwrap_or_default();
    format!(
        r#"<div class="life-performance-stat"><div><b>{value}</b><span>{label}</span></div>{progress}</div>"#,
    )
}

fn performance_summary(projection: &CreatorLifecycleProjection) -> String {
    let linked = projection.summary.linked_work_count;
    let confirmed = projection.summary.confirmed_author_work_count;
    let eligible = projection.summary.eligible_point_count;
    let (directory, directory_label) = match linked {
        Some(count) => (count.to_string(), "作品目录"),
        None => (
            format!("≥{}", projection.summary.linked_work_count_lower_bound),
            "作品目录下限",
        ),
    };
    let confirmed_progress = linked
        .filter(|count| *count > 0)
        .map(|count| confirmed as f64 / count as f64 * 100.0);
    let eligible_progress = (confirmed > 0).then(|| eligible as f64 / confirmed as f64 * 100.0);
    format!(
        r#"<div class="life-performance-summary" aria-label="作品覆盖摘要">
              {directory}{confirmed}{eligible}
            </div>"#,
        directory = performance_stat(directory, directory_label, linked.map(|_| 100.0), false),
        confirmed = performance_stat(
            confirmed.to_string(),
            "作者已确认",
            confirmed_progress,
            false,
        ),
        eligible = performance_stat(eligible.to_string(), "当前可分析", eligible_progress, true,),
    )
}

fn performance_trend_chart(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    const WIDTH: f64 = 980.0;
    const HEIGHT: f64 = 470.0;
    const LEFT: f64 = 62.0;
    const RIGHT: f64 = 28.0;
    const TOP: f64 = 28.0;
    const BOTTOM: f64 = 54.0;

    let buckets = performance_buckets(&projection.points, trend_grain);
    let bucket_medians = buckets
        .iter()
        .map(|(_, points)| {
            median_f64(
                &points
                    .iter()
                    .map(|point| point.metric_value as f64)
                    .collect::<Vec<_>>(),
            )
            .unwrap_or(0.0)
        })
        .collect::<Vec<_>>();
    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let max_metric = bucket_medians
        .iter()
        .copied()
        .fold(0.0_f64, f64::max)
        .max(1.0)
        * 1.18;
    let max_posts = buckets
        .iter()
        .map(|(_, points)| points.len())
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let step = if buckets.len() > 1 {
        plot_width / (buckets.len() - 1) as f64
    } else {
        plot_width / 2.0
    };
    let x_for = |index: usize| {
        if buckets.len() == 1 {
            LEFT + plot_width / 2.0
        } else {
            LEFT + step * index as f64
        }
    };
    let y_for = |value: f64| TOP + plot_height - value / max_metric * plot_height;
    let grid = (0..5)
        .map(|index| {
            let y = TOP + plot_height / 4.0 * index as f64;
            let value = max_metric * (1.0 - index as f64 / 4.0);
            let compact = compact_metric(value);
            let exact = exact_metric(value);
            format!(
                r#"<line class="life-trend-grid" x1="{LEFT}" y1="{y:.1}" x2="{}" y2="{y:.1}"/><text class="life-trend-axis" x="{}" y="{:.1}" text-anchor="end" aria-label="精确数值 {exact}"><title>精确数值 {exact}</title>{compact}</text>"#,
                WIDTH - RIGHT,
                LEFT - 12.0,
                y + 4.0,
            )
        })
        .collect::<String>();
    let bars = buckets
        .iter()
        .enumerate()
        .map(|(index, (_, points))| {
            let x = x_for(index);
            let height = points.len() as f64 / max_posts * 72.0;
            let width = (step * 0.42).min(30.0).max(12.0);
            format!(
                r#"<rect class="life-trend-bar" x="{:.1}" y="{:.1}" width="{width:.1}" height="{height:.1}" rx="1"/>"#,
                x - width / 2.0,
                TOP + plot_height - height,
            )
        })
        .collect::<String>();
    let line_points = bucket_medians
        .iter()
        .enumerate()
        .map(|(index, value)| (x_for(index), y_for(*value)))
        .collect::<Vec<_>>();
    let line = smooth_svg_path(&line_points);
    let area = format!(
        "{line} L {:.1},{:.1} L {:.1},{:.1} Z",
        line_points.last().map(|(x, _)| *x).unwrap_or(LEFT),
        TOP + plot_height,
        line_points.first().map(|(x, _)| *x).unwrap_or(LEFT),
        TOP + plot_height,
    );
    let benchmark = median_f64(
        &projection
            .points
            .iter()
            .map(|point| point.metric_value as f64)
            .collect::<Vec<_>>(),
    )
    .unwrap_or(0.0);
    let benchmark_y = y_for(benchmark);
    let dots = buckets
        .iter()
        .enumerate()
        .map(|(index, (_, points))| {
            let x = x_for(index);
            let y = y_for(bucket_medians[index]);
            let is_new = points.iter().any(|point| point.new_in_latest_patrol);
            let ring = if is_new {
                format!(r#"<circle class="life-trend-new-ring" cx="{x:.1}" cy="{y:.1}" r="9.5"/>"#,)
            } else {
                String::new()
            };
            format!(
                r#"{ring}<circle class="life-trend-dot{}" cx="{x:.1}" cy="{y:.1}" r="4.4"/>"#,
                if is_new { " life-trend-dot-new" } else { "" },
            )
        })
        .collect::<String>();
    let labels = buckets
        .iter()
        .enumerate()
        .filter(|(index, _)| {
            buckets.len() <= 7 || *index == 0 || *index + 1 == buckets.len() || *index % 2 == 0
        })
        .map(|(index, (label, _))| {
            let anchor = if index == 0 {
                "start"
            } else if index + 1 == buckets.len() {
                "end"
            } else {
                "middle"
            };
            format!(
                r#"<text class="life-trend-axis" x="{:.1}" y="{}" text-anchor="{anchor}">{}</text>"#,
                x_for(index),
                HEIGHT - 24.0,
                escape(label),
            )
        })
        .collect::<String>();
    let trend_right = WIDTH - RIGHT;
    let benchmark_copy_y = benchmark_y - 7.0;
    let benchmark_label = compact_metric(benchmark);
    let benchmark_exact = exact_metric(benchmark);
    let vertical_label_y = TOP + plot_height / 2.0;
    let chart_switch = performance_chart_switch(
        target,
        projection,
        selected_work,
        LifeChartView::Trend,
        trend_grain,
        list_context,
    );
    let grain_switch =
        performance_grain_switch(target, projection, trend_grain, selected_work, list_context);
    format!(
        r#"<div class="life-trend-head"><div><div class="life-trend-eyebrow">作品复核</div><h2>作品表现趋势</h2><p>柱形表示发布密度，主线使用同一时间桶的{metric}中位数；橙红只标记最近巡查新增。</p></div><div class="life-chart-head-actions"><nav class="life-chart-switch" aria-label="图表视图">{chart_switch}</nav><nav class="life-grain-switch" aria-label="趋势粒度"><span>粒度</span>{grain_switch}</nav></div></div>
            <div class="life-trend-legend" aria-label="趋势图图例"><span><i class="life-legend-bar"></i>发布作品数</span><span><i class="life-legend-line"></i>{metric}中位数</span><span><i class="life-legend-dash"></i>当前窗口中位基准</span><span><i class="life-legend-ring"></i>巡查新增</span></div>
            <figure class="life-trend-figure"><svg class="life-trend-chart" viewBox="0 0 980 470" role="img" aria-labelledby="life-trend-chart-title life-trend-chart-desc"><title id="life-trend-chart-title">创作者作品表现趋势</title><desc id="life-trend-chart-desc">横轴为当前时间窗口的{grain}，柱形表示可分析作品数，墨线表示同一时间桶的{metric}中位数，虚线表示当前窗口所有已纳入作品的{metric}中位数。橙红圆环仅表示该桶包含最近巡查新增作品。</desc><defs><linearGradient id="life-trend-area-gradient" x1="0" y1="0" x2="0" y2="1"><stop offset="0%" stop-color="var(--lgi-signal-soft)" stop-opacity="0.92"/><stop offset="100%" stop-color="var(--lgi-canvas)" stop-opacity="0"/></linearGradient></defs>{grid}{bars}<path class="life-trend-area" d="{area}"/><line class="life-trend-benchmark" x1="{LEFT}" y1="{benchmark_y:.1}" x2="{trend_right:.1}" y2="{benchmark_y:.1}"/><text class="life-trend-benchmark-copy" x="{trend_right:.1}" y="{benchmark_copy_y:.1}" text-anchor="end" aria-label="当前窗口精确中位数 {benchmark_exact}"><title>当前窗口精确中位数 {benchmark_exact}</title>当前窗口中位 · {benchmark_label}</text><path class="life-trend-line" d="{line}"/>{dots}{labels}<text class="life-trend-axis" x="17" y="{vertical_label_y:.1}" transform="rotate(-90 17 {vertical_label_y:.1})" text-anchor="middle">{metric}中位数</text></svg></figure>"#,
        metric = metric_label(projection.metric),
        grain = trend_grain.label(),
        chart_switch = chart_switch,
        grain_switch = grain_switch,
        grid = grid,
        bars = bars,
        area = area,
        benchmark_y = benchmark_y,
        benchmark_label = benchmark_label,
        benchmark_exact = benchmark_exact,
        line = line,
        dots = dots,
        labels = labels,
        LEFT = LEFT,
        trend_right = trend_right,
        benchmark_copy_y = benchmark_copy_y,
        vertical_label_y = vertical_label_y,
    )
}

fn smooth_svg_path(points: &[(f64, f64)]) -> String {
    let Some((first_x, first_y)) = points.first().copied() else {
        return String::new();
    };
    if points.len() == 1 {
        return format!("M {first_x:.1},{first_y:.1}");
    }
    let mut path = format!("M {first_x:.1},{first_y:.1}");
    for index in 0..points.len() - 1 {
        let previous = points[index.saturating_sub(1)];
        let current = points[index];
        let next = points[index + 1];
        let following = points.get(index + 2).copied().unwrap_or(next);
        let control_one = (
            current.0 + (next.0 - previous.0) / 6.0,
            current.1 + (next.1 - previous.1) / 6.0,
        );
        let control_two = (
            next.0 - (following.0 - current.0) / 6.0,
            next.1 - (following.1 - current.1) / 6.0,
        );
        path.push_str(&format!(
            " C {:.1},{:.1} {:.1},{:.1} {:.1},{:.1}",
            control_one.0, control_one.1, control_two.0, control_two.1, next.0, next.1
        ));
    }
    path
}

fn performance_review(
    projection: &CreatorLifecycleProjection,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
) -> String {
    match chart_view {
        LifeChartView::Trend => performance_trend_review(projection, trend_grain),
        LifeChartView::Distribution => performance_distribution_review(projection),
    }
}

fn performance_trend_review(
    projection: &CreatorLifecycleProjection,
    trend_grain: LifeTrendGrain,
) -> String {
    let buckets = performance_buckets(&projection.points, trend_grain);
    let metric_values = projection
        .points
        .iter()
        .map(|point| point.metric_value as f64)
        .collect::<Vec<_>>();
    let median = median_f64(&metric_values).unwrap_or(0.0);
    let median_label = compact_metric(median);
    let median_exact = exact_metric(median);
    let active_buckets = buckets.len().max(1);
    let density = projection.summary.eligible_point_count as f64 / active_buckets as f64;
    let new_count = projection
        .points
        .iter()
        .filter(|point| point.new_in_latest_patrol)
        .count();
    let max_posts = buckets
        .iter()
        .map(|(_, points)| points.len())
        .max()
        .unwrap_or(1)
        .max(1) as f64;
    let strip = buckets
        .iter()
        .rev()
        .take(7)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .map(|(_, points)| {
            let size = 2.0 + points.len() as f64 / max_posts * 5.0;
            let hot = points.iter().any(|point| point.new_in_latest_patrol);
            format!(
                r#"<i{} style="--life-strip-size:{size:.3}"></i>"#,
                if hot { " class=\"life-mini-hot\"" } else { "" },
            )
        })
        .collect::<String>();
    format!(
        r#"<aside class="life-review" aria-label="复核摘要"><div class="life-review-head"><b>复核摘要</b><span>把“变化”转成可核验线索。</span></div>
              <div class="life-review-item"><div><span>当前窗口中位{metric}</span></div><strong title="精确中位数 {median_exact}" aria-label="精确中位数 {median_exact}">{median}</strong><p>只按当前可分析作品计算；缺少指标的作品不会以零参与中位数。</p><div class="life-mini-strip">{strip}</div></div>
              <div class="life-review-item"><div><span>发布密度</span><em>{density:.1} / {density_unit}</em></div><strong>{eligible}</strong><p>当前窗口内 {eligible} 篇可分析作品，分布在 {bucket_count} 个{grain}时间桶。</p></div>
              <div class="life-review-item"><div><span>最近巡查新增</span><em>{new_count} 篇</em></div><strong>{new_count}</strong><p>这里仅标记已进入当前可分析集的新增作品，未把读取未知写成零。</p></div>
              <div class="life-review-boundary"><b>复核边界</b><p>观察结果不等于结论。先查证单篇作品与其来源，再做解释。</p></div></aside>"#,
        metric = metric_label(projection.metric),
        median = median_label,
        median_exact = median_exact,
        strip = strip,
        density = density,
        eligible = projection.summary.eligible_point_count,
        density_unit = trend_grain.density_unit(),
        bucket_count = buckets.len(),
        grain = trend_grain.label(),
        new_count = new_count,
    )
}

fn performance_distribution_review(projection: &CreatorLifecycleProjection) -> String {
    let values = projection
        .points
        .iter()
        .map(|point| point.metric_value as f64)
        .collect::<Vec<_>>();
    let typical_low = percentile_f64(&values, 0.25).unwrap_or(0.0);
    let typical_high = percentile_f64(&values, 0.75).unwrap_or(0.0);
    let typical_low_label = compact_metric(typical_low);
    let typical_high_label = compact_metric(typical_high);
    let typical_low_exact = exact_metric(typical_low);
    let typical_high_exact = exact_metric(typical_high);
    let high_discussion = projection
        .points
        .iter()
        .filter(|point| point.discussion_rate.is_some_and(|rate| rate > 0.20))
        .count();
    let discussion_unavailable = projection
        .points
        .iter()
        .filter(|point| point.discussion_rate.is_none())
        .count();
    format!(
        r#"<aside class="life-review" aria-label="复核摘要"><div class="life-review-head"><b>复核摘要</b><span>把“分布”转成逐篇可查的范围。</span></div>
              <div class="life-review-item"><div><span>当前纳入作品</span></div><strong>{eligible}</strong><p>只包括当前时间窗与当前指标均为已知的作品；Known zero 仍保留在图中。</p></div>
              <div class="life-review-item"><div><span>典型区间</span><em>25%–75%</em></div><strong title="精确区间 {typical_low_exact}–{typical_high_exact}" aria-label="精确区间 {typical_low_exact}–{typical_high_exact}">{typical_low}–{typical_high}</strong><p>按当前纳入作品的{metric}第 25 至第 75 百分位计算，不表示行业常态或质量判断。</p></div>
              <div class="life-review-item"><div><span>高讨论率</span><em>&gt; 20%</em></div><strong>{high_discussion}</strong><p>只标记评论／点赞可判且超过 20% 的作品；它是复核信号，不改变纵轴指标。</p></div>
              <div class="life-review-item"><div><span>讨论率未可判</span></div><strong>{discussion_unavailable}</strong><p>评论或点赞未知，或点赞为 0 时不计算比率，不写成低讨论率。</p></div>
              <div class="life-review-item"><div><span>当前指标未知</span></div><strong>{metric_unknown}</strong><p>这类作品没有进入当前分布；未知不以 0 或典型区间的边界替代。</p></div>
              <div class="life-review-boundary"><b>复核边界</b><p>先点击具体作品查证其来源与语境；点的高低不等于内容价值。</p></div></aside>"#,
        eligible = projection.summary.eligible_point_count,
        typical_low = typical_low_label,
        typical_high = typical_high_label,
        typical_low_exact = typical_low_exact,
        typical_high_exact = typical_high_exact,
        metric = metric_label(projection.metric),
        high_discussion = high_discussion,
        discussion_unavailable = discussion_unavailable,
        metric_unknown = projection.exclusions.metric_unknown,
    )
}

fn percentile_f64(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = percentile.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (index - lower as f64))
}

fn performance_evidence(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    list_context: TargetListContext<'_>,
) -> String {
    let rows = projection
        .points
        .iter()
        .rev()
        .take(4)
        .map(|point| {
            let work = point.work_public_ref.to_string();
            let href = performance_href(
                target,
                projection,
                chart_view,
                trend_grain,
                Some(work.as_str()),
                list_context,
                Some("creator-lifecycle"),
            );
            let association = match point.association_state {
                CreatorLifecycleAssociation::DirectoryLinked => "目录已列入，作者待确认",
                CreatorLifecycleAssociation::AuthorConfirmed => "作者已确认",
            };
            format!(
                r#"<tr><td><a class="life-evidence-title" href="{href}">{title}</a></td><td>{published}</td><td class="life-evidence-number">{value}</td><td>{association}</td></tr>"#,
                title = escape(point.title.as_deref().unwrap_or("标题未知")),
                published = escape(&point.published_local_date),
                value = point.metric_value,
                association = association,
            )
        })
        .collect::<String>();
    let content = if rows.is_empty() {
        r#"<p class="life-evidence-empty">当前窗口没有可展示的单篇作品；未知数据不会替换成占位读数。</p>"#.to_owned()
    } else {
        format!(
            r#"<div class="life-evidence-table-wrap"><table class="life-evidence-table"><thead><tr><th>近期作品证据</th><th>发布时间</th><th>{metric}</th><th>归属</th></tr></thead><tbody>{rows}</tbody></table></div>"#,
            metric = metric_label(projection.metric),
        )
    };
    format!(
        r#"<section class="life-evidence"><div class="life-evidence-head"><h2>近期作品证据</h2><p>展示当前窗口最后 4 篇可分析作品；选择一篇可继续进入语料核验。</p></div>{content}</section>"#,
    )
}

fn lifecycle_controls(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    list_context: TargetListContext<'_>,
) -> String {
    let windows = [
        (CreatorLifecycleWindow::Recent90Days, "近 90 天"),
        (CreatorLifecycleWindow::All, "全部周期"),
    ]
    .into_iter()
    .map(|(window, label)| {
        let current = if window == projection.window {
            r#" aria-current="true""#
        } else {
            ""
        };
        let href_params = vec![
            ("dtab", "works"),
            ("wview", "performance"),
            ("life_window", window.as_str()),
            ("life_metric", projection.metric.as_str()),
            ("life_chart", chart_view.as_str()),
            ("life_grain", trend_grain.as_str()),
        ];
        let href =
            list_context.drawer_href(target.target_ref, &href_params, Some("creator-lifecycle"));
        format!(r#"<a class="life-chip"{current} href="{href}">{label}</a>"#)
    })
    .collect::<String>();
    let metrics = [
        (CreatorLifecycleMetric::Likes, "点赞"),
        (CreatorLifecycleMetric::Comments, "评论"),
        (CreatorLifecycleMetric::Collects, "收藏"),
        (CreatorLifecycleMetric::Shares, "转发"),
    ]
    .into_iter()
    .map(|(metric, label)| {
        let current = if metric == projection.metric {
            r#" aria-current="true""#
        } else {
            ""
        };
        let href_params = vec![
            ("dtab", "works"),
            ("wview", "performance"),
            ("life_window", projection.window.as_str()),
            ("life_metric", metric.as_str()),
            ("life_chart", chart_view.as_str()),
            ("life_grain", trend_grain.as_str()),
        ];
        let href =
            list_context.drawer_href(target.target_ref, &href_params, Some("creator-lifecycle"));
        format!(r#"<a class="life-chip"{current} href="{href}">{label}</a>"#)
    })
    .collect::<String>();
    format!(
        r#"<div class="life-controls" aria-label="生命周期口径">
              <div class="life-control-row"><span class="life-control-label">时间</span>{windows}</div>
              <div class="life-control-row"><span class="life-control-label">指标</span>{metrics}</div>
            </div>"#,
    )
}

fn performance_distribution_chart(
    target: &ObservationTarget,
    projection: &CreatorLifecycleProjection,
    chart_view: LifeChartView,
    trend_grain: LifeTrendGrain,
    selected_work: Option<&str>,
    list_context: TargetListContext<'_>,
) -> String {
    const WIDTH: f64 = 980.0;
    const HEIGHT: f64 = 470.0;
    const LEFT: f64 = 62.0;
    const RIGHT: f64 = 28.0;
    const TOP: f64 = 28.0;
    const BOTTOM: f64 = 54.0;
    let min_x = projection
        .points
        .first()
        .map(|point| point.published_at_epoch_ms)
        .unwrap_or(0);
    let max_x = projection
        .points
        .last()
        .map(|point| point.published_at_epoch_ms)
        .unwrap_or(min_x);
    let x_span = (max_x - min_x).max(1) as f64;
    let plot_width = WIDTH - LEFT - RIGHT;
    let plot_height = HEIGHT - TOP - BOTTOM;
    let max_y = projection
        .points
        .iter()
        .map(|point| point.metric_value as f64)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let x_for = |point: &CreatorLifecyclePoint| {
        if min_x == max_x {
            LEFT + plot_width / 2.0
        } else {
            LEFT + (point.published_at_epoch_ms - min_x) as f64 / x_span * plot_width
        }
    };
    let y_for = |value: f64| TOP + (1.0 - value / max_y) * plot_height;
    let grid = (0..5)
        .map(|index| {
            let y = TOP + plot_height / 4.0 * index as f64;
            let value = max_y * (1.0 - index as f64 / 4.0);
            let compact = compact_metric(value);
            let exact = exact_metric(value);
            format!(
                r#"<line class="life-trend-grid" x1="{LEFT}" y1="{y:.1}" x2="{}" y2="{y:.1}"/><text class="life-trend-axis" x="{}" y="{:.1}" text-anchor="end" aria-label="精确数值 {exact}"><title>精确数值 {exact}</title>{compact}</text>"#,
                WIDTH - RIGHT,
                LEFT - 12.0,
                y + 4.0,
            )
        })
        .collect::<String>();
    let points = projection
        .points
        .iter()
        .map(|point| {
            let is_selected = selected_work
                .map(|selected| point.work_public_ref.to_string() == selected)
                .unwrap_or(false);
            let selected_class = if is_selected {
                " life-point-selected"
            } else {
                ""
            };
            let current = if is_selected {
                r#" aria-current="true""#
            } else {
                ""
            };
            let association_label = match point.association_state {
                CreatorLifecycleAssociation::DirectoryLinked => {
                    "来自主页作品目录，详情作者待确认"
                }
                CreatorLifecycleAssociation::AuthorConfirmed => {
                    "作者已确认"
                }
            };
            let (new_class, new_label, new_mark) = if point.new_in_latest_patrol {
                (
                    " life-point-new",
                    "，最近一次巡查新增",
                    format!(
                        r#"<rect class="life-point-new-mark" x="{:.1}" y="{:.1}" width="6" height="6" aria-hidden="true"/>"#,
                        x_for(point) + 6.0,
                        y_for(point.metric_value as f64) - 11.0,
                    ),
                )
            } else {
                ("", "", String::new())
            };
            let (discussion_class, discussion_label, discussion_ring) = point
                .discussion_rate
                .filter(|rate| *rate > 0.20)
                .map(|rate| {
                    (
                        " life-point-high-discussion",
                        format!("，评论／点赞 {:.0}%（高讨论率复核信号）", rate * 100.0),
                        format!(
                            r#"<circle class="life-point-high-discussion-ring" cx="{:.1}" cy="{:.1}" r="10" aria-hidden="true"/>"#,
                            x_for(point),
                            y_for(point.metric_value as f64),
                        ),
                    )
                })
                .unwrap_or(("", String::new(), String::new()));
            let title = point.title.as_deref().unwrap_or("标题未知");
            let work = point.work_public_ref.to_string();
            let href = performance_href(
                target,
                projection,
                chart_view,
                trend_grain,
                Some(work.as_str()),
                list_context,
                Some("creator-lifecycle"),
            );
            format!(
                r#"<a class="life-point{new_class}{discussion_class}{selected_class}" href="{href}" aria-label="{title}，{published}，{metric_label} {value}，{association_label}{new_label}{discussion_label}"{current}>{discussion_ring}{new_mark}<circle class="life-point-hit" cx="{x:.1}" cy="{y:.1}" r="7" aria-hidden="true"/><circle class="life-point-visible" cx="{x:.1}" cy="{y:.1}" r="4.6" aria-hidden="true"/></a>"#,
                title = escape(title),
                published = escape(&point.published_local_date),
                metric_label = metric_label(projection.metric),
                value = point.metric_value,
                x = x_for(point),
                y = y_for(point.metric_value as f64),
            )
        })
        .collect::<String>();
    let start = projection
        .points
        .first()
        .map(|point| point.published_local_date.as_str())
        .unwrap_or("—");
    let end = projection
        .points
        .last()
        .map(|point| point.published_local_date.as_str())
        .unwrap_or("—");
    let chart_switch = performance_chart_switch(
        target,
        projection,
        selected_work,
        LifeChartView::Distribution,
        trend_grain,
        list_context,
    );
    format!(
        r#"<div class="life-trend-head"><div><div class="life-trend-eyebrow">作品复核</div><h2>作品表现分布</h2><p>每个点是一篇当前可分析作品；横轴为发布时间，纵轴保持当前{metric}原始数值。</p></div><div class="life-chart-head-actions"><nav class="life-chart-switch" aria-label="图表视图">{chart_switch}</nav><span class="life-trend-grain">复核信号 · 高讨论率 &gt; 20%</span></div></div>
            <div class="life-trend-legend" aria-label="分布图图例"><span><i class="life-legend-dot"></i>每个点代表一篇作品</span><span><i class="life-legend-ring"></i>高讨论率 &gt; 20%</span><span><i class="life-legend-new"></i>最近巡查新增</span></div>
            <figure class="life-trend-figure"><svg class="life-trend-chart life-distribution-chart" viewBox="0 0 980 470" role="group" aria-labelledby="life-chart-title life-chart-desc"><title id="life-chart-title">创作者作品表现分布图</title><desc id="life-chart-desc">横轴为当前时间窗内合格作品的发布时间，纵轴为当前指标{metric}的原始数值。每个可交互点代表一篇作品。橙红外圈表示评论／点赞超过 20% 的复核信号；评论或点赞未知、或点赞为零的作品不参与该信号。</desc>{grid}<line class="life-axis" x1="{LEFT}" y1="{plot_bottom:.1}" x2="{plot_right:.1}" y2="{plot_bottom:.1}"/>{points}<text class="life-axis-copy" x="{LEFT}" y="{caption_y:.1}">{start}</text><text class="life-axis-copy" x="{plot_right:.1}" y="{caption_y:.1}" text-anchor="end">{end}</text><text class="life-axis-copy" x="17" y="{vertical_label_y:.1}" transform="rotate(-90 17 {vertical_label_y:.1})" text-anchor="middle">{metric}</text></svg></figure>"#,
        metric = metric_label(projection.metric),
        chart_switch = chart_switch,
        grid = grid,
        LEFT = LEFT,
        plot_bottom = TOP + plot_height,
        plot_right = WIDTH - RIGHT,
        caption_y = HEIGHT - 24.0,
        start = escape(start),
        end = escape(end),
        vertical_label_y = TOP + plot_height / 2.0,
    )
}

fn lifecycle_exclusions(projection: &CreatorLifecycleProjection) -> String {
    let exclusion_items = [
        ("作者未确认", projection.exclusions.author_not_verified),
        ("作者不匹配", projection.exclusions.author_mismatch),
        (
            "发布时间不合格",
            projection.exclusions.published_at_not_qualified,
        ),
        ("不在当前时间窗", projection.exclusions.outside_window),
        ("指标未知", projection.exclusions.metric_unknown),
    ]
    .into_iter()
    .filter(|(_, count)| *count > 0)
    .map(|(label, count)| format!("<li>{label} {count}</li>"))
    .collect::<String>();
    if exclusion_items.is_empty() && !projection.receipt.truncated {
        return String::new();
    }
    let truncated = if projection.receipt.truncated {
        "<li>当前作品范围仍有后续内容</li>"
    } else {
        ""
    };
    format!(
        r#"<ul class="life-exclusions" aria-label="未进入当前曲线的原因">{exclusion_items}{truncated}</ul>"#,
    )
}

fn selected_work_summary(point: &CreatorLifecyclePoint, metric: CreatorLifecycleMetric) -> String {
    format!(
        r#"<section class="life-selected" aria-label="选中作品摘要">
              <h3>{title}</h3>
              <dl>
                <div><dt>发布时间</dt><dd>{published}</dd></div>
                <div><dt>{metric}</dt><dd>{value}</dd></div>
              </dl>
              <a class="life-corpus-link" href="/corpus/evidence?work={work}">到语料页查看这篇作品</a>
            </section>"#,
        title = escape(point.title.as_deref().unwrap_or("标题未知")),
        published = escape(&point.published_local_date),
        metric = metric_label(metric),
        value = point.metric_value,
        work = point.work_public_ref,
    )
}

fn lifecycle_state(title: &str, body: &str) -> String {
    format!(
        r#"<section class="c-dw-section life-panel"><div class="life-heading"><h2>作品表现</h2></div><div class="life-state"><b>{title}</b><p>{body}</p></div></section>"#,
        title = escape(title),
        body = escape(body),
    )
}

fn metric_label(metric: CreatorLifecycleMetric) -> &'static str {
    match metric {
        CreatorLifecycleMetric::Likes => "点赞",
        CreatorLifecycleMetric::Comments => "评论",
        CreatorLifecycleMetric::Collects => "收藏",
        CreatorLifecycleMetric::Shares => "转发",
    }
}

fn archive_gap_overview(
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
) -> String {
    if !is_creator {
        return String::new();
    }
    let directory = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory())
            .map(|value| value.works_listed.to_string())
            .unwrap_or_else(|| "—".to_owned()),
    };
    let detail = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory() && value.works_listed > 0)
            .map(|value| format!("{}/{}", value.details_captured, value.works_listed))
            .unwrap_or_else(|| "—".to_owned()),
    };
    let analyzable = match lifecycle {
        LifecycleView::Projection(projection) => {
            projection.summary.eligible_point_count.to_string()
        }
        LifecycleView::ReadUnavailable { .. }
        | LifecycleView::NotRead { .. }
        | LifecycleView::QueryInvalid => "当前读不到".to_owned(),
    };
    let mut gaps = match lifecycle {
        LifecycleView::Projection(projection) => lifecycle_exclusions(projection),
        _ => String::new(),
    };
    if let Some(count) = archive
        .value()
        .map(|value| value.quarantined)
        .filter(|count| *count > 0)
    {
        gaps.push_str(&format!(
            r#"<ul class="life-exclusions" aria-label="档案待处理项"><li>待处理记录 {count}</li></ul>"#
        ));
    }
    if let Some(count) = archive
        .value()
        .map(|value| value.blocked_details)
        .filter(|count| *count > 0)
    {
        gaps.push_str(&format!(
            r#"<ul class="life-exclusions" aria-label="档案待处理项"><li>详情读取受阻 {count} 条（已停止自动重试）</li></ul>"#
        ));
    }
    if matches!(archive, TargetArchiveRead::Unavailable) {
        gaps = r#"<p class="c-dw-note">档案状态暂时无法读取；这里不会把未知显示成零或“尚未建立”。</p>"#.to_owned();
    } else if gaps.is_empty() {
        gaps = r#"<p class="c-dw-note">当前读取没有给出额外缺口；这只描述已建立的作品目录，不代表平台全部作品。</p>"#.to_owned();
    }
    format!(
        r#"<section class="c-dw-section c-dw-archive-summary" id="target-archive">
              <div class="c-dw-section-head"><b>档案缺口</b><span>分别看，不合成总分</span></div>
              <div class="c-dw-readouts c-dw-archive-readouts">
                <div><b>{directory}</b><span>作品目录</span></div>
                <div><b>{detail}</b><span>详情进度</span></div>
                <div><b>{analyzable}</b><span>当前可分析</span></div>
              </div>
              {gaps}
            </section>"#,
    )
}

/// 档案把目录、详情、可分析和深层材料分开，不能把不同分母压成一个完成百分比。
fn archive_tab(
    target: &ObservationTarget,
    archive: TargetArchiveRead<'_>,
    is_creator: bool,
    lifecycle: LifecycleView<'_>,
    list_context: TargetListContext<'_>,
) -> String {
    if !is_creator {
        return lifecycle_state(
            "关键词不建立创作者档案",
            "关键词观察记录搜索命中与巡查变化，不扫描某个创作者的作品目录，也不显示创作者作品分布。",
        );
    }
    let directory = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory())
            .map(|value| value.works_listed.to_string())
            .unwrap_or_else(|| "—".to_owned()),
    };
    let detail = match archive {
        TargetArchiveRead::Unavailable => "当前读不到".to_owned(),
        TargetArchiveRead::Known(value) => value
            .filter(|value| value.has_displayable_directory() && value.works_listed > 0)
            .map(|value| format!("{}/{}", value.details_captured, value.works_listed))
            .unwrap_or_else(|| "—".to_owned()),
    };
    let analyzable = match lifecycle {
        LifecycleView::Projection(projection) => {
            projection.summary.eligible_point_count.to_string()
        }
        LifecycleView::ReadUnavailable { .. }
        | LifecycleView::NotRead { .. }
        | LifecycleView::QueryInvalid => "当前读不到".to_owned(),
    };
    let primary_action =
        target_primary_action(target, true, archive, KeywordArchiveRead::Unavailable);
    let problems = match (archive.value(), primary_action) {
        (Some(value), _) if value.has_actionable_problems() => {
            let quarantined = if value.quarantined > 0 {
                format!("<li>{} 条记录已被隔离，未计入作品目录或详情进度。</li>", value.quarantined)
            } else {
                String::new()
            };
            let blocked = if value.blocked_details > 0 {
                format!("<li>{} 条作品详情连续读取失败，已停止自动重试；它们不是页面不存在，也没有生成 Attempt、Package、Receipt 或详情。</li>", value.blocked_details)
            } else {
                String::new()
            };
            format!(
                r#"<div id="archive-problems" class="c-dw-problem" tabindex="-1"><b>档案有待处理项</b><ul>{quarantined}{blocked}</ul><p>其余材料可继续推进；处理这类问题必须发起一个新的、受控的采集决定。</p></div>"#
            )
        }
        (Some(value), TargetPrimaryAction::ViewArchiveProblems)
            if value.started || value.attempted =>
        {
            r#"<div id="archive-problems" class="c-dw-problem" tabindex="-1"><b>本次建档尚未形成可用基线</b><p>系统已经尝试建立档案，但当前作品目录或覆盖结果还不足以开启巡查；这里不会把已发起误写成已完成。</p></div>"#.to_owned()
        }
        (Some(value), TargetPrimaryAction::RebuildDirectory)
            if value.requires_directory_rebuild() =>
        {
            r#"<div id="archive-problems" class="c-dw-problem" tabindex="-1"><b>尚未形成受当前标准证明的目录边界</b><p>已有历史记录会保留；建立标准目录会获取主页作品链接（最多 200 篇；页面结束则按实际结束），再逐篇补齐详情。它不会删除或覆盖已有作品。</p></div>"#.to_owned()
        }
        _ => String::new(),
    };
    let history_boundary = match archive.value() {
        Some(value)
            if value.directory_baseline
                == linggan_evidence::ArchiveDirectoryBaseline::HistoricalDirectory =>
        {
            r#"<p class="c-dw-note">当前目录来自已接纳的历史采集记录。它准确说明本库已有多少作品与详情，不把这个数量写成平台总作品数；后续巡查接纳的新作品会进入同一目录。</p>"#
        }
        _ => "",
    };
    let action = match primary_action {
        TargetPrimaryAction::AssignDomain => {
            let href = list_context.domain_assignment_href(target.target_ref);
            format!(r#"<a class="c-btn-primary" href="{href}">分配领域</a>"#)
        }
        TargetPrimaryAction::ViewArchiveProgress => r#"<span class="c-dw-action-note">已有建档任务等待处理或执行中；本页不会重复提交。目录和详情只会随真实采集回执更新。</span>"#.to_owned(),
        TargetPrimaryAction::ViewArchiveProblems => r#"<span class="c-dw-action-note">当前先查看上面的档案待处理项；页面不会把隔离记录或读取受阻详情算成已完成。</span>"#.to_owned(),
        TargetPrimaryAction::ViewArchiveUnavailable => r#"<span class="c-dw-action-note">档案状态暂时无法读取，本页不会在未知状态下发起写操作。</span>"#.to_owned(),
        TargetPrimaryAction::EstablishArchive
        | TargetPrimaryAction::RebuildDirectory
        | TargetPrimaryAction::ContinueArchive => {
            let label = archive_action_label(primary_action);
            let gap_field = if primary_action == TargetPrimaryAction::ContinueArchive { r#"<input type="hidden" name="archive_action" value="gaps">"# } else { "" };
            let fields = list_context.return_fields(
                Some(target.target_ref),
                Some(TargetDrawerTab::Baseline),
                Some("target-archive"),
            );
            let domain_field = list_context.acquisition_domain_field();
            format!(
                r#"<form class="c-dw-primary-form" method="post" action="/collection/targets/archive">{fields}{domain_field}{gap_field}<button class="c-btn-primary" type="submit" name="row_target_ref" value="{target_ref}">{label}</button></form>"#,
                target_ref = target.target_ref,
            )
        }
        TargetPrimaryAction::OpenPatrol(_)
        | TargetPrimaryAction::ViewCreator
        | TargetPrimaryAction::ViewKeyword => {
            let href = list_context.drawer_href(
                target.target_ref,
                &[("dtab", "overview")],
                Some("creator-lifecycle"),
            );
            format!(r#"<a class="c-btn-secondary" href="{href}">查看作品分布</a>"#)
        }
    };
    format!(
        r#"<section id="target-archive" class="c-dw-section" tabindex="-1">
             <div class="c-dw-section-head"><b>作品档案</b><span>当前已取得范围</span></div>
             <div class="c-dw-readouts c-dw-archive-readouts">
               <div><b>{directory}</b><span>作品目录</span></div>
               <div><b>{detail}</b><span>详情进度</span></div>
               <div><b>{analyzable}</b><span>当前可分析</span></div>
             </div>
             <p class="c-dw-note">建立档案会读取创作者主页当前可见的前 200 篇作品链接作为上限，先建立去重目录，再逐步补齐详情与已授权的数据化处理。200 不是平台总作品数。</p>
             {history_boundary}
             <div class="c-dw-material-boundary"><b>评论与深层材料</b><p>评论正文、媒体、OCR 与 ASR 结果仍在语料页按具体作品查看，不在这里合成完成数字。</p></div>
             {problems}
             {action}
           </section>"#,
    )
}

fn archive_action_label(action: TargetPrimaryAction) -> &'static str {
    match action {
        TargetPrimaryAction::EstablishArchive => "建立档案",
        TargetPrimaryAction::RebuildDirectory => "处理异常",
        TargetPrimaryAction::ContinueArchive => "补采缺口",
        _ => unreachable!("only archive write actions have labels"),
    }
}

/// 巡查只展示当前可读的开关与时间，完整规则通过已有版本化规则入口管理。
/// 这个目标底下的巡检规则清单。
///
/// 一个关键词可以同时盯综合榜和点赞榜（`0076`），看不见就管不了——人无法知道自己配过
/// 几条、哪条还在跑。读不到时如实说读不到，不显示成「一条都没有」：后者会让人再配一条，
/// 而那会变成第二条同口径的规则。
fn monitor_rule_list(
    target: &ObservationTarget,
    rules: Option<&[linggan_evidence::MonitorRuleSummary]>,
    list_context: TargetListContext<'_>,
) -> String {
    let Some(rules) = rules else {
        return r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>巡检规则</b><span>读取暂不可用</span></div><p class="c-dw-note">当前读不到这个目标的规则。未知不表示没有规则，也不表示规则正常。</p></section>"#.to_owned();
    };
    if rules.is_empty() {
        return format!(
            r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>巡检规则</b><span>还没有</span></div><p class="c-dw-note">这个目标还没有巡检规则。建档是一次性的历史挖掘，巡检是此后每周看新增——两件事，需要各自的口径。</p>{add}</section>"#,
            add = add_rule_link(target, list_context),
        );
    }
    let mut rows = String::new();
    for rule in rules {
        let cadence = rule.interval_seconds.map_or_else(
            || "周期读不到".to_owned(),
            |seconds| format!("每 {} 小时", seconds / 3600),
        );
        let mut sampling = Vec::new();
        if let Some(rounds) = rule.scroll_rounds {
            sampling.push(format!("下拉 {rounds} 次"));
        }
        if let Some(top) = rule.top_by_likes {
            sampling.push(format!("取赞前 {top}"));
        }
        if let Some(days) = rule.published_within_days {
            sampling.push(format!("近 {days} 天"));
        }
        let sampling = if sampling.is_empty() {
            "未设取样上限".to_owned()
        } else {
            sampling.join(" · ")
        };
        let state = if rule.automatic_enabled {
            r#"<span class="c-tg-truth c-tg-ok">巡查中</span>"#
        } else {
            r#"<span class="c-tg-truth c-tg-neutral">已暂停</span>"#
        };
        let next = rule.next_run_at.as_deref().unwrap_or("未排定");
        let last = rule.last_succeeded_at.as_deref().unwrap_or("尚未成功巡查");
        let return_fields = list_context.return_fields(
            Some(target.target_ref),
            Some(TargetDrawerTab::Patrol),
            None,
        );
        // 编辑指定这一条的口径。此前规则台上只有「停用」，没有任何入口能编辑第二条
        // 规则——面板只按目标寻址，点开永远是最早那条。
        let edit_opener = format!("drawer-edit-rule-{}", rule.rule_ref);
        let edit = format!(
            r#"<a id="{edit_opener}" class="c-btn-quiet" data-monitor-rule-trigger="{target_ref}" href="{href}">编辑</a>"#,
            href = list_context.monitor_rule_slot_href(
                target.target_ref,
                Some(&rule.slot_key),
                &edit_opener
            ),
            target_ref = target.target_ref,
            edit_opener = escape(&edit_opener),
        );
        // **每条规则各自的暂停／启用。** 列表上那个开关讲的是整个目标（一起开关全部规则）；
        // 这里讲的是「这个目标按哪几个口径观察」——盯三个榜时想只停点赞那一条，只有这里能做。
        //
        // 版本号取**这条规则自己的**（`MonitorRuleSummary.revision`）：用别条的会永远撞
        // 「版本已过期」，而界面只说版本过期，看不出是拿错了谁的号。
        let (toggle_kind, toggle_label) = if rule.automatic_enabled {
            ("pause", "暂停")
        } else {
            ("resume", "启用")
        };
        let toggle = format!(
            r#"<form method="post" action="/collection/targets/rules">{return_fields}               <input type="hidden" name="target_ref" value="{target_ref}">               <input type="hidden" name="rule_slot" value="{slot_key}">               <input type="hidden" name="expected_revision" value="{revision}">               <input type="hidden" name="idempotency_key" value="{idempotency_key}">               <button class="c-btn-quiet" type="submit" name="command_kind" value="{toggle_kind}">{toggle_label}</button></form>"#,
            target_ref = target.target_ref,
            slot_key = escape(&rule.slot_key),
            revision = rule.revision,
            idempotency_key = uuid::Uuid::new_v4(),
        );
        // 最后一条规则不提供停用。**理由不再是数据库约束**——`0078` 删掉了「监控中必须有
        // 规则」那两条 CHECK（调度改成遍历规则，没有规则自然产不出到期项）。留着这个限制
        // 是产品判断：停掉最后一条规则，这个目标就在「还开着观察、却什么都不会跑」的状态
        // 上，界面上看不出差别。要停就停止观察这个目标，那是另一个决定，不该藏在这里。
        let retire = if rules.len() > 1 {
            format!(
                // `row_target_ref` 是 retire 那个 wire 的必填项，而 `return_fields` 只发
                // `return_*`。少了它整个表单 422——**这个「停用」按钮从来没生效过**，
                // 点下去只拿到一个 422，规则一动不动，页面也不说为什么。
                concat!(
                    r#"<form method="post" action="/collection/targets/monitor-rules/retire">{return_fields}"#,
                    r#"<input type="hidden" name="row_target_ref" value="{target_ref}"/>"#,
                    r#"<input type="hidden" name="rule_ref" value="{rule_ref}"/>"#,
                    r#"<button class="c-btn-quiet" type="submit">停用</button></form>"#,
                ),
                return_fields = return_fields,
                target_ref = target.target_ref,
                rule_ref = rule.rule_ref,
            )
        } else {
            String::new()
        };
        rows.push_str(&format!(
            r#"<tr><th scope="row">{slot}</th><td>{state}</td><td>{cadence}</td><td>{sampling}</td><td>{next}</td><td>{last}</td><td class="c-dw-rule-actions">{edit}{toggle}{retire}</td></tr>"#,
            slot = escape(&monitor_slot_label(&rule.slot_key, target.target_kind == "creator")),
            cadence = escape(&cadence),
            sampling = escape(&sampling),
            next = escape(next),
            last = escape(last),
        ));
    }
    // 表格而不是一串卡片：几条规则之间要比的就是「哪条什么周期、哪条停了、哪条下次什么时候
    // 跑」——同一列上下对齐才看得出差别，散着放要逐条读。
    format!(
        r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>巡检规则</b><span>{count} 条</span></div>
           <div class="c-dw-catalog-table-wrap c-dw-rule-table-wrap">
             <table class="c-dw-catalog-table c-dw-rule-table">
               <caption class="v7-sr-only">这个目标的巡检规则，一行一条口径</caption>
               <thead><tr><th scope="col">口径</th><th scope="col">状态</th><th scope="col">周期</th><th scope="col">取样</th><th scope="col">下次巡查</th><th scope="col">最近成功</th><th scope="col">操作</th></tr></thead>
               <tbody>{rows}</tbody>
             </table>
           </div>
           <p class="c-dw-note">一个关键词可以同时盯几个榜，各有各的周期。这里的暂停只停这一条；列表上那个开关一起开关全部规则。停用只是不再排期，它签发过的工单与材料仍然留着——删掉会让那些材料说不清是按什么口径取回来的。</p>{add}</section>"#,
        count = rules.len(),
        add = add_rule_link(target, list_context),
    )
}

fn monitor_slot_label(slot_key: &str, is_creator: bool) -> String {
    match slot_key {
        "comprehensive" => "综合排序".to_owned(),
        "latest" => "最新排序".to_owned(),
        "most_liked" => "最多点赞".to_owned(),
        "most_collected" => "最多收藏".to_owned(),
        "most_commented" => "最多评论".to_owned(),
        "primary" if is_creator => "主页目录".to_owned(),
        "primary" => "当前口径".to_owned(),
        other => other.to_owned(),
    }
}

/// 「加一条规则」。
///
/// **只对关键词提供**：口径是排序，博主没有排序可言——它只有一条看主页目录的规则，再加
/// 一条会落到同一个 `primary` 槽上，变成改那一条，而按钮写着「加」。
///
/// 走新建模式（`rule_slot=new`）。此前它与「管理巡查」指向同一个链接，于是点开拿到的是最早
/// 那条规则的表单、口径还是只读的——**第二条规则从界面上根本加不出来**。
fn add_rule_link(target: &ObservationTarget, list_context: TargetListContext<'_>) -> String {
    if target.target_kind != "keyword" {
        return String::new();
    }
    let opener_id = format!("drawer-add-rule-{}", target.target_ref);
    format!(
        r#"<a id="{opener_id}" class="c-btn-secondary" data-monitor-rule-trigger="{target_ref}" href="{href}">加一条规则</a>"#,
        href = list_context.monitor_rule_slot_href(target.target_ref, Some("new"), &opener_id),
        target_ref = target.target_ref,
    )
}

fn patrol_tab(
    target: &ObservationTarget,
    inspector: TargetInspectorView<'_>,
    rules: Option<&[linggan_evidence::MonitorRuleSummary]>,
    list_context: TargetListContext<'_>,
) -> String {
    let opener_id = format!("drawer-monitor-rule-{}", target.target_ref);
    let rule_href = list_context.monitor_rule_href(target.target_ref, &opener_id);
    let rule_list = monitor_rule_list(target, rules, list_context);
    if matches!(inspector, TargetInspectorView::ReadUnavailable) {
        return format!(
            r#"<section class="c-dw-section"><div class="c-dw-section-head"><b>巡查变化</b><span>读取暂不可用</span></div><div class="life-state"><b>当前读不到巡查状态</b><p>未知不表示没有巡查结果，也不表示规则正常。</p></div></section>{rule_list}"#
        );
    }
    if let TargetInspectorView::Projection(inspector) = inspector {
        let last = inspector
            .patrol
            .last_succeeded_at
            .as_deref()
            .unwrap_or("尚未取得成功结果");
        let next = inspector.patrol.next_run_at.as_deref().unwrap_or_else(|| {
            if inspector.patrol.state == TargetInspectorPatrolState::Disabled {
                "未开启"
            } else {
                "待排定"
            }
        });
        return format!(
            r#"<section class="c-dw-section c-dw-patrol">
                 <div class="c-dw-section-head"><b>巡查变化</b><span>按时间查看</span></div>
                 <ol class="c-dw-timeline">
                   <li><time>{next}</time><div><b>下次巡查</b><p>{state}</p></div></li>
                   <li><time>{last}</time><div><b>最近一次成功巡查</b><p>命中 {hits} · 新增 {new}</p></div></li>
                 </ol>
                 <p class="c-dw-note">当前只显示目标级成功结果与下一次安排；没有可核验的历史差分时，不生成运行日志或“零变化”。</p>
                 <a id="{opener_id}" class="c-btn-secondary" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">管理巡查</a>
               </section>{rule_list}"#,
            next = escape(next),
            state = inspector_patrol_copy(inspector.patrol.state),
            last = escape(last),
            hits = inspector_count_copy(inspector.patrol.latest_hits),
            new = inspector_count_copy(inspector.patrol.latest_new),
            target_ref = target.target_ref,
        );
    }
    format!(
        r#"<section class="c-dw-section">
             <div class="c-dw-section-head"><b>持续巡查</b><span>当前规则与时间</span></div>
             <div class="c-dw-readouts">
               <div><b>{enabled}</b><span>巡查</span></div>
               <div><b>{last}</b><span>上次巡查</span></div>
               <div><b>{next}</b><span>下次巡查</span></div>
               <div><b>尚未取得</b><span>最近结果</span></div>
             </div>
             <p class="c-dw-note">巡查发现的新作品和后续互动数据，在接纳后会进入同一作品目录与生命周期。当前还没有可显示的本轮新增和数据更新汇总。</p>
             <a id="{opener_id}" class="c-btn-secondary" data-monitor-rule-trigger="{target_ref}" href="{rule_href}">管理巡查</a>
           </section>{rule_list}"#,
        enabled = lifecycle_patrol_copy(target).1,
        last = escape(
            target
                .last_patrol_succeeded_at
                .as_deref()
                .unwrap_or("尚未巡查")
        ),
        next = escape(
            if target.monitoring_enabled && target.lifecycle_state == "monitoring" {
                target.next_patrol_at.as_deref().unwrap_or("待排定")
            } else if target.monitoring_enabled {
                "待状态修复"
            } else {
                "—"
            }
        ),
        target_ref = target.target_ref,
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

pub(super) fn percent_encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(byte as char);
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_metric_uses_chinese_units_without_losing_exact_value() {
        assert_eq!(compact_metric(9_439.0), "9,439");
        assert_eq!(compact_metric(1_234_567.0), "123.5万");
        assert_eq!(exact_metric(1_234_567.0), "1,234,567");
        assert_eq!(exact_metric(12_345.5), "12,345.5");
    }

    fn target(state: &str) -> ObservationTarget {
        ObservationTarget {
            target_ref: uuid::Uuid::new_v4(),
            platform: "xhs".to_owned(),
            target_kind: "creator".to_owned(),
            identity_key: "creator-lifecycle-test".to_owned(),
            display_name: Some("生命周期作者".to_owned()),
            identity_facts: None,
            source: "manual".to_owned(),
            lifecycle_state: state.to_owned(),
            first_stored_at: "2026-09-04T09:00:00+08".to_owned(),
            monitoring_enabled: state == "monitoring",
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
            domain_name: Some("ADHD".to_owned()),
        }
    }

    fn keyword_target() -> ObservationTarget {
        let mut target = target("monitoring");
        target.target_kind = "keyword".to_owned();
        target.identity_key = "考研自习".to_owned();
        target.display_name = Some("考研自习".to_owned());
        target
    }

    #[test]
    fn work_links_keep_the_work_and_an_explicit_domain_when_selected() {
        let work_ref = uuid::Uuid::from_u128(42);
        let work = linggan_evidence::CatalogWork {
            public_ref: work_ref,
            content_external_id: "xhs-note".to_owned(),
            title: Some("一篇作品".to_owned()),
            creator_display_name: Some("作者".to_owned()),
            match_position: None,
            published_at: None,
            source: CatalogSource::InitialArchive,
            detail_state: CatalogDetailState::Complete,
            execution_state: None,
            media_state: "—",
            comment_count: Some(2),
            last_captured_at: None,
        };
        let catalog = linggan_evidence::CreatorDirectoryProjection { works: vec![work] };
        let target = target("monitoring");
        let all = works_list(
            &target,
            TargetCatalogView::Creator(Some(&catalog)),
            None,
            None,
            TargetListContext {
                domain: Some("all"),
                ..TargetListContext::default()
            },
        );
        assert!(all.contains(&format!("href=\"/corpus/evidence?work={work_ref}\"")));
        let domain_ref = uuid::Uuid::from_u128(2);
        let scoped = works_list(
            &target,
            TargetCatalogView::Creator(Some(&catalog)),
            None,
            None,
            TargetListContext {
                domain: Some("00000000-0000-0000-0000-000000000002"),
                ..TargetListContext::default()
            },
        );
        assert!(scoped.contains(&format!(
            "href=\"/corpus/evidence?domain={domain_ref}&amp;work={work_ref}\""
        )));
    }

    #[test]
    fn dismissed_unassigned_targets_remain_read_only() {
        let mut creator = target("dismissed");
        creator.domain_name = None;
        assert_eq!(
            target_primary_action(
                &creator,
                true,
                TargetArchiveRead::Unavailable,
                KeywordArchiveRead::Unavailable,
            ),
            TargetPrimaryAction::ViewCreator
        );
        let mut inspector = keyword_projection(&creator);
        inspector.required_action = TargetInspectorAction::StartArchive;
        let creator_html = inspector_overview(
            &creator,
            true,
            &inspector,
            TargetArchiveRead::Known(None),
            KeywordArchiveRead::Unavailable,
            &[],
            TargetListContext::default(),
        );
        assert!(creator_html.contains("当前无需处理"));
        assert!(!creator_html.contains(r#"action="/collection/targets/archive""#));
        assert!(!creator_html.contains(">分配领域</a>"));

        let mut keyword = keyword_target();
        keyword.lifecycle_state = "dismissed".to_owned();
        keyword.domain_name = None;
        assert_eq!(
            target_primary_action(
                &keyword,
                false,
                TargetArchiveRead::Unavailable,
                KeywordArchiveRead::Unavailable,
            ),
            TargetPrimaryAction::ViewKeyword
        );
    }

    #[test]
    fn unassigned_creator_inspector_cannot_offer_archive_before_domain_assignment() {
        let mut creator = target("pending_decision");
        creator.domain_name = None;
        let mut inspector = keyword_projection(&creator);
        inspector.required_action = TargetInspectorAction::StartArchive;

        let html = inspector_overview(
            &creator,
            true,
            &inspector,
            TargetArchiveRead::Known(None),
            KeywordArchiveRead::Unavailable,
            &[],
            TargetListContext::default(),
        );

        assert!(html.contains("需要分配领域"));
        assert!(html.contains(">分配领域</a>"));
        assert!(!html.contains(r#"action="/collection/targets/archive""#));
    }

    fn rule(
        slot_key: &str,
        automatic_enabled: bool,
        revision: i32,
    ) -> linggan_evidence::MonitorRuleSummary {
        linggan_evidence::MonitorRuleSummary {
            rule_ref: uuid::Uuid::new_v4(),
            slot_key: slot_key.to_owned(),
            automatic_enabled,
            revision,
            interval_seconds: Some(86_400),
            scroll_rounds: Some(3),
            top_by_likes: Some(20),
            published_within_days: Some(7),
            next_run_at: Some("2026-09-14 08:00".to_owned()),
            last_succeeded_at: None,
        }
    }

    /// 关键词抽屉要用的那一小块检查器投影：只看巡查那一栏，其余字段与主操作无关。
    fn keyword_projection(target: &ObservationTarget) -> TargetInspectorProjection {
        use linggan_evidence::{
            TargetInspectorArchive, TargetInspectorCoverage, TargetInspectorDirectoryState,
            TargetInspectorExecution, TargetInspectorPatrol,
        };
        TargetInspectorProjection {
            target_ref: target.target_ref,
            target_kind: target.target_kind.clone(),
            as_of: "2026-09-14 08:00".to_owned(),
            archive: TargetInspectorArchive {
                state: TargetInspectorArchiveState::NotApplicable,
                directory_state: TargetInspectorDirectoryState::NotApplicable,
                started: false,
                attempted: false,
                author_profile_captures: TargetInspectorCount::Known(0),
            },
            execution: TargetInspectorExecution {
                state: TargetInspectorExecutionState::Idle,
                queued_work_orders: 0,
                awaiting_producer_tasks: 0,
                running_attempts: 0,
                blocked_tasks: 0,
            },
            patrol: TargetInspectorPatrol {
                state: TargetInspectorPatrolState::Normal,
                last_dispatched_at: None,
                last_succeeded_at: None,
                next_run_at: None,
                latest_hits: TargetInspectorCount::Known(0),
                latest_new: TargetInspectorCount::Known(0),
            },
            coverage: TargetInspectorCoverage {
                directory_works: TargetInspectorCount::Known(0),
                captured_details: TargetInspectorCount::Known(0),
                missing_details: TargetInspectorCount::Known(0),
                quarantined_records: TargetInspectorCount::Known(0),
                blocked_details: TargetInspectorCount::Known(0),
            },
            required_action: TargetInspectorAction::NoActionHealthy,
        }
    }

    /// 同一个关键词在抽屉和列表行上必须是同一个答案。
    ///
    /// 这里渲染的是抽屉本体，不是 `target_primary_action`：那个函数在修之前也是绿的，
    /// 拿它当断言守不住任何东西。缺陷在抽屉这一侧——它此前走检查器投影，而关键词的档案态
    /// 恒为 `NotApplicable`，于是每个关键词都得到「当前无需处理」，列表行写着「建立档案」
    /// 而抽屉里连按钮都没有。
    #[test]
    fn keyword_drawer_gives_the_same_primary_action_as_the_list_row() {
        let mut keyword = keyword_target();
        keyword.monitoring_enabled = false;
        keyword.lifecycle_state = "stored".to_owned();
        let projection = keyword_projection(&keyword);
        let render = |archive: KeywordArchiveRead| {
            inspector_overview(
                &keyword,
                false,
                &projection,
                // 建档完整度是创作者的东西，关键词在那个映射里本来就没有条目。
                TargetArchiveRead::Known(None),
                archive,
                &[],
                TargetListContext::default(),
            )
        };

        let not_archived = render(KeywordArchiveRead::NotArchived);
        assert!(not_archived.contains("需要建立档案"), "{not_archived}");
        assert!(
            not_archived.contains(">建立档案</button>"),
            "{not_archived}"
        );

        let pending = render(KeywordArchiveRead::DetailPending);
        assert!(pending.contains("有详情缺口需要补采"), "{pending}");
        assert!(pending.contains(">补采缺口</button>"), "{pending}");
        assert!(
            pending.contains(r#"name="archive_action" value="gaps""#),
            "{pending}"
        );

        let complete = render(KeywordArchiveRead::Complete);
        assert!(complete.contains("开始每周巡检"), "{complete}");

        // 建档态读不到时既不催也不进入巡查，与列表行走同一个兜底。
        let unknown = render(KeywordArchiveRead::Unavailable);
        assert!(unknown.contains("当前无法判断"), "{unknown}");
        assert!(!unknown.contains("设置巡查"), "{unknown}");
    }

    /// 检查器读失败只让巡查事实未知，不能把已经独立读到的关键词建档态一起压掉。
    #[test]
    fn keyword_drawer_keeps_its_primary_action_when_inspector_read_is_unavailable() {
        let mut keyword = keyword_target();
        keyword.monitoring_enabled = false;
        keyword.lifecycle_state = "stored".to_owned();
        let render = |archive: KeywordArchiveRead| {
            overview_tab(
                &[],
                &keyword,
                TargetArchiveRead::Known(None),
                false,
                TargetInspectorView::ReadUnavailable,
                LifecycleView::QueryInvalid,
                None,
                archive,
                TargetListContext::default(),
            )
        };

        let not_archived = render(KeywordArchiveRead::NotArchived);
        assert!(
            not_archived.contains("目标状态暂时读不到"),
            "{not_archived}"
        );
        assert!(
            not_archived.contains("当前无法判断巡查与最近结果"),
            "{not_archived}"
        );
        assert!(not_archived.contains("需要建立档案"), "{not_archived}");
        assert!(
            not_archived.contains(">建立档案</button>"),
            "{not_archived}"
        );

        let pending = render(KeywordArchiveRead::DetailPending);
        assert!(pending.contains("目标状态暂时读不到"), "{pending}");
        assert!(pending.contains("有详情缺口需要补采"), "{pending}");
        assert!(pending.contains(">补采缺口</button>"), "{pending}");
        assert!(
            pending.contains(r#"name="archive_action" value="gaps""#),
            "{pending}"
        );
    }

    /// **还没建过档的词，就算在巡查也仍然给「建立档案」。**
    ///
    /// 这一条此前钉的是反过来的一面：「巡查优先于催建档，哪怕建档态说还没建过」。那个
    /// 前提是「巡查是在它自己的底座上往前跑」——可这两件事可以同时为真：建档那一轮没
    /// 发出去，人已经把观察开关打开了。这时巡查按口径每周只取回前 N 条，历史那一整段
    /// 再也补不回来；主操作让位给「查看结果」之后，界面上写着「尚未建立」，却没有一个
    /// 入口能把它建起来。
    ///
    /// **巡查优先的是「档案已经建完」，不是「还没建过」。**
    #[test]
    fn a_patrolling_keyword_without_an_archive_is_still_offered_it() {
        let keyword = keyword_target();
        let projection = keyword_projection(&keyword);
        let html = inspector_overview(
            &keyword,
            false,
            &projection,
            TargetArchiveRead::Known(None),
            KeywordArchiveRead::NotArchived,
            &[],
            TargetListContext::default(),
        );

        assert!(html.contains("需要建立档案"), "{html}");
        assert!(html.contains(">建立档案</button>"), "{html}");
    }

    /// 底座建完的词才归巡查——这时抽屉不再问建档，它的下一步是看命中。
    #[test]
    fn a_patrolling_keyword_with_a_finished_archive_asks_for_nothing() {
        let keyword = keyword_target();
        let projection = keyword_projection(&keyword);
        let html = inspector_overview(
            &keyword,
            false,
            &projection,
            TargetArchiveRead::Known(None),
            KeywordArchiveRead::Complete,
            &[],
            TargetListContext::default(),
        );

        assert!(html.contains("当前无需处理"), "{html}");
        assert!(!html.contains(">建立档案</button>"), "{html}");
    }

    /// **规则台是一张表。** 几条规则之间要比的就是「哪条什么周期、哪条停了、哪条下次什么
    /// 时候跑」——同一列上下对齐才看得出差别，散着放要逐条读。
    #[test]
    fn the_rule_board_is_a_table_with_one_row_per_rule() {
        let target = keyword_target();
        let rules = [rule("comprehensive", true, 1), rule("most_liked", false, 2)];
        let html = monitor_rule_list(
            &target,
            Some(&rules),
            TargetListContext {
                filter: None,
                sort: None,
                domain: None,
            },
        );
        assert!(html.contains("<table"), "规则台要是表格");
        for column in [
            "口径",
            "状态",
            "周期",
            "取样",
            "下次巡查",
            "最近成功",
            "操作",
        ] {
            assert!(html.contains(column), "缺表头 {column}");
        }
        assert_eq!(
            html.matches("<tr><th scope=\"row\">").count(),
            2,
            "一行一条规则"
        );
    }

    /// **每条规则各自的暂停／启用按钮，必须带齐这个命令需要的字段。**
    ///
    /// 少一项整个表单 422，点下去规则一动不动、页面也不说为什么——`row_target_ref` 少在
    /// 「停用」上正是这样：那个按钮从来没生效过。
    #[test]
    fn each_rule_row_can_be_paused_or_resumed_on_its_own() {
        let target = keyword_target();
        let rules = [rule("comprehensive", true, 3), rule("most_liked", false, 1)];
        let html = monitor_rule_list(
            &target,
            Some(&rules),
            TargetListContext {
                filter: None,
                sort: None,
                domain: None,
            },
        );
        // **逐行核对，不用整页 contains。** 整页查「存在 expected_revision=3」与「存在
        // rule_slot=comprehensive」是两句互不相干的话：哪天把两行的版本号或口径错配了，
        // 两句仍然都成立，测试照样绿。要断言的是**同一行里**这两项对得上。
        let rows = html
            .split("<tr><th scope=\"row\">")
            .skip(1)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        for (label, slot, revision, toggle) in [
            ("综合排序", "comprehensive", 3, ("pause", "暂停")),
            ("最多点赞", "most_liked", 1, ("resume", "启用")),
        ] {
            let row = rows
                .iter()
                .find(|row| row.starts_with(label))
                .unwrap_or_else(|| panic!("{label} 那一行不在表里"));
            assert!(
                row.contains(&format!(r#"name="rule_slot" value="{slot}""#)),
                "{label} 那一行要说清作用在哪条口径上"
            );
            assert!(
                row.contains(&format!(r#"name="expected_revision" value="{revision}""#)),
                "{label} 那一行要报**自己的**版本号，用别条的会永远撞「版本已过期」"
            );
            assert!(
                row.contains(&format!(r#"value="{}">{}"#, toggle.0, toggle.1)),
                "{label} 那一行的按钮该是「{}」",
                toggle.1
            );
            // 点完要回到这张表，而不是被丢进规则编辑弹窗。
            assert!(row.contains(r#"name="return_dtab" value="patrol""#));
            // 停用要带 retire 那个 wire 的必填项，否则整个表单 422、点下去什么都不发生。
            assert!(
                row.contains(r#"name="row_target_ref""#),
                "{label} 那一行的停用缺 row_target_ref"
            );
        }
    }

    #[test]
    fn drawer_statusline_uses_durable_archive_progress() {
        let archiving_target = target("archiving");
        let untouched = statusline(&archiving_target, TargetArchiveRead::Known(None));
        assert!(untouched.contains("尚未建档"));
        assert!(!untouched.contains("建档中"));

        let in_progress = ArchiveCompleteness {
            work_in_progress: true,
            ..ArchiveCompleteness::default()
        };
        assert!(
            statusline(
                &archiving_target,
                TargetArchiveRead::Known(Some(&in_progress))
            )
            .contains("建档待处理")
        );

        let partial = ArchiveCompleteness {
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
        };
        assert!(
            statusline(&archiving_target, TargetArchiveRead::Known(Some(&partial)))
                .contains("档案待完善")
        );

        let mut established_target = target("archived");
        established_target.monitoring_enabled = false;
        let established = ArchiveCompleteness {
            details_captured: 12,
            // 三态必须自洽：12 篇全部取得，就没有待取得的了。
            pending_details: 0,
            ..partial
        };
        assert!(
            statusline(
                &established_target,
                TargetArchiveRead::Known(Some(&established))
            )
            .contains("档案已建立")
        );
        assert_eq!(
            target_primary_action(
                &archiving_target,
                true,
                TargetArchiveRead::Known(Some(&established)),
                KeywordArchiveRead::Unavailable
            ),
            TargetPrimaryAction::ViewArchiveProblems,
            "equal counters do not authorize patrol while the bounded baseline is still archiving"
        );
        assert!(
            statusline(&archiving_target, TargetArchiveRead::Unavailable)
                .contains("档案状态暂时无法读取")
        );
    }

    #[test]
    fn enabled_but_non_monitoring_target_is_never_presented_as_patrolling() {
        let mut corrupted_keyword = target("archiving");
        corrupted_keyword.target_kind = "keyword".to_owned();
        corrupted_keyword.monitoring_enabled = true;
        corrupted_keyword.next_patrol_at = Some("09-08 20:07".to_owned());

        assert_eq!(
            lifecycle_patrol_copy(&corrupted_keyword),
            ("warn", "状态异常，未调度")
        );
        assert!(
            statusline(&corrupted_keyword, TargetArchiveRead::Unavailable)
                .contains("状态异常，未调度")
        );
        assert!(!statusline(&corrupted_keyword, TargetArchiveRead::Unavailable).contains("巡查中"));
        let patrol = patrol_tab(
            &corrupted_keyword,
            TargetInspectorView::NotRead,
            None,
            TargetListContext::default(),
        );
        assert!(patrol.contains("待状态修复"));
        assert!(!patrol.contains("已开启"));
    }

    #[test]
    fn works_read_failure_does_not_repeat_an_archive_action_in_the_header() {
        let target = target("monitoring");
        let partial = ArchiveCompleteness {
            started: true,
            attempted: true,
            author_profile_captures: 1,
            works_listed: 31,
            details_captured: 27,
            directory_baseline: linggan_evidence::ArchiveDirectoryBaseline::RebuildRequired,
            ..ArchiveCompleteness::default()
        };
        let mut completeness = std::collections::HashMap::new();
        completeness.insert(target.identity_key.clone(), partial);
        let html = render(
            Some(&target),
            None,
            Some(&completeness),
            Some(&target.target_ref.to_string()),
            TargetDrawerTab::Baseline,
            LifecycleView::NotRead {
                window: CreatorLifecycleWindow::Recent90Days,
                metric: CreatorLifecycleMetric::Likes,
            },
            None,
            TargetListContext::default(),
        );

        assert!(!html.contains(">处理异常</button>"));
        assert!(html.contains("当前读不到可核验的作品记录"));
        assert!(html.contains("作品目录"));
        assert!(!html.contains("作品生命周期"));
        assert!(!html.contains(">31</b><span>作品目录"));
        assert!(!html.contains(">27/31</b><span>详情进度"));
    }

    #[test]
    fn drawer_href_encodes_each_query_component_and_fragment() {
        let target_ref = uuid::Uuid::from_u128(44);
        let href = TargetListContext::default().drawer_href(
            target_ref,
            &[("life_work", "\"><svg onload=alert(1)>")],
            Some("archive problems#1"),
        );
        assert!(href.contains("life_work=%22%3E%3Csvg%20onload%3Dalert%281%29%3E"));
        assert!(href.ends_with("#archive%20problems%231"));
        assert!(!href.contains("<svg"));
        assert!(!href.contains('"'));
    }
}
