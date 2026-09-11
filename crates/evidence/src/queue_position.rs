//! 「我刚才那个请求，现在排在第几」。
//!
//! 发起一次采集之后，工单先进队列，等一个空闲工位来认领。此前界面只说「执行要等一个
//! 空闲工位，可能需要几分钟」——2026-09-10 实测一次关键词建档等了约 3.5 分钟，这段时间
//! 里页面上没有任何东西在动，人无法分辨「在排队」和「没发出去」。
//!
//! 位置必须用派发自己的那把尺子量（`dispatch_order_sql!`）。按创建时间数「前面有几个」
//! 看着合理，但 batch 通道的出队顺序里有一项轮转公平：同一个来源上次发租越久远越靠前。
//! 用错尺子算出来的名次与真实出队顺序无关，那比不报更糟——人会据此判断还要等多久。

use crate::dispatch::{dispatch_order_sql, dispatch_ready_predicate_sql};
use linggan_storage_postgres::Database;
use std::collections::HashMap;
use uuid::Uuid;

/// 一个观察目标当前在队列里的位置。
#[derive(Debug, Clone, Copy)]
pub struct TargetQueuePosition {
    /// **这个目标最近一次排进去的那张工单**，在它自己那条派发通道里前面还排着多少张。
    ///
    /// 不是「这个目标所有工单里排得最靠前的那张」。名次只在同一条通道内可比：
    /// immediate 与 scheduled 各有各的队列、各自独立出队，「在 batch 里排第 1」
    /// 与「在 immediate 里排第 51」之间没有先后关系。跨通道取最小值会把一个目标
    /// 报成「下一个就是它」，而人刚点的那张其实还堵在另一条通道的第 51 位——那正是
    /// 这个功能要消除的那种沉默。
    ///
    /// 取最近创建的那张，因为人问的是「我刚才点的那下排在第几」。
    pub ahead: i64,
    /// 这个目标自己有多少张工单在队列里现在就可以派。
    pub ready: i64,
    /// 还有多少张在等时间——到点才轮得到（定时巡检）或还在失败退避里。
    /// 它们不占当前队列位置，所以单独说，不混进 `ahead`。
    pub waiting_for_time: i64,
}

/// 一批目标各自排在第几。
///
/// 列表页一次要显示很多行，逐行查会变成 N+1。没出现在返回里的目标就是队列里没有它的
/// 工单；读不到则以 `Err` 浮上来，由调用方如实呈现，不压成「没有排队」。
pub async fn read_target_queue_positions(
    database: &Database,
    target_refs: &[Uuid],
) -> Result<HashMap<Uuid, TargetQueuePosition>, sqlx::Error> {
    if target_refs.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, i64, i64, i64)> = sqlx::query_as(concat!(
        "WITH ready AS ( \
             SELECT work_order.work_order_ref,work_order.target_ref,work_order.created_at, \
                    row_number() OVER (PARTITION BY work_order.dispatch_lane ORDER BY ",
        dispatch_order_sql!("work_order.dispatch_lane"),
        ") AS queue_position \
             FROM collection_work_order work_order \
             WHERE ",
        dispatch_ready_predicate_sql!(),
        " ), waiting AS ( \
             SELECT work_order.target_ref,count(*) AS deferred \
             FROM collection_work_order work_order \
             WHERE work_order.queue_state='queued' \
               AND (work_order.retry_not_before_at>scope_001_now() \
                    OR work_order.scheduled_for>scope_001_now()) \
               AND (work_order.expires_at IS NULL OR work_order.expires_at>scope_001_now()) \
             GROUP BY work_order.target_ref \
         ), ready_totals AS ( \
             SELECT target_ref,count(*) AS ready FROM ready \
             WHERE target_ref=ANY($1) GROUP BY target_ref \
         ), mine AS ( \
             SELECT DISTINCT ON (target_ref) target_ref,queue_position \
             FROM ready WHERE target_ref=ANY($1) \
             ORDER BY target_ref,created_at DESC,work_order_ref DESC \
         ) \
         SELECT COALESCE(mine.target_ref,ready_totals.target_ref,waiting.target_ref), \
                COALESCE(mine.queue_position,1)-1,COALESCE(ready_totals.ready,0), \
                COALESCE(waiting.deferred,0) \
         FROM mine \
         FULL OUTER JOIN ready_totals USING(target_ref) \
         FULL OUTER JOIN waiting USING(target_ref) \
         WHERE COALESCE(mine.target_ref,ready_totals.target_ref,waiting.target_ref)=ANY($1)",
    ))
    .bind(target_refs)
    .fetch_all(database.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|(target_ref, ahead, ready, waiting_for_time)| {
            (
                target_ref,
                TargetQueuePosition {
                    ahead,
                    ready,
                    waiting_for_time,
                },
            )
        })
        .collect())
}
