//! Literal document-frequency vocabulary and bounded, non-authoritative vector recall.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

const PHRASES: &[&str] = &[
    "ADHD",
    "adhd",
    "A娃",
    "a娃",
    "多动症",
    "注意缺陷",
    "执行功能",
    "启动困难",
    "注意力",
    "专注力",
    "感统",
    "情绪调节",
    "情绪失控",
    "家庭干预",
    "行为干预",
    "家校沟通",
    "陪写作业",
    "写作业",
    "家长",
    "孩子",
    "老师",
    "学校",
    "幼儿园",
    "副作用",
    "药物",
    "吃药",
    "停药",
    "睡眠",
    "失眠",
    "拖延",
    "焦虑",
    "抑郁",
    "共病",
    "多动",
    "冲动",
    "奖励",
    "惩罚",
    "正反馈",
    "负反馈",
    "内耗",
    "确诊",
    "诊断",
    "评估",
    "执行成本",
    "亲子关系",
    "亲子冲突",
    "对立违抗",
    "神经多样性",
    "天线宝宝",
    "玻璃心",
    "高敏感",
    "身体双倍",
    "任务启动",
    "时间管理",
    "番茄钟",
];
const STOPS: &[&str] = &[
    "这个",
    "那个",
    "我们",
    "你们",
    "他们",
    "真的",
    "就是",
    "但是",
    "然后",
    "因为",
    "所以",
    "已经",
    "现在",
    "没有",
    "一个",
    "还是",
    "可以",
    "自己",
    "什么",
    "怎么",
    "时候",
    "这样",
    "那样",
    "都是",
    "不是",
    "感觉",
    "可能",
    "谢谢",
    "哈哈",
    "哈哈哈",
    "哈哈哈哈",
];

/// Terms are literal spans of conservatively cleaned comments. The dictionary preserves
/// community phrases; unknown short phrases remain searchable instead of becoming invented tags.
pub fn comment_terms(body: &str) -> Vec<String> {
    if !body.trim().is_empty()
        && body.split_whitespace().all(|token| {
            token.starts_with('@')
                && token
                    .chars()
                    .skip(1)
                    .all(|c| c.is_alphanumeric() || c == '_')
        })
    {
        return Vec::new();
    }
    let clean = crate::comment_cleaning::clean(body);
    if !["direct", "context"].contains(&clean.state.as_str()) {
        return Vec::new();
    }
    let mut terms = BTreeSet::new();
    let mut remaining = clean.text.clone();
    for phrase in PHRASES {
        if remaining.contains(phrase) {
            terms.insert((*phrase).to_owned());
            remaining = remaining.replace(phrase, " ");
        }
    }
    // Common grammatical boundaries reduce full-sentence chunks without stripping negation
    // from the source or modifying the semantic packet.
    for clause in remaining.split(|c: char| {
        c.is_whitespace()
            || (!c.is_alphanumeric() && c != '_')
            || "的了着和与在是吗呢吧啊哦也都很把被让给从到就又才但而".contains(c)
    }) {
        let count = clause.chars().count();
        if (2..=12).contains(&count)
            && !STOPS.contains(&clause)
            && !clause.chars().all(|c| c.is_numeric())
        {
            terms.insert(clause.to_owned());
        }
    }
    terms
        .into_iter()
        .filter(|term| body.contains(term))
        .take(80)
        .collect()
}

pub fn definition_key(name: &str, meaning: &str) -> String {
    // Only casing and surrounding whitespace are normalized. Negation, punctuation and
    // qualifiers remain significant; vocabulary overlap is not semantic equivalence.
    let canonical = format!(
        "{}\u{0}{}",
        name.trim().to_lowercase(),
        meaning.trim().to_lowercase()
    );
    Sha256::digest(canonical.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Providers may implement this port after separate embedding capability qualification.
/// A vector's identity includes its model and dimension; a chat-model flag is insufficient.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DefinitionVector {
    pub problem_ref: Uuid,
    pub model_version: String,
    pub dimensions: usize,
    pub values: Vec<f64>,
}

pub trait DefinitionEmbeddingPort {
    fn embed(&self, semantic_definition: &str) -> Result<DefinitionVector, String>;
}

/// Exact cosine candidate retrieval. Results are capped at ten and carry no assignment or
/// merge authority. Callers must compare definitions, evidence and counterexamples separately.
pub fn recall_vector_candidates(
    query: &DefinitionVector,
    candidates: &[DefinitionVector],
) -> Result<Vec<(Uuid, f64)>, &'static str> {
    fn valid(v: &DefinitionVector) -> bool {
        v.dimensions > 0
            && v.dimensions <= 8192
            && v.dimensions == v.values.len()
            && !v.model_version.trim().is_empty()
            && v.values.iter().all(|n| n.is_finite())
            && v.values.iter().map(|n| n * n).sum::<f64>().is_finite()
            && v.values.iter().map(|n| n * n).sum::<f64>() > 0.0
    }
    if !valid(query) {
        return Err("invalid_embedding");
    }
    let norm = query.values.iter().map(|n| n * n).sum::<f64>().sqrt();
    let mut matches = Vec::new();
    for candidate in candidates {
        if !valid(candidate)
            || candidate.model_version != query.model_version
            || candidate.dimensions != query.dimensions
        {
            return Err("incomparable_embedding");
        }
        let dot: f64 = query
            .values
            .iter()
            .zip(&candidate.values)
            .map(|(a, b)| a * b)
            .sum();
        let cosine = dot / (norm * candidate.values.iter().map(|n| n * n).sum::<f64>().sqrt());
        matches.push((candidate.problem_ref, cosine.clamp(-1.0, 1.0)));
    }
    matches.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    matches.truncate(10);
    Ok(matches)
}
