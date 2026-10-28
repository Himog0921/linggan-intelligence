use super::CONTRACT;
use linggan_evidence::target_inspection::DossierView;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Citation {
    pub work_ref: Uuid,
    pub citation_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Claim {
    pub kind: String,
    pub statement: String,
    pub target_citations: Vec<Citation>,
    pub reference_citations: Vec<Citation>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Analysis {
    pub relation: String,
    pub claims: Vec<Claim>,
    pub limitations: Vec<String>,
    pub partial: bool,
    pub method_version: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawClaim {
    kind: String,
    statement: String,
    target_citations: Vec<String>,
    reference_citations: Vec<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawAnalysis {
    relation: String,
    claims: Vec<RawClaim>,
    limitations: Vec<String>,
}
const RELATIONS: [&str; 5] = [
    "mostly_repeated",
    "supplemented",
    "changed_angle",
    "different_problem",
    "undetermined",
];
const KINDS: [&str; 8] = [
    "shared_point",
    "added_step",
    "added_example",
    "added_condition",
    "added_counterexample",
    "changed_scene",
    "expression_only",
    "different_problem",
];

pub fn response_schema() -> Value {
    json!({
        "type":"object","additionalProperties":false,"required":["relation","claims","limitations"],
        "properties":{
            "relation":{"type":"string","enum":RELATIONS},
            "claims":{"type":"array","maxItems":24,"items":{"type":"object","additionalProperties":false,
                "required":["kind","statement","targetCitations","referenceCitations"],
                "properties":{"kind":{"type":"string","enum":KINDS},"statement":{"type":"string","minLength":1,"maxLength":1600},
                    "targetCitations":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"string"}},
                    "referenceCitations":{"type":"array","minItems":1,"maxItems":6,"items":{"type":"string"}}}}},
            "limitations":{"type":"array","maxItems":12,"items":{"type":"string","maxLength":1600}}
        }
    })
}

pub fn prompt(materials: &[DossierView], instruction: &str) -> String {
    let documents = materials
        .iter()
        .enumerate()
        .map(|(i, d)| {
            json!({
                "work":format!("W{}",i+1),"partial":d.partial,"coverageLimitations":d.limitations,
                "fragments":d.fragments.iter().enumerate().map(|(n,f)|json!({
                    "id":format!("W{}-S{}",i+1,n+1),"field":f.field,"content":f.structure,"speaker":f.speaker,"issues":f.issues,"sourceRef":f.source_ref,"sourceVersion":f.source_version,"readingRef":f.reading_ref,"unitRef":f.unit_ref,"asOf":f.knowledge_cutoff,"sourceLocation":f.source_location
                })).collect::<Vec<_>>()
            })
        })
        .collect::<Vec<_>>();
    json!({"task":instruction,"documents":documents}).to_string()
}

pub fn validate_analysis(text: &str, materials: &[DossierView]) -> Result<Analysis, &'static str> {
    let raw: RawAnalysis = serde_json::from_str(text).map_err(|_| "comparison_schema_invalid")?;
    if !RELATIONS.contains(&raw.relation.as_str())
        || raw.claims.len() > 24
        || raw.limitations.len() > 12
        || raw.limitations.iter().any(|s| s.chars().count() > 1600)
    {
        return Err("comparison_schema_invalid");
    }
    let mut citations = BTreeMap::new();
    for (i, material) in materials.iter().enumerate() {
        for (n, fragment) in material.fragments.iter().enumerate() {
            citations.insert(
                format!("W{}-S{}", i + 1, n + 1),
                (
                    i,
                    Citation {
                        work_ref: material.work_ref,
                        citation_id: fragment.citation_id.clone(),
                    },
                ),
            );
        }
    }
    let mut discarded = 0;
    let mut claims = Vec::new();
    for claim in raw.claims {
        if !KINDS.contains(&claim.kind.as_str())
            || claim.statement.trim().is_empty()
            || claim.statement.chars().count() > 1600
        {
            discarded += 1;
            continue;
        }
        let resolve = |ids: Vec<String>, target: bool| -> Option<Vec<Citation>> {
            if ids.is_empty() || ids.len() > 6 {
                return None;
            }
            ids.into_iter()
                .map(|id| {
                    citations
                        .get(&id)
                        .filter(|(i, _)| (*i == 0) == target)
                        .map(|(_, c)| c.clone())
                })
                .collect()
        };
        let Some(target) = resolve(claim.target_citations, true) else {
            discarded += 1;
            continue;
        };
        let Some(reference) = resolve(claim.reference_citations, false) else {
            discarded += 1;
            continue;
        };
        claims.push(Claim {
            kind: claim.kind,
            statement: claim.statement,
            target_citations: target,
            reference_citations: reference,
        });
    }
    let mut limitations = raw.limitations;
    if discarded > 0 {
        limitations.push(format!(
            "有 {discarded} 项返回内容缺少可解析的双侧引用，未发布这些断言。"
        ));
    }
    let partial = discarded > 0 || materials.iter().any(|d| d.partial);
    if partial {
        limitations
            .push("判断仅覆盖本次已取得且可读取的来源材料，不代表整篇或全部作者历史。".into());
    }
    let relation = if partial || claims.is_empty() {
        "undetermined".into()
    } else {
        raw.relation
    };
    Ok(Analysis {
        relation,
        claims,
        limitations,
        partial,
        method_version: CONTRACT.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use linggan_evidence::target_inspection::SourceFragment;
    fn materials(partial: bool) -> Vec<DossierView> {
        (1..=2)
            .map(|i| DossierView {
                work_ref: Uuid::from_u128(i),
                knowledge_cutoff: "2026-10-23T00:00:00Z".into(),
                material_revision: format!("revision-{i}"),
                dossier_revision: format!("dossier-{i}"),
                owner_contract: "research-dossier.v2".into(),
                fragments: vec![SourceFragment {
                    citation_id: format!("citation-{i}"),
                    source_ref: Uuid::from_u128(i + 2),
                    source_version: "source-v1".into(),
                    field: "body".into(),
                    start: 0,
                    end: 4,
                    text: "合成正文".into(),
                    structure: json!({"ref":format!("citation-{i}"),"kind":"paragraph","text":"合成正文","speaker":"author"}),
                    speaker: "author".into(),
                    ..Default::default()
                }],
                partial,
                limitations: vec![],
            })
            .collect()
    }
    fn response() -> Value {
        json!({"relation":"mostly_repeated","claims":[{"kind":"shared_point","statement":"两侧均有相同步骤。",
        "targetCitations":["W1-S1"],"referenceCitations":["W2-S1"]}],"limitations":[]})
    }
    #[test]
    fn complete_comparison_resolves_both_sides_to_exact_source_citations() {
        let result = validate_analysis(&response().to_string(), &materials(false)).unwrap();
        assert_eq!(result.relation, "mostly_repeated");
        assert!(!result.partial);
        assert_eq!(
            result.claims[0].target_citations[0].citation_id,
            "citation-1"
        );
        assert_eq!(
            result.claims[0].reference_citations[0].work_ref,
            Uuid::from_u128(2)
        );
    }
    #[test]
    fn missing_images_force_undetermined_even_if_provider_claims_repetition() {
        let result = validate_analysis(&response().to_string(), &materials(true)).unwrap();
        assert!(result.partial);
        assert_eq!(result.relation, "undetermined");
        assert_eq!(
            result.claims.len(),
            1,
            "qualified excerpts remain useful within the limited scope"
        );
    }
    #[test]
    fn unknown_or_wrong_side_citations_cannot_publish_a_claim() {
        for invalid in ["W1-S1", "W2-S9"] {
            let mut raw = response();
            raw["claims"][0]["referenceCitations"] = json!([invalid]);
            let result = validate_analysis(&raw.to_string(), &materials(false)).unwrap();
            assert!(result.claims.is_empty());
            assert!(result.partial);
            assert_eq!(result.relation, "undetermined");
        }
    }
    #[test]
    fn v2_prompt_preserves_structure_roles_positions_and_full_text_once() {
        let mut input = materials(false);
        let source = &mut input[0].fragments[0];
        source.structure = json!({"ref":source.citation_id,"kind":"table","speaker":"other","columns":["condition","action"],"rows":[["5mg","do not increase TABLE-ONCE"]],"bbox":[0.1,0.2,0.4,0.3],"time":{"startMs":100,"endMs":200}});
        source.text = "5mg do not increase TABLE-ONCE".into();
        source.speaker = "other".into();
        source.reading_ref = Some(Uuid::from_u128(30));
        source.unit_ref = Some(Uuid::from_u128(31));
        source.knowledge_cutoff = "2026-10-23T00:00:00Z".into();
        let body = prompt(&input, "synthetic comparison instruction");
        let packet: Value = serde_json::from_str(&body).unwrap();
        let block = &packet["documents"][0]["fragments"][0];
        assert_eq!(body.matches("TABLE-ONCE").count(), 1);
        assert!(block.get("text").is_none());
        assert_eq!(block["content"]["time"], json!({"startMs":100,"endMs":200}));
        assert_eq!(block["content"]["bbox"], json!([0.1, 0.2, 0.4, 0.3]));
        assert_eq!(block["speaker"], "other");
        assert_eq!(block["readingRef"], Uuid::from_u128(30).to_string());
        let original = super::super::fingerprint(&input);
        input[0].knowledge_cutoff = "2026-10-23T01:00:00Z".into();
        input[0].fragments[0].knowledge_cutoff = input[0].knowledge_cutoff.clone();
        assert_eq!(
            super::super::fingerprint(&input),
            original,
            "unchanged material stays reusable across cutoffs; the immutable request binds its cutoff"
        );
        input[0].fragments[0].speaker = "author".into();
        assert_ne!(super::super::fingerprint(&input), original);
    }
}
