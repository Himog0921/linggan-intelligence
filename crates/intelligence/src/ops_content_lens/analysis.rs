use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use uuid::Uuid;

/// The run stores the ordered shards; a foundation item refers to one index.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelShardRef {
    pub run_ref: Uuid,
    pub shard_index: u32,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelMethod {
    pub taxonomy_ref: Uuid,
    pub task_config_ref: Uuid,
    pub prompt_version_ref: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, Ord, PartialOrd, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LabelDimension {
    Moment,
    Offer,
    Situation,
    Audience,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveLabel {
    pub value: String,
    pub run_ref: Uuid,
    pub adjudication_ref: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationLink {
    pub work_ref: Uuid,
    pub revision_ref: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountSource {
    ViewReport(Uuid),
    Publication(Uuid),
    FieldAdoption(Uuid),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourcedCount {
    pub value: Option<u64>,
    pub source: Option<CountSource>,
}

/// Keyed by publication_ref in AnalysisInput. The same title key may occur many times.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationInput {
    pub title_key: String,
    pub account_ref: Option<Uuid>,
    pub business_date: String,
    pub labels: BTreeMap<LabelDimension, EffectiveLabel>,
    pub views: SourcedCount,
    pub comments: SourcedCount,
    /// Keyed by the existing creation-publication link_ref.
    pub creation_links: BTreeMap<Uuid, CreationLink>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisInput {
    pub domain_ref: Option<Uuid>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub method: LabelMethod,
    pub total_publications: u64,
    pub with_title: u64,
    pub pending_titles: u64,
    pub publications: BTreeMap<Uuid, PublicationInput>,
}

impl AnalysisInput {
    /// The database combines this hash with metric_ref for snapshot identity.
    /// Sorted map keys make row, dimension and creation-link retrieval order irrelevant.
    pub fn input_hash(&self) -> Result<String, serde_json::Error> {
        Ok(Sha256::digest(serde_json::to_vec(self)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }
}
