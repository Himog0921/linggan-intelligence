use super::ResearchDossier;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    #[serde(default)]
    pub kinds: Vec<String>,
    #[serde(default)]
    pub unit_refs: Vec<String>,
    #[serde(default)]
    pub block_refs: Vec<String>,
    #[serde(default)]
    pub image_pages: Vec<u64>,
}

pub fn select(dossier: &mut ResearchDossier, selection: &Selection) -> Result<(), String> {
    for kind in &selection.kinds {
        if !matches!(
            kind.as_str(),
            "body" | "tags" | "image" | "audio" | "video_frame"
        ) {
            return Err("invalid_material_kind".into());
        }
    }
    if selection.image_pages.contains(&0) {
        return Err("invalid_image_page".into());
    }
    let units: BTreeSet<_> = dossier
        .materials
        .iter()
        .filter_map(|m| m["ref"].as_str())
        .collect();
    let blocks: BTreeSet<_> = dossier
        .materials
        .iter()
        .flat_map(|m| m["blocks"].as_array().into_iter().flatten())
        .filter_map(|b| b["ref"].as_str())
        .collect();
    let pages: BTreeSet<_> = dossier
        .materials
        .iter()
        .filter(|m| m["kind"] == "image")
        .filter_map(|m| m["page"].as_u64())
        .collect();
    if selection
        .unit_refs
        .iter()
        .any(|r| !units.contains(r.as_str()))
        || selection
            .block_refs
            .iter()
            .any(|r| !blocks.contains(r.as_str()))
        || selection.image_pages.iter().any(|r| !pages.contains(r))
    {
        return Err("selection_reference_not_found".into());
    }
    let explicit = !(selection.kinds.is_empty()
        && selection.unit_refs.is_empty()
        && selection.block_refs.is_empty()
        && selection.image_pages.is_empty());
    if !explicit {
        return Ok(());
    }
    let complete = dossier.revision.clone();
    let original = dossier.materials.len();
    dossier.materials.retain_mut(|unit| {
        let selected = selection.kinds.iter().any(|k| unit["kind"] == *k)
            || selection.unit_refs.iter().any(|r| unit["ref"] == *r)
            || (unit["kind"] == "image"
                && unit["page"]
                    .as_u64()
                    .is_some_and(|p| selection.image_pages.contains(&p)));
        if selected {
            return true;
        }
        if let Some(blocks) = unit
            .get_mut("blocks")
            .and_then(serde_json::Value::as_array_mut)
        {
            blocks.retain(|b| selection.block_refs.iter().any(|r| b["ref"] == *r));
            return !blocks.is_empty();
        }
        false
    });
    let included: BTreeSet<_> = dossier
        .materials
        .iter()
        .flat_map(|m| m["blocks"].as_array().into_iter().flatten())
        .filter_map(|b| b["ref"].as_str())
        .collect();
    dossier
        .fragments
        .retain(|f| f.field == "title" || included.contains(f.fragment_id.as_str()));
    dossier.selection = json!({"mode":"explicit","kinds":selection.kinds,"unitRefs":selection.unit_refs,"blockRefs":selection.block_refs,
        "imagePages":selection.image_pages,"omittedUnits":original-dossier.materials.len(),"completeRevision":complete});
    super::reading::rebuild(dossier);
    super::refresh_revision(dossier);
    Ok(())
}

/// A range in the fixed reading order. Whole blocks and their exact sources stay intact.
pub fn segment(
    dossier: &mut ResearchDossier,
    offset: usize,
    size: usize,
) -> Result<serde_json::Value, String> {
    if !(1..=200).contains(&size) {
        return Err("invalid_segment_size".into());
    }
    let refs = super::ordered_refs(dossier);
    let total = refs.len();
    if offset > total {
        return Err("segment_offset_out_of_range".into());
    }
    let end = offset.saturating_add(size).min(total);
    let selected: Vec<_> = refs[offset..end].iter().map(|r| (*r).to_owned()).collect();
    let complete = dossier.revision.clone();
    let scope = if dossier.selection["mode"] == "explicit" {
        "selected"
    } else {
        "full"
    };
    if selected.is_empty() {
        let omitted = dossier.materials.len();
        dossier.materials.clear();
        dossier.fragments.retain(|f| f.field == "title");
        dossier.selection = json!({"mode":"explicit","kinds":[],"unitRefs":[],"blockRefs":[],"imagePages":[],"omittedUnits":omitted,"completeRevision":complete});
        super::reading::rebuild(dossier);
        super::refresh_revision(dossier);
    } else {
        select(
            dossier,
            &Selection {
                block_refs: selected,
                ..Default::default()
            },
        )?;
    }
    Ok(
        json!({"completeRevision":complete,"asOf":dossier.as_of,"offset":offset,"end":end,"totalBlocks":total,
        "nextOffset":if end<total{Some(end)}else{None},"scope":scope}),
    )
}
