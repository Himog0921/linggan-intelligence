//! T1/T2 for one research unit: one model call over the comments of one work.
//!
//! Prepare freezes what the call sees (the L1 context, the catalog, the targets still eligible)
//! and records it before dispatch. Settle re-checks the sources, admits the output piece by piece
//! and writes results, Problems and support in the one transaction that settles the invocation.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        TaskKey, admission,
        batch_store::InputRef,
        comment_understanding::{
            self as understanding, CONTRACT, TargetFrame, TargetInput,
            catalog::{self, CATALOG_FULL_MAX},
            publish, read_target_in, target_alias, unit,
        },
        store::FoundationError,
        transport::ResponseSchema,
    },
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
    research_text::content_hash,
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;

struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    unit_ref: Uuid,
    prompt_version_ref: Uuid,
}

fn denied(reason: &str) -> ExecutorError {
    FoundationError::AdmissionDenied(vec![reason.into()]).into()
}

async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::CommentResearchUnit { unit_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(denied("batch_cancelled"));
    }
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::CommentUnderstanding,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::CommentUnderstanding,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let unit = unit::load_in(&mut tx, unit_ref).await?;
    let linked: bool = sqlx::query_scalar(
        "SELECT $2=(SELECT item_ref FROM linggan_comment_research_unit WHERE unit_ref=$1)
          AND NOT EXISTS(SELECT 1 FROM linggan_comment_research_unit_target ut
           JOIN linggan_comment_research_request r ON r.request_ref=ut.request_ref
           WHERE ut.unit_ref=$1 AND NOT (r.task_config_ref=$3 AND r.batch_ref=$4 AND r.method_version=$5))",
    )
    .bind(unit_ref)
    .bind(item.item_ref)
    .bind(item.task_config_ref)
    .bind(item.batch_ref)
    .bind(CONTRACT)
    .fetch_one(&mut *tx)
    .await?;
    if !linked || unit.item_ref != item.item_ref {
        return Err(ExecutorError::Invalid);
    }

    // Only comments that are still researchable are sent; the others fail on their own.
    let mut sent = Vec::new();
    let mut withheld = Vec::new();
    for frozen in unit.targets.iter().filter(|target| !target.settled) {
        match read_target_in(
            &mut tx,
            unit.domain_ref,
            unit.work_ref,
            frozen.source_ref,
            frozen.role,
            &frozen.knowledge_cutoff,
        )
        .await?
        {
            Ok(target) => sent.push((frozen, target)),
            Err(_) => withheld.push(frozen),
        }
    }
    if sent.is_empty() {
        return Err(denied("ai_source_unavailable"));
    }
    let work_context = unit::l1_card_in(&mut tx, unit.l1_request_ref)
        .await?
        .as_ref()
        .and_then(understanding::work_context)
        .ok_or_else(|| denied("comment_l1_unavailable"))?;
    let problems = catalog::read_in(&mut tx, unit.domain_ref).await?;
    if problems.len() > CATALOG_FULL_MAX {
        return Err(denied("comment_catalog_too_large"));
    }
    let catalog_refs: Vec<Uuid> = problems.iter().map(|p| p.problem_ref).collect();
    let inputs: Vec<TargetInput> = sent
        .iter()
        .map(|(frozen, target)| TargetInput {
            alias: target_alias(frozen.position as usize - 1),
            research_text: target.cleaned.text.clone(),
            parent_context: target.parent.payload(),
        })
        .collect();
    let fingerprint = content_hash(
        &json!([
            unit.l1_request_ref,
            catalog_refs,
            sent.iter()
                .map(|(_, target)| target.input_hash())
                .collect::<Vec<_>>(),
        ])
        .to_string(),
    );
    let payload = understanding::payload(&work_context, &catalog::entries(&problems), &inputs);
    let super::research_call::Call {
        invocation,
        request,
        prompt_version_ref,
    } = super::research_call::prepare_in(
        &mut tx,
        item,
        secrets,
        super::research_call::Input {
            contract: CONTRACT,
            reference: unit_ref,
            fingerprint: &fingerprint,
            payload,
            response_schema: ResponseSchema {
                name: "comment_understanding".into(),
                schema: understanding::response_schema(),
            },
            material_class: "comment_material",
        },
    )
    .await?;
    unit::record_dispatch_in(&mut tx, unit_ref, invocation, &catalog_refs).await?;
    for frozen in withheld {
        publish::write_failure_in(
            &mut tx,
            frozen.request_ref,
            invocation,
            "ai_source_unavailable",
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        unit_ref,
        prompt_version_ref,
    })
}

