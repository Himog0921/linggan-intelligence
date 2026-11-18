//! Retirement compatibility contract: old task and input references remain readable
//! for historical batches. This does not expose or execute the retired analysis feature.
use linggan_intelligence::ai_foundation::{TaskKey, batch_store::InputRef};
use serde_json::json;

#[test]
fn historical_note_analysis_task_and_input_remain_deserializable() {
    let task: TaskKey = serde_json::from_str("\"note_interpretation\"").unwrap();
    assert_eq!(task.as_str(), "note_interpretation");

    let input: InputRef = serde_json::from_value(json!({
        "kind": "note_analysis",
        "requestRef": "00000000-0000-4000-8000-000000000001",
        "includeComments": true
    }))
    .unwrap();
    assert!(matches!(
        input,
        InputRef::NoteAnalysis {
            include_comments: true,
            ..
        }
    ));
}

#[test]
fn retired_note_analysis_no_longer_uses_the_automatic_brake_policy() {
    assert!(!TaskKey::NoteInterpretation.uses_automatic_brake());
    assert!(TaskKey::DirectionListDraft.uses_automatic_brake());
}
