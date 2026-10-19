use super::{Column, FINGERPRINT_VERSION, Fragment, Material, VERSION};
use crate::creator_discovery::hash;
use serde_json::json;
use unicode_normalization::UnicodeNormalization;

fn normalized(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .nfc()
        .collect()
}

pub(super) fn finish(material: &mut Material) {
    material.fragments.sort_by_key(|f| {
        (
            field_order(&f.field),
            f.logical_position.clone(),
            hash(&normalized(&f.text)),
        )
    });
    material.fragments.dedup_by(|a, b| {
        a.field == b.field
            && a.logical_position == b.logical_position
            && normalized(&a.text) == normalized(&b.text)
    });
    for (i, f) in material.fragments.iter_mut().enumerate() {
        f.citation_id = format!("S{}", i + 1);
    }
    for c in &mut material.columns {
        c.reasons.sort();
        c.reasons.dedup();
    }
    material.columns.sort_by_key(|c| field_order(&c.field));
    material.not_ready_reasons.sort();
    material.not_ready_reasons.dedup();
    material.ready = material.not_ready_reasons.is_empty();
    material.material_fingerprint = content_fingerprint(&material.columns, &material.fragments);
    let sources: Vec<_> = material
        .fragments
        .iter()
        .map(|f| {
            json!([
                f.citation_id,
                f.source_ref,
                f.source_version,
                f.start,
                f.end,
                f.text_hash
            ])
        })
        .collect();
    material.provenance_hash = hash(&json!([material.domain_ref, sources]).to_string());
    material.snapshot_identity = hash(
        &json!([
            VERSION,
            material.domain_ref,
            material.work_ref,
            material.as_of,
            material.material_fingerprint,
            material.provenance_hash,
            material.ready,
            material.not_ready_reasons
        ])
        .to_string(),
    );
}

fn content_fingerprint(columns: &[Column], fragments: &[Fragment]) -> String {
    let text: Vec<_> = fragments
        .iter()
        .map(|f| json!([f.field, f.logical_position, hash(&normalized(&f.text))]))
        .collect();
    hash(&json!([FINGERPRINT_VERSION, columns, text]).to_string())
}

pub(super) fn fragment(
    field: &str,
    position: String,
    source: uuid::Uuid,
    version: String,
    start: usize,
    text: String,
) -> Fragment {
    Fragment {
        citation_id: String::new(),
        field: field.into(),
        logical_position: position,
        source_ref: source,
        source_version: version,
        start,
        end: start + text.chars().count(),
        text_hash: hash(&text),
        text,
    }
}

fn field_order(field: &str) -> usize {
    match field {
        "title" => 0,
        "cover_headline" => 1,
        "body" => 2,
        "tags" => 3,
        "ocr" => 4,
        "transcript" => 5,
        _ => 6,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::note_material_input::ColumnState;
    use uuid::Uuid;
    #[test]
    fn normalization_and_sources_have_separate_identities_and_unicode_offsets() {
        let cols = vec![Column {
            field: "body".into(),
            state: ColumnState::Available,
            reasons: vec![],
        }];
        let a = fragment(
            "body",
            "native".into(),
            Uuid::new_v4(),
            "v1".into(),
            0,
            "e\u{301}\r\n中🙂".into(),
        );
        let b = fragment(
            "body",
            "native".into(),
            Uuid::new_v4(),
            "v2".into(),
            0,
            "é\n中🙂".into(),
        );
        assert_eq!(
            content_fingerprint(&cols, std::slice::from_ref(&a)),
            content_fingerprint(&cols, std::slice::from_ref(&b))
        );
        assert_ne!(a.text_hash, b.text_hash);
        assert_eq!(a.end, 6);
        assert_eq!(b.end, 4);
        let mut changed = b;
        changed.text.push('变');
        assert_ne!(
            content_fingerprint(&cols, &[a]),
            content_fingerprint(&cols, &[changed])
        );
        let missing = vec![Column {
            field: "body".into(),
            state: ColumnState::NotAcquired,
            reasons: vec!["unknown".into()],
        }];
        assert_ne!(
            content_fingerprint(&cols, &[]),
            content_fingerprint(&missing, &[])
        );
    }
}
