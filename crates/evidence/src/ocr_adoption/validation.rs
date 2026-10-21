use super::*;
use std::collections::HashSet;

pub(super) fn validate(input: &ImageInput) -> Result<(), InputError> {
    let image = &input.image;
    if input.schema_version != 1
        || image.width == 0
        || image.height == 0
        || image.width > 100_000
        || image.height > 100_000
        || input.layout.blocks.len() > 512
        || input.anchors.lines.len() > 4096
        || input.layout.terminations.len() > 512
        || input
            .layout
            .blocks
            .iter()
            .map(|b| b.text.len())
            .sum::<usize>()
            > 2_000_000
        || input
            .anchors
            .lines
            .iter()
            .map(|l| l.text.len())
            .sum::<usize>()
            > 2_000_000
    {
        return Err(InputError::Bounds);
    }
    if image != &input.layout.image
        || image != &input.anchors.image
        || image.work_ref.is_nil()
        || !reference(&image.image_ref)
        || input.sequence_ordinal == Some(0)
        || (input.image_role == ImageRole::FirstImage && input.sequence_ordinal != Some(1))
        || (input.image_role == ImageRole::Interior
            && input.sequence_ordinal.is_some_and(|p| p < 2))
    {
        return Err(InputError::Identity);
    }
    if !hash(&image.blob_sha256)
        || [&input.layout.pipeline, &input.anchors.pipeline]
            .iter()
            .any(|p| !hash(&p.raw_sha256) || !reference(&p.version))
    {
        return Err(InputError::Source);
    }
    let mut refs = HashSet::new();
    let mut orders = HashSet::new();
    for block in &input.layout.blocks {
        if !reference(&block.block_ref)
            || !reference(&block.label)
            || !refs.insert(block.block_ref.as_str())
            || !orders.insert(block.reading_order)
        {
            return Err(InputError::Reference);
        }
        if !bbox(block.bbox, image) {
            return Err(InputError::Geometry);
        }
    }
    let mut line_refs = HashSet::new();
    orders.clear();
    for line in &input.anchors.lines {
        if !reference(&line.line_ref)
            || !line_refs.insert(line.line_ref.as_str())
            || !orders.insert(line.reading_order)
            || line.text.trim().is_empty()
        {
            return Err(InputError::Reference);
        }
        if !bbox(line.bbox, image)
            || !line.confidence.is_finite()
            || !(0.0..=1.0).contains(&line.confidence)
        {
            return Err(InputError::Geometry);
        }
    }
    let mut receipts = HashSet::new();
    for receipt in &input.layout.terminations {
        let block = input
            .layout
            .blocks
            .iter()
            .find(|b| b.block_ref == receipt.block_ref);
        if !receipts.insert(&receipt.block_ref)
            || block.is_none_or(|b| content_hash(b.text.as_bytes()) != receipt.output_sha256)
            || receipt
                .finish_reason
                .as_ref()
                .is_some_and(|s| !reference(s))
        {
            return Err(InputError::Receipt);
        }
    }
    Ok(())
}

fn bbox(b: [f64; 4], image: &ImageIdentity) -> bool {
    b.iter().all(|v| v.is_finite() && *v >= 0.0)
        && b[0] < b[2]
        && b[1] < b[3]
        && b[2] <= f64::from(image.width)
        && b[3] <= f64::from(image.height)
}
fn reference(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 512 && !s.chars().any(char::is_control)
}
fn hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}
