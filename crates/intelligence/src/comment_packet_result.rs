//! Per-field acceptance with exact target-comment evidence and separately scoped context support.
use super::{
    ContextQuote, EvidenceBasis, Outcome, ProblemCandidate, Quote, ResearchPacket, SemanticLabel,
    Stance, semantic_context_for_comment,
};
use crate::{
    comment_analysis::{
        CommentAnalysisInput, CommentAnalysisOutput, CommentAnalysisSpan, validate_comment_analysis,
    },
    comment_cleaning::{CleanComment, clean, outbound},
    comment_research::{ResearchBasis, ResearchDimension, ResearchFacet, comment_source_hash},
};
use serde::de::{Error as DeError, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::{collections::HashSet, fmt};
#[path = "comment_packet_diagnostics.rs"]
mod diagnostics;
#[cfg(test)]
#[path = "comment_packet_parser_tests.rs"]
mod parser_tests;

const DUPLICATE_KEY_MARKER: &str = "comment_packet_duplicate_json_key";

/// `serde_json::Value` keeps the last duplicate map entry. Walk the input once before decoding
/// into `Value` so an ambiguous provider response cannot silently replace an earlier value.
struct NoDuplicateJsonKeys;

struct NoDuplicateJsonKeysVisitor;

impl<'de> Deserialize<'de> for NoDuplicateJsonKeys {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(NoDuplicateJsonKeysVisitor)
    }
}

impl<'de> Visitor<'de> for NoDuplicateJsonKeysVisitor {
    type Value = NoDuplicateJsonKeys;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value without duplicate object keys")
    }

    fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_str<E>(self, _: &str) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_string<E>(self, _: String) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: DeError,
    {
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element::<NoDuplicateJsonKeys>()?.is_some() {}
        Ok(NoDuplicateJsonKeys)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = HashSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key) {
                return Err(A::Error::custom(DUPLICATE_KEY_MARKER));
            }
            map.next_value::<NoDuplicateJsonKeys>()?;
        }
        Ok(NoDuplicateJsonKeys)
    }
}

pub(crate) struct NormalizedEnvelope {
    pub(crate) value: Value,
    pub(crate) kind: &'static str,
}

pub(crate) fn normalized_envelope(raw: &str) -> Result<NormalizedEnvelope, &'static str> {
    let trimmed = raw.trim();
    let (json, kind) = if trimmed.starts_with("```") {
        let body = trimmed
            .strip_prefix("```json")
            .and_then(|rest| {
                rest.strip_prefix('\n')
                    .or_else(|| rest.strip_prefix("\r\n"))
            })
            .ok_or("json_fence_invalid")?;
        let closing = body.rfind("\n```").ok_or("json_fence_invalid")?;
        if &body[closing + 1..] != "```" {
            return Err("json_fence_invalid");
        }
        (&body[..closing], "json_fence")
    } else {
        (trimmed, "bare_json")
    };
    match serde_json::from_str::<NoDuplicateJsonKeys>(json) {
        Ok(_) => {}
        Err(error) if error.to_string().contains(DUPLICATE_KEY_MARKER) => {
            return Err("duplicate_json_key");
        }
        Err(_) => return Err("json_invalid"),
    }
    let value = serde_json::from_str(json).map_err(|_| "json_invalid")?;
    Ok(NormalizedEnvelope { value, kind })
}

fn bounded(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.chars().count() <= max
}
struct ItemFields<'a> {
    outcome: Outcome,
    labels: &'a [Value],
    problems: &'a [Value],
    stances: &'a [Value],
}

fn item_fields(raw: &Value) -> Result<ItemFields<'_>, &'static str> {
    const REQUIRED: [&str; 8] = [
        "commentRef",
        "outcome",
        "labels",
        "problems",
        "stances",
        "contextMissing",
        "uncertaintyReason",
        "limitations",
    ];
    let object = raw.as_object().ok_or("item_schema_invalid")?;
    if object.len() != REQUIRED.len()
        || REQUIRED.iter().any(|field| !object.contains_key(*field))
        || raw["commentRef"].as_str().is_none()
    {
        return Err("item_schema_invalid");
    }
    let outcome =
        serde_json::from_value(raw["outcome"].clone()).map_err(|_| "item_schema_invalid")?;
    let labels = raw["labels"].as_array().ok_or("item_schema_invalid")?;
    let problems = raw["problems"].as_array().ok_or("item_schema_invalid")?;
    let stances = raw["stances"].as_array().ok_or("item_schema_invalid")?;
    if labels.len() > 8 || problems.len() > 8 || stances.len() > 8 {
        return Err("output_bounds");
    }
    Ok(ItemFields {
        outcome,
        labels,
        problems,
        stances,
    })
}

