use super::{ResearchDossier, VERSION, content, coverage, inventory::Inventory};
use crate::{creator_discovery::Fragment, work_resource_current::WorkResourceCurrent};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn assemble(
    work: &WorkResourceCurrent,
    readings: Vec<Value>,
    inventory: Inventory,
    layouts: Vec<Value>,
    as_of: &str,
) -> ResearchDossier {
    let mut materials = vec![body(work, &readings)];
    if let Some(tags) = tags(work, &inventory) {
        materials.push(tags);
    }
    let slots = super::duplicates::collapse(inventory.slots.clone(), &readings);
    let mut media = media_materials(work, &readings, &slots);
    if let Some(expected) = expected_pages(&inventory) {
        for page in 1..=expected {
            if !media
                .iter()
                .any(|m| m["kind"] == "image" && m["page"] == page)
            {
                let mut missing = empty(work.public_ref, "image", None, Some(page), "not_acquired");
                missing["issues"] = json!([issue(
                    &format!("missing_page_{page}"),
                    "missing_page",
                    "平台图片序列中的这一页尚未取得",
                    "source_refresh"
                )]);
                media.push(missing);
            }
        }
    }
    media.sort_by_key(|m| {
        (
            match m["kind"].as_str() {
                Some("image") => 0,
                Some("audio") => 1,
                _ => 2,
            },
            m["page"].as_u64().unwrap_or(u64::MAX),
            m["slotKey"].to_string(),
        )
    });
    materials.extend(media);
    let note = note(work, &materials, &slots);
    let fragments = fragments(work, &materials);
    let coverage = coverage::project(&materials, &slots, expected_pages(&inventory));
    let base = format!(
        "/api/local/work-resources/{}/research-dossier",
        work.public_ref
    );
    let mut dossier = ResearchDossier {
        contract: VERSION,
        work_ref: work.public_ref,
        revision: String::new(),
        material_revision: String::new(),
        as_of: as_of.into(),
        note,
        materials,
        coverage,
        attachments: json!({"comments":{"href":format!("{base}/comments")},
            "author":{"href":format!("{base}/metadata?sections=author")},"interaction":{"href":format!("{base}/metadata?sections=interaction")},
            "versions":{"href":format!("{base}/candidates")}}),
        selection: Value::Null,
        fragments,
        media: layouts,
        media_inventory: slots,
        candidate_statuses: vec![],
        metadata: metadata(work),
    };
    super::refresh_revision(&mut dossier);
    dossier.selection = json!({"mode":"full","kinds":[],"unitRefs":[],"blockRefs":[],"imagePages":[],"omittedUnits":0,"completeRevision":dossier.revision});
    dossier
}

pub(super) fn public_material(mut value: Value) -> Value {
    if let Some(object) = value.as_object_mut() {
        for key in [
            "derivativeRef",
            "jobRef",
            "sourceLocation",
            "createdAt",
            "revision",
        ] {
            object.remove(key);
        }
        object.entry("originalUrl").or_insert(Value::Null);
    }
    value
}

fn body(work: &WorkResourceCurrent, readings: &[Value]) -> Value {
    let source = work.body_source.material_ref.map(|s| s.to_string());
    if let Some(body) = readings
        .iter()
        .find(|m| m["kind"] == "body" && m["sourceVersion"].as_str() == source.as_deref())
    {
        return public_material(body.clone());
    }
    let mut unit = empty(
        work.public_ref,
        "body",
        None,
        None,
        if work.body_state == "KNOWN" {
            "present"
        } else {
            "not_acquired"
        },
    );
    unit["sourceRef"] = json!(work.body_source.material_ref);
    unit["sourceVersion"] = json!(source);
    if source.is_some() {
        unit["issues"] = json!([issue(
            "body_not_processed",
            "not_processed",
            "正文已接纳，尚未生成规范化底稿；需要显式重建",
            "source_refresh"
        )]);
    }
    unit
}

fn tags(work: &WorkResourceCurrent, inventory: &Inventory) -> Option<Value> {
    let source = work.detail_material_ref?;
    let tags = inventory.tags.get(&source)?;
    let values = tags["values"].as_array()?;
    let state = tags["state"].as_str()?;
    if state != "provided" {
        return None;
    }
    let mut unit = empty(
        work.public_ref,
        "tags",
        None,
        None,
        if values.is_empty() {
            "known_empty"
        } else {
            "present"
        },
    );
    unit["sourceRef"] = json!(source);
    unit["sourceVersion"] = json!(format!(
        "{}:{}",
        source,
        tags["ruleVersion"].as_str().unwrap_or("")
    ));
    unit["reading"] = json!({"generatedBy":"platform","adoption":"automatic","verification":"unchecked","ruleVersion":tags["ruleVersion"],"review":null});
    if !values.is_empty() {
        unit["blocks"] = json!([{"ref":format!("{}.{}.0",work.public_ref,source),"kind":"list","ordered":false,"items":values,"speaker":"author"}]);
    }
    Some(unit)
}

