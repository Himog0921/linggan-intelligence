//! Exact creator request preparation shared by preview and dispatch. No I/O or model calls.
use crate::{creator_discovery_analysis::SYSTEM, model_settings::ModelError};
use linggan_evidence::{
    creator_discovery::{DiscoveryWork, Fragment},
    research_dossier,
};
use serde_json::{Value, json};

pub(crate) const ENVELOPE_BYTES: usize = 128;

pub fn prepare(
    domain: &Value,
    work: &DiscoveryWork,
    input_budget: usize,
) -> Result<(Value, Vec<Fragment>), ModelError> {
    let mut fragments = work.fragments.clone();
    loop {
        let manifest: Vec<_> = fragments
            .iter()
            .map(
                |f| json!({"fragmentId":f.fragment_id,"field":f.field,"start":f.start,"end":f.end}),
            )
            .collect();
        let input = json!({"domain":domain,"workRef":work.work_ref,"sourceFingerprint":work.fingerprint,
            "materialContract":research_dossier::VERSION,"materials":research_dossier::render_materials(&fragments),
            "fragments":manifest,"acquisitionContext":{"kind":work.acquisition_kind,"selectionBiased":true}});
        if input.to_string().len() + SYSTEM.len() + ENVELOPE_BYTES <= input_budget {
            return Ok((input, fragments));
        }
        let Some(longest) = fragments.iter_mut().max_by_key(|f| f.text.chars().count()) else {
            return Err(ModelError::InputLimit);
        };
        let length = longest.text.chars().count();
        if length <= 1 {
            return Err(ModelError::InputLimit);
        }
        longest.text = longest.text.chars().take(length / 2).collect();
        longest.end = longest.start + longest.text.chars().count();
    }
}