fn accepted_auxiliary_list(raw: &Value, name: &str, rejected: &mut Vec<Value>) -> Vec<String> {
    let Some(items) = raw[name].as_array() else {
        rejected.push(json!({"path":name,"code":"field_schema_invalid"}));
        return Vec::new();
    };
    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if index >= 8 {
                rejected.push(
                    json!({"path":format!("{name}[{index}]"),"code":"auxiliary_items_bounds"}),
                );
                return None;
            }
            match item.as_str() {
                Some(value) if bounded(value, 200) => Some(value.to_owned()),
                Some(_) => {
                    rejected.push(
                        json!({"path":format!("{name}[{index}]"),"code":"auxiliary_text_bounds"}),
                    );
                    None
                }
                None => {
                    rejected.push(
                        json!({"path":format!("{name}[{index}]"),"code":"field_schema_invalid"}),
                    );
                    None
                }
            }
        })
        .collect()
}

fn accepted_auxiliary_reason(raw: &Value, rejected: &mut Vec<Value>) -> Option<String> {
    match &raw["uncertaintyReason"] {
        Value::Null => None,
        Value::String(value) if bounded(value, 200) => Some(value.to_owned()),
        Value::String(_) => {
            rejected.push(json!({"path":"uncertaintyReason","code":"auxiliary_text_bounds"}));
            None
        }
        _ => {
            rejected.push(json!({"path":"uncertaintyReason","code":"field_schema_invalid"}));
            None
        }
    }
}

struct ValidatedItem<'a> {
    fields: ItemFields<'a>,
    context_missing: Vec<String>,
    limitations: Vec<String>,
    uncertainty_reason: Option<String>,
    rejected: Vec<Value>,
}

