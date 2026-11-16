use super::{
    Command, Database, FoundationError, Receipt, TaskKey, Uuid, Value, admission, deep,
    deep_manifest::{self, SubQuestion},
};
use crate::ai_foundation::{
    batch_store::{self, BatchInput, BatchKind, InputRef},
    store::valid_reason,
    task_config::lock_setting,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{Postgres, Row, Transaction};

pub(crate) const METHOD: &str = "content-understanding.l2.model.v2";
pub(crate) use super::deep_v2::{INSUFFICIENT as INSUFFICIENT_EVIDENCE, response_schema};
const TASK: TaskKey = TaskKey::ContentUnderstandingDeep;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerateDeep {
    command_ref: Uuid,
    domain_ref: Uuid,
    work_ref: Uuid,
    core_ref: Uuid,
    research_mode: String,
    base_mode: Option<String>,
    sub_questions: Vec<SubQuestion>,
    task_config_ref: Uuid,
    task_revision: i64,
}

/// Start-time admission, mirroring L1 `reasons()`: report a non-High model or an
/// oversized input now instead of letting the executor defer the item.
fn start_reasons(
    thinking: &str,
    limit: i32,
    system: &str,
    body: &Value,
    images: usize,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if thinking != "high" {
        reasons.push("content_understanding_requires_high".into());
    }
    if crate::ai_foundation::transport::estimate_input_tokens(system, &body.to_string())
        + images as u64 * 1024
        > limit as u64
    {
        reasons.push("input_too_large".into());
    }
    reasons
}

pub async fn generate_deep(db: &Database, r: &GenerateDeep) -> Result<Receipt, FoundationError> {
    let research_mode = r.research_mode.trim();
    let base_mode = r
        .base_mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let question = deep_manifest::validate_request(research_mode, base_mode, &r.sub_questions)
        .map_err(|_| FoundationError::Invalid)?;
    if [r.command_ref, r.domain_ref, r.work_ref, r.core_ref]
        .iter()
        .any(Uuid::is_nil)
        || r.task_revision < 0
        || !valid_reason(&question, 300)
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
    let cutoff = linggan_evidence::research_dossier::cutoff_in(&mut c.tx, None).await?;
    let packet = super::source::load(&mut c.tx, r.domain_ref, r.work_ref, &cutoff).await?;
    let (card, core_basis) = deep::current_card_basis_in(
        &mut c.tx,
        r.work_ref,
        r.core_ref,
        &packet.snapshot.material_snapshot_identity,
    )
    .await?
    .ok_or(FoundationError::Conflict)?;
    let (manifest, fingerprint) = deep_manifest::build(
        &packet,
        &card,
        research_mode,
        base_mode,
        &r.sub_questions,
        METHOD,
        &core_basis,
    )
    .map_err(|_| FoundationError::Invalid)?;
    let limits: Option<(String, i32)> = sqlx::query_as("SELECT m.thinking_level,m.input_token_limit FROM linggan_ai_task_config c JOIN linggan_model_config m ON m.config_ref=c.model_config_ref WHERE c.task_config_ref=$1")
        .bind(config).fetch_optional(&mut *c.tx).await?;
    if let Some((thinking, limit)) = limits {
        let (_, mut system, instruction, skill) =
            crate::ai_foundation::prompt::frozen_in(&mut c.tx, config, TASK).await?;
        if let Some(skill) = skill {
            system.push_str("\n\n");
            system.push_str(&skill);
        }
        let mut body = super::deep_v2::payload(
            &question,
            research_mode,
            base_mode,
            &r.sub_questions,
            &card,
            &manifest,
        );
        body["instruction"] = json!(instruction);
        let images = manifest
            .as_array()
            .into_iter()
            .flatten()
            .filter(|material| matches!(material["materialKind"].as_str(), Some("cover" | "image")))
            .count();
        let reasons = start_reasons(&thinking, limit, &system, &body, images);
        if !reasons.is_empty() {
            return Err(FoundationError::AdmissionDenied(reasons));
        }
    }
    let existing = sqlx::query("SELECT r.request_ref,r.batch_ref,r.item_ref,c.failure_code
        FROM linggan_content_understanding_deep_request r JOIN linggan_ai_batch_item_control c USING(item_ref)
        WHERE r.core_ref=$1 AND r.domain_ref=$2 AND r.input_fingerprint=$3
          AND (c.state IN ('pending','running') OR c.failure_code='ai_dispatch_unknown')
        ORDER BY (c.failure_code IS NOT DISTINCT FROM 'ai_dispatch_unknown') DESC LIMIT 1")
        .bind(r.core_ref).bind(r.domain_ref).bind(&fingerprint).fetch_optional(&mut *c.tx).await?;
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
    sqlx::query("INSERT INTO linggan_content_understanding_deep_request(request_ref,domain_ref,core_ref,question,task_config_ref,batch_ref,item_ref,research_mode,base_mode,sub_questions,core_basis,input_manifest,input_fingerprint)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)")
        .bind(r.command_ref).bind(r.domain_ref).bind(r.core_ref).bind(&question).bind(config)
        .bind(batch).bind(item).bind(research_mode).bind(base_mode)
        .bind(json!(&r.sub_questions)).bind(&core_basis).bind(&manifest).bind(&fingerprint)
        .execute(&mut *c.tx).await?;
    c.finish(Receipt {
        request_ref: r.command_ref,
        batch_ref: batch,
        item_ref: item,
        reused: false,
    })
    .await
}

pub(crate) struct FrozenDeep {
    pub(crate) domain: Uuid,
    pub(crate) work: Uuid,
    pub(crate) core: Uuid,
    pub(crate) question: String,
    pub(crate) research_mode: String,
    pub(crate) base_mode: Option<String>,
    pub(crate) sub_questions: Vec<SubQuestion>,
    pub(crate) core_basis: Value,
    pub(crate) input_manifest: Value,
    pub(crate) input_fingerprint: String,
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
            "SELECT r.domain_ref,r.core_ref,r.question,r.research_mode,r.base_mode,
              r.sub_questions,r.core_basis,r.input_manifest,r.input_fingerprint,c.work_ref
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
            research_mode: row.get("research_mode"),
            base_mode: row.get("base_mode"),
            sub_questions: serde_json::from_value(row.get::<Value, _>("sub_questions"))
                .map_err(|_| FoundationError::Invalid)?,
            core_basis: row.get("core_basis"),
            input_manifest: row.get("input_manifest"),
            input_fingerprint: row.get("input_fingerprint"),
        })
    }

    /// Current packet, Core card and its basis for the frozen request.
    pub(crate) async fn packet_card_basis_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<Option<(super::source::Packet, Value, Value)>, FoundationError> {
        let cutoff = linggan_evidence::research_dossier::cutoff_in(tx, None).await?;
        let packet = match super::source::load(tx, self.domain, self.work, &cutoff).await {
            Ok(packet) => packet,
            Err(FoundationError::NotFound) => return Ok(None),
            Err(error) => return Err(error),
        };
        let Some((card, basis)) = deep::current_card_basis_in(
            tx,
            self.work,
            self.core,
            &packet.snapshot.material_snapshot_identity,
        )
        .await?
        else {
            return Ok(None);
        };
        Ok(Some((packet, card, basis)))
    }

    pub(crate) fn payload(&self, card: &Value) -> Value {
        super::deep_v2::payload(
            &self.question,
            &self.research_mode,
            self.base_mode.as_deref(),
            &self.sub_questions,
            card,
            &self.input_manifest,
        )
    }

    pub(crate) fn accept(
        &self,
        text: &str,
        card: &Value,
    ) -> Result<(Value, Value, Value), &'static str> {
        super::deep_v2::accept(text, card, &self.input_manifest, &self.sub_questions)
    }

    pub(crate) async fn publish_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        request: Uuid,
        invocation: Uuid,
        findings: Value,
        rejected: Value,
        sub_question_status: Value,
    ) -> Result<(), FoundationError> {
        sqlx::query("INSERT INTO linggan_content_understanding_deep(analysis_ref,core_ref,domain_ref,method_version,question,findings,invocation_ref,research_mode,sub_question_status,rejected)
            VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(request).bind(self.core).bind(self.domain).bind(METHOD).bind(&self.question)
            .bind(findings).bind(invocation).bind(&self.research_mode)
            .bind(sub_question_status).bind(rejected)
            .execute(&mut **tx).await?;
        Ok(())
    }

    pub(crate) async fn lock_in(
        &self,
        tx: &mut Transaction<'_, Postgres>,
    ) -> Result<(), FoundationError> {
        deep::lock_core(tx, self.core).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_reasons_accept_a_high_model_within_the_limit() {
        assert!(start_reasons("high", 100_000, "系统", &json!({"q":"问"}), 0).is_empty());
    }

    #[test]
    fn start_reasons_report_a_non_high_model() {
        assert_eq!(
            start_reasons("medium", 100_000, "系统", &json!({"q":"问"}), 0),
            vec!["content_understanding_requires_high".to_string()]
        );
    }

    #[test]
    fn start_reasons_reserve_each_image_against_the_limit() {
        assert_eq!(
            start_reasons("high", 1000, "系统", &json!({"q":"问"}), 1),
            vec!["input_too_large".to_string()]
        );
    }
}
