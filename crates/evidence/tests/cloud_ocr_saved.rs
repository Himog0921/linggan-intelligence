//! Private replay only. The manifest and real originals remain outside Git and CI.
use linggan_evidence::{
    cloud_ocr_adoption::{CloudSource, project},
    ocr_adoption::{Assessment, ImageInput, content_hash},
};
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
        let local: Value = serde_json::from_slice(&read("localResult")).unwrap();
        let input: ImageInput = serde_json::from_value(local["observation"].clone()).unwrap();
        let assessment: Assessment = serde_json::from_value(local["assessment"].clone()).unwrap();
        let source: CloudSource = serde_json::from_value(entry["source"].clone()).unwrap();
        let projection = project(&source, &visual, Some((&input, &assessment))).unwrap();
        let regions = projection["regions"].as_array().unwrap();
        assert_eq!(regions.len(), visual["regions"].as_array().unwrap().len());
        for (a, b) in regions.iter().zip(visual["regions"].as_array().unwrap()) {
            assert_eq!(a["text"], b["text"]);
            assert_eq!(a["bbox"], b["bbox"]);
        }
        if entry["expectedTitle"].is_string() {
            assert_eq!(projection["coverTitle"], entry["expectedTitle"]);
        }
        if entry["expectNoWorkTitle"] == true {
            assert!(projection["coverTitle"].is_null());
        }
        let pending = regions
            .iter()
            .flat_map(|r| r["spans"].as_array().unwrap())
            .filter(|s| s["status"] == "pending")
            .count();
        if entry["requirePending"] == true {
            assert!(pending > 0, "saved disagreement lost");
        }
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
