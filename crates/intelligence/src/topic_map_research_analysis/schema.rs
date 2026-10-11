use super::{EXTRACT_CONTRACT, MAX_DISCUSSIONS, RESOLVE_CONTRACT, STAGES, resolution::RELATIONS};
use serde_json::{Value, json};

fn object(properties: &[(&str, Value)]) -> Value {
    let fields: serde_json::Map<_, _> = properties
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.clone()))
        .collect();
    json!({"type":"object","properties":fields,
        "required":properties.iter().map(|(key,_)|*key).collect::<Vec<_>>(),
        "additionalProperties":false})
}
fn string(max: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max})
}
fn enumeration(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn array(items: Value, min: usize, max: usize) -> Value {
    json!({"type":"array","items":items,"minItems":min,"maxItems":max})
}
fn uuid() -> Value {
    json!({"type":"string","format":"uuid"})
}
fn evidence(min: usize) -> Value {
    array(
        object(&[
            ("fragmentId", string(500)),
            ("start", json!({"type":"integer","minimum":0})),
            ("end", json!({"type":"integer","minimum":1})),
        ]),
        min,
        12,
    )
}
fn concept_properties() -> Vec<(&'static str, Value)> {
    vec![
        ("label", string(120)),
        ("definition", string(1000)),
        ("inclusionCriteria", array(string(300), 1, 8)),
        ("exclusionCriteria", array(string(300), 1, 8)),
        ("domainFit", enumeration(&["in_scope"])),
        ("domainReason", string(500)),
        ("abstractionReason", string(500)),
    ]
}
fn proposed_topic() -> Value {
    let mut properties = concept_properties();
    properties.push((
        "parent",
        object(&[
            ("kind", enumeration(&["root", "existing", "proposed"])),
            ("topicRef", json!({"anyOf":[uuid(), {"type":"null"}]})),
            ("definitionRef", json!({"anyOf":[uuid(), {"type":"null"}]})),
            (
                "proposal",
                json!({"anyOf":[object(&concept_properties()), {"type":"null"}]}),
            ),
            ("reason", string(500)),
        ]),
    ));
    object(&properties)
}

fn discussion() -> Value {
    object(&[
        ("label", string(120)),
        ("statement", string(1000)),
        ("definition", string(1000)),
        ("inclusionCriteria", array(string(300), 1, 8)),
        ("exclusionCriteria", array(string(300), 1, 8)),
        (
            "speakerRole",
            enumeration(&["author", "commenter", "quoted", "unknown"]),
        ),
        (
            "evidenceRole",
            enumeration(&["support", "challenge", "context"]),
        ),
        ("rationale", string(500)),
        ("topicRef", json!({"type":"null"})),
        ("evidence", evidence(1)),
    ])
}

/// The same complete contract is sent to the model and displayed in method details.
pub fn output_schema() -> Value {
    object(&[
        ("contract", enumeration(&[EXTRACT_CONTRACT])),
        (
            "outcome",
            enumeration(&["analyzed", "no_signal", "insufficient"]),
        ),
        ("discussions", array(discussion(), 0, MAX_DISCUSSIONS)),
        (
            "scenes",
            array(
                object(&[("label", string(300)), ("evidence", evidence(1))]),
                0,
                8,
            ),
        ),
        (
            "journey",
            object(&[
                ("mainStage", enumeration(&STAGES)),
                ("involvedStages", array(enumeration(&STAGES[..5]), 0, 5)),
                (
                    "overlays",
                    array(
                        enumeration(&["obstruction_recurrence", "transition_handoff"]),
                        0,
                        2,
                    ),
                ),
                ("path", enumeration(&["family", "adult", "both", "unknown"])),
                ("rationale", string(500)),
                ("evidence", evidence(0)),
            ]),
        ),
        (
            "responseMatches",
            array(
                object(&[
                    (
                        "status",
                        enumeration(&[
                            "direct",
                            "partial",
                            "not_applicable",
                            "unmatched",
                            "unknown",
                        ]),
                    ),
                    ("unanswered", array(string(300), 0, 8)),
                    ("evidence", evidence(0)),
                ]),
                0,
                10,
            ),
        ),
        (
            "angles",
            array(
                object(&[
                    ("label", string(120)),
                    ("title", string(120)),
                    ("answerTask", string(500)),
                    ("evidence", evidence(1)),
                ]),
                0,
                5,
            ),
        ),
        (
            "productOpportunities",
            array(
                object(&[
                    ("need", string(500)),
                    ("hypothesis", string(500)),
                    ("verificationQuestion", string(500)),
                    ("alternativeExplanation", string(500)),
                    ("evidence", evidence(1)),
                ]),
                0,
                5,
            ),
        ),
        ("limitations", array(string(500), 0, 12)),
    ])
}

pub fn resolution_schema() -> Value {
    object(&[
        ("contract", enumeration(&[RESOLVE_CONTRACT])),
        (
            "decisions",
            array(
                object(&[
                    ("unitId", string(200)),
                    (
                        "status",
                        enumeration(&["matched", "new", "uncertain", "out_of_scope"]),
                    ),
                    (
                        "matches",
                        array(
                            object(&[
                                ("topicRef", uuid()),
                                ("definitionRef", uuid()),
                                ("reason", string(500)),
                            ]),
                            0,
                            4,
                        ),
                    ),
                    (
                        "proposedTopic",
                        json!({"anyOf":[proposed_topic(),{"type":"null"}]}),
                    ),
                    (
                        "relations",
                        array(
                            object(&[
                                ("topicRef", uuid()),
                                ("definitionRef", uuid()),
                                ("relation", enumeration(&RELATIONS)),
                                ("reason", string(500)),
                            ]),
                            0,
                            64,
                        ),
                    ),
                    ("reason", string(500)),
                ]),
                0,
                MAX_DISCUSSIONS,
            ),
        ),
    ])
}
