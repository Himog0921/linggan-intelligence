// Target-scoped monitoring-rule read and render surface.
//
// This module owns no scheduling or admission decision. It reads the append-only active rule
// and durable command receipt written by `linggan_evidence`, then renders one server-owned
// modal outside the target list's batch form. A saved rule is never described as collection
// success; only a later Work/Lease/Task/Receipt chain may make that claim.
use super::super::target_drawer::TargetListContext;
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
            fixed_interval_seconds: rule
                .fixed_interval_seconds
                .unwrap_or(rule.fallback_interval_seconds)
                .to_string(),
            surface_key: rule.surface_key.clone(),
            ranking_key: rule.ranking_key.clone().unwrap_or_default(),
            // 空表示「没记录过口径」，不是 0。既有规则由 0046 补过，新规则从上面的默认来。
            scroll_rounds: rule.scroll_rounds.map(|v| v.to_string()).unwrap_or_default(),
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
    let receipt = read_receipt(database, target_ref, receipt_ref).await?;
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
        "SELECT rule_revision_ref,revision,automatic_enabled,fixed_interval_seconds, \
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
const COLLECTION_SCRIPT: &str = r#"<script src="/assets/collection-workspace.js"></script>"#;

#[cfg(test)]
pub fn attach_to_collection_document(document: &str, overlay: &str) -> String {
    if overlay.is_empty() {
        return document.to_owned();
    }
    let Some(script_at) = document.rfind(COLLECTION_SCRIPT) else {
        return document.to_owned();
    };
    format!(
        "{}{}{}",
        &document[..script_at],
        overlay,
        &document[script_at..]
    )
}

pub fn render_monitor_rule_modal(
    panel: &MonitorRulePanel,
    form: &MonitorRuleFormState,
    error: Option<&MonitorRuleFormError>,
    list_context: TargetListContext<'_>,
) -> String {
    let opener_id = format!("monitor-rule-{}", panel.target_ref);
    let return_url = list_context.list_href(None);
    let close_href = list_context.list_href(Some(&opener_id));
    let return_filter = list_context.filter.unwrap_or_default();
    let return_sort = list_context.sort.unwrap_or_default();
    let disabled = panel.lifecycle_state == "dismissed";
    let revision = panel.active_rule.as_ref().map_or_else(
        || "尚无规则".to_owned(),
        |rule| format!("REV {}", rule.revision),
    );
    format!(
        r#"<div class="c-rule-overlay" data-monitor-rule-overlay>
          <section id="c-monitor-rule" class="c-rule-modal" role="dialog" aria-modal="true"
                   aria-labelledby="c-rule-title" data-return-url="{return_url}"
                   data-return-focus="{opener_id}">
            <header class="c-rule-head">
              <div>
                <p class="c-rule-context">{kind} · {revision}</p>
                <h2 id="c-rule-title" tabindex="-1" data-monitor-rule-initial-focus>监控规则 · {name}</h2>
                <p>保存只写入一版规则，不代表采集已经开始。暂停只停止未来自动调度，人工观察仍可申请。</p>
              </div>
              <a class="c-btn-quiet c-rule-close" href="{close_href}" aria-label="关闭监控规则" data-monitor-rule-close>关闭</a>
            </header>
            {feedback}
            {dismissed}
            <form class="c-rule-form" method="post" action="/collection/targets/rules" data-monitor-rule-form data-readonly="{readonly}">
              <input type="hidden" name="target_ref" value="{target_ref}">
              <input type="hidden" name="expected_revision" value="{expected_revision}">
              <input type="hidden" name="idempotency_key" value="{idempotency_key}">
              <input type="hidden" name="surface_key" value="{surface_key}">
              {ranking_hidden}
              <input type="hidden" name="task_contract_version" value="{task_contract_version}">
              <input type="hidden" name="return_filter" value="{return_filter}">
              <input type="hidden" name="return_sort" value="{return_sort}">
              <div class="c-rule-body">
                {errors}
                <section class="c-rule-section" aria-labelledby="c-rule-cadence-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-cadence-title">自动观察</h3>
                    <span>Asia/Shanghai · 从启用时刻开始计时</span>
                  </div>
                  <label class="c-rule-switch">
                    <input type="checkbox" name="automatic_enabled" value="true"{automatic_checked}{disabled_attr}>
                    <span><b>按规则自动观察</b><small>关闭后不再产生未来定时工单；人工观察仍可申请。</small></span>
                  </label>
                  <div class="c-rule-grid">
                    {interval}
                  </div>
                  <p class="c-rule-hint">{cadence_hint}</p>
                </section>
                {sampling}
              </div>
              <footer class="c-rule-actions">
                <div class="c-rule-secondary-actions">
                  <button class="c-btn-quiet" type="submit" name="command_kind" value="manual_observe"{disabled_attr}>人工观察一次</button>
                  {pause_or_resume}
                </div>
                <button class="c-btn-primary" type="submit" name="command_kind" value="save_rule"{disabled_attr}>保存新版本</button>
              </footer>
            </form>
          </section>
        </div>"#,
        kind = if panel.target_kind == "keyword" {
            "关键词"
        } else {
            "创作者"
        },
        revision = escape(&revision),
        name = escape(&panel.target_name),
        return_url = return_url,
        close_href = close_href,
        opener_id = escape(&opener_id),
        target_ref = panel.target_ref,
        expected_revision = form.expected_revision,
        idempotency_key = form.idempotency_key,
        surface_key = escape(&form.surface_key),
        task_contract_version = escape(&form.task_contract_version),
        return_filter = escape(return_filter),
        return_sort = escape(return_sort),
        feedback = feedback_markup(panel.receipt.as_ref()),
        dismissed = dismissed_markup(disabled),
        errors = errors_markup(error),
        automatic_checked = checked(form.automatic_enabled),
        disabled_attr = if disabled { " disabled" } else { "" },
        readonly = disabled,
        interval = interval_select(
            "fixed_interval_seconds",
            "观察间隔（仅可选择下列 5 档）",
            &form.fixed_interval_seconds,
            disabled,
        ),
        pause_or_resume = pause_or_resume(panel, disabled),
        sampling = sampling_policy_section(panel, form, disabled),
        // 排序由 hidden 传回——**除了新开一条的时候**：那时采样口径那一节里是个同名的
        // `<select>`，再发一个 hidden 会让同一个字段提交两个值，服务端按先到的那个解析，
        // 于是人选的榜被静默丢掉。表单不带这一项则会把已有的排序清掉，所以两者必须恰好
        // 有一个在。
        ranking_hidden = if panel.target_kind == "keyword" && is_new_rule(panel) {
            String::new()
        } else {
            format!(
                r#"<input type="hidden" name="ranking_key" value="{}">"#,
                escape(&form.ranking_key)
            )
        },
        cadence_hint = if form.surface_key.trim() == "keyword_search" {
            "每次运行按下面的采样口径搜一轮。关键词观察不做作者资格核验——那是创作者档案的事。"
        } else {
            "每次运行固定执行：作者主页资格核验 + 最近 30 条作品观察。历史建档完整度不会阻断后续观察。"
        },
    )
}

