//! Freeze an explicitly selected synthetic set; no sampling or reads from a live corpus.
use super::{Input, Invalid, Origin, prepare_input};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SampleEntry {
    pub work_ref: Uuid,
    pub snapshot_identity: String,
    pub dossier_revision: String,
    pub material_fingerprint: String,
    pub provenance_hash: String,
    /// SHA-256 of the canonical InputPreview JSON, including states and literal fragments.
    pub input_sha256: String,
}

/// A reproducibility receipt, never proof of a stored snapshot or real source eligibility.
/// Construct through freeze_sample; there is no untrusted JSON import or serving write.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FrozenSample {
    pub contract: &'static str,
    pub origin: Origin,
    pub selection_rule: &'static str,
    pub fixture_revision: String,
    pub domain_ref: Uuid,
    pub as_of: String,
    pub direction_version_ref: Uuid,
    pub direction_refs: Vec<Uuid>,
    pub skill_version_ref: Uuid,
    /// UUID order is canonical; caller order has no semantic effect on this explicit set.
    pub entries: Vec<SampleEntry>,
    pub manifest_sha256: String,
}

fn json_hash(value: &impl Serialize) -> Result<String, Invalid> {
    let bytes = serde_json::to_vec(value).map_err(|_| Invalid("offline_sample_encoding_failed"))?;
    Ok(Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn freeze_sample(fixture_revision: &str, inputs: &[Input]) -> Result<FrozenSample, Invalid> {
    let first = inputs.first().ok_or(Invalid("offline_sample_empty"))?;
    if fixture_revision.trim().is_empty() || first.material.as_of.trim().is_empty() {
        return Err(Invalid("offline_sample_revision_or_time_missing"));
    }
    let mut refs = BTreeSet::new();
    let mut entries = Vec::with_capacity(inputs.len());
    let membership: BTreeSet<_> = first.direction_refs.iter().copied().collect();
    for input in inputs {
        let preview = prepare_input(input)?;
        if input.material.domain_ref != first.material.domain_ref
            || input.material.as_of != first.material.as_of
            || input.direction_version_ref != first.direction_version_ref
            || input.skill_version_ref != first.skill_version_ref
            || input
                .direction_refs
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                != membership
            || !refs.insert(input.material.work_ref)
        {
            return Err(Invalid("offline_sample_scope_or_member_conflict"));
        }
        entries.push(SampleEntry {
            work_ref: input.material.work_ref,
            snapshot_identity: input.material.snapshot_identity.clone(),
            dossier_revision: input.material.dossier_revision.clone(),
            material_fingerprint: input.material.material_fingerprint.clone(),
            provenance_hash: input.material.provenance_hash.clone(),
            input_sha256: json_hash(&preview)?,
        });
    }
    entries.sort_by_key(|entry| entry.work_ref);
    let mut frozen = FrozenSample {
        contract: "note-interpretation-offline-sample.v1",
        origin: Origin::OfflineMock,
        selection_rule: "explicit-synthetic-members-uuid-order.v1",
        fixture_revision: fixture_revision.into(),
        domain_ref: first.material.domain_ref,
        as_of: first.material.as_of.clone(),
        direction_version_ref: first.direction_version_ref,
        direction_refs: membership.into_iter().collect(),
        skill_version_ref: first.skill_version_ref,
        entries,
        manifest_sha256: String::new(),
    };
    // The digest field is empty in the canonical preimage, avoiding a self-referential hash.
    frozen.manifest_sha256 = json_hash(&frozen)?;
    Ok(frozen)
}
