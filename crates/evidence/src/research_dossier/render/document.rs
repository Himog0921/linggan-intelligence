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
        if self.role.as_deref() != Some(role) {
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
    let mut seen_issues = BTreeSet::new();
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
            for gap in section["gaps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|gap| gap["beforeGroupRef"] == group["ref"])
            {
                render_gap(&mut out, gap);
            }
            let selected: Vec<_> = group["blockRefs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|r| r.as_str().and_then(|r| entries.get(r).copied()))
                .collect();
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
                        &mut out,
                        issue,
                        unit["ref"].as_str().unwrap_or(""),
                        &mut seen_issues,
                    );
                }
            }
        }
        for gap in section["gaps"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|gap| gap["beforeGroupRef"].is_null())
        {
            render_gap(&mut out, gap);
        }
    }
    render_gaps(&mut out, dossier);
    for issue in dossier.coverage["issues"].as_array().into_iter().flatten() {
        render_issue(&mut out, issue, "coverage", &mut seen_issues);
    }
    for unit in &dossier.materials {
        for issue in unit["issues"].as_array().into_iter().flatten() {
            render_issue(
                &mut out,
                issue,
                unit["ref"].as_str().unwrap_or(""),
                &mut seen_issues,
            );
        }
    }
    out
}

fn section_label(kind: &str) -> &str {
    match kind {
        "publication" | "body" => "发布文案",
        "images" | "image" => "图片文字",
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
        Some("paragraph" | "transcript_paragraph") => {
            let mut text = String::new();
            for (index, (_, block)) in entries.iter().enumerate() {
                if index > 0 {
                    text.push_str(group["joiners"][index - 1].as_str().unwrap_or(""));
                }
                text.push_str(block["text"].as_str().unwrap_or(""));
            }
            let mut display = json!({"kind":"paragraph","text":text,"speaker":first["speaker"]});
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
    attribution.render(out, unit, block);
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

fn render_issue(out: &mut String, issue: &Value, scope: &str, seen: &mut BTreeSet<String>) {
    let Some(message) = issue["message"].as_str() else {
        return;
    };
    let identity = issue["key"].as_str().unwrap_or(message);
    if seen.insert(format!("{scope}:{identity}")) {
        out.push_str(&format!(
            "> 待确认：{}\n\n",
            text::escape(message).replace('\n', "\n> ")
        ));
    }
}

fn render_gap(out: &mut String, gap: &Value) {
    let pages = gap["missingPages"]
        .as_array()
        .into_iter()
        .flatten()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("、");
    out.push_str(&format!(
        "> 图片第 {pages} 页文字待补全，此处内容不完整。\n\n"
    ));
}

fn render_gaps(out: &mut String, dossier: &ResearchDossier) {
    let mut lines = vec![];
    for kind in ["body", "images", "audio", "videoFrames"] {
        let label = match dossier.coverage["sources"][kind]["state"].as_str() {
            Some("not_acquired") => "来源尚未取得，不能判断原作是否没有内容。",
            Some("not_processed") => "已有来源，尚无当前可用的识读文字。",
            Some("restricted") => "当前来源不可读取。",
            Some("partial") => "当前范围尚不完整，已取得的文字照常保留。",
            Some("known_empty") => "来源已确认无可读文字。",
            Some("not_applicable") => "不适用。",
            _ => continue,
        };
        lines.push(format!("{}：{label}", section_label(kind)));
    }
    let inventory = &dossier.coverage["imageInventory"];
    let positioned: BTreeSet<_> = dossier.reading_view["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|s| s["gaps"].as_array().into_iter().flatten())
        .flat_map(|g| g["missingPages"].as_array().into_iter().flatten())
        .filter_map(Value::as_u64)
        .collect();
    let missing: Vec<_> = super::super::reading::image_text_gap_pages(dossier)
        .into_iter()
        .filter(|p| !positioned.contains(p))
        .collect();
    if !missing.is_empty() {
        lines.push(format!(
            "图片第 {} 页文字待补全。",
            missing
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join("、")
        ));
    }
    if inventory["unknownOrder"] == true {
        lines.push("图片原始顺序未确认，当前保留已观察顺序。".into());
    }
    if inventory["expectedPages"].is_null()
        && inventory["acquiredPages"].as_u64().is_some_and(|n| n > 0)
    {
        lines.push("原作图片总页数未知。".into());
    }
    for unit in &dossier.materials {
        if let Some(detail) = processing_note(unit) {
            let label = section_label(unit["kind"].as_str().unwrap_or(""));
            let page = unit["page"]
                .as_u64()
                .map_or(String::new(), |n| format!("第 {n} 页"));
            lines.push(format!("{label}{page}：{detail}"));
        }
    }
    if !lines.is_empty() {
        out.push_str("## 内容范围与待处理\n\n");
        for line in lines {
            out.push_str(&format!("- {line}\n"));
        }
        out.push('\n');
    }
}

fn processing_note(unit: &Value) -> Option<String> {
    if !matches!(
        unit["sourceState"].as_str(),
        Some("present" | "known_empty")
    ) {
        return None;
    }
    let label = match unit["processing"]["state"].as_str() {
        Some("pending") => "待处理",
        Some("running") => "处理中",
        Some("failed") => "处理失败",
        Some("cancelled") => "处理已取消",
        Some("invalidated") => "处理结果已失效",
        _ => return None,
    };
    if unit["sourceState"] == "known_empty" && unit["currentRevisionRef"].is_string() {
        return Some(format!("{label}；既有已确认无文字的结果仍有效。"));
    }
    let has_blocks = unit["blocks"]
        .as_array()
        .is_some_and(|blocks| !blocks.is_empty());
    if !has_blocks
        && unit["issues"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|issue| issue["code"] == "existing_result_pending")
    {
        return Some(
            if unit["kind"] == "image" {
                "已有识别结果，待整理。"
            } else {
                "已有处理结果，待接入。"
            }
            .into(),
        );
    }
    Some(format!(
        "{label}{}",
        if has_blocks {
            "，已有读本仍可读。"
        } else {
            "。"
        }
    ))
}
