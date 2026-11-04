use super::{ResearchDossier, content};
use crate::creator_discovery::{Fragment, hash};
use serde_json::{Value, json};

pub fn context_sources(dossier: &ResearchDossier) -> Vec<Value> {
    dossier.fragments.iter().map(|f|{
        let material=dossier.materials.iter().find(|m|m["blocks"].as_array().is_some_and(|b|b.iter().any(|b|b["ref"]==f.fragment_id)));
        json!({"fragmentId":f.fragment_id,"blockRef":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,
            "readingRef":f.reading_ref,"unitRef":f.unit_ref,"asOf":dossier.as_of,
            "verification":material.map(|m|m["reading"]["verification"].clone()),
            "contentType":f.structure.get("contentType"),"regionRef":f.structure.get("regionRef"),"regionLabel":f.structure.get("regionLabel"),
            "kind":if f.structure["contentType"]=="visual_description" {"image_visual_description"} else {match f.field.as_str(){"title"=>"native_title","ocr"=>"image_substantive_text","transcript"=>"asr_text","frame_text"=>"frame_ocr_text",other=>other}},
            "text":f.text,"characterCount":f.text.chars().count(),"speaker":f.speaker,"structure":f.structure,"issues":f.issues,
            "slotOrdinal":material.map(|m|m["page"].clone()),"sourceLocation":material.map(|m|json!({"unitRef":m["ref"],"readingRef":m["currentRevisionRef"],"slotKey":m["slotKey"],"page":m["page"],"bbox":f.structure.get("bbox"),"time":f.structure.get("time")})),
            "contentHash":hash(&f.text)})
    }).collect()
}

/// Model-facing projection of the frozen context; audit metadata stays in the manifest.
pub fn model_context(manifest: &Value) -> Value {
    let sources: Vec<Value> = manifest["sources"].as_array().into_iter().flatten().map(|source| {
        let mut value = json!({"source":source["blockRef"], "kind":source["kind"], "text":source["text"]});
        for key in ["unitRef", "speaker", "regionLabel"] {
            if let Some(v) = source.get(key).filter(|v| !v.is_null()) { value[key] = v.clone(); }
        }
        for key in ["sourceRole", "time"] {
            if let Some(v) = source["structure"].get(key).filter(|v| !v.is_null()) { value[key] = v.clone(); }
        }
        if let Some(parameters) = source["structure"]["parameters"].as_array().filter(|p| !p.is_empty()) {
            value["parameterCandidates"] = Value::Array(parameters.iter().map(|parameter| {
                let mut candidate = parameter.clone();
                for key in ["bindingStatus", "basisStatus"] {
                    if candidate.get(key).is_none_or(Value::is_null) { candidate[key] = json!("unconfirmed"); }
                }
                candidate
            }).collect());
        }
        value
    }).collect();
    let states: serde_json::Map<String, Value> = manifest["coverage"]["sources"].as_object().into_iter().flatten()
        .map(|(key,value)| (key.clone(), value["state"].clone())).collect();
    json!({"title":manifest["readingTitle"]["text"], "materials":sources,"availability":states})
}

/// Full block order; callers supply an explicit task selection, never a character allowance.
pub fn render_materials(fragments: &[Fragment]) -> String {
    let mut out = String::new();
    for (index, f) in fragments.iter().enumerate() {
        out.push_str(&format!("[S{}] {}\n", index + 1, field_label(&f.field)));
        if f.speaker == "other" {
            out.push_str(&format!("角色：{}\n", content::speaker_label(&f.speaker)));
        }
        if let Some(label) = f.structure["regionLabel"].as_str() {
            out.push_str(&format!("区域：{label}\n"));
        }
        if f.structure["contentType"] == "visual_description" {
            out.push_str("视觉描述：\n");
        } else if f.structure["contentType"] == "extracted_text" {
            out.push_str(match f.structure["sourceRole"].as_str() {
                Some("background") => "背景文字：\n",
                Some("brand") => "品牌文字：\n",
                Some("subtitle") => "字幕文字：\n",
                _ => "图片文字：\n",
            });
        }
        out.push_str(&f.text);
        out.push_str("\n\n");
        for candidate in f.structure["parameters"].as_array().into_iter().flatten() {
            let value = candidate["value"].as_str().unwrap_or("");
            if !value.is_empty() && !f.text.contains(value) {
                out.push_str(&format!("参数候选（归属未确认）：{}\n\n", candidate));
            }
        }
    }
    out
}

