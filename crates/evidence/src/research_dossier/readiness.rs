use super::{ResearchDossier, content};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialState {
    Ready,
    Preparing,
    Blocked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    ImageText,
    Video,
    Unknown,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialGap {
    pub code: String,
    pub state: MaterialState,
    pub message: String,
    pub slot_key: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialReadiness {
    pub state: MaterialState,
    pub kind: NoteKind,
    pub gaps: Vec<MaterialGap>,
    pub scope_note: Option<String>,
}

impl MaterialReadiness {
    pub fn can_start(&self) -> bool {
        self.state == MaterialState::Ready
    }
}

pub fn material_readiness(dossier: &ResearchDossier) -> MaterialReadiness {
    check(
        &dossier.materials,
        &dossier.media_inventory,
        dossier.coverage["imageInventory"]["expectedPages"].as_u64(),
    )
}

fn check(materials: &[Value], slots: &[Value], expected: Option<u64>) -> MaterialReadiness {
    let videos: Vec<_> = slots
        .iter()
        .filter(|s| s["purpose"] == "video" && s["component"] == "motion")
        .collect();
    let images: Vec<_> = slots
        .iter()
        .filter(|s| s["component"] == "image" && image_origins(s) > 0)
        .collect();
    let kind = if !videos.is_empty() {
        NoteKind::Video
    } else if !images.is_empty() || expected.is_some_and(|n| n > 0) {
        NoteKind::ImageText
    } else {
        NoteKind::Unknown
    };
    let mut result = MaterialReadiness {
        state: MaterialState::Ready,
        kind,
        gaps: vec![],
        scope_note: (kind == NoteKind::Video)
            .then(|| "本次依据口播与已提取的画面文字，未覆盖整段视频的全部画面。".into()),
    };
    if !materials.iter().any(|m| {
        m["kind"] == "body"
            && m["sourceRef"].is_string()
            && (m["sourceState"] == "known_empty" || (m["sourceState"] == "present" && has_text(m)))
    }) {
        result.gap(
            "native_text_unavailable",
            MaterialState::Blocked,
            "完整正文的取得状态尚未确认，不能将未取得当作原帖没有正文。",
            None,
        );
    }
    if kind == NoteKind::Unknown {
        result.gap(
            "media_inventory_unknown",
            MaterialState::Blocked,
            "原始媒体清单尚未确认，暂不能判断图文或视频材料是否齐全。",
            None,
        );
    }
    if kind == NoteKind::ImageText {
        let observed: u64 = images.iter().map(|s| image_origins(s)).sum();
        match expected {
            None => result.gap(
                "image_inventory_unknown",
                MaterialState::Blocked,
                "正文图片总数尚未确认，暂不能确认原图组齐全。",
                None,
            ),
            Some(count) if count != observed => result.gap(
                "image_inventory_incomplete",
                MaterialState::Blocked,
                "正文图片清单尚未齐全，封面不能代替正文原图。",
                None,
            ),
            _ => {}
        }
    }
    for image in images {
        result.original(image, "原图");
    }
    for video in videos {
        if !result.original(video, "原视频") {
            continue;
        }
        let mut prepared = true;
        let mut readable = false;
        for (kind, name) in [("audio", "口播"), ("video_frame", "画面文字")] {
            let material = materials
                .iter()
                .find(|m| m["kind"] == kind && m["slotKey"] == video["slotKey"]);
            if let Some(m) = material {
                let current = m["currentRevisionRef"].is_string()
                    && matches!(
                        m["sourceState"].as_str(),
                        Some("present" | "known_empty" | "not_applicable")
                    );
                if current {
                    readable |= has_text(m);
                    continue;
                }
            }
            prepared = false;
            let processing = material.map(|m| &m["processing"]).unwrap_or(&Value::Null);
            let (state, message) = match processing["state"].as_str() {
                Some("pending") => (MaterialState::Preparing, format!("{name}等待处理。")),
                Some("running") => (MaterialState::Preparing, format!("正在处理{name}。")),
                Some("failed") => (
                    MaterialState::Blocked,
                    format!("{name}处理失败，暂未生成视频内容理解。"),
                ),
                Some("cancelled") => (MaterialState::Blocked, format!("{name}处理已取消。")),
                Some("invalidated") => (MaterialState::Blocked, format!("{name}处理结果已失效。")),
                Some("completed") => (
                    MaterialState::Blocked,
                    format!("{name}处理已结束，但尚无当前可用读本。"),
                ),
                _ => (
                    MaterialState::Blocked,
                    format!("{name}尚未处理，暂未生成视频内容理解。"),
                ),
            };
            result.gap(
                &format!("{kind}_not_ready"),
                state,
                &message,
                video["slotKey"].as_str(),
            );
        }
        if prepared && !readable {
            result.gap("video_content_unavailable", MaterialState::Blocked, "视频材料已处理，但当前没有可用的口播或画面文字；现有能力不能据此理解无字幕的动作画面。", video["slotKey"].as_str());
        }
    }
    result
}

fn image_origins(slot: &Value) -> u64 {
    let is_body =
        |value: &Value| matches!(value["purpose"].as_str(), Some("body_image" | "live_photo"));
    u64::from(is_body(slot))
        + slot["sameBytesSources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|s| is_body(s))
            .count() as u64
}

fn has_text(material: &Value) -> bool {
    material["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .any(content::has_source_text)
}

impl MaterialReadiness {
    fn gap(&mut self, code: &str, state: MaterialState, message: &str, slot: Option<&str>) {
        if state == MaterialState::Blocked || self.state == MaterialState::Ready {
            self.state = state;
        }
        self.gaps.push(MaterialGap {
            code: code.into(),
            state,
            message: message.into(),
            slot_key: slot.map(str::to_owned),
        });
    }

    fn original(&mut self, slot: &Value, name: &str) -> bool {
        if slot["sourceState"] == "present"
            && slot["originalUrl"].is_string()
            && slot["localReadable"] == true
        {
            return true;
        }
        let (state, message) = if slot["sourceState"] == "restricted" {
            (MaterialState::Blocked, format!("{name}当前不可使用。"))
        } else if slot["sourceState"] == "present" {
            (
                MaterialState::Blocked,
                format!("{name}本地副本暂不可读取。"),
            )
        } else if slot["download"]["state"] == "running" {
            (MaterialState::Preparing, format!("正在取得{name}。"))
        } else {
            (
                MaterialState::Blocked,
                format!("{name}尚未取得，暂未生成整篇理解。"),
            )
        };
        self.gap(
            "original_unavailable",
            state,
            &message,
            slot["slotKey"].as_str(),
        );
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn body() -> Value {
        json!({"kind":"body","sourceRef":"native","sourceState":"known_empty","blocks":[]})
    }
    fn image() -> Value {
        json!({"slotKey":"image:1","component":"image","purpose":"body_image","sourceState":"present","originalUrl":"/local/image","localReadable":true})
    }
    fn video() -> Value {
        json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"present","originalUrl":"/local/video","localReadable":true})
    }
    fn reading(kind: &str) -> Value {
        json!({"kind":kind,"slotKey":"video:1","sourceState":"present","currentRevisionRef":"reading","blocks":[{"kind":"paragraph","text":"步骤"}]})
    }
    #[test]
    fn empty_native_body_and_images_without_ocr_are_ready_even_with_unknown_order() {
        let result = check(&[body()], &[image()], Some(1));
        assert!(result.can_start());
        assert_eq!(result.kind, NoteKind::ImageText);
        assert!(result.gaps.is_empty());
    }
    #[test]
    fn body_does_not_replace_unknown_or_incomplete_media() {
        assert!(!check(&[body()], &[], None).can_start());
        assert!(!check(&[body()], &[image()], None).can_start());
        assert!(!check(&[body()], &[image()], Some(2)).can_start());
        let mut cover = image();
        cover["purpose"] = json!("cover");
        assert!(!check(&[body()], &[cover], Some(1)).can_start());
    }
    #[test]
    fn original_missing_unreadable_restricted_and_downloading_remain_distinct() {
        let mut slot = image();
        slot["localReadable"] = json!(false);
        assert!(
            check(&[body()], &[slot.clone()], Some(1)).gaps[0]
                .message
                .contains("不可读取")
        );
        slot["sourceState"] = json!("restricted");
        assert!(
            check(&[body()], &[slot.clone()], Some(1)).gaps[0]
                .message
                .contains("不可使用")
        );
        slot["sourceState"] = json!("not_acquired");
        assert_eq!(
            check(&[body()], &[slot.clone()], Some(1)).state,
            MaterialState::Blocked
        );
        slot["download"] = json!({"state":"running"});
        assert_eq!(
            check(&[body()], &[slot], Some(1)).state,
            MaterialState::Preparing
        );
    }
    #[test]
    fn native_unknown_is_not_originally_empty() {
        let mut native = body();
        native["sourceRef"] = Value::Null;
        assert_eq!(
            check(&[native], &[image()], Some(1)).gaps[0].code,
            "native_text_unavailable"
        );
    }
    #[test]
    fn live_photo_motion_is_optional_and_same_byte_cover_alias_counts_as_a_body_image() {
        let mut still = image();
        still["purpose"] = json!("cover");
        still["sameBytesSources"] = json!([{"purpose":"live_photo"}]);
        let mut motion = video();
        motion["purpose"] = json!("live_photo");
        motion["localReadable"] = json!(false);
        assert!(check(&[body()], &[still, motion], Some(1)).can_start());
    }
    #[test]
    fn video_requires_both_applicable_readings_and_reports_pending_or_failure() {
        let audio = reading("audio");
        let mut frames = reading("video_frame");
        frames["currentRevisionRef"] = Value::Null;
        frames["processing"] = json!({"state":"running"});
        assert_eq!(
            check(&[body(), audio.clone(), frames.clone()], &[video()], None).state,
            MaterialState::Preparing
        );
        frames["processing"] = json!({"state":"failed"});
        assert_eq!(
            check(&[body(), audio, frames], &[video()], None).state,
            MaterialState::Blocked
        );
    }
    #[test]
    fn no_audio_no_speech_and_no_frame_text_are_valid_outcomes_not_failures() {
        for state in ["not_applicable", "known_empty"] {
            let mut audio = reading("audio");
            audio["sourceState"] = json!(state);
            audio["blocks"] = json!([]);
            let result = check(&[body(), audio, reading("video_frame")], &[video()], None);
            assert!(result.can_start());
            assert!(result.scope_note.unwrap().contains("未覆盖"));
        }
        let mut frames = reading("video_frame");
        frames["blocks"] = json!([{"kind":"gap","text":"未检出文字"}]);
        assert!(check(&[body(), reading("audio"), frames], &[video()], None).can_start());
    }
    #[test]
    fn empty_video_processing_does_not_grant_visual_understanding_from_its_caption() {
        let mut audio = reading("audio");
        audio["sourceState"] = json!("known_empty");
        audio["blocks"] = json!([]);
        let mut frames = reading("video_frame");
        frames["blocks"] = json!([{"kind":"gap","text":"未检出文字"}]);
        let result = check(&[body(), audio, frames], &[video()], None);
        assert_eq!(result.gaps[0].code, "video_content_unavailable");
    }
    #[test]
    fn failed_retry_keeps_valid_current_readings_and_no_cover_is_required_for_video() {
        let mut audio = reading("audio");
        audio["processing"] = json!({"state":"failed"});
        assert!(check(&[body(), audio, reading("video_frame")], &[video()], None).can_start());
        let mut restricted = video();
        restricted["sourceState"] = json!("restricted");
        assert!(
            !check(
                &[body(), reading("audio"), reading("video_frame")],
                &[restricted],
                None
            )
            .can_start()
        );
    }
}
