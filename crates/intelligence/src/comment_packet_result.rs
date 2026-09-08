//! Strict per-comment contract validation and server-owned evidence resolution.
use super::{Outcome, PacketComment, Quote, ResearchPacket, semantic_context};
use crate::{
    comment_analysis::{
        CommentAnalysisInput, CommentAnalysisOutput, CommentAnalysisSpan, validate_comment_analysis,
    },
    comment_cleaning::CleanComment,
    comment_research::{ResearchBasis, ResearchDimension, ResearchFacet, comment_source_hash},
};
use serde_json::{Value, json};

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
fn validate_output(output: &PacketComment) -> Result<(), &'static str> {
    if output.labels.len() > 8
        || output.problems.len() > 8
        || output.stances.len() > 8
        || output.context_missing.len() > 8
        || output.limitations.len() > 8
        || output
            .context_missing
            .iter()
            .chain(&output.limitations)
            .any(|s| !bounded(s, 200))
        || output
            .uncertainty_reason
            .as_ref()
            .is_some_and(|s| !bounded(s, 200))
    {
        return Err("output_bounds");
    }
    if output.outcome == Outcome::Uncertain && output.uncertainty_reason.is_none() {
        return Err("uncertainty_reason_missing");
    }
    if output.outcome != Outcome::Interpretable
        && (!output.labels.is_empty() || !output.problems.is_empty() || !output.stances.is_empty())
    {
        return Err("outcome_conflict");
    }
    if output.outcome == Outcome::Interpretable
        && output.labels.is_empty()
        && output.problems.is_empty()
        && output.stances.is_empty()
    {
        return Err("interpretable_without_evidence");
    }
    Ok(())
}
struct ItemEvidence<'a> {
    input: &'a CommentAnalysisInput,
    cleaned: &'a CleanComment,
    spans: Vec<CommentAnalysisSpan>,
}
impl ItemEvidence<'_> {
    fn resolve(
        &mut self,
        quotes: &[Quote],
        dimension: ResearchDimension,
        label: String,
    ) -> Result<Vec<Value>, &'static str> {
        if quotes.is_empty() || quotes.len() > 4 {
            return Err("evidence_bounds");
        }
        let mut refs = Vec::new();
        for q in quotes {
            let (start_char, end_char, quote) = self.cleaned.resolve(&q.quote, &self.input.body)?;
            refs.push(
                    json!({"sourceRef":self.input.source_ref,"startChar":start_char,"endChar":end_char}),
                );
            self.spans.push(CommentAnalysisSpan {
                source_ref: self.input.source_ref,
                start_char,
                end_char,
                quote,
                facets: vec![ResearchFacet {
                    dimension: dimension.clone(),
                    label: label.clone(),
                    basis: ResearchBasis::Explicit,
                }],
            });
        }
        Ok(refs)
    }
}
impl ResearchPacket {
    pub fn parse(&self, raw: &str) -> Result<Vec<Result<Value, &'static str>>, &'static str> {
        let envelope: Value = serde_json::from_str(raw).map_err(|_| "json_invalid")?;
        let obj = envelope.as_object().ok_or("schema_invalid")?;
        if obj.len() != 1 {
            return Err("schema_invalid");
        }
        let items = obj
            .get("comments")
            .and_then(Value::as_array)
            .ok_or("schema_invalid")?;
        if items.len() > 100 {
            return Err("output_bounds");
        }
        // Parse each target independently: one malformed item cannot discard its valid siblings.
        Ok(self
            .inputs
            .iter()
            .enumerate()
            .map(|(i, input)| {
                let id = format!("C{:03}", i + 1);
                let matching: Vec<_> = items
                    .iter()
                    .filter(|item| {
                        item.get("commentRef").and_then(Value::as_str) == Some(id.as_str())
                    })
                    .collect();
                if matching.is_empty() {
                    return Err("missing_comment");
                }
                if matching.len() != 1 {
                    return Err("duplicate_comment");
                }
                if !matching[0]
                    .as_object()
                    .is_some_and(|o| o.contains_key("uncertaintyReason"))
                {
                    return Err("item_schema_invalid");
                }
                if matching[0]["problems"].as_array().is_some_and(|ps| {
                    ps.iter().any(|p| {
                        !p.as_object()
                            .is_some_and(|o| o.contains_key("candidateRef"))
                    })
                }) {
                    return Err("item_schema_invalid");
                }
                let output: PacketComment = serde_json::from_value(matching[0].clone())
                    .map_err(|_| "item_schema_invalid")?;
                self.validate_item(i, input, output)
            })
            .collect())
    }
    fn validate_item(
        &self,
        index: usize,
        input: &CommentAnalysisInput,
        output: PacketComment,
    ) -> Result<Value, &'static str> {
        validate_output(&output)?;
        let mut evidence = ItemEvidence {
            input,
            cleaned: &self.cleaned[index],
            spans: Vec::new(),
        };
        let mut labels = Vec::new();
        let mut problems = Vec::new();
        let mut stances = Vec::new();
        for label in &output.labels {
            let name = serde_json::to_value(&label.label).map_err(|_| "item_schema_invalid")?;
            labels.push(json!({"label":name,"evidence":evidence.resolve(&label.evidence,ResearchDimension::Expression,name.as_str().unwrap_or_default().into())?}));
        }
        for problem in &output.problems {
            if !bounded(&problem.name, 100) || !bounded(&problem.meaning, 200) {
                return Err("problem_bounds");
            }
            if problem.equivalence_reason.chars().count() > 200 {
                return Err("problem_bounds");
            }
            let candidate = problem
                .candidate_ref
                .as_deref()
                .and_then(|r| {
                    self.candidates
                        .iter()
                        .enumerate()
                        .find(|(i, _)| r == format!("P{:03}", i + 1))
                        .map(|(_, c)| c)
                })
                .filter(|_| problem.boundary_match && bounded(&problem.equivalence_reason, 200));
            problems.push(json!({"name":problem.name,"meaning":problem.meaning,"evidence":evidence.resolve(&problem.evidence,ResearchDimension::Problem,problem.name.clone())?,"candidateRef":candidate.map(|c|c.reference),"candidateRevision":candidate.map(|c|c.revision),"boundaryMatch":candidate.is_some(),"equivalenceReason":problem.equivalence_reason,"retrievalMethod":"lexical_terms.v1","serverValidatedCandidate":candidate.is_some()}));
        }
        for stance in &output.stances {
            if !bounded(&stance.target, 100) {
                return Err("stance_bounds");
            }
            stances.push(json!({"target":stance.target,"position":stance.position,"evidence":evidence.resolve(&stance.evidence,ResearchDimension::Expression,stance.target.clone())?}));
        }
        // Legacy readers receive at most eight precise spans; the complete semantic structure is separate.
        evidence.spans.truncate(8);
        let result = CommentAnalysisOutput {
            source_ref: input.source_ref,
            source_sha256: input.source_sha256.clone(),
            spans: evidence.spans,
            limitations: output.limitations,
        };
        let mut result =
            validate_comment_analysis(input, &result).map_err(|_| "facets_or_evidence_invalid")?;
        result["semantic"] = json!({"outcome":output.outcome,"labels":labels,"problems":problems,"stances":stances,"contextMissing":output.context_missing,"uncertaintyReason":output.uncertainty_reason});
        result["candidateSnapshot"]=json!(self.candidates.iter().map(|c|json!({"problemRef":c.reference,"revision":c.revision,"definitionFingerprint":c.fingerprint,"sourceRefs":c.source_refs})).collect::<Vec<_>>());
        result["contextFingerprint"] = json!(comment_source_hash(
            &semantic_context(&input.context).to_string()
        ));
        result["contextRefs"]["researchSourceRefs"] = json!(self.context_refs);
        result["contextRefs"]["workRef"] = input
            .context
            .pointer("/workUrl")
            .and_then(Value::as_str)
            .and_then(|s| s.rsplit('/').next())
            .map(|s| json!(s))
            .unwrap_or(Value::Null);
        result["contextRefs"]["mediaJobs"] = json!(
            input.context["derivatives"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|v| v["state"] == "ACQUIRED" && v["displayText"].is_string())
                .map(|v| v["jobRef"].clone())
                .collect::<Vec<_>>()
        );
        Ok(result)
    }
}
