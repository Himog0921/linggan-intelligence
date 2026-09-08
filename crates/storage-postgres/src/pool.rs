//! The one place that owns a PostgreSQL connection pool for this workspace.

use std::str::FromStr;

use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::error::StorageError;

/// A connected PostgreSQL database. Modules that own an invariant borrow this and keep their
/// own SQL private; there is no shared CRUD repository.
#[derive(Debug, Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(url: &str) -> Result<Self, StorageError> {
        Self::connect_with(url, None).await
    }

    /// Connects with every pooled connection pinned to one schema. SCOPE-001 uses this so each
    /// proof runs against its own isolated copy of the migration.
    pub async fn connect_within_schema(url: &str, schema: &str) -> Result<Self, StorageError> {
        Self::connect_with(url, Some(schema)).await
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    async fn connect_with(url: &str, schema: Option<&str>) -> Result<Self, StorageError> {
        let mut options = PgConnectOptions::from_str(url).map_err(StorageError::Connect)?;
        if let Some(schema) = schema {
            options = options.options([("search_path", validated_schema_name(schema)?)]);
        }
        // 每条连接都固定在 Asia/Shanghai。
        //
        // 时间**存储**不受影响：`timestamptz` 存的是绝对时刻，`scope_001_now()` 也是，
        // 会话时区只决定读出来时怎么格式化。此前所有 `to_char` 与 `::text` 都直接印出
        // 数据库的 UTC 值，而页面自己写着「中国标准时间 UTC+08」——差 8 小时，一个还没到
        // 的巡检时间会显示成已经过去。
        //
        // 放在这里而不是逐条 SQL 加 `AT TIME ZONE`：那有 19 处 `to_char` 与 59 处 `::text`，
        // 漏一处就是一个错的时间，而且下一个新增的读取还会再漏一次。连接池是这个工作区
        // 唯一的连接入口，在这里定一次，往后所有读取自动正确。
        //
        // 必须用 `after_connect` 而不是 startup 参数：sqlx 建立连接时把 `TimeZone=UTC`
        // 写死在自己的 startup 里（`sqlx-postgres/src/connection/establish.rs`），
        // 传进去的同名参数会被它覆盖掉——试过，会话时区仍然是 UTC。
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .after_connect(|connection, _meta| {
                Box::pin(async move {
                    sqlx::query("SET TIME ZONE 'Asia/Shanghai'")
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect_with(options)
            .await
            .map_err(StorageError::Connect)?;
        Ok(Self { pool })
    }
}

/// Schema names reach SQL as identifiers rather than bind parameters, so the accepted shape is
/// narrow on purpose.
pub(crate) fn validated_schema_name(schema: &str) -> Result<&str, StorageError> {
    let usable = !schema.is_empty()
        && schema.len() <= 48
        && schema.starts_with(|character: char| character.is_ascii_lowercase())
        && schema.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if usable {
        Ok(schema)
    } else {
        Err(StorageError::UnusableSchemaName(schema.to_owned()))
    }
}
