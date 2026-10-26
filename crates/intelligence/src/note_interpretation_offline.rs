//! C0 pure contracts over the shared dossier. No provider, queue, database or serving writes.
//! Mock output has a distinct wire shape; it is never a model or business acceptance receipt.
use linggan_evidence::dossier_input::{self, Column, ColumnState, Fragment, Material};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

mod catalogue;
mod comparison;
pub mod fixture;
mod review;
mod sample;
mod statistics;
pub use catalogue::{CatalogueDraft, DirectionDraft, DirectionGroup};
pub use comparison::{ChangeKind, StructureChange, validate_comparison};
pub use review::{ManualJudgment, ReviewedOutput, project_judgment, validate_judgment};
pub use sample::{FrozenSample, SampleEntry, freeze_sample};
pub use statistics::{NoteStatistics, Summary, summarize};

pub const CONTRACT: &str = "note-interpretation-offline.v1";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("{0}")]
pub struct Invalid(pub &'static str);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Identity {
    pub domain_ref: Uuid,
    pub work_ref: Uuid,
    pub snapshot_identity: String,
    pub direction_version_ref: Uuid,
    pub skill_version_ref: Uuid,
}

/// The caller supplies a frozen catalogue. UUIDs here are membership, never publication proof.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Input {
    pub material: Material,
    pub direction_version_ref: Uuid,
    pub skill_version_ref: Uuid,
    pub direction_refs: Vec<Uuid>,
}

impl Input {
    pub fn from_catalogue_draft(
        material: Material,
        skill_version_ref: Uuid,
        catalogue: &CatalogueDraft,
    ) -> Result<Self, Invalid> {
        catalogue.validate()?;
        let input = Self {
            material,
            direction_version_ref: catalogue.version_ref,
            skill_version_ref,
            direction_refs: catalogue
                .directions
                .iter()
                .map(|d| d.direction_ref)
                .collect(),
        };
        input.validate()?;
        Ok(input)
    }

    pub fn identity(&self) -> Identity {
        Identity {
            domain_ref: self.material.domain_ref,
            work_ref: self.material.work_ref,
            snapshot_identity: self.material.snapshot_identity.clone(),
            direction_version_ref: self.direction_version_ref,
            skill_version_ref: self.skill_version_ref,
        }
    }

    pub fn validate(&self) -> Result<(), Invalid> {
        if self.material.contract != dossier_input::VERSION
            || self.material.fingerprint_contract != dossier_input::FINGERPRINT_VERSION
        {
            return Err(Invalid("offline_material_contract_mismatch"));
        }
        if !self
            .material
            .fragments
            .iter()
            .any(|f| linggan_evidence::research_dossier::content::has_source_text(&f.structure))
        {
            return Err(Invalid("offline_material_no_readable_text"));
        }
        if self.material.domain_ref.is_nil()
            || self.material.work_ref.is_nil()
            || self.material.snapshot_identity.is_empty()
            || self.direction_version_ref.is_nil()
            || self.skill_version_ref.is_nil()
            || self.direction_refs.is_empty()
            || self.direction_refs.iter().any(Uuid::is_nil)
            || self.direction_refs.iter().collect::<BTreeSet<_>>().len()
                != self.direction_refs.len()
        {
            return Err(Invalid("offline_version_or_catalogue_invalid"));
        }
        Ok(())
    }
}

/// Local preview, not a provider payload or egress permission. Retains dossier citations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputPreview {
    pub contract: String,
    pub origin: Origin,
    pub identity: Identity,
    pub as_of: String,
    pub columns: Vec<Column>,
    pub reading_title: serde_json::Value,
    pub fragments: Vec<Fragment>,
}

pub fn prepare_input(input: &Input) -> Result<InputPreview, Invalid> {
    input.validate()?;
    let columns = input.material.columns.clone();
    let fragments = input.material.fragments.clone();
    let mut column_fields = BTreeSet::new();
    if columns.iter().any(|column| {
        !canonical_field(&column.field) || !column_fields.insert(column.field.as_str())
    }) {
        return Err(Invalid("offline_input_column_invalid"));
    }
    let mut citations = BTreeSet::new();
    let mut blocks = BTreeSet::new();
    for fragment in &fragments {
        if fragment.citation_id.is_empty()
            || !citations.insert(fragment.citation_id.clone())
            || fragment.end.checked_sub(fragment.start) != Some(fragment.text.chars().count())
            || !matches!(
                fragment.speaker.as_str(),
                "author" | "other" | "unconfirmed"
            )
        {
            return Err(Invalid("offline_input_fragment_invalid"));
        }
        let content = &fragment.structure;
        if content["ref"].as_str() != Some(fragment.logical_position.as_str())
            || fragment.logical_position.is_empty()
            || !blocks.insert(fragment.logical_position.as_str())
            || linggan_evidence::research_dossier::content::validate_blocks(std::slice::from_ref(
                content,
            ))
            .is_err()
            || linggan_evidence::research_dossier::content::validate_issues(&serde_json::json!(
                fragment.issues
            ))
            .is_err()
            || linggan_evidence::research_dossier::content::block_text(content) != fragment.text
            || content["speaker"].as_str().unwrap_or("unconfirmed") != fragment.speaker
        {
            return Err(Invalid("offline_input_structure_invalid"));
        }
        if !columns.iter().any(|c| {
            c.field == fragment.field
                && matches!(
                    c.state,
                    ColumnState::Available
                        | ColumnState::Waiting
                        | ColumnState::NotAcquired
                        | ColumnState::Restricted
                )
        }) {
            return Err(Invalid("offline_input_column_conflict"));
        }
    }
    Ok(InputPreview {
        contract: "note-interpretation-offline-input.v2".into(),
        origin: Origin::OfflineMock,
        identity: input.identity(),
        as_of: input.material.as_of.clone(),
        columns,
        reading_title: input.material.reading_title.clone(),
        fragments,
    })
}

