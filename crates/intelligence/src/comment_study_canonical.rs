//! The one text template recall encodes.
//!
//! A Signal and a Problem core are only comparable if they were written as the same kind of
//! sentence, so both are built here and nowhere else. Two encoders that drifted apart would still
//! produce 512 numbers each and still rank them against one another — the failure would be silent.
//!
//! Code owns this template rather than the model: letting a round decide its own embedding text
//! makes distances incomparable between rounds. Nothing volatile belongs in it — not member
//! counts, not dates, not titles — because a Problem's core vector must stay fixed while its
//! support grows.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Bump when the template or its field extraction changes: distances across revisions of this
/// function are not comparable, and the profile records which revision produced a vector.
pub const PREPROCESSING_REVISION: &str = "comment-study.canonical.v1";

const UNSPECIFIED: &str = "未明确";

/// Builds the canonical text for one framed expression.
///
/// `frame` is the accepted `problemFrame`: `actor`, `goalOrExpectedState`, `barrierOrUnmetNeed`
/// and `context`, each `{value, basis}`. A field the commenter never expressed stays `未明确`
/// rather than being filled in from the domain or from a neighbouring field — an invented goal
/// would make two different problems encode alike.
pub fn canonical_text(proposition: &str, frame: Option<&Value>) -> String {
    format!(
        "表达：{}\n主体：{}\n目标：{}\n障碍：{}\n场景：{}",
        proposition.trim(),
        framed_field(frame, "actor"),
        framed_field(frame, "goalOrExpectedState"),
        framed_field(frame, "barrierOrUnmetNeed"),
        framed_field(frame, "context"),
    )
}

pub fn canonical_hash(text: &str) -> String {
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads one frame field. `subject` is accepted as an alias for `actor` so a Problem core written
/// with the definition vocabulary still lands in the same slot as the Signals it was built from.
fn framed_field(frame: Option<&Value>, key: &str) -> String {
    let Some(frame) = frame else {
        return UNSPECIFIED.to_owned();
    };
    let field = frame.get(key).or_else(|| {
        if key == "actor" {
            frame.get("subject")
        } else {
            None
        }
    });
    field
        .map(|field| field.get("value").unwrap_or(field))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(UNSPECIFIED)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn frame() -> Value {
        json!({
            "actor":{"value":"孩子","basis":"我家孩子"},
            "goalOrExpectedState":{"value":"自主开始作业","basis":"不催就不开始"},
            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
            "context":{"value":"家庭作业","basis":"写作业"}
        })
    }

    #[test]
    fn a_signal_and_a_problem_core_built_from_the_same_fields_encode_identically() {
        // The invariant the whole recall path rests on. If these two ever diverge, a Problem and
        // the Signals judged against it stop being the same kind of sentence, and every cosine
        // between them becomes meaningless while still looking like a number.
        let from_signal = canonical_text("孩子在家庭作业中存在自主启动困难", Some(&frame()));
        let from_problem_core = canonical_text(
            "孩子在家庭作业中存在自主启动困难",
            Some(&json!({
                "subject":"孩子",
                "goalOrExpectedState":"自主开始作业",
                "barrierOrUnmetNeed":"需要外部催促",
                "context":"家庭作业"
            })),
        );
        assert_eq!(from_signal, from_problem_core);
        assert_eq!(canonical_hash(&from_signal), canonical_hash(&from_problem_core));
    }

    #[test]
    fn an_unexpressed_field_stays_unspecified_rather_than_borrowing_a_neighbour() {
        let text = canonical_text(
            "孩子写作业拖延",
            Some(&json!({
                "actor":{"value":"孩子","basis":"我家孩子"},
                "goalOrExpectedState":{"value":null,"basis":null},
                "barrierOrUnmetNeed":{"value":"拖延","basis":"拖延"},
                "context":{"value":"  ","basis":null}
            })),
        );
        assert!(text.contains("目标：未明确"), "{text}");
        assert!(text.contains("场景：未明确"), "{text}");
        assert!(text.contains("主体：孩子"), "{text}");
    }

    #[test]
    fn a_frameless_expression_still_encodes_with_every_slot_present() {
        // Slot count is part of the shape: dropping absent slots would make a frameless signal
        // encode as a structurally different sentence from a framed one.
        let text = canonical_text("这个视频配乐很好听", None);
        assert_eq!(text.lines().count(), 5);
        assert!(text.starts_with("表达：这个视频配乐很好听"));
    }

    #[test]
    fn differing_barriers_under_one_goal_do_not_collapse_to_the_same_text() {
        // The manual's own counter-example: same goal, different mechanism is not the same
        // problem, so the encoded text must keep them apart before a model ever compares them.
        let reading = canonical_text(
            "想按时做完作业",
            Some(&json!({"actor":{"value":"孩子"},"goalOrExpectedState":{"value":"按时完成作业"},
                         "barrierOrUnmetNeed":{"value":"不会读题"},"context":{"value":"家庭作业"}})),
        );
        let starting = canonical_text(
            "想按时做完作业",
            Some(&json!({"actor":{"value":"孩子"},"goalOrExpectedState":{"value":"按时完成作业"},
                         "barrierOrUnmetNeed":{"value":"迟迟无法开始"},"context":{"value":"家庭作业"}})),
        );
        assert_ne!(reading, starting);
        assert_ne!(canonical_hash(&reading), canonical_hash(&starting));
    }
}
