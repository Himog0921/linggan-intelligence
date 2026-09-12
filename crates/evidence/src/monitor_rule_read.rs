//! 一个观察目标底下有哪几条巡检规则。
//!
//! 规则从「一个目标一条」变成多条之后（`0076`），界面必须能把它们列出来：一个关键词
//! 同时盯综合榜和点赞榜是常态，看不见就管不了——人无法知道自己配过几条、哪条还在跑。
//!
//! 博主只有一条（首要槽），列出来仍然成立，只是永远只有一行。

use linggan_storage_postgres::Database;
use uuid::Uuid;

/// 一条巡检规则当前的样子。
#[derive(Debug, Clone)]
pub struct MonitorRuleSummary {
    pub rule_ref: Uuid,
    /// 口径：关键词是排序键，博主是 `primary`。
    pub slot_key: String,
    pub is_primary: bool,
    /// 自动巡检开着没有。关掉的规则仍然留着——它签发过的工单与材料还挂在它的版本上。
    pub automatic_enabled: bool,
    /// 多久跑一次（秒）。读不到时为 `None`，不编一个默认值。
    pub interval_seconds: Option<i32>,
    /// 每轮下拉几次、取前多少篇、只看最近多少天。缺的就是没设，不是 0。
    pub scroll_rounds: Option<i32>,
    pub top_by_likes: Option<i32>,
    pub published_within_days: Option<i32>,
    pub next_run_at: Option<String>,
    pub last_succeeded_at: Option<String>,
}

/// 列出这个目标当前在用的规则。已停用的不返回——它们是历史，不是可管理的对象。
///
/// 表还不在的环境返回 `None`，**不是空列表**：「这个库还没有规则这回事」与「这个目标
/// 一条规则都没配」是两件事，压成同一个空列表，界面会对一个正在巡检的目标说「还没有
/// 巡检规则」，而人据此会再配一条。
pub async fn read_target_monitor_rules(
    database: &Database,
    target_ref: Uuid,
) -> Result<Option<Vec<MonitorRuleSummary>>, sqlx::Error> {
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('collection_monitor_rule') IS NOT NULL")
            .fetch_one(database.pool())
            .await?;
    if !schema_ready {
        return Ok(None);
    }
    let rows: Vec<MonitorRuleRow> = sqlx::query_as(
        "SELECT rule.rule_ref,rule.slot_key,rule.is_primary, \
                revision.automatic_enabled, \
                CASE WHEN revision.mode='fixed' THEN revision.fixed_interval_seconds \
                     ELSE revision.fallback_interval_seconds END AS interval_seconds, \
                revision.scroll_rounds,revision.top_by_likes,revision.published_within_days, \
                linggan_human_moment(rule.monitor_next_run_at) AS next_run_at, \
                linggan_human_moment(rule.last_patrol_succeeded_at) AS last_succeeded_at \
         FROM collection_monitor_rule rule \
         LEFT JOIN collection_monitor_rule_revision revision \
                ON revision.rule_revision_ref=rule.active_revision_ref \
         WHERE rule.target_ref=$1 AND rule.retired_at IS NULL \
         ORDER BY rule.is_primary DESC,rule.slot_key",
    )
    .bind(target_ref)
    .fetch_all(database.pool())
    .await?;
    Ok(Some(
        rows.into_iter()
            .map(|row| MonitorRuleSummary {
                rule_ref: row.0,
                slot_key: row.1,
                is_primary: row.2,
                automatic_enabled: row.3.unwrap_or(false),
                interval_seconds: row.4,
                scroll_rounds: row.5,
                top_by_likes: row.6,
                published_within_days: row.7,
                next_run_at: row.8,
                last_succeeded_at: row.9,
            })
            .collect(),
    ))
}

type MonitorRuleRow = (
    Uuid,
    String,
    bool,
    Option<bool>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<String>,
    Option<String>,
);

#[derive(Debug, thiserror::Error)]
pub enum MonitorRuleRetireError {
    #[error("that rule does not belong to this target")]
    UnknownRule,
    #[error("the only remaining rule cannot be retired while the target is monitoring")]
    LastRuleOfAMonitoredTarget,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// 停用一条规则。
///
/// **不删**：它签发过的工单与材料还挂在它的版本上，删掉等于让那些材料说不清是按什么口径
/// 取回来的。停用只是不再排期。
///
/// 监控中的目标不允许把最后一条规则停掉——`0042` 之后的那两条 CHECK 守的正是「监控中
/// 必须有规则」。要停最后一条，先停止观察这个目标。
pub async fn retire_monitor_rule(
    database: &Database,
    target_ref: Uuid,
    rule_ref: Uuid,
) -> Result<(), MonitorRuleRetireError> {
    let mut transaction = database.pool().begin().await?;
    let belongs: Option<bool> = sqlx::query_scalar(
        "SELECT is_primary FROM collection_monitor_rule \
         WHERE rule_ref=$1 AND target_ref=$2 AND retired_at IS NULL FOR UPDATE",
    )
    .bind(rule_ref)
    .bind(target_ref)
    .fetch_optional(&mut *transaction)
    .await?;
    let Some(is_primary) = belongs else {
        transaction.rollback().await?;
        return Err(MonitorRuleRetireError::UnknownRule);
    };
    let (live_rules, monitoring): (i64, bool) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM collection_monitor_rule \
                  WHERE target_ref=$1 AND retired_at IS NULL), \
                (SELECT monitoring_enabled FROM collection_observation_target \
                  WHERE target_ref=$1)",
    )
    .bind(target_ref)
    .fetch_one(&mut *transaction)
    .await?;
    if monitoring && live_rules <= 1 {
        transaction.rollback().await?;
        return Err(MonitorRuleRetireError::LastRuleOfAMonitoredTarget);
    }
    sqlx::query(
        "UPDATE collection_monitor_rule \
         SET retired_at=scope_001_now(),monitor_next_run_at=NULL WHERE rule_ref=$1",
    )
    .bind(rule_ref)
    .execute(&mut *transaction)
    .await?;

    // 停掉的是首要规则时，**必须把首要位交给还在的那条**。
    //
    // 目标行上的 `active_monitor_rule_revision_ref` 指着首要规则的当前版本，全项目有五处
    // 把它当作「这个目标的规则」在读（发租的取样口径、派发的规则闸、运行产能、管理巡查
    // 面板）。留下一个没有首要规则的目标，那五处会一直读着一条已经停用的规则的版本——
    // 而界面上这条规则已经不在列表里了，没人看得出它还在影响什么。
    if is_primary {
        let promoted: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
            "SELECT rule_ref,active_revision_ref FROM collection_monitor_rule \
             WHERE target_ref=$1 AND retired_at IS NULL \
             ORDER BY created_at,rule_ref LIMIT 1",
        )
        .bind(target_ref)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((promoted_rule_ref, promoted_revision_ref)) = promoted else {
            // 上面的闸门保证监控中的目标至少还留着一条；走到这里说明目标没在监控，
            // 那就没有首要规则可言，也没有谁在读那个指针。
            transaction.commit().await?;
            return Ok(());
        };
        sqlx::query("UPDATE collection_monitor_rule SET is_primary=true WHERE rule_ref=$1")
            .bind(promoted_rule_ref)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            "UPDATE collection_observation_target \
             SET active_monitor_rule_revision_ref=$2 WHERE target_ref=$1",
        )
        .bind(target_ref)
        .bind(promoted_revision_ref)
        .execute(&mut *transaction)
        .await?;
    }
    transaction.commit().await?;
    Ok(())
}
