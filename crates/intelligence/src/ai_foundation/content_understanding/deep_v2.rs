//! L2 `model.v2` payload, per-finding admission and the whole-run decision.
//! Pure functions: the executor owns persistence and publication.
use super::deep_manifest::SubQuestion;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const INVALID: &str = "ai_output_invalid";
pub(crate) const INSUFFICIENT: &str = "content_understanding_deep_insufficient_evidence";
pub(crate) const REJECTED_EXCESS: &str = "content_understanding_deep_rejected_excess";

/// A run that is not published. Kept with the raw output for evaluation.
#[derive(Debug)]
pub(crate) struct Withheld {
    pub(crate) code: &'static str,
    pub(crate) rejected: Value,
}

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

/// Extra fields are ignored and statuses are read one by one: a format slip
/// drops that item instead of the whole run (the L1 lesson, content-card §6).
#[derive(Deserialize)]
struct Draft {
    #[serde(rename = "subQuestionStatus", default)]
    sub_question_status: Vec<Value>,
    #[serde(default)]
    findings: Vec<Value>,
}
const MAX_FINDINGS: usize = 8;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftStatus {
    id: String,
    status: String,
    reason: Option<String>,
    note: Option<String>,
}
/// One finding may answer several sub-questions. Accept a list, or one string such as
/// "Q1,Q3" / "Q1、Q3" (the form models actually write), and keep an ordered, de-duplicated list.
fn answer_ids<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        One(String),
        Many(Vec<String>),
    }
    let parts: Vec<String> = match Raw::deserialize(deserializer)? {
        Raw::One(text) => text
            .split(|c: char| c == ',' || c == '，' || c == '、' || c == ';' || c == '；' || c.is_whitespace())
            .map(str::to_string)
            .collect(),
        Raw::Many(list) => list,
    };
    let mut ids = Vec::new();
    for part in parts {
        let id = part.trim().to_string();
        if !id.is_empty() && !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftFinding {
    id: String,
    #[serde(deserialize_with = "answer_ids")]
    answers: Vec<String>,
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
#[serde(rename_all = "camelCase")]
struct DraftSupport {
    material_ref: String,
    excerpt: Option<String>,
    observation: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftApplication {
    original_use: String,
    method: String,
    steps: Vec<String>,
    when_to_use: String,
    adaptation: String,
    avoid_when: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
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
    // Punctuation and spacing slips keep the excerpt; the stored text is the
    // material's own contiguous slice.
    let excerpt = if text.contains(excerpt) {
        excerpt
    } else {
        crate::content_understanding::loose_quote_in_source(text, excerpt)
            .ok_or("excerpt_not_found")?
            .1
    };
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
        || draft.answers.is_empty()
        || draft.answers.iter().any(|id| !answered.contains(id))
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
                && support["excerpt"].as_str().is_some_and(|excerpt| {
                    excerpt.contains(quote)
                        || crate::content_understanding::loose_quote_in_source(excerpt, quote)
                            .is_some()
                })
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
) -> Result<(Value, Value, Value), Withheld> {
    let draft: Draft = serde_json::from_str(text).map_err(|_| Withheld {
        code: INVALID,
        rejected: json!([]),
    })?;
    let materials = materials(manifest);
    let expected: BTreeSet<&str> = sub_questions
        .iter()
        .map(|question| question.id.as_str())
        .collect();
    let mut status = BTreeMap::<String, Value>::new();
    let entries = draft
        .sub_question_status
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<DraftStatus>(entry).ok());
    for entry in entries {
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
    // Findings past the cap are kept for evaluation but are not judged, so a
    // long answer cannot withhold the run.
    let total = draft.findings.len().min(MAX_FINDINGS);
    let mut published = Vec::new();
    let mut rejected = Vec::new();
    let mut failed = 0;
    let mut earlier = BTreeMap::<String, String>::new();
    for (index, finding) in draft.findings.iter().enumerate() {
        if index >= MAX_FINDINGS {
            rejected.push(json!({"finding":finding,"rejectCode":"over_limit"}));
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
            Err(code) => {
                failed += 1;
                rejected.push(json!({"finding":finding,"rejectCode":code}));
            }
        }
    }
    if published.is_empty() || failed * 3 > total {
        return Err(Withheld {
            code: if total == 0 {
                INSUFFICIENT
            } else {
                REJECTED_EXCESS
            },
            rejected: Value::Array(rejected),
        });
    }
    let answered_with_finding: BTreeSet<&str> = published
        .iter()
        .flat_map(|finding| finding["answers"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
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
            let mut value = json!({
                "id":material["id"],"materialKind":material["materialKind"],
                "text":material["text"],"timeStartMs":material["timeStartMs"],
                "timeEndMs":material["timeEndMs"]
            });
            // The Skill tells the model to treat model_read evidence as a description,
            // not source text; it can only do that if the marker is sent.
            if let Some(kind) = material["evidenceKind"].as_str() {
                value["evidenceKind"] = json!(kind);
            }
            value
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
                    "answers":{"type":"array","minItems":1,"items":{"type":"string"}},
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

    fn good(id: &str) -> Value {
        let mut value = finding();
        value["id"] = json!(id);
        value
    }
    fn wrap(bad: Value) -> String {
        draft(vec![good("F1"), good("F2"), bad])
    }

    #[test]
    fn rejects_each_rule_with_its_code() {
        let cases: Vec<(Value, &'static str)> = vec![
            (json!({"id":"FB","answers":"Q9","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "answers_unknown"),
            (json!({"id":"FB","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"MX","excerpt":"先摆材料"}]}), "unknown_material"),
            (json!({"id":"FB","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"不存在的句子"}]}), "excerpt_not_found"),
            (json!({"id":"FB","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"甲","support":[{"materialRef":"M1","excerpt":"先摆"}]}), "excerpt_too_short"),
            (json!({"id":"FB","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"作者写了「没有的话」。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "quote_mark_misuse"),
            (json!({"id":"FB","answers":"Q1","findingType":"structure","assertionKind":"observation","statement":"先摆材料导致读者照做。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "causal_language"),
            (json!({"id":"FB","answers":"Q1","findingType":"transferable_method","assertionKind":"observation","statement":"先把材料摆出来。","support":[{"materialRef":"M1","excerpt":"先摆材料"}]}), "application_incomplete"),
        ];
        for (finding, expected) in cases {
            let (_, rejected, _) = accept(&wrap(finding), &card(), &manifest(), &questions()).unwrap();
            assert_eq!(rejected[0]["rejectCode"], json!(expected));
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
            accept(&draft(vec![good("F1"), method.clone()]), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(published.as_array().unwrap().len(), 2);

        let mut no_basis = method.clone();
        no_basis["basisFindingIds"] = json!([]);
        let (_, rejected, _) = accept(&wrap(no_basis), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(rejected[0]["rejectCode"], json!("method_without_basis"));

        let mut self_cycle = method.clone();
        self_cycle["id"] = json!("F1");
        self_cycle["basisFindingIds"] = json!(["F1"]);
        let (_, rejected, _) = accept(&wrap(self_cycle), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(rejected[0]["rejectCode"], json!("basis_cycle"));

        let mut later = method;
        later["id"] = json!("F3");
        later["basisFindingIds"] = json!(["F9"]);
        let mut tail = good("F9");
        tail["support"] = json!([{"materialRef":"M1","excerpt":"先摆材料"}]);
        let (_, rejected, _) = accept(
            &draft(vec![good("F1"), good("F2"), later, good("F9")]),
            &card(),
            &manifest(),
            &questions(),
        )
        .unwrap();
        assert_eq!(rejected[0]["rejectCode"], json!("basis_cycle"));
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
        let withheld = accept(
            &draft(vec![finding(), bad.clone(), bad]),
            &card(),
            &manifest(),
            &questions(),
        )
        .unwrap_err();
        assert_eq!(withheld.code, REJECTED_EXCESS);
        assert_eq!(withheld.rejected.as_array().unwrap().len(), 2);
    }

    #[test]
    fn extra_fields_and_a_bad_status_entry_drop_only_that_item() {
        let mut extra = good("F2");
        extra["confidence"] = json!("high");
        extra["support"][0]["note"] = json!("开头");
        let text = json!({
            "subQuestionStatus":[{"id":"Q1","status":"answered","confidence":1},{"status":"answered"}],
            "findings":[finding(),extra],
            "summary":"多余的顶层字段"
        })
        .to_string();
        let (published, rejected, statuses) =
            accept(&text, &card(), &manifest(), &questions()).unwrap();
        assert_eq!(published.as_array().unwrap().len(), 2);
        assert_eq!(rejected, json!([]));
        assert_eq!(statuses[0]["status"], json!("answered"));
    }

    #[test]
    fn excerpt_with_punctuation_slips_stores_the_material_slice() {
        let mut slipped = finding();
        slipped["statement"] = json!("作者写「先摆材料再按步骤」。");
        slipped["support"] = json!([{"materialRef":"M1","excerpt":"先摆材料 再按步骤"}]);
        let (published, _, _) =
            accept(&draft(vec![slipped]), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(
            published[0]["support"][0]["excerpt"],
            json!("先摆材料，再按步骤")
        );
    }

    #[test]
    fn findings_past_the_cap_are_kept_but_not_judged() {
        let findings: Vec<Value> = (1..=12).map(|n| good(&format!("F{n}"))).collect();
        let (published, rejected, _) =
            accept(&draft(findings), &card(), &manifest(), &questions()).unwrap();
        assert_eq!(published.as_array().unwrap().len(), 8);
        assert_eq!(rejected.as_array().unwrap().len(), 4);
        assert!(
            rejected
                .as_array()
                .unwrap()
                .iter()
                .all(|item| item["rejectCode"] == json!("over_limit"))
        );
    }

    fn two_questions() -> Vec<SubQuestion> {
        vec![
            SubQuestion { id: "Q1".into(), text: "正文怎么展开？".into() },
            SubQuestion { id: "Q2".into(), text: "怎么调整？".into() },
        ]
    }
    fn two_status_draft(findings: Vec<Value>) -> String {
        json!({"subQuestionStatus":[{"id":"Q1","status":"answered"},{"id":"Q2","status":"answered"}],"findings":findings})
            .to_string()
    }
    fn answering(id: &str, answers: Value) -> Value {
        let mut value = good(id);
        value["answers"] = answers;
        value
    }

    #[test]
    fn one_finding_may_answer_several_sub_questions_as_list_or_text() {
        for answers in [json!(["Q1", "Q2"]), json!("Q1,Q2"), json!("Q1、Q2"), json!("Q1, Q2"), json!(["Q1", "Q2", "Q1"])] {
            let text = two_status_draft(vec![answering("F1", answers.clone()), good("F2"), good("F3")]);
            let (published, rejected, statuses) =
                accept(&text, &card(), &manifest(), &two_questions()).unwrap();
            assert_eq!(rejected, json!([]), "{answers}");
            assert_eq!(published[0]["answers"], json!(["Q1", "Q2"]), "{answers}");
            assert_eq!(published[1]["answers"], json!(["Q1"]));
            assert_eq!(statuses[0]["status"], json!("answered"));
            assert_eq!(statuses[1]["status"], json!("answered"), "Q2 is covered by F1");
        }
    }

    #[test]
    fn an_unknown_or_empty_answer_list_is_rejected() {
        for answers in [json!(["Q1", "Q9"]), json!([]), json!(""), json!(" , ")] {
            let text = two_status_draft(vec![good("F1"), good("F2"), answering("FB", answers.clone())]);
            let (_, rejected, _) = accept(&text, &card(), &manifest(), &two_questions()).unwrap();
            assert_eq!(rejected[0]["rejectCode"], json!("answers_unknown"), "{answers}");
        }
    }

    #[test]
    fn a_sub_question_marked_answered_without_a_finding_becomes_unsupported() {
        let text = two_status_draft(vec![good("F1"), good("F2")]);
        let (_, _, statuses) = accept(&text, &card(), &manifest(), &two_questions()).unwrap();
        assert_eq!(statuses[0]["status"], json!("answered"));
        assert_eq!(statuses[1]["status"], json!("unsupported"));
    }

    #[test]
    fn payload_marks_model_read_evidence_only_where_present() {
        let sent = payload(
            "问题",
            "content_design",
            None,
            &questions(),
            &card(),
            &manifest(),
        );
        assert_eq!(sent["materials"][1]["evidenceKind"], json!("model_read"));
        assert!(sent["materials"][0].get("evidenceKind").is_none());
    }

    #[test]
    fn empty_draft_with_insufficient_status_is_not_a_failure() {
        let text = json!({"subQuestionStatus":[{"id":"Q1","status":"insufficient","reason":"no_visual_material","note":"没有图片"}],"findings":[]}).to_string();
        assert_eq!(
            accept(&text, &card(), &manifest(), &questions())
                .unwrap_err()
                .code,
            INSUFFICIENT
        );
    }
}