pub fn render_monitor_rule_unavailable(
    target_ref: Uuid,
    state: MonitorRuleUnavailableState,
    list_context: TargetListContext<'_>,
) -> String {
    let opener_id = format!("monitor-rule-{target_ref}");
    let return_url = list_context.list_href(None);
    let close_href = list_context.list_href(Some(&opener_id));
    let (title, body, code) = match state {
        MonitorRuleUnavailableState::TargetNotFound => (
            "没有找到这个观察目标",
            "这个本机标识没有对应目标；它不证明平台上的对象不存在，也不证明目标曾被删除。",
            "TARGET_NOT_FOUND",
        ),
        MonitorRuleUnavailableState::SchemaUnavailable => (
            "监控规则当前不可读",
            "规则表尚未可用。这次没有保存任何规则，也没有创建采集工作。",
            "RULE_SCHEMA_UNAVAILABLE",
        ),
        MonitorRuleUnavailableState::ReadUnavailable => (
            "监控规则读取失败",
            "目标列表仍可读取，但这个目标的规则状态当前未知。刷新会重新读取，不会自动重试或创建工作。",
            "RULE_READ_UNAVAILABLE",
        ),
    };
    format!(
        r#"<div class="c-rule-overlay" data-monitor-rule-overlay>
          <section id="c-monitor-rule" class="c-rule-modal c-rule-modal-compact" role="dialog"
                   aria-modal="true" aria-labelledby="c-rule-title" data-return-url="{return_url}"
                   data-return-focus="{opener_id}">
            <header class="c-rule-head">
              <div>
                <p class="c-rule-context">{code}</p>
                <h2 id="c-rule-title" tabindex="-1" data-monitor-rule-initial-focus>{title}</h2>
                <p>{body}</p>
              </div>
              <a class="c-btn-quiet c-rule-close" href="{close_href}" aria-label="关闭监控规则" data-monitor-rule-close>关闭</a>
            </header>
          </section>
        </div>"#,
        return_url = return_url,
        opener_id = escape(&opener_id),
        close_href = close_href,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorRuleUnavailableState {
    TargetNotFound,
    SchemaUnavailable,
    ReadUnavailable,
}

fn feedback_markup(receipt: Option<&MonitorRuleReceiptView>) -> String {
    let Some(receipt) = receipt else {
        return String::new();
    };
    let (tone, title) = match receipt.outcome.as_str() {
        "applied" => ("c-rule-feedback-ok", "命令已应用"),
        "replay" => ("c-rule-feedback-info", "已返回原命令结果"),
        "stale_revision" => ("c-rule-feedback-warn", "规则版本已经变化"),
        "identity_conflict" => ("c-rule-feedback-bad", "命令标识发生冲突"),
        _ => ("c-rule-feedback-bad", "命令未应用"),
    };
    let applied = receipt.applied_rule_revision_ref.map_or_else(
        || "未产生规则版本".to_owned(),
        |value| format!("规则 {}", short_ref(value)),
    );
    format!(
        r#"<section class="c-rule-feedback {tone}" data-monitor-rule-receipt data-outcome="{outcome}">
             <div><b>{title}</b><span data-monitor-rule-outcome>{reason}</span></div>
             <p>{meaning}</p>
             <dl>
               <div><dt>回执</dt><dd>{receipt_ref}</dd></div>
               <div><dt>记录时间</dt><dd>{recorded_at}</dd></div>
               <div><dt>结果</dt><dd>{applied}</dd></div>
             </dl>
           </section>"#,
        outcome = escape(&receipt.outcome),
        reason = escape(&receipt.reason_code),
        meaning = escape(receipt_meaning(&receipt.outcome, &receipt.reason_code)),
        receipt_ref = receipt.receipt_ref,
        recorded_at = escape(&receipt.recorded_at),
        applied = escape(&applied),
    )
}

fn receipt_meaning(outcome: &str, reason: &str) -> &'static str {
    match (outcome, reason) {
        ("applied", "rule_saved") => "规则已保存。没有据此声称采集已开始或完成。",
        ("applied", "monitor_paused") => "未来自动调度已暂停；人工观察仍可申请。",
        ("applied", "monitor_resumed") => "自动调度已恢复；尚未据此声称已经派出任务。",
        ("applied", "monitor_stopped") => "目标已停止新增工作，既有历史仍保留。",
        ("applied", "manual_observe_created") => {
            "人工观察工单已入队，尚未被插件认领；这不是执行或采集回执。"
        }
        ("applied", "manual_observe_reused") => "已有工作覆盖这次人工观察，没有重复创建。",
        ("replay", _) => "相同命令已处理过；这里显示的是耐久重放结果。",
        ("stale_revision", _) => "页面基于旧版本提交，没有覆盖当前规则。请核对当前值后重试。",
        ("identity_conflict", _) => "同一命令标识携带了不同内容，没有覆盖原命令。",
        (_, "baseline_not_ready") => "这是旧版本记录的历史原因；当前规则不再以建档完整度作为自动观察门槛。",
        (_, "invalid_interval") => "观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。",
        (_, "invalid_schedule") => "当前版本只接受固定、全天的单一观察间隔。",
        (_, "target_not_requestable") => "这个目标当前不允许新增观察工作。",
        _ => "命令没有改变当前规则，也没有创建采集事实。",
    }
}

fn errors_markup(error: Option<&MonitorRuleFormError>) -> String {
    let Some(error) = error else {
        return String::new();
    };
    let items = error
        .fields
        .iter()
        .map(|field| {
            format!(
                r##"<li><a href="#{field}">{message}</a></li>"##,
                field = escape(field.field),
                message = escape(field.message),
            )
        })
        .collect::<String>();
    format!(
        r#"<div class="c-rule-errors" role="alert" data-monitor-rule-error data-error-code="{code}">
             <b>这次没有保存</b>
             <p>{summary}</p>
             <ul>{items}</ul>
           </div>"#,
        code = escape(error.code),
        summary = escape(error_summary(error.code)),
    )
}

fn error_summary(code: &str) -> &'static str {
    match code {
        "stale_revision" => "规则版本已变化；下面保留了你提交的值，没有覆盖当前版本。",
        "identity_conflict" => "命令标识冲突；原命令仍保留，下面的值尚未应用。",
        "baseline_not_ready" => "这是旧版本记录的历史原因；当前规则不再以建档完整度作为自动观察门槛。",
        "read_model_not_connected" => "本机规则读写当前不可用；没有写入规则或采集工作。",
        _ => "请修正标出的字段后再提交；当前规则没有改变。",
    }
}

