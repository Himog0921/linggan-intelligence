//! Offline current prompt/schema export and provider response replay. No database or network.
use linggan_intelligence::ocr_recognition_schema as visual;
use serde_json::json;
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("prompt") {
        println!(
            "{}",
            json!({"contract":visual::CONTRACT,"prompt":visual::prompt_for_image("完整提取可见材料，保持对象和上下文归属，返回规定JSON。",uuid::Uuid::nil(),None),"schema":visual::response_schema().schema})
        );
        return Ok(());
    }
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    if std::env::args().nth(1).as_deref() == Some("dossier") {
        use linggan_evidence::research_dossier::{
            ResearchDossier, Selection, VERSION, markdown, select,
        };
        let v: serde_json::Value = serde_json::from_str(&input)?;
        let mut dossier = ResearchDossier {
            contract: VERSION,
            work_ref: serde_json::from_value(v["workRef"].clone())?,
            revision: v["revision"].as_str().unwrap_or("offline").into(),
            material_revision: v["materialRevision"].as_str().unwrap_or("offline").into(),
            as_of: v["asOf"].as_str().unwrap_or("").into(),
            note: v["note"].clone(),
            materials: serde_json::from_value(v["materials"].clone())?,
            reading_view: v["readingView"].clone(),
            coverage: v["coverage"].clone(),
            attachments: v["attachments"].clone(),
            selection: v["selection"].clone(),
            fragments: vec![],
            media: vec![],
            media_inventory: vec![],
            candidate_statuses: vec![],
            metadata: json!({}),
        };
        select(
            &mut dossier,
            &Selection {
                kinds: vec![
                    "body".into(),
                    "tags".into(),
                    "image".into(),
                    "audio".into(),
                    "video_frame".into(),
                ],
                ..Default::default()
            },
        )?;
        println!(
            "{}",
            json!({"markdown":markdown(&dossier),"dossier":dossier})
        );
        return Ok(());
    }
    let parsed = visual::parse_visual_diagnostic(&input, Some("stop"))?;
    let value = serde_json::to_value(parsed)?;
    let blocks = linggan_evidence::cloud_ocr_adoption::blocks_from_visual(&value)?;
    let issues = linggan_evidence::cloud_ocr_adoption::issues_from_visual(&value)?;
    println!(
        "{}",
        json!({"visual":value,"blocks":blocks,"issues":issues})
    );
    Ok(())
}