fn validated_item(raw: &Value) -> Result<ValidatedItem<'_>, &'static str> {
    let fields = item_fields(raw)?;
    let mut rejected = Vec::new();
    let context_missing = accepted_auxiliary_list(raw, "contextMissing", &mut rejected);
    let limitations = accepted_auxiliary_list(raw, "limitations", &mut rejected);
    let uncertainty_reason = accepted_auxiliary_reason(raw, &mut rejected);
    if fields.outcome == Outcome::Uncertain && uncertainty_reason.is_none() {
        return Err("uncertainty_reason_missing");
    }
    let semantic_fields_present =
        !fields.labels.is_empty() || !fields.problems.is_empty() || !fields.stances.is_empty();
    if fields.outcome != Outcome::Interpretable && semantic_fields_present {
        return Err("outcome_conflict");
    }
    if fields.outcome == Outcome::Interpretable && !semantic_fields_present {
        return Err("interpretable_without_evidence");
    }
    Ok(ValidatedItem {
        fields,
        context_missing,
        limitations,
        uncertainty_reason,
        rejected,
    })
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
        basis: &EvidenceBasis,
    ) -> Result<Vec<Value>, &'static str> {
        if quotes.is_empty() || quotes.len() > 4 {
            return Err("evidence_bounds");
        }
        let mut refs = Vec::new();
        let mut spans = Vec::new();
        for q in quotes {
            let (start_char, end_char, quote) = self.cleaned.resolve(&q.quote, &self.input.body)?;
            refs.push(json!({"sourceRef":self.input.source_ref,"startChar":start_char,"endChar":end_char}));
            if *basis != EvidenceBasis::Uncertain {
                spans.push(CommentAnalysisSpan {
                    source_ref: self.input.source_ref,
                    start_char,
                    end_char,
                    quote,
                    facets: vec![ResearchFacet {
                        dimension: dimension.clone(),
                        label: label.clone(),
                        basis: if *basis == EvidenceBasis::Explicit {
                            ResearchBasis::Explicit
                        } else {
                            ResearchBasis::Inferred
                        },
                    }],
                });
            }
        }
        // A later invalid quote must not leave accepted spans from this rejected field.
        self.spans.extend(spans);
        Ok(refs)
    }
}
impl ResearchPacket {
    /// The caller retains the original provider text; this only exposes which strict transport
    /// form was safe to normalize for trace metadata.
    pub fn normalization_kind(raw: &str) -> Option<&'static str> {
        normalized_envelope(raw)
            .ok()
            .map(|normalized| normalized.kind)
    }

    pub fn parse(&self, raw: &str) -> Result<Vec<Result<Value, &'static str>>, &'static str> {
        let envelope = normalized_envelope(raw)?.value;
        self.parse_envelope(&envelope)
    }

    fn parse_envelope(
        &self,
        envelope: &Value,
    ) -> Result<Vec<Result<Value, &'static str>>, &'static str> {
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
        // Unknown identities invalidate the envelope, rather than silently ignoring invented results.
        if items.iter().any(|item| {
            !item
                .get("commentRef")
                .and_then(Value::as_str)
                .is_some_and(|id| (0..self.inputs.len()).any(|i| id == format!("C{:03}", i + 1)))
        }) {
            return Err("unknown_comment");
        }
        Ok(self
            .inputs
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let id = format!("C{:03}", i + 1);
                let matching: Vec<_> = items
                    .iter()
                    .filter(|item| item["commentRef"] == id)
                    .collect();
                if matching.is_empty() {
                    return Err("missing_comment");
                }
                if matching.len() != 1 {
                    return Err("duplicate_comment");
                }
                let result = self.evaluate_item(i, matching[0])?;
                if result["semantic"]["acceptance"] == "rejected" {
                    Err("all_fields_rejected")
                } else {
                    Ok(result)
                }
            })
            .collect())
    }
    fn context_evidence(
        &self,
        index: usize,
        basis: &EvidenceBasis,
        quotes: &[ContextQuote],
    ) -> Result<Vec<Value>, &'static str> {
        if quotes.len() > 4
            || (*basis == EvidenceBasis::Explicit && !quotes.is_empty())
            || (*basis == EvidenceBasis::ContextResolved && quotes.is_empty())
        {
            return Err("context_evidence_required_or_conflicting");
        }
        let fragments = super::context_selection::fragments(
            &self.inputs[index].context,
            &self.cleaned[index].text,
        );
        let mut result = Vec::new();
        for q in quotes {
            let fragment = fragments
                .as_array()
                .into_iter()
                .flatten()
                .find(|f| f["fragmentRef"] == q.fragment_ref)
                .ok_or("context_fragment_unknown")?;
            let text = fragment["text"]
                .as_str()
                .ok_or("context_fragment_unknown")?;
            if q.quote.is_empty()
                || q.quote.contains('█')
                || text.match_indices(&q.quote).count() != 1
            {
                return Err("context_quote_missing_or_ambiguous");
            }
            let source_path = fragment["sourcePath"]
                .as_str()
                .ok_or("context_fragment_unknown")?;
            let original = self.inputs[index]
                .context
                .pointer(source_path)
                .and_then(Value::as_str)
                .ok_or("context_fragment_unknown")?;
            let clean_source = outbound(clean(original));
            let offset = text
                .find(&q.quote)
                .ok_or("context_quote_missing_or_ambiguous")?;
            let start_index = fragment["cleanStartChar"]
                .as_u64()
                .ok_or("context_fragment_unknown")? as usize
                + text[..offset].chars().count();
            let end_index = start_index + q.quote.chars().count();
            let start = clean_source
                .offsets
                .get(start_index)
                .ok_or("context_quote_missing_or_ambiguous")?
                .0;
            let end = clean_source
                .offsets
                .get(end_index - 1)
                .ok_or("context_quote_missing_or_ambiguous")?
                .1;
            result.push(json!({"fragmentRef":q.fragment_ref,"kind":fragment["kind"],"sourcePath":source_path,"sourceRef":fragment["sourceRef"],"startChar":start,"endChar":end}));
        }
        Ok(result)
    }
    fn resolve_semantic_field(
        &self,
        index: usize,
        group: &str,
        raw_field: &Value,
        evidence: &mut ItemEvidence<'_>,
    ) -> Result<(Value, EvidenceBasis), &'static str> {
        match group {
            "labels" => {
                let field: SemanticLabel = serde_json::from_value(raw_field.clone())
                    .map_err(|_| "field_schema_invalid")?;
                let context =
                    self.context_evidence(index, &field.basis, &field.context_evidence)?;
                let name =
                    serde_json::to_value(&field.label).map_err(|_| "field_schema_invalid")?;
                let refs = evidence.resolve(
                    &field.evidence,
                    ResearchDimension::Expression,
                    name.as_str().unwrap_or_default().into(),
                    &field.basis,
                )?;
                Ok((
                    json!({"label":name,"evidence":refs,"basis":field.basis,"contextEvidence":context}),
                    field.basis,
                ))
            }
            "problems" => {
                let field: ProblemCandidate = serde_json::from_value(raw_field.clone())
                    .map_err(|_| "field_schema_invalid")?;
                if !bounded(&field.name, 100) || !bounded(&field.meaning, 200) {
                    return Err("problem_bounds");
                }
                let context =
                    self.context_evidence(index, &field.basis, &field.context_evidence)?;
                let refs = evidence.resolve(
                    &field.evidence,
                    ResearchDimension::Problem,
                    field.name.clone(),
                    &field.basis,
                )?;
                Ok((
                    json!({"name":field.name,"meaning":field.meaning,"evidence":refs,"basis":field.basis,"contextEvidence":context,"candidateRef":null,"serverValidatedCandidate":false,"retrievalMethod":"none_task_a"}),
                    field.basis,
                ))
            }
            _ => {
                let field: Stance = serde_json::from_value(raw_field.clone())
                    .map_err(|_| "field_schema_invalid")?;
                if !bounded(&field.target, 100) {
                    return Err("stance_bounds");
                }
                let context =
                    self.context_evidence(index, &field.basis, &field.context_evidence)?;
                let refs = evidence.resolve(
                    &field.evidence,
                    ResearchDimension::Expression,
                    field.target.clone(),
                    &field.basis,
                )?;
                Ok((
                    json!({"target":field.target,"position":field.position,"evidence":refs,"basis":field.basis,"contextEvidence":context}),
                    field.basis,
                ))
            }
        }
    }
    /// Evaluates fields without committing acceptance. `parse` is the sole public acceptance path.
    fn evaluate_item(&self, index: usize, raw: &Value) -> Result<Value, &'static str> {
        let ValidatedItem {
            fields: output,
            context_missing,
            limitations,
            uncertainty_reason,
            mut rejected,
        } = validated_item(raw)?;
        let input = &self.inputs[index];
        let mut evidence = ItemEvidence {
            input,
            cleaned: &self.cleaned[index],
            spans: Vec::new(),
        };
        let mut accepted = json!({"labels":[],"problems":[],"stances":[]});
        let mut uncertain = Vec::new();
        for (group, fields) in [
            ("labels", output.labels),
            ("problems", output.problems),
            ("stances", output.stances),
        ] {
            for (n, raw_field) in fields.iter().enumerate() {
                let checkpoint = evidence.spans.len();
                let field = self.resolve_semantic_field(index, group, raw_field, &mut evidence);
                let field = field.and_then(|(value, basis)| {
                    if basis == EvidenceBasis::Uncertain && uncertainty_reason.is_none() {
                        Err("uncertainty_reason_missing")
                    } else {
                        Ok((value, basis))
                    }
                });
                match field {
                    Ok((value, EvidenceBasis::Uncertain)) => {
                        uncertain.push(json!({"field":group,"value":value}))
                    }
                    Ok((value, _)) => accepted[group].as_array_mut().unwrap().push(value),
                    Err(code) => {
                        evidence.spans.truncate(checkpoint);
                        rejected.push(json!({"path":format!("{group}[{n}]"),"code":code}));
                    }
                }
            }
        }
        let kept = accepted
            .as_object()
            .unwrap()
            .values()
            .map(|a| a.as_array().unwrap().len())
            .sum::<usize>();
        let all_rejected =
            output.outcome == Outcome::Interpretable && kept == 0 && uncertain.is_empty();
        let outcome = if !all_rejected && output.outcome == Outcome::Interpretable && kept == 0 {
            Outcome::Uncertain
        } else {
            output.outcome
        };
        evidence.spans.truncate(8);
        let legacy = CommentAnalysisOutput {
            source_ref: input.source_ref,
            source_sha256: input.source_sha256.clone(),
            spans: evidence.spans,
            limitations,
        };
        let mut result =
            validate_comment_analysis(input, &legacy).map_err(|_| "facets_or_evidence_invalid")?;
        result["semantic"] = json!({"outcome":outcome,"labels":accepted["labels"],"problems":accepted["problems"],"stances":accepted["stances"],"uncertainFields":uncertain,"rejectedFields":rejected,"acceptance":if all_rejected {"rejected"} else if rejected.is_empty() {"complete"} else {"partial"},"contextMissing":context_missing,"uncertaintyReason":uncertainty_reason});
        result["schemaVersion"] = json!(super::EXTRACTION_SCHEMA_VERSION);
        result["candidateSnapshot"] = json!([]);
        result["contextFingerprint"] = json!(comment_source_hash(
            &semantic_context_for_comment(&input.context, &self.cleaned[index].text).to_string()
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