fn dismissed_markup(disabled: bool) -> String {
    if !disabled {
        return String::new();
    }
    r#"<p class="c-rule-disabled" data-control-reason="target_not_requestable"><b>目标已停止</b>历史与规则仍可查看，但不能保存、恢复或发起新的观察。</p>"#.to_owned()
}

fn pause_or_resume(panel: &MonitorRulePanel, disabled: bool) -> String {
    let Some(rule) = panel.active_rule.as_ref() else {
        return String::new();
    };
    let (kind, label) = if rule.automatic_enabled {
        ("pause", "暂停未来自动调度")
    } else {
        ("resume", "恢复自动调度")
    };
    format!(
        r#"<button class="c-btn-quiet" type="submit" name="command_kind" value="{kind}"{disabled}>{label}</button>"#,
        disabled = if disabled { " disabled" } else { "" },
    )
}

/// 关键词观察的采样口径。
///
/// 这一节回答的是「这一轮的 20 篇是怎么来的」：按什么排序、翻了几次页、从中取几篇、
/// 只要多新的内容。没有它，采回来的数字无法复核——而复核不了的数字不该拿去比较。
///
/// 只对关键词目标渲染。创作者主页没有排序也没有「取前 N」可言，给它一组输入框，
/// 等于请人填一份不存在的事实。
fn sampling_policy_section(
    panel: &MonitorRulePanel,
    form: &MonitorRuleFormState,
    disabled: bool,
) -> String {
    if form.surface_key.trim() != "keyword_search" {
        return String::new();
    }
    let disabled_attr = if disabled { " disabled" } else { "" };
    // 平台只提供这四档发布时间。此前这里是个任意天数的输入框——填 3 天，平台只能给
    // 「一周内」，那是个做不到的承诺。**取值仍是天数**：存的是「我想要多新的内容」这个
    // 意图，天数是跨平台通用的表达；档位是各平台的执行细节，由插件按自己的能力兑现，
    // 并把实际生效的那一档回写进回执。
    const PUBLISH_WINDOWS: [(&str, &str); 4] =
        [("", "不限"), ("1", "一天内"), ("7", "一周内"), ("180", "半年内")];
    let current_window = form.published_within_days.trim();
    let mut publish_windows = PUBLISH_WINDOWS
        .iter()
        .map(|(value, text)| {
            format!(
                r#"<option value="{value}"{selected}>{text}</option>"#,
                selected = if current_window == *value {
                    " selected"
                } else {
                    ""
                },
            )
        })
        .collect::<String>();
    // 库里存着一个不在四档里的天数时，多给一个选项把它显示出来。
    // 否则浏览器会默认选中第一项「不限」，**打开规则再保存一次就把那个值悄悄改没了**——
    // 界面不该在人没动过某一项的情况下改掉它。
    if !current_window.is_empty()
        && !PUBLISH_WINDOWS
            .iter()
            .any(|(value, _)| *value == current_window)
    {
        publish_windows.push_str(&format!(
            r#"<option value="{value}" selected>保持当前 · {value} 天（非平台档位）</option>"#,
            value = escape(current_window),
        ));
    }
    // 排序在**新开一条**时可选，在**编辑已有规则**时只读。
    //
    // 可选：排序就是这条规则的身份（`0076`），一个关键词盯几个榜就是几条规则。此前这里
    // 一律只读，理由是「排序属于目标身份（`{词}::{排序}`）」——那是 `0076` 之前的设计，
    // 而它把整个多规则能力锁在了界面之外：「加一条规则」点进来也只能改最早那条。
    //
    // 只读：编辑一条已有规则时改它的排序，等于把这条规则搬到另一个榜上——它签发过的工单
    // 与采回来的材料会突然说不清是按什么口径取的。要换榜就新开一条，再停用旧的那条。
    let ranking_label = KEYWORD_RANKINGS
    .iter()
    .find(|(value, _)| *value == form.ranking_key.trim())
    .map_or_else(
        // 认不出的排序显示原文：那是一个机器标识而不是我们的描述性标签，
        // 把它藏起来会让人看不出这个目标到底在按什么采。
        || escape(form.ranking_key.trim()),
        |(_, text)| (*text).to_owned(),
    );
    let (ranking_field, ranking_hint) = if is_new_rule(panel) {
        // 只提供还没有规则的榜。已经有规则的那个再列出来，保存会撞上那条规则、被判成
        // 过期版本，而人只看到一句「版本已过期」，看不出是因为这个榜已经在盯了。
        let mut offered = KEYWORD_RANKINGS
            .iter()
            .filter(|(value, _)| {
                panel
                    .available_rankings
                    .iter()
                    .any(|available| available == value)
                    || *value == form.ranking_key.trim()
            })
            .map(|(value, text)| {
                format!(
                    r#"<option value="{value}"{selected}>{text}</option>"#,
                    value = escape(value),
                    text = escape(text),
                    selected = if *value == form.ranking_key.trim() {
                        " selected"
                    } else {
                        ""
                    },
                )
            })
            .collect::<String>();
        if offered.is_empty() {
            // 五个榜都已经有规则了。不给一个空下拉——那看起来像坏了。
            offered = format!(
                r#"<option value="{value}" selected>{label}</option>"#,
                value = escape(form.ranking_key.trim()),
                label = ranking_label,
            );
        }
        (
            format!(
                r#"<label for="ranking_key"><span>排序依据</span><select id="ranking_key" name="ranking_key"{disabled_attr}>{offered}</select></label>"#
            ),
            if panel.available_rankings.is_empty() {
                "这个关键词的五个榜都已经有规则了。要换口径，先停用其中一条。".to_owned()
            } else {
                "排序就是这条规则的身份：同一个关键词盯几个榜就是几条规则，各有各的周期。这里只列出还没有规则的榜。".to_owned()
            },
        )
    } else {
        (
            format!(r#"<label><span>排序依据</span><output>{ranking_label}</output></label>"#),
            "排序是这条规则的身份，不在编辑里改——改它等于把这条规则搬到另一个榜上，它采回来的材料会说不清是按什么口径取的。要按别的榜采，回规则台加一条。".to_owned(),
        )
    };
    format!(
        r#"<section class="c-rule-section" aria-labelledby="c-rule-sampling-title">
                  <div class="c-rule-section-head">
                    <h3 id="c-rule-sampling-title">采样口径</h3>
                    <span>决定每一轮怎么取样，随样本一起留痕</span>
                  </div>
                  <div class="c-rule-grid">
                    {ranking_field}
                    <label for="scroll_rounds"><span>下拉刷新次数</span><input id="scroll_rounds" name="scroll_rounds" type="number" min="0" max="20" value="{scroll_rounds}"{disabled_attr}></label>
                    <label for="top_by_likes"><span>取点赞前几篇</span><input id="top_by_likes" name="top_by_likes" type="number" min="1" max="200" value="{top_by_likes}"{disabled_attr}></label>
                    <label for="published_within_days"><span>只要多新的内容</span><select id="published_within_days" name="published_within_days"{disabled_attr}>{publish_windows}</select></label>
                  </div>
                  <p class="c-rule-hint">{ranking_hint}</p>
                  <p class="c-rule-hint">按下拉次数控制，不按条数控制——页面每次加载出多少条不由我们决定，只有「拉了几次」是能说准的事实。时间范围只有这四档，因为平台就只给这四档；选了之后，回执里记的是页面上<b>实际生效</b>的那一档，不是这里选的值。同一批样本的点赞数不可跨时间比较。综合排序掺入个性化推荐，采回来的是平台认为这个账号会喜欢的内容，不是这个领域客观最好的内容。</p>
                </section>"#,
        scroll_rounds = escape(&form.scroll_rounds),
        ranking_field = ranking_field,
        ranking_hint = ranking_hint,
        top_by_likes = escape(&form.top_by_likes),
    )
}

/// 这个面板是不是在新开一条规则。
///
/// 判据是「读到了一条在用规则没有」，不是「URL 上有没有带口径」：指定的口径还没有规则时
/// （规则台上的链接过期了，或者那条刚被停用），那也是新开一条。
fn is_new_rule(panel: &MonitorRulePanel) -> bool {
    panel.active_rule.is_none()
}

fn interval_select(name: &str, label: &str, selected: &str, disabled: bool) -> String {
    let options = [
        (21_600, "6 小时"),
        (43_200, "12 小时"),
        (86_400, "24 小时"),
        (172_800, "2 天"),
        (604_800, "7 天"),
    ]
    .iter()
    .map(|(value, text)| {
        format!(
            r#"<option value="{value}"{selected}>{text}</option>"#,
            selected = if selected == value.to_string() {
                " selected"
            } else {
                ""
            },
        )
    })
    .collect::<String>();
    format!(
        r#"<label for="{name}"><span>{label}</span><select id="{name}" name="{name}"{disabled}>{options}</select></label>"#,
        disabled = if disabled { " disabled" } else { "" },
    )
}

fn checked(value: bool) -> &'static str {
    if value { " checked" } else { "" }
}

