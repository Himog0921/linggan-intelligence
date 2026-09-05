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
pub async fn store_pending_target(
    database: &Database,
    identity: &TargetIdentity,
    source: TargetSource,
    display_name: Option<&str>,
    identity_facts: Option<&Value>,
) -> Result<(ObservationTarget, StoreOutcome), CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }

    let target_ref = Uuid::new_v4();
    let inserted = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO collection_observation_target \
             (target_ref, platform, target_kind, identity_key, display_name, identity_facts, source) \
         VALUES ($1, $2, $3, $4, $5, $6, $7) \
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
    let row: Option<ListedTargetRow> = sqlx::query_as(
        "SELECT target.target_ref, target.platform, target.target_kind, target.identity_key, \
                target.display_name, target.identity_facts, target.source, target.lifecycle_state, \
                target.first_stored_at::text, \
                (target.monitoring_enabled AND COALESCE(rule.automatic_enabled,false)), \
                target.group_name, \
                to_char(target.last_patrol_dispatched_at, 'MM-DD HH24:MI'), \
                to_char(target.last_patrol_succeeded_at, 'MM-DD HH24:MI'), \
                CASE WHEN target.monitoring_enabled AND COALESCE(rule.automatic_enabled,false) \
                     THEN to_char(target.monitor_next_run_at, 'MM-DD HH24:MI') END \
         FROM collection_observation_target target \
         LEFT JOIN collection_monitor_rule_revision rule \
           ON rule.rule_revision_ref=target.active_monitor_rule_revision_ref \
         WHERE target.target_ref = $1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?;
    Ok(row.map(listed_target))
}

/// 数各维度的来源。**不受当前筛选影响**：tab 上的数字要回答「切过去有多少」，
/// 用筛选后的结果去数，每个 tab 都会显示当前这一档的数量，那毫无意义。
pub async fn count_targets(database: &Database) -> Result<TargetCounts, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Ok(TargetCounts::default());
    }
    let row: (i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*), \
                count(*) FILTER (WHERE target.target_kind = 'creator'), \
                count(*) FILTER (WHERE target.target_kind = 'keyword'), \
                count(*) FILTER (WHERE target.lifecycle_state = 'archiving'), \
                count(*) FILTER (WHERE target.monitoring_enabled \
                    AND COALESCE(rule.automatic_enabled,false)) \
         FROM collection_observation_target target \
         LEFT JOIN collection_monitor_rule_revision rule \
           ON rule.rule_revision_ref=target.active_monitor_rule_revision_ref",
    )
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
pub async fn list_targets(
    database: &Database,
    filter: Option<&str>,
    limit: i64,
) -> Result<Vec<ObservationTarget>, CollectionTargetError> {
    if !collection_target_schema_is_ready(database).await? {
        return Err(CollectionTargetError::SchemaUnavailable);
    }
    // 筛选值来自页面页签：两个是目标类型，两个是生命周期。它们筛的是不同的列，因此
    // 分开传，而不是把一个字符串塞进一列去猜。
    let (kind, state) = match filter.unwrap_or("") {
        "creator" | "keyword" => (filter, None),
        value @ ("archiving" | "monitoring") => (None, Some(value)),
        _ => (None, None),
    };
    let rows = sqlx::query_as::<_, ListedTargetRow>(
        "SELECT target.target_ref, target.platform, target.target_kind, target.identity_key, \
                target.display_name, target.identity_facts, target.source, target.lifecycle_state, \
                target.first_stored_at::text, \
                (target.monitoring_enabled AND COALESCE(rule.automatic_enabled,false)), \
                target.group_name, \
                to_char(target.last_patrol_dispatched_at, 'MM-DD HH24:MI'), \
                to_char(target.last_patrol_succeeded_at, 'MM-DD HH24:MI'), \
                CASE WHEN target.monitoring_enabled AND COALESCE(rule.automatic_enabled,false) \
                     THEN to_char(target.monitor_next_run_at, 'MM-DD HH24:MI') END \
         FROM collection_observation_target target \
         LEFT JOIN collection_monitor_rule_revision rule \
           ON rule.rule_revision_ref=target.active_monitor_rule_revision_ref \
         WHERE ($1::text IS NULL OR target.target_kind = $1) \
           AND ($2::text IS NULL OR target.lifecycle_state = $2) \
         ORDER BY target.first_stored_at DESC LIMIT $3",
    )
    .bind(kind)
    .bind(state)
    .bind(limit)
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
        }
    }
}
