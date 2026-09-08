//! Safe compatibility diagnostics for the fixed synthetic probe. This is not model admission.
use crate::comment_packet::synthetic_packet;
use serde_json::{Value, json};

pub(super) fn evaluate(text: &str) -> (bool, Option<&'static str>, Value) {
    let packet = synthetic_packet();
    let diagnostics = packet.validation_diagnostics(text);
    let parsed = packet
        .parse(text)
        .and_then(|items| items.into_iter().next().ok_or("missing_comment")?);
    let (status, code, accepted, rejected, uncertain) = match parsed {
        Ok(value) => {
            let semantic = &value["semantic"];
            let count = |key: &str| semantic[key].as_array().map_or(0, Vec::len);
            let accepted = count("labels") + count("problems") + count("stances");
            let rejected = count("rejectedFields");
            (
                if rejected == 0 { "passed" } else { "partial" },
                if rejected == 0 {
                    None
                } else {
                    Some("partial_fields_rejected")
                },
                Some(accepted),
                Some(rejected),
                Some(count("uncertainFields")),
            )
        }
        Err(code) => (
            "failed",
            Some(code),
            (code == "all_fields_rejected").then_some(0),
            (code == "all_fields_rejected").then_some(diagnostics.len()),
            None,
        ),
    };
    (
        status == "passed",
        code,
        json!({"status":status,"acceptedFields":accepted,"rejectedFields":rejected,
            "uncertainFields":uncertain,"diagnosticsRecorded":true,"diagnostics":diagnostics}),
    )
}

pub(super) fn not_evaluated() -> Value {
    json!({"status":"not_evaluated","acceptedFields":null,"rejectedFields":null,
        "uncertainFields":null,"diagnosticsRecorded":true,"diagnostics":[]})
}

#[cfg(test)]
mod tests {
    use super::*;
    fn output() -> Value {
        let text = synthetic_packet().cleaned[0].text.clone();
        json!({"comments":[{"commentRef":"C001","outcome":"interpretable",
            "labels":[{"label":"need","basis":"explicit","contextEvidence":[],"evidence":[{"quote":text}]}],
            "problems":[],"stances":[],"contextMissing":[],"uncertaintyReason":null,"limitations":[]}]})
    }
    #[test]
    fn partial_probe_retains_counts_and_precise_safe_diagnostics() {
        let mut output = output();
        let mut invalid = output["comments"][0]["labels"][0].clone();
        invalid["evidence"][0]["quote"] = json!("PRIVATE_PROVIDER_TEXT_NOT_IN_SOURCE");
        output["comments"][0]["labels"]
            .as_array_mut()
            .unwrap()
            .push(invalid);
        let (qualified, code, report) = evaluate(&output.to_string());
        assert!(!qualified);
        assert_eq!(code, Some("partial_fields_rejected"));
        assert_eq!(report["status"], "partial");
        assert_eq!(report["acceptedFields"], 1);
        assert_eq!(report["rejectedFields"], 1);
        assert_eq!(
            report["diagnostics"][0]["path"],
            "$.comments[0].labels[1].evidence[0].quote"
        );
        assert_eq!(
            report["diagnostics"][0]["code"],
            "quote_missing_or_ambiguous"
        );
        assert!(!report.to_string().contains("PRIVATE_PROVIDER_TEXT"));
        // Research still accepts the independent valid field, not the failed quote.
        let parsed = synthetic_packet()
            .parse(&output.to_string())
            .unwrap()
            .remove(0)
            .unwrap();
        assert_eq!(parsed["semantic"]["labels"].as_array().unwrap().len(), 1);
    }
    #[test]
    fn complete_and_malformed_outputs_have_distinct_truthful_reports() {
        let (qualified, code, report) = evaluate(&output().to_string());
        assert!(qualified);
        assert_eq!(code, None);
        assert_eq!(report["status"], "passed");
        assert_eq!(report["rejectedFields"], 0);
        let (_, code, report) = evaluate("PRIVATE_INVALID_JSON");
        assert_eq!(code, Some("json_invalid"));
        assert_eq!(report["status"], "failed");
        assert!(report["acceptedFields"].is_null());
        assert!(!report.to_string().contains("PRIVATE_INVALID_JSON"));
    }
    #[test]
    fn all_rejected_fields_and_unknown_properties_do_not_leak_values() {
        let mut output = output();
        output["comments"][0]["labels"][0]["label"] = json!("PRIVATE_INVALID_LABEL");
        let (_, code, report) = evaluate(&output.to_string());
        assert_eq!(code, Some("all_fields_rejected"));
        assert_eq!(report["acceptedFields"], 0);
        assert_eq!(report["rejectedFields"], 1);
        assert_eq!(
            report["diagnostics"][0]["path"],
            "$.comments[0].labels[0].label"
        );
        assert!(!report.to_string().contains("PRIVATE_INVALID_LABEL"));
        output["PRIVATE_UNKNOWN_PROPERTY"] = json!("PRIVATE_UNKNOWN_VALUE");
        let (_, _, report) = evaluate(&output.to_string());
        assert!(!report.to_string().contains("PRIVATE_UNKNOWN"));
        assert_eq!(not_evaluated()["status"], "not_evaluated");
    }
}
