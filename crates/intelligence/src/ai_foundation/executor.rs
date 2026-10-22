//! Live session ownership and transaction fences; owning a process never grants model permission.
use linggan_contracts::ai_limits::AI_LIMITS;
use linggan_storage_postgres::Database;
use sqlx::{Connection, PgConnection, Postgres, Row, Transaction};
use uuid::Uuid;

mod budget;
mod dispatch;
pub mod ocr_audit;
pub mod ocr_recognition;
mod preparation;
pub mod probe;
pub mod queue;
pub(crate) mod read;
mod receipt;
mod schema;
pub mod service;
pub mod writing;

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("ai_executor_already_running")]
    Busy,
    #[error("ai_executor_fence_lost")]
    FenceLost,
    #[error("ai_executor_invalid")]
    Invalid,
    #[error("ai_executor_schema_not_ready")]
    SchemaNotReady,
    #[error("ai_executor_drain_timed_out")]
    DrainTimedOut,
    #[error("ai_executor_task_failed")]
    TaskFailed,
    #[error("ai_executor_receipt_timed_out")]
    ReceiptTimedOut,
    #[error("ai_executor_heartbeat_timed_out")]
    HeartbeatTimedOut,
    #[error("ai_executor_database_unavailable")]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Configuration(#[from] super::store::FoundationError),
    #[error("ai_executor_model_unavailable")]
    Model(#[from] crate::model_settings::ModelError),
    #[error("ai_executor_writing_unavailable")]
    Writing(#[from] crate::content_workbench_ai::AiError),
}
impl ExecutorError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Busy => "ai_executor_already_running",
            Self::FenceLost => "ai_executor_fence_lost",
            Self::Invalid => "ai_executor_invalid",
            Self::SchemaNotReady => "ai_executor_schema_not_ready",
            Self::DrainTimedOut => "ai_executor_drain_timed_out",
            Self::TaskFailed => "ai_executor_task_failed",
            Self::ReceiptTimedOut => "ai_executor_receipt_timed_out",
            Self::HeartbeatTimedOut => "ai_executor_heartbeat_timed_out",
            Self::Database(_) => "ai_executor_database_unavailable",
            Self::Configuration(_) => "ai_executor_configuration_unavailable",
            Self::Model(error) => error.code(),
            Self::Writing(error) => error.code(),
        }
    }
    pub(super) fn is_database(&self) -> bool {
        matches!(
            self,
            Self::Database(_)
                | Self::Configuration(super::store::FoundationError::Database(_))
                | Self::Model(crate::model_settings::ModelError::Database(_))
                | Self::Writing(crate::content_workbench_ai::AiError::Database(_))
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutorFence {
    instance_ref: Uuid,
    generation: i64,
}
impl ExecutorFence {
    pub fn instance_ref(self) -> Uuid {
        self.instance_ref
    }
    pub fn generation(self) -> i64 {
        self.generation
    }
    /// All executor mutations use this boundary, including writes after a provider await.
    /// The shared row lock prevents a replacement generation from racing the transaction.
    pub async fn begin<'a>(
        self,
        db: &'a Database,
        require_running: bool,
    ) -> Result<Transaction<'a, Postgres>, ExecutorError> {
        self.begin_mode(db, require_running, false).await
    }
    async fn begin_mode<'a>(
        self,
        db: &'a Database,
        require_running: bool,
        recovery: bool,
    ) -> Result<Transaction<'a, Postgres>, ExecutorError> {
        let mut tx = db.pool().begin().await?;
        let owned: bool = sqlx::query_scalar(
            "SELECT linggan_ai_executor_is_owner($1,$2,$3) FROM linggan_ai_executor_current c \
             JOIN linggan_ai_executor_instance i USING(instance_ref,generation) \
             WHERE c.instance_ref=$1 AND c.generation=$2 FOR SHARE OF c,i",
        )
        .bind(self.instance_ref)
        .bind(self.generation)
        .bind(require_running)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(false);
        if !owned {
            return Err(ExecutorError::FenceLost);
        }
        sqlx::query(
            "SELECT set_config('linggan.ai_executor_instance',$1,true),\
             set_config('linggan.ai_executor_generation',$2,true),\
             set_config('linggan.ai_executor_recovery',$3,true)",
        )
        .bind(self.instance_ref.to_string())
        .bind(self.generation.to_string())
        .bind(if recovery { "true" } else { "false" })
        .execute(&mut *tx)
        .await?;
        Ok(tx)
    }
}

