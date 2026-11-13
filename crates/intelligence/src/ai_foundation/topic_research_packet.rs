//! Stage-specific topic packets and validators; transport does not decide topic semantics.
mod single_pass;

use super::{store::FoundationError, topic_comment_source as comments, topic_research as owner};
use crate::{
    topic_map_research::ResearchInput,
    topic_map_research_analysis::{
        self as analysis, ResearchOutput, ResolutionOutput, verification,
    },
};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct Packet {
    pub payload: Value,
    pub schema: Value,
    pub context: Value,
}
fn invalid() -> FoundationError {
    FoundationError::AdmissionDenied(vec!["topic_dependency_failed".into()])
}
pub(crate) async fn draft_in(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
) -> Result<ResearchOutput, FoundationError> {
    let output = owner::previous_in(tx, window, 0).await?;
    let projection = if output["contract"] == single_pass::CONTRACT { output["projection"].clone() } else { output };
    serde_json::from_value(projection).map_err(|_| invalid())
}
async fn assignment_units(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    input: &ResearchInput,
) -> Result<Vec<(String, analysis::Discussion)>, FoundationError> {
    if comments::is_comment(input) {
        Ok(crate::topic_map_core::units_for_discussions(
            input,
            &comments::discussions(input),
        ))
    } else {
        Ok(crate::topic_map_core::units(
            input,
            &draft_in(tx, window).await?,
        ))
    }
}
fn resolution_schema(input: &ResearchInput) -> Value {
    let mut schema = analysis::resolution_schema();
    if comments::is_comment(input) {
        schema["properties"]["decisions"]["items"]["properties"]["status"]["enum"] =
            json!(["matched", "uncertain", "out_of_scope"]);
        schema["properties"]["decisions"]["items"]["properties"]["proposedTopic"] =
            json!({"type":"null"});
        schema["properties"]["decisions"]["items"]["properties"]["relations"]["maxItems"] =
            json!(0);
    }
    schema
}
/// Frozen IDs remain unchanged; withdrawn or superseded candidate text cannot be sent or admitted.
pub(crate) async fn candidate_sources_current(
    db: &Database,
    input: &ResearchInput,
    context: &Value,
) -> Result<(), FoundationError> {
    let domain: Uuid = serde_json::from_value(input.domain["domainRef"].clone())
        .map_err(|_| FoundationError::Invalid)?;
    let qualified = crate::topic_map_core::catalog(db, domain)
        .await
        .map_err(|e| match e {
            crate::model_settings::ModelError::Database(e) => FoundationError::Database(e),
            _ => invalid(),
        })?;
    if context["candidates"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|candidate| {
            !qualified
                .as_array()
                .unwrap()
                .iter()
                .any(|current| current["definitionRef"] == candidate["definitionRef"])
        })
    {
        return Err(FoundationError::AdmissionDenied(vec![
            "ai_source_unavailable".into(),
        ]));
    }
    Ok(())
}
pub(crate) async fn prepare(
    db: &Database,
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    stage: u8,
    input: &ResearchInput,
) -> Result<Packet, FoundationError> {
    if owner::single_pass(input) { return single_pass::prepare(db, tx, window, input).await; }
    let fragments = || {
        input.fragments.iter().enumerate().map(|(index,f)|json!({"fragmentIndex":index,"fragmentId":f.fragment_id,"field":f.field,"sourceRef":f.source_ref,"sourceVersion":f.source_version,"speaker":f.speaker,"content":f.model_content(),"issues":f.issues})).collect::<Vec<_>>()
    };
    match stage {
        0 => Ok(Packet {
            payload: json!({"contract":analysis::COMPACT_EXTRACT_CONTRACT,"input":{"domain":input.domain,"workRef":input.work.work_ref,"fragments":fragments(),"commentStudy":input.comment_study,"contentCore":input.coverage["core"],"understandingInstruction":if input.coverage["inputContract"] == crate::topic_map_research::CORE_INPUT_CONTRACT { "Consume the saved L1 Core and its evidence. Project author topics, scenes and journey; do not repeat raw-material understanding. Image evidence is a saved model observation, not verified OCR or a verbatim source quote. Comments cannot propose directions." } else { "" },"roleMetadata":input.role_metadata,"coverage":input.coverage},"outputFieldNames":analysis::compact_field_names(),"outputSchema":analysis::compact_output_schema()}),
            schema: analysis::compact_output_schema(),
            context: json!({}),
        }),
        1 => {
            let draft = draft_in(tx, window).await?;
            let packet =
                verification::extraction_review(&draft, &input.fragments).map_err(|_| invalid())?;
            Ok(Packet {
                context: json!({"checks":packet["checks"]}),
                payload: json!({"contract":verification::VERIFY_CONTRACT,"input":packet}),
                schema: verification::verify_schema(),
            })
        }
        2 => {
            if !comments::is_comment(input) {
                owner::previous_in(tx, window, 1).await?;
            }
            let units = assignment_units(tx, window, input).await?;
            if units.is_empty() {
                return Err(FoundationError::AdmissionDenied(vec![
                    "topic_assignment_not_required".into(),
                ]));
            }
            let domain: Uuid =
                serde_json::from_value(input.domain["domainRef"].clone()).map_err(|_| invalid())?;
            let catalog =
                crate::topic_map_core::catalog(db, domain)
                    .await
                    .map_err(|e| match e {
                        crate::model_settings::ModelError::Database(e) => {
                            FoundationError::Database(e)
                        }
                        _ => invalid(),
                    })?;
            let candidates = catalog.as_array().ok_or_else(|| {
                FoundationError::AdmissionDenied(vec!["topic_catalog_scope_too_large".into()])
            })?;
            let context = json!({"candidates":candidates,"recall":{"method":"complete_catalog","catalogTopics":candidates.len(),"similarityIsMembership":false}});
            Ok(Packet {
                payload: json!({"contract":analysis::RESOLVE_CONTRACT,"input":{"domain":input.domain,"units":units.iter().map(|(id,d)|json!({"unitId":id,"discussion":d})).collect::<Vec<_>>(),"fragments":fragments(),"definitions":candidates,"coverage":input.coverage,"assignmentInstruction":if comments::is_comment(input) { "These are saved comment semantic signals. Match their meaning only to existing supplied topics. Do not re-understand comments, create a topic, infer author membership or force a match. Keep uncertain/out_of_scope with reasons when no existing topic fits; proposedTopic must be null and relations must be empty." } else { "" }},"outputSchema":resolution_schema(input)}),
                schema: resolution_schema(input),
                context,
            })
        }
        3 => {
            let resolution: ResolutionOutput =
                serde_json::from_value(owner::previous_in(tx, window, 2).await?)
                    .map_err(|_| invalid())?;
            let units = assignment_units(tx, window, input).await?;
            let context:Value=sqlx::query_scalar("SELECT d.context FROM linggan_topic_research_dispatch d JOIN linggan_topic_research_stage s USING(item_ref) WHERE s.window_ref=$1 AND s.stage_number=2").bind(window).fetch_one(&mut **tx).await?;
            candidate_sources_current(db, input, &context).await?;
            let packet = verification::resolution_review(
                &units,
                &resolution,
                &context["candidates"],
                &input.fragments,
            )
            .map_err(|_| invalid())?;
            Ok(Packet {
                payload: json!({"contract":verification::VERIFY_CONTRACT,"input":packet}),
                schema: verification::verify_schema(),
                context: json!({"checks":packet["checks"],"candidates":context["candidates"],"recall":context["recall"]}),
            })
        }
        _ => Err(FoundationError::Invalid),
    }
}
/// Safe rule codes only; never includes model text or source bodies in diagnostics.
pub(crate) fn validate(
    text: &str,
    stage: u8,
    input: &ResearchInput,
    context: &Value,
    draft: Option<&ResearchOutput>,
) -> Result<Value, &'static str> {
    if owner::single_pass(input) { return single_pass::validate(text, input, context); }
    match stage {
        0 => {
            let value =
                analysis::decode_compact_output(text, &input.fragments).map_err(|e| e.code)?;
            analysis::validate_output(&value, &input.fragments, &[])?;
            analysis::validate_context_links(&value, &input.fragments, &input.comment_study)?;
            serde_json::to_value(value).map_err(|_| "output_serialization_invalid")
        }
        1 | 3 => {
            let value: verification::VerificationOutput =
                serde_json::from_str(text).map_err(|_| "output_schema_invalid")?;
            verification::validate_verification(&value, &context["checks"])?;
            serde_json::to_value(value).map_err(|_| "output_serialization_invalid")
        }
        2 => {
            let value: ResolutionOutput =
                serde_json::from_str(text).map_err(|_| "output_schema_invalid")?;
            let units = if comments::is_comment(input) {
                comments::validate_matched_only(&value)?;
                crate::topic_map_core::units_for_discussions(input, &comments::discussions(input))
            } else {
                crate::topic_map_core::units(input, draft.ok_or("topic_dependency_failed")?)
            };
            analysis::validate_resolution(&value, &units, &context["candidates"])?;
            serde_json::to_value(value).map_err(|_| "output_serialization_invalid")
        }
        _ => Err("topic_stage_invalid"),
    }
}
pub(crate) async fn publish_in(
    tx: &mut Transaction<'_, Postgres>,
    window: Uuid,
    stage: u8,
    invocation: Uuid,
    input: &ResearchInput,
    context: &Value,
    parsed: &Value,
) -> Result<(), crate::model_settings::ModelError> {
    if owner::single_pass(input) { return single_pass::publish_in(tx, window, invocation, input, context, parsed).await; }
    let draft = if comments::is_comment(input) {
        None
    } else {
        Some(draft_in(tx, window).await.map_err(|e| match e {
            FoundationError::Database(e) => crate::model_settings::ModelError::Database(e),
            _ => crate::model_settings::ModelError::InvalidOutput,
        })?)
    };
    let core = if stage == 1 && draft.as_ref().is_some_and(|d| d.discussions.is_empty()) {
        json!({"units":[]})
    } else if stage == 3 {
        let resolution: ResolutionOutput = serde_json::from_value(
            owner::previous_in(tx, window, 2)
                .await
                .map_err(|_| crate::model_settings::ModelError::InvalidOutput)?,
        )
        .map_err(|_| crate::model_settings::ModelError::InvalidOutput)?;
        let units = assignment_units(tx, window, input)
            .await
            .map_err(|_| crate::model_settings::ModelError::InvalidOutput)?;
        let eligible = context["candidates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["definitionRef"].as_str()?.parse().ok())
            .collect::<Vec<_>>();
        let accepted=crate::topic_map_core::accept_in(tx,crate::topic_map_core::Acceptance {input,task:window,invocation,units:&units,resolution:&resolution,candidates:&context["candidates"],recall:&context["recall"],semantic_verification:&json!({"contract":verification::VERIFY_CONTRACT,"passed":true}),eligible_definitions:&eligible}).await?;
        json!({"units":accepted})
    } else {
        return Ok(());
    };
    sqlx::query("INSERT INTO linggan_topic_research_result(result_ref,window_ref,invocation_ref,output_json,core_json,method_version) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(Uuid::new_v4()).bind(window).bind(invocation).bind(if comments::is_comment(input) { json!({"contract":comments::CONTRACT,"semanticRequestRef":input.coverage["semanticRequestRef"]}) } else { serde_json::to_value(draft).map_err(|_|crate::model_settings::ModelError::InvalidOutput)? }).bind(core).bind(analysis::METHOD_VERSION).execute(&mut **tx).await?;
    Ok(())
}
