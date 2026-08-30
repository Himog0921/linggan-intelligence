use crate::{
    AgentContractError, CandidateAnalysisInvocation, CandidateAnalysisOutput, ExecutionOutcome,
    ModelToolLoopError, ModelToolLoopPort, TopicMaterialRole, validate_candidate_output,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::fmt::Write;
use std::future::Future;
use std::pin::Pin;
use thiserror::Error;
use uuid::Uuid;

pub type AgentRuntimeFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentInvocationState {
    Accepted,
    Running,
    Succeeded,
    Cancelled,
    Rejected,
    Failed,
}

impl AgentInvocationState {
    fn parse(value: &str) -> Result<Self, AgentRuntimeError> {
        match value {
            "accepted" => Ok(Self::Accepted),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "cancelled" => Ok(Self::Cancelled),
            "rejected" => Ok(Self::Rejected),
            "failed" => Ok(Self::Failed),
            _ => Err(AgentRuntimeError::CorruptLedger),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInvocationReceipt {
    pub invocation_ref: Uuid,
    pub state: AgentInvocationState,
    pub adapter_identity: String,
    pub input_sha256: String,
    pub output: Option<CandidateAnalysisOutput>,
    pub failure_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AgentRuntimeError {
    #[error(transparent)]
    Contract(#[from] AgentContractError),
    #[error("Agent invocation idempotency key was reused for different input")]
    IdempotencyConflict,
    #[error("frozen Topic Material Pack does not match the authoritative Topic workspace")]
    MaterialPackMismatch,
    #[error("unknown Agent invocation {0}")]
    UnknownInvocation(Uuid),
    #[error("Agent invocation cannot transition from {0:?}")]
    InvalidState(AgentInvocationState),
    #[error("Agent invocation ledger is internally inconsistent")]
    CorruptLedger,
    #[error("Agent runtime database unavailable: {0}")]
    Database(String),
}

pub trait AgentRuntime {
    fn submit<'a>(
        &'a self,
        invocation: &'a CandidateAnalysisInvocation,
    ) -> AgentRuntimeFuture<'a, Result<AgentInvocationReceipt, AgentRuntimeError>>;

    fn status(
        &self,
        invocation_ref: Uuid,
    ) -> AgentRuntimeFuture<'_, Result<AgentInvocationReceipt, AgentRuntimeError>>;

    fn cancel(
        &self,
        invocation_ref: Uuid,
        actor_ref: Uuid,
        reason: &str,
    ) -> AgentRuntimeFuture<'_, Result<AgentInvocationReceipt, AgentRuntimeError>>;
}

pub struct PostgresAgentRuntime<'a, P> {
    database: &'a Database,
    port: P,
}

impl<'a, P> PostgresAgentRuntime<'a, P> {
    pub fn new(database: &'a Database, port: P) -> Self {
        Self { database, port }
    }
}

impl<P: ModelToolLoopPort + Sync> AgentRuntime for PostgresAgentRuntime<'_, P> {
    fn submit<'a>(
        &'a self,
        invocation: &'a CandidateAnalysisInvocation,
    ) -> AgentRuntimeFuture<'a, Result<AgentInvocationReceipt, AgentRuntimeError>> {
        Box::pin(async move { self.submit_inner(invocation).await })
    }

    fn status(
        &self,
        invocation_ref: Uuid,
    ) -> AgentRuntimeFuture<'_, Result<AgentInvocationReceipt, AgentRuntimeError>> {
        Box::pin(async move { read_receipt(self.database, invocation_ref).await })
    }

    fn cancel(
        &self,
        invocation_ref: Uuid,
        actor_ref: Uuid,
        reason: &str,
    ) -> AgentRuntimeFuture<'_, Result<AgentInvocationReceipt, AgentRuntimeError>> {
        let reason = reason.to_owned();
        Box::pin(async move { self.cancel_inner(invocation_ref, actor_ref, &reason).await })
    }
}

