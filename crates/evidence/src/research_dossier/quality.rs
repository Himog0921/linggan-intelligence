//! Rebuildable, source-bound cross-image diagnostics; never resolves or rewrites a value.
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn specific(issue: &Value) -> bool {
    if issue["code"] == "cloud_image_unchecked" {
        return false;
    }
    !matches!(
        issue["message"].as_str(),
        Some(
            "界面文字，归属待核验"
                | "背景文字，归属待核验"
                | "字幕，归属待核验"
                | "品牌文字，归属待核验"
                | "原文可读性或表达归属需要核验"
        )
    )
}

pub(super) fn rebuild(dossier: &mut super::ResearchDossier) {
    type Key = (String, String, String);
    let mut parameters: BTreeMap<Key, Vec<(usize, String, String)>> = BTreeMap::new();
    for (index, material) in dossier.materials.iter_mut().enumerate() {
        if let Some(issues) = material.get_mut("issues").and_then(Value::as_array_mut) {
            issues.retain(|i| i["code"] != "cross_image_parameter_conflict");
        }
        if material["kind"] != "image" || material["sourceState"] == "restricted" {
            continue;
        }
        for block in material["blocks"].as_array().into_iter().flatten() {
            for p in block["parameters"].as_array().into_iter().flatten() {
                let Some(subject) = p["subject"].as_str() else {
                    continue;
                };
                let Some(attribute) = p["attribute"].as_str() else {
                    continue;
                };
                let Some(basis) = p["basis"].as_str() else {
                    continue;
                };
                let Some(value) = p["value"].as_str() else {
                    continue;
                };
                // A block-only selection must include both the quoted value and its named object.
                if !material["blocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|b| {
                        b["sourceRole"] != "ui"
                            && compact(&super::content::block_text(b)).contains(&compact(subject))
                    })
                {
                    continue;
                }
                parameters
                    .entry((compact(subject), attribute.into(), basis.into()))
                    .or_default()
                    .push((
                        index,
                        value.into(),
                        block["ref"].as_str().unwrap_or("").into(),
                    ));
            }
        }
    }
    for ((subject, attribute, basis), entries) in parameters {
        let units: BTreeSet<_> = entries.iter().map(|e| e.0).collect();
        let values: BTreeSet<_> = entries
            .iter()
            .map(|e| comparable(&attribute, &e.1))
            .collect();
        if units.len() < 2 || values.len() < 2 {
            continue;
        }
        let label = match attribute.as_str() {
            "rating" => "推荐星级",
            "age" => "年龄",
            "players" => "人数",
            "duration" => "时长",
            _ => continue,
        };
        let basis_label = match basis.as_str() {
            "package" => "包装标注",
            "author_experience" => "作者体验",
            _ => "规则",
        };
        // Stable image numbers are local reading positions, explicitly not platform page numbers.
        let observations: Vec<_> = entries
            .iter()
            .map(|(index, value, _)| {
                let number = dossier.materials[..=*index]
                    .iter()
                    .filter(|m| m["kind"] == "image")
                    .count();
                format!("图片资料{number}：{value}")
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let message = format!(
            "{subject} · {label}（{basis_label}）跨图不一致：{}。请核对原图，未自动选值。",
            observations.join("；")
        );
        for (index, _, block_ref) in entries {
            let issue = json!({"key":format!("parameter_conflict:{subject}:{attribute}:{basis}"),"code":"cross_image_parameter_conflict","message":message,"resolution":"reading_edit","blockRef":block_ref});
            if let Some(issues) = dossier.materials[index]["issues"].as_array_mut() {
                if !issues.iter().any(|i| i["key"] == issue["key"]) {
                    issues.push(issue);
                }
            }
        }
    }
    // Selection rebuilds diagnostics from only the selected blocks; no unselected value leaks.
    for fragment in &mut dossier.fragments {
        fragment
            .issues
            .retain(|i| i["code"] != "cross_image_parameter_conflict");
        for material in &dossier.materials {
            for issue in material["issues"].as_array().into_iter().flatten() {
                if issue["code"] == "cross_image_parameter_conflict"
                    && issue["blockRef"] == fragment.fragment_id
                {
                    fragment.issues.push(issue.clone());
                }
            }
        }
    }
}
fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

// Normalize only equivalent written units; never rewrite the cited source value.
fn comparable(attribute: &str, value: &str) -> String {
    let value = compact(value)
        .to_lowercase()
        .replace(['–', '—', '～', '~'], "-");
    match attribute {
        "duration" => value.replace("分钟", "min").replace("mins", "min"),
        "players" => value.replace("players", "人").replace("player", "人"),
        "age" => value.replace("years", "岁").replace("year", "岁"),
        _ => value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn dossier(values: &[(&str, &str, &str)]) -> super::super::ResearchDossier {
        super::super::ResearchDossier {
            contract: super::super::VERSION, work_ref: uuid::Uuid::nil(), revision: String::new(), material_revision: String::new(), as_of: String::new(), note:json!({}),
            materials: values.iter().enumerate().map(|(i,(subject,basis,value))|json!({"ref":format!("i{i}"), "kind":"image", "sourceState":"present", "issues":[], "blocks":[{"ref":format!("b{i}"),"kind":"paragraph","text":format!("{subject} {value}"),"parameters":[{"subject":subject,"attribute":"duration","basis":basis,"value":value}]}]})).collect(),
            reading_view:json!({}),coverage:json!({}),attachments:json!({}),selection:json!({}),fragments:vec![],media:vec![],media_inventory:vec![],candidate_statuses:vec![],metadata:json!({}),
        }
    }
    #[test]
    fn conflicts_preserve_both_sources_and_do_not_compare_unlike_claims() {
        let mut d = dossier(&[
            ("山河之旅", "package", "30min"),
            ("山河之旅", "package", "15分钟"),
            ("山河之旅", "author_experience", "10分钟"),
            ("其他游戏", "package", "20分钟"),
        ]);
        let original = d.materials[0]["blocks"].clone();
        rebuild(&mut d);
        assert_eq!(d.materials[0]["blocks"], original);
        assert!(
            d.materials[0]["issues"][0]["message"]
                .as_str()
                .unwrap()
                .contains("图片资料2：15分钟")
        );
        assert_eq!(d.materials[2]["issues"], json!([]));
        assert_eq!(d.materials[3]["issues"], json!([]));
        rebuild(&mut d);
        assert_eq!(d.materials[0]["issues"].as_array().unwrap().len(), 1);
        d.materials.remove(1);
        rebuild(&mut d);
        assert_eq!(d.materials[0]["issues"], json!([]));
    }
    #[test]
    fn selected_parameter_without_its_object_name_does_not_leak_a_cross_block_claim() {
        let mut d = dossier(&[("游戏", "package", "30min"), ("游戏", "package", "15分钟")]);
        d.materials[0]["blocks"][0]["text"] = json!("30min");
        rebuild(&mut d);
        assert!(d.materials.iter().all(|m| m["issues"] == json!([])));
    }
    #[test]
    fn equivalent_units_and_restricted_sources_do_not_create_conflicts() {
        let mut d = dossier(&[
            ("游戏", "package", "30min"),
            ("游戏", "package", "30 分钟"),
            ("游戏", "package", "15分钟"),
        ]);
        d.materials[2]["sourceState"] = json!("restricted");
        rebuild(&mut d);
        assert!(d.materials.iter().all(|m| m["issues"] == json!([])));
    }
}
