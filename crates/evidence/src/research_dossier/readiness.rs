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
    check(&dossier.materials, &dossier.media_inventory, false, false)
}

/// L1 needs the video's spoken content even when other material is readable.
pub fn l1_material_readiness(dossier: &ResearchDossier) -> MaterialReadiness {
    check(
        &dossier.materials,
        &dossier.media_inventory,
        dossier.note["sourceContentType"] == "video",
        true,
    )
}

fn check(
    materials: &[Value],
    slots: &[Value],
    source_is_video: bool,
    require_transcript: bool,
) -> MaterialReadiness {
    let has_video = slots
        .iter()
        .any(|s| s["purpose"] == "video" && s["component"] == "motion");
    let has_image = slots.iter().any(|s| s["component"] == "image");
    let kind = if has_video || source_is_video {
        NoteKind::Video
    } else if has_image {
        NoteKind::ImageText
    } else {
        NoteKind::Unknown
    };
    let readable_text = materials.iter().any(|m| {
        (m["kind"] == "body"
            || (m["kind"] == "audio"
                && m["currentRevisionRef"].is_string()
                && !slots
                    .iter()
                    .any(|s| s["slotKey"] == m["slotKey"] && s["sourceState"] == "restricted")))
            && m["sourceState"] == "present"
            && has_text(m)
    });
    let readable_image = slots.iter().any(|s| {
        s["component"] == "image"
            && s["sourceState"] == "present"
            && s["originalUrl"].is_string()
            && s["localReadable"] == true
    });
    let mut result = MaterialReadiness {
        state: MaterialState::Ready,
        kind,
        gaps: vec![],
        scope_note: (kind == NoteKind::Video)
            .then(|| "仅分析已取得的文字和图片，不包含视频画面内容。".into()),
    };
    // For video, a caption or cover alone cannot represent the spoken content.
    // Every video slot must have a current, nonempty transcript for that slot.
    for video in slots
        .iter()
        .filter(|s| require_transcript && s["purpose"] == "video" && s["component"] == "motion")
    {
        let transcript_available = materials.iter().any(|m| {
            m["kind"] == "audio"
                && m["slotKey"] == video["slotKey"]
                && video["sourceState"] != "restricted"
                && m["sourceState"] == "present"
                && m["currentRevisionRef"].is_string()
                && m["sourceRef"]
                    .as_str()
                    .and_then(|value| value.parse::<uuid::Uuid>().ok())
                    .is_some()
                && m["sourceVersion"].is_string()
                && has_referenceable_text(m)
        });
        if !transcript_available {
            let (state, message) = video_transcript_gap(video);
            result.gap(
                "video_transcript_required",
                state,
                message,
                video["slotKey"].as_str(),
            );
        }
    }
    if require_transcript && source_is_video && !has_video {
        result.gap(
            "video_transcript_required",
            MaterialState::Blocked,
            "视频来源尚未取得有效转录，完成转录后才能进行内容理解。",
            None,
        );
    }
    if !readable_text && !readable_image {
        let state = if result.state == MaterialState::Preparing {
            MaterialState::Preparing
        } else {
            MaterialState::Blocked
        };
        result.gap(
            "no_readable_material",
            state,
            "尚无可供分析的正文、口播或图片。",
            None,
        );
    }
    result
}

fn video_transcript_gap(video: &Value) -> (MaterialState, &'static str) {
    use MaterialState::{Blocked, Preparing};
    if video["sourceState"] == "restricted" {
        return (Blocked, "视频来源受限，不能用于内容理解。");
    }
    let asr = &video["processing"]["asr"];
    let audio = &video["processing"]["audio_extract"];
    if matches!(
        asr["reason"].as_str().or(audio["reason"].as_str()),
        Some("no_speech_observed" | "no_audio_stream_observed")
    ) {
        return (Blocked, "处理已结束，未取得可用口播，不能启动 L1。");
    }
    if [asr, audio].iter().any(|job| {
        matches!(job["queueState"].as_str(), Some("pending" | "leased" | "retry_wait"))
            || (job["queueState"].is_null()
                && matches!(job["state"].as_str(), Some("pending" | "running")))
    }) {
        return (Preparing, "音频提取或转录任务正在队列中，生成有效转录后会自动重新检查。");
    }
    match video["download"]["state"].as_str() {
        Some("running") => (Preparing, "视频正在下载，取得原件后将进入媒体处理。"),
        Some("expired_url" | "mime_mismatch" | "size_limit" | "network_error")
            if video["sourceState"] == "not_acquired" =>
        {
            (Blocked, "视频下载未取得合格原件，请查看下载记录中的失败原因。")
        }
        _ => (Blocked, "视频口播尚未取得有效转录，完成转录后才能进行内容理解。"),
    }
}

