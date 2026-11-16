//! L2 `model.v2` payload, per-finding admission and the whole-run decision.
//! Pure functions: the executor owns persistence and publication.
use super::deep_manifest::SubQuestion;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const INVALID: &str = "ai_output_invalid";
pub(crate) const INSUFFICIENT: &str = "content_understanding_deep_insufficient_evidence";
pub(crate) const REJECTED_EXCESS: &str = "content_understanding_deep_rejected_excess";

pub(crate) const BANNED_PATTERNS: [&str; 14] = [
    "标题要有吸引力",
    "内容要有情绪",
    "结构要清晰",
    "结构清晰",
    "多使用数字",
    "抓住痛点",
    "干货满满",
    "值得学习",
    "层层递进",
    "语言生动",
    "通俗易懂",
    "简洁明了",
    "引人入胜",
    "很有价值",
];
pub(crate) const CAUSAL_PATTERNS: [&str; 11] = [
    "导致",
    "带来流量",
    "带来了大量",
    "之所以火",
    "之所以受欢迎",
    "爆款原因",
    "因此爆",
    "因此获得",
    "必然",
    "一定会",
    "让笔记",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    #[serde(rename = "subQuestionStatus", default)]
    sub_question_status: Vec<DraftStatus>,
    findings: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftStatus {
    id: String,
    status: String,
    reason: Option<String>,
    note: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftFinding {
    id: String,
    answers: String,
    finding_type: String,
    assertion_kind: String,
    statement: String,
    #[serde(default)]
    basis_unit_ids: Vec<String>,
    #[serde(default)]
    basis_finding_ids: Vec<String>,
    support: Vec<DraftSupport>,
    application: Option<DraftApplication>,
    hypothesis: Option<DraftHypothesis>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftSupport {
    material_ref: String,
    excerpt: Option<String>,
    observation: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftApplication {
    original_use: String,
    method: String,
    steps: Vec<String>,
    when_to_use: String,
    adaptation: String,
    avoid_when: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DraftHypothesis {
    mechanism: String,
    observable_signal: String,
}

fn non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}
fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}
fn quoted(statement: &str) -> Vec<&str> {
    statement
        .split('「')
        .skip(1)
        .filter_map(|part| part.split_once('」').map(|(quote, _)| quote))
        .filter(|quote| non_empty(quote))
        .collect()
}
fn materials(manifest: &Value) -> BTreeMap<&str, &Value> {
    manifest
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|material| Some((material["id"].as_str()?, material)))
        .collect()
}
fn support_type(material: &Value) -> Option<&'static str> {
    match material["materialKind"].as_str()? {
        "title" | "body" | "tag" | "asr" => Some("verbatim_text"),
        "cover" | "image" => Some("visual_observation"),
        "core_evidence" if material["evidenceKind"].as_str() == Some("model_read") => {
            Some("model_read")
        }
        "core_evidence" => Some("verbatim_text"),
        _ => None,
    }
}
fn persisted_support(
    support: &DraftSupport,
    materials: &BTreeMap<&str, &Value>,
) -> Result<(&'static str, Value), &'static str> {
    let material = materials
        .get(support.material_ref.as_str())
        .ok_or("unknown_material")?;
    let kind = support_type(material).ok_or("unknown_material")?;
    if kind == "visual_observation" {
        let observation = support
            .observation
            .as_deref()
            .filter(|value| non_empty(value))
            .ok_or("assertion_rule")?;
        if observation.contains(['「', '」']) {
            return Err("quote_mark_misuse");
        }
        return Ok((
            kind,
            json!({"materialRef":support.material_ref,"observation":observation,"supportType":kind}),
        ));
    }
    let text = material["text"].as_str().ok_or("unknown_material")?;
    let excerpt = support
        .excerpt
        .as_deref()
        .filter(|value| non_empty(value))
        .ok_or("excerpt_not_found")?;
    if !text.contains(excerpt) {
        return Err("excerpt_not_found");
    }
    if kind == "verbatim_text" && excerpt.chars().count() < 4 {
        return Err("excerpt_too_short");
    }
    Ok((
        kind,
        json!({"materialRef":support.material_ref,"excerpt":excerpt,"supportType":kind}),
    ))
}
fn jaccard(statement: &str, reference: &str) -> f64 {
    let bigrams = |text: &str| -> BTreeSet<(char, char)> {
        text.chars()
            .filter(|value| !value.is_whitespace())
            .collect::<Vec<_>>()
            .windows(2)
            .map(|pair| (pair[0], pair[1]))
            .collect()
    };
    let left = bigrams(statement);
    let right = bigrams(reference);
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    left.intersection(&right).count() as f64 / left.union(&right).count() as f64
}
fn flags(
    statement: &str,
    card: &Value,
    supports: &[Value],
    materials: &BTreeMap<&str, &Value>,
    basis: &[String],
) -> Value {
    let mut reference = card["core"]["summary"]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let mut restates = jaccard(statement, &reference) >= 0.6;
    for unit in card["core"]["knowledgeUnits"].as_array().into_iter().flatten() {
        if let Some(text) = unit["statement"].as_str() {
            reference.push('\n');
            reference.push_str(text);
            restates = restates || jaccard(statement, text) >= 0.6;
        }
    }
    let mut external = false;
    let mut title_only = !supports.is_empty();
    for support in supports {
        let kind = support["materialRef"]
            .as_str()
            .and_then(|reference| materials.get(reference))
            .and_then(|material| material["materialKind"].as_str())
            .unwrap_or_default();
        if !matches!(kind, "core_evidence" | "core_unit") {
            external = true;
        }
        if kind != "title" {
            title_only = false;
        }
    }
    json!({
        "restatesCore":restates,
        "noNewMaterial":!external && basis.len() < 2,
        "titleOnly":title_only
    })
}

/// Per-finding admission. The first failing rule becomes the reject code.
fn admit(
    finding: &Value,
    index: usize,
    positions: &BTreeMap<String, usize>,
    materials: &BTreeMap<&str, &Value>,
    card: &Value,
    answered: &BTreeSet<String>,
    earlier: &BTreeMap<String, String>,
) -> Result<Value, &'static str> {
    let draft: DraftFinding =
        serde_json::from_value(finding.clone()).map_err(|_| "assertion_rule")?;
    if finding["id"].as_str().is_none()
        || draft.id.trim().is_empty()
        || !answered.contains(&draft.answers)
    {
        return Err("answers_unknown");
    }
    if !["hook", "structure", "value_delivery", "expression_mechanism", "engagement_hypothesis", "transferable_method"]
        .contains(&draft.finding_type.as_str())
        || !["observation", "author_claim", "interpretation", "hypothesis"]
            .contains(&draft.assertion_kind.as_str())
        || draft.statement.chars().count() > 400
        || !non_empty(&draft.statement)
    {
        return Err("assertion_rule");
    }
    if draft.finding_type == "engagement_hypothesis" && draft.assertion_kind != "hypothesis" {
        return Err("assertion_rule");
    }
    if contains_any(&draft.statement, &BANNED_PATTERNS) {
        return Err("banned_pattern");
    }
    if matches!(draft.assertion_kind.as_str(), "observation" | "interpretation")
        && contains_any(&draft.statement, &CAUSAL_PATTERNS)
    {
        return Err("causal_language");
    }
    if draft.support.is_empty() || draft.support.len() > 6 {
        return Err("assertion_rule");
    }
    let unit_ids: BTreeSet<&str> = card["core"]["knowledgeUnits"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|unit| unit["id"].as_str())
        .collect();
    if draft
        .basis_unit_ids
        .iter()
        .any(|unit| !unit_ids.contains(unit.as_str()))
    {
        return Err("assertion_rule");
    }
    let mut supports = Vec::new();
    let mut types = Vec::new();
    for support in &draft.support {
        let (kind, value) = persisted_support(support, materials)?;
        types.push(kind);
        supports.push(value);
    }
    if types
        .iter()
        .any(|kind| !matches!(*kind, "verbatim_text" | "visual_observation" | "model_read"))
    {
        return Err("unknown_material");
    }
    for quote in quoted(&draft.statement) {
        let supported = supports.iter().any(|support| {
            support["supportType"] == json!("verbatim_text")
                && support["excerpt"]
                    .as_str()
                    .is_some_and(|excerpt| excerpt.contains(quote))
        });
        if !supported {
            return Err("quote_mark_misuse");
        }
    }
    match draft.assertion_kind.as_str() {
        "observation" => {
            if !types.iter().any(|kind| *kind != "model_read") {
                return Err("assertion_rule");
            }
        }
        "author_claim" => {
            if !types.iter().any(|kind| *kind == "verbatim_text") {
                return Err("assertion_rule");
            }
        }
        "hypothesis" => {
            let Some(hypothesis) = draft.hypothesis.as_ref() else {
                return Err("hypothesis_incomplete");
            };
            if !non_empty(&hypothesis.mechanism) || !non_empty(&hypothesis.observable_signal) {
                return Err("hypothesis_incomplete");
            }
        }
        _ => {}
    }
    if draft.finding_type == "transferable_method" {
        let Some(application) = draft.application.as_ref() else {
            return Err("application_incomplete");
        };
        if !non_empty(&application.original_use)
            || !non_empty(&application.method)
            || application.steps.is_empty()
            || application.steps.iter().any(|step| !non_empty(step))
            || !non_empty(&application.when_to_use)
            || !non_empty(&application.adaptation)
            || !non_empty(&application.avoid_when)
        {
            return Err("application_incomplete");
        }
        if draft.basis_finding_ids.is_empty() {
            return Err("method_without_basis");
        }
        for basis in &draft.basis_finding_ids {
            if basis == &draft.id || positions.get(basis).is_some_and(|position| *position >= index) {
                return Err("basis_cycle");
            }
            let Some(kind) = earlier.get(basis) else {
                return Err("method_without_basis");
            };
            if !matches!(kind.as_str(), "observation" | "interpretation") {
                return Err("method_without_basis");
            }
        }
    }
    let finding_flags = flags(
        &draft.statement,
        card,
        &supports,
        materials,
        &draft.basis_unit_ids,
    );
    Ok(json!({
        "id":draft.id,"answers":draft.answers,"findingType":draft.finding_type,
        "assertionKind":draft.assertion_kind,"statement":&draft.statement,
        "basisUnitIds":&draft.basis_unit_ids,"basisFindingIds":&draft.basis_finding_ids,
        "support":supports,
        "application":draft.application.map(|value|json!({
            "originalUse":value.original_use,"method":value.method,"steps":value.steps,
            "whenToUse":value.when_to_use,"adaptation":value.adaptation,"avoidWhen":value.avoid_when
        })),
        "hypothesis":draft.hypothesis.map(|value|json!({
            "mechanism":value.mechanism,"observableSignal":value.observable_signal
        })),
        "flags":finding_flags
    }))
}

/// Returns (published findings, rejected items, sub-question statuses).
pub(crate) fn accept(
    text: &str,
    card: &Value,
    manifest: &Value,
    sub_questions: &[SubQuestion],
) -> Result<(Value, Value, Value), &'static str> {
    let draft: Draft = serde_json::from_str(text).map_err(|_| INVALID)?;
    let materials = materials(manifest);
    let expected: BTreeSet<&str> = sub_questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    let mut status = BTreeMap::<String, Value>::new();
    for entry in &draft.sub_question_status {
        if !expected.contains(entry.id.as_str()) {
            continue;
        }
        if entry.status != "answered" && entry.status != "insufficient" {
            continue;
        }
        status.insert(
            entry.id.clone(),
            json!({"id":entry.id,"status":entry.status,"reason":entry.reason,"note":entry.note}),
        );
    }
    for question in sub_questions {
        status.entry(question.id.clone()).or_insert_with(|| {
            json!({"id":question.id.clone(),"status":"insufficient","reason":"missing_status","note":""})
        });
    }
    let answered: BTreeSet<String> = status
        .values()
        .filter(|entry| entry["status"] == json!("answered"))
        .filter_map(|entry| entry["id"].as_str().map(str::to_string))
        .collect();
    let positions: BTreeMap<String, usize> = draft
        .findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| Some((finding["id"].as_str()?.to_string(), index)))
        .collect();
    let total = draft.findings.len();
    let mut published = Vec::new();
    let mut rejected = Vec::new();
    let mut earlier = BTreeMap::<String, String>::new();
    for (index, finding) in draft.findings.iter().enumerate() {
        if index >= 8 {
            rejected.push(json!({"finding":finding,"rejectCode":"assertion_rule"}));
            continue;
        }
        match admit(finding, index, &positions, &materials, card, &answered, &earlier) {
            Ok(value) => {
                earlier.insert(
                    value["id"].as_str().unwrap_or_default().to_string(),
                    value["assertionKind"].as_str().unwrap_or_default().to_string(),
                );
                published.push(value);
            }
            Err(code) => rejected.push(json!({"finding":finding,"rejectCode":code})),
        }
    }
    if published.is_empty() {
        return if total == 0 {
            Err(INSUFFICIENT)
        } else {
            Err(REJECTED_EXCESS)
        };
    }
    if rejected.len() * 3 > total {
        return Err(REJECTED_EXCESS);
    }
    let answered_with_finding: BTreeSet<&str> = published
        .iter()
        .filter_map(|finding| finding["answers"].as_str())
        .collect();
    for entry in status.values_mut() {
        if entry["status"] == json!("answered") {
            let id = entry["id"].as_str().unwrap_or_default();
            if !answered_with_finding.contains(id) {
                entry["status"] = json!("unsupported");
            }
        }
    }
    let statuses: Vec<Value> = sub_questions
        .iter()
        .filter_map(|question| status.remove(&question.id))
        .collect();
    Ok((
        Value::Array(published),
        Value::Array(rejected),
        Value::Array(statuses),
    ))
}

