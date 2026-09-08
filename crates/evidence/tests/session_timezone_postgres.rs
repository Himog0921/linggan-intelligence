//! 会话时区必须是 Asia/Shanghai。
//!
//! 页面上每一个时间都从这条连接读出来格式化。此前会话是 UTC，而页头写着「中国标准
//! 时间 UTC+08」——一个还有一小时才到的巡检时间会显示成早已过去，且不会有任何报错。

use linggan_storage_postgres::Database;

/// sqlx 建立连接时把 `TimeZone=UTC` 写死在自己的 startup 参数里，传同名参数会被它
/// 覆盖，所以时区只能在连接建立之后设。这条断言真的连一次库确认结果，而不是检查源码
/// 里写了什么——写法可以变，会话时区不能变。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn every_pooled_connection_reads_time_in_shanghai() {
    let url = std::env::var("SCOPE_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    let database = Database::connect(&url).await.expect("connects");

    let timezone: String = sqlx::query_scalar("SHOW TimeZone")
        .fetch_one(database.pool())
        .await
        .expect("reads the session timezone");
    assert_eq!(timezone, "Asia/Shanghai");

    // 存储不受影响：同一时刻的绝对值不变，只是读出来的写法不同。
    let (shanghai, utc): (String, String) = sqlx::query_as(
        "SELECT to_char(timestamptz '2026-09-08 03:30:00+00', 'MM-DD HH24:MI'), \
                to_char(timestamptz '2026-09-08 03:30:00+00' AT TIME ZONE 'UTC', 'MM-DD HH24:MI')",
    )
    .fetch_one(database.pool())
    .await
    .expect("formats both ways");
    assert_eq!(shanghai, "09-08 11:30", "读出来应当是北京时间");
    assert_eq!(utc, "09-08 03:30", "同一时刻的 UTC 值不变");
}