fn has_referenceable_text(material: &Value) -> bool {
    material["blocks"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|block| block["ref"].as_str().is_some() && content::has_source_text(block))
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn text() -> Value {
        json!({"kind":"body","sourceRef":"native","sourceState":"present","blocks":[{"kind":"paragraph","text":"可分析的正文"}]})
    }
    fn image() -> Value {
        json!({"slotKey":"image:1","component":"image","purpose":"body_image","sourceState":"present","originalUrl":"/local/image","localReadable":true})
    }

    #[test]
    fn l1_waits_for_scheduled_media_retry_and_leaves_empty_transcripts_terminal() {
        let mut video = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"present",
            "processing":{"asr":{"state":"failed","queueState":"retry_wait","reason":"process_timeout"}}});
        let readiness = check(&[], &[video.clone()], true, true);
        assert_eq!(readiness.state, MaterialState::Preparing);
        assert!(readiness.gaps.iter().all(|gap| gap.state == MaterialState::Preparing));
        video["processing"]["asr"] = json!({"state":"completed","queueState":"completed","reason":"no_speech_observed"});
        let readiness = check(&[text()], &[video], true, true);
        assert_eq!(readiness.state, MaterialState::Blocked);
        assert!(readiness.gaps[0].message.contains("处理已结束"));
    }
    #[test]
    fn existing_text_or_image_is_usable_without_complete_inventory() {
        assert!(check(&[text()], &[], false, true).can_start());
        assert!(check(&[], &[image()], false, true).can_start());
        assert!(check(&[text()], &[image()], false, true).can_start());
    }
    #[test]
    fn missing_or_restricted_content_is_not_usable() {
        assert!(!check(&[], &[], false, true).can_start());
        let mut missing = image();
        missing["localReadable"] = json!(false);
        assert!(!check(&[], &[missing], false, true).can_start());
        let mut restricted = image();
        restricted["sourceState"] = json!("restricted");
        assert!(!check(&[], &[restricted], false, true).can_start());
    }
    #[test]
    fn video_requires_transcript_even_with_caption_and_cover() {
        let video = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"present"});
        let result = check(&[text()], std::slice::from_ref(&video), false, true);
        assert!(!result.can_start());
        assert!(check(&[text()], std::slice::from_ref(&video), false, false).can_start());
        assert_eq!(result.gaps[0].code, "video_transcript_required");
        assert!(result.scope_note.unwrap().contains("不包含视频画面"));
        let audio = json!({"kind":"audio","slotKey":"video:1","sourceState":"present","currentRevisionRef":"r","sourceRef":"00000000-0000-4000-8000-000000000001","sourceVersion":"v1","blocks":[{"ref":"text-1","kind":"paragraph","text":"有效的口播转录"}]});
        assert!(check(&[text(), audio], &[video], false, true).can_start());
    }
    #[test]
    fn captured_video_type_blocks_when_video_slot_is_missing() {
        let result = check(&[text()], &[], true, true);
        assert_eq!(result.kind, NoteKind::Video);
        assert!(!result.can_start());
        assert_eq!(result.gaps[0].code, "video_transcript_required");
        assert!(check(&[text()], &[], false, true).can_start());
    }

    #[test]
    fn each_video_requires_its_own_nonempty_current_transcript() {
        let first = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"present"});
        let second = json!({"slotKey":"video:2","component":"motion","purpose":"video","sourceState":"present"});
        let audio = json!({"kind":"audio","slotKey":"video:1","sourceState":"present","currentRevisionRef":"r","sourceRef":"00000000-0000-4000-8000-000000000001","sourceVersion":"v1","blocks":[{"ref":"text-1","kind":"paragraph","text":"有效转录"}]});
        let result = check(
            &[text(), audio.clone()],
            &[first.clone(), second],
            false,
            true,
        );
        assert!(!result.can_start());
        assert_eq!(result.gaps[0].slot_key.as_deref(), Some("video:2"));
        let mut unreferenced = audio.clone();
        unreferenced["blocks"][0]
            .as_object_mut()
            .unwrap()
            .remove("ref");
        assert!(
            !check(
                &[text(), unreferenced],
                std::slice::from_ref(&first),
                false,
                true
            )
            .can_start()
        );
        let mut empty = audio;
        empty["blocks"] = json!([]);
        assert!(!check(&[text(), empty], &[first], false, true).can_start());
    }

    #[test]
    fn restricted_video_does_not_authorize_its_speech() {
        let video = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"restricted"});
        let audio = json!({"kind":"audio","slotKey":"video:1","sourceState":"present","currentRevisionRef":"r","sourceRef":"00000000-0000-4000-8000-000000000001","sourceVersion":"v1","blocks":[{"ref":"text-1","kind":"paragraph","text":"文字"}]});
        assert!(!check(&[audio], &[video], false, true).can_start());
    }
}
