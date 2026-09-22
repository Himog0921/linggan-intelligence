//! COLLECTION-001 · Observation target persistence.
//!
//! Storing a target proves it was saved. It is not an Acquisition Request, not an
//! Authorization, and not a claim that any capture will run (INV-36). Nothing in this module
//! reaches a platform, and nothing here creates a Work Order.

use linggan_contracts::{
    CollectionContractError, LifecycleState, TargetIdentity, TargetKind, TargetSource,
};
use linggan_storage_postgres::Database;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum CollectionTargetError {
    #[error("collection target schema is not applied")]
    SchemaUnavailable,
    #[error(transparent)]
    Contract(#[from] CollectionContractError),
    #[error("that lifecycle move is not allowed: {from} -> {to}")]
    IllegalTransition { from: String, to: String },
    #[error("no observation target with that reference")]
    UnknownTarget,
    #[error("no active observation domain with that reference")]
    UnknownDomain,
    #[error("that observation target already belongs to another domain")]
    DomainAlreadyAssigned,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A stored target as the read side sees it.
#[derive(Debug, Clone)]
pub struct ObservationTarget {
    pub target_ref: Uuid,
    pub platform: String,
    pub target_kind: String,
    pub identity_key: String,
    pub display_name: Option<String>,
    pub identity_facts: Option<Value>,
    pub source: String,
    pub lifecycle_state: String,
    pub first_stored_at: String,
    /// 巡检是否开着。与 `lifecycle_state` 分开：一个目标可以「已建档」但被人暂停巡检，
    /// 压成一个状态就分不清「没在跑」是因为暂停还是因为还没建档。
    pub monitoring_enabled: bool,
    /// 人给的分组名。为空表示未分组——那是正常状态，不是缺失。
    pub group_name: Option<String>,
    /// 上一次真的派出巡检的时间。没派过就是 `None`，不是「很久以前」。
    pub last_patrol_dispatched_at: Option<String>,
    /// 上一次已经产生并接纳可用结果的巡查时间。用户看到的「上次巡查」只能用
    /// 这个字段，不能拿上面的派出时间冒充成功结果。
    pub last_patrol_succeeded_at: Option<String>,
    /// 下一次到期时间是调度器与规则命令共同维护的真实计划点，而不是页面从上次派出
    /// 时间反推的猜测。**巡检没开时不读**——算一个永远不会到来的时间，会让人以为它排上队了。
    pub next_patrol_at: Option<String>,
    /// 这个目标在观察哪个领域。
    ///
    /// `None` 表示目标尚未明确分配领域，或领域表尚未建立。页面必须保持这个未知边界，
    /// 不能把空值显示成本领域；写入侧会在明确分配前拒绝采集。
    pub domain_name: Option<String>,
    /// 该领域是不是本领域。用于列表里给外部领域加标记；`None` 同上。
    pub domain_is_own: Option<bool>,
}

/// Whether a store call created a target or found the one already there.
///
/// The distinction is kept out of the caller's reach as a boolean deliberately: a push that
/// lands on an existing target is a normal outcome, not a failure, but the page must be able
/// to say which happened rather than silently implying a new target was created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreOutcome {
    Stored,
    AlreadyPresent,
}

/// Result of the explicit person-owned step that turns a plugin-discovered candidate into a
/// domain-scoped observation target. Repeating the same assignment is safe; changing an existing
/// assignment is a different product decision and is deliberately rejected here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetDomainAssignmentOutcome {
    Assigned,
    AlreadyAssigned,
}

pub async fn assign_target_domain(
    database: &Database,
    target_ref: Uuid,
    domain_ref: Uuid,
) -> Result<TargetDomainAssignmentOutcome, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await?
        || !crate::observation_domain::observation_domain_schema_is_ready(database).await?
    {
        return Err(CollectionTargetError::SchemaUnavailable);
    }
    let mut tx = database.pool().begin().await?;
    let current: Option<Option<Uuid>> = sqlx::query_scalar(
        "SELECT domain_ref FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(current) = current else {
        return Err(CollectionTargetError::UnknownTarget);
    };
    let domain_is_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')",
    )
    .bind(domain_ref)
    .fetch_one(&mut *tx)
    .await?;
    if !domain_is_active {
        return Err(CollectionTargetError::UnknownDomain);
    }
    match current {
        Some(current) if current == domain_ref => {
            tx.commit().await?;
            Ok(TargetDomainAssignmentOutcome::AlreadyAssigned)
        }
        Some(_) => Err(CollectionTargetError::DomainAlreadyAssigned),
        None => {
            sqlx::query(
                "UPDATE collection_observation_target SET domain_ref=$2 WHERE target_ref=$1",
            )
            .bind(target_ref)
            .bind(domain_ref)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(TargetDomainAssignmentOutcome::Assigned)
        }
    }
}

