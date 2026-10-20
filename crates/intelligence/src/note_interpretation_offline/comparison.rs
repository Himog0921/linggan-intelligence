use super::{CatalogueDraft, Invalid};
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Keep,
    Merge,
    Delete,
    New,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StructureChange {
    pub kind: ChangeKind,
    pub old_refs: Vec<Uuid>,
    pub new_refs: Vec<Uuid>,
}

pub fn validate_comparison(
    old: &CatalogueDraft,
    new: &CatalogueDraft,
    changes: &[StructureChange],
) -> Result<(), Invalid> {
    old.validate()?;
    new.validate()?;
    let mut old_seen = BTreeSet::new();
    let mut new_seen = BTreeSet::new();
    for change in changes {
        let valid = match change.kind {
            ChangeKind::Keep => change.old_refs.len() == 1 && change.new_refs.len() == 1,
            ChangeKind::Merge => change.old_refs.len() >= 2 && change.new_refs.len() == 1,
            ChangeKind::Delete => change.old_refs.len() == 1 && change.new_refs.is_empty(),
            ChangeKind::New => change.old_refs.is_empty() && change.new_refs.len() == 1,
        };
        if !valid
            || change.old_refs.iter().any(|r| !old_seen.insert(*r))
            || change.new_refs.iter().any(|r| !new_seen.insert(*r))
        {
            return Err(Invalid("offline_structure_change_invalid"));
        }
    }
    if old_seen != old.directions.iter().map(|d| d.direction_ref).collect()
        || new_seen != new.directions.iter().map(|d| d.direction_ref).collect()
    {
        return Err(Invalid("offline_structure_change_incomplete"));
    }
    Ok(())
}
