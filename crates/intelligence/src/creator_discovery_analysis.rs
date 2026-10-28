//! Validation of evidence-bearing automatic labels. No numeric or identity write authority.
use linggan_evidence::creator_discovery::Fragment;
use serde_json::{Value, json};
use std::collections::HashSet;

fn supported(value: &Value, allowed: &HashSet<&str>, enums: &[&str]) -> Option<Value> {
    let v = value["value"].as_str()?;
    if !enums.contains(&v) {
        return None;
    }
    let reason = value["reason"].as_str()?;
    if reason.chars().count() > 300 {
        return None;
    }
    let ids = value["evidenceFragmentIds"].as_array()?;
    if ids.len() > 12
        || ids
            .iter()
            .any(|x| x.as_str().is_none_or(|s| !allowed.contains(s)))
    {
        return None;
    }
    if v != "unknown" && ids.is_empty() {
        return None;
    }
    Some(json!({"value":v,"reason":reason,"evidenceFragmentIds":ids}))
}
pub fn validate_work(output: &Value, fragments: &[Fragment]) -> Value {
    let allowed: HashSet<_> = fragments.iter().map(|f| f.fragment_id.as_str()).collect();
    let unknown = json!({"value":"unknown","reason":"结果缺少有效依据","evidenceFragmentIds":[]});
    let relevance = supported(
        &output["relevance"],
        &allowed,
        &["related", "unrelated", "unknown"],
    )
    .unwrap_or_else(|| unknown.clone());
    let mut traits = json!({});
    for key in [
        "personal_experience",
        "professional_output",
        "explicit_promotion",
    ] {
        let author_sources: HashSet<_> = fragments
            .iter()
            .filter(|f| f.speaker == "author")
            .map(|f| f.fragment_id.as_str())
            .collect();
        let sources = if key == "personal_experience" {
            &author_sources
        } else {
            &allowed
        };
        traits[key] = supported(&output["traits"][key], sources, &["yes", "no", "unknown"])
            .unwrap_or_else(|| unknown.clone());
    }
    let hints: Vec<Value> = output["topicHints"]
        .as_array()
        .into_iter()
        .flatten()
        .take(3)
        .filter_map(|hint| {
            let text = hint["text"].as_str()?;
            if text.is_empty() || text.chars().count() > 40 {
                return None;
            }
            let ids = hint["evidenceFragmentIds"].as_array()?;
            if ids.is_empty()
                || ids.len() > 12
                || ids
                    .iter()
                    .any(|x| x.as_str().is_none_or(|s| !allowed.contains(s)))
            {
                return None;
            }
            Some(json!({"text":text,"evidenceFragmentIds":ids}))
        })
        .collect();
    json!({"relevance":relevance,"traits":traits,"topicHints":hints})
}
/// related fragment IDs and broad-profile sample IDs are derived by the caller from current works.
pub fn validate_focus(
    output: &Value,
    fragments: &[Fragment],
    related: &std::collections::HashMap<String, uuid::Uuid>,
    broad: &std::collections::HashSet<uuid::Uuid>,
) -> Value {
    let allowed: HashSet<_> = fragments.iter().map(|f| f.fragment_id.as_str()).collect();
    let unknown = json!({"value":"unknown","reason":"当前材料尚不足判断账号整体方向","evidenceFragmentIds":[]});
    let mut candidate = supported(
        &output["focus"],
        &allowed,
        &["vertical_tendency", "multi_topic", "unknown"],
    )
    .unwrap_or_else(|| unknown.clone());
    let ids: HashSet<_> = candidate["evidenceFragmentIds"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    let cited_related: HashSet<_> = ids.iter().filter_map(|id| related.get(*id)).collect();
    let cited_works: HashSet<_> = fragments
        .iter()
        .filter(|f| f.field != "biography" && ids.contains(f.fragment_id.as_str()))
        .filter_map(|f| {
            f.fragment_id
                .split('.')
                .next()
                .and_then(|x| uuid::Uuid::parse_str(x).ok())
        })
        .collect();
    let biography = fragments
        .iter()
        .any(|f| f.field == "biography" && ids.contains(f.fragment_id.as_str()));
    let wide = cited_works.iter().filter(|w| broad.contains(w)).count() >= 2;
    if (candidate["value"] == "vertical_tendency"
        && (cited_related.len() < 2 || (!biography && !wide)))
        || (candidate["value"] == "multi_topic" && !wide)
    {
        candidate = unknown.clone();
    }
    let bio_ids: HashSet<_> = fragments
        .iter()
        .filter(|f| f.field == "biography")
        .map(|f| f.fragment_id.as_str())
        .collect();
    let institution = supported(
        &output["institution_or_brand"],
        &bio_ids,
        &["yes", "no", "unknown"],
    )
    .unwrap_or_else(|| unknown.clone());
    let evidence:Vec<_>=fragments.iter().filter(|f|candidate.to_string().contains(&f.fragment_id)||institution.to_string().contains(&f.fragment_id)).map(|f|json!({"fragmentId":f.fragment_id,"sourceRef":f.source_ref,"field":f.field,"text":f.text.chars().take(200).collect::<String>()})).collect();
    json!({"focus":candidate,"institution_or_brand":institution,"evidence":evidence,"sampleBasis":if broad.len()>=2{"profile_discovery_sample"}else{"limited_domain_sample"},"sampleLimitations":["仅使用当前领域已取得材料，不代表账号全部作品"]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_field_does_not_discard_other_valid_fields() {
        let f = Fragment {
            fragment_id: "w.body".into(),
            source_ref: uuid::Uuid::nil(),
            field: "body".into(),
            source_version: "v1".into(),
            start: 0,
            end: 2,
            text: "实践".into(),
            ..Fragment::default()
        };
        let r = validate_work(
            &json!({"relevance":{"value":"related","reason":"讨论领域","evidenceFragmentIds":["w.body"]},"traits":{"personal_experience":{"value":"yes","reason":"伪造","evidenceFragmentIds":["other"]}},"topicHints":[{"text":"启动","evidenceFragmentIds":["w.body"]}]}),
            &[f],
        );
        assert_eq!(r["relevance"]["value"], "related");
        assert_eq!(r["traits"]["personal_experience"]["value"], "unknown");
        assert_eq!(r["topicHints"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn keyword_sample_never_becomes_vertical() {
        let r = validate_focus(
            &json!({"focus":{"value":"vertical_tendency","reason":"相关比例100%","evidenceFragmentIds":[]}}),
            &[],
            &std::collections::HashMap::new(),
            &HashSet::new(),
        );
        assert_eq!(r["focus"]["value"], "unknown");
    }

    #[test]
    fn focus_needs_two_independent_related_works_and_context() {
        let one = uuid::Uuid::from_u128(1);
        let two = uuid::Uuid::from_u128(2);
        let make = |work: uuid::Uuid, field: &str| Fragment {
            fragment_id: format!("{work}.{field}.evidence"),
            source_ref: work,
            field: field.into(),
            source_version: "v1".into(),
            start: 0,
            end: 2,
            text: "样本".into(),
            ..Fragment::default()
        };
        let fragments = vec![make(one, "body"), make(two, "body"), make(one, "biography")];
        let a = fragments[0].fragment_id.clone();
        let b = fragments[1].fragment_id.clone();
        let bio = fragments[2].fragment_id.clone();
        let related = std::collections::HashMap::from([(a.clone(), one), (b.clone(), two)]);
        let output = |ids: Vec<String>| json!({"focus":{"value":"vertical_tendency","reason":"合成依据","evidenceFragmentIds":ids},"institution_or_brand":{"value":"yes","reason":"合成依据","evidenceFragmentIds":[a]}});
        let duplicate = validate_focus(
            &output(vec![a.clone(), bio.clone()]),
            &fragments,
            &related,
            &HashSet::new(),
        );
        assert_eq!(duplicate["focus"]["value"], "unknown");
        assert_eq!(duplicate["institution_or_brand"]["value"], "unknown");
        let no_context = validate_focus(
            &output(vec![a.clone(), b.clone()]),
            &fragments,
            &related,
            &HashSet::new(),
        );
        assert_eq!(no_context["focus"]["value"], "unknown");
        let valid = validate_focus(
            &output(vec![a.clone(), b, bio]),
            &fragments,
            &related,
            &HashSet::new(),
        );
        assert_eq!(valid["focus"]["value"], "vertical_tendency");
        let institution = validate_focus(
            &json!({"focus":{"value":"unknown","reason":"材料不足","evidenceFragmentIds":[]},"institution_or_brand":{"value":"yes","reason":"公开简介自述","evidenceFragmentIds":[fragments[2].fragment_id]}}),
            &fragments,
            &related,
            &HashSet::new(),
        );
        assert_eq!(institution["institution_or_brand"]["value"], "yes");
    }
    #[test]
    fn quoted_and_unconfirmed_material_cannot_certify_author_personal_experience() {
        let output = json!({"traits":{"personal_experience":{"value":"yes","reason":"原声","evidenceFragmentIds":["block"]}}});
        for speaker in ["other", "unconfirmed"] {
            let f = Fragment {
                fragment_id: "block".into(),
                field: "ocr".into(),
                text: "我家孩子的经历".into(),
                speaker: speaker.into(),
                ..Fragment::default()
            };
            assert_eq!(
                validate_work(&output, &[f])["traits"]["personal_experience"]["value"],
                "unknown"
            );
        }
    }
}