/// Presentation eligibility for a creator avatar. A target may retain its observed source URL
/// in `identity_facts`, but ordinary UI may only render a qualified local materialization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationTargetAvatar {
    Local { local_asset_path: String },
    Pending,
    Unavailable,
    NotObserved,
}

/// Resolve creator-avatar state in one read for a target list. Detail-context and standalone
/// profile avatars share the same canonical author relationship and local asset lifecycle.
pub async fn read_target_avatars(
    database: &Database,
    targets: &[ObservationTarget],
) -> Result<HashMap<Uuid, ObservationTargetAvatar>, CollectionTargetError> {
    let creator_refs = targets
        .iter()
        .filter(|target| target.target_kind == "creator")
        .map(|target| target.target_ref)
        .collect::<Vec<_>>();
    if creator_refs.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = sqlx::query_as::<_, (Uuid, bool, Option<String>, Option<String>)>(
        "SELECT target.target_ref, \
                EXISTS (SELECT 1 FROM linggan_media_resource_relation relation \
                        WHERE relation.platform=target.platform AND relation.subject_kind='author' \
                          AND relation.subject_external_id=target.identity_key \
                          AND relation.relationship_kind='author.avatar') AS observed, \
                asset.local_asset_path, work.state \
         FROM collection_observation_target target \
         LEFT JOIN LATERAL ( \
             SELECT materialization.local_asset_path \
             FROM linggan_media_resource_relation relation \
             JOIN linggan_media_observation observation ON observation.slot_key=relation.slot_key \
             JOIN linggan_media_download_attempt attempt ON attempt.media_observation_ref=observation.observation_ref \
             JOIN linggan_media_materialization materialization USING(download_attempt_ref) \
             JOIN linggan_media_blob blob ON blob.sha256=materialization.blob_sha256 \
             WHERE relation.platform=target.platform AND relation.subject_kind='author' \
               AND relation.subject_external_id=target.identity_key \
               AND relation.relationship_kind='author.avatar' AND blob.mime_type LIKE 'image/%' \
               AND NOT EXISTS (SELECT 1 FROM linggan_current_material_media_disposition disposition \
                               WHERE disposition.slot_key=relation.slot_key \
                                  OR disposition.blob_sha256=materialization.blob_sha256 \
                                  OR disposition.materialization_ref=materialization.materialization_ref) \
             ORDER BY materialization.verified_at DESC LIMIT 1 \
         ) asset ON true \
         LEFT JOIN LATERAL ( \
             SELECT acquisition.state \
             FROM linggan_media_resource_relation relation \
             JOIN linggan_media_observation observation ON observation.slot_key=relation.slot_key \
             LEFT JOIN linggan_media_acquisition_work acquisition USING(observation_ref) \
             WHERE relation.platform=target.platform AND relation.subject_kind='author' \
               AND relation.subject_external_id=target.identity_key \
               AND relation.relationship_kind='author.avatar' \
             ORDER BY acquisition.updated_at DESC NULLS LAST LIMIT 1 \
         ) work ON true \
         WHERE target.target_ref = ANY($1)",
    )
    .bind(creator_refs)
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(target_ref, observed, local_asset_path, work_state)| {
            let state = match local_asset_path {
                Some(local_asset_path) => ObservationTargetAvatar::Local { local_asset_path },
                None if !observed => ObservationTargetAvatar::NotObserved,
                // An acquisition that completed without a renderable image (or exhausted its
                // retries) is not still “materializing”.  Keep it visibly unavailable rather
                // than promising progress that no worker can make.
                None if matches!(work_state.as_deref(), Some("completed" | "terminal")) => {
                    ObservationTargetAvatar::Unavailable
                }
                None => ObservationTargetAvatar::Pending,
            };
            (target_ref, state)
        })
        .collect())
}

pub async fn collection_target_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar::<_, bool>(
        "SELECT to_regclass('collection_observation_target') IS NOT NULL \
             AND to_regclass('collection_observation_target_transition') IS NOT NULL",
    )
    .fetch_one(database.pool())
    .await
}

