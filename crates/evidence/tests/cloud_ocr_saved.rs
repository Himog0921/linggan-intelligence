//! Private replay only. The manifest and real originals remain outside Git and CI.
use linggan_evidence::{cloud_ocr_adoption::blocks_from_visual, ocr_adoption::content_hash};
use serde_json::{Value, json};
use std::path::Path;
#[test]
#[ignore = "requires explicitly supplied private saved-response manifest; zero model calls"]
fn saved_cloud_responses_replay() {
    let manifest =
        std::env::var("OCR_SAVED_REPLAY_MANIFEST").expect("private manifest path required");
    let data: Value = serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    let entries = data["images"].as_array().unwrap();
    assert!(!entries.is_empty());
    let mut results = vec![];
    for entry in entries {
        let read = |key: &str| std::fs::read(entry[key].as_str().unwrap()).unwrap();
        let image = read("original");
        assert_eq!(content_hash(&image), entry["imageSha256"]);
        let raw = read("cloudJson");
        assert_eq!(content_hash(&raw), entry["cloudJsonSha256"]);
        let visual: Value = serde_json::from_slice(&raw).unwrap();
        let projection = blocks_from_visual(&visual).unwrap();
        for region in visual["regions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| r["legibility"] == "clear")
        {
            let original = region["text"].as_str().unwrap();
            let rendered = projection
                .iter()
                .map(linggan_evidence::research_dossier::content::block_text)
                .collect::<Vec<_>>()
                .join("\n\n");
            for paragraph in original.split("\n\n").filter(|t| !t.trim().is_empty()) {
                assert!(rendered.contains(paragraph));
            }
        }
        let pending = projection
            .iter()
            .filter(|b| b["issues"].as_array().is_some_and(|i| !i.is_empty()))
            .count();
        results.push(json!({"id":entry["id"],"projection":projection,"rawPreserved":true,"pendingSpans":pending}));
    }
    let output = Path::new(data["output"].as_str().unwrap());
    assert!(output.parent().unwrap().is_dir());
    std::fs::write(
        output,
        serde_json::to_vec_pretty(&json!({"newProviderCalls":0,"results":results})).unwrap(),
    )
    .unwrap();
    println!(
        "saved cloud response projection replay PASS: {} images; zero new provider calls",
        entries.len()
    );
}
