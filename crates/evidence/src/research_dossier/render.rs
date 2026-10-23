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

pub fn markdown(dossier: &ResearchDossier) -> String {
    // Image-derived display titles remain complete evidence blocks at their source position.
    let title = dossier.note["platformTitle"].as_str().unwrap_or("研究底稿");
    let mut out = format!(
        "# {title}\n\n作品：{}\n截点：{}\n合同：{}\n\n",
        dossier.work_ref, dossier.as_of, dossier.contract
    );
    if dossier.selection["mode"] == "explicit" {
        out.push_str("> 当前导出为显式选择的材料。\n\n");
    }
    for unit in &dossier.materials {
        let label = match unit["kind"].as_str() {
            Some("image") => unit["page"]
                .as_u64()
                .map_or("图片（页序未知）".into(), |p| format!("图片 {p}")),
            Some("body") => "正文".into(),
            Some("tags") => "话题标签".into(),
            Some("audio") => "音频转写".into(),
            _ => "视频帧文字".into(),
        };
        out.push_str(&format!("## {label}\n\n"));
        let blocks = unit["blocks"].as_array().map(Vec::as_slice).unwrap_or(&[]);
        if blocks.is_empty() {
            out.push_str(&format!("{}\n\n", empty_label(unit)));
        }
        for block in blocks {
            render_block(&mut out, block);
        }
        for issue in unit["issues"].as_array().into_iter().flatten() {
            render_issue(&mut out, issue);
        }
    }
    out.push_str("## 覆盖与来源\n\n");
    let inventory = &dossier.coverage["imageInventory"];
    let total = inventory["expectedPages"]
        .as_u64()
        .map_or("未知".into(), |n| n.to_string());
    out.push_str(&format!(
        "图片总页数：{total}；已取得：{}；已识读：{}。\n\n",
        inventory["acquiredPages"], inventory["processedPages"]
    ));
    for unit in &dossier.materials {
        if let Some(source) = unit["sourceRef"].as_str() {
            out.push_str(&format!(
                "- 材料 {}；来源 {}；版本 {}；识读 {}\n",
                unit["ref"].as_str().unwrap_or(""),
                source,
                unit["sourceVersion"].as_str().unwrap_or(""),
                unit["currentRevisionRef"].as_str().unwrap_or("未生成")
            ));
        }
    }
    out
}

fn empty_label(unit: &Value) -> &'static str {
    match unit["sourceState"].as_str() {
        Some("known_empty") => "来源明确为空。",
        Some("restricted") => "来源已受限，内容不可读取。",
        Some("not_applicable") => "此材料不适用。",
        Some("present") => "来源已取得，尚未生成可读底稿。",
        _ => "尚未取得此材料。",
    }
}

fn render_block(out: &mut String, block: &Value) {
    if block["speaker"] != "author" && block["kind"] != "dialogue" {
        out.push_str(&format!(
            "*{}*\n\n",
            content::speaker_label(block["speaker"].as_str().unwrap_or("unconfirmed"))
        ));
    }
    let text = content::block_text(block);
    match block["kind"].as_str() {
        Some("heading") => out.push_str(&format!("### {text}\n\n")),
        Some("quote") => out.push_str(&format!("> {}\n\n", text.replace('\n', "\n> "))),
        Some("gap") => out.push_str(&format!("〔{text}〕\n\n")),
        _ => {
            out.push_str(&text);
            out.push_str("\n\n");
        }
    }
    if let Some(time) = block.get("time") {
        if time["startMs"] == time["endMs"] {
            out.push_str(&format!("*来源时间：{} ms*\n\n", time["startMs"]));
        } else {
            out.push_str(&format!(
                "*来源时间：{}–{} ms*\n\n",
                time["startMs"], time["endMs"]
            ));
        }
    }
    for issue in block["issues"].as_array().into_iter().flatten() {
        render_issue(out, issue);
    }
}

