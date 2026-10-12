//! Only the proposal's exact citations can support a fact under review.
use super::super::{Citation, valid_fragments};
use linggan_evidence::creator_discovery::Fragment;
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) struct EvidenceTable<'a> {
    fragments: &'a [Fragment],
    ids: HashMap<(String, usize, usize), String>,
    pub entries: Vec<Value>,
}

impl<'a> EvidenceTable<'a> {
    pub fn new(fragments: &'a [Fragment]) -> Result<Self, &'static str> {
        if !valid_fragments(fragments) {
            return Err("semantic_review_source_invalid");
        }
        Ok(Self {
            fragments,
            ids: HashMap::new(),
            entries: vec![],
        })
    }

    pub fn citations(&mut self, citations: &[Citation]) -> Result<Vec<String>, &'static str> {
        citations.iter().map(|c| self.add(c)).collect()
    }

    fn add(&mut self, citation: &Citation) -> Result<String, &'static str> {
        let f = self
            .fragments
            .iter()
            .find(|f| f.fragment_id == citation.fragment_id)
            .ok_or("semantic_review_source_invalid")?;
        if citation.start < f.start || citation.end > f.end || citation.start >= citation.end {
            return Err("semantic_review_source_invalid");
        }
        let key = (citation.fragment_id.clone(), citation.start, citation.end);
        if let Some(id) = self.ids.get(&key) {
            return Ok(id.clone());
        }
        let chars: Vec<_> = f.text.chars().collect();
        let start = citation.start - f.start;
        let end = citation.end - f.start;
        let text: String = chars[start..end].iter().collect();
        let id = format!("e{}", self.entries.len());
        self.entries
            .push(json!({"evidenceId":id,"fragmentId":f.fragment_id,
            "field":f.field,"start":citation.start,"end":citation.end,"text":text,
            "contextOnly":f.field=="parent_comment_context" || f.field.starts_with("work_context:")}));
        self.ids.insert(key, id.clone());
        Ok(id)
    }
}

/// Absence/insufficiency requires looking at the actual frozen input, not an empty proposal.
/// This table never supplies missing citations for an analyzed claim.
pub(super) fn outcome_context(fragments: &[Fragment]) -> Value {
    json!(
        fragments
            .iter()
            .map(|f| json!({"fragmentId":f.fragment_id,
        "field":f.field,"text":f.text,"contextOnly":true}))
            .collect::<Vec<_>>()
    )
}