/// Model-facing projection: identity, kind, text and transcript time only.
pub(crate) fn payload(
    question: &str,
    research_mode: &str,
    base_mode: Option<&str>,
    sub_questions: &[SubQuestion],
    card: &Value,
    manifest: &Value,
) -> Value {
    let materials: Vec<Value> = manifest
        .as_array()
        .into_iter()
        .flatten()
        .map(|material| {
            json!({
                "id":material["id"],"materialKind":material["materialKind"],
                "text":material["text"],"timeStartMs":material["timeStartMs"],
                "timeEndMs":material["timeEndMs"]
            })
        })
        .collect();
    json!({
        "question":question,"researchMode":research_mode,"baseMode":base_mode,
        "subQuestions":sub_questions,
        "core":{"summary":card["core"]["summary"],"knowledgeUnits":card["core"]["knowledgeUnits"]},
        "materials":materials,
        "bannedPatterns":BANNED_PATTERNS,"causalPatterns":CAUSAL_PATTERNS,
        "resultSchema":response_schema()
    })
}

pub(crate) fn response_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,
        "required":["subQuestionStatus","findings"],
        "properties":{
            "subQuestionStatus":{"type":"array","maxItems":4,"items":{
                "type":"object","additionalProperties":false,
                "required":["id","status"],
                "properties":{
                    "id":{"type":"string"},
                    "status":{"type":"string","enum":["answered","insufficient"]},
                    "reason":{"type":"string"},
                    "note":{"type":"string"}
                }
            }},
            "findings":{"type":"array","maxItems":8,"items":{
                "type":"object","additionalProperties":false,
                "required":["id","answers","findingType","assertionKind","statement","support"],
                "properties":{
                    "id":{"type":"string"},
                    "answers":{"type":"string"},
                    "findingType":{"type":"string","enum":["hook","structure","value_delivery","expression_mechanism","engagement_hypothesis","transferable_method"]},
                    "assertionKind":{"type":"string","enum":["observation","author_claim","interpretation","hypothesis"]},
                    "statement":{"type":"string","maxLength":400},
                    "basisUnitIds":{"type":"array","items":{"type":"string"}},
                    "basisFindingIds":{"type":"array","items":{"type":"string"}},
                    "support":{"type":"array","minItems":1,"maxItems":6,"items":{
                        "type":"object","additionalProperties":false,
                        "required":["materialRef"],
                        "properties":{
                            "materialRef":{"type":"string"},
                            "excerpt":{"type":"string"},
                            "observation":{"type":"string"}
                        }
                    }},
                    "application":{"type":"object","additionalProperties":false,
                        "required":["originalUse","method","steps","whenToUse","adaptation","avoidWhen"],
                        "properties":{
                            "originalUse":{"type":"string"},"method":{"type":"string"},
                            "steps":{"type":"array","items":{"type":"string"}},
                            "whenToUse":{"type":"string"},"adaptation":{"type":"string"},"avoidWhen":{"type":"string"}
                        }},
                    "hypothesis":{"type":"object","additionalProperties":false,
                        "required":["mechanism","observableSignal"],
                        "properties":{"mechanism":{"type":"string"},"observableSignal":{"type":"string"}}}
                }
            }}
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn questions() -> Vec<SubQuestion> {
        vec![SubQuestion {
            id: "Q1".into(),
            text: "正文怎么展开？".into(),
        }]
    }
    fn card() -> Value {
        json!({
            "core":{"summary":{"text":"摘要"},"knowledgeUnits":[{"id":"u1","statement":"单元"}]},
            "evidence":[]
        })
    }
    fn manifest() -> Value {
        json!([
            {"id":"M1","materialKind":"body","text":"先摆材料，再按步骤核对。","timeStartMs":null,"timeEndMs":null,"mediaSha256":null},
            {"id":"M2","materialKind":"core_evidence","evidenceKind":"model_read","text":"模型说画面有材料。","timeStartMs":null,"timeEndMs":null,"mediaSha256":null}
        ])
    }
    fn finding() -> Value {
        json!({
            "id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation",
            "statement":"作者先摆材料。","basisUnitIds":["u1"],
            "support":[{"materialRef":"M1","excerpt":"先摆材料"}]
        })
    }
    fn draft(findings: Vec<Value>) -> String {
        json!({"subQuestionStatus":[{"id":"Q1","status":"answered"}],"findings":findings})
            .to_string()
    }

    #[test]
    fn accepts_a_grounded_observation_and_derives_support_type() {
        let (published, rejected, statuses) =
            accept(&draft(vec![finding()]), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(rejected, json!([]));
        assert_eq!(published[0]["support"][0]["supportType"], json!("verbatim_text"));
        assert_eq!(published[0]["flags"]["noNewMaterial"], json!(false));
        assert_eq!(statuses[0]["status"], json!("answered"));
    }

    #[test]
    fn rejects_each_rule_with_its_code() {
        let cases: Vec<(Value, &'static str)> = vec![
            (json!({"id":"F1","answers":"Q9","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "answers_unknown"),
            (json!({"id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"MX","excerpt":"先摆材料"}]}), "unknown_material"),
            (json!({"id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"不存在的句子"}]}), "excerpt_not_found"),
            (json!({"id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"先摆"}]}), "excerpt_too_short"),
            (json!({"id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"作者写了「没有的话」。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "quote_mark_misuse"),
            (json!({"id":"F1","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"先摆材料导致读者照做。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "causal_language"),
            (json!({"id":"F1","answers":"Q1","findingType":"transferable_method","assertionKind":"observation","statement":"先把材料摆出来。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "application_incomplete"),
        ];
        for (finding, expected) in cases {
            let error = accept(&draft(vec![finding]), &card(), &manifest(), &questions()).unwrap_err();
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn method_requires_an_earlier_observation_basis() {
        let method = json!({
            "id":"F2","answers":"Q1","findingType":"transferable_method","assertionKind":"interpretation",
            "statement":"把第一步拆成动作。","basisFindingIds":["F1"],
            "support":[{"materialRef":"M1","excerpt":"先摆材料"}],
            "application":{"originalUse":"正文开头","method":"拆动作","steps":["列出第一步"],"whenToUse":"卡住时","adaptation":"换场景","avoidWhen":"不知道做什么时"}
        });
        let (published, _, _) =
            accept(&draft(vec![finding(), method.clone()]), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(published.as_array().unwrap().len(), 2);
        let alone = accept(&draft(vec![method.clone()]), &card(), &manifest(), &questions()).unwrap_err();
        assert_eq!(alone, "method_without_basis");
        let mut later = method.clone();
        later["id"] = json!("F1");
        later["basisFindingIds"] = json!(["F1"]);
        let cycle = accept(&draft(vec![later]), &card(), &manifest(), &questions()).unwrap_err();
        assert_eq!(cycle, "basis_cycle");
    }

    #[test]
    fn whole_run_publishes_at_one_third_and_withholds_above_it() {
        let bad = json!({"id":"F9","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"MX","excerpt":"先摆材料"}]});
        let mut second = finding();
        second["id"] = json!("F2");
        let (published, rejected, _) = accept(
            &draft(vec![finding(), second, bad.clone()]),
            &card(),
            &manifest(),
            &questions(),
        )
        .unwrap();
        assert_eq!(published.as_array().unwrap().len(), 2);
        assert_eq!(rejected.as_array().unwrap().len(), 1);
        assert_eq!(
            accept(&draft(vec![finding(), bad.clone(), bad]), &card(), &manifest(), &questions())
                .unwrap_err(),
            REJECTED_EXCESS
        );
    }

    #[test]
    fn empty_draft_with_insufficient_status_is_not_a_failure() {
        let text = json!({"subQuestionStatus":[{"id":"Q1","status":"insufficient","reason":"no_visual_material","note":"没有图片"}],"findings":[]}).to_string();
        assert_eq!(
            accept(&text, &card(), &manifest(), &questions()).unwrap_err(),
            INSUFFICIENT
        );
    }
}
