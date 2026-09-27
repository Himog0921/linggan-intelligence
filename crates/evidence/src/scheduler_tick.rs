//! 一轮 tick 的账本：一行 run、每步一行、外加心跳。
//!
//! **不是第二套巡检账本。** run 仍是一轮一行（`collection_scheduler_run`），目标级决定仍住
//! `collection_scheduler_target_decision`，本模块只是把「这一轮跑了哪几步、每步什么结果」
//! 记进同一个 run 的子表。此前只有巡查写账本，另外三步只打日志：一步崩了，账本上只留下
//! 「这一轮没派活」，分不清「没有到期的活」和「这一步根本没跑成」。
//!
//! 几步纪律：
//!
//! 1. **开始与结束分两笔写**。步骤行先写「开始了」，跑完再补结果；没有补上的那一行就是
//!    `outcome IS NULL`——「未知」是查得出来的状态，不是缺席。
//! 2. **计数留空不写 0**。这一步不数的东西留 NULL；0 是一个事实（数过了，是零），
//!    不能拿它冒充「没数」。
//! 3. **一步失败，其余照跑，各自留痕**：run 的结局与心跳的结局都按「有一步失败就是失败」
//!    取，失败分类只记受限码（SQLSTATE 之类），不记原始报文。
//! 4. 账本不在（未迁移的库）时 `begin` 返回 `None`：这一轮不记账，也不假装记了。

use std::{future::Future, time::Duration};

use linggan_storage_postgres::Database;
use uuid::Uuid;

use crate::step_report::{StepFailure, StepOutcome, StepReport, outcome_of_failure};

/// 四步的名字。与 `0098` 的 CHECK 一致；两边同时改才算改完（证明测试逐键插入一遍）。
pub const STEP_MEDIA_ACQUISITION: &str = "media_acquisition";
pub const STEP_PROGRESSIVE_DOSSIERS: &str = "progressive_dossiers";
pub const STEP_KEYWORD_DETAILS: &str = "keyword_details";
pub const STEP_PATROL: &str = "patrol";

/// 一轮 tick 的步骤，按执行顺序。
pub const TICK_STEP_KEYS: &[&str] = &[
    STEP_MEDIA_ACQUISITION,
    STEP_PROGRESSIVE_DOSSIERS,
    STEP_KEYWORD_DETAILS,
    STEP_PATROL,
];

/// 一轮 tick 的账本句柄。
#[derive(Debug, Clone)]
pub struct TickLedger {
    run_ref: Uuid,
    database: Database,
}

