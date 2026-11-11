use super::deep::{self, Finding, FindingType};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(crate) const INSUFFICIENT_EVIDENCE: &str = "content_understanding_deep_insufficient_evidence";
const INVALID: &str = "ai_output_invalid";

type TextBlock<'a> = (&'a str, &'a str, &'a str);

fn text_position(entry: &Value) -> Option<(TextBlock<'_>, u64, u64)> {
    let block = (
        entry["sourceRef"].as_str()?,
        entry["sourceVersion"].as_str()?,
        entry["inputCitationId"].as_str()?,
    );
    let (start, end) = (entry["start"].as_u64()?, entry["end"].as_u64()?);
    (entry["quote"].is_string() && start < end).then_some((block, start, end))
}

pub(super) fn payload(question: &str, card: &Value) -> Value {
    let mut blocks = BTreeMap::new();
    let evidence: Vec<_> = card["evidence"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            if let Some(quote) = entry["quote"].as_str() {
                let mut result = json!({"id":entry["id"],"kind":"material_quote","quote":quote});
                if let Some((block, start, end)) = text_position(entry) {
                    let next = blocks.len() + 1;
                    let group = blocks.entry(block).or_insert(next);
                    result["textPosition"] = json!({"block":group,"start":start,"end":end});
                }
                result
            } else {
                json!({"id":entry["id"],"kind":"model_read","modelQuote":entry["modelQuote"],
                    "observation":entry["observation"],"speakerRole":entry["speakerRole"]})
            }
        })
        .collect();
    json!({"question":question,"core":{
        "summary":card["core"]["summary"],"knowledgeUnits":card["core"]["knowledgeUnits"]
    },"evidence":evidence,"resultSchema":response_schema()})
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    findings: Vec<DraftFinding>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftFinding {
    id: String,
    finding_type: FindingType,
    statement: String,
    basis_unit_ids: Vec<String>,
    question_focus: String,
    support: Vec<Support>,
    presentation_order: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Support {
    evidence_ref: String,
    quote: String,
}

fn supports_order(order: &[String], cited: &BTreeMap<&str, &Value>) -> bool {
    match order {
        [] => true,
        [first, second] => cited
            .get(first.as_str())
            .and_then(|entry| text_position(entry))
            .zip(
                cited
                    .get(second.as_str())
                    .and_then(|entry| text_position(entry)),
            )
            .is_some_and(
                |((first_block, _, first_end), (second_block, second_start, _))| {
                    first_block == second_block && first_end <= second_start
                },
            ),
        _ => false,
    }
}

fn quoted_statements_supported(statement: &str, support: &[Support]) -> bool {
    for (open, close) in [('「', '」'), ('“', '”')] {
        for quoted in statement.split(open).skip(1) {
            let Some((quote, _)) = quoted.split_once(close) else {
                return false;
            };
            if quote.trim().is_empty() || !support.iter().any(|s| s.quote.contains(quote)) {
                return false;
            }
        }
    }
    true
}

impl DraftFinding {
    fn validate(self, question: &str, evidence: &BTreeMap<&str, &Value>) -> Option<Finding> {
        if self.question_focus.trim().chars().count() < 2
            || !question.contains(self.question_focus.trim())
            || !(1..=10).contains(&self.support.len())
        {
            return None;
        }
        let mut cited = BTreeMap::new();
        for support in &self.support {
            let id = support.evidence_ref.as_str();
            let entry = *evidence.get(id)?;
            if support.quote.trim().is_empty()
                || !["quote", "modelQuote", "observation"]
                    .iter()
                    .filter_map(|key| entry[*key].as_str())
                    .any(|text| text.contains(&support.quote))
                || cited.insert(id, entry).is_some()
            {
                return None;
            }
        }
        if !quoted_statements_supported(&self.statement, &self.support)
            || !supports_order(&self.presentation_order, &cited)
        {
            return None;
        }
        Some(Finding {
            id: self.id,
            finding_type: self.finding_type,
            statement: self.statement,
            basis_unit_ids: self.basis_unit_ids,
            evidence_refs: self.support.into_iter().map(|s| s.evidence_ref).collect(),
        })
    }
}

pub(crate) fn accept(text: &str, question: &str, card: &Value) -> Result<Value, &'static str> {
    let draft: Draft = serde_json::from_str(text).map_err(|_| INVALID)?;
    if draft.findings.is_empty() {
        return Err(INSUFFICIENT_EVIDENCE);
    }
    let evidence = card["evidence"]
        .as_array()
        .ok_or(INVALID)?
        .iter()
        .filter_map(|entry| Some((entry["id"].as_str()?, entry)))
        .collect();
    let findings = draft
        .findings
        .into_iter()
        .map(|finding| finding.validate(question, &evidence))
        .collect::<Option<Vec<_>>>()
        .ok_or(INVALID)?;
    if !deep::valid_findings(&findings, card) {
        return Err(INVALID);
    }
    serde_json::to_value(findings).map_err(|_| INVALID)
}

pub(crate) fn response_schema() -> Value {
    json!({
        "type":"object", "additionalProperties":false, "required":["findings"],
        "description":"每条发现必须直接回答人工问题，并用所引原材料覆盖该条的每个关键判断。去掉题外背景、重复发现和缺乏直接支持的内容。Core 用于定位，不能替代原材料。区分作者主张、画面所见和推断。statement 直接给读者阅读，使用作者原文、画面或口播等来源名称，省去模型、证据编号和校验过程。证据不足时返回空 findings。",
        "properties":{"findings":{"type":"array","maxItems":20,"items":{
            "type":"object","additionalProperties":false,
            "required":["id","findingType","statement","basisUnitIds","questionFocus","support","presentationOrder"],
            "properties":{
                "id":{"type":"string"},
                "findingType":{"type":"string","enum":["hook","structure","value_delivery","expression_mechanism","engagement_hypothesis","transferable_method"]},
                "statement":{"type":"string","maxLength":600,"description":"写出对问题的回答。直接引语用「」并逐字出现在 support 中。若证据只显示两项内容均存在，就并列描述。表述先后必须填 presentationOrder。将互动效果标为假设，作者主张保留归因。"},
                "basisUnitIds":{"type":"array","maxItems":10,"items":{"type":"string"}},
                "questionFocus":{"type":"string","description":"从问题逐字摘录本条直接回答的具体片段。若无法说明该条怎样回答该片段，删除该条。"},
                "support":{"type":"array","minItems":1,"maxItems":10,"items":{
                    "type":"object","additionalProperties":false,"required":["evidenceRef","quote"],
                    "properties":{
                        "evidenceRef":{"type":"string","description":"仅使用提供的证据 ID，每个 ID 至多一次。"},
                        "quote":{"type":"string","description":"从该证据的 quote、modelQuote 或 observation 中逐字摘录直接支撑发现的连续原句。引文覆盖陈述中的引语、数字和关键判断。"}
                    }
                }},
                "presentationOrder":{"type":"array","maxItems":2,"items":{"type":"string"},
                    "description":"无先后主张填空数组。有先后主张按顺序给出两条 support 内的证据 ID，两条须有同一 textPosition.block 且第一条 end 不晚于第二条 start。位置只能证明同一文本片段的呈现顺序。"}
            }
        }}}
    })
}

#[cfg(test)]
mod tests;
