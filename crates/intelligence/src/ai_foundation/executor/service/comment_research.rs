//! Comment semantic analysis uses frozen dossier and target inputs and the shared execution ledger.
use super::super::{
    ExecutorError,
    queue::ClaimedItem,
    receipt::{self, ItemResult},
};
use crate::{
    ai_foundation::{
        TaskKey, admission, batch_store::InputRef, comment_research as owner,
        store::FoundationError,
    },
    comment_cleaning, comment_study_semantic,
    model_invocation::finish_invocation_in,
    model_secrets::ModelSecretStore,
    pi_adapter::{PiAdapter, PiRequest, PiResponse, safe_result},
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use uuid::Uuid;

struct Prepared {
    invocation: Uuid,
    request: PiRequest,
    request_ref: Uuid,
    prompt_version_ref: Uuid,
}
fn schema() -> Value {
    let framed = json!({"type":"object","additionalProperties":false,"required":["value","basis"],"properties":{"value":{"type":["string","null"]},"basis":{"type":["string","null"]}}});
    let frame = json!({"type":["object","null"],"additionalProperties":false,"required":["actor","goalOrExpectedState","barrierOrUnmetNeed","context"],"properties":{"actor":framed,"goalOrExpectedState":framed,"barrierOrUnmetNeed":framed,"context":framed}});
    json!({"type":"object","additionalProperties":false,"required":["contract","outcome","reason","signals"],"properties":{
      "contract":{"type":"string","enum":["comment-study.semantic.v1"]},
      "outcome":{"type":"string","enum":["signals","no_signal","needs_context"]},
      "reason":{"type":["string","null"]},"signals":{"type":"array","maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["kind","proposition","evidence","problemFrame"],"properties":{"kind":{"type":"string","enum":["problem","need","belief","emotion","experience","solution","quote","context","question"]},"proposition":{"type":"string"},"evidence":{"type":"string"},"problemFrame":frame}}}}})
}

fn validated_output(value: &Value, raw: &str, source_ref: Uuid) -> Option<Value> {
    let fields = value.as_object()?;
    if fields.len() != 4
        || fields
            .keys()
            .any(|key| !["contract", "outcome", "reason", "signals"].contains(&key.as_str()))
    {
        return None;
    }
    let outcome = value.get("outcome")?.as_str()?;
    let reason = value.get("reason")?;
    let signals = value.get("signals")?.as_array()?;
    match outcome {
        "signals" if !signals.is_empty() && reason.is_null() => {}
        "no_signal" | "needs_context"
            if signals.is_empty()
                && reason
                    .as_str()
                    .is_some_and(|s| !s.trim().is_empty() && s.chars().count() <= 500) => {}
        _ => return None,
    }
    let accepted = comment_study_semantic::accept_semantic_output(
        json!({"contract":value.get("contract")?,"signals":signals}),
        raw,
    )
    .ok()?;
    let cleaner = comment_cleaning::clean(raw);
    let mut safe = Vec::new();
    for signal in accepted {
        let frame = match signal.problem_frame {
            None => None,
            Some(mut frame) => {
                for field in [
                    "actor",
                    "goalOrExpectedState",
                    "barrierOrUnmetNeed",
                    "context",
                ] {
                    let part = frame.get_mut(field)?.as_object_mut()?;
                    let basis = part.remove("basis")?;
                    if let Some(basis) = basis.as_str() {
                        let (start, end, _) = cleaner.resolve(basis, raw).ok()?;
                        part.insert("basisSpan".into(), json!({"start":start,"end":end}));
                    } else {
                        part.insert("basisSpan".into(), Value::Null);
                    }
                }
                Some(frame)
            }
        };
        safe.push(json!({"kind":signal.kind,"proposition":signal.proposition,"sourceRef":source_ref,
            "evidenceSpan":{"start":signal.evidence_start,"end":signal.evidence_end},"problemFrame":frame,
            "eligibilityState":signal.eligibility_state,"eligibilityReason":signal.eligibility_reason}));
    }
    Some(
        json!({"contract":"comment-study.semantic.v1","outcome":outcome,"reason":reason,"signals":safe}),
    )
}
async fn prepare(
    db: &Database,
    item: &ClaimedItem,
    secrets: &dyn ModelSecretStore,
) -> Result<Prepared, ExecutorError> {
    let InputRef::CommentResearch { request_ref } = item.input else {
        return Err(ExecutorError::Invalid);
    };
    let mut tx = item.owner.begin(db, true).await?;
    if item.lock_in(&mut tx).await? {
        return Err(FoundationError::AdmissionDenied(vec!["batch_cancelled".into()]).into());
    }
    admission::check_queue_configuration(
        &mut tx,
        TaskKey::CommentStudy,
        item.task_config_ref,
        true,
    )
    .await?;
    admission::check_queue_input_scope(
        &mut tx,
        TaskKey::CommentStudy,
        item.task_config_ref,
        &item.input,
    )
    .await?;
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_comment_research_request WHERE request_ref=$1 AND task_config_ref=$2 AND batch_ref=$3 AND method_version=$4)")
        .bind(request_ref).bind(item.task_config_ref).bind(item.batch_ref).bind(owner::CONTRACT).fetch_one(&mut *tx).await?;
    if !linked {
        return Err(ExecutorError::Invalid);
    }
    let (source, input) = owner::frozen_in(&mut tx, request_ref).await?;
    let super::research_call::Call{invocation,request,prompt_version_ref}=super::research_call::prepare_in(&mut tx,item,secrets,super::research_call::Input{contract:owner::CONTRACT,reference:request_ref,fingerprint:&input.input_fingerprint,payload:json!({"workContext":source.source.context_manifest,"target":{"researchText":input.research_text},"parentContext":input.input_manifest["parentContext"]}),response_schema:crate::ai_foundation::transport::ResponseSchema{name:"comment_research_semantic".into(),schema:schema()},material_class:"comment_material"}).await?;
    tx.commit().await?;
    Ok(Prepared {
        invocation,
        request,
        request_ref,
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
            let reason = reasons
                .first()
                .map(String::as_str)
                .unwrap_or("admission_denied");
            if reason == "ai_source_unavailable" {
                item.skip_unattempted(db, "ai_source_unavailable").await?;
            } else {
                item.defer(db, reason).await?;
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
async fn settle(
    db: &Database,
    item: &ClaimedItem,
    prepared: &Prepared,
    response: Option<&PiResponse>,
) -> Result<(), ExecutorError> {
    let (mut tx, cancelled) = receipt::lock_in(db, item, prepared.invocation, response).await?;
    let sources = owner::frozen_in(&mut tx, prepared.request_ref).await;
    let parsed = response
        .filter(|r| r.ok)
        .and_then(|r| r.text.as_deref())
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| {
            sources.as_ref().ok().and_then(|(source, _)| {
                validated_output(&v, &source.raw_text, source.source.source_ref)
            })
        });
    let allowed = !cancelled
        && sources.is_ok()
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
    let success = allowed && parsed.is_some();
    let failure =
        super::research_call::failure(response, allowed, success, "comment_publication_withheld");
    let mut result = response
        .map(safe_result)
        .unwrap_or_else(|| json!({"ok":false}));
    result["executorReceipt"] = json!(item.lease_token);
    result["requestRef"] = json!(prepared.request_ref);
    result["promptVersion"] = json!(prepared.prompt_version_ref);
    sqlx::query("INSERT INTO linggan_comment_research_result(request_ref,invocation_ref,result,failure_code) VALUES($1,$2,$3,$4)")
        .bind(prepared.request_ref).bind(prepared.invocation).bind(if success {parsed.as_ref()} else {None}).bind(failure).execute(&mut *tx).await?;
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
    let safe_output = json!({"requestRef":prepared.request_ref,"methodVersion":owner::CONTRACT});
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evidence_is_only_admitted_from_target_and_stored_as_spans() {
        let source = Uuid::new_v4();
        let output = json!({"contract":"comment-study.semantic.v1","outcome":"signals","reason":null,"signals":[{"kind":"emotion","proposition":"焦虑","evidence":"我很着急","problemFrame":null}]});
        let result = validated_output(&output, "孩子写作业，我很着急。", source).unwrap();
        assert_eq!(
            result["signals"][0]["evidenceSpan"],
            json!({"start":6,"end":10})
        );
        assert!(result.to_string().find("我很着急").is_none());
        assert!(validated_output(&output, "父评论才包含这句话", source).is_none());
    }
    #[test]
    fn missing_context_is_distinct_from_no_signal_and_requires_reason() {
        let source = Uuid::new_v4();
        assert!(validated_output(&json!({"contract":"comment-study.semantic.v1","outcome":"needs_context","reason":"缺少父评论","signals":[]}), "这怎么办", source).is_some());
        assert!(validated_output(&json!({"contract":"comment-study.semantic.v1","outcome":"no_signal","reason":null,"signals":[]}), "这怎么办", source).is_none());
    }
}
