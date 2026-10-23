use super::{Column, FINGERPRINT_VERSION, Fragment, Material, VERSION};
use crate::creator_discovery::hash;
use serde_json::json;
pub(super) fn finish(material: &mut Material) {
    for (i, f) in material.fragments.iter_mut().enumerate() {
        f.citation_id = format!("S{}", i + 1);
    }
    for c in &mut material.columns {
        c.reasons.sort();
        c.reasons.dedup();
    }
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
                f.text_hash, f.logical_position, f.reading_ref, f.unit_ref
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
        .map(|f| json!([f.field, f.logical_position, hash(&f.text), f.speaker, hash(&f.structure.to_string()), f.issues]))
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
        speaker: "unconfirmed".into(), structure: serde_json::Value::Null, issues: vec![],
        reading_ref: None, unit_ref: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dossier_input::ColumnState;
    use uuid::Uuid;
    #[test]
    fn exact_text_structure_and_roles_have_distinct_identities_with_unicode_offsets() {
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
        assert_ne!(
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
        let base = fragment("ocr", "page-1.block-1".into(), Uuid::new_v4(), "v1".into(), 0, "相同字面文字".into());
        let mut role = base.clone(); role.speaker = "author".into();
        let mut structure = base.clone(); structure.structure = json!({"kind":"quote","text":base.text});
        let mut issue = base.clone(); issue.issues = vec![json!({"code":"unclear","message":"归属未确认"})];
        for changed in [role, structure, issue] {
            assert_ne!(content_fingerprint(&cols, std::slice::from_ref(&base)), content_fingerprint(&cols, &[changed]));
        }
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
