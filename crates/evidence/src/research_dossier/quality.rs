//! Rebuildable, source-bound cross-image diagnostics; never resolves or rewrites a value.
use serde_json::{Value, json};

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

fn parameter_attribute_label(value: Option<&str>) -> &'static str {
    match value {
        Some("rating") => "推荐指数",
        Some("age") => "年龄",
        Some("players") => "人数",
        Some("duration") => "时长",
        _ => "未知",
    }
}
fn parameter_basis_label(value: Option<&str>) -> &'static str {
    match value {
        Some("package") => "包装标注",
        Some("author_experience") => "作者体验",
        Some("rule") => "规则",
        _ => "未知",
    }
}

/// A provider's subject/basis strings are candidates, not evidence of object binding.
/// Rebuild from the selected blocks, including historical candidates, without name joins.
pub(super) fn rebuild(dossier: &mut super::ResearchDossier) {
    for material in &mut dossier.materials {
        if let Some(issues) = material.get_mut("issues").and_then(Value::as_array_mut) {
            issues.retain(|i| {
                !matches!(
                    i["code"].as_str(),
                    Some(
                        "cross_image_parameter_conflict"
                            | "parameter_binding_unconfirmed"
                            | "parameter_candidate_unanchored"
                    )
                )
            });
        }
        if material["kind"] != "image" || material["sourceState"] == "restricted" {
            continue;
        }
        if let Some(blocks) = material["blocks"].as_array_mut() {
            for block in blocks {
                if !block
                    .get("parameters")
                    .is_some_and(|p| p.as_array().is_some_and(|p| !p.is_empty()))
                {
                    continue;
                }
                let region = block["regionRef"].clone();
                let reference = block["ref"].clone();
                let count = if let Some(parameters) =
                    block.get_mut("parameters").and_then(Value::as_array_mut)
                {
                    for parameter in parameters.iter_mut() {
                        if region.is_string() {
                            parameter["sourceRegionRef"] = region.clone();
                            parameter["bindingStatus"] = json!("unconfirmed");
                            parameter["basisStatus"] = json!("unconfirmed");
                        }
                    }
                    parameters.len()
                } else {
                    0
                };
                let text: String = super::content::block_text(block)
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect();
                let candidate_issues: Vec<Value> = block["parameters"].as_array().into_iter().flatten().enumerate().filter_map(|(index, p)| {
                    let value = p["value"].as_str()?;
                    let compact: String = value.chars().filter(|c| !c.is_whitespace()).collect();
                    (!text.contains(&compact)).then(|| json!({"key":format!("parameter_candidate_{index}"),"code":"parameter_candidate_unanchored",
                        "message":format!("未绑定参数候选：{}；模型标注对象：{}，字段：{}，依据：{}；对象及依据未确认。",value,p["subject"].as_str().unwrap_or("未知"),parameter_attribute_label(p["attribute"].as_str()),parameter_basis_label(p["basis"].as_str())),
                        "resolution":"reading_edit","blockRef":reference}))
                }).collect();
                if block.get("issues").is_none() {
                    block["issues"] = json!([]);
                }
                if let Some(issues) = block["issues"].as_array_mut() {
                    issues.retain(|i| {
                        !matches!(
                            i["code"].as_str(),
                            Some(
                                "cross_image_parameter_conflict"
                                    | "parameter_binding_unconfirmed"
                                    | "parameter_candidate_unanchored"
                            )
                        )
                    });
                    issues.extend(candidate_issues);
                    if count > 0 {
                        issues.push(json!({"key":"parameter_binding_unconfirmed","code":"parameter_binding_unconfirmed","message":"参数候选的对象与依据尚未确认，未据此合并跨图商品参数。","resolution":"reading_edit","blockRef":reference}));
                    }
                }
            }
        }
    }
    // Selection must not preserve old, wider-scope parameter relations in AI sources.
    for fragment in &mut dossier.fragments {
        if let Some(block) = dossier
            .materials
            .iter()
            .flat_map(|m| m["blocks"].as_array().into_iter().flatten())
            .find(|b| b["ref"] == fragment.fragment_id)
        {
            if block.get("parameters").is_none() {
                continue;
            }
            fragment.structure = block.clone();
            fragment.issues.retain(|i| {
                !matches!(
                    i["code"].as_str(),
                    Some(
                        "cross_image_parameter_conflict"
                            | "parameter_binding_unconfirmed"
                            | "parameter_candidate_unanchored"
                    )
                )
            });
            fragment.issues.extend(
                block["issues"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|i| {
                        matches!(
                            i["code"].as_str(),
                            Some(
                                "parameter_binding_unconfirmed" | "parameter_candidate_unanchored"
                            )
                        )
                    })
                    .cloned(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_names_do_not_bind_parameters_or_drop_candidates() {
        let mut d = super::super::ResearchDossier {
            contract: super::super::VERSION, work_ref: uuid::Uuid::nil(), revision: String::new(), material_revision:String::new(),as_of:String::new(),note:json!({}),
            materials:["30min","15分钟"].iter().enumerate().map(|(i,value)| json!({"kind":"image","sourceState":"present","issues":[{"code":"cross_image_parameter_conflict"}],"blocks":[{"ref":format!("b{i}"),"kind":"paragraph","regionRef":format!("r{i}"),"text":format!("游戏 {value}"),"parameters":[{"subject":"游戏","attribute":"duration","value":value,"basis":"package"}]}]})).collect(),
            reading_view:json!({}),coverage:json!({}),attachments:json!({}),selection:json!({}),fragments:vec![],media:vec![],media_inventory:vec![],candidate_statuses:vec![],metadata:json!({}),
        };
        rebuild(&mut d);
        assert!(d.materials.iter().all(|m| m["issues"] == json!([])));
        assert_eq!(
            d.materials[0]["blocks"][0]["parameters"][0]["value"],
            "30min"
        );
        assert_eq!(
            d.materials[0]["blocks"][0]["parameters"][0]["bindingStatus"],
            "unconfirmed"
        );
        rebuild(&mut d);
        assert_eq!(
            d.materials[0]["blocks"][0]["issues"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
