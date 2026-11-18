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
    check(&dossier.materials, &dossier.media_inventory, None)
}

fn check(materials: &[Value], slots: &[Value], _expected: Option<u64>) -> MaterialReadiness {
    let has_video = slots
        .iter()
        .any(|s| s["purpose"] == "video" && s["component"] == "motion");
    let has_image = slots.iter().any(|s| s["component"] == "image");
    let kind = if has_video {
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
    if !readable_text && !readable_image {
        result.gap(
            "no_readable_material",
            MaterialState::Blocked,
            "尚无可供分析的正文、口播或图片。",
            None,
        );
    }
    result
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
    fn existing_text_or_image_is_usable_without_complete_inventory() {
        assert!(check(&[text()], &[], None).can_start());
        assert!(check(&[], &[image()], None).can_start());
        assert!(check(&[text()], &[image()], Some(20)).can_start());
    }
    #[test]
    fn missing_or_restricted_content_is_not_usable() {
        assert!(!check(&[], &[], None).can_start());
        let mut missing = image();
        missing["localReadable"] = json!(false);
        assert!(!check(&[], &[missing], None).can_start());
        let mut restricted = image();
        restricted["sourceState"] = json!("restricted");
        assert!(!check(&[], &[restricted], None).can_start());
    }
    #[test]
    fn video_can_use_caption_but_does_not_claim_video_understanding() {
        let video = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"present"});
        let result = check(&[text()], &[video], None);
        assert!(result.can_start());
        assert!(result.scope_note.unwrap().contains("不包含视频画面"));
    }
    #[test]
    fn restricted_video_does_not_authorize_its_speech() {
        let video = json!({"slotKey":"video:1","component":"motion","purpose":"video","sourceState":"restricted"});
        let audio = json!({"kind":"audio","slotKey":"video:1","sourceState":"present","currentRevisionRef":"r","blocks":[{"kind":"paragraph","text":"文字"}]});
        assert!(!check(&[audio], &[video], None).can_start());
    }
}
