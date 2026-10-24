//! Cloud OCR observation storage and canonical candidate blocks. A successful
//! recognition never selects itself as the current material reading.
use crate::ocr_adoption::{ImageIdentity, ImageRole};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub const PROCESSOR_VERSION: &str = "cloud-manual-v3";
pub const RULE_VERSION: &str = "cloud-ocr-observation.v3";
pub fn is_processor_version(version: &str) -> bool {
    ["cloud-manual-v2", PROCESSOR_VERSION].iter().any(|prefix| {
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
        let mut blocks = if original.trim().is_empty() {
            vec![
                json!({"ref":reference,"kind":"gap","speaker":"unconfirmed","text":"此处文字无法辨认"}),
            ]
        } else {
            crate::research_dossier::content::blocks_from_text(original, "unconfirmed")
        };
        for (index, block) in blocks.iter_mut().enumerate() {
            block["ref"] = json!(format!("{reference}.{index}"));
            if ["main_title", "chapter_heading"].contains(&role) && block["kind"] == "paragraph" {
                block["kind"] = json!("heading");
            }
            if let Some(b) = coordinates {
                block["bbox"] = json!(b);
            }
            if block["kind"] == "gap"
                || legibility != "clear"
                || [
                    "other",
                    "uncertain",
                    "ui",
                    "background",
                    "subtitle",
                    "brand",
                ]
                .contains(&role)
            {
                let role_message = match role {
                    "ui" => "界面文字，归属待核验",
                    "background" => "背景文字，归属待核验",
                    "subtitle" => "字幕，归属待核验",
                    "brand" => "品牌文字，归属待核验",
                    _ => "原文可读性或表达归属需要核验",
                };
                let message = match legibility {
                    "partial" => format!("{role_message}；部分字符无法确认"),
                    "unreadable" => format!("{role_message}；字符无法可靠辨认"),
                    _ => role_message.to_owned(),
                };
                block["issues"] = json!([{"key":format!("{reference}.{index}:reading"),"code":"uncertain_reading","message":message,"resolution":"reading_edit","blockRef":block["ref"]}]);
            }
        }
        output.extend(blocks);
    }
    if output.is_empty() {
        output.push(json!({"ref":"empty","kind":"gap","speaker":"unconfirmed","text":"识别未返回文字，尚未确认图片无文字","issues":[{"key":"recognition_empty","code":"recognition_empty","message":"未返回文字不证明原图没有文字","resolution":"reading_edit","blockRef":"empty"}]}));
    }
    crate::research_dossier::content::validate_blocks(&output).map_err(|e| invalid(&e))?;
    Ok(output)
}

pub async fn persist_in(
    tx: &mut Transaction<'_, Postgres>,
    source: &CloudSource,
    derivative: Uuid,
    layout: Uuid,
    raw: &str,
    visual: &Value,
) -> Result<Vec<Value>, sqlx::Error> {
    if source.image.width == 0 || source.image.height == 0 {
        return Err(invalid("cloud_source_invalid"));
    }
    let blocks = blocks_from_visual(visual)?;
    let observation = json!({"producer":"cloud_vision","source":source,"rawSha256":crate::ocr_adoption::content_hash(raw.as_bytes()),"result":visual});
    let fingerprint = crate::ocr_adoption::content_hash(observation.to_string().as_bytes());
    // Original producer diagnostics stay immutable and never serve as a second
    // material projection. The common reading table owns all user/agent content.
    sqlx::query("INSERT INTO linggan_media_ocr_adoption(adoption_ref,derivative_ref,layout_ref,rule_version,input_fingerprint,observation,assessment,line_ref_map) VALUES($1,$2,$3,$4,$5,$6,$7,'[]')")
        .bind(Uuid::new_v4()).bind(derivative).bind(layout).bind(RULE_VERSION).bind(fingerprint).bind(observation)
        .bind(json!({"producer":"cloud_vision","sourceRelation":"image_transcription","ruleVersion":RULE_VERSION})).execute(&mut **tx).await?;
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unreadable_region_retains_returned_characters_as_unverified_text() {
        let v = json!({"regions":[{"regionRef":"r","text":"禁止 … 10 mg","role":"body","legibility":"unreadable","bbox":null}]});
        let blocks = blocks_from_visual(&v).unwrap();
        assert_eq!(blocks[0]["text"], "禁止 … 10 mg");
        assert_eq!(blocks[0]["kind"], "paragraph");
        assert_eq!(blocks[0]["issues"][0]["resolution"], "reading_edit");
        assert!(is_processor_version("cloud-manual-v2"));
        assert!(is_processor_version(&format!(
            "{PROCESSOR_VERSION}:{}",
            Uuid::nil()
        )));
        assert!(!is_processor_version("cloud-manual-v1"));
    }
    #[test]
    fn role_messages_always_preserve_nonclear_legibility() {
        for role in ["ui", "background", "subtitle", "brand"] {
            for (legibility, warning) in [
                ("partial", "部分字符无法确认"),
                ("unreadable", "字符无法可靠辨认"),
            ] {
                let blocks = blocks_from_visual(&json!({"regions":[{"regionRef":"r","text":"禁止 … 10 mg","role":role,"legibility":legibility,"bbox":null}]})).unwrap();
                assert_eq!(blocks[0]["text"], "禁止 … 10 mg");
                let message = blocks[0]["issues"][0]["message"].as_str().unwrap();
                assert!(message.contains("归属待核验"));
                assert!(message.contains(warning), "{role}/{legibility}: {message}");
            }
        }
    }
    #[test]
    fn preserves_readable_unknown_role_and_converts_only_observed_box() {
        let v = json!({"regions":[{"regionRef":"r","text":"不可以超过 10 mg\n\n第二段","role":"background","legibility":"clear","bbox":[0.1,0.2,0.9,0.8]}]});
        let blocks = blocks_from_visual(&v).unwrap();
        assert!(blocks.iter().any(|b| b["text"] == "不可以超过 10 mg"));
        assert_eq!(
            blocks[0]["bbox"],
            json!([0.1, 0.2, 0.8, 0.6000000000000001])
        );
        assert_eq!(blocks[0]["issues"][0]["resolution"], "reading_edit");
    }
}