fn short_ref(value: Uuid) -> String {
    value.to_string().chars().take(8).collect()
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

    fn panel() -> MonitorRulePanel {
        MonitorRulePanel {
            target_ref: Uuid::parse_str("d87b17da-f93c-42de-8051-8b21b2c1e90d").unwrap(),
            target_name: "边界测试 <创作者>".to_owned(),
            target_kind: "creator".to_owned(),
            identity_key: "creator-boundary".to_owned(),
            lifecycle_state: "archived".to_owned(),
            active_rule: Some(ActiveMonitorRule {
                rule_revision_ref: Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                revision: 3,
                automatic_enabled: true,
                fixed_interval_seconds: Some(86_400),
                fallback_interval_seconds: 86_400,
                surface_key: "creator_profile".to_owned(),
                ranking_key: None,
                scroll_rounds: None,
                top_by_likes: None,
                published_within_days: None,
                task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
                created_at: "2026-09-04 09:00:00+08".to_owned(),
            }),
            editing_slot: Some("primary".to_owned()),
            available_rankings: Vec::new(),
            receipt: Some(MonitorRuleReceiptView {
                receipt_ref: Uuid::parse_str("92dbfd51-9678-4781-a489-c00805576852").unwrap(),
                command_kind: "save_rule".to_owned(),
                expected_revision: 2,
                outcome: "applied".to_owned(),
                reason_code: "rule_saved".to_owned(),
                applied_rule_revision_ref: Some(
                    Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
                ),
                recorded_at: "2026-09-04 09:00:00+08".to_owned(),
            }),
        }
    }

    #[test]
    fn modal_is_a_sibling_form_with_stable_contract_selectors() {
        let panel = panel();
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = render_monitor_rule_modal(
            &panel,
            &form,
            None,
            TargetListContext {
                filter: Some("creator"),
                sort: Some("last"),
                domain: None,
            },
        );
        for marker in [
            "data-monitor-rule-overlay",
            "data-monitor-rule-form",
            "data-monitor-rule-receipt",
            "data-monitor-rule-outcome",
            "name=\"expected_revision\"",
            "name=\"idempotency_key\"",
            "name=\"return_filter\" value=\"creator\"",
            "name=\"return_sort\" value=\"last\"",
            "观察间隔（仅可选择下列 5 档）",
            "最近 30 条作品观察",
        ] {
            assert!(html.contains(marker), "missing {marker}");
        }
        assert!(html.contains("边界测试 &lt;创作者&gt;"));
        assert!(!html.contains("动态间隔"));
        assert!(!html.contains("采集成功"));
        assert!(!html.contains("Cookie"));
        assert!(!html.contains("account identity"));
    }

    #[test]
    fn rejected_form_retains_values_but_rotates_command_identity() {
        let panel = panel();
        let original = MonitorRuleFormState::from_panel(&panel);
        let original_key = original.idempotency_key;
        let mut retained = original.with_new_command_identity();
        retained.fixed_interval_seconds = "1".to_owned();
        let html = render_monitor_rule_modal(
            &panel,
            &retained,
            Some(&MonitorRuleFormError {
                code: "invalid_interval",
                fields: vec![MonitorRuleFieldError {
                    field: "fixed_interval_seconds",
                    message: "观察间隔只支持 6 小时、12 小时、24 小时、2 天或 7 天。",
                }],
            }),
            TargetListContext::default(),
        );
        assert_ne!(original_key, retained.idempotency_key);
        assert!(html.contains("data-error-code=\"invalid_interval\""));
        assert!(html.contains("href=\"#fixed_interval_seconds\""));
    }

    #[test]
    fn attach_places_modal_before_the_behavior_script_and_after_the_list_form() {
        let document = format!("<form id=\"list\"></form>{COLLECTION_SCRIPT}</body></html>");
        let attached = attach_to_collection_document(&document, "<aside>modal</aside>");
        let form_end = attached.find("</form>").unwrap();
        let modal = attached.find("<aside>modal</aside>").unwrap();
        let script = attached.find(COLLECTION_SCRIPT).unwrap();
        assert!(form_end < modal && modal < script);
    }

    #[test]
    fn unavailable_modal_keeps_unknown_distinct_from_empty_or_failure() {
        let html = render_monitor_rule_unavailable(
            Uuid::new_v4(),
            MonitorRuleUnavailableState::ReadUnavailable,
            TargetListContext::default(),
        );
        assert!(html.contains("规则状态当前未知"));
        assert!(!html.contains("没有监控规则"));
        assert!(!html.contains("采集失败"));
    }
}