fn empty(work: Uuid, kind: &str, slot: Option<&str>, page: Option<u64>, state: &str) -> Value {
    json!({"ref":format!("{work}.{kind}.{}",slot.map(str::to_owned).unwrap_or_else(||page.map_or("native".into(),|v|v.to_string()))),
        "kind":kind,"slotKey":slot,"page":page,"sourceState":state,"sourceRef":null,"sourceVersion":null,"currentRevisionRef":null,
        "originalUrl":null,"reading":null,"blocks":[],"issues":[]})
}

pub(super) fn issue(key: &str, code: &str, message: &str, resolution: &str) -> Value {
    json!({"key":key,"code":code,"message":message,"resolution":resolution})
}

fn expected_pages(inventory: &Inventory) -> Option<u64> {
    let m = inventory.manifest.as_ref()?;
    (m["state"] == "complete" && m["source"] == "platform_image_list")
        .then(|| m["expectedPages"].as_u64())
        .flatten()
}

fn media_materials(work: &WorkResourceCurrent, readings: &[Value], slots: &[Value]) -> Vec<Value> {
    let mut materials = vec![];
    for slot in slots {
        let kinds: &[&str] = if slot["component"] == "motion" {
            &["audio", "video_frame"]
        } else {
            &["image"]
        };
        for kind in kinds {
            let reading = readings.iter().find(|r| {
                r["slotKey"] == slot["slotKey"]
                    && r["kind"] == *kind
                    && r["sourceVersion"] == slot["sourceVersion"]
            });
            let mut material = if let Some(r) = reading {
                public_material(r.clone())
            } else {
                empty(
                    work.public_ref,
                    kind,
                    slot["slotKey"].as_str(),
                    slot["page"].as_u64(),
                    slot["sourceState"].as_str().unwrap_or("not_acquired"),
                )
            };
            material["sourceRef"] = slot["sourceRef"].clone();
            material["sourceVersion"] = slot["sourceVersion"].clone();
            material["page"] = if *kind == "image" {
                slot["page"].clone()
            } else {
                Value::Null
            };
            material["originalUrl"] = if *kind == "image" {
                slot["originalUrl"].clone()
            } else {
                Value::Null
            };
            if let Some(page_source) = slot.get("pageSource") {
                material["pageSource"] = page_source.clone();
            }
            if slot["sourceState"] == "restricted" {
                material["sourceState"] = json!("restricted");
                material["currentRevisionRef"] = Value::Null;
                material["blocks"] = json!([]);
                material["reading"] = Value::Null;
                material["issues"] = json!([]);
            } else if reading.is_none() && slot["sourceState"] == "present" {
                material["issues"] = json!([issue(
                    "material_not_processed",
                    "not_processed",
                    "来源已取得，尚无当前可用的规范化识读",
                    "source_refresh"
                )]);
            }
            if *kind == "image" && slot["displayOrderState"] != "KNOWN" {
                material["issues"]
                    .as_array_mut()
                    .expect("issues")
                    .push(issue(
                        "page_order_unknown",
                        "page_order_unknown",
                        "平台图片页序尚未确认",
                        "source_refresh",
                    ));
            }
            if *kind == "video_frame" && material["sourceState"] == "present" {
                material["issues"]
                    .as_array_mut()
                    .expect("issues")
                    .push(issue(
                        "video_sampling_incomplete",
                        "sampling_incomplete",
                        "仅包含实际采样的帧，未覆盖整段视频",
                        "source_refresh",
                    ));
            }
            if slot["duplicateReadingConflict"] == true {
                material["issues"]
                    .as_array_mut()
                    .expect("issues")
                    .push(issue(
                        "duplicate_image_readings",
                        "duplicate_image_readings",
                        "同一已验证图像存在不同的已采用读本；需对照来源确认，当前保留各自血缘",
                        "source_refresh",
                    ));
            }
            materials.push(material);
        }
    }
    materials
}

