use super::{ResearchDossier, content};
use crate::creator_discovery::{Fragment, hash};
use serde_json::{Value, json};

pub fn context_sources(dossier: &ResearchDossier) -> Vec<Value> {
    dossier.fragments.iter().map(|f|{
        let material=dossier.materials.iter().find(|m|m["blocks"].as_array().is_some_and(|b|b.iter().any(|b|b["ref"]==f.fragment_id)));
        json!({"fragmentId":f.fragment_id,"blockRef":f.fragment_id,"sourceRef":f.source_ref,"sourceVersion":f.source_version,
            "readingRef":f.reading_ref,"unitRef":f.unit_ref,"asOf":dossier.as_of,
            "kind":match f.field.as_str(){"title"=>"native_title","ocr"=>"image_substantive_text","transcript"=>"asr_text","frame_text"=>"frame_ocr_text",other=>other},
            "text":f.text,"characterCount":f.text.chars().count(),"speaker":f.speaker,"structure":f.structure,"issues":f.issues,
            "slotOrdinal":material.map(|m|m["page"].clone()),"sourceLocation":material.map(|m|json!({"unitRef":m["ref"],"readingRef":m["currentRevisionRef"],"slotKey":m["slotKey"],"page":m["page"],"bbox":f.structure.get("bbox"),"time":f.structure.get("time")})),
            "contentHash":hash(&f.text)})
    }).collect()
}

/// Full block order; callers supply an explicit task selection, never a character allowance.
pub fn render_materials(fragments: &[Fragment]) -> String {
    let mut out = String::new();
    for (index, f) in fragments.iter().enumerate() {
        out.push_str(&format!("[S{}] {}\n", index + 1, field_label(&f.field)));
        if f.speaker != "author" {
            out.push_str(&format!("角色：{}\n", content::speaker_label(&f.speaker)));
        }
        out.push_str(&f.text);
        out.push_str("\n\n");
        for issue in &f.issues {
            if let Some(message) = issue["message"].as_str() {
                out.push_str(&format!("> 待确认：{message}\n"));
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
    fn markdown_uses_the_same_ordered_blocks_and_human_readable_positions() {
        let d = fixture(None, Some("同名标题"));
        let output = markdown(&d);
        assert!(output.starts_with("# 同名标题\n"));
        assert!(output.find("正文后续").unwrap() < output.find("图片后续").unwrap());
        for text in [
            "00:00.010–00:00.020",
            "00:00.040",
            "正文标题待确认",
            "图片标题待确认",
        ] {
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
        assert!(output.contains("正文标题") && output.contains("图片标题待确认"));
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
            assert_eq!(output.matches(message).count(), 1);
        }
        assert!(output.contains("来源已确认无可读文字"));
        assert!(output.contains("既有已确认无文字的结果仍有效"));
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
            5,
            "one run in body/images/frames and one in each audio material"
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
    fn markdown_preserves_execution_state_and_valid_empty_results() {
        for (state, label) in [
            ("pending", "待处理"),
            ("running", "处理中"),
            ("failed", "处理失败"),
            ("cancelled", "处理已取消"),
            ("invalidated", "处理结果已失效"),
        ] {
            let mut d = fixture(Some("合成处理状态验证"), None);
            d.materials = vec![
                json!({"ref":"image","kind":"image","page":1,"sourceState":"present",
                "currentRevisionRef":null,"blocks":[],"processing":{"state":state}}),
            ];
            d.coverage["sources"] = json!({"images":{"state":"not_processed"}});
            super::super::reading::rebuild(&mut d);
            assert!(markdown(&d).contains(&format!("图片文字第 1 页：{label}。")));
            d.materials[0]["sourceState"] = json!("known_empty");
            d.materials[0]["currentRevisionRef"] = json!("valid-empty-reading");
            d.coverage["sources"]["images"]["state"] = json!("known_empty");
            let output = markdown(&d);
            assert!(output.contains(&format!("{label}；既有已确认无文字的结果仍有效。")));
            assert!(!output.contains("尚无当前可用的识读文字"));
            d.materials[0]["sourceState"] = json!("present");
            d.materials[0]["blocks"] = json!([{"ref":"readable","kind":"paragraph","text":"既有有效文字。","speaker":"author"}]);
            d.coverage["sources"]["images"]["state"] = json!("available");
            super::super::reading::rebuild(&mut d);
            assert!(markdown(&d).contains(&format!("{label}，已有读本仍可读。")));
            assert!(markdown(&d).contains("既有有效文字。"));
        }
    }
}