#[cfg(test)]
mod sampling_policy_tests {
    use super::*;

    fn keyword_panel(identity_key: &str) -> MonitorRulePanel {
        MonitorRulePanel {
            target_ref: Uuid::nil(),
            target_name: "考研自习".to_owned(),
            target_kind: "keyword".to_owned(),
            identity_key: identity_key.to_owned(),
            lifecycle_state: "paused".to_owned(),
            active_rule: None,
            receipt: None,
            // 默认是「新开一条」：五个榜都还空着。
            editing_slot: None,
            available_rankings: KEYWORD_RANKINGS
                .iter()
                .map(|(value, _)| (*value).to_owned())
                .collect(),
        }
    }

    /// 已经有一条在用规则的关键词面板：编辑模式。
    fn keyword_panel_editing(identity_key: &str, slot: &str) -> MonitorRulePanel {
        let mut panel = keyword_panel(identity_key);
        panel.editing_slot = Some(slot.to_owned());
        panel.available_rankings.retain(|value| value != slot);
        panel.active_rule = Some(ActiveMonitorRule {
            rule_revision_ref: Uuid::parse_str("0da99cb2-0f4a-4da5-ac1b-c99e10bb64ed").unwrap(),
            revision: 2,
            automatic_enabled: true,
            fixed_interval_seconds: Some(86_400),
            fallback_interval_seconds: 86_400,
            surface_key: "keyword_search".to_owned(),
            ranking_key: Some(slot.to_owned()),
            scroll_rounds: Some(3),
            top_by_likes: Some(20),
            published_within_days: None,
            task_contract_version: PRODUCER_TASK_SPEC_VERSION.to_owned(),
            created_at: "2026-09-04 09:00:00+08".to_owned(),
        });
        panel
    }

