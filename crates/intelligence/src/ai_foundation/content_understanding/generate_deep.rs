use super::{Command, Database, FoundationError, Receipt, TaskKey, Uuid, Value, admission, deep};
use crate::ai_foundation::{
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::valid_reason,
    task_config::lock_setting,
};
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction};

pub(crate) const METHOD: &str = "content-understanding.l2.model.v1";
use super::deep_grounding::accept;
pub(crate) use super::deep_grounding::{INSUFFICIENT_EVIDENCE, response_schema};
const TASK: TaskKey = TaskKey::ContentUnderstandingDeep;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateDeep {
    command_ref: Uuid,
    domain_ref: Uuid,
    work_ref: Uuid,
    core_ref: Uuid,
    question: String,
    task_config_ref: Uuid,
    task_revision: i64,
}

pub async fn generate_deep(db: &Database, r: &GenerateDeep) -> Result<Receipt, FoundationError> {
    if [r.command_ref, r.domain_ref, r.work_ref, r.core_ref]
        .iter()
        .any(Uuid::is_nil)
        || r.task_revision < 0
        || !valid_reason(&r.question, 300)
    {
        return Err(FoundationError::Invalid);
    }
    let mut c = Command::begin(db, r.command_ref, TASK.as_str(), r).await?;
    if let Some(receipt) = c.replay()? {
        return Ok(receipt);
    }
    let config = lock_setting(&mut c.tx, TASK, r.task_revision).await?;
    if config != r.task_config_ref {
        return Err(FoundationError::Conflict);
    }
    let input = InputRef::ContentUnderstandingDeep {
        request_ref: r.command_ref,
    };
    admission::check_queue_configuration(&mut c.tx, TASK, config, false).await?;
    admission::check_queue_input_scope(&mut c.tx, TASK, config, &input).await?;
    deep::lock_core(&mut c.tx, r.core_ref).await?;
    if deep::current_card_in(&mut c.tx, r.domain_ref, r.work_ref, r.core_ref)
        .await?
        .is_none()
    {
        return Err(FoundationError::Conflict);
    }
    let question = r.question.trim();
    let existing = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref,c.failure_code
        FROM linggan_content_understanding_deep_request r JOIN linggan_ai_batch_item_control c USING(item_ref)
        WHERE r.core_ref=$1 AND r.domain_ref=$2 AND r.question=$3
          AND (c.state IN ('pending','running') OR c.failure_code='ai_dispatch_unknown')
        ORDER BY (c.failure_code IS NOT DISTINCT FROM 'ai_dispatch_unknown') DESC LIMIT 1")
        .bind(r.core_ref).bind(r.domain_ref).bind(question).fetch_optional(&mut *c.tx).await?;
    if let Some(existing) = existing {
        if existing.get::<Option<String>, _>("failure_code").as_deref()
            == Some("ai_dispatch_unknown")
        {
            return Err(FoundationError::AdmissionDenied(vec![
                "ai_prior_dispatch_unresolved".into(),
            ]));
        }
        return c
            .finish(Receipt {
                request_ref: existing.get("request_ref"),
                batch_ref: existing.get("batch_ref"),
                item_ref: existing.get("item_ref"),
                reused: true,
            })
            .await;
    }
    let inputs = [BatchInput {
        item_key: r.command_ref.to_string(),
        input,
    }];
    let hash = batch_store::input_hash(TASK, &inputs, false)?;
    let batch = batch_store::insert_batch(
        &mut c.tx,
        r.command_ref,
        TASK,
        config,
        BatchKind::Interactive,
        &inputs,
        &hash,
    )
    .await?;
    let item = sqlx::query_scalar("SELECT item_ref FROM linggan_ai_batch_item WHERE batch_ref=$1")
        .bind(batch)
        .fetch_one(&mut *c.tx)
        .await?;
    sqlx::query("INSERT INTO linggan_content_understanding_deep_request(request_ref,domain_ref,core_ref,question,task_config_ref,batch_ref,item_ref)
        VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(r.command_ref).bind(r.domain_ref).bind(r.core_ref).bind(question).bind(config)
        .bind(batch).bind(item).execute(&mut *c.tx).await?;
    c.finish(Receipt {
        request_ref: r.command_ref,
        batch_ref: batch,
        item_ref: item,
        reused: false,
    })
    .await
}

pub(crate) struct FrozenDeep {
    domain: Uuid,
    work: Uuid,
    core: Uuid,
    question: String,
}

impl FrozenDeep {
    pub(crate) async fn load(
        tx: &mut Transaction<'_, Postgres>,
        request: Uuid,
        config: Uuid,
        batch: Uuid,
        item: Uuid,
    ) -> Result<Self, FoundationError> {
        let row = sqlx::query(
            "SELECT r.domain_ref,r.core_ref,r.question,c.work_ref
            FROM linggan_content_understanding_deep_request r
            JOIN linggan_content_understanding_result c ON c.request_ref=r.core_ref
            WHERE r.request_ref=$1 AND r.task_config_ref=$2 AND r.batch_ref=$3 AND r.item_ref=$4",
        )
        .bind(request)
        .bind(config)
        .bind(batch)
        .bind(item)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(FoundationError::Invalid)?;
        Ok(Self {
            domain: row.get("domain_ref"),
            work: row.get("work_ref"),
            core: row.get("core_ref"),
            question: row.get("question"),
        })
    }

    pub(crate) async fn card_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<Value>, FoundationError> {
        match deep::current_card_in(tx, self.domain, self.work, self.core).await {
            Err(FoundationError::NotFound) => Ok(None),
            other => other,
        }
    }

    pub(crate) fn payload(&self, card: &Value) -> Value {
        super::deep_grounding::payload(&self.question, card)
    }

    pub(crate) fn accept(&self, text: &str, card: &Value) -> Result<Value, &'static str> {
        accept(text, &self.question, card)
    }

    pub(crate) async fn publish_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: Uuid,
        invocation: Uuid,
        findings: Value,
    ) -> Result<(), FoundationError> {
        sqlx::query("INSERT INTO linggan_content_understanding_deep(analysis_ref,core_ref,domain_ref,method_version,question,findings,invocation_ref)
            VALUES($1,$2,$3,$4,$5,$6,$7)")
            .bind(request).bind(self.core).bind(self.domain).bind(METHOD).bind(&self.question)
            .bind(findings).bind(invocation).execute(&mut **tx).await?;
        Ok(())
    }

    pub(crate) async fn lock_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), FoundationError> {
        deep::lock_core(tx, self.core).await
    }
}