impl TickLedger {
    /// 开一轮：写 run 行与心跳起点。账本表不在时返回 `None`。
    ///
    /// run 行故意只写开始，不写结局：这一轮要是被杀在半路，那一行就是 `outcome IS NULL`
    /// ——「有一轮没跑完」必须自己看得出来，而不是悄悄消失。
    pub async fn begin(database: &Database) -> Result<Option<Self>, sqlx::Error> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('collection_scheduler_run') IS NOT NULL \
                    AND to_regclass('collection_scheduler_run_step') IS NOT NULL",
        )
        .fetch_one(database.pool())
        .await?;
        if !ready {
            return Ok(None);
        }
        let run_ref = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO collection_scheduler_run (scheduler_run_ref,scheduler_key) VALUES ($1,'patrol')",
        )
        .bind(run_ref)
        .execute(database.pool())
        .await?;
        sqlx::query(
            "INSERT INTO collection_scheduler_heartbeat \
                 (scheduler_key,last_tick_started_at,last_outcome,last_error) \
             VALUES ('patrol',scope_001_now(),'unknown',NULL) \
             ON CONFLICT (scheduler_key) DO UPDATE SET \
                 last_tick_started_at=EXCLUDED.last_tick_started_at,last_outcome='unknown',last_error=NULL",
        )
        .execute(database.pool())
        .await?;
        Ok(Some(Self {
            run_ref,
            database: database.clone(),
        }))
    }

    pub fn run_ref(&self) -> Uuid {
        self.run_ref
    }

    /// 记一步开始。此后这一步要么被 `record_step` 收尾，要么留在「未知」上。
    ///
    /// 只有 [`Self::run_step`] 调它：单独「开始」而不收尾，正是本表要能查出来的那种行，
    /// 不该由调用方随手造出来。
    async fn step_started(&self, step_key: &'static str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO collection_scheduler_run_step \
                 (scheduler_run_step_ref,scheduler_run_ref,step_key) VALUES ($1,$2,$3) \
             ON CONFLICT (scheduler_run_ref,step_key) DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(self.run_ref)
        .bind(step_key)
        .execute(self.database.pool())
        .await?;
        Ok(())
    }

    /// 给一步收尾。行不在（开始那笔没写成）算异常：账本不能悄悄少一行。
    ///
    /// 与 [`Self::step_started`] 一样只对 [`Self::run_step`] 开放：一步的**开始与收尾**
    /// 是配对的两笔，由调用方各写一半，等于把「半行」留给所有人自己拼。
    async fn record_step(&self, report: &StepReport) -> Result<(), sqlx::Error> {
        let (considered, produced, skipped) = match &report.outcome {
            StepOutcome::Ok {
                considered,
                produced,
                skipped,
            } => (*considered, *produced, *skipped),
            _ => (None, None, None),
        };
        let updated = sqlx::query(
            "UPDATE collection_scheduler_run_step SET completed_at=scope_001_now(),outcome=$3, \
                 skipped_reason=$4,error_class=$5,considered_count=$6,produced_count=$7,skipped_count=$8 \
             WHERE scheduler_run_ref=$1 AND step_key=$2",
        )
        .bind(self.run_ref)
        .bind(report.step_key)
        .bind(report.outcome.code())
        .bind(report.outcome.reason())
        .bind(report.outcome.error_class())
        .bind(considered)
        .bind(produced)
        .bind(skipped)
        .execute(self.database.pool())
        .await?;
        if updated.rows_affected() == 0 {
            return Err(sqlx::Error::RowNotFound);
        }
        Ok(())
    }

    /// 跑一步、记一行——**记账的唯一入口**。
    ///
    /// 返回这一步的报告与它的结果：账本只负责如实记下发生了什么，值不值当重试、要不要打印
    /// 逐条原因，留给调用方。三个返回形状各自对应一件事实：
    ///
    /// - `(报告, Some(Ok(值)))`：跑了，报告里是 ok / skipped；
    /// - `(报告, Some(Err(错误)))`：跑了并失败，报告里是 failed（或 skipped——错误本身说的是
    ///   「这一步的表不在」，那不是故障，是没轮到）；
    /// - `(报告, None)`：**没跑**。开始那一笔就没写进账本——跑了却记不上，账本会把它显示成
    ///   「从未开始」，那比不跑更坏。
    pub async fn run_step<T, E, F>(
        &self,
        step_key: &'static str,
        work: impl Future<Output = Result<T, E>>,
        summarize: F,
    ) -> (StepReport, Option<Result<T, E>>)
    where
        E: StepFailure,
        F: FnOnce(&T) -> StepOutcome,
    {
        if let Err(_error) = self.step_started(step_key).await {
            return (
                StepReport {
                    step_key,
                    outcome: StepOutcome::failed("step_not_recorded"),
                    duration: Duration::ZERO,
                },
                None,
            );
        }
        let started = std::time::Instant::now();
        let result = work.await;
        let outcome = match &result {
            Ok(value) => summarize(value),
            Err(error) => outcome_of_failure(error),
        };
        let report = StepReport {
            step_key,
            outcome,
            duration: started.elapsed(),
        };
        // 收尾那笔写失败不回滚这一步做过的事，但要让它可见：这一行会永远停在「未知」上。
        if let Err(error) = self.record_step(&report).await {
            eprintln!(
                "linggan runtime: step {} finished but its ledger row could not be closed: {error}",
                report.step_key
            );
        }
        (report, Some(result))
    }

    /// 收轮：把整轮的结局写进 run 行与心跳。
    ///
    /// run 的两个计数列仍是**巡查步**的数（历史含义：这一轮考虑了几条到期规则、排出了几张
    /// 工单），其余步骤的数在各自己的步骤行里。心跳只报巡查步的产出与跳过数，keeping 既有
    /// 页面的口径不变；「有一步失败」则整轮记失败，错误写受限的 `步骤:分类`。
    pub async fn finish(&self, reports: &[StepReport]) -> Result<(), sqlx::Error> {
        let failed = reports.iter().find_map(|report| {
            report
                .outcome
                .error_class()
                .map(|class| (report.step_key, class))
        });
        let patrol = reports.iter().find(|report| report.step_key == STEP_PATROL);
        let considered = patrol
            .and_then(|report| match &report.outcome {
                StepOutcome::Ok { considered, .. } => *considered,
                _ => None,
            })
            .unwrap_or(0);
        let produced = patrol
            .and_then(|report| report.outcome.produced_count())
            .unwrap_or(0);
        let skipped = patrol
            .and_then(|report| match &report.outcome {
                StepOutcome::Ok { skipped, .. } => *skipped,
                _ => None,
            })
            .unwrap_or(0);
        let outcome = tick_outcome(reports);
        let last_error = failed.map(|(step_key, class)| format!("{step_key}:{class}"));

        sqlx::query(
            "UPDATE collection_scheduler_run SET completed_at=scope_001_now(),outcome=$2, \
                 considered_count=$3,dispatched_count=$4 WHERE scheduler_run_ref=$1",
        )
        .bind(self.run_ref)
        .bind(outcome)
        .bind(i32::try_from(considered).unwrap_or(i32::MAX))
        .bind(i32::try_from(produced).unwrap_or(i32::MAX))
        .execute(self.database.pool())
        .await?;

        sqlx::query(
            "UPDATE collection_scheduler_heartbeat SET \
                 last_tick_completed_at=scope_001_now(),last_outcome=$1, \
                 dispatched_count=$2,skipped_count=$3,last_error=$4 \
             WHERE scheduler_key='patrol'",
        )
        .bind(outcome)
        .bind(i32::try_from(produced).unwrap_or(i32::MAX))
        .bind(i32::try_from(skipped).unwrap_or(i32::MAX))
        .bind(last_error.as_deref())
        .execute(self.database.pool())
        .await?;
        Ok(())
    }
}