fn render_issue(out: &mut String, issue: &Value) {
    if let Some(message) = issue["message"].as_str() {
        out.push_str(&format!("> 待确认：{message}\n\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research_dossier::{Selection, VERSION, select};
    use uuid::Uuid;

    fn fixture(platform_title: Option<&str>, body_heading: Option<&str>) -> ResearchDossier {
        let mut materials = vec![];
        if let Some(text) = body_heading {
            materials.push(json!({"ref":"body-unit","kind":"body","sourceState":"present","issues":[],"blocks":[
                {"ref":"body-heading","kind":"heading","text":text,"speaker":"other","time":{"startMs":10,"endMs":20},
                    "issues":[{"key":"body-title","code":"unclear","message":"正文标题待确认","resolution":"reading_edit"}]},
                {"ref":"body-next","kind":"paragraph","text":"正文后续","speaker":"author"}
            ]}));
        }
        materials.push(json!({"ref":"image-unit","kind":"image","page":1,"sourceState":"present","issues":[],"blocks":[
            {"ref":"image-heading","kind":"heading","text":"同名标题","speaker":"unconfirmed","time":{"startMs":40,"endMs":40},
                "issues":[{"key":"image-title","code":"unclear","message":"图片标题待确认","resolution":"reading_edit"}]},
            {"ref":"image-next","kind":"paragraph","text":"图片后续","speaker":"author"}
        ]}));
        ResearchDossier {
            contract: VERSION,
            work_ref: Uuid::from_u128(1),
            revision: "full-revision".into(),
            material_revision: "material-revision".into(),
            as_of: "2026-10-23T00:00:00Z".into(),
            note: json!({"platformTitle":platform_title,"displayTitle":platform_title.unwrap_or("同名标题"),
                "displayTitleBasis":if platform_title.is_some(){"platform"}else{"image_heading"}}),
            materials,
            coverage: json!({"imageInventory":{"expectedPages":1,"acquiredPages":1,"processedPages":1}}),
            attachments: json!({}),
            selection: json!({"mode":"full"}),
            fragments: vec![],
            media: vec![],
            media_inventory: vec![],
            candidate_statuses: vec![],
            metadata: json!({}),
        }
    }

    fn section<'a>(markdown: &'a str, label: &str) -> &'a str {
        let remaining = markdown.split_once(&format!("## {label}\n\n")).unwrap().1;
        remaining
            .split_once("\n## ")
            .map_or(remaining, |(section, _)| section)
            .trim_end()
    }

    fn assert_body(markdown: &str, heading: &str) {
        assert_eq!(
            section(markdown, "正文"),
            format!(
                "*他人*\n\n### {heading}\n\n*来源时间：10–20 ms*\n\n> 待确认：正文标题待确认\n\n正文后续"
            )
        );
    }

    fn assert_image(markdown: &str) {
        assert_eq!(
            section(markdown, "图片 1"),
            "*角色未确认*\n\n### 同名标题\n\n*来源时间：40 ms*\n\n> 待确认：图片标题待确认\n\n图片后续"
        );
    }

    #[test]
    fn markdown_image_title_preserves_local_role_time_and_issues_once() {
        let output = markdown(&fixture(None, None));
        assert!(output.starts_with("# 研究底稿\n\n"));
        assert_image(&output);
        for text in [
            "同名标题",
            "角色未确认",
            "来源时间：40 ms",
            "图片标题待确认",
            "图片后续",
        ] {
            assert_eq!(output.matches(text).count(), 1, "{text}");
        }
    }

    #[test]
    fn markdown_same_text_headings_keep_both_source_positions() {
        let output = markdown(&fixture(None, Some("同名标题")));
        assert!(output.starts_with("# 研究底稿\n\n"));
        assert_eq!(output.matches("### 同名标题\n").count(), 2);
        assert_body(&output, "同名标题");
        assert_image(&output);
        assert!(output.find("## 正文\n").unwrap() < output.find("## 图片 1\n").unwrap());
    }

    #[test]
    fn markdown_explicit_body_selection_does_not_emit_unselected_image_title() {
        for heading in ["同名标题", "仅正文标题"] {
            let mut dossier = fixture(None, Some(heading));
            let note = dossier.note.clone();
            select(
                &mut dossier,
                &Selection {
                    kinds: vec!["body".into()],
                    ..Default::default()
                },
            )
            .unwrap();
            assert_eq!(
                dossier.note, note,
                "selection retains the full note display metadata"
            );
            assert_eq!(dossier.materials.len(), 1);
            let output = markdown(&dossier);
            assert!(output.starts_with("# 研究底稿\n\n"));
            assert!(output.contains("> 当前导出为显式选择的材料。"));
            assert_body(&output, heading);
            assert_eq!(output.matches(&format!("### {heading}\n")).count(), 1);
            for omitted in ["## 图片 1", "图片标题待确认", "来源时间：40 ms", "图片后续"]
            {
                assert!(!output.contains(omitted), "{omitted}");
            }
            if heading != "同名标题" {
                assert!(!output.contains("同名标题"));
            }
        }
    }

    #[test]
    fn markdown_platform_title_does_not_replace_same_text_material_headings() {
        let output = markdown(&fixture(Some("同名标题"), Some("同名标题")));
        assert!(output.starts_with("# 同名标题\n\n"));
        assert_eq!(
            output.lines().filter(|line| *line == "# 同名标题").count(),
            1
        );
        assert_eq!(output.matches("### 同名标题\n").count(), 2);
        assert_body(&output, "同名标题");
        assert_image(&output);
    }
}