pub(super) async fn execute(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
    adapter: &PiAdapter,
) -> Result<(), ExecutorError> {
    let prepared = match prepare(db, item, secrets).await {
        Ok(value) => value,
        Err(ExecutorError::Configuration(FoundationError::AdmissionDenied(reasons))) => {
            match reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied")
            {
                "ai_source_unavailable" => {
                    item.skip_unattempted(db, "ai_source_unavailable").await?
                }
                "comment_l1_unavailable" => {
                    item.skip_unattempted(db, "comment_l1_unavailable").await?
                }
                "comment_catalog_too_large" => {
                    item.skip_unattempted(db, "comment_catalog_too_large")
                        .await?
                }
                reason => item.defer(db, reason).await?,
            }
            return Ok(());
        }
        Err(error) => return super::super::preparation::handle(db, item, error).await,
    };
    let response = super::super::dispatch::call(db, item, adapter, &prepared.request).await;
    receipt::finalize(db, item, prepared.invocation, response.as_ref(), || {
        settle(db, item, &prepared, response.as_ref())
    })
    .await
}

/// The adapter reports an output cut off by the token limit as a failed call, not as a short
/// answer, so a cut-off unit is recognized by its failure and finish reason.
fn is_truncated(response: &PiResponse) -> bool {
    response.failure_code.as_deref() == Some("output_limit")
        || response
            .diagnostic
            .as_ref()
            .and_then(|d| d.finish_reason.as_deref())
            == Some("length")
}

async fn settle(
    db: &Database,
    item: &ClaimedItem,
    prepared: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) = receipt::lock_in(db, item, prepared.invocation, response).await?;
    let unit = unit::load_in(&mut tx, prepared.unit_ref).await?;
    let catalog_refs = unit::dispatch_in(&mut tx, prepared.unit_ref, prepared.invocation)
        .await?
        .ok_or(ExecutorError::Invalid)?;
    // The targets that were sent are those with no result yet; any that can no longer be read are
    // withheld, so a comment withdrawn during the call never gets a published result.
    let mut frames = Vec::new();
    let mut withheld = Vec::new();
    for frozen in unit.targets.iter().filter(|target| !target.settled) {
        match read_target_in(
            &mut tx,
            unit.domain_ref,
            unit.work_ref,
            frozen.source_ref,
            frozen.role,
            &frozen.knowledge_cutoff,
        )
        .await?
        {
            Ok(target) => frames.push(TargetFrame {
                alias: target_alias(frozen.position as usize - 1),
                raw_text: target.raw_text,
                cleaned: target.cleaned,
            }),
            Err(_) => withheld.push(frozen),
        }
    }
    let allowed = !cancelled
        && admission::check_queue_configuration(&mut tx, item.task_key, item.task_config_ref, true)
            .await
            .is_ok()
        && admission::check_queue_input_scope(
            &mut tx,
            item.task_key,
            item.task_config_ref,
            &item.input,
        )
        .await
        .is_ok();
    let aliases: HashSet<String> = (0..catalog_refs.len())
        .map(understanding::catalog_alias)
        .collect();
    let admitted = if !allowed {
        None
    } else if response.is_some_and(is_truncated) {
        Some(Err("unit_truncated"))
    } else {
        response.filter(|r| r.ok).map(|r| {
            match r
                .text
                .as_deref()
                .map(|text| understanding::accept_unit(text, &frames, &aliases))
            {
                Some(Ok(unit)) => Ok(unit),
                _ => Err("unit_output_invalid"),
            }
        })
    };
    let success = matches!(admitted, Some(Ok(_)));
    let mut failure =
        super::research_call::failure(response, allowed, success, "comment_publication_withheld");
    if let Some(Err(code)) = &admitted {
        failure = Some(code);
    }
    let published = match admitted {
        Some(Ok(accepted)) => {
            let published = publish::publish_in(
                &mut tx,
                &unit,
                prepared.invocation,
                &catalog_refs,
                &accepted,
            )
            .await?;
            for frozen in &withheld {
                publish::write_failure_in(
                    &mut tx,
                    frozen.request_ref,
                    prepared.invocation,
                    "comment_publication_withheld",
                )
                .await?;
            }
            Some(published)
        }
        _ => {
            let code = failure.unwrap_or("ai_output_invalid");
            let unsettled = unit.targets.iter().filter(|target| !target.settled);
            for frozen in unsettled {
                publish::write_failure_in(&mut tx, frozen.request_ref, prepared.invocation, code)
                    .await?;
            }
            None
        }
    };
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["unitRef"] = json!(prepared.unit_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    receipt::observe_in(&mut tx, prepared.invocation, response).await?;
    finish_invocation_in(
        &mut tx,
        prepared.invocation,
        response,
        success,
        failure,
        &result,
    )
    .await?;
    let safe_output: Value = match &published {
        Some(p) => json!({
            "unitRef": prepared.unit_ref, "methodVersion": CONTRACT,
            "acceptedTargets": p.accepted, "rejectedTargets": p.rejected,
            "newProblems": p.new_problems, "withheldTargets": withheld.len(),
            "ignoredEntries": p.ignored_entries,
        }),
        None => json!({"unitRef": prepared.unit_ref, "methodVersion": CONTRACT}),
    };
    receipt::write_item_in(
        &mut tx,
        item,
        ItemResult {
            success,
            cancelled,
            output: success.then_some(&safe_output),
            failure,
            counts_as_failure: !success && allowed && !receipt::not_dispatched(response),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
