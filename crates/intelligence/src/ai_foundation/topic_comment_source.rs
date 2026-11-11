//! Existing qualified semantic signals are assignment inputs, never a second understanding pass.
use super::{comment_research, store::FoundationError};
use crate::{
    topic_map_research::ResearchInput,
    topic_map_research_analysis::{Citation, Discussion},
};
use linggan_evidence::creator_discovery::{self, Fragment};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

mod read;
pub use read::{Query, read};
pub const CONTRACT: &str = "topic-map.comment-semantic-source.v1";
pub(crate) fn is_comment(input: &ResearchInput) -> bool {
    input.coverage["inputContract"] == CONTRACT
}
fn unavailable() -> FoundationError {
    FoundationError::AdmissionDenied(vec!["ai_source_unavailable".into()])
}

pub(crate) async fn load_in(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<ResearchInput, FoundationError> {
    let row = sqlx::query("SELECT r.domain_ref,r.work_ref,r.knowledge_cutoff::text AS cutoff,result.result FROM linggan_comment_research_request r JOIN linggan_comment_research_result result USING(request_ref) JOIN linggan_ai_batch_item i USING(batch_ref) JOIN linggan_ai_batch_item_control c USING(item_ref) WHERE r.request_ref=$1 AND c.state='succeeded'")
        .bind(request).fetch_optional(&mut **tx).await?.ok_or_else(unavailable)?;
    let (selected, prepared) = comment_research::frozen_in(tx, request).await?;
    let domain: Uuid = row.get("domain_ref");
    let work: Uuid = row.get("work_ref");
    let cutoff: String = row.get("cutoff");
    let mut data =
        creator_discovery::load_topic_materials_for_works_at_in(tx, domain, &[work], Some(&cutoff))
            .await
            .map_err(|e| match e {
                creator_discovery::DiscoveryError::Database(e) => FoundationError::Database(e),
                _ => unavailable(),
            })?;
    let work = data
        .works
        .pop()
        .filter(|w| w.work_ref == work)
        .ok_or_else(unavailable)?;
    let result: Value = row.get("result");
    let signals = result["signals"].as_array().ok_or_else(unavailable)?;
    let mut fragments = Vec::new();
    for (ordinal, signal) in signals.iter().enumerate() {
        let start = signal["evidenceSpan"]["start"]
            .as_u64()
            .ok_or_else(unavailable)? as usize;
        let end = signal["evidenceSpan"]["end"]
            .as_u64()
            .ok_or_else(unavailable)? as usize;
        if end <= start || end > selected.raw_text.chars().count() {
            return Err(unavailable());
        }
        let text: String = selected
            .raw_text
            .chars()
            .skip(start)
            .take(end - start)
            .collect();
        fragments.push(Fragment {
            fragment_id: format!("{}.comment.{}.signal.{ordinal}",work.work_ref,selected.comment_external_id),
            source_ref: selected.source.source_ref, field: "studied_comment".into(), source_version: prepared.raw_sha256.clone(),
            start,end,text:text.clone(),speaker:"commenter".into(),
            structure:json!({"kind":"paragraph","text":text,"speaker":"commenter","semanticRequestRef":request,"signalOrdinal":ordinal,"signal":signal}),
            ..Default::default()
        });
    }
    let hash = creator_discovery::hash(&json!({"contract":CONTRACT,"requestRef":request,"inputHash":prepared.input_hash,"signals":signals}).to_string());
    Ok(ResearchInput {
        work,
        fragments,
        topics: json!([]),
        domain: data.domain,
        hash,
        context_work_refs: vec![],
        comment_study: json!([]),
        role_metadata: json!({}),
        coverage: json!({"inputContract":CONTRACT,"semanticRequestRef":request,"asOf":cutoff}),
    })
}
pub(crate) fn discussions(input: &ResearchInput) -> Vec<Discussion> {
    input
        .fragments
        .iter()
        .map(|f| {
            let signal = &f.structure["signal"];
            let statement = signal["proposition"].as_str().unwrap_or("").to_owned();
            Discussion {
                label: statement.chars().take(80).collect(),
                topic_ref: None,
                evidence: vec![Citation {
                    fragment_id: f.fragment_id.clone(),
                    start: f.start,
                    end: f.end,
                }],
                statement,
                definition: String::new(),
                inclusion_criteria: vec![],
                exclusion_criteria: vec![],
                speaker_role: "commenter".into(),
                evidence_role: "context".into(),
                rationale: format!(
                    "Saved comment signal: {}",
                    signal["kind"].as_str().unwrap_or("")
                ),
            }
        })
        .collect()
}
pub(crate) fn validate_matched_only(
    resolution: &crate::topic_map_research_analysis::ResolutionOutput,
) -> Result<(), &'static str> {
    if resolution
        .decisions
        .iter()
        .any(|d| d.status == "new" || d.proposed_topic.is_some() || !d.relations.is_empty())
    {
        Err("comment_topic_creation_forbidden")
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn catalog_versions(catalog: &Value) -> Value {
    let mut entries = catalog
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| json!({"topicRef":c["topicRef"],"definitionRef":c["definitionRef"]}))
        .collect::<Vec<_>>();
    entries.sort_by_key(Value::to_string);
    json!(entries)
}
pub(crate) fn assignment_current(unit: &Value, frozen_versions: &Value, catalog: &Value) -> bool {
    if unit["status"] == "matched" {
        unit["assignments"].as_array().is_some_and(|assignments| {
            !assignments.is_empty()
                && assignments.iter().all(|a| {
                    catalog.as_array().into_iter().flatten().any(|c| {
                        c["topicRef"] == a["topicRef"] && c["definitionRef"] == a["definitionRef"]
                    })
                })
        })
    } else {
        frozen_versions == &catalog_versions(catalog)
    }
}
