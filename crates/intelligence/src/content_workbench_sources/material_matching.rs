//! Transparent local intent and relevance rules; no model or taxonomy mutation.
use super::*;

pub(super) fn permitted_terms(input: &[String]) -> Result<Vec<String>, SourceError> {
    if input.len() > 12 {
        return Err(SourceError::Invalid);
    }
    let mut terms = Vec::new();
    for term in input {
        let t = term.trim().to_lowercase();
        if !(2..=40).contains(&t.chars().count())
            || t.chars().any(char::is_control)
            || t.contains(['%', '_'])
        {
            return Err(SourceError::Invalid);
        }
        if ![
            "孩子",
            "家长",
            "儿童",
            "adhd",
            "注意力",
            "方法",
            "问题",
            "我们",
            "你们",
        ]
        .contains(&t.as_str())
        {
            if !terms.contains(&t) {
                terms.push(t);
            }
        }
    }
    Ok(terms)
}
pub(super) fn intent_terms(request: &PrepareRequest) -> Result<Vec<String>, SourceError> {
    if request.title.chars().count() > 500
        || request.angle.chars().count() > 4000
        || request.outline.len() > 100
        || request.outline.iter().any(|s| s.chars().count() > 4000)
    {
        return Err(SourceError::Invalid);
    }
    if let Some(terms) = &request.terms {
        return permitted_terms(terms);
    }
    let intent = format!(
        "{} {} {}",
        request.title,
        request.angle,
        request.outline.join(" ")
    );
    // This explicit, inspectable query family is limited to the first approved workbench topic.
    // It is a retrieval intention, never a newly asserted Topic definition or model classification.
    if intent.contains("作业")
        && ["启动", "开始", "第一步", "催", "拖延", "磨蹭"]
            .iter()
            .any(|t| intent.contains(t))
    {
        return Ok(["作业", "开始", "启动", "拖延", "磨蹭", "催促", "第一步"]
            .map(str::to_owned)
            .to_vec());
    }
    let phrases: Vec<String> = intent
        .split(|c: char| {
            c.is_whitespace() || c.is_ascii_punctuation() || "，。！？、；：（）《》“”".contains(c)
        })
        .filter(|t| (4..=40).contains(&t.chars().count()))
        .take(8)
        .map(str::to_owned)
        .collect();
    permitted_terms(&phrases)
}
pub(super) fn matches(text: &str, terms: &[String]) -> Vec<String> {
    let text = text.to_lowercase();
    terms
        .iter()
        .filter(|t| text.contains(t.as_str()))
        .cloned()
        .collect()
}
pub(super) fn related(text: &str, terms: &[String]) -> Option<Vec<String>> {
    let found = matches(text, terms);
    if terms.first().is_some_and(|t| t == "作业") && terms.len() > 1 {
        // Same nearby passage must establish the scenario and the action/problem together.
        let chars: Vec<char> = text.chars().collect();
        for window in chars.chunks(240) {
            let passage: String = window.iter().collect();
            let hits = matches(&passage, terms);
            if hits.iter().any(|t| t == "作业") && hits.iter().any(|t| t != "作业") {
                return Some(hits);
            }
        }
        // Overlapping windows cover a pair crossing a page/chunk boundary.
        for window in chars.get(120..).unwrap_or_default().chunks(240) {
            let hits = matches(&window.iter().collect::<String>(), terms);
            if hits.iter().any(|t| t == "作业") && hits.iter().any(|t| t != "作业") {
                return Some(hits);
            }
        }
        None
    } else if found.len() >= 2 || found.iter().any(|s| s.chars().count() >= 4) {
        Some(found)
    } else {
        None
    }
}
pub(super) fn reasons(terms: &[String], parent: bool) -> Vec<String> {
    let mut out = vec![format!("当前原文命中：{}", terms.join("、"))];
    if parent {
        out.push("同篇父笔记已匹配，评论自身包含问题或动作词".into());
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_passage_requires_scenario_and_problem_not_general_adhd() {
        let request = PrepareRequest {
            domain_ref: Uuid::new_v4(),
            title: "每天催三遍，不如把第一步变小".into(),
            angle: "作业启动".into(),
            outline: vec![],
            topic_ref: None,
            terms: None,
        };
        let terms = intent_terms(&request).unwrap();
        assert!(related("孩子ADHD注意力问题，与家长一起加油", &terms).is_none());
        assert!(related("写作业一直拖延，如何开始第一步😀", &terms).is_some());
        assert!(related(&format!("作业{}拖延", "甲".repeat(500)), &terms).is_none());
        assert!(related("开始做家务", &terms).is_none());
        assert!(related("作业很顺利", &terms).is_none());
    }
    #[test]
    fn no_single_character_wildcards_or_empty_inventory_fallback() {
        assert!(permitted_terms(&["的".into()]).is_err());
        assert!(permitted_terms(&["%作业".into()]).is_err());
        assert!(permitted_terms(&["ADHD".into()]).unwrap().is_empty());
        assert!(related("随便一篇新笔记", &[]).is_none());
    }
}
