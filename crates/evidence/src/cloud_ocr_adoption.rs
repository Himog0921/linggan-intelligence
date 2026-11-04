//! Current cloud OCR output conversion into canonical reading blocks.
use crate::ocr_source::{ImageIdentity, ImageRole};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

pub const VISUAL_INFORMATION_MISSING: &str = "未返回视觉描述字段，非文字信息尚未提供";
pub const PROCESSOR_VERSION: &str = "cloud-manual-v6";
pub const RULE_VERSION: &str = "cloud-ocr-observation.v6";
pub fn is_processor_version(version: &str) -> bool {
    [
        PROCESSOR_VERSION,
        "cloud-manual-v5",
        "cloud-manual-v4",
        "cloud-manual-v3",
    ]
    .iter()
    .any(|prefix| {
        version == *prefix
            || version
                .strip_prefix(prefix)
                .and_then(|v| v.strip_prefix(':'))
                .is_some_and(|run| Uuid::parse_str(run).is_ok())
    })
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudSource {
    pub image: ImageIdentity,
    pub image_role: ImageRole,
    pub sequence_ordinal: Option<u32>,
}
fn compact(value: &str) -> String {
    value.chars().filter(|c| !c.is_whitespace()).collect()
}
// Tables are independent provider observations. Do not require a second text copy.
// Only omit text lines that are mechanically represented by the table; retain all
// other lines verbatim, including signs, conditions and footnotes.
fn supplemental_table_text(table: &Value, original: &str) -> String {
    let rows: Vec<Vec<String>> = std::iter::once(&table["columns"])
        .chain(table["rows"].as_array().into_iter().flatten())
        .filter_map(Value::as_array)
        .map(|row| {
            row.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .collect();
    let lines: Vec<_> = original.lines().collect();
    let mut retained = Vec::new();
    let mut offset = 0;
    while offset < lines.len() {
        let matched = rows.iter().find_map(|row| {
            let max_lines = 1 + row
                .iter()
                .map(|cell| cell.matches('\n').count())
                .sum::<usize>();
            (1..=max_lines.min(lines.len() - offset)).find(|length| {
                let candidate = lines[offset..offset + length].join("\n");
                let parts: Vec<_> = candidate.split(['\t', '|', '｜']).map(compact).collect();
                parts.len() == row.len()
                    && parts.iter().zip(row).all(|(part, cell)| {
                        part == &compact(cell)
                            || (*length == 1 && matches!(part.as_str(), "-" | "—"))
                    })
                    && parts
                        .iter()
                        .zip(row)
                        .any(|(part, cell)| !part.is_empty() && part == &compact(cell))
            })
        });
        if let Some(length) = matched {
            offset += length;
        } else {
            retained.push(lines[offset]);
            offset += 1;
        }
    }
    retained.join("\n")
}

fn invalid(message: &str) -> sqlx::Error {
    sqlx::Error::Protocol(message.into())
}

/// The visual provider describes rectangles as x1/y1/x2/y2. The single dossier
/// contract describes x/y/width/height, and this is the only conversion point.
fn bbox(value: &Value) -> Result<Option<[f64; 4]>, sqlx::Error> {
    if value.is_null() {
        return Ok(None);
    }
    let a = value
        .as_array()
        .filter(|v| v.len() == 4)
        .ok_or_else(|| invalid("cloud_bbox_invalid"))?;
    let mut b = [0.; 4];
    for (i, n) in a.iter().enumerate() {
        b[i] = n
            .as_f64()
            .filter(|v| v.is_finite() && (0.0..=1.0).contains(v))
            .ok_or_else(|| invalid("cloud_bbox_invalid"))?;
    }
    if b[2] <= b[0] || b[3] <= b[1] {
        return Err(invalid("cloud_bbox_invalid"));
    }
    Ok(Some([b[0], b[1], b[2] - b[0], b[3] - b[1]]))
}

/// Convert whole-image limitations into the same issues consumed by the dossier and AI.
pub fn issues_from_visual(visual: &Value) -> Result<Vec<Value>, sqlx::Error> {
    let mut output = vec![];
    for (index, message) in string_array(&visual["limitations"])?
        .into_iter()
        .enumerate()
    {
        output.push(
            json!({"key":format!("cloud_image_limitation_{index}"),"code":"image_limitation",
            "message":message,"resolution":"reading_edit"}),
        );
    }
    Ok(output)
}

fn string_array(value: &Value) -> Result<Vec<&str>, sqlx::Error> {
    value
        .as_array()
        .ok_or_else(|| invalid("cloud_uncertainties_invalid"))?
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|s| !s.trim().is_empty())
                .ok_or_else(|| invalid("cloud_uncertainties_invalid"))
        })
        .collect()
}