/// Store a target in `pending_decision`, or return the one that is already there.
///
/// Deduplication is enforced by the unique index on (platform, kind, identity_key), not by a
/// read-then-write in application code: two pushes arriving together must not both succeed.
/// The legacy workbench has no such index, and its failure mode is a silent parallel duplicate
/// with its own independent baseline.
/// `domain` 是这个目标要观察的领域。`None` 表示插件只发现了目标身份，领域仍待人决定；
/// 它只出现在“全部领域”视图，且在明确分配前不能开始采集。
pub async fn store_pending_target(
    database: &Database,
    identity: &TargetIdentity,
    source: TargetSource,
    display_name: Option<&str>,
    identity_facts: Option<&Value>,
    domain: Option<Uuid>,
) -> Result<(ObservationTarget, StoreOutcome), CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }

    let target_ref = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, identity_facts, source, domain_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (platform, target_kind, identity_key) DO NOTHING \
         RETURNING target_ref",
    )
    .bind(target_ref)
    .bind(identity.platform())
    .bind(identity.kind().as_str())
    .bind(identity.key())
    .bind(display_name)
    .bind(identity_facts)
    .bind(source.as_str())
    .bind(domain)
    .fetch_optional(database.pool())
    .await?;

    let stored = read_target_by_identity(database, identity).await?;

    if inserted.is_some() {
        record_transition(
            database,
            stored.target_ref,
            None,
            LifecycleState::PendingDecision,
            "person",
            "target_stored",
            None,
        )
        .await?;
        Ok((stored, StoreOutcome::Stored))
    } else {
        Ok((stored, StoreOutcome::AlreadyPresent))
    }
}

async fn read_target_by_identity(
    database: &Database,
    identity: &TargetIdentity,
) -> Result<ObservationTarget, CollectionTargetError> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target \
         WHERE platform = $1 AND target_kind = $2 AND identity_key = $3",
    )
    .bind(identity.platform())
    .bind(identity.kind().as_str())
    .bind(identity.key())
    .fetch_optional(database.pool())
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;
    Ok(row.into())
}

/// Targets in one lifecycle state, newest first.
/// 各筛选维度下的来源数量。
///
/// 放进 tab 标签里（内容工作台的做法：`全部来源 88 / 博主 84 / 关键词 4`），而不是在
/// 列表上方再摆一排带计数的按钮——那会让同一组筛选在一屏里出现两遍。
#[derive(Debug, Default, Clone)]
pub struct TargetCounts {
    pub total: i64,
    pub creator: i64,
    pub keyword: i64,
    pub archiving: i64,
    /// 巡检开着的来源数。**按开关数，不按生命周期数**——一个目标可以「已建档」但被人
    /// 暂停巡检，用生命周期数会把它算成在巡检。
    pub monitoring: i64,
}

// `ObservationTarget` has one list-shaped read contract, regardless of whether it is being
// rendered as a row or opened through a deep-linked drawer.  Keep the tuple projection here
// rather than letting the two routes spell their columns independently: adding a field to one
// path used to make a valid target look unreadable only in the drawer.
//
// The domain table is intentionally split into two static projections. PostgreSQL resolves
// table references while planning a statement, so a left join cannot safely stand in for a
// migration that has not been applied yet.
/// 列表一行要读的列。
///
/// 「下次巡查」「上次派发」「巡检开着没有」都是**对这个目标所有在用规则的聚合**：
/// 下次取最早到期的那条，上次取最近派发过的那条，开关是「有没有一条自动巡检开着」。
/// 此前它们读的是目标行上那份副本，而调度只推进规则行——下一次巡检排出去之后两处分叉，
/// 界面上会永远停在一个越来越旧的过去时间。副本已在 `0078` 删除，这里只剩一个真相。
///
/// `bool_or` 在**一条规则都没有**时返回 NULL 而不是 false，所以外面套了一层 `COALESCE`：
/// 「自动巡查」这一列在 Rust 侧是非空布尔，读到 NULL 不是显示成假，是整条查询解码失败。
/// `0078` 删掉了「监控中必须有规则」那两条 CHECK（调度遍历规则，没有规则自然不产到期项），
/// 于是这个状态在数据库层面变成合法的了——读取侧必须自己接得住。
macro_rules! listed_target_columns {
    () => {
        "target.target_ref, target.platform, target.target_kind, target.identity_key, \
         target.display_name, target.identity_facts, target.source, target.lifecycle_state, \
         target.first_stored_at::text, \
         (target.monitoring_enabled AND rules.automatic_any), \
         target.group_name, \
         linggan_human_moment(rules.last_dispatched_at), \
         linggan_human_moment(target.last_patrol_succeeded_at), \
         CASE WHEN target.monitoring_enabled AND rules.automatic_any \
              THEN linggan_human_moment(rules.next_run_at) END"
    };
}

