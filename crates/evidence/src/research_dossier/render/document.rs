//! Human Markdown of the sole reading plan. Machine citations stay in render.rs.
use super::{ResearchDossier, content};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

mod text;

type Entry<'a> = (&'a Value, &'a Value);

#[derive(Default)]
struct Attribution {
    role: Option<String>,
    audio_unit: Option<String>,
}

impl Attribution {
    fn render(&mut self, out: &mut String, unit: &Value, block: &Value) {
        let audio_unit = (unit["kind"] == "audio")
            .then(|| unit["ref"].as_str())
            .flatten();
        if audio_unit != self.audio_unit.as_deref() {
            self.role = None;
            self.audio_unit = audio_unit.map(str::to_owned);
        }
        if block["kind"] == "dialogue" {
            // Each turn already has its own attribution; the next prose starts a new run.
            self.role = None;
            return;
        }
        let role = block["speaker"].as_str().unwrap_or("unconfirmed");
        if self.role.as_deref() != Some(role) && role != "unconfirmed" {
            out.push_str(&format!("*{}*\n\n", content::speaker_label(role)));
            self.role = Some(role.to_owned());
        }
    }
}

pub fn markdown(dossier: &ResearchDossier) -> String {
    let title = dossier.note["displayTitle"].as_str().unwrap_or("无标题");
    let mut out = format!(
        "# {}\n\n读取截止：{}\n\n",
        text::escape(title),
        dossier.as_of
    );
    if dossier.selection["mode"] == "explicit" {
        out.push_str("> 当前导出只包含本次选择的内容。\n\n");
    }
    let entries: BTreeMap<_, _> = dossier
        .materials
        .iter()
        .flat_map(|unit| {
            unit["blocks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(move |block| block["ref"].as_str().map(|r| (r, (unit, block))))
        })
        .collect();
    let mut review_notes = String::new();
    let mut seen_issues = BTreeSet::new();
    let mut seen_images = BTreeSet::new();
    for section in dossier.reading_view["sections"]
        .as_array()
        .into_iter()
        .flatten()
    {
        let mut attribution = Attribution::default();
        out.push_str(&format!(
            "## {}\n\n",
            section_label(section["kind"].as_str().unwrap_or(""))
        ));
        for group in section["groups"].as_array().into_iter().flatten() {
            let selected: Vec<_> = group["blockRefs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r.as_str().and_then(|r| entries.get(r).copied()))
                .collect();
            for (unit, _) in &selected {
                if unit["kind"] == "image" && seen_images.insert(unit["ref"].clone().to_string()) {
                    let label = unit["page"].as_u64().map_or_else(
                        || "图片 · 页码未确认".to_owned(),
                        |page| format!("第 {page} 张图片"),
                    );
                    out.push_str(&format!("### {label}\n\n"));
                    if let Some(url) = unit["originalUrl"].as_str() {
                        out.push_str(&format!(
                            "[查看原图](<{}>)\n\n",
                            url.replace('<', "%3C")
                                .replace('>', "%3E")
                                .replace('\n', "%0A")
                                .replace('\r', "%0D")
                        ));
                    } else {
                        out.push_str("原图尚未取得。\n\n");
                    }
                }
            }
            render_group(&mut out, dossier, group, &selected, &mut attribution);
            for (unit, block) in &selected {
                for issue in block["issues"].as_array().into_iter().flatten().chain(
                    unit["issues"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|issue| {
                            issue["blockRef"].is_null()
                                || group["blockRefs"]
                                    .as_array()
                                    .is_some_and(|refs| refs.contains(&issue["blockRef"]))
                        }),
                ) {
                    render_issue(
                        &mut review_notes,
                        issue,
                        unit["ref"].as_str().unwrap_or(""),
                        &review_location(unit, issue),
                        &mut seen_issues,
                    );
                }
            }
        }
    }
    render_gaps(&mut out, dossier);
    for issue in dossier.coverage["issues"].as_array().into_iter().flatten() {
        render_issue(
            &mut review_notes,
            issue,
            "coverage",
            "资料范围",
            &mut seen_issues,
        );
    }
    for unit in &dossier.materials {
        for issue in unit["issues"].as_array().into_iter().flatten() {
            render_issue(
                &mut review_notes,
                issue,
                unit["ref"].as_str().unwrap_or(""),
                &review_location(unit, issue),
                &mut seen_issues,
            );
        }
    }
    if !review_notes.is_empty() {
        out.push_str("## 核对说明\n\n");
        out.push_str(&review_notes);
    }
    out
}

fn section_label(kind: &str) -> &str {
    match kind {
        "publication" | "body" => "发布文案",
        "images" | "image" => "图片资料",
        "audio" => "视频口播",
        "frames" | "videoFrames" | "video_frame" => "画面文字",
        _ => "内容",
    }
}

fn render_group(
    out: &mut String,
    dossier: &ResearchDossier,
    group: &Value,
    entries: &[Entry<'_>],
    attribution: &mut Attribution,
) {
    let Some((unit, first)) = entries.first() else {
        return;
    };
    if group["continuation"]["fromPrevious"] == true {
        out.push_str("*承接上一段内容。*\n\n");
    }
    match group["mode"].as_str() {
        Some("heading" | "paragraph" | "transcript_paragraph") => {
            let mut text = String::new();
            for (index, (_, block)) in entries.iter().enumerate() {
                if index > 0 {
                    text.push_str(group["joiners"][index - 1].as_str().unwrap_or(""));
                }
                text.push_str(block["text"].as_str().unwrap_or(""));
            }
            let mut display = json!({"kind":if group["mode"] == "heading" {"heading"} else {"paragraph"},"text":text,"speaker":first["speaker"]});
            for key in ["contentType", "regionRef", "regionLabel"] {
                if let Some(value) = first.get(key) {
                    display[key] = value.clone();
                }
            }
            if let (Some(start), Some(end)) = (
                first["time"]["startMs"].as_i64(),
                entries
                    .last()
                    .and_then(|(_, b)| b["time"]["endMs"].as_i64()),
            ) {
                display["time"] = json!({"startMs":start,"endMs":end});
            }
            render_block(out, unit, &display, attribution);
        }
        Some("list" | "table") => {
            let mut display = (*first).clone();
            let key = if group["mode"] == "list" {
                "items"
            } else {
                "rows"
            };
            display[key] = json!(
                entries
                    .iter()
                    .flat_map(|(_, block)| block[key].as_array().into_iter().flatten())
                    .collect::<Vec<_>>()
            );
            render_block(out, unit, &display, attribution);
        }
        Some("tags") => {
            attribution.render(out, unit, first);
            render_tags(out, dossier, group);
        }
        _ => {
            for (unit, block) in entries {
                if unit["kind"] == "tags" && block["kind"] == "list" {
                    attribution.render(out, unit, block);
                    render_tags(out, dossier, &json!({"blockRefs":[block["ref"]]}));
                } else {
                    render_block(out, unit, block, attribution);
                }
            }
        }
    }
    if group["continuation"]["toNext"] == true {
        out.push_str("*后续内容不在本次范围内。*\n\n");
    }
}

fn render_tags(out: &mut String, dossier: &ResearchDossier, group: &Value) {
    let labels = super::super::reading::tags_for_group(dossier, group);
    out.push_str(
        &labels
            .iter()
            .map(|label| format!("#{}", text::escape(label)))
            .collect::<Vec<_>>()
            .join("　"),
    );
    out.push_str("\n\n");
}

fn render_block(out: &mut String, unit: &Value, block: &Value, attribution: &mut Attribution) {
    if let Some(label) = block["regionLabel"].as_str() {
        out.push_str(&format!("*区域：{}*\n\n", text::escape(label)));
    }
    match block["contentType"].as_str() {
        Some("visual_description") => out.push_str("*视觉描述（模型观察，非原文）*\n\n"),
        Some("extracted_text") => out.push_str("*图片文字（识别提取）*\n\n"),
        _ => attribution.render(out, unit, block),
    }
    if let (Some(start), Some(end)) = (
        block["time"]["startMs"].as_i64(),
        block["time"]["endMs"].as_i64(),
    ) {
        let range = if start == end {
            timestamp(start)
        } else {
            format!("{}–{}", timestamp(start), timestamp(end))
        };
        out.push_str(&format!("*{range}*\n\n"));
    }
    let text = text::block(block);
    match block["kind"].as_str() {
        Some("heading") => out.push_str(&format!(
            "{} {text}\n\n",
            "#".repeat((block["level"].as_u64().unwrap_or(1) + 2).min(6) as usize)
        )),
        Some("quote") => out.push_str(&format!("> {}\n\n", text.replace('\n', "\n> "))),
        Some("gap") => out.push_str(&format!(
            "{}\n\n",
            if text.starts_with('〔') {
                text
            } else {
                format!("〔{text}〕")
            }
        )),
        _ => {
            out.push_str(&text);
            out.push_str("\n\n");
        }
    }
}

fn timestamp(milliseconds: i64) -> String {
    let seconds = milliseconds / 1000;
    let base = format!("{:02}:{:02}", seconds / 60, seconds % 60);
    if milliseconds % 1000 == 0 {
        base
    } else {
        format!("{base}.{:03}", milliseconds % 1000)
    }
}

fn review_location(unit: &Value, issue: &Value) -> String {
    let mut label = if unit["kind"] == "image" {
        unit["page"].as_u64().map_or_else(
            || "图片 · 页码未确认".into(),
            |page| format!("第 {page} 张图片"),
        )
    } else {
        section_label(unit["kind"].as_str().unwrap_or("")).to_owned()
    };
    if let Some(region) = unit["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|block| issue["blockRef"].is_string() && block["ref"] == issue["blockRef"])
        .and_then(|block| block["regionLabel"].as_str())
    {
        label.push_str(&format!(" · {}", text::escape(region)));
    }
    if let Some(url) = unit["originalUrl"].as_str() {
        label.push_str(&format!(
            " · [查看原图](<{}>)",
            url.replace('<', "%3C")
                .replace('>', "%3E")
                .replace('\n', "%0A")
                .replace('\r', "%0D")
        ));
    }
    label
}

fn render_issue(
    out: &mut String,
    issue: &Value,
    scope: &str,
    location: &str,
    seen: &mut BTreeSet<String>,
) {
    if issue["code"] == "cloud_image_unchecked" {
        return;
    }
    let Some(message) = issue["message"].as_str() else {
        return;
    };
    let identity = issue["key"].as_str().unwrap_or(message);
    if seen.insert(format!("{scope}:{identity}")) {
        out.push_str(&format!(
            "> {location} · 待确认：{}\n\n",
            text::escape(message).replace('\n', "\n> ")
        ));
    }
}

fn render_gaps(out: &mut String, dossier: &ResearchDossier) {
    let mut lines = vec![];
    for (kind, label) in [
        ("body", "发布文案"),
        ("image", "图片文字"),
        ("audio", "视频口播"),
    ] {
        let units: Vec<_> = dossier
            .materials
            .iter()
            .filter(|u| u["kind"] == kind)
            .collect();
        if units.is_empty() {
            continue;
        }
        let missing: Vec<_> = units
            .iter()
            .filter(|u| {
                !matches!(
                    u["sourceState"].as_str(),
                    Some("known_empty" | "not_applicable")
                ) && !u["blocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(content::has_source_text)
            })
            .collect();
        if !missing.is_empty() {
            let state = if missing.iter().any(|u| u["processing"]["state"] == "failed") {
                "处理失败"
            } else if missing
                .iter()
                .any(|u| u["processing"]["state"] == "running")
            {
                "处理中"
            } else if missing.iter().any(|u| u["sourceState"] == "restricted") {
                "来源不可读取"
            } else if missing.iter().all(|u| u["sourceState"] == "not_acquired") {
                "来源未取得"
            } else {
                "暂无可用文字"
            };
            lines.push(format!("{label}：{state}。"));
        } else if kind == "audio" && units.iter().all(|u| u["sourceState"] == "known_empty") {
            lines.push("视频口播：无可用口播。".into());
        }
    }
    // Known missing pages affect reading continuity; unknown platform metadata
    // belongs to source inspection, not to the exported content.
    let missing: BTreeSet<_> = super::super::reading::image_text_gap_pages(dossier)
        .into_iter()
        .chain(
            dossier.reading_view["sections"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|s| s["gaps"].as_array().into_iter().flatten())
                .flat_map(|g| g["missingPages"].as_array().into_iter().flatten())
                .filter_map(Value::as_u64),
        )
        .collect();
    if !missing.is_empty() {
        let pages = format!(
            "图片第 {} 页文字待补全。",
            missing
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join("、")
        );
        if let Some(line) = lines.iter_mut().find(|line| line.starts_with("图片文字：")) {
            line.push_str(&pages);
        } else {
            lines.push(pages);
        }
    }
    if !lines.is_empty() {
        out.push_str("## 未包含内容\n\n");
        for line in lines {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod review_tests {
    use super::*;
    #[test]
    fn detached_notes_keep_image_and_region_attribution() {
        let issue = json!({"key":"blur", "blockRef":"b", "message":"末行数字看不清"});
        let unit = json!({"kind":"image", "page":null, "originalUrl":"/api/local/media/first", "blocks":[{"ref":"b", "regionLabel":"右栏"}]});
        let mut out = String::new();
        render_issue(
            &mut out,
            &issue,
            "first",
            &review_location(&unit, &issue),
            &mut BTreeSet::new(),
        );
        assert!(out.contains("图片 · 页码未确认 · 右栏 · [查看原图](</api/local/media/first>)"));
        assert!(out.contains("末行数字看不清"));
    }
}