    /// 身份写着「最多点赞」的目标，默认不该配上综合排序——那会让界面与身份各说各话。
    #[test]
    fn a_new_rule_defaults_to_the_ranking_in_the_target_identity() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::most_liked"));
        assert_eq!(form.ranking_key, "most_liked");
        // 规格里那套统一采集动作就是关键词面的默认口径。
        assert_eq!(form.scroll_rounds, "3");
        assert_eq!(form.top_by_likes, "20");
        // 发布时间默认不限：写死一周会让一直在采全部时间的关键词说假话。
        assert_eq!(form.published_within_days, "");
    }

    /// 创作者主页没有排序也没有取样口径可言，一项都不该预填。
    #[test]
    fn a_creator_rule_carries_no_sampling_policy() {
        let mut panel = keyword_panel("creator-1");
        panel.target_kind = "creator".to_owned();
        let form = MonitorRuleFormState::from_panel(&panel);
        assert_eq!(form.ranking_key, "");
        assert_eq!(form.scroll_rounds, "");
        assert_eq!(form.top_by_likes, "");
        assert!(sampling_policy_section(&panel, &form, false).is_empty());
    }

    /// 时间范围只给平台真正支持的四档：此前是个任意天数输入框，填 3 天平台只能给
    /// 「一周内」，那是个做不到的承诺。
    #[test]
    fn the_publish_window_offers_only_what_the_platform_supports() {
        let form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::latest"));
        let html = sampling_policy_section(&keyword_panel("考研自习::latest"), &form, false);
        for (value, text) in [("", "不限"), ("1", "一天内"), ("7", "一周内"), ("180", "半年内")] {
            assert!(html.contains(&format!(r#"<option value="{value}""#)));
            assert!(html.contains(text));
        }
        assert!(!html.contains(r#"name="published_within_days" type="number""#));
    }

    /// 库里存着非四档的天数时要显示出来。浏览器会默认选中第一项「不限」，
    /// 打开规则再保存一次就把那个值悄悄改没了——界面不该改掉人没动过的东西。
    #[test]
    fn a_non_standard_window_is_shown_instead_of_being_silently_dropped() {
        let mut form = MonitorRuleFormState::from_panel(&keyword_panel("考研自习::latest"));
        form.published_within_days = "3".to_owned();
        let html = sampling_policy_section(&keyword_panel("考研自习::latest"), &form, false);
        assert!(html.contains(r#"<option value="3" selected>"#));
        assert!(html.contains("非平台档位"));
        // 「不限」不该同时被选中。
        assert!(!html.contains(r#"<option value="" selected>"#));
    }

    /// 关键词面渲染出那一节：三项可编辑。
    #[test]
    fn the_keyword_surface_renders_every_sampling_input() {
        let panel = keyword_panel_editing("考研自习::latest", "latest");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = sampling_policy_section(&panel, &form, false);
        for field in ["scroll_rounds", "top_by_likes", "published_within_days"] {
            assert!(html.contains(&format!(r#"name="{field}""#)), "{field} 缺失");
        }
    }

    /// **新开一条时排序必须可选。**
    ///
    /// 这是「一个关键词盯几个榜」这件事在界面上唯一的入口。此前它一律只读（理由写的是
    /// 「排序属于目标身份」，那是 `0076` 之前的设计），于是存储层支持多规则、而界面上
    /// 第二条永远加不出来：「加一条规则」点进去拿到的是最早那条的表单，口径锁死，
    /// 保存只会改那一条。
    #[test]
    fn a_new_rule_lets_a_person_pick_the_ranking() {
        let panel = keyword_panel("考研自习");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = sampling_policy_section(&panel, &form, false);
        assert!(
            html.contains(r#"<select id="ranking_key" name="ranking_key""#),
            "新开一条规则必须能选榜，否则多规则从界面上到不了"
        );
        for (value, text) in KEYWORD_RANKINGS {
            assert!(html.contains(&format!(r#"value="{value}""#)), "{value} 缺失");
            assert!(html.contains(text), "{text} 缺失");
        }
        assert_eq!(form.expected_revision, 0, "新规则报的是它自己的第 0 版");
    }

    /// **已经有规则的榜不再提供。**
    ///
    /// 再提供一次，保存会撞上那条已存在的规则、被判成过期版本，而人只看到一句「版本已
    /// 过期」，看不出是因为这个榜已经在盯了。
    #[test]
    fn a_new_rule_only_offers_rankings_without_a_live_rule() {
        let mut panel = keyword_panel("考研自习");
        panel
            .available_rankings
            .retain(|value| value != "most_liked" && value != "comprehensive");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = sampling_policy_section(&panel, &form, false);
        assert!(!html.contains(r#"value="most_liked""#), "已有规则的榜不该再列");
        assert!(
            !html.contains(r#"value="comprehensive""#),
            "已有规则的榜不该再列"
        );
        assert!(html.contains(r#"value="most_collected""#));
        assert_eq!(
            form.ranking_key, "most_collected",
            "默认落在第一个还空着的榜上，而不是一个已经有规则的榜"
        );
    }

    /// **编辑已有规则时排序只读。**
    ///
    /// 改一条已有规则的排序，等于把它搬到另一个榜上——它签发过的工单与采回来的材料会
    /// 突然说不清是按什么口径取的。要换榜就新开一条，再停用旧的。
    #[test]
    fn editing_an_existing_rule_keeps_its_ranking_fixed() {
        let panel = keyword_panel_editing("考研自习::most_liked", "most_liked");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = sampling_policy_section(&panel, &form, false);
        assert!(!html.contains(r#"<select id="ranking_key""#));
        assert!(html.contains("<output>最多点赞</output>"));
        assert_eq!(form.expected_revision, 2, "报的是这条规则自己的版本号");
    }

    /// **新开一条时不能同时发一个同名的 hidden。**
    ///
    /// 同一个字段提交两个值，服务端按先到的那个解析，于是人选的榜被静默丢掉——表单看起来
    /// 正常工作，实际每次都存进同一条规则。
    #[test]
    fn the_new_rule_form_sends_the_ranking_exactly_once() {
        let panel = keyword_panel("考研自习");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = render_monitor_rule_modal(
            &panel,
            &form,
            None,
            TargetListContext {
                filter: None,
                sort: None,
                domain: None,
            },
        );
        assert_eq!(
            html.matches(r#"name="ranking_key""#).count(),
            1,
            "排序字段只能出现一次：新开一条时是 select，编辑时是 hidden"
        );
        assert!(html.contains(r#"<select id="ranking_key""#));
    }

    /// 编辑模式相反：排序只有 hidden 那一份。
    #[test]
    fn the_edit_form_sends_the_ranking_exactly_once_too() {
        let panel = keyword_panel_editing("考研自习::latest", "latest");
        let form = MonitorRuleFormState::from_panel(&panel);
        let html = render_monitor_rule_modal(
            &panel,
            &form,
            None,
            TargetListContext {
                filter: None,
                sort: None,
                domain: None,
            },
        );
        assert_eq!(html.matches(r#"name="ranking_key""#).count(), 1);
        assert!(html.contains(r#"<input type="hidden" name="ranking_key" value="latest">"#));
    }
}
