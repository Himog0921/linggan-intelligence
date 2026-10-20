//! Session-only classification choices. A new machine version never rewrites the original choice.
use super::{Identity, Input, Invalid, Origin, Output, Relevance, mock_unknown, validate_output};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManualJudgment {
    pub origin: Origin,
    pub identity: Identity,
    pub relevance: Relevance,
    pub primary_direction: Option<Uuid>,
    pub also_related: Option<Uuid>,
    pub reason: Option<String>,
}

fn apply(machine: &Output, judgment: &ManualJudgment) -> Output {
    Output {
        relevance: judgment.relevance,
        primary_direction: judgment.primary_direction,
        also_related: judgment.also_related,
        reason: judgment.reason.clone(),
        ..machine.clone()
    }
}

pub fn validate_judgment(original: &Input, judgment: &ManualJudgment) -> Result<(), Invalid> {
    if judgment.identity != original.identity() {
        return Err(Invalid("offline_judgment_identity_mismatch"));
    }
    validate_output(original, &apply(&mock_unknown(original)?, judgment))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedOutput {
    pub machine: Output,
    pub effective: Output,
    pub judgment: Option<ManualJudgment>,
    pub manual_applied: bool,
    pub needs_review: bool,
}

/// Original validation is explicit: an arbitrary old version cannot masquerade as a saved choice.
pub fn project_judgment(
    current: &Input,
    machine: &Output,
    original: Option<(&Input, &ManualJudgment)>,
) -> Result<ReviewedOutput, Invalid> {
    validate_output(current, machine)?;
    let mut reviewed = ReviewedOutput {
        machine: machine.clone(),
        effective: machine.clone(),
        judgment: None,
        manual_applied: false,
        needs_review: false,
    };
    if let Some((old_input, judgment)) = original {
        validate_judgment(old_input, judgment)?;
        let old = &judgment.identity;
        let new = current.identity();
        if old.domain_ref != new.domain_ref
            || old.work_ref != new.work_ref
            || old.snapshot_identity != new.snapshot_identity
        {
            return Err(Invalid("offline_judgment_source_mismatch"));
        }
        reviewed.judgment = Some(judgment.clone());
        reviewed.needs_review = [judgment.primary_direction, judgment.also_related]
            .into_iter()
            .flatten()
            .any(|reference| !current.direction_refs.contains(&reference));
        if !reviewed.needs_review {
            reviewed.effective = apply(machine, judgment);
            validate_output(current, &reviewed.effective)?;
            reviewed.manual_applied = true;
        }
    }
    Ok(reviewed)
}
