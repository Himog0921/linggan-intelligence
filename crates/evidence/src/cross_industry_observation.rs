//! CORPUS-CROSS-INDUSTRY-001 · 一篇跨行业样本的每一次被观察。
//!
//! `cross_industry_sample` 是一篇一行：那张表回答「这篇笔记是什么」，靠 upsert 保持当前
//! 最好的一份事实。它答不了「这篇被看到过几次、热度怎么变」——每次观察都把上一次覆盖掉
//! 了。巡检一周跑一次，覆盖掉的正是唯一一份上周的读数，而平台不提供历史值，补不回来。
//!
//! 本行业侧不需要这一层：那边每一轮采集都产生一条新的 discovery_finding，时间序列天然
//! 存在。跨行业侧走 upsert，于是必须把每一次观察单独记下来（`0073`）。

use crate::cross_industry_admission::SamplingProvenance;
use crate::producer_runtime::ProducerRuntimeError;
use linggan_contracts::ProducerCapturePackage;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// 记下这一次观察：什么时候、从哪个词的哪个榜、当时读到多少赞。
///
/// 样本行上的口径只留得下**最近一次**是从哪个维度看到它的：同一个词的综合榜和点赞榜
/// 完全可能收录同一篇，后一轮会把前一轮的口径整套换掉。而「同时上了几个榜」恰恰是
/// 识别真爆款的信号，也正是关键词建档想要的东西。
///
/// 所以按维度逐条留痕，一篇一榜一行。不复制样本事实——标题、互动数、链接仍然只有一份，
/// 这里只回答出现过与否、第一次和最近一次是什么时候。
///
/// 没有口径的那一轮（详情面、评论面，以及未绑定规则的一次性请求）不写：它们不是从
/// 某个榜上看到这篇的。
pub(crate) async fn record_sample_observation(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    sample_ref: Uuid,
    domain_ref: Uuid,
    sampling: Option<&SamplingProvenance>,
    reading: &ObservationReading,
) -> Result<(), ProducerRuntimeError> {
    let Some(sampling) = sampling else {
        return Ok(());
    };
    let schema_ready: bool =
        sqlx::query_scalar("SELECT to_regclass('cross_industry_sample_observation') IS NOT NULL")
            .fetch_one(&mut **tx)
            .await
            .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        return Ok(());
    }
    // 同一个包里同一篇只观察一次；重复提交同一个包不该被记成两次观察。
    sqlx::query(
        "INSERT INTO cross_industry_sample_observation \
             (observation_ref,sample_ref,domain_ref,package_ref,keyword,sort_order, \
              like_count,comment_count,collect_count,discovery_order) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) \
         ON CONFLICT (sample_ref,package_ref) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(sample_ref)
    .bind(domain_ref)
    .bind(package.package_ref())
    .bind(sampling.keyword.as_str())
    .bind(sampling.sort_order.as_str())
    .bind(reading.like_count)
    .bind(reading.comment_count)
    .bind(reading.collect_count)
    .bind(reading.discovery_order)
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

/// 记下「这篇作品是从这个创作者主页发现的」。
///
/// 目标不复制进关系行：沿 Package → Lease Task → Lease → WorkOrder 可以唯一回到目标。
/// 这里保存的只是那条已接纳 record 与样本之间的追加式联系。关键词榜单仍使用上面的
/// `cross_industry_sample_observation`，两种来源不会靠假默认值混成一张表。
pub(crate) async fn record_creator_sample_observation(
    tx: &mut Transaction<'_, Postgres>,
    package: &ProducerCapturePackage,
    sample_ref: Uuid,
    domain_ref: Uuid,
    record_ordinal: i32,
    discovery_order: Option<i32>,
) -> Result<(), ProducerRuntimeError> {
    if package.package_kind() != "profile_discovery" {
        return Ok(());
    }
    let schema_ready: bool = sqlx::query_scalar(
        "SELECT to_regclass('cross_industry_creator_sample_observation') IS NOT NULL",
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    if !schema_ready {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO cross_industry_creator_sample_observation \
             (observation_ref,sample_ref,domain_ref,package_ref,record_ordinal, \
              discovery_order,observed_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7) \
         ON CONFLICT DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(sample_ref)
    .bind(domain_ref)
    .bind(package.package_ref())
    .bind(record_ordinal)
    .bind(discovery_order)
    .bind(package.observed_at())
    .execute(&mut **tx)
    .await
    .map_err(ProducerRuntimeError::Internal)?;
    Ok(())
}

/// 这一次观察读到的东西。与样本行上的「当前最好的一份事实」分开：**「这篇现在有多少赞」
/// 与「上周五看到它时有多少赞」是两个事实，后者推不出前者，也不该被前者覆盖。**
pub(crate) struct ObservationReading {
    pub like_count: Option<i64>,
    pub comment_count: Option<i64>,
    pub collect_count: Option<i64>,
    /// 插件报告的 `_discoveryOrder`，原样保存（从 0 起计）。
    pub discovery_order: Option<i32>,
}
