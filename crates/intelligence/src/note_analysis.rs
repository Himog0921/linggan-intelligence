//! One note's derived understanding; never a topic assignment or comment-study result.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod schema;
mod validation;
pub use schema::response_schema;
pub use validation::{accept, context, validate};
pub const CONTRACT: &str = "note-analysis.result.v1";
pub const METHOD: &str = "note-analysis.method.v0.3";
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Note,
    NoteWithComments,
}
impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::NoteWithComments => "note_with_comments",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub citation_id: String,
    pub kind: String,
    pub source_ref: Uuid,
    pub source_version: String,
    pub start: usize,
    pub text: String,
    pub speaker: String,
    pub parent_citation_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Packet {
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub scope: Scope,
    pub as_of: String,
    pub material_snapshot_identity: String,
    pub coverage: serde_json::Value,
    pub sources: Vec<Source>,
}
impl Packet {
    /// Counts observed on the platform are display metadata, not causal model evidence.
    pub fn model_material(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).expect("packet serialization");
        value["coverage"]
            .as_object_mut()
            .map(|coverage| coverage.remove("platformCommentCount"));
        value
    }
    pub fn fingerprint(&self) -> String {
        crate::research_text::content_hash(
            &serde_json::json!([self.domain_ref, self.work_ref, self.scope, self.sources])
                .to_string(),
        )
    }
    pub fn hash(&self) -> String {
        crate::research_text::content_hash(
            &serde_json::to_string(self).expect("packet serialization"),
        )
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Observation,
    AuthorClaim,
    Interpretation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Statement {
    pub id: String,
    pub text: String,
    pub kind: Kind,
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Understanding {
    pub discussion: Statement,
    pub audience: Option<Statement>,
    pub situation: Option<Statement>,
    pub answer: Statement,
    pub offered_value: Option<Statement>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Feature {
    pub id: String,
    pub text: String,
    pub kind: Kind,
    pub evidence_refs: Vec<String>,
    pub label: String,
    pub category: FeatureCategory,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeatureCategory {
    AnswerForm,
    Expression,
    Participation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Explanation {
    pub id: String,
    pub text: String,
    pub kind: Interpretation,
    pub feature_ids: Vec<String>,
    pub feedback_ids: Vec<String>,
    pub evidence_refs: Vec<String>,
    pub support: String,
    pub uncertainty: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpretation {
    Interpretation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Tag {
    pub label: String,
    pub dimension: Dimension,
    pub judgment_ids: Vec<String>,
    pub retrieval_use: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    Discussion,
    Audience,
    Situation,
    OfferedValue,
    AnswerForm,
    Expression,
    Participation,
    Response,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Method {
    pub id: String,
    pub text: String,
    pub kind: ReuseIdea,
    pub basis_ids: Vec<String>,
    pub conditions: String,
    pub open_question: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReuseIdea {
    ReuseIdea,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub id: String,
    pub kind: String,
    pub source_ref: Uuid,
    pub source_version: String,
    pub citation_id: String,
    pub start: usize,
    pub end: usize,
    pub quote: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub contract: String,
    pub summary: String,
    pub summary_basis_ids: Vec<String>,
    pub understanding: Understanding,
    pub features: Vec<Feature>,
    pub feedback: Vec<Statement>,
    pub explanations: Vec<Explanation>,
    pub tags: Vec<Tag>,
    pub method_candidates: Vec<Method>,
    pub limitations: Vec<String>,
    pub evidence: Vec<Evidence>,
}
impl Understanding {
    pub fn statements(&self) -> Vec<&Statement> {
        let mut v = vec![&self.discussion, &self.answer];
        v.extend(self.audience.iter());
        v.extend(self.situation.iter());
        v.extend(self.offered_value.iter());
        v
    }
}