// Keep every row reader on the same static column contract. SQLx intentionally
// rejects runtime-built SQL, and the table-presence compatibility branch needs
// static statements because PostgreSQL resolves an absent table at plan time.
const READ_TARGET_WITH_DOMAIN: &str = concat!(
    "SELECT ",
    listed_target_columns!(),
    ", domain.name, domain.is_own_domain \
     FROM collection_observation_target target \
     LEFT JOIN LATERAL ( \
       SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) \
                AS automatic_any, \
              min(rule.monitor_next_run_at) AS next_run_at, \
              max(rule.last_patrol_dispatched_at) AS last_dispatched_at \
       FROM collection_monitor_rule rule \
       LEFT JOIN collection_monitor_rule_revision revision \
              ON revision.rule_revision_ref=rule.active_revision_ref \
       WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
     LEFT JOIN observation_domain domain ON domain.domain_ref = target.domain_ref \
     WHERE target.target_ref = $1"
);
const READ_TARGET_WITHOUT_DOMAIN: &str = concat!(
    "SELECT ",
    listed_target_columns!(),
    ", NULL::text, NULL::boolean \
     FROM collection_observation_target target \
     LEFT JOIN LATERAL ( \
       SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) \
                AS automatic_any, \
              min(rule.monitor_next_run_at) AS next_run_at, \
              max(rule.last_patrol_dispatched_at) AS last_dispatched_at \
       FROM collection_monitor_rule rule \
       LEFT JOIN collection_monitor_rule_revision revision \
              ON revision.rule_revision_ref=rule.active_revision_ref \
       WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
     WHERE target.target_ref = $1"
);
const LIST_TARGETS_WITH_DOMAIN: &str = concat!(
    "SELECT ",
    listed_target_columns!(),
    ", domain.name, domain.is_own_domain \
     FROM collection_observation_target target \
     LEFT JOIN LATERAL ( \
       SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) \
                AS automatic_any, \
              min(rule.monitor_next_run_at) AS next_run_at, \
              max(rule.last_patrol_dispatched_at) AS last_dispatched_at \
       FROM collection_monitor_rule rule \
       LEFT JOIN collection_monitor_rule_revision revision \
              ON revision.rule_revision_ref=rule.active_revision_ref \
       WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
     LEFT JOIN observation_domain domain ON domain.domain_ref = target.domain_ref \
     WHERE ($1::text IS NULL OR target.target_kind = $1) \
       AND ($2::text IS NULL OR target.lifecycle_state = $2) \
       AND ($4::uuid IS NULL OR target.domain_ref = $4) \
     ORDER BY target.first_stored_at DESC LIMIT $3"
);
const LIST_TARGETS_WITHOUT_DOMAIN: &str = concat!(
    "SELECT ",
    listed_target_columns!(),
    ", NULL::text, NULL::boolean \
     FROM collection_observation_target target \
     LEFT JOIN LATERAL ( \
       SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) \
                AS automatic_any, \
              min(rule.monitor_next_run_at) AS next_run_at, \
              max(rule.last_patrol_dispatched_at) AS last_dispatched_at \
       FROM collection_monitor_rule rule \
       LEFT JOIN collection_monitor_rule_revision revision \
              ON revision.rule_revision_ref=rule.active_revision_ref \
       WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
     WHERE ($1::text IS NULL OR target.target_kind = $1) \
       AND ($2::text IS NULL OR target.lifecycle_state = $2) \
       AND $4::uuid IS NULL \
     ORDER BY target.first_stored_at DESC LIMIT $3"
);

/// 按引用读一个观察目标。
///
/// 抽屉必须用它，**不能从当前列表里找**：列表是筛过的，一个被筛掉的目标会让抽屉说
/// 「未找到」——而它其实好好地在库里。那是在撒谎，且人无从判断是链接失效还是筛选挡住。
pub async fn read_target(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<ObservationTarget>, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Ok(None);
    }
    let domain_ready = crate::observation_domain::observation_domain_schema_is_ready(database)
        .await
        .unwrap_or(false);
    let row: Option<ListedTargetRow> = sqlx::query_as(if domain_ready {
        READ_TARGET_WITH_DOMAIN
    } else {
        READ_TARGET_WITHOUT_DOMAIN
    })
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(listed_target))
}

