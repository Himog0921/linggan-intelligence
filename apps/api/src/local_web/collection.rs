//! Collection read projections shared by the JSON endpoints. Page rendering lives in apps/web.
pub mod collection_control_rule_view {
    include!("collection_control_rule_view.rs");
}
pub mod collection_control_surface_view {
    include!("collection_control_surface_view.rs");
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SchedulerState {
    Running,
    Stale,
    Unreadable,
}

#[derive(Clone, Copy)]
pub struct SurfaceState {
    /// 已登记但当前没有在岗插件的工位数。DESIGN-006：每个面至少有一个读数回答
    /// 「这里有没有出问题」，因为没人因为「有 30 台」而行动，只因为「3 台停了」而行动。
    pub vacant_stations: Option<i64>,
    /// 报到了却没归位的插件安装数。它们不会被派活。
    pub unclaimed_installations: Option<i64>,
    pub total_targets: Option<i64>,
    pub monitoring_targets: Option<i64>,
    pub archiving_targets: Option<i64>,
    pub scheduler_state: SchedulerState,
}