pub fn blocks_from_visual(visual: &Value) -> Result<Vec<Value>, sqlx::Error> {
    let regions = visual["regions"]
        .as_array()
        .ok_or_else(|| invalid("cloud_regions_invalid"))?;
    let mut output = vec![];
    let mut seen = std::collections::BTreeSet::new();
    for region in regions {
        let reference = region["regionRef"]
            .as_str()
            .filter(|r| !r.is_empty())
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        let original = region["text"]
            .as_str()
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        let description = region["visualDescription"]
            .as_str()
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        let location = region["location"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        let uncertainties = string_array(&region["uncertainties"])?;
        let role = region["role"]
            .as_str()
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        let legibility = region["legibility"]
            .as_str()
            .ok_or_else(|| invalid("cloud_region_invalid"))?;
        if !seen.insert(reference)
            || !["clear", "partial", "unreadable"].contains(&legibility)
            || ![
                "main_title",
                "body",
                "chapter_heading",
                "subtitle",
                "background",
                "ui",
                "brand",
                "call_to_action",
                "other",
                "uncertain",
            ]
            .contains(&role)
        {
            return Err(invalid("cloud_region_invalid"));
        }
        let coordinates = bbox(&region["bbox"])?;
        let mut blocks = if region["table"].is_object() {
            let mut blocks = vec![
                json!({"kind":"table","speaker":"unconfirmed","columns":region["table"]["columns"],"rows":region["table"]["rows"]}),
            ];
            let supplement = supplemental_table_text(&region["table"], original);
            if !supplement.trim().is_empty() {
                blocks.extend(crate::research_dossier::content::blocks_from_text(
                    &supplement,
                    "unconfirmed",
                ));
            }
            blocks
        } else if original.trim().is_empty() {
            if description.trim().is_empty() {
                vec![
                    json!({"kind":"gap","speaker":"unconfirmed","text":"此区域没有可用的识别内容"}),
                ]
            } else {
                vec![]
            }
        } else {
            crate::research_dossier::content::blocks_from_text(original, "unconfirmed")
        };
        for block in &mut blocks {
            block["contentType"] = json!("extracted_text");
            if ["main_title", "chapter_heading"].contains(&role) && block["kind"] == "paragraph" {
                block["kind"] = json!("heading");
            }
        }
        if !description.trim().is_empty() {
            // Descriptions are always ordinary, unconfirmed paragraphs, even when they
            // contain a Markdown marker. They must never become image titles or quotes.
            blocks.push(
                json!({"kind":"paragraph","speaker":"unconfirmed","text":description,
                "contentType":"visual_description"}),
            );
        }
        let parameter_targets: Vec<_> = region["parameters"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|p| {
                blocks
                    .iter()
                    .position(|b| {
                        b["contentType"] == "extracted_text"
                            && p["value"].as_str().is_some_and(|value| {
                                compact(&crate::research_dossier::content::block_text(b))
                                    .contains(&compact(value))
                            })
                    })
                    .unwrap_or(0)
            })
            .collect();
        for (index, block) in blocks.iter_mut().enumerate() {
            let block_ref = format!("{reference}.{index}");
            block["ref"] = json!(block_ref);
            block["regionRef"] = json!(reference);
            block["regionLabel"] = json!(location);
            block["sourceRole"] = json!(role);
            let parameters: Vec<_> = region["parameters"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(parameter_index, _)| parameter_targets[*parameter_index] == index)
                .map(|(_, parameter)| {
                    let mut parameter = parameter.clone();
                    parameter["sourceRegionRef"] = json!(reference);
                    parameter["bindingStatus"] = json!("unconfirmed");
                    parameter["basisStatus"] = json!("unconfirmed");
                    parameter
                })
                .collect();
            if !parameters.is_empty() {
                block["parameters"] = json!(parameters);
            }
            if let Some(b) = coordinates {
                block["bbox"] = json!(b);
            }
            let mut issues = vec![];
            if block["contentType"] == "extracted_text"
                && (block["kind"] == "gap" || legibility != "clear")
            {
                let message = if legibility == "unreadable" {
                    "字符无法可靠辨认"
                } else {
                    "部分字符无法确认"
                };
                issues.push(
                    json!({"key":format!("{block_ref}:reading"),"code":"uncertain_reading",
                    "message":message,"resolution":"reading_edit","blockRef":block_ref}),
                );
            }
            for (i, message) in uncertainties.iter().enumerate().filter(|_| index == 0) {
                issues.push(
                    json!({"key":format!("{block_ref}:uncertainty:{i}"),"code":if *message==VISUAL_INFORMATION_MISSING{"visual_information_missing"}else{"region_uncertainty"},
                    "message":message,"resolution":"reading_edit","blockRef":block_ref}),
                );
            }
            if !issues.is_empty() {
                block["issues"] = json!(issues);
            }
        }
        output.extend(blocks);
    }
    if output.is_empty() {
        output.push(json!({"ref":"empty","kind":"gap","speaker":"unconfirmed","text":"识图未返回可用内容，需对照原图",
            "issues":[{"key":"recognition_empty","code":"recognition_empty","message":"未返回内容不证明原图没有信息",
                "resolution":"reading_edit","blockRef":"empty"}]}));
    }
    crate::research_dossier::content::validate_blocks(&output).map_err(|e| invalid(&e))?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clear_image_has_no_generic_review_issue() {
        assert!(
            issues_from_visual(&json!({"limitations":[]}))
                .unwrap()
                .is_empty()
        );
        let issues = issues_from_visual(&json!({"limitations":["右栏末行被裁切"]})).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0]["code"], "image_limitation");
    }
    #[test]
    fn unreadable_region_retains_returned_characters_as_unverified_text() {
        let v = json!({"regions":[{"regionRef":"r","location":"中部","visualDescription":"","uncertainties":[],"text":"禁止 … 10 mg","role":"body","legibility":"unreadable","bbox":null}]});
        let blocks = blocks_from_visual(&v).unwrap();
        assert_eq!(blocks[0]["text"], "禁止 … 10 mg");
        assert_eq!(blocks[0]["kind"], "paragraph");
        assert_eq!(blocks[0]["issues"][0]["code"], "uncertain_reading");
        assert!(!is_processor_version("cloud-manual-v2"));
        assert!(is_processor_version(&format!(
            "{PROCESSOR_VERSION}:{}",
            Uuid::nil()
        )));
        assert!(!is_processor_version("cloud-manual-v1"));
    }
    #[test]
    fn table_never_discards_unrepresented_conditions_and_parameters_stay_on_their_block() {
        assert_eq!(
            supplemental_table_text(&json!({"columns":["温度"],"rows":[["5℃"]]}), "温度\n-5℃"),
            "-5℃"
        );
        let mut region = json!({"regionRef":"r","location":"中部","visualDescription":"","uncertainties":[],"text":"阶段 时间\n过渡 5分钟","role":"body","legibility":"clear","bbox":null,"table":{"columns":["阶段","时间"],"rows":[["过渡","5分钟"]]},"parameters":[]});
        assert_eq!(
            blocks_from_visual(&json!({"regions":[region.clone()]})).unwrap()[0]["kind"],
            "table"
        );
        region["text"] = json!("阶段 时间\n过渡 5分钟\n仅在孩子愿意时使用");
        let blocks = blocks_from_visual(&json!({"regions":[region]})).unwrap();
        assert!(blocks.iter().any(|b| {
            crate::research_dossier::content::block_text(b).contains("仅在孩子愿意时使用")
        }));
        let blocks=blocks_from_visual(&json!({"regions":[{"regionRef":"r","location":"中部","text":"山河之旅\n\n30分钟","visualDescription":"","uncertainties":[],"role":"body","legibility":"clear","bbox":null,"parameters":[{"subject":"山河之旅","attribute":"duration","value":"30分钟","basis":"package"}]}]})).unwrap();
        assert!(blocks[0].get("parameters").is_none());
        assert_eq!(blocks[1]["parameters"][0]["value"], "30分钟");
    }
    #[test]
    fn table_dedup_never_erases_cross_row_or_column_conflicts() {
        let table = json!({"columns":["对象","结果"],"rows":[["甲","是"],["乙","否"]]});
        assert_eq!(
            supplemental_table_text(&table, "甲\t否\n是\t甲\n甲\t是\n乙\t否"),
            "甲\t否\n是\t甲"
        );
    }
    #[test]
    fn missing_visual_message_becomes_a_machine_readable_gap() {
        let blocks = blocks_from_visual(&json!({"regions":[{"regionRef":"r","location":"顶部","text":"保留文字","visualDescription":"","uncertainties":[VISUAL_INFORMATION_MISSING],"role":"body","legibility":"clear","bbox":null}]})).unwrap();
        assert_eq!(blocks[0]["issues"][0]["code"], "visual_information_missing");
    }
    #[test]
    fn table_dedup_preserves_sentences_and_removes_exact_multiline_cell_copies() {
        let table =
            json!({"columns":["对象", "说明"],"rows":[["甲", "第一行\n第二行"],["乙", "否"]]});
        assert_eq!(
            supplemental_table_text(&table, "对象\t说明\n甲\t第一行\n第二行\n甲否乙"),
            "甲否乙"
        );
    }
    #[test]
    fn independent_table_survives_placeholder_text_and_keeps_unbound_parameters() {
        let visual = json!({"regions":[{"regionRef":"r","location":"中部表格","visualDescription":"","uncertainties":[],"text":"年龄\t行为\n2-5岁\t-","role":"body","legibility":"clear","bbox":null,"table":{"columns":["年龄","行为"],"rows":[["2-5岁","抽噎，停一下又继续哭"]]},"parameters":[{"subject":"未证明对象","attribute":"age","value":"8+","basis":"package"}]}]});
        let blocks = blocks_from_visual(&visual).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["rows"][0][1], "抽噎，停一下又继续哭");
        assert_eq!(blocks[0]["parameters"][0]["bindingStatus"], "unconfirmed");
        assert_eq!(blocks[0]["parameters"][0]["value"], "8+");
    }
    #[test]
    fn role_messages_always_preserve_nonclear_legibility() {
        for role in ["ui", "background", "subtitle", "brand"] {
            for (legibility, warning) in [
                ("partial", "部分字符无法确认"),
                ("unreadable", "字符无法可靠辨认"),
            ] {
                let blocks = blocks_from_visual(&json!({"regions":[{"regionRef":"r","location":"中部","visualDescription":"","uncertainties":[],"text":"禁止 … 10 mg","role":role,"legibility":legibility,"bbox":null}]})).unwrap();
                assert_eq!(blocks[0]["text"], "禁止 … 10 mg");
                let message = blocks[0]["issues"][0]["message"].as_str().unwrap();
                assert!(!message.contains("归属待核验"));
                assert!(message.contains(warning), "{role}/{legibility}: {message}");
            }
        }
    }
    #[test]
    fn preserves_readable_unknown_role_and_converts_only_observed_box() {
        let v = json!({"regions":[{"regionRef":"r","location":"中部","visualDescription":"","uncertainties":[],"text":"不可以超过 10 mg\n\n第二段","role":"background","legibility":"clear","bbox":[0.1,0.2,0.9,0.8]}]});
        let blocks = blocks_from_visual(&v).unwrap();
        assert!(blocks.iter().any(|b| b["text"] == "不可以超过 10 mg"));
        assert_eq!(
            blocks[0]["bbox"],
            json!([0.1, 0.2, 0.8, 0.6000000000000001])
        );
        assert!(blocks[0].get("issues").is_none());
    }
    #[test]
    fn keeps_text_description_and_region_context_distinct() {
        let visual = json!({"regions":[{"regionRef":"card1","location":"左上卡片","text":"不适合3岁以下",
            "visualDescription":"两个棋子位于棋盘上","uncertainties":["人数标签模糊"],"role":"body",
            "legibility":"clear","bbox":[0.1,0.1,0.5,0.5]}],"limitations":["右侧被裁切"]});
        let blocks = blocks_from_visual(&visual).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0]["contentType"], "extracted_text");
        assert_eq!(blocks[0]["text"], "不适合3岁以下");
        assert_eq!(blocks[1]["contentType"], "visual_description");
        assert_eq!(blocks[1]["speaker"], "unconfirmed");
        assert_eq!(blocks[0]["regionRef"], blocks[1]["regionRef"]);
        assert_eq!(blocks[1]["regionLabel"], "左上卡片");
        assert_eq!(blocks[0]["issues"][0]["message"], "人数标签模糊");
        assert!(blocks[1].get("issues").is_none());
        assert_eq!(
            issues_from_visual(&visual).unwrap()[0]["message"],
            "右侧被裁切"
        );
    }
    #[test]
    fn scene_without_text_has_description_without_false_unreadable_gap() {
        let blocks=blocks_from_visual(&json!({"regions":[{"regionRef":"scene","location":"全图","text":"",
            "visualDescription":"# 白色碗位于桌面上","uncertainties":[],"role":"other","legibility":"clear","bbox":null}]})).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0]["kind"], "paragraph");
        assert_eq!(blocks[0]["contentType"], "visual_description");
        assert_eq!(blocks[0]["speaker"], "unconfirmed");
        assert!(!blocks[0]["text"].as_str().unwrap().contains("无法辨认"));
    }
}