fn field_label(field: &str) -> &str {
    match field {
        "title" => "平台标题",
        "body" => "正文",
        "tags" => "话题标签",
        "ocr" => "图片文字",
        "cover_headline" => "图片标题",
        "transcript" => "音频转写",
        "frame_text" => "视频帧文字",
        other => other,
    }
}

mod document;
pub use document::markdown;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_dossier::{Selection, VERSION, select};
    use uuid::Uuid;

    #[test]
    fn model_input_keeps_content_and_attribution_without_audit_duplicates() {
        let manifest = json!({
            "readingTitle":{"text":"桌游"},
            "coverage":{"sources":{"images":{"state":"partial","issues":["页码未确认"]}}},
            "sources":[{
                "blockRef":"block-1", "unitRef":"image-2", "kind":"image_substantive_text",
                "text":"名称 | 年龄\n游戏甲 | 5岁", "speaker":"unconfirmed", "regionLabel":"左栏",
                "issues":[{"message":"核对说明"}], "verification":"unchecked",
                "structure":{"text":"重复正文", "sourceRole":"main",
                    "parameters":[{"subject":"游戏甲","attribute":"age","value":"5岁","basis":"左栏"}]}
            },{
                "blockRef":"block-2", "unitRef":"image-2", "kind":"image_visual_description",
                "text":"左侧是圆形卡片。", "structure":{}
            }]
        });
        let result = model_context(&manifest);
        assert_eq!(result["materials"][0]["text"], manifest["sources"][0]["text"]);
        assert_eq!(result["materials"][0]["unitRef"], "image-2");
        assert_eq!(result["materials"][0]["parameterCandidates"][0]["bindingStatus"], "unconfirmed");
        assert_eq!(result["materials"][0]["parameterCandidates"][0]["basisStatus"], "unconfirmed");
        assert_eq!(result["materials"][1]["kind"], "image_visual_description");
        assert_eq!(result["availability"]["images"], "partial");
        let output = result.to_string();
        for omitted in ["核对说明", "页码未确认", "重复正文", "verification", "structure"] {
            assert!(!output.contains(omitted));
        }
    }

    fn fixture(platform_title: Option<&str>, body_heading: Option<&str>) -> ResearchDossier {
        let mut materials = vec![];
        if let Some(text) = body_heading {
            materials.push(json!({"ref":"body-unit","kind":"body","purpose":"cover","sourceState":"present","issues":[],"blocks":[
                {"ref":"body-heading","kind":"heading","text":text,"speaker":"other","time":{"startMs":10,"endMs":20},
                    "issues":[{"key":"body-title","code":"unclear","message":"正文标题待确认","resolution":"reading_edit"}]},
                {"ref":"body-next","kind":"paragraph","text":"正文后续","speaker":"author"}
            ]}));
        }
        materials.push(json!({"ref":"image-unit","kind":"image","page":1,"purpose":"cover","sourceState":"present","issues":[],"blocks":[
            {"ref":"image-heading","kind":"heading","text":"同名标题","speaker":"unconfirmed","time":{"startMs":40,"endMs":40},
                "issues":[{"key":"image-title","code":"unclear","message":"图片标题待确认","resolution":"reading_edit"}]},
            {"ref":"image-next","kind":"paragraph","text":"图片后续","speaker":"author"}
        ]}));
        let mut dossier = ResearchDossier {
            contract: VERSION,
            work_ref: Uuid::from_u128(1),
            revision: "full-revision".into(),
            material_revision: "material-revision".into(),
            as_of: "2026-10-23T00:00:00Z".into(),
            note: json!({"platformTitle":platform_title,"displayTitle":platform_title.unwrap_or("同名标题"),
                "displayTitleBasis":if platform_title.is_some(){"platform"}else{"image_heading"}}),
            materials,
            reading_view: Value::Null,
            coverage: json!({"imageInventory":{"expectedPages":1,"acquiredPages":1,"processedPages":1}}),
            attachments: json!({}),
            selection: json!({"mode":"full"}),
            fragments: vec![],
            media: vec![],
            media_inventory: vec![],
            candidate_statuses: vec![],
            metadata: json!({}),
        };
        super::super::reading::rebuild(&mut dossier);
        dossier
    }

    #[test]
    fn ai_materials_explain_machine_status_once_and_keep_types_and_specific_issues() {
        let fragments: Vec<_> = ["extracted_text", "visual_description", "visual_description"]
            .into_iter()
            .enumerate()
            .map(|(i, kind)| Fragment {
                fragment_id: format!("f{i}"),
                source_ref: Uuid::nil(),
                field: "ocr".into(),
                source_version: "v1".into(),
                start: 0,
                end: 2,
                text: format!("内容{i}"),
                speaker: "unconfirmed".into(),
                structure: json!({"contentType":kind,"regionLabel":"左栏"}),
                issues: if i == 0 {
                    vec![json!({"message":"末行数字看不清"})]
                } else {
                    vec![]
                },
                reading_ref: None,
                unit_ref: None,
            })
            .collect();
        let output = render_materials(&fragments);
        assert!(!output.contains("核对状态以来源记录为准"));
        assert_eq!(output.matches("视觉描述：").count(), 2);
        assert_eq!(output.matches("图片文字：").count(), 1);
        assert!(!output.contains("末行数字看不清"));
        assert!(!output.contains("模型观察，非原文"));
        assert!(!output.contains("未经人工核实"));
        assert!(render_materials(&[]).is_empty());
        let mut dossier = fixture(Some("平台标题"), None);
        dossier.fragments = fragments;
        dossier.materials[0]["blocks"] = json!([{"ref":"f0"},{"ref":"f1"},{"ref":"f2"}]);
        for status in [json!("verified"), json!("unchecked"), Value::Null] {
            dossier.materials[0]["reading"] = json!({"verification":status});
            let sources = context_sources(&dossier);
            assert!(
                sources
                    .iter()
                    .all(|source| source["verification"] == status)
            );
            assert_eq!(sources[0]["kind"], "image_substantive_text");
            assert_eq!(sources[1]["kind"], "image_visual_description");
        }
        dossier.materials[0]
            .as_object_mut()
            .unwrap()
            .remove("reading");
        assert!(context_sources(&dossier)[0]["verification"].is_null());
    }
    #[test]
    fn online_material_keeps_type_region_and_image_source_in_document() {
        let mut d = fixture(Some("平台标题"), None);
        d.materials[0]["originalUrl"] = json!("/api/local/media/example");
        d.materials[0]["blocks"] = json!([
            {"ref":"text", "kind":"paragraph", "text":"仅在安静时进行。", "speaker":"unconfirmed", "contentType":"extracted_text", "regionRef":"r1", "regionLabel":"左栏"},
            {"ref":"visual", "kind":"paragraph", "text":"左栏有手势示意。", "speaker":"unconfirmed", "contentType":"visual_description", "regionRef":"r1", "regionLabel":"左栏"}
        ]);
        super::super::reading::rebuild(&mut d);
        let output = markdown(&d);
        for expected in [
            "图片 1",
            "[查看原图](</api/local/media/example>)",
            "图片文字（识别提取）",
            "视觉描述（模型观察，非原文）",
            "仅在安静时进行。",
            "左栏有手势示意。",
        ] {
            assert!(output.contains(expected), "{expected}: {output}");
        }
        assert_eq!(output.matches("查看原图").count(), 1);
        assert_eq!(d.note["displayTitle"], "平台标题");
    }

    #[test]
    fn markdown_uses_the_same_ordered_blocks_and_human_readable_positions() {
        let d = fixture(None, Some("同名标题"));
        let output = markdown(&d);
        assert!(output.starts_with("# 同名标题\n"));
        assert!(output.find("正文后续").unwrap() < output.find("图片后续").unwrap());
        for text in ["00:00.010–00:00.020", "00:00.040"] {
            assert_eq!(output.matches(text).count(), 1, "{text}");
        }
        for technical in [
            "来源块",
            "body-heading",
            "image-heading",
            "research-dossier.v2",
        ] {
            assert!(!output.contains(technical), "{technical}");
        }
        assert_eq!(
            output.matches("### 同名标题").count(),
            2,
            "identical text at two real positions survives"
        );
    }

    #[test]
    fn markdown_selection_rebuilds_title_without_unselected_image_text() {
        let mut d = fixture(None, Some("正文标题"));
        select(
            &mut d,
            &Selection {
                kinds: vec!["body".into()],
                ..Default::default()
            },
        )
        .unwrap();
        let output = markdown(&d);
        assert!(d.note["displayTitle"].is_null());
        assert!(output.starts_with("# 无标题\n"));
        assert!(
            output.contains("正文后续")
                && !output.contains("同名标题")
                && !output.contains("图片后续")
        );
    }

    #[test]
    fn markdown_platform_title_preserves_material_headings_and_issues() {
        let d = fixture(Some("平台标题"), Some("正文标题"));
        let output = markdown(&d);
        assert!(output.starts_with("# 平台标题\n"));
        assert!(output.contains("正文标题"));
        assert!(!output.contains("核对说明") && !output.contains("图片标题待确认"));
        assert_eq!(
            d.materials[0]["blocks"][0]["issues"][0]["message"],
            "正文标题待确认"
        );
    }

    #[test]
    fn issue_keys_are_local_to_their_material_and_empty_readings_stay_valid() {
        let mut d = fixture(Some("平台标题"), Some("正文标题"));
        for (index, unit) in d.materials.iter_mut().enumerate() {
            unit["issues"] =
                json!([{"key":"ocr_low_confidence_0","message":format!("第{index}处独立问题")}]);
        }
        d.coverage["issues"] = json!([{"key":"ocr_low_confidence_0","message":"独立覆盖问题"}]);
        d.materials.push(json!({"ref":"empty-audio","kind":"audio","sourceState":"known_empty",
            "currentRevisionRef":"known-empty-reading","processing":{"state":"failed"},"blocks":[],"issues":[]}));
        d.coverage["sources"] = json!({"audio":{"state":"known_empty"}});
        let output = markdown(&d);
        for message in ["第0处独立问题", "第1处独立问题", "独立覆盖问题"] {
            assert!(!output.contains(message));
        }
        assert!(output.contains("无可用口播"));
        assert!(!output.contains("既有已确认无文字的结果仍有效"));
        assert!(!output.contains("已处理，未检出可读文字"));
        assert!(!output.contains("处理失败，尚无当前可用"));
    }

    #[test]
    fn markdown_attribution_marks_role_runs_dialogue_and_independent_audio() {
        let mut d = fixture(Some("合成归属验证"), None);
        let paragraph = |reference: &str, speaker: &str| json!({"ref":reference,"kind":"paragraph","text":format!("{reference}的完整原话。"),"speaker":speaker});
        let mut body = vec![
            paragraph("a1", "author"),
            paragraph("a2", "author"),
            paragraph("o1", "other"),
            paragraph("o2", "other"),
            paragraph("a3", "author"),
        ];
        body.push(json!({"ref":"dialogue","kind":"dialogue","turns":[{"speaker":"author","text":"作者对话原句"},{"speaker":"other","text":"他人对话原句"}]}));
        body.extend([
            paragraph("a4", "author"),
            paragraph("u1", "unconfirmed"),
            paragraph("u2", "unconfirmed"),
        ]);
        let audio: Vec<_> = (0..26).map(|i| json!({"ref":format!("audio-{i}"),"kind":"paragraph","text":format!("第{i}段口播原话。"),"speaker":"unconfirmed","time":{"startMs":i*2000,"endMs":i*2000+1000}})).collect();
        d.materials = vec![
            json!({"ref":"body","kind":"body","blocks":body}),
            json!({"ref":"image1","kind":"image","page":1,"blocks":[paragraph("image1", "unconfirmed")]}),
            json!({"ref":"image2","kind":"image","page":2,"blocks":[paragraph("image2", "unconfirmed")]}),
            json!({"ref":"audio1","kind":"audio","blocks":audio}),
            json!({"ref":"audio2","kind":"audio","blocks":[paragraph("other-audio1", "unconfirmed"),paragraph("other-audio2", "unconfirmed")]}),
            json!({"ref":"frames","kind":"video_frame","blocks":[paragraph("frame", "unconfirmed")]}),
        ];
        super::super::reading::rebuild(&mut d);
        let before = serde_json::to_value(&d).unwrap();
        let output = markdown(&d);
        assert_eq!(
            output.matches("*角色未确认*").count(),
            0,
            "unknown role remains source metadata, not reading boilerplate"
        );
        assert_eq!(output.matches("*他人*").count(), 1);
        assert_eq!(
            output.matches("*作者*").count(),
            3,
            "first author, other→author, and dialogue→author must all be explicit"
        );
        assert!(
            output.contains("作者：作者对话原句\n\n他人：他人对话原句\n\n*作者*\n\na4的完整原话。")
        );
        for i in 0..26 {
            assert_eq!(output.matches(&format!("第{i}段口播原话。")).count(), 1);
        }
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
    }

    #[test]
    fn native_tags_use_one_line_without_changing_ordinary_lists_or_source_text() {
        let mut d = fixture(Some("合成标签验证"), None);
        d.materials = vec![
            json!({"ref":"body","kind":"body","sourceRef":"native-source","blocks":[
                {"ref":"body-p","kind":"paragraph","text":"保留实际发布文案。","speaker":"author"},
                {"ref":"body-list","kind":"list","ordered":false,"items":["第一条任务","第二条任务"],"speaker":"author"}]}),
            json!({"ref":"tags","kind":"tags","sourceRef":"native-source","blocks":[
                {"ref":"native-tags","kind":"list","ordered":false,"items":[" 任务[话题] ","任务","边界","<literal>[话题]"],"speaker":"author"}]}),
        ];
        super::super::reading::rebuild(&mut d);
        let before = serde_json::to_value(&d).unwrap();
        let output = markdown(&d);
        assert_eq!(
            output
                .matches("#任务　#任务　#边界　#\\<literal\\>")
                .count(),
            1
        );
        assert!(output.contains("- 第一条任务\n- 第二条任务"));
        assert!(!output.contains("- 任务"));
        assert!(output.contains("保留实际发布文案。"));
        assert_eq!(serde_json::to_value(&d).unwrap(), before);
    }

    #[test]
    fn ai_keeps_unanchored_parameter_candidates_with_explicit_binding_state() {
        let mut d = fixture(Some("参数核对"), None);
        d.materials = vec![
            json!({"ref":"image-unit","kind":"image","sourceState":"present","blocks":[
            {"ref":"image-param","kind":"paragraph","text":"原图标题","speaker":"unconfirmed",
             "contentType":"extracted_text","regionRef":"r1","parameters":[
                {"subject":"候选商品","attribute":"age","value":"8+","basis":"package"}]}]}),
        ];
        d.fragments = vec![Fragment {
            fragment_id: "image-param".into(),
            source_ref: Uuid::nil(),
            field: "ocr".into(),
            source_version: "v1".into(),
            start: 0,
            end: 4,
            text: "原图标题".into(),
            speaker: "unconfirmed".into(),
            structure: d.materials[0]["blocks"][0].clone(),
            issues: vec![],
            reading_ref: None,
            unit_ref: None,
        }];
        super::super::reading::rebuild(&mut d);
        let text = render_materials(&d.fragments);
        assert!(text.contains("原图标题"));
        assert_eq!(text.matches("参数候选（归属未确认）").count(), 1);
        assert!(text.contains("8+") && text.contains("package"));
        let sources = context_sources(&d);
        let candidate = sources
            .iter()
            .find(|s| s["blockRef"] == "image-param")
            .unwrap();
        assert_eq!(
            candidate["structure"]["parameters"][0]["bindingStatus"],
            "unconfirmed"
        );
        assert_eq!(
            candidate["structure"]["parameters"][0]["sourceRegionRef"],
            "r1"
        );
    }

    #[test]
    fn markdown_preserves_execution_state_and_valid_empty_results() {
        for (state, label) in [
            ("pending", "暂无可用文字"),
            ("running", "处理中"),
            ("failed", "处理失败"),
            ("cancelled", "暂无可用文字"),
            ("invalidated", "暂无可用文字"),
        ] {
            let mut d = fixture(Some("合成处理状态验证"), None);
            d.materials = vec![
                json!({"ref":"image","kind":"image","page":1,"sourceState":"present",
                "currentRevisionRef":null,"blocks":[],"processing":{"state":state}}),
            ];
            d.coverage["sources"] = json!({"images":{"state":"not_processed"}});
            super::super::reading::rebuild(&mut d);
            assert!(markdown(&d).contains(&format!("图片文字：{label}。")));
            d.materials[0]["sourceState"] = json!("known_empty");
            d.materials[0]["currentRevisionRef"] = json!("valid-empty-reading");
            d.coverage["sources"]["images"]["state"] = json!("known_empty");
            let output = markdown(&d);
            assert!(!output.contains("未包含内容"));
            assert!(!output.contains("尚无当前可用的识读文字"));
            d.materials[0]["sourceState"] = json!("present");
            d.materials[0]["blocks"] = json!([{"ref":"readable","kind":"paragraph","text":"既有有效文字。","speaker":"author"}]);
            d.coverage["sources"]["images"]["state"] = json!("available");
            super::super::reading::rebuild(&mut d);
            assert!(!markdown(&d).contains("未包含内容"));
            assert!(markdown(&d).contains("既有有效文字。"));
        }
    }
}