impl<P: ModelToolLoopPort + Sync> PostgresAgentRuntime<'_, P> {
    async fn submit_inner(
        &self,
        invocation: &CandidateAnalysisInvocation,
    ) -> Result<AgentInvocationReceipt, AgentRuntimeError> {
        invocation.execution_request()?;
        ensure_authoritative_pack(self.database, invocation).await?;
        let request_sha256 = request_hash(invocation)?;
        if let Some(receipt) = replay_by_key(self.database, invocation, &request_sha256).await? {
            return Ok(receipt);
        }
        let invocation_ref = Uuid::new_v4();
        let inserted = insert_invocation(
            self.database,
            invocation_ref,
            invocation,
            &request_sha256,
            self.port.adapter_identity(),
        )
        .await?;
        if !inserted {
            return replay_by_key(self.database, invocation, &request_sha256)
                .await?
                .ok_or(AgentRuntimeError::CorruptLedger);
        }
        read_receipt(self.database, invocation_ref).await
    }

    pub async fn execute(
        &self,
        invocation_ref: Uuid,
    ) -> Result<AgentInvocationReceipt, AgentRuntimeError> {
        let invocation = claim_for_execution(self.database, invocation_ref).await?;
        let request = invocation.execution_request_with_ref(invocation_ref)?;
        let outcome = self.port.run(&request);
        finalize_execution(self.database, &request, outcome).await?;
        read_receipt(self.database, invocation_ref).await
    }

    pub async fn recover_interrupted(
        &self,
        invocation_ref: Uuid,
    ) -> Result<AgentInvocationReceipt, AgentRuntimeError> {
        let changed = sqlx::query(
            "UPDATE linggan_agent_invocation SET state='failed',failure_code='interrupted_before_finalize', \
             finalized_at=scope_001_now(),updated_at=scope_001_now() \
             WHERE invocation_ref=$1 AND state='running'",
        )
        .bind(invocation_ref)
        .execute(self.database.pool())
        .await
        .map_err(database_error)?
        .rows_affected();
        if changed == 0 {
            let receipt = read_receipt(self.database, invocation_ref).await?;
            return Err(AgentRuntimeError::InvalidState(receipt.state));
        }
        read_receipt(self.database, invocation_ref).await
    }

    async fn cancel_inner(
        &self,
        invocation_ref: Uuid,
        actor_ref: Uuid,
        reason: &str,
    ) -> Result<AgentInvocationReceipt, AgentRuntimeError> {
        if reason.trim().is_empty() || reason.len() > 500 {
            return Err(AgentRuntimeError::Contract(
                AgentContractError::InvalidInvocation("invalid cancellation reason"),
            ));
        }
        let failure_code = format!("cancelled_by:{actor_ref}:{reason}");
        let changed = sqlx::query(
            "UPDATE linggan_agent_invocation SET state='cancelled',failure_code=$2, \
             finalized_at=scope_001_now(),updated_at=scope_001_now() \
             WHERE invocation_ref=$1 AND state IN ('accepted','running')",
        )
        .bind(invocation_ref)
        .bind(failure_code)
        .execute(self.database.pool())
        .await
        .map_err(database_error)?
        .rows_affected();
        if changed == 0 {
            let receipt = read_receipt(self.database, invocation_ref).await?;
            return Err(AgentRuntimeError::InvalidState(receipt.state));
        }
        read_receipt(self.database, invocation_ref).await
    }
}

async fn ensure_authoritative_pack(
    database: &Database,
    invocation: &CandidateAnalysisInvocation,
) -> Result<(), AgentRuntimeError> {
    let pack = &invocation.material_pack;
    let identity_matches: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_topic_material_pack pack \
         JOIN linggan_topic_classification_run run USING(classification_run_ref) \
         JOIN linggan_topic_definition definition USING(definition_ref) \
         JOIN linggan_topic_workspace topic USING(topic_ref) \
         WHERE pack.material_pack_ref=$1 AND run.classification_run_ref=$2 \
           AND definition.definition_ref=$3 AND definition.version=$4 AND topic.topic_ref=$5 \
           AND pack.source_boundary=$6)",
    )
    .bind(pack.material_pack_ref)
    .bind(pack.classification_run_ref)
    .bind(pack.definition_ref)
    .bind(pack.definition_version)
    .bind(pack.topic_ref)
    .bind(&pack.source_boundary)
    .fetch_one(database.pool())
    .await
    .map_err(database_error)?;
    if !identity_matches || !members_match(database, invocation).await? {
        return Err(AgentRuntimeError::MaterialPackMismatch);
    }
    Ok(())
}

