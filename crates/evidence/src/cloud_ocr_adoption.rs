//! A cloud producer on the existing OCR result/adoption chain. No local line anchors
//! are fabricated. Machine preview and clean research eligibility remain distinct.
use crate::ocr_adoption::{Assessment, ImageIdentity, ImageInput, ImageRole, Reason};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub const PROCESSOR_VERSION: &str = "cloud-manual-v1";
pub const RULE_VERSION: &str = "cloud-ocr-adoption.v1";
pub fn is_processor_version(version: &str) -> bool {
    version == PROCESSOR_VERSION
        || version
            .strip_prefix("cloud-manual-v1:")
            .is_some_and(|run| Uuid::parse_str(run).is_ok())
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
fn normalize(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}
fn cover(source: &CloudSource) -> bool {
    matches!(
        source.image_role,
        ImageRole::FirstImage | ImageRole::ExplicitCover
    )
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, sqlx::Error> {
    value[field]
        .as_str()
        .ok_or_else(|| invalid("cloud_ocr_field_invalid"))
}
fn span(start: usize, end: usize, status: &str, reason: Option<&str>) -> Value {
    json!({"start":start,"end":end,"status":status,"reason":reason})
}
fn preview(regions: &[Value], skip_title: bool) -> String {
    regions
        .iter()
        .filter(|r| !skip_title || r["role"] != "main_title")
        .filter_map(|r| {
            let text = r["text"].as_str()?;
            let points: Vec<_> = text.chars().collect();
            let fragments: Vec<_> = r["spans"]
                .as_array()?
                .iter()
                .filter_map(|s| {
                    if s["status"] == "excluded" {
                        return None;
                    }
                    if s["status"] == "pending" {
                        return Some("〔待核验〕".to_owned());
                    }
                    Some(
                        points[s["start"].as_u64()? as usize..s["end"].as_u64()? as usize]
                            .iter()
                            .collect::<String>(),
                    )
                })
                .collect();
            let result = fragments.join("");
            (!result.is_empty()).then_some(result)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
fn bbox(value: &Value) -> Result<Option<[f64; 4]>, sqlx::Error> {
    if value.is_null() {
        return Ok(None);
    }
    let values = value
        .as_array()
        .filter(|a| a.len() == 4)
        .ok_or_else(|| invalid("cloud_ocr_coordinates_invalid"))?;
    let mut b = [0.; 4];
    for (index, value) in values.iter().enumerate() {
        b[index] = value
            .as_f64()
            .filter(|n| n.is_finite() && (0.0..=1.0).contains(n))
            .ok_or_else(|| invalid("cloud_ocr_coordinates_invalid"))?;
    }
    if b[0] >= b[2] || b[1] >= b[3] {
        return Err(invalid("cloud_ocr_coordinates_invalid"));
    }
    Ok(Some(b))
}
fn overlap(a: [f64; 4], b: [f64; 4]) -> f64 {
    ((a[2].min(b[2]) - a[0].max(b[0])).max(0.) * (a[3].min(b[3]) - a[1].max(b[1])).max(0.))
        / ((a[2] - a[0]) * (a[3] - a[1]))
}
/// Unique near-literal matches identify disagreements only. They do not certify
/// the line OCR as correct, replace characters, or estimate character geometry.
fn line_disagreements<'a>(
    text: &str,
    lines: impl Iterator<Item = (&'a str, &'a str)>,
) -> Option<Vec<Value>> {
    let points: Vec<_> = text.chars().collect();
    let compact: Vec<_> = points
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_whitespace())
        .map(|(n, c)| (n, *c))
        .collect();
    if compact.len() > 4096 {
        return None;
    }
    let mut doubtful = std::collections::BTreeMap::new();
    for (reference, original) in lines.take(512) {
        let line: Vec<_> = original.chars().filter(|c| !c.is_whitespace()).collect();
        if !(6..=256).contains(&line.len()) || line.len() > compact.len() {
            continue;
        }
        if compact
            .windows(line.len())
            .any(|w| w.iter().map(|(_, c)| *c).eq(line.iter().copied()))
        {
            continue;
        }
        let starts: Vec<_> = compact
            .windows(3)
            .enumerate()
            .filter(|(_, w)| w.iter().map(|(_, c)| *c).eq(line[..3].iter().copied()))
            .map(|(i, _)| i)
            .take(33)
            .collect();
        if starts.len() > 32 {
            continue;
        }
        let mut matches = vec![];
        for start in starts {
            if start + line.len() > compact.len() {
                continue;
            }
            let differences: Vec<_> = compact[start..start + line.len()]
                .iter()
                .zip(&line)
                .enumerate()
                .filter(|(_, ((_, a), b))| a != *b)
                .map(|(i, _)| i)
                .take(2)
                .collect();
            if differences.len() == 1 {
                matches.push(compact[start + differences[0]].0);
            }
        }
        if matches.len() == 1 {
            doubtful.insert(matches[0], (reference, original));
        }
    }
    if doubtful.is_empty() {
        return None;
    }
    let mut output = vec![];
    let mut cursor = 0;
    for (offset, (reference, original)) in doubtful {
        if offset > cursor {
            output.push(span(cursor, offset, "adopted", None));
        }
        let mut current = span(
            offset,
            offset + 1,
            "pending",
            Some("与已有同图行 OCR 存在字词分歧，均保留原文"),
        );
        current["comparisonRef"] = json!(reference);
        current["comparisonText"] = json!(original);
        output.push(current);
        cursor = offset + 1;
    }
    if cursor < points.len() {
        output.push(span(cursor, points.len(), "adopted", None));
    }
    Some(output)
}
/// A same-source disagreement is uncertainty, never an instruction to replace text.
fn local_spans(
    text: &str,
    b: Option<[f64; 4]>,
    local: Option<(&ImageInput, &Assessment)>,
) -> Option<Vec<Value>> {
    let (input, assessment) = local?;
    if let Some(spans) = line_disagreements(
        text,
        input
            .anchors
            .lines
            .iter()
            .map(|line| (line.line_ref.as_str(), line.text.as_str())),
    ) {
        return Some(spans);
    }
    let b = b?;
    let block = input
        .layout
        .blocks
        .iter()
        .filter(|l| {
            let other = [
                l.bbox[0] / f64::from(input.image.width),
                l.bbox[1] / f64::from(input.image.height),
                l.bbox[2] / f64::from(input.image.width),
                l.bbox[3] / f64::from(input.image.height),
            ];
            overlap(b, other) >= 0.6 && overlap(other, b) >= 0.6
        })
        .max_by_key(|l| usize::from(l.text == text))?;
    let n = text.chars().count();
    if n == 0 {
        return None;
    }
    let risk = assessment
        .regions
        .iter()
        .find(|r| r.block_ref == block.block_ref)
        .is_some_and(|r| {
            r.reasons.iter().any(|reason| {
                matches!(
                    reason,
                    Reason::DecorationLike
                        | Reason::UnverifiedSymbol
                        | Reason::MathematicalLayoutUnverified
                )
            })
        });
    if risk {
        return Some(vec![span(
            0,
            n,
            "pending",
            Some("已有同区块记录提示图形或符号歧义"),
        )]);
    }
    if normalize(&block.text) == normalize(text) {
        return None;
    }
    let a: Vec<_> = text.chars().collect();
    let other: Vec<_> = block.text.chars().collect();
    let prefix = a.iter().zip(&other).take_while(|(a, b)| a == b).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(other[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    if (prefix + suffix) * 10 < a.len().max(other.len()) * 7 {
        return None;
    }
    let end = n - suffix;
    if end == prefix {
        return Some(vec![span(
            0,
            n,
            "pending",
            Some("已有同区块原文含当前识别缺失文字"),
        )]);
    }
    let mut spans = vec![];
    if prefix > 0 {
        spans.push(span(0, prefix, "adopted", None));
    }
    let mut middle = span(
        prefix,
        end,
        "pending",
        Some("与已有同图同区块文字不一致，保留两种原文"),
    );
    middle["comparisonText"] = json!(block.text);
    middle["comparisonRef"] = json!(block.block_ref);
    spans.push(middle);
    if end < n {
        spans.push(span(end, n, "adopted", None));
    }
    Some(spans)
}
pub fn project(
    source: &CloudSource,
    visual: &Value,
    local: Option<(&ImageInput, &Assessment)>,
) -> Result<Value, sqlx::Error> {
    if source.image.width == 0
        || source.image.height == 0
        || source.image.blob_sha256.len() != 64
        || (source.image_role == ImageRole::FirstImage && source.sequence_ordinal != Some(1))
        || local.is_some_and(|(input, _)| {
            input.image.work_ref != source.image.work_ref
                || input.image.blob_sha256 != source.image.blob_sha256
                || input.image.width != source.image.width
                || input.image.height != source.image.height
        })
    {
        return Err(invalid("cloud_ocr_source_invalid"));
    }
    let values = visual["regions"]
        .as_array()
        .filter(|r| r.len() <= 200)
        .ok_or_else(|| invalid("cloud_ocr_regions_invalid"))?;
    let mut regions = vec![];
    let mut seen = std::collections::HashSet::new();
    for value in values {
        let region_ref = text(value, "regionRef")?;
        let original = text(value, "text")?;
        let role = text(value, "role")?;
        let legibility = text(value, "legibility")?;
        if region_ref.is_empty() || !seen.insert(region_ref) || original.chars().count() > 20_000 {
            return Err(invalid("cloud_ocr_region_invalid"));
        }
        if ![
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
            || !["clear", "partial", "unreadable"].contains(&legibility)
        {
            return Err(invalid("cloud_ocr_role_invalid"));
        }
        let b = bbox(&value["bbox"])?;
        let length = original.chars().count();
        let (status, reason) = if ["background", "ui", "brand"].contains(&role) {
            ("excluded", Some("界面、背景或品牌文字"))
        } else if visual["unreadable"] == true
            || legibility != "clear"
            || ["other", "uncertain", "subtitle"].contains(&role)
        {
            ("pending", Some("模型可读性或表达归属尚未确认"))
        } else {
            ("adopted", None)
        };
        let spans = if length == 0 {
            vec![]
        } else if status == "adopted" {
            local_spans(original, b, local).unwrap_or_else(|| vec![span(0, length, status, reason)])
        } else {
            vec![span(0, length, status, reason)]
        };
        let pending = spans.iter().any(|s| s["status"] == "pending");
        regions.push(json!({"regionRef":region_ref,"text":original,"role":role,"legibility":legibility,"bbox":b,"status":if status=="adopted"&&pending{"adopted"}else{status},"reason":reason,"sourceStart":0,"sourceEnd":length,"spans":spans}));
    }
    let title = if cover(source) {
        visual["titleText"].as_str()
    } else {
        None
    };
    let title_status = if !cover(source) || visual["titlePresence"] == "absent" {
        "absent"
    } else if visual["unreadable"] == true
        || visual["titlePresence"] != "present"
        || title.is_none()
        || regions
            .iter()
            .filter(|r| r["role"] == "main_title")
            .any(|r| {
                r["spans"]
                    .as_array()
                    .is_some_and(|s| s.iter().any(|s| s["status"] != "adopted"))
            })
    {
        "pending"
    } else {
        "adopted"
    };
    if let Some(title) = title {
        let visible = regions
            .iter()
            .filter(|r| r["role"] == "main_title")
            .filter_map(|r| r["text"].as_str())
            .collect::<String>();
        if normalize(title) != normalize(&visible) {
            return Err(invalid("cloud_ocr_title_not_literal"));
        }
    }
    Ok(
        json!({"producer":"cloud_vision","ruleVersion":RULE_VERSION,"image":source.image,"imageRole":source.image_role,"ordinal":source.sequence_ordinal,
        "coverTitle":title,"coverTitleStatus":title_status,"regions":regions,"imageText":preview(&regions,cover(source)),"humanVerified":false}),
    )
}
pub async fn persist_in(
    tx: &mut Transaction<'_, Postgres>,
    source: &CloudSource,
    derivative: Uuid,
    layout: Uuid,
    raw: &str,
    visual: &Value,
) -> Result<Value, sqlx::Error> {
    // Optional existing local comparison; no OCR is scheduled to obtain one.
    let local=sqlx::query("SELECT a.observation,a.assessment FROM linggan_media_ocr_adoption a JOIN linggan_media_ocr_layout l USING(layout_ref) WHERE l.content_public_ref=$1 AND l.blob_sha256=$2 AND l.engine='paddleocr' ORDER BY a.created_at DESC,a.adoption_ref DESC LIMIT 1")
        .bind(source.image.work_ref).bind(&source.image.blob_sha256).fetch_optional(&mut **tx).await?;
    let local = local
        .map(|r| {
            Ok::<_, sqlx::Error>((
                serde_json::from_value::<ImageInput>(r.get("observation"))
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
                serde_json::from_value::<Assessment>(r.get("assessment"))
                    .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            ))
        })
        .transpose()?;
    let mut assessment = project(source, visual, local.as_ref().map(|(i, a)| (i, a)))?;
    let raw_sha = crate::ocr_adoption::content_hash(raw.as_bytes());
    let observation =
        json!({"producer":"cloud_vision","source":source,"rawSha256":raw_sha,"result":visual});
    let fingerprint = crate::ocr_adoption::content_hash(observation.to_string().as_bytes());
    assessment["inputFingerprint"] = json!(fingerprint);
    let adoption = Uuid::new_v4();
    // No local anchors or human verification are implied by successful cloud transport.
    sqlx::query("INSERT INTO linggan_media_ocr_adoption(adoption_ref,derivative_ref,layout_ref,rule_version,input_fingerprint,observation,assessment,line_ref_map,image_text,cover_title,state) VALUES($1,$2,$3,$4,$5,$6,$7,'[]',$8,$9,'NEEDS_REVIEW')")
        .bind(adoption).bind(derivative).bind(layout).bind(RULE_VERSION).bind(&fingerprint).bind(observation).bind(&assessment).bind(assessment["imageText"].as_str().unwrap_or("")).bind(assessment["coverTitle"].as_str()).execute(&mut **tx).await?;
    assessment["adoptionRef"] = json!(adoption);
    Ok(assessment)
}
pub(crate) async fn attach_media(
    tx: &mut Transaction<'_, Postgres>,
    items: &mut [Value],
    as_of: &str,
) -> Result<(), sqlx::Error> {
    let jobs: Vec<_> = items
        .iter()
        .filter(|i| {
            i["state"] == "ACQUIRED"
                && i["processorVersion"]
                    .as_str()
                    .is_some_and(is_processor_version)
        })
        .filter_map(|i| i["jobRef"].as_str()?.parse::<Uuid>().ok())
        .collect();
    if jobs.is_empty() {
        return Ok(());
    }
    let rows=sqlx::query("SELECT j.job_ref,a.*,l.image_width,l.image_height FROM linggan_media_ocr_adoption a JOIN linggan_media_derivative d USING(derivative_ref) JOIN linggan_media_processing_job j USING(job_ref) JOIN linggan_media_ocr_layout l USING(layout_ref) WHERE j.job_ref=ANY($1) AND a.created_at<=$2::text::timestamptz AND a.rule_version=$3")
        .bind(jobs).bind(as_of).bind(RULE_VERSION).fetch_all(&mut **tx).await?;
    for item in items.iter_mut().filter(|i| {
        i["state"] == "ACQUIRED"
            && i["processorVersion"]
                .as_str()
                .is_some_and(is_processor_version)
    }) {
        item["displayText"] = Value::Null;
        item["researchText"] = Value::Null;
        item["ocrLayering"] = Value::Null;
        let Some(row) = rows
            .iter()
            .find(|r| json!(r.get::<Uuid, _>("job_ref")) == item["jobRef"])
        else {
            continue;
        };
        let assessment: Value = row.get("assessment");
        let observation: Value = row.get("observation");
        let source: CloudSource = serde_json::from_value(observation["source"].clone())
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
        let fingerprint = crate::ocr_adoption::content_hash(observation.to_string().as_bytes());
        if fingerprint != row.get::<String, _>("input_fingerprint")
            || assessment["image"] != json!(source.image)
            || assessment["inputFingerprint"] != fingerprint
        {
            return Err(invalid("cloud_ocr_stored_identity_mismatch"));
        }
        let regions = assessment["regions"]
            .as_array()
            .ok_or_else(|| invalid("cloud_ocr_stored_regions_invalid"))?;
        let raw_blocks: Vec<_> = regions
            .iter()
            .map(|r| json!({"blockRef":r["regionRef"],"text":r["text"],"bbox":r["bbox"]}))
            .collect();
        let pending: Vec<_> = regions
            .iter()
            .filter(|r| {
                r["spans"]
                    .as_array()
                    .is_some_and(|s| s.iter().any(|s| s["status"] == "pending"))
            })
            .map(|r| r["regionRef"].clone())
            .collect();
        item["displayText"] = json!(
            assessment["imageText"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(600)
                .collect::<String>()
        );
        item["derivativeRef"] = json!(row.get::<Uuid, _>("derivative_ref"));
        item["ocrAdoption"] = json!({"adoptionRef":row.get::<Uuid,_>("adoption_ref"),"assessment":assessment,"rawBlocks":raw_blocks,"lineRefMap":[],"pendingBlocks":pending,"projectionVersion":RULE_VERSION,"sourceRelation":"image_transcription_not_author_claim"});
        item["ocrLayering"] = json!({"layoutRef":row.get::<Uuid,_>("layout_ref"),"layeringRef":row.get::<Uuid,_>("adoption_ref"),"layerVersion":RULE_VERSION,"state":"NEEDS_REVIEW","decisionSource":"cloud_vision_unverified","imageWidth":row.get::<i32,_>("image_width"),"imageHeight":row.get::<i32,_>("image_height"),"engineVersion":PROCESSOR_VERSION,"coverHeadline":assessment["coverTitle"],"imageSubstantiveText":Value::Null,"cleanCorpusEligible":false,"retainedLines":[],"excludedLines":[],"sourceFragments":[]});
        crate::ocr_adoption_persistence::attach_in(tx, item, row.get("adoption_ref"), as_of)
            .await?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn local_disagreement_preserves_unicode_context_and_does_not_choose_a_winner() {
        let original = "开场🌱：叶片保持清晣。结尾。";
        let spans =
            line_disagreements(original, std::iter::once(("line-original", "叶片保持清晰")))
                .unwrap();
        let points: Vec<_> = original.chars().collect();
        let doubtful = spans
            .iter()
            .find(|span| span["status"] == "pending")
            .unwrap();
        let changed: String = points[doubtful["start"].as_u64().unwrap() as usize
            ..doubtful["end"].as_u64().unwrap() as usize]
            .iter()
            .collect();
        assert_eq!(changed, "晣");
        assert_eq!(doubtful["comparisonText"], "叶片保持清晰");
        assert_eq!(
            spans
                .iter()
                .map(|span| points[span["start"].as_u64().unwrap() as usize
                    ..span["end"].as_u64().unwrap() as usize]
                    .iter()
                    .collect::<String>())
                .collect::<String>(),
            original
        );
        assert!(
            line_disagreements(
                "叶片保持清晣，叶片保持清晣",
                std::iter::once(("line", "叶片保持清晰"))
            )
            .is_none(),
            "ambiguous text matches cannot assign a conflict to one occurrence"
        );
    }
    fn source(role: ImageRole) -> CloudSource {
        CloudSource {
            image: ImageIdentity {
                work_ref: Uuid::nil(),
                image_ref: "synthetic".into(),
                blob_sha256: "a".repeat(64),
                width: 700,
                height: 900,
            },
            image_role: role,
            sequence_ordinal: Some(if role == ImageRole::Interior { 2 } else { 1 }),
        }
    }
    #[test]
    fn interior_heading_is_body_and_unknown_coordinates_remain_null() {
        let v = json!({"titlePresence":"present","titleText":"页内标题","regions":[{"regionRef":"r1","text":"页内标题","role":"main_title","legibility":"clear","bbox":null},{"regionRef":"r2","text":"完整正文","role":"body","legibility":"clear","bbox":[0.1,0.2,0.9,0.8]}],"unreadable":false});
        let p = project(&source(ImageRole::Interior), &v, None).unwrap();
        assert!(p["coverTitle"].is_null());
        assert_eq!(p["imageText"], "页内标题\n完整正文");
        assert!(p["regions"][0]["bbox"].is_null());
        let mut bad = v;
        bad["regions"][0]["bbox"] = json!([0, 0, 900, 1000]);
        assert!(project(&source(ImageRole::Interior), &bad, None).is_err());
    }
    #[test]
    fn cover_requires_literal_title_and_keeps_uncertainty_and_other_text() {
        let v = json!({"titlePresence":"present","titleText":"真实封面标题","regions":[{"regionRef":"r1","text":"真实封面标题","role":"main_title","legibility":"clear","bbox":null},{"regionRef":"r2","text":"仍需核对原字","role":"body","legibility":"partial","bbox":null},{"regionRef":"r3","text":"其他要点","role":"body","legibility":"clear","bbox":null}],"unreadable":false});
        let p = project(&source(ImageRole::FirstImage), &v, None).unwrap();
        assert_eq!(p["coverTitle"], "真实封面标题");
        assert_eq!(p["imageText"], "〔待核验〕\n其他要点");
        assert_eq!(p["regions"][1]["text"], "仍需核对原字");
        let mut bad = v;
        bad["titleText"] = json!("新增的标题");
        assert!(project(&source(ImageRole::FirstImage), &bad, None).is_err());
    }
}