/// 数各维度的来源。**不受当前筛选影响**：tab 上的数字要回答「切过去有多少」，
/// 用筛选后的结果去数，每个 tab 都会显示当前这一档的数量，那毫无意义。
///
/// 但**受当前领域影响**：筛选是在一个领域里切换的，如果计数跨领域去数，站在「考研自习」
/// 下会看到「创作者 2」而列表里只有 1 个——那个 2 说的是别的世界的事。
pub async fn count_targets(
    database: &Database,
    domain: Option<Uuid>,
) -> Result<TargetCounts, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Ok(TargetCounts::default());
    }
    let domain_ready = crate::observation_domain::observation_domain_schema_is_ready(database)
        .await
        .unwrap_or(false);
    const COUNT_HEAD: &str = "SELECT count(*), \
                count(*) FILTER (WHERE target.target_kind = 'creator'), \
                count(*) FILTER (WHERE target.target_kind = 'keyword'), \
                count(*) FILTER (WHERE target.lifecycle_state = 'archiving'), \
                count(*) FILTER (WHERE target.monitoring_enabled AND EXISTS ( \
                    SELECT 1 FROM collection_monitor_rule rule \
                    JOIN collection_monitor_rule_revision revision \
                      ON revision.rule_revision_ref=rule.active_revision_ref \
                    WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
                      AND revision.automatic_enabled)) \
         FROM collection_observation_target target";
    const COUNT_IN_DOMAIN: &str = concat!(
        "SELECT count(*), \
                count(*) FILTER (WHERE target.target_kind = 'creator'), \
                count(*) FILTER (WHERE target.target_kind = 'keyword'), \
                count(*) FILTER (WHERE target.lifecycle_state = 'archiving'), \
                count(*) FILTER (WHERE target.monitoring_enabled AND EXISTS ( \
                    SELECT 1 FROM collection_monitor_rule rule \
                    JOIN collection_monitor_rule_revision revision \
                      ON revision.rule_revision_ref=rule.active_revision_ref \
                    WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL \
                      AND revision.automatic_enabled)) \
         FROM collection_observation_target target \
         WHERE $1::uuid IS NULL OR target.domain_ref = $1"
    );
    let row: (i64, i64, i64, i64, i64) = if domain_ready {
        sqlx::query_as(COUNT_IN_DOMAIN).bind(domain)
    } else {
        sqlx::query_as(COUNT_HEAD)
    }
    .fetch_one(database.pool())
    .await?;
    Ok(TargetCounts {
        total: row.0,
        creator: row.1,
        keyword: row.2,
        archiving: row.3,
        monitoring: row.4,
    })
}

/// 列出观察目标，可按类型或生命周期筛选。
///
/// **不按状态硬筛**：观察目标列表就是「我在长期看谁」，一个已建档、正在巡检的博主当然
/// 还在看。此前列表只读 `pending_decision`，于是目标一开始建档就从列表里消失——那把
/// 「观察目标列表」做成了「待办列表」，两者不是一回事。
/// 列出观察目标。
///
/// `domain` 传 `Some` 时只列该领域的目标，传 `None` 是「全部领域」——采集是运维视角，
/// 一眼看到所有领域在跑什么是真实需求，与语料页只看一个世界的读法不同。
///
/// 没标过 `domain_ref` 的目标只出现在“全部领域”视图。把它塞进任一具体领域会在页面上
/// 隐藏尚未作出的归属决定，并与采集准入的领域闸门产生矛盾。
pub async fn list_targets(
    database: &Database,
    filter: Option<&str>,
    domain: Option<Uuid>,
    limit: i64,
) -> Result<Vec<ObservationTarget>, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }
    // 领域表还没建立时，这个列表照常出，只是没有领域归属可显示。少一列远好过整页读不出来。
    let domain_ready = crate::observation_domain::observation_domain_schema_is_ready(database)
        .await
        .unwrap_or(false);
    // 筛选值来自页面页签：两个是目标类型，两个是生命周期。它们筛的是不同的列，因此
    // 分开传，而不是把一个字符串塞进一列去猜。
    let (kind, state) = match filter.unwrap_or("") {
        "creator" | "keyword" => (filter, None),
        value @ ("archiving" | "monitoring") => (None, Some(value)),
        _ => (None, None),
    };
    let rows = sqlx::query_as::<_, ListedTargetRow>(if domain_ready {
        LIST_TARGETS_WITH_DOMAIN
    } else {
        LIST_TARGETS_WITHOUT_DOMAIN
    })
    .bind(kind)
    .bind(state)
    .bind(limit)
    .bind(domain)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().map(listed_target).collect())
}

/// 列表比其它查询多读一列巡检开关，因此单独一个行类型——把它加进共用的 `TargetRow`
/// 会打断另外两个只选九列的查询。
/// 列表行转对象。列表与单条读取共用它——两处各写一遍，迟早会有一处漏掉新字段。
fn listed_target(row: ListedTargetRow) -> ObservationTarget {
    ObservationTarget {
        monitoring_enabled: row.9,
        group_name: row.10,
        last_patrol_dispatched_at: row.11,
        last_patrol_succeeded_at: row.12,
        next_patrol_at: row.13,
        domain_name: row.14,
        domain_is_own: row.15,
        ..ObservationTarget::from((
            row.0, row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8,
        ))
    }
}

type ListedTargetRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    Option<Value>,
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<bool>,
);

pub async fn list_targets_in_state(
    database: &Database,
    state: LifecycleState,
    limit: i64,
) -> Result<Vec<ObservationTarget>, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }
    let rows = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target \
         WHERE lifecycle_state = $1 \
         ORDER BY first_stored_at DESC \
         LIMIT $2",
    )
    .bind(state.as_str())
    .bind(limit)
    .fetch_all(database.pool())
    .await?;
    Ok(rows.into_iter().map(ObservationTarget::from).collect())
}

/// Move a target to a new lifecycle state, refusing moves the state machine does not allow.
///
/// The guard is applied against the row's current state read inside the same transaction, so
/// two concurrent moves cannot both see the old state and both succeed.
pub async fn transition_target(
    database: &Database,
    target_ref: Uuid,
    to: LifecycleState,
    actor: &str,
    reason_code: &str,
    reason: Option<&str>,
) -> Result<ObservationTarget, CollectionTargetError> {
    let mut transaction = database.pool().begin().await?;

    let current: (String, String) = sqlx::query_as(
        "SELECT lifecycle_state,target_kind FROM collection_observation_target \
         WHERE target_ref = $1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;

    let from = LifecycleState::parse(&current.0)?;
    let target_kind = TargetKind::parse(&current.1)?;
    if from == to {
        transaction.rollback().await?;
        return read_target_by_ref(database, target_ref).await;
    }
    if !from.may_move_to_for(to, target_kind) {
        transaction.rollback().await?;
        return Err(CollectionTargetError::IllegalTransition {
            from: from.as_str().to_owned(),
            to: to.as_str().to_owned(),
        });
    }

    sqlx::query(
        "UPDATE collection_observation_target \
         SET lifecycle_state = $2, lifecycle_changed_at = scope_001_now() \
         WHERE target_ref = $1",
    )
    .bind(target_ref)
    .bind(to.as_str())
    .execute(&mut *transaction)
    .await?;

    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(from.as_str())
    .bind(to.as_str())
    .bind(actor)
    .bind(reason_code)
    .bind(reason)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;
    read_target_by_ref(database, target_ref).await
}

async fn read_target_by_ref(
    database: &Database,
    target_ref: Uuid,
) -> Result<ObservationTarget, CollectionTargetError> {
    let row = sqlx::query_as::<_, TargetRow>(
        "SELECT target_ref, platform, target_kind, identity_key, display_name, identity_facts, \
                source, lifecycle_state, to_char(first_stored_at, 'YYYY-MM-DD\"T\"HH24:MI:SSOF') \
         FROM collection_observation_target WHERE target_ref = $1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?
    .ok_or(CollectionTargetError::UnknownTarget)?;
    Ok(row.into())
}

async fn record_transition(
    database: &Database,
    target_ref: Uuid,
    from: Option<LifecycleState>,
    to: LifecycleState,
    actor: &str,
    reason_code: &str,
    reason: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO collection_observation_target_transition \
             (transition_ref, target_ref, from_state, to_state, actor, reason_code, reason) \
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind(from.map(LifecycleState::as_str))
    .bind(to.as_str())
    .bind(actor)
    .bind(reason_code)
    .bind(reason)
    .execute(database.pool())
    .await?;
    Ok(())
}

type TargetRow = (
    Uuid,
    String,
    String,
    String,
    Option<String>,
    Option<Value>,
    String,
    String,
    Option<String>,
);

impl From<TargetRow> for ObservationTarget {
    fn from(row: TargetRow) -> Self {
        Self {
            target_ref: row.0,
            platform: row.1,
            target_kind: row.2,
            identity_key: row.3,
            display_name: row.4,
            identity_facts: row.5,
            source: row.6,
            lifecycle_state: row.7,
            first_stored_at: row.8.unwrap_or_default(),
            // 只有列表查询读它们；其它入口保持默认，由调用方按需另读。
            monitoring_enabled: false,
            group_name: None,
            last_patrol_dispatched_at: None,
            last_patrol_succeeded_at: None,
            next_patrol_at: None,
            domain_name: None,
            domain_is_own: None,
        }
    }
}

/// 删除一个观察目标之前，先把「会消失什么、会留下什么」摆出来。
///
/// 这不是一句「确定删除吗」。不可逆的操作要让人看着真实数字决定：删掉的是控制面
/// （我要盯着这个人这个决定，以及它产生的申请、准入、工单、租约），留下的是采集事实
/// （作品、详情、评论）——它们在数据库层就禁止删除，而且作者归属推自 append-only 事实，
/// 不依赖这个目标是否存在。
#[derive(Debug, Clone)]
pub struct TargetDeletionPreview {
    /// 人必须输入的确认词。显示名未知或为空时稳定回退到 identity key。
    pub confirmation_name: String,
    pub target_kind: String,
    pub work_orders: i64,
    pub leases: i64,
    pub lease_tasks: i64,
    pub rule_revisions: i64,
    pub requests: i64,
    /// 归属这个作者、会保留下来的作品数。关键词目标没有作者可言，为 0。
    pub retained_works: i64,
    pub retained_details: i64,
    /// 跨行业样本是已采集的参照材料，不能随观察决定一起抹掉。
    pub blocking_cross_industry_samples: i64,
    /// 随目标控制面一起删除的、只在该目标目录内成立的人工作品失效结论。
    pub material_retirements: i64,
}

pub async fn read_target_deletion_preview(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<TargetDeletionPreview>, sqlx::Error> {
    let row: Option<(String, String, i64, i64, i64, i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key),target.target_kind, \
                (SELECT count(*) FROM collection_work_order w WHERE w.target_ref=target.target_ref), \
                (SELECT count(*) FROM collection_work_order w \
                 JOIN collection_work_order_lease l USING(work_order_ref) \
                 WHERE w.target_ref=target.target_ref), \
                (SELECT count(*) FROM collection_work_order w \
                 JOIN collection_work_order_lease l USING(work_order_ref) \
                 JOIN collection_work_order_lease_task lt ON lt.lease_ref=l.lease_ref \
                 WHERE w.target_ref=target.target_ref), \
                (SELECT count(*) FROM collection_monitor_rule_revision r \
                 WHERE r.target_ref=target.target_ref), \
                (SELECT count(*) FROM collection_acquisition_request q \
                 WHERE q.target_ref=target.target_ref), \
                (SELECT count(*) FROM linggan_material_content_author a \
                 WHERE a.author_external_id=target.identity_key AND a.platform=target.platform), \
                (SELECT count(*) FROM linggan_material_content_author a \
                 JOIN linggan_material_content_detail d \
                   ON d.content_public_ref=a.content_public_ref \
                 WHERE a.author_external_id=target.identity_key AND a.platform=target.platform), \
                (SELECT count(*) FROM cross_industry_sample s \
                 WHERE s.target_ref=target.target_ref), \
                (SELECT count(*) FROM collection_material_retirement retirement \
                 WHERE retirement.target_ref=target.target_ref) \
         FROM collection_observation_target target WHERE target.target_ref=$1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(|row| TargetDeletionPreview {
        confirmation_name: row.0,
        target_kind: row.1,
        work_orders: row.2,
        leases: row.3,
        lease_tasks: row.4,
        rule_revisions: row.5,
        requests: row.6,
        retained_works: row.7,
        retained_details: row.8,
        blocking_cross_industry_samples: row.9,
        material_retirements: row.10,
    }))
}

/// 删除的结果。「删不了」与「没找到」是两件事，不能都报成失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetDeletionOutcome {
    Deleted,
    UnknownTarget,
    /// 输入的名字与目标名字不一致。不可逆操作要求手打名字，点两下太容易了。
    NameMismatch,
    /// 有必须保留的跨行业材料挂在它下面，不能把材料和观察决定一起抹掉。
    BlockedByProtectedFacts {
        rows: i64,
    },
}

/// 彻底删除一个观察目标：删掉控制面，保留采集事实。
///
/// 采集事实（作品、详情、评论、采集包、回执）在数据库层挂着 append-only 触发器，删除会被
/// 直接拒绝——那是这个系统的地基，不为一次清理去拆。而这些事实本来也不需要跟着走：
/// 「这篇笔记是这个博主发的」是世界的事实，「我要盯着这个人」是我的决定，删掉后者不该
/// 让前者失效。作者归属推自 append-only 事实（见 `linggan_material_content_author`），
/// 因此删除之后作品仍然属于这个博主、仍然检索得到。
pub async fn delete_observation_target(
    database: &Database,
    target_ref: Uuid,
    confirmed_display_name: &str,
) -> Result<TargetDeletionOutcome, sqlx::Error> {
    let mut tx = database.pool().begin().await?;
    let found: Option<(String,)> = sqlx::query_as(
        "SELECT COALESCE(NULLIF(btrim(display_name),''),identity_key) \
         FROM collection_observation_target WHERE target_ref=$1 FOR UPDATE",
    )
    .bind(target_ref)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((confirmation_name,)) = found else {
        tx.rollback().await?;
        return Ok(TargetDeletionOutcome::UnknownTarget);
    };
    if confirmation_name != confirmed_display_name.trim() {
        tx.rollback().await?;
        return Ok(TargetDeletionOutcome::NameMismatch);
    }
    let blocking: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cross_industry_sample WHERE target_ref=$1")
            .bind(target_ref)
            .fetch_one(&mut *tx)
            .await?;
    if blocking > 0 {
        tx.rollback().await?;
        return Ok(TargetDeletionOutcome::BlockedByProtectedFacts { rows: blocking });
    }

    // 跨行业详情补采的作用域（`0074`）挂在工单上。漏了它，删一个做过详情补采的关键词
    // 会在删工单那一步撞外键，整笔回滚——而界面只会说「删除没有完成」。
    //
    // 单独一条而不是并进下面那串：只装了控制面那一段 schema 的库里没有这张表，而
    // PostgreSQL **在解析阶段就会因表不存在报错**，写在 `WHERE` 里的存在性判断根本来不及
    // 生效。它只 FK 工单，先删掉不影响其余顺序。
    let cross_industry_scope_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('collection_work_order_cross_industry_target') IS NOT NULL",
    )
    .fetch_one(&mut *tx)
    .await?;
    if cross_industry_scope_ready {
        sqlx::query(
            "DELETE FROM collection_work_order_cross_industry_target WHERE work_order_ref IN ( \
               SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1)",
        )
        .bind(target_ref)
        .execute(&mut *tx)
        .await?;
    }

    // 顺序由外键决定，从叶子往根删。任何一条走不通都会整笔回滚——半删的目标比不删更糟。
    for statement in [
        // 作品失效是“这个目标的目录里不再补这篇”的人工作品控制结论。删除整个观察
        // 决定后它已没有可应用的目录，因此跟控制面一起删除；底层作品、详情、评论、
        // Package 与 Receipt 仍由各自的 append-only 边界保留。
        "DELETE FROM collection_material_retirement WHERE target_ref=$1",
        "DELETE FROM collection_monitor_rule_command_receipt WHERE target_ref=$1",
        "DELETE FROM collection_monitor_rule_command_identity WHERE target_ref=$1",
        "DELETE FROM collection_scheduler_target_decision WHERE target_ref=$1",
        "DELETE FROM collection_work_order_lease_task_dispatch_failure WHERE task_id IN ( \
           SELECT lt.task_id FROM collection_work_order w \
           JOIN collection_work_order_lease l USING(work_order_ref) \
           JOIN collection_work_order_lease_task lt ON lt.lease_ref=l.lease_ref \
           WHERE w.target_ref=$1)",
        "DELETE FROM collection_work_order_lease_task WHERE lease_ref IN ( \
           SELECT l.lease_ref FROM collection_work_order w \
           JOIN collection_work_order_lease l USING(work_order_ref) WHERE w.target_ref=$1)",
        "DELETE FROM collection_work_order_material_target WHERE work_order_ref IN ( \
           SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1)",
        "DELETE FROM collection_work_order_lease WHERE work_order_ref IN ( \
           SELECT work_order_ref FROM collection_work_order WHERE target_ref=$1)",
        "DELETE FROM collection_work_order WHERE target_ref=$1",
        "DELETE FROM collection_observation_target_transition WHERE target_ref=$1",
        "DELETE FROM collection_admission_decision WHERE request_ref IN ( \
           SELECT request_ref FROM collection_acquisition_request WHERE target_ref=$1)",
        "DELETE FROM collection_admission_decision WHERE target_ref=$1",
        "DELETE FROM collection_acquisition_request WHERE target_ref=$1",
        "UPDATE collection_observation_target \
         SET monitoring_enabled=false,lifecycle_state='dismissed' WHERE target_ref=$1",
        // 规则身份与规则版本互相引用（规则指着当前版本，版本挂在规则上，`0076`），
        // 谁都不能先删。先把规则那一侧的引用松开，再删版本，最后删规则身份。
        "UPDATE collection_monitor_rule SET active_revision_ref=NULL WHERE target_ref=$1",
        "DELETE FROM collection_monitor_rule_revision WHERE target_ref=$1",
        "DELETE FROM collection_monitor_rule WHERE target_ref=$1",
        "DELETE FROM collection_observation_target WHERE target_ref=$1",
    ] {
        sqlx::query(statement)
            .bind(target_ref)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(TargetDeletionOutcome::Deleted)
}