async fn members_match(
    database: &Database,
    invocation: &CandidateAnalysisInvocation,
) -> Result<bool, AgentRuntimeError> {
    let rows = sqlx::query(
        "SELECT work_public_ref,role,rationale FROM linggan_topic_material_member \
         WHERE classification_run_ref=$1 ORDER BY ordinal",
    )
    .bind(invocation.material_pack.classification_run_ref)
    .fetch_all(database.pool())
    .await
    .map_err(database_error)?;
    if rows.len() != invocation.material_pack.members.len() {
        return Ok(false);
    }
    Ok(rows
        .iter()
        .zip(&invocation.material_pack.members)
        .all(|(row, member)| {
            row.get::<Uuid, _>("work_public_ref") == member.work_public_ref
                && row.get::<String, _>("role") == role_name(member.role)
                && row.get::<String, _>("rationale") == member.rationale
        }))
}

fn role_name(role: TopicMaterialRole) -> &'static str {
    match role {
        TopicMaterialRole::Support => "support",
        TopicMaterialRole::Challenge => "challenge",
        TopicMaterialRole::Boundary => "boundary",
    }
}

async fn replay_by_key(
    database: &Database,
    invocation: &CandidateAnalysisInvocation,
    request_sha256: &str,
) -> Result<Option<AgentInvocationReceipt>, AgentRuntimeError> {
    let row = sqlx::query(
        "SELECT invocation_ref,request_sha256 FROM linggan_agent_invocation WHERE idempotency_key=$1",
    )
    .bind(&invocation.idempotency_key)
    .fetch_optional(database.pool())
    .await
    .map_err(database_error)?;
    let Some(row) = row else { return Ok(None) };
    if row.get::<String, _>("request_sha256") != request_sha256 {
        return Err(AgentRuntimeError::IdempotencyConflict);
    }
    let invocation_ref = row.get("invocation_ref");
    read_receipt(database, invocation_ref).await.map(Some)
}

async fn insert_invocation(
    database: &Database,
    invocation_ref: Uuid,
    invocation: &CandidateAnalysisInvocation,
    request_sha256: &str,
    adapter_identity: &str,
) -> Result<bool, AgentRuntimeError> {
    let frozen_request = serde_json::to_value(invocation).map_err(json_error)?;
    let budget = serde_json::to_value(&invocation.budget).map_err(json_error)?;
    let pack = &invocation.material_pack;
    sqlx::query(
        "INSERT INTO linggan_agent_invocation( \
         invocation_ref,idempotency_key,request_sha256,actor_ref,delegation_revision,purpose, \
         profile_key,profile_version,topic_ref,definition_ref,definition_version, \
         classification_run_ref,material_pack_ref,frozen_request,budget,adapter_identity,state) \
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,'accepted') \
         ON CONFLICT (idempotency_key) DO NOTHING",
    )
    .bind(invocation_ref)
    .bind(&invocation.idempotency_key)
    .bind(request_sha256)
    .bind(invocation.actor_ref)
    .bind(&invocation.delegation_revision)
    .bind(&invocation.purpose)
    .bind(&invocation.profile_key)
    .bind(invocation.profile_version)
    .bind(pack.topic_ref)
    .bind(pack.definition_ref)
    .bind(pack.definition_version)
    .bind(pack.classification_run_ref)
    .bind(pack.material_pack_ref)
    .bind(frozen_request)
    .bind(budget)
    .bind(adapter_identity)
    .execute(database.pool())
    .await
    .map_err(database_error)
    .map(|result| result.rows_affected() == 1)
}

async fn claim_for_execution(
    database: &Database,
    invocation_ref: Uuid,
) -> Result<CandidateAnalysisInvocation, AgentRuntimeError> {
    let mut transaction = database.pool().begin().await.map_err(database_error)?;
    let row = sqlx::query(
        "UPDATE linggan_agent_invocation SET state='running',started_at=scope_001_now(), \
         updated_at=scope_001_now() WHERE invocation_ref=$1 AND state='accepted' \
         RETURNING frozen_request",
    )
    .bind(invocation_ref)
    .fetch_optional(&mut *transaction)
    .await
    .map_err(database_error)?;
    if let Some(row) = row {
        let invocation: CandidateAnalysisInvocation =
            serde_json::from_value(row.get("frozen_request")).map_err(json_error)?;
        sqlx::query(
            "INSERT INTO linggan_agent_tool_receipt( \
             receipt_ref,invocation_ref,ordinal,tool_name,resource_ref,outcome) \
             VALUES($1,$2,1,'read_topic_material_pack',$3,'succeeded')",
        )
        .bind(Uuid::new_v4())
        .bind(invocation_ref)
        .bind(invocation.material_pack.material_pack_ref)
        .execute(&mut *transaction)
        .await
        .map_err(database_error)?;
        transaction.commit().await.map_err(database_error)?;
        return Ok(invocation);
    }
    transaction.rollback().await.map_err(database_error)?;
    let receipt = read_receipt(database, invocation_ref).await?;
    Err(AgentRuntimeError::InvalidState(receipt.state))
}