fn note(work: &WorkResourceCurrent, materials: &[Value], slots: &[Value]) -> Value {
    let title = work.title.as_deref().filter(|title| {
        work.title_state == "KNOWN"
            && work.title_qualification == "source_value"
            && !title.trim().is_empty()
    });
    let heading = materials
        .iter()
        .filter(|m| {
            m["kind"] == "image"
                && (m["page"] == 1
                    || slots
                        .iter()
                        .any(|s| s["purpose"] == "cover" && s["slotKey"] == m["slotKey"]))
        })
        .flat_map(|m| m["blocks"].as_array().into_iter().flatten())
        .find(|b| b["kind"] == "heading")
        .and_then(|b| b["text"].as_str());
    json!({"platform":work.platform,"externalId":work.content_external_id,"sourceUrl":source_url(work),
        "platformTitle":title,"titleSource":work.title_source.material_ref.map(|r|json!({"sourceRef":r,"sourceVersion":r.to_string()})),
        "displayTitle":title.or(heading),"displayTitleBasis":if title.is_some(){"platform"}else if heading.is_some(){"image_heading"}else{"missing"}})
}

fn source_url(work: &WorkResourceCurrent) -> Option<String> {
    match work.platform.as_str() {
        "xhs" => Some(format!(
            "https://www.xiaohongshu.com/explore/{}",
            work.content_external_id
        )),
        _ => None,
    }
}

pub(super) fn fragments(work: &WorkResourceCurrent, materials: &[Value]) -> Vec<Fragment> {
    let mut output = vec![];
    if let (Some(title), Some(source)) = (
        work.title.as_deref().filter(|title| {
            work.title_state == "KNOWN"
                && work.title_qualification == "source_value"
                && !title.trim().is_empty()
        }),
        work.title_source.material_ref,
    ) {
        let reference = format!("{}.{}.title", work.public_ref, source);
        let structure = json!({"ref":reference,"kind":"heading","text":title,"speaker":"author"});
        output.push(Fragment {
            fragment_id: reference,
            source_ref: source,
            field: "title".into(),
            source_version: source.to_string(),
            start: 0,
            end: title.chars().count(),
            text: title.into(),
            speaker: "author".into(),
            structure,
            issues: vec![],
            reading_ref: None,
            unit_ref: None,
        });
    }
    for unit in materials {
        let Some(source) = unit["sourceRef"]
            .as_str()
            .and_then(|v| v.parse::<Uuid>().ok())
        else {
            continue;
        };
        let Some(version) = unit["sourceVersion"].as_str() else {
            continue;
        };
        if unit["sourceState"] == "restricted" {
            continue;
        }
        for block in unit["blocks"].as_array().into_iter().flatten() {
            let Some(reference) = block["ref"].as_str() else {
                continue;
            };
            let field = match unit["kind"].as_str() {
                Some("body") => "body",
                Some("tags") => "tags",
                Some("image") => "ocr",
                Some("audio") => "transcript",
                Some("video_frame") => "frame_text",
                _ => continue,
            };
            let text = content::block_text(block);
            let mut issues = unit["issues"].as_array().cloned().unwrap_or_default();
            issues.extend(block["issues"].as_array().into_iter().flatten().cloned());
            output.push(Fragment {
                fragment_id: reference.into(),
                source_ref: source,
                field: field.into(),
                source_version: version.into(),
                start: 0,
                end: text.chars().count(),
                text,
                speaker: block["speaker"].as_str().unwrap_or("unconfirmed").into(),
                structure: block.clone(),
                issues,
                reading_ref: unit["currentRevisionRef"]
                    .as_str()
                    .and_then(|r| r.parse().ok()),
                unit_ref: unit["ref"].as_str().and_then(|r| r.parse().ok()),
            });
        }
    }
    output
}

fn metadata(work: &WorkResourceCurrent) -> Value {
    json!({"author":{"externalId":work.author_external_id,"displayName":work.creator_display_name,"state":work.creator_display_name_state,
        "sourceRef":work.creator_source.material_ref,"observedAt":work.creator_source.observed_at},
        "interaction":{"likes":{"value":work.like_count,"state":work.like_count_state,"sourceRef":work.like_source.material_ref,"observedAt":work.like_source.observed_at},
        "comments":{"value":work.comment_count,"state":work.comment_count_state,"sourceRef":work.comment_source.material_ref,"observedAt":work.comment_source.observed_at},
        "collects":{"value":work.collect_count,"state":work.collect_count_state,"sourceRef":work.collect_source.material_ref,"observedAt":work.collect_source.observed_at},
        "shares":{"value":work.share_count,"state":work.share_count_state,"sourceRef":work.share_source.material_ref,"observedAt":work.share_source.observed_at}},
        "publishedAt":work.published_at,"publishedPrecision":work.published_at_precision,"observedAt":work.observed_at})
}
