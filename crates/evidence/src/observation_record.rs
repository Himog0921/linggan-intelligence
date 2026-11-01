//! 一位创作者在本领域的观察记录：何时加入、用途、观察档案与巡查的真实时间线。
//!
//! 只回答「系统里记着什么」，不推算：没有巡查结果就是 `None`，规则没排期就没有下次时间，
//! 后端把未知留成空，由页面如实写「未记录」。工作记录（巡查带回了什么）复用
//! `observation_summary` 的读取，统计口径与采集页完全一致。

use crate::observation_summary::{PatrolReadState, read_patrol_summaries_for_refs};
use linggan_contracts::creator_discovery::decode_creator_key;
use linggan_storage_postgres::Database;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationTransition {
    pub occurred_at: String,
    pub from_state: Option<String>,
    pub to_state: String,
    pub reason_code: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatorObservationRecord {
    pub target_ref: Uuid,
    /// 加入本领域的时间与用途（`observation_domain_target`）。
    pub joined_at: String,
    pub usage_role: String,
    pub first_stored_at: Option<String>,
    pub lifecycle_state: String,
    pub lifecycle_changed_at: Option<String>,
    /// monitoring / paused / dismissed / not_enabled，与列表里的观察状态同一口径。
    pub monitoring_state: String,
    pub automatic_enabled: bool,
    pub last_dispatched_at: Option<String>,
    /// 目标级「上次巡查成功」，包含手动的一次性观察。
    pub last_succeeded_at: Option<String>,
    pub next_run_at: Option<String>,
    pub missed_runs: i64,
    /// 最近一次成功巡查带回的作品数；`None` 表示没有合格的成功巡查，或没读成。
    pub latest_hits: Option<i64>,
    pub latest_new: Option<i64>,
    pub patrol_state: String,
    pub last_transition: Option<ObservationTransition>,
}

/// 读不到目标或它不在这个领域，返回 `Ok(None)`：那是「未加入本领域」，不是错误。
pub async fn read_creator_observation_record(
    database: &Database,
    domain: Uuid,
    creator_key: &str,
) -> Result<Option<CreatorObservationRecord>, sqlx::Error> {
    let Some((platform, author)) = decode_creator_key(creator_key) else {
        return Ok(None);
    };
    let row = sqlx::query(
        "SELECT target.target_ref, relation.role, \
                linggan_human_moment(relation.created_at) AS joined_at, \
                linggan_human_moment(target.first_stored_at) AS first_stored_at, \
                target.lifecycle_state, \
                linggan_human_moment(target.lifecycle_changed_at) AS lifecycle_changed_at, \
                target.monitoring_enabled, \
                COALESCE(rules.automatic_any,false) AS automatic_enabled, \
                linggan_human_moment(rules.last_dispatched_at) AS last_dispatched_at, \
                linggan_human_moment(target.last_patrol_succeeded_at) AS last_succeeded_at, \
                CASE WHEN target.monitoring_enabled AND COALESCE(rules.automatic_any,false) \
                     THEN linggan_human_moment(rules.next_run_at) END AS next_run_at, \
                COALESCE(rules.missed_runs,0) AS missed_runs \
         FROM collection_observation_target target \
         JOIN observation_domain_target relation \
           ON relation.target_ref=target.target_ref AND relation.domain_ref=$1 \
         LEFT JOIN LATERAL ( \
           SELECT COALESCE(bool_or(COALESCE(revision.automatic_enabled,false)),false) AS automatic_any, \
                  min(rule.monitor_next_run_at) AS next_run_at, \
                  max(rule.last_patrol_dispatched_at) AS last_dispatched_at, \
                  max(rule.monitor_missed_run_count) AS missed_runs \
           FROM collection_monitor_rule rule \
           LEFT JOIN collection_monitor_rule_revision revision \
             ON revision.rule_revision_ref=rule.active_revision_ref \
           WHERE rule.target_ref=target.target_ref AND rule.retired_at IS NULL) rules ON true \
         WHERE target.platform=$2 AND target.target_kind='creator' AND target.identity_key=$3",
    )
    .bind(domain)
    .bind(&platform)
    .bind(&author)
    .fetch_optional(database.pool())
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let target_ref: Uuid = row.get("target_ref");
    let lifecycle_state: String = row.get("lifecycle_state");
    let monitoring_enabled: bool = row.get("monitoring_enabled");
    let automatic_enabled: bool = row.get("automatic_enabled");
    let monitoring_state = if matches!(lifecycle_state.as_str(), "paused" | "dismissed") {
        lifecycle_state.clone()
    } else if monitoring_enabled && automatic_enabled {
        "monitoring".to_owned()
    } else {
        "not_enabled".to_owned()
    };
    let transition = sqlx::query(
        "SELECT linggan_human_moment(occurred_at) AS occurred_at, from_state, to_state, \
                reason_code, reason \
         FROM collection_observation_target_transition \
         WHERE target_ref=$1 ORDER BY occurred_at DESC, transition_ref DESC LIMIT 1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await?
    .map(|row| ObservationTransition {
        occurred_at: row.get("occurred_at"),
        from_state: row.get("from_state"),
        to_state: row.get("to_state"),
        reason_code: row.get("reason_code"),
        reason: row.get("reason"),
    });
    // 巡查结果读不到时保留其余记录：未知的只是「带回了什么」，不是整条观察关系。
    let summary = read_patrol_summaries_for_refs(database, &[target_ref])
        .await
        .ok()
        .and_then(|mut summaries| summaries.remove(&target_ref));
    Ok(Some(CreatorObservationRecord {
        target_ref,
        joined_at: row.get("joined_at"),
        usage_role: row.get("role"),
        first_stored_at: row.get("first_stored_at"),
        lifecycle_state,
        lifecycle_changed_at: row.get("lifecycle_changed_at"),
        monitoring_state,
        automatic_enabled,
        last_dispatched_at: row.get("last_dispatched_at"),
        last_succeeded_at: row.get("last_succeeded_at"),
        next_run_at: row.get("next_run_at"),
        missed_runs: i64::from(row.get::<i32, _>("missed_runs")),
        latest_hits: summary.as_ref().and_then(|value| value.latest_hits),
        latest_new: summary.as_ref().and_then(|value| value.latest_new),
        patrol_state: summary
            .map(|value| patrol_state_label(value.patrol_state))
            .unwrap_or("unavailable")
            .to_owned(),
        last_transition: transition,
    }))
}

fn patrol_state_label(state: PatrolReadState) -> &'static str {
    match state {
        PatrolReadState::Disabled => "disabled",
        PatrolReadState::Waiting => "waiting",
        PatrolReadState::Running => "running",
        PatrolReadState::Normal => "normal",
        PatrolReadState::Partial => "partial",
        PatrolReadState::Blocked => "blocked",
        PatrolReadState::Unavailable => "unavailable",
    }
}