async fn finalize_execution(
    database: &Database,
    request: &crate::ExecutionRequest,
    outcome: Result<ExecutionOutcome, ModelToolLoopError>,
) -> Result<(), AgentRuntimeError> {
    match outcome {
        Ok(outcome) => match validate_candidate_output(request, &outcome) {
            Ok(()) => finalize_success(database, request, &outcome).await,
            Err(error) => {
                finalize_failure(
                    database,
                    request.invocation_ref,
                    "output_rejected",
                    "rejected",
                )
                .await?;
                Err(error.into())
            }
        },
        Err(_) => {
            finalize_failure(database, request.invocation_ref, "adapter_failed", "failed").await?;
            Ok(())
        }
    }
}

async fn finalize_success(
    database: &Database,
    request: &crate::ExecutionRequest,
    outcome: &ExecutionOutcome,
) -> Result<(), AgentRuntimeError> {
    let output = serde_json::to_value(&outcome.output).map_err(json_error)?;
    let mut transaction = database.pool().begin().await.map_err(database_error)?;
    let changed = sqlx::query(
        "UPDATE linggan_agent_invocation SET state='succeeded',candidate_output=$2, \
         finalized_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(request.invocation_ref)
    .bind(output)
    .execute(&mut *transaction)
    .await
    .map_err(database_error)?
    .rows_affected();
    if changed != 1 {
        transaction.rollback().await.map_err(database_error)?;
        let receipt = read_receipt(database, request.invocation_ref).await?;
        return Err(AgentRuntimeError::InvalidState(receipt.state));
    }
    transaction.commit().await.map_err(database_error)?;
    Ok(())
}

async fn finalize_failure(
    database: &Database,
    invocation_ref: Uuid,
    failure_code: &str,
    state: &str,
) -> Result<(), AgentRuntimeError> {
    sqlx::query(
        "UPDATE linggan_agent_invocation SET state=$2,failure_code=$3, \
         finalized_at=scope_001_now(),updated_at=scope_001_now() \
         WHERE invocation_ref=$1 AND state='running'",
    )
    .bind(invocation_ref)
    .bind(state)
    .bind(failure_code)
    .execute(database.pool())
    .await
    .map_err(database_error)?;
    Ok(())
}

async fn read_receipt(
    database: &Database,
    invocation_ref: Uuid,
) -> Result<AgentInvocationReceipt, AgentRuntimeError> {
    let row = sqlx::query(
        "SELECT state,adapter_identity,request_sha256,candidate_output,failure_code \
         FROM linggan_agent_invocation WHERE invocation_ref=$1",
    )
    .bind(invocation_ref)
    .fetch_optional(database.pool())
    .await
    .map_err(database_error)?
    .ok_or(AgentRuntimeError::UnknownInvocation(invocation_ref))?;
    let output = row
        .get::<Option<serde_json::Value>, _>("candidate_output")
        .map(serde_json::from_value)
        .transpose()
        .map_err(json_error)?;
    Ok(AgentInvocationReceipt {
        invocation_ref,
        state: AgentInvocationState::parse(&row.get::<String, _>("state"))?,
        adapter_identity: row.get("adapter_identity"),
        input_sha256: row.get("request_sha256"),
        output,
        failure_code: row.get("failure_code"),
    })
}

fn request_hash(invocation: &CandidateAnalysisInvocation) -> Result<String, AgentRuntimeError> {
    let bytes = serde_json::to_vec(invocation).map_err(json_error)?;
    let mut hash = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        write!(&mut hash, "{byte:02x}")
            .map_err(|error| AgentRuntimeError::Database(error.to_string()))?;
    }
    Ok(hash)
}

fn json_error(error: serde_json::Error) -> AgentRuntimeError {
    AgentRuntimeError::Database(error.to_string())
}

fn database_error(error: sqlx::Error) -> AgentRuntimeError {
    AgentRuntimeError::Database(error.to_string())
}
