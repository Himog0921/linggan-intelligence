//! Select useful reading content without rewriting immutable recognition blocks.
use super::super::content;
use serde_json::Value;

pub(super) fn automatic_cover(material: &Value) -> bool {
    material["kind"] == "image"
        && material["purpose"] == "cover"
        && material["reading"]["generatedBy"] == "ordinary_ocr"
        && material["reading"]["adoption"] == "automatic"
}

pub(super) fn substantive_body(material: &Value) -> bool {
    material["kind"] == "body"
        && material["sourceState"] != "restricted"
        && material["blocks"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|block| {
                content::has_source_text(block)
                    && content::pure_tags(&content::block_text(block)).is_none()
            })
}