fn canonical_field(field: &str) -> bool {
    matches!(
        field,
        "title" | "body" | "tags" | "ocr" | "transcript" | "frame_text"
    )
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Relevance {
    Related,
    NotRelated,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Certain,
    Possible,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Perspective {
    ParentLivedExperience,
    AdultSelfReport,
    ProfessionalEducation,
    OrganizationPromotion,
    Other,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keyword {
    pub text: String,
    pub citation_id: String,
    /// Absolute Unicode scalar offsets in the source fragment's coordinate system.
    pub start: usize,
    pub end: usize,
}

/// Only this offline origin is accepted. Real results will need the C1 invocation contract.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    OfflineMock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub contract: String,
    pub origin: Origin,
    pub identity: Identity,
    pub relevance: Relevance,
    pub primary_direction: Option<Uuid>,
    pub also_related: Option<Uuid>,
    pub confidence: Confidence,
    pub perspective: Perspective,
    /// Order is relevance rank. Evidence must match a literal dossier fragment.
    pub keywords: Vec<Keyword>,
    pub reason: Option<String>,
}

pub fn validate_output(input: &Input, output: &Output) -> Result<(), Invalid> {
    prepare_input(input)?;
    if output.contract != CONTRACT || output.identity != input.identity() {
        return Err(Invalid("offline_output_identity_mismatch"));
    }
    if output.relevance != Relevance::Related
        && (output.primary_direction.is_some() || output.also_related.is_some())
    {
        return Err(Invalid("offline_unrelated_or_unknown_assignment"));
    }
    if output.primary_direction.is_some() && output.primary_direction == output.also_related {
        return Err(Invalid("offline_duplicate_direction"));
    }
    for reference in [output.primary_direction, output.also_related]
        .into_iter()
        .flatten()
    {
        if !input.direction_refs.contains(&reference) {
            return Err(Invalid("offline_direction_not_in_version"));
        }
    }
    if output.keywords.len() > 5 {
        return Err(Invalid("offline_too_many_keywords"));
    }
    if output
        .reason
        .as_ref()
        .is_some_and(|s| s.chars().count() > 200)
    {
        return Err(Invalid("offline_reason_too_long"));
    }
    for keyword in &output.keywords {
        let mut fragments = input
            .material
            .fragments
            .iter()
            .filter(|f| f.citation_id == keyword.citation_id);
        let fragment = fragments
            .next()
            .ok_or(Invalid("offline_keyword_citation_missing"))?;
        if fragments.next().is_some()
            || !canonical_field(&fragment.field)
            || !input.material.columns.iter().any(|c| {
                c.field == fragment.field
                    && matches!(
                        c.state,
                        ColumnState::Available
                            | ColumnState::Waiting
                            | ColumnState::NotAcquired
                            | ColumnState::Restricted
                    )
            })
            || fragment.end.checked_sub(fragment.start) != Some(fragment.text.chars().count())
            || keyword.start < fragment.start
            || keyword.end > fragment.end
            || keyword.start >= keyword.end
        {
            return Err(Invalid("offline_keyword_evidence_invalid"));
        }
        let literal: String = fragment
            .text
            .chars()
            .skip(keyword.start - fragment.start)
            .take(keyword.end - keyword.start)
            .collect();
        if keyword.text.is_empty() || keyword.text != literal {
            return Err(Invalid("offline_keyword_not_literal"));
        }
    }
    Ok(())
}

/// No inference: a stable unknown fixture makes absence of a real model explicit.
pub fn mock_unknown(input: &Input) -> Result<Output, Invalid> {
    let preview = prepare_input(input)?;
    Ok(Output {
        contract: CONTRACT.into(),
        origin: Origin::OfflineMock,
        identity: preview.identity,
        relevance: Relevance::Unknown,
        primary_direction: None,
        also_related: None,
        confidence: Confidence::Possible,
        perspective: Perspective::Unknown,
        keywords: vec![],
        reason: Some("离线示例；尚未进行模型分析".into()),
    })
}