/// 整轮的结局，沿用既有词表（`0036` 定的那一套），不含新词。
///
/// **有一步失败就是失败**：一步崩了、其余三步照跑——这一轮在账本上不能长得像「什么都没
/// 发生」。巡查步自己没产出时，按它排出去的工单与跳过的目标分 `queued`/`partial`/`idle`。
///
/// 一个定义、两个读者（收轮写行、worker 发事件）：两处各算一次，迟早会算出两个结局。
pub fn tick_outcome(reports: &[StepReport]) -> &'static str {
    if reports
        .iter()
        .any(|report| report.outcome.error_class().is_some())
    {
        return "failed";
    }
    let patrol = reports.iter().find(|report| report.step_key == STEP_PATROL);
    let produced = patrol
        .and_then(|report| report.outcome.produced_count())
        .unwrap_or(0);
    let skipped = patrol
        .and_then(|report| match &report.outcome {
            StepOutcome::Ok { skipped, .. } => *skipped,
            _ => None,
        })
        .unwrap_or(0);
    if produced > 0 && skipped > 0 {
        "partial"
    } else if produced > 0 {
        "queued"
    } else {
        "idle"
    }
}

/// 心跳表在不在、就绪三列在不在。两个调用方（写判定、开新一轮）都先问这一句。
///
/// 分开问两张事实：心跳表在而三列不在，是「`0098` 还没应用」——那时不写就绪，也不假装
/// 它是 `unknown`（那会和「刚启动、还没判」撞成同一个样子）。
pub(crate) async fn heartbeat_readiness_columns_present(
    database: &Database,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT to_regclass('collection_scheduler_heartbeat') IS NOT NULL \
                AND (SELECT count(*) FROM information_schema.columns \
                     WHERE table_schema=current_schema() \
                       AND table_name='collection_scheduler_heartbeat' \
                       AND column_name IN \
                           ('readiness_state','readiness_detail','readiness_checked_at')) = 3",
    )
    .fetch_one(database.pool())
    .await
}

/// 把就绪判定写进心跳：持续故障的**持久**落点。
///
/// 心跳的 tick 列（上次开始/结束、上次结局）不在这里动：未就绪的一轮没有跑任何步骤，
/// 把它记成「跑过但没结果」或「空闲」都是假话——「这台机器接不了活」这件事由就绪列自己说。
pub async fn record_readiness(
    database: &Database,
    readiness: &crate::RuntimeReadiness,
) -> Result<(), sqlx::Error> {
    if !heartbeat_readiness_columns_present(database).await? {
        // 心跳表不在、或就绪列还没迁移（`0098` 未应用）：不写，也不拿别的列冒充这件事。
        return Ok(());
    }
    sqlx::query(
        "UPDATE collection_scheduler_heartbeat SET readiness_state=$1,readiness_detail=$2, \
             readiness_checked_at=$3::timestamptz WHERE scheduler_key='patrol'",
    )
    .bind(readiness.state.code())
    .bind(readiness.detail.as_deref())
    .bind(readiness.checked_at.as_deref())
    .execute(database.pool())
    .await?;
    Ok(())
}
