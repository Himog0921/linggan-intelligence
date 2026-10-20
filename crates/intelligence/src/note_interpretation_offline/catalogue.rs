//! Two-level offline catalogue shape. Structural validity does not publish a direction list.
use super::{FrozenSample, Invalid, Origin};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectionGroup {
    pub group_ref: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DirectionDraft {
    pub direction_ref: Uuid,
    pub group_ref: Uuid,
    pub name: String,
    pub definition: String,
    pub example_work_refs: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogueDraft {
    pub origin: Origin,
    pub version_ref: Uuid,
    pub groups: Vec<DirectionGroup>,
    pub directions: Vec<DirectionDraft>,
    /// Explicit sample membership; selection/provenance freezing is a separate C0 step.
    pub sample_work_refs: Vec<Uuid>,
}

impl CatalogueDraft {
    pub fn validate_sample(&self, sample: &FrozenSample) -> Result<(), Invalid> {
        self.validate()?;
        let declared: BTreeSet<_> = self.sample_work_refs.iter().copied().collect();
        let frozen: BTreeSet<_> = sample.entries.iter().map(|entry| entry.work_ref).collect();
        let directions: BTreeSet<_> = self.directions.iter().map(|d| d.direction_ref).collect();
        if self.version_ref != sample.direction_version_ref
            || declared != frozen
            || directions != sample.direction_refs.iter().copied().collect()
        {
            return Err(Invalid("offline_catalogue_sample_mismatch"));
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), Invalid> {
        if self.version_ref.is_nil() || !(20..=40).contains(&self.directions.len()) {
            return Err(Invalid("offline_catalogue_size_or_version_invalid"));
        }
        let groups: BTreeSet<_> = self.groups.iter().map(|g| g.group_ref).collect();
        let works: BTreeSet<_> = self.sample_work_refs.iter().copied().collect();
        let directions: BTreeSet<_> = self.directions.iter().map(|d| d.direction_ref).collect();
        if groups.is_empty()
            || groups.len() != self.groups.len()
            || works.len() != self.sample_work_refs.len()
            || directions.len() != self.directions.len()
            || groups
                .iter()
                .chain(works.iter())
                .chain(directions.iter())
                .any(Uuid::is_nil)
            || self.groups.iter().any(|g| g.name.trim().is_empty())
        {
            return Err(Invalid("offline_catalogue_identity_invalid"));
        }
        for direction in &self.directions {
            let examples: BTreeSet<_> = direction.example_work_refs.iter().copied().collect();
            if !groups.contains(&direction.group_ref)
                || direction.name.trim().is_empty()
                || direction.definition.trim().is_empty()
                || !(2..=3).contains(&examples.len())
                || examples.len() != direction.example_work_refs.len()
                || !examples.is_subset(&works)
            {
                return Err(Invalid("offline_direction_definition_or_examples_invalid"));
            }
        }
        Ok(())
    }
}
