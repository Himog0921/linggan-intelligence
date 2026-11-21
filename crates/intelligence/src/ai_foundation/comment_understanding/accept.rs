//! Admission of one `comment-understanding.v1` response.
//!
//! A defect costs only the smallest piece that cannot be trusted: a signal, then a target, and
//! the whole unit only when its top level is unusable. Evidence must be one unambiguous span of
//! the target's own text, so a comment can never be backed by another comment, the parent or the
//! work context.
use super::contract::{
    CONTRACT, FIELD_MAX_CHARS, KEY_MAX_CHARS, LABEL_MAX_CHARS, MAX_SIGNALS_PER_TARGET,
    REASON_MAX_CHARS, RESULT_CONTRACT, TEXT_MAX_CHARS, optional_text, trimmed,
};
use crate::{comment_cleaning::CleanComment, comment_study_semantic::SignalKind};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use uuid::Uuid;

/// A target as frozen for the call: the original text and its cleaned form, the one evidence
/// is resolved against.
#[derive(Debug, Clone)]
pub struct TargetFrame {
    pub alias: String,
    pub raw_text: String,
    pub cleaned: CleanComment,
}

/// The unit's output is unusable as a whole; every target of the unit fails with the same code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitRejection {
    OutputInvalid,
}

impl UnitRejection {
    pub fn failure_code(self) -> &'static str {
        match self {
            Self::OutputInvalid => "unit_output_invalid",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewProblem {
    pub key: String,
    pub label: String,
    pub actor: Option<String>,
    pub goal: String,
    pub barrier: String,
    pub context: Option<String>,
}

/// Where a problem or need signal was placed: an alias of the frozen catalog, or a new Problem
/// of this unit named by its `newKey`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assignment {
    Existing(String),
    New(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Signals,
    NoSignal,
    NeedsContext,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Signals => "signals",
            Self::NoSignal => "no_signal",
            Self::NeedsContext => "needs_context",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedSignal {
    pub kind: &'static str,
    pub proposition: String,
    pub evidence: String,
    pub evidence_start: i32,
    pub evidence_end: i32,
    pub assignment: Option<Assignment>,
    pub eligibility_state: &'static str,
    pub eligibility_reason: Option<&'static str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedTarget {
    pub alias: String,
    pub outcome: Outcome,
    pub reason: Option<String>,
    pub reason_code: Option<&'static str>,
    pub signals: Vec<AcceptedSignal>,
    pub dropped_signals: usize,
}

impl AcceptedTarget {
    /// The stored per-target result. `resolve` maps an assignment to the Problem it was written
    /// under; the order of `signals` is the signal ordinal downstream readers use.
    pub fn result_json(
        &self,
        source_ref: Uuid,
        resolve: &dyn Fn(&Assignment) -> Option<Uuid>,
    ) -> Value {
        let signals: Vec<Value> = self
            .signals
            .iter()
            .map(|signal| {
                json!({
                    "kind": signal.kind,
                    "proposition": signal.proposition,
                    "sourceRef": source_ref,
                    "evidenceSpan": {"start": signal.evidence_start, "end": signal.evidence_end},
                    "problemFrame": null,
                    "eligibilityState": signal.eligibility_state,
                    "eligibilityReason": signal.eligibility_reason,
                    "problemRef": signal.assignment.as_ref().and_then(|a| resolve(a)),
                })
            })
            .collect();
        json!({
            "contract": RESULT_CONTRACT,
            "outcome": self.outcome.as_str(),
            "reason": self.reason,
            "reasonCode": self.reason_code,
            "signals": signals,
            "droppedSignals": self.dropped_signals,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOutcome {
    Accepted(AcceptedTarget),
    Rejected { alias: String, code: &'static str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedUnit {
    /// Only Problems that an accepted signal refers to, in the order the model declared them.
    pub new_problems: Vec<NewProblem>,
    /// One entry per frozen target, in frozen order.
    pub targets: Vec<TargetOutcome>,
    /// Entries naming no target of this unit (or no readable target at all).
    pub ignored_entries: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawUnit {
    contract: String,
    new_problems: Vec<Value>,
    targets: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawNewProblem {
    new_key: String,
    label: String,
    actor: Option<String>,
    goal: String,
    barrier: String,
    context: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawTarget {
    outcome: String,
    reason: Option<String>,
    reason_code: Option<Value>,
    signals: Vec<Value>,
    #[serde(rename = "targetKey")]
    _target_key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSignal {
    kind: SignalKind,
    proposition: String,
    evidence: String,
    problem: Option<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawAssignment {
    r#ref: Option<String>,
    new_key: Option<String>,
}

pub fn accept_unit(
    text: &str,
    targets: &[TargetFrame],
    sent_comments: &[&str],
    catalog: &HashSet<String>,
) -> Result<AcceptedUnit, UnitRejection> {
    let unit: RawUnit = serde_json::from_str(text).map_err(|_| UnitRejection::OutputInvalid)?;
    if unit.contract != CONTRACT {
        return Err(UnitRejection::OutputInvalid);
    }
    let (declared, proposals) = new_problem_declarations(unit.new_problems);
    let known: HashSet<&str> = targets.iter().map(|t| t.alias.as_str()).collect();
    let mut entries: HashMap<String, Vec<Value>> = HashMap::new();
    let mut ignored_entries = 0;
    for entry in unit.targets {
        match entry
            .get("targetKey")
            .and_then(Value::as_str)
            .filter(|k| known.contains(k))
        {
            Some(key) => entries.entry(key.to_owned()).or_default().push(entry),
            None => ignored_entries += 1,
        }
    }
    let outcomes: Vec<TargetOutcome> = targets
        .iter()
        .map(|frame| {
            let rejected = |code| TargetOutcome::Rejected {
                alias: frame.alias.clone(),
                code,
            };
            match entries.remove(&frame.alias) {
                None => rejected("target_missing"),
                Some(mut list) if list.len() == 1 => {
                    match accept_target(list.remove(0), frame, sent_comments, catalog, &proposals) {
                        Ok(target) => TargetOutcome::Accepted(target),
                        Err(code) => rejected(code),
                    }
                }
                Some(_) => rejected("target_invalid"),
            }
        })
        .collect();
    let used: HashSet<&str> = outcomes
        .iter()
        .filter_map(|outcome| match outcome {
            TargetOutcome::Accepted(target) => Some(target),
            TargetOutcome::Rejected { .. } => None,
        })
        .flat_map(|target| &target.signals)
        .filter_map(|signal| match &signal.assignment {
            Some(Assignment::New(key)) => Some(key.as_str()),
            _ => None,
        })
        .collect();
    let new_problems = declared
        .iter()
        .filter(|p| used.contains(p.key.as_str()))
        .cloned()
        .collect();
    Ok(AcceptedUnit {
        new_problems,
        targets: outcomes,
        ignored_entries,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProposalStatus {
    Valid,
    Invalid,
    Duplicate,
}

/// Keep the first-seen order of valid definitions, while retaining the reason a key failed.
fn new_problem_declarations(
    values: Vec<Value>,
) -> (Vec<NewProblem>, HashMap<String, ProposalStatus>) {
    use std::collections::hash_map::Entry;

    let mut definitions = Vec::new();
    let mut statuses = HashMap::new();
    for value in values {
        let Some(key) = value["newKey"].as_str().map(str::trim) else {
            continue;
        };
        match statuses.entry(key.to_owned()) {
            Entry::Occupied(mut entry) => {
                entry.insert(ProposalStatus::Duplicate);
            }
            Entry::Vacant(entry) => {
                if let Some(definition) = new_problem(value) {
                    entry.insert(ProposalStatus::Valid);
                    definitions.push(definition);
                } else {
                    entry.insert(ProposalStatus::Invalid);
                }
            }
        }
    }
    definitions.retain(|definition| statuses[&definition.key] == ProposalStatus::Valid);
    (definitions, statuses)
}

fn new_problem(value: Value) -> Option<NewProblem> {
    let raw: RawNewProblem = serde_json::from_value(value).ok()?;
    Some(NewProblem {
        key: trimmed(&raw.new_key, KEY_MAX_CHARS)?,
        label: trimmed(&raw.label, LABEL_MAX_CHARS)?,
        actor: optional_text(raw.actor.as_deref(), FIELD_MAX_CHARS).ok()?,
        goal: trimmed(&raw.goal, FIELD_MAX_CHARS)?,
        barrier: trimmed(&raw.barrier, FIELD_MAX_CHARS)?,
        context: optional_text(raw.context.as_deref(), FIELD_MAX_CHARS).ok()?,
    })
}

fn accept_target(
    entry: Value,
    frame: &TargetFrame,
    sent_comments: &[&str],
    catalog: &HashSet<String>,
    proposals: &HashMap<String, ProposalStatus>,
) -> Result<AcceptedTarget, &'static str> {
    let raw: RawTarget = serde_json::from_value(entry).map_err(|_| "target_invalid")?;
    let outcome = match raw.outcome.as_str() {
        "signals" => Outcome::Signals,
        "no_signal" => Outcome::NoSignal,
        "needs_context" => Outcome::NeedsContext,
        _ => return Err("target_invalid"),
    };
    if (outcome == Outcome::Signals) == raw.signals.is_empty() {
        return Err("target_invalid");
    }
    if outcome != Outcome::Signals {
        let reason = raw
            .reason
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
            .map(|reason| reason.chars().take(REASON_MAX_CHARS).collect());
        return Ok(AcceptedTarget {
            alias: frame.alias.clone(),
            outcome,
            reason,
            reason_code: (outcome == Outcome::NoSignal
                && raw.reason_code.as_ref().and_then(Value::as_str) == Some("promotion"))
            .then_some("promotion"),
            signals: Vec::new(),
            dropped_signals: 0,
        });
    }
    let mut dropped_signals = raw.signals.len().saturating_sub(MAX_SIGNALS_PER_TARGET);
    let mut seen = BTreeSet::new();
    let mut signals = Vec::new();
    for value in raw.signals.into_iter().take(MAX_SIGNALS_PER_TARGET) {
        match accept_signal(value, frame, sent_comments, catalog, proposals, &mut seen) {
            Some(signal) => signals.push(signal),
            None => dropped_signals += 1,
        }
    }
    if signals.is_empty() {
        return Err("target_all_signals_invalid");
    }
    Ok(AcceptedTarget {
        alias: frame.alias.clone(),
        outcome,
        reason: None,
        reason_code: None,
        signals,
        dropped_signals,
    })
}

fn accept_signal(
    value: Value,
    frame: &TargetFrame,
    sent_comments: &[&str],
    catalog: &HashSet<String>,
    proposals: &HashMap<String, ProposalStatus>,
    seen: &mut BTreeSet<(&'static str, i32, i32)>,
) -> Option<AcceptedSignal> {
    let raw: RawSignal = serde_json::from_value(value).ok()?;
    let proposition = trimmed(&raw.proposition, TEXT_MAX_CHARS)?;
    if sent_comments.iter().map(|raw| raw.trim()).any(|other| {
        !other.is_empty()
            && !frame.raw_text.contains(other)
            && !frame.cleaned.text.contains(other)
            && proposition.contains(other)
    }) {
        return None;
    }
    let evidence = trimmed(&raw.evidence, TEXT_MAX_CHARS)?;
    let (start, end, original) = frame.cleaned.resolve(&evidence, &frame.raw_text).ok()?;
    let kind = raw.kind.as_str();
    if !seen.insert((kind, start, end)) {
        return None;
    }
    let (assignment, eligibility_state, eligibility_reason) = if raw.kind.has_problem_frame() {
        classify(raw.problem, catalog, proposals)
    } else {
        (None, "not_applicable", None)
    };
    Some(AcceptedSignal {
        kind,
        proposition,
        evidence: original,
        evidence_start: start,
        evidence_end: end,
        assignment,
        eligibility_state,
        eligibility_reason,
    })
}

/// A problem or need signal is eligible only with a usable assignment. An unusable one keeps
/// the signal and loses the assignment, so no signal is ever attached to a Problem that was not
/// in the frozen catalog or declared by this unit.
fn classify(
    problem: Option<Value>,
    catalog: &HashSet<String>,
    proposals: &HashMap<String, ProposalStatus>,
) -> (Option<Assignment>, &'static str, Option<&'static str>) {
    let deferred = |code| (None, "deferred_context", Some(code));
    let Some(problem) = problem else {
        return deferred("no_problem_assignment");
    };
    let Ok(problem) = serde_json::from_value::<RawAssignment>(problem) else {
        return deferred("problem_assignment_shape_invalid");
    };
    match (
        problem.r#ref.as_deref().map(str::trim),
        problem.new_key.as_deref().map(str::trim),
    ) {
        (Some(alias), None) if catalog.contains(alias) => (
            Some(Assignment::Existing(alias.to_owned())),
            "eligible",
            None,
        ),
        (Some(_), None) => deferred("problem_catalog_alias_unknown"),
        (None, Some(key)) => match proposals.get(key) {
            Some(ProposalStatus::Valid) => {
                (Some(Assignment::New(key.to_owned())), "eligible", None)
            }
            Some(ProposalStatus::Invalid) => deferred("problem_new_definition_invalid"),
            Some(ProposalStatus::Duplicate) => deferred("problem_new_key_duplicate"),
            None => deferred("problem_new_key_missing"),
        },
        _ => deferred("problem_assignment_shape_invalid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment_cleaning::clean;

    fn frame(alias: &str, raw: &str) -> TargetFrame {
        TargetFrame {
            alias: alias.into(),
            raw_text: raw.into(),
            cleaned: clean(raw),
        }
    }

    fn catalog(aliases: &[&str]) -> HashSet<String> {
        aliases.iter().map(|a| (*a).to_owned()).collect()
    }

    fn new_problem_json(key: &str) -> Value {
        json!({"newKey":key,"label":"孩子写作业启动困难","actor":"小学生家长","goal":"孩子按时开始写作业",
            "barrier":"需要反复催促才动笔","context":null})
    }

    fn unit(new_problems: Value, targets: Value) -> String {
        json!({"contract":CONTRACT,"newProblems":new_problems,"targets":targets}).to_string()
    }

    fn signal(kind: &str, evidence: &str, problem: Value) -> Value {
        json!({"kind":kind,"proposition":"评论者的孩子写作业需要催","evidence":evidence,"problem":problem})
    }

    fn target(key: &str, signals: Vec<Value>) -> Value {
        json!({"targetKey":key,"outcome":"signals","reason":null,"signals":signals})
    }

    fn accepted(outcome: &TargetOutcome) -> &AcceptedTarget {
        match outcome {
            TargetOutcome::Accepted(target) => target,
            other => panic!("expected an accepted target, got {other:?}"),
        }
    }

    fn accept_unit(
        text: &str,
        targets: &[TargetFrame],
        catalog: &HashSet<String>,
    ) -> Result<AcceptedUnit, UnitRejection> {
        let sent: Vec<_> = targets
            .iter()
            .map(|target| target.raw_text.as_str())
            .collect();
        super::accept_unit(text, targets, &sent, catalog)
    }

    #[test]
    fn valid_evidence_cannot_justify_a_proposition_citing_another_sent_comment() {
        let frames = [frame("t1", "写作业每天都要催。")];
        let other = frame("t2", "孩子周五想学滑冰！");
        assert_ne!(other.raw_text, other.cleaned.text);
        let mut borrowed = signal("belief", "每天都要催", json!(null));
        borrowed["proposition"] = json!(format!("用户说：{}", other.cleaned.text));
        let text = unit(json!([]), json!([target("t1", vec![borrowed])]));
        let result = super::accept_unit(
            &text,
            &frames,
            &[
                frames[0].raw_text.as_str(),
                &other.raw_text,
                &other.cleaned.text,
            ],
            &catalog(&[]),
        )
        .unwrap();
        assert_eq!(
            result.targets[0],
            TargetOutcome::Rejected {
                alias: "t1".into(),
                code: "target_all_signals_invalid"
            }
        );
    }

    #[test]
    fn promotion_requires_no_signal_and_an_explicit_code() {
        for (outcome, code, expected) in [
            ("no_signal", json!("promotion"), json!("promotion")),
            ("no_signal", json!(null), Value::Null),
            ("needs_context", json!("promotion"), Value::Null),
            ("no_signal", json!("unknown"), Value::Null),
        ] {
            let text = unit(
                json!([]),
                json!([{
                    "targetKey":"t1", "outcome":outcome, "reason":"联系入口",
                    "reasonCode":code, "signals":[]
                }]),
            );
            let result = accept_unit(&text, &[frame("t1", "联系入口")], &catalog(&[])).unwrap();
            assert_eq!(
                accepted(&result.targets[0]).result_json(Uuid::nil(), &|_| None)["reasonCode"],
                expected
            );
        }
    }

    #[test]
    fn assignment_faults_preserve_the_original_signal_and_its_evidence() {
        let evidence = "我需要一份学习记录表";
        for (problem, expected) in [
            (json!(null), "no_problem_assignment"),
            (json!("n1"), "problem_assignment_shape_invalid"),
            (
                json!({"ref":"p1","newKey":"n1"}),
                "problem_assignment_shape_invalid",
            ),
            (
                json!({"ref":"p9","newKey":null}),
                "problem_catalog_alias_unknown",
            ),
            (json!({"ref":null,"newKey":"n9"}), "problem_new_key_missing"),
            (
                json!({"ref":null,"newKey":"n7"}),
                "problem_new_definition_invalid",
            ),
            (
                json!({"ref":null,"newKey":"n8"}),
                "problem_new_key_duplicate",
            ),
        ] {
            let mut need = signal("need", evidence, problem);
            need["proposition"] = json!(evidence);
            let text = unit(
                json!([{"newKey":"n7"}, new_problem_json("n8"), {"newKey":"n8"}]),
                json!([target("t1", vec![need])]),
            );
            let result = accept_unit(&text, &[frame("t1", evidence)], &catalog(&["p1"])).unwrap();
            let accepted = accepted(&result.targets[0]);
            assert_eq!(accepted.dropped_signals, 0);
            assert_eq!(accepted.signals[0].eligibility_state, "deferred_context");
            assert_eq!(accepted.signals[0].eligibility_reason, Some(expected));
            assert!(accepted.signals[0].assignment.is_none());
            assert!(result.new_problems.is_empty());
        }
    }

    #[test]
    fn malformed_duplicate_also_disqualifies_a_new_key() {
        let definitions = vec![
            json!({"newKey":"n1","label":"任务启动","goal":"开始写作业","barrier":"需要提醒"}),
            json!({"newKey":"n1"}),
        ];
        assert!(new_problem_declarations(definitions).0.is_empty());
    }

    #[test]
    fn two_targets_share_one_new_problem_and_unreferenced_definitions_are_dropped() {
        let frames = [
            frame("t1", "写作业每天都要催。"),
            frame("t2", "我家也是不催不动笔。"),
        ];
        let text = unit(
            json!([new_problem_json("n1"), new_problem_json("n2")]),
            json!([
                target(
                    "t1",
                    vec![signal(
                        "problem",
                        "写作业每天都要催",
                        json!({"ref":null,"newKey":"n1"})
                    )]
                ),
                target(
                    "t2",
                    vec![signal(
                        "problem",
                        "不催不动笔",
                        json!({"ref":null,"newKey":"n1"})
                    )]
                ),
            ]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&[])).unwrap();
        assert_eq!(unit.new_problems.len(), 1);
        assert_eq!(unit.new_problems[0].key, "n1");
        let first = accepted(&unit.targets[0]);
        assert_eq!(
            first.signals[0].assignment,
            Some(Assignment::New("n1".into()))
        );
        assert_eq!(first.signals[0].eligibility_state, "eligible");
        assert_eq!(
            (
                first.signals[0].evidence_start,
                first.signals[0].evidence_end
            ),
            (0, 8)
        );
    }

    #[test]
    fn evidence_must_come_from_the_targets_own_text() {
        let frames = [
            frame("t1", "写作业每天都要催。"),
            frame("t2", "我家也是不催不动笔。"),
        ];
        let text = unit(
            json!([]),
            json!([
                // t1 quotes t2: the signal is dropped and the target has nothing left.
                target("t1", vec![signal("belief", "不催不动笔", json!(null))]),
                target("t2", vec![signal("belief", "不催不动笔", json!(null))]),
            ]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&[])).unwrap();
        assert_eq!(
            unit.targets[0],
            TargetOutcome::Rejected {
                alias: "t1".into(),
                code: "target_all_signals_invalid"
            }
        );
        assert_eq!(accepted(&unit.targets[1]).signals.len(), 1);
    }

    #[test]
    fn one_bad_signal_is_dropped_and_the_rest_of_the_target_is_kept() {
        let frames = [frame("t1", "写作业每天都要催，我很焦虑。")];
        let text = unit(
            json!([]),
            json!([target(
                "t1",
                vec![
                    signal("emotion", "我很焦虑", json!(null)),
                    signal("emotion", "原文里没有这句", json!(null)),
                    json!({"kind":"nonsense","proposition":"x","evidence":"我很焦虑","problem":null}),
                    signal("emotion", "我很焦虑", json!(null)),
                ]
            )]),
        );
        let target =
            accepted(&accept_unit(&text, &frames, &catalog(&[])).unwrap().targets[0]).clone();
        assert_eq!(target.signals.len(), 1);
        assert_eq!(target.dropped_signals, 3);
    }

    #[test]
    fn missing_duplicated_and_foreign_targets_never_touch_other_targets() {
        let frames = [
            frame("t1", "甲乙丙。"),
            frame("t2", "丁戊己。"),
            frame("t3", "庚辛壬。"),
        ];
        let text = unit(
            json!([]),
            json!([
                target("t1", vec![signal("belief", "甲乙丙", json!(null))]),
                target("t1", vec![signal("belief", "甲乙丙", json!(null))]),
                target("t3", vec![signal("belief", "庚辛壬", json!(null))]),
                target("t9", vec![signal("belief", "庚辛壬", json!(null))]),
                json!({"no":"key"}),
            ]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&[])).unwrap();
        assert_eq!(
            unit.targets[0],
            TargetOutcome::Rejected {
                alias: "t1".into(),
                code: "target_invalid"
            }
        );
        assert_eq!(
            unit.targets[1],
            TargetOutcome::Rejected {
                alias: "t2".into(),
                code: "target_missing"
            }
        );
        assert_eq!(accepted(&unit.targets[2]).signals.len(), 1);
        assert_eq!(unit.ignored_entries, 2);
    }

    #[test]
    fn outcome_and_signals_must_agree() {
        let frames = [
            frame("t1", "甲乙丙。"),
            frame("t2", "丁戊己。"),
            frame("t3", "庚辛壬。"),
        ];
        let text = unit(
            json!([]),
            json!([
                {"targetKey":"t1","outcome":"signals","reason":null,"signals":[]},
                {"targetKey":"t2","outcome":"no_signal","reason":"纯附和","signals":[signal("belief","丁戊己",json!(null))]},
                {"targetKey":"t3","outcome":"no_signal","reason":"  纯附和  ","signals":[]},
            ]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&[])).unwrap();
        assert_eq!(
            unit.targets[0],
            TargetOutcome::Rejected {
                alias: "t1".into(),
                code: "target_invalid"
            }
        );
        assert_eq!(
            unit.targets[1],
            TargetOutcome::Rejected {
                alias: "t2".into(),
                code: "target_invalid"
            }
        );
        assert_eq!(accepted(&unit.targets[2]).reason.as_deref(), Some("纯附和"));
    }

    #[test]
    fn assignments_outside_the_frozen_catalog_or_declarations_lose_the_assignment_only() {
        let frames = [frame("t1", "写作业每天都要催，想要办法。")];
        let text = unit(
            json!([new_problem_json("n1"), new_problem_json("n1")]),
            json!([target(
                "t1",
                vec![
                    signal(
                        "problem",
                        "写作业每天都要催",
                        json!({"ref":"p9","newKey":null})
                    ),
                    signal("need", "想要办法", json!({"ref":null,"newKey":"n1"})),
                    signal("emotion", "想要办法", json!({"ref":"p1","newKey":null})),
                ]
            )]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&["p1"])).unwrap();
        let target = accepted(&unit.targets[0]);
        // p9 is not in the catalog; n1 was declared twice, so it is ambiguous and dropped.
        assert_eq!(target.signals[0].assignment, None);
        assert_eq!(
            target.signals[0].eligibility_reason,
            Some("problem_catalog_alias_unknown")
        );
        assert_eq!(target.signals[1].assignment, None);
        assert!(unit.new_problems.is_empty());
        // A non-problem signal never carries an assignment.
        assert_eq!(target.signals[2].assignment, None);
        assert_eq!(target.signals[2].eligibility_state, "not_applicable");
    }

    #[test]
    fn a_problem_signal_without_assignment_stays_deferred() {
        let frames = [frame("t1", "写作业每天都要催。")];
        let text = unit(
            json!([]),
            json!([target(
                "t1",
                vec![signal("problem", "写作业每天都要催", json!(null))]
            )]),
        );
        let target = accepted(
            &accept_unit(&text, &frames, &catalog(&["p1"]))
                .unwrap()
                .targets[0],
        )
        .clone();
        assert_eq!(target.signals[0].eligibility_state, "deferred_context");
        assert_eq!(
            target.signals[0].eligibility_reason,
            Some("no_problem_assignment")
        );
    }

    #[test]
    fn existing_catalog_alias_is_kept_and_both_keys_set_is_invalid() {
        let frames = [frame("t1", "写作业每天都要催，想要办法。")];
        let text = unit(
            json!([new_problem_json("n1")]),
            json!([target(
                "t1",
                vec![
                    signal(
                        "problem",
                        "写作业每天都要催",
                        json!({"ref":"p1","newKey":null})
                    ),
                    signal("need", "想要办法", json!({"ref":"p1","newKey":"n1"})),
                ]
            )]),
        );
        let unit = accept_unit(&text, &frames, &catalog(&["p1"])).unwrap();
        let target = accepted(&unit.targets[0]);
        assert_eq!(
            target.signals[0].assignment,
            Some(Assignment::Existing("p1".into()))
        );
        assert_eq!(target.signals[1].assignment, None);
        assert!(unit.new_problems.is_empty());
    }

    #[test]
    fn signals_beyond_the_ordinal_limit_are_counted_not_stored() {
        let raw = (0..14).map(|i| format!("句{i}号。")).collect::<String>();
        let signals: Vec<Value> = (0..14)
            .map(|i| signal("belief", &format!("句{i}号"), json!(null)))
            .collect();
        let text = unit(json!([]), json!([target("t1", signals)]));
        let target = accepted(
            &accept_unit(&text, &[frame("t1", &raw)], &catalog(&[]))
                .unwrap()
                .targets[0],
        )
        .clone();
        assert_eq!(target.signals.len(), MAX_SIGNALS_PER_TARGET);
        assert_eq!(target.dropped_signals, 2);
    }

    #[test]
    fn a_broken_top_level_rejects_the_whole_unit() {
        let frames = [frame("t1", "甲乙丙。")];
        for text in [
            "not json".to_owned(),
            json!({"contract":"other","newProblems":[],"targets":[]}).to_string(),
            json!({"contract":CONTRACT,"targets":[]}).to_string(),
            json!({"contract":CONTRACT,"newProblems":[],"targets":[],"extra":1}).to_string(),
        ] {
            assert_eq!(
                accept_unit(&text, &frames, &catalog(&[])),
                Err(UnitRejection::OutputInvalid)
            );
        }
    }

    #[test]
    fn stored_result_keeps_the_fields_downstream_readers_use() {
        let frames = [frame("t1", "写作业每天都要催。")];
        let text = unit(
            json!([new_problem_json("n1")]),
            json!([target(
                "t1",
                vec![signal(
                    "problem",
                    "写作业每天都要催",
                    json!({"ref":null,"newKey":"n1"})
                )]
            )]),
        );
        let target =
            accepted(&accept_unit(&text, &frames, &catalog(&[])).unwrap().targets[0]).clone();
        let (source, problem) = (Uuid::new_v4(), Uuid::new_v4());
        let result = target.result_json(source, &|a| {
            (*a == Assignment::New("n1".into())).then_some(problem)
        });
        assert_eq!(result["contract"], RESULT_CONTRACT);
        assert_eq!(result["outcome"], "signals");
        let stored = &result["signals"][0];
        assert_eq!(stored["kind"], "problem");
        assert_eq!(stored["sourceRef"], json!(source));
        assert_eq!(stored["evidenceSpan"], json!({"start":0,"end":8}));
        assert_eq!(stored["problemFrame"], Value::Null);
        assert_eq!(stored["eligibilityState"], "eligible");
        assert_eq!(stored["problemRef"], json!(problem));
        assert_eq!(result["droppedSignals"], 0);
    }
}