/// Detached from the pool: Drop closes the socket, so a lock can never leak to another borrower.
pub struct ExecutorSession {
    connection: PgConnection,
    fence: ExecutorFence,
}
impl ExecutorSession {
    pub async fn acquire(
        db: &Database,
        source_revision: &str,
        concurrency: u32,
    ) -> Result<Self, ExecutorError> {
        if !(AI_LIMITS.default_concurrency..=AI_LIMITS.max_concurrency).contains(&concurrency)
            || source_revision.trim().is_empty()
            || source_revision.len() > 128
            || source_revision.chars().any(char::is_control)
        {
            return Err(ExecutorError::Invalid);
        }
        let mut connection = db.pool().acquire().await?.detach();
        let locked: bool =
            sqlx::query_scalar("SELECT pg_try_advisory_lock(linggan_ai_executor_lock_key())")
                .fetch_one(&mut connection)
                .await?;
        if !locked {
            return Err(ExecutorError::Busy);
        }
        let mut tx = connection.begin().await?;
        let row = sqlx::query(
            "SELECT instance_ref,generation FROM linggan_ai_executor_current WHERE singleton FOR UPDATE",
        )
        .fetch_one(&mut *tx)
        .await?;
        let generation = row
            .get::<i64, _>("generation")
            .checked_add(1)
            .ok_or(ExecutorError::Invalid)?;
        if let Some(old) = row.get::<Option<Uuid>, _>("instance_ref") {
            sqlx::query("UPDATE linggan_ai_executor_instance SET state='lost' WHERE instance_ref=$1 AND state IN ('running','draining')")
                .bind(old).execute(&mut *tx).await?;
        }
        let instance_ref = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_ai_executor_instance(instance_ref,generation,owner_backend_pid,process_id,source_revision,concurrency,state) VALUES($1,$2,pg_backend_pid(),$3,$4,$5,'running')")
            .bind(instance_ref).bind(generation).bind(std::process::id() as i32)
            .bind(source_revision).bind(concurrency as i32).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE linggan_ai_executor_current SET instance_ref=$1,generation=$2 WHERE singleton",
        )
        .bind(instance_ref)
        .bind(generation)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Self {
            connection,
            fence: ExecutorFence {
                instance_ref,
                generation,
            },
        })
    }
    pub fn fence(&self) -> ExecutorFence {
        self.fence
    }
    pub async fn heartbeat(&mut self) -> Result<(), ExecutorError> {
        let changed=sqlx::query("UPDATE linggan_ai_executor_instance SET heartbeat_at=clock_timestamp() WHERE instance_ref=$1 AND linggan_ai_executor_is_owner($1,$2,false)")
            .bind(self.fence.instance_ref).bind(self.fence.generation).execute(&mut self.connection).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        Ok(())
    }
    pub async fn request_drain(&mut self) -> Result<(), ExecutorError> {
        let changed=sqlx::query("UPDATE linggan_ai_executor_instance SET state='draining',drain_requested_at=COALESCE(drain_requested_at,clock_timestamp()),heartbeat_at=clock_timestamp() WHERE instance_ref=$1 AND linggan_ai_executor_is_owner($1,$2,false)")
            .bind(self.fence.instance_ref).bind(self.fence.generation).execute(&mut self.connection).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        Ok(())
    }
    /// A success acknowledgement may be written only after this database transition succeeds.
    pub async fn complete_drain(&mut self) -> Result<(), ExecutorError> {
        let changed=sqlx::query("UPDATE linggan_ai_executor_instance i SET state='drained',drained_at=clock_timestamp(),heartbeat_at=clock_timestamp()
            WHERE instance_ref=$1 AND state='draining' AND linggan_ai_executor_is_owner($1,$2,false)
             AND NOT EXISTS(SELECT 1 FROM linggan_ai_batch_item_control WHERE lease_instance_ref=$1 AND state='running')")
            .bind(self.fence.instance_ref).bind(self.fence.generation).execute(&mut self.connection).await?;
        if changed.rows_affected() != 1 {
            return Err(ExecutorError::FenceLost);
        }
        Ok(())
    }
}
