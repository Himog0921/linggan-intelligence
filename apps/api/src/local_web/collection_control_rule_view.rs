// Target-scoped monitoring-rule read and render surface.
//
// This module owns no scheduling or admission decision. It reads the append-only active rule
// and durable command receipt written by `linggan_evidence`, then renders one server-owned
// modal outside the target list's batch form. A saved rule is never described as collection
// success; only a later Work/Lease/Task/Receipt chain may make that claim.
use linggan_contracts::PRODUCER_TASK_SPEC_VERSION;
use linggan_evidence::collection_control_schema_is_ready;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

const DEFAULT_INTERVAL_SECONDS: i32 = 86_400;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonitorRulePanelRead {
    Found(MonitorRulePanel),
    TargetNotFound,
    SchemaUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRulePanel {
    pub target_ref: Uuid,
    pub target_name: String,
    pub target_kind: String,
    /// 目标的身份键。关键词是 `{词}::{排序}`——新建规则时的排序默认值从它来，
    /// 否则一个身份写着「最多点赞」的目标会默认配上综合排序，界面与身份各说各话。
    pub identity_key: String,
    pub lifecycle_state: String,
    pub active_rule: Option<ActiveMonitorRule>,
    pub receipt: Option<MonitorRuleReceiptView>,
    /// 这个面板在编辑哪条口径。`None` 表示**新开一条**。
    ///
    /// 口径是规则的身份（`0076`）：同一个关键词盯综合榜和点赞榜是两条规则。面板一次只
    /// 编辑一条，所以必须说清是哪一条——此前它只按目标寻址，于是「加一条规则」点进来
    /// 拿到的是最早那条的表单，保存只会改那一条，**第二条永远加不出来**。
    pub editing_slot: Option<String>,
    /// 还没有在用规则的排序。只在新开一条时有意义：已经有规则的榜不重复提供，否则保存
    /// 会撞上那条已存在的规则、被判成过期版本，而人看不出为什么。
    pub available_rankings: Vec<String>,
}

/// 面板要打开哪一条规则。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRuleSelection<'a> {
    /// 不指定：最早建的那条在用规则。博主永远只有一条，这就是它；关键词上是「管理巡查」
    /// 那个入口的既有行为。
    CurrentRule,
    /// 指定口径。规则台上每条规则的「编辑」走这一支。
    Slot(&'a str),
    /// 新开一条。「加一条规则」走这一支。
    NewRule,
}

/// 关键词能选的五个榜。顺序即界面顺序。
pub const KEYWORD_RANKINGS: [(&str, &str); 5] = [
    ("most_liked", "最多点赞"),
    ("most_collected", "最多收藏"),
    ("most_commented", "最多评论"),
    ("latest", "最新"),
    ("comprehensive", "综合排序"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveMonitorRule {
    pub rule_revision_ref: Uuid,
    pub revision: i32,
    pub automatic_enabled: bool,
    pub creator_follow_details: bool,
    pub fixed_interval_seconds: Option<i32>,
    pub fallback_interval_seconds: i32,
    pub surface_key: String,
    pub ranking_key: Option<String>,
    /// 采样口径。只有关键词搜索面才有；创作者主页恒为 `None`。
    pub scroll_rounds: Option<i32>,
    pub top_by_likes: Option<i32>,
    pub published_within_days: Option<i32>,
    pub task_contract_version: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleReceiptView {
    pub receipt_ref: Uuid,
    pub command_kind: String,
    pub expected_revision: i32,
    pub outcome: String,
    pub reason_code: String,
    pub applied_rule_revision_ref: Option<Uuid>,
    pub recorded_at: String,
}

/// Raw strings are retained only in the in-memory failed form. The route handler is
/// responsible for parsing the closed set before it calls the evidence command owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFormState {
    pub expected_revision: i32,
    pub idempotency_key: Uuid,
    pub automatic_enabled: bool,
    pub creator_follow_details: bool,
    pub fixed_interval_seconds: String,
    pub surface_key: String,
    pub ranking_key: String,
    /// 采样口径，保持原始输入。解析与范围判定交给 handler，与间隔同一处置——
    /// 失败时把人填过的字面值原样送回表单，不要悄悄换成一个能通过校验的数。
    pub scroll_rounds: String,
    pub top_by_likes: String,
    pub published_within_days: String,
    pub task_contract_version: String,
}

impl MonitorRuleFormState {
    pub fn from_panel(panel: &MonitorRulePanel) -> Self {
        let default_surface = if panel.target_kind == "keyword" {
            "keyword_search"
        } else {
            "creator_profile"
        };
        // 新开一条时的默认排序：**先用还没被占用的那个榜**。已经有规则的榜再默认一次，
        // 保存就会撞上那条已存在的规则、被判成过期版本，而人看不出为什么。
        //
        // 都占满了（或指定了要编辑的那一条）才回落到目标身份里那个排序；身份里也读不出来
        // 才是综合排序。身份不再是口径的来源（`0076` 之后排序属于规则），但它仍是历史目标
        // 唯一能说明「当初按什么采的」的地方，作为回落比一律给综合排序更接近事实。
        let identity_ranking = if panel.target_kind == "keyword" {
            panel
                .identity_key
                .rsplit_once("::")
                .map_or("comprehensive", |(_, ranking)| ranking)
        } else {
            ""
        };
        let default_ranking: &str = if panel.target_kind == "keyword" {
            panel
                .editing_slot
                .as_deref()
                .or_else(|| panel.available_rankings.first().map(String::as_str))
                .unwrap_or(identity_ranking)
        } else {
            ""
        };
        // 关键词面的默认口径就是规格里那套统一采集动作：下拉 3 次、取点赞前 20。
        // 发布时间默认不限——写死一周会让本领域那条一直在采全部时间的关键词说假话。
        let (default_scroll, default_top) = if panel.target_kind == "keyword" {
            ("3", "20")
        } else {
            ("", "")
        };
        let Some(rule) = panel.active_rule.as_ref() else {
            return Self {
                expected_revision: 0,
                idempotency_key: Uuid::new_v4(),
                automatic_enabled: false,
                creator_follow_details: false,
                fixed_interval_seconds: DEFAULT_INTERVAL_SECONDS.to_string(),
                surface_key: default_surface.to_owned(),
                ranking_key: default_ranking.to_owned(),
                scroll_rounds: default_scroll.to_owned(),
                top_by_likes: default_top.to_owned(),
                published_within_days: String::new(),
                task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
            };
        };
        Self {
            expected_revision: rule.revision,
            idempotency_key: Uuid::new_v4(),
            // The current surface has one schedule meaning: an all-day fixed
            // interval.  A historical dynamic/window revision remains readable
            // in the immutable rule table, but opening it must not silently
            // retain a second scheduling language through hidden inputs.
            automatic_enabled: rule.automatic_enabled,
            creator_follow_details: rule.creator_follow_details,
            fixed_interval_seconds: rule
                .fixed_interval_seconds
                .unwrap_or(rule.fallback_interval_seconds)
                .to_string(),
            surface_key: rule.surface_key.clone(),
            ranking_key: rule.ranking_key.clone().unwrap_or_default(),
            // 空表示「没记录过口径」，不是 0。既有规则由 0046 补过，新规则从上面的默认来。
            scroll_rounds: rule
                .scroll_rounds
                .map(|v| v.to_string())
                .unwrap_or_default(),
            top_by_likes: rule.top_by_likes.map(|v| v.to_string()).unwrap_or_default(),
            published_within_days: rule
                .published_within_days
                .map(|v| v.to_string())
                .unwrap_or_default(),
            task_contract_version: rule.task_contract_version.clone(),
        }
    }

    /// A rejected/stale/conflicting durable command owns its old idempotency key. Rendering
    /// the retained values with a new key lets the person correct and submit a new command
    /// instead of converting that correction into an identity conflict.
    pub fn with_new_command_identity(mut self) -> Self {
        self.idempotency_key = Uuid::new_v4();
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFormError {
    pub code: &'static str,
    pub fields: Vec<MonitorRuleFieldError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRuleFieldError {
    pub field: &'static str,
    pub message: &'static str,
}

/// Read only fields safe for the local UI. The query deliberately never selects credential
/// hashes, account digests, raw account identity, Cookie, HTML, or platform error text.
pub async fn read_monitor_rule_panel(
    database: &Database,
    target_ref: Uuid,
    selection: MonitorRuleSelection<'_>,
    receipt_ref: Option<Uuid>,
) -> Result<MonitorRulePanelRead, sqlx::Error> {
    if !collection_control_schema_is_ready(database).await? {
        return Ok(MonitorRulePanelRead::SchemaUnavailable);
    }
    let target = sqlx::query(
        "SELECT target.target_ref,target.target_kind,target.identity_key, \
                COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key) AS target_name, \
                target.lifecycle_state \
         FROM collection_observation_target target WHERE target.target_ref=$1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?;
    let Some(target) = target else {
        return Ok(MonitorRulePanelRead::TargetNotFound);
    };
    // 在用规则的口径，按建立顺序。既用来定位要编辑的那一条，也用来算还剩哪些榜可选。
    let live_slots: Vec<(String, Option<Uuid>)> = sqlx::query_as(
        "SELECT slot_key,active_revision_ref FROM collection_monitor_rule \
         WHERE target_ref=$1 AND retired_at IS NULL ORDER BY created_at,rule_ref",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await?;
    let receipt = read_receipt(database, target_ref, receipt_ref).await?;
    // **只有命令返回页显式带回执时，才让回执决定面板选中的规则。**
    //
    // `read_receipt(None)` 会为普通打开的面板读取最近一条回执，供反馈区展示；这条历史回执
    // 不能覆盖 URL 的 `rule_slot=new`，否则「加一条规则」会退回到旧规则的只读编辑表单。
    // 命令提交后的跳转会显式带 `rule_receipt`，此时才用刚应用的版本定位刚保存的那条规则。
    let receipt_slot = match receipt_ref.and_then(|_| {
        receipt
            .as_ref()
            .and_then(|receipt| receipt.applied_rule_revision_ref)
    }) {
        Some(revision_ref) => {
            sqlx::query_scalar::<_, String>(
                "SELECT rule.slot_key FROM collection_monitor_rule rule \
             JOIN collection_monitor_rule_revision revision ON revision.rule_ref=rule.rule_ref \
             WHERE revision.rule_revision_ref=$1 AND rule.retired_at IS NULL",
            )
            .bind(revision_ref)
            .fetch_optional(database.pool())
            .await?
        }
        None => None,
    };
    let selection = match receipt_slot.as_deref() {
        Some(slot) => MonitorRuleSelection::Slot(slot),
        None => selection,
    };
    // 选中哪一条：不指定就是最早那条（博主永远只有一条，关键词上是「管理巡查」的既有
    // 行为）；指定口径就是那一条；新开一条则谁也不读。
    let selected = match selection {
        MonitorRuleSelection::NewRule => None,
        MonitorRuleSelection::CurrentRule => live_slots.first().cloned(),
        MonitorRuleSelection::Slot(slot) => live_slots
            .iter()
            .find(|(slot_key, _)| slot_key == slot)
            .cloned(),
    };
    // 指定的口径还没有规则时，仍然把它当作「要新开的那一条」预填进表单，而不是悄悄
    // 回落到别的规则——后者会让人以为改的是刚才点的那一条。
    let editing_slot = match (selection, selected.as_ref()) {
        (MonitorRuleSelection::NewRule, _) => None,
        (MonitorRuleSelection::Slot(slot), None) => Some(slot.to_owned()),
        (_, Some((slot_key, _))) => Some(slot_key.clone()),
        (MonitorRuleSelection::CurrentRule, None) => None,
    };
    let active_rule = match selected.as_ref().and_then(|(_, revision)| *revision) {
        Some(revision_ref) => read_active_rule(database, target_ref, revision_ref).await?,
        None => None,
    };
    let available_rankings = KEYWORD_RANKINGS
        .iter()
        .filter(|(value, _)| {
            !live_slots
                .iter()
                .any(|(slot_key, _)| slot_key.as_str() == *value)
        })
        .map(|(value, _)| (*value).to_owned())
        .collect::<Vec<_>>();
    Ok(MonitorRulePanelRead::Found(MonitorRulePanel {
        target_ref: target.try_get("target_ref")?,
        target_name: target.try_get("target_name")?,
        identity_key: target.try_get("identity_key")?,
        target_kind: target.try_get("target_kind")?,
        lifecycle_state: target.try_get("lifecycle_state")?,
        active_rule,
        receipt,
        editing_slot,
        available_rankings,
    }))
}

async fn read_active_rule(
    database: &Database,
    target_ref: Uuid,
    rule_ref: Uuid,
) -> Result<Option<ActiveMonitorRule>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT rule_revision_ref,revision,automatic_enabled,creator_follow_details,fixed_interval_seconds, \
                fallback_interval_seconds,surface_key,ranking_key, \
                scroll_rounds,top_by_likes,published_within_days, \
                task_contract_version,created_at::text AS created_at \
         FROM collection_monitor_rule_revision \
         WHERE target_ref=$1 AND rule_revision_ref=$2",
    )
    .bind(target_ref)
    .bind(rule_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(ActiveMonitorRule {
            rule_revision_ref: row.try_get("rule_revision_ref")?,
            revision: row.try_get("revision")?,
            automatic_enabled: row.try_get("automatic_enabled")?,
            creator_follow_details: row.try_get("creator_follow_details")?,
            fixed_interval_seconds: row.try_get("fixed_interval_seconds")?,
            fallback_interval_seconds: row.try_get("fallback_interval_seconds")?,
            surface_key: row.try_get("surface_key")?,
            ranking_key: row.try_get("ranking_key")?,
            scroll_rounds: row.try_get("scroll_rounds")?,
            top_by_likes: row.try_get("top_by_likes")?,
            published_within_days: row.try_get("published_within_days")?,
            task_contract_version: row.try_get("task_contract_version")?,
            created_at: row.try_get("created_at")?,
        })
    })
    .transpose()
}

async fn read_receipt(
    database: &Database,
    target_ref: Uuid,
    receipt_ref: Option<Uuid>,
) -> Result<Option<MonitorRuleReceiptView>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT command_receipt_ref,command_kind,expected_revision,outcome,reason_code, \
                applied_rule_revision_ref,recorded_at::text AS recorded_at \
         FROM collection_monitor_rule_command_receipt \
         WHERE target_ref=$1 AND ($2::uuid IS NULL OR command_receipt_ref=$2) \
         ORDER BY recorded_at DESC LIMIT 1",
    )
    .bind(target_ref)
    .bind(receipt_ref)
    .fetch_optional(database.pool())
    .await?;
    row.map(|row| {
        Ok(MonitorRuleReceiptView {
            receipt_ref: row.try_get("command_receipt_ref")?,
            command_kind: row.try_get("command_kind")?,
            expected_revision: row.try_get("expected_revision")?,
            outcome: row.try_get("outcome")?,
            reason_code: row.try_get("reason_code")?,
            applied_rule_revision_ref: row.try_get("applied_rule_revision_ref")?,
            recorded_at: row.try_get("recorded_at")?,
        })
    })
    .transpose()
}

#[cfg(test)]
mod form_contract_tests {
    use super::*;
    fn panel(kind: &str) -> MonitorRulePanel {
        MonitorRulePanel {
            target_ref: Uuid::nil(),
            target_name: "合成目标".into(),
            target_kind: kind.into(),
            identity_key: "合成关键词::most_liked".into(),
            lifecycle_state: "pending_decision".into(),
            active_rule: None,
            receipt: None,
            editing_slot: None,
            available_rankings: vec!["latest".into(), "comprehensive".into()],
        }
    }
    #[test]
    fn new_keyword_rule_prefers_an_unused_ranking_and_retains_bounded_sampling_defaults() {
        let form = MonitorRuleFormState::from_panel(&panel("keyword"));
        assert_eq!(form.expected_revision, 0);
        assert_eq!(form.surface_key, "keyword_search");
        assert_eq!(form.ranking_key, "latest");
        assert_eq!(form.scroll_rounds, "3");
        assert_eq!(form.top_by_likes, "20");
        assert_eq!(form.published_within_days, "");
    }
    #[test]
    fn creator_rule_does_not_gain_a_keyword_sampling_policy() {
        let form = MonitorRuleFormState::from_panel(&panel("creator"));
        assert_eq!(form.surface_key, "creator_profile");
        assert_eq!(form.ranking_key, "");
        assert_eq!(form.scroll_rounds, "");
        assert_eq!(form.top_by_likes, "");
    }
    #[test]
    fn rejected_command_rotates_identity_without_discarding_the_draft() {
        let mut form = MonitorRuleFormState::from_panel(&panel("keyword"));
        form.fixed_interval_seconds = "1".into();
        form.published_within_days = "3".into();
        let identity = form.idempotency_key;
        let corrected = form.with_new_command_identity();
        assert_ne!(identity, corrected.idempotency_key);
        assert_eq!(corrected.fixed_interval_seconds, "1");
        assert_eq!(corrected.published_within_days, "3");
    }
    #[test]
    fn editing_a_named_slot_keeps_that_rules_revision_and_ranking() {
        let mut p = panel("keyword");
        p.editing_slot = Some("most_liked".into());
        p.active_rule = Some(ActiveMonitorRule {
            rule_revision_ref: Uuid::nil(),
            revision: 7,
            automatic_enabled: true,
            creator_follow_details: false,
            fixed_interval_seconds: Some(21600),
            fallback_interval_seconds: 86400,
            surface_key: "keyword_search".into(),
            ranking_key: Some("most_liked".into()),
            scroll_rounds: Some(4),
            top_by_likes: Some(15),
            published_within_days: Some(3),
            task_contract_version: "synthetic-v1".into(),
            created_at: "2026-10-16 09:00".into(),
        });
        let f = MonitorRuleFormState::from_panel(&p);
        assert_eq!(f.expected_revision, 7);
        assert_eq!(f.ranking_key, "most_liked");
        assert_eq!(f.fixed_interval_seconds, "21600");
        assert_eq!(f.published_within_days, "3");
    }
}
