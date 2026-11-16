//! Pure L1 draft admission and source-bound Content Card assembly; no I/O or model calls.
use linggan_contracts::is_rfc3339_timestamp;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub mod model_draft;
mod schema;
pub use schema::response_schema;
#[cfg(test)]
mod tests;

pub const METHOD: &str = "content-understanding.l1.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextSource {
    pub citation_id: String,
    pub source_ref: Uuid,
    pub source_version: String,
    /// Unicode scalar offset of this input slice within its canonical source.
    pub start: usize,
    pub text: String,
    pub kind: String,
    pub speaker: String,
    pub structure: Value,
    pub issues: Vec<Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageSource {
    pub citation_id: String,
    pub source_ref: Uuid,
    pub source_version: String,
    /// The caller must calculate and verify this against the bytes actually sent.
    pub blob_sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub work_ref: Uuid,
    pub material_snapshot_identity: String,
    pub material_as_of: String,
    pub included_kinds: Vec<String>,
    pub coverage_mode: String,
    pub texts: Vec<TextSource>,
    pub images: Vec<ImageSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generation {
    pub analysis_ref: Uuid,
    pub skill_version_ref: String,
    pub prompt_version_ref: String,
    pub model_config_ref: String,
    pub provider_returned_model: String,
    pub created_at: String,
}

// Draft types are read item by item: a malformed item is dropped and noted,
// it does not discard the rest of the model answer. Unknown fields are ignored
// because identity, offsets and hashes are always rebuilt by the program.
#[derive(Debug)]
struct Draft {
    core: CoreDraft,
    image_contributions: Vec<ImageContribution>,
    evidence_candidates: EvidenceCandidates,
}
#[derive(Debug)]
struct CoreDraft {
    summary: Summary,
    content_type: ContentType,
    main_topic_unit_id: String,
    knowledge_units: Vec<KnowledgeUnit>,
    candidate_labels: Vec<CandidateLabel>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Summary {
    text: String,
    unit_ids: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContentType {
    PracticalGuide,
    Checklist,
    Explanation,
    PersonalStory,
    Opinion,
    CaseAnalysis,
    InteractiveChallenge,
    Question,
    ProductIntro,
    Mixed,
    Other,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UnitType {
    Discussion,
    AudienceContext,
    AuthorPosition,
    Provision,
    ReaderValue,
    AnswerForm,
    Expression,
    Participation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AssertionKind {
    Observation,
    AuthorClaim,
    Interpretation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SpeakerRole {
    Author,
    Quoted,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeUnit {
    id: String,
    unit_type: UnitType,
    statement: String,
    assertion_kind: AssertionKind,
    speaker_role: SpeakerRole,
    evidence_refs: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LabelDimension {
    // This draft-only category belongs to knowledge units and is never published as a label.
    AuthorPosition,
    Discussion,
    Audience,
    Situation,
    OfferedValue,
    AnswerForm,
    Expression,
    Participation,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateLabel {
    dimension: LabelDimension,
    label: String,
    retrieval_use: String,
    unit_ids: Vec<String>,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ContributionStatus {
    Used,
    NoNewInformation,
    Unclear,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageContribution {
    input_citation_id: String,
    status: ContributionStatus,
    summary: String,
    #[serde(default)]
    unit_ids: Vec<String>,
    #[serde(default)]
    evidence_refs: Vec<String>,
}
#[derive(Debug)]
struct EvidenceCandidates {
    text: Vec<TextCandidate>,
    vision: Vec<VisionCandidate>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextCandidate {
    id: String,
    input_citation_id: String,
    quote: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VisionCandidate {
    id: String,
    input_citation_id: String,
    #[serde(default)]
    model_quote: Option<String>,
    #[serde(default)]
    observation: Option<String>,
    speaker_role: SpeakerRole,
}

fn sized(value: &str, min: usize, max: usize) -> bool {
    (min..=max).contains(&value.chars().count())
}
pub(crate) fn local_id(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
// `frame_text` is no longer produced (video frame text retired 2026-11-16); it stays
// valid so historical frozen snapshots can still be read.
const TEXT_KINDS: &[&str] = &["title", "body", "tags", "ocr", "transcript", "frame_text"];

fn validate_snapshot(snapshot: &Snapshot) -> Result<(), &'static str> {
    let mut aliases = BTreeSet::new();
    let mut actual_kinds = BTreeSet::new();
    for source in &snapshot.texts {
        if !sized(&source.citation_id, 1, 500)
            || !sized(&source.source_version, 1, 160)
            || !TEXT_KINDS.contains(&source.kind.as_str())
            || !aliases.insert(source.citation_id.as_str())
        {
            return Err("invalid_snapshot");
        }
        actual_kinds.insert(source.kind.as_str());
    }
    for source in &snapshot.images {
        if !sized(&source.citation_id, 1, 500)
            || !sized(&source.source_version, 1, 160)
            || !hash(&source.blob_sha256)
            || !aliases.insert(source.citation_id.as_str())
        {
            return Err("invalid_snapshot");
        }
        actual_kinds.insert("image");
    }
    let declared: BTreeSet<_> = snapshot.included_kinds.iter().map(String::as_str).collect();
    if !hash(&snapshot.material_snapshot_identity)
        || !is_rfc3339_timestamp(&snapshot.material_as_of)
        || !matches!(snapshot.coverage_mode.as_str(), "complete" | "partial")
        || !(1..=7).contains(&snapshot.included_kinds.len())
        || declared.len() != snapshot.included_kinds.len()
        || declared != actual_kinds
    {
        return Err("invalid_snapshot");
    }
    Ok(())
}
fn validate_generation(generation: &Generation) -> Result<(), &'static str> {
    if ![
        &generation.skill_version_ref,
        &generation.prompt_version_ref,
        &generation.model_config_ref,
        &generation.provider_returned_model,
    ]
    .into_iter()
    .all(|v| sized(v, 1, 160))
        || !is_rfc3339_timestamp(&generation.created_at)
    {
        return Err("invalid_generation");
    }
    Ok(())
}

fn transcript_line_break(text: &str) -> Option<&str> {
    text.strip_prefix("\r\n")
        .or_else(|| text.strip_prefix('\n'))
}

fn is_han(ch: char) -> bool {
    matches!(ch, '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}')
}

// For a Chinese ASR line break already present on both sides, an extra comma
// before the break can be a model formatting artifact. No other punctuation
// or characters are removed and the caller stores the original source slice.
fn transcript_layout_match<'a>(mut source: &'a str, quote: &str) -> Option<&'a str> {
    let mut previous = None;
    let mut chars = quote.chars().peekable();
    while let Some(expected) = chars.next() {
        if let Some(rest) = source.strip_prefix(expected) {
            source = rest;
            if matches!(expected, ',' | '，')
                && previous.is_some_and(is_han)
                && chars.peek().copied().is_some_and(is_han)
                && let Some(rest) = transcript_line_break(source)
            {
                source = rest;
            }
        } else if matches!(expected, ',' | '，')
            && previous.is_some_and(is_han)
            && let Some(rest) = transcript_line_break(source)
        {
            let next = chars.next()?;
            if next == '\r' && chars.next() != Some('\n') || !matches!(next, '\r' | '\n') {
                return None;
            }
            if !chars.peek().copied().is_some_and(is_han) {
                return None;
            }
            source = rest;
        } else {
            return None;
        }
        previous = Some(expected);
    }
    Some(source)
}

/// Locate a quoted phrase in one source. The first hit wins; `repeated`
/// reports that the same phrase occurs again later in the source.
fn quote_in_source<'a>(
    source: &'a str,
    quote: &str,
    allow_transcript_wrap: bool,
) -> Option<(usize, &'a str, bool)> {
    // A transcript can wrap a fragment immediately after a comma. The
    // stored quotation still comes from one unchanged, contiguous slice.
    let check_wrap =
        allow_transcript_wrap && !quote.contains(['\r', '\n']) && source.contains('\n');
    let first = quote.chars().next()?;
    let check_layout = allow_transcript_wrap && quote.contains('\n') && source.contains('\n');
    let mut candidates = source.match_indices(first).flat_map(|(byte, _)| {
        let tail = &source[byte..];
        let strict = (|| {
            let mut remaining = tail;
            let mut after_comma = false;
            for expected in quote.chars() {
                if check_wrap && after_comma {
                    remaining = transcript_line_break(remaining).unwrap_or(remaining);
                }
                remaining = remaining.strip_prefix(expected)?;
                after_comma = matches!(expected, ',' | '，');
            }
            Some(remaining)
        })();
        let layout = check_layout
            .then(|| transcript_layout_match(tail, quote))
            .flatten()
            .filter(|rest| strict.is_none_or(|other| other.len() != rest.len()));
        [strict, layout]
            .into_iter()
            .flatten()
            .map(move |rest| (byte, &source[byte..source.len() - rest.len()]))
    });
    let (byte, slice) = candidates.next()?;
    Some((byte, slice, candidates.next().is_some()))
}

/// Match on letters and digits only, then return the original source slice.
/// The stored quotation therefore stays a literal copy of the material.
fn loose_quote_in_source<'a>(source: &'a str, quote: &str) -> Option<(usize, &'a str)> {
    let wanted: Vec<char> = quote.chars().filter(|c| c.is_alphanumeric()).collect();
    if wanted.len() < 4 {
        return None;
    }
    let kept: Vec<(usize, char)> = source
        .char_indices()
        .filter(|(_, c)| c.is_alphanumeric())
        .collect();
    let at = kept
        .windows(wanted.len())
        .position(|window| window.iter().map(|(_, c)| *c).eq(wanted.iter().copied()))?;
    let start = kept[at].0;
    let (last, ch) = kept[at + wanted.len() - 1];
    Some((start, &source[start..last + ch.len_utf8()]))
}

/// Find a model quotation in the frozen material: the cited source first, then
/// sources of the same kind, then all others; exact matches before loose ones.
fn locate_quote<'a>(
    snapshot: &'a Snapshot,
    cited: Option<usize>,
    quote: &str,
) -> Option<(usize, usize, &'a str, &'static str)> {
    let texts = &snapshot.texts;
    let kind = cited.map(|index| texts[index].kind.as_str());
    let mut order: Vec<usize> = cited.into_iter().collect();
    order.extend(
        (0..texts.len()).filter(|i| Some(*i) != cited && kind == Some(texts[*i].kind.as_str())),
    );
    order.extend(
        (0..texts.len()).filter(|i| Some(*i) != cited && kind != Some(texts[*i].kind.as_str())),
    );
    for (rank, &index) in order.iter().enumerate() {
        let source = &texts[index];
        if let Some((byte, slice, repeated)) =
            quote_in_source(&source.text, quote, source.kind == "transcript")
        {
            let how = match (rank == 0 && cited.is_some(), repeated) {
                (true, false) => "exact",
                (true, true) => "quote_repeated_first_used",
                _ => "quote_reanchored",
            };
            return Some((index, byte, slice, how));
        }
    }
    order.iter().enumerate().find_map(|(rank, &index)| {
        let (byte, slice) = loose_quote_in_source(&texts[index].text, quote)?;
        let how = if rank == 0 && cited.is_some() {
            "quote_punctuation_ignored"
        } else {
            "quote_reanchored_punctuation_ignored"
        };
        Some((index, byte, slice, how))
    })
}

/// Accept the new draft protocol only. The caller retains the untouched raw response.
/// Structure/source admission is not semantic certification or permission to use the sources.
pub fn accept(
    draft: Value,
    snapshot: &Snapshot,
    generation: &Generation,
) -> Result<Value, &'static str> {
    accept_draft(draft, snapshot, generation, false, &BTreeSet::new())
}

const MAX_UNITS: usize = 12;
const MAX_LABELS: usize = 12;
const MAX_EVIDENCE: usize = 64;
const MAX_REFS: usize = 8;

fn note(notes: &mut Vec<Value>, kind: &str, id: &str, reason: &str) {
    let id: String = id.chars().take(64).collect();
    notes.push(json!({"kind": kind, "id": id, "reason": reason}));
}
fn dedupe(values: &mut Vec<String>) {
    let mut seen = BTreeSet::new();
    values.retain(|value| seen.insert(value.clone()));
}
fn meaningful(value: Option<String>) -> Option<String> {
    value.filter(|s| sized(s, 1, 500) && !s.trim().is_empty())
}
/// Unknown enum values fall back to the neutral value instead of failing the item.
fn neutral_default<T: DeserializeOwned>(slot: Option<&mut Value>, neutral: &str) -> bool {
    match slot {
        Some(slot) if serde_json::from_value::<T>(slot.clone()).is_err() => {
            *slot = json!(neutral);
            true
        }
        _ => false,
    }
}
fn items<T: DeserializeOwned>(list: Option<&Value>, kind: &str, notes: &mut Vec<Value>) -> Vec<T> {
    let mut kept = Vec::new();
    for item in list.and_then(Value::as_array).into_iter().flatten() {
        match serde_json::from_value(item.clone()) {
            Ok(parsed) => kept.push(parsed),
            Err(_) => {
                let id = item["id"].as_str().or(item["inputCitationId"].as_str());
                note(notes, kind, id.unwrap_or(""), "unreadable_item");
            }
        }
    }
    kept
}

fn read_draft(mut value: Value, notes: &mut Vec<Value>) -> Result<Draft, &'static str> {
    for pointer in ["/core/knowledgeUnits", "/evidenceCandidates/vision"] {
        for item in value
            .pointer_mut(pointer)
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            neutral_default::<SpeakerRole>(item.get_mut("speakerRole"), "unknown");
        }
    }
    for item in value
        .pointer_mut("/imageContributions")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        neutral_default::<ContributionStatus>(item.get_mut("status"), "unclear");
    }
    if neutral_default::<ContentType>(value.pointer_mut("/core/contentType"), "other") {
        note(notes, "contentType", "", "unknown_value_set_to_other");
    }
    let core = value.get("core").ok_or("invalid_draft")?;
    let summary = serde_json::from_value(core["summary"].clone()).map_err(|_| "invalid_draft")?;
    let content_type =
        serde_json::from_value(core["contentType"].clone()).unwrap_or(ContentType::Other);
    Ok(Draft {
        core: CoreDraft {
            summary,
            content_type,
            main_topic_unit_id: core["mainTopicUnitId"].as_str().unwrap_or_default().into(),
            knowledge_units: items(core.get("knowledgeUnits"), "knowledgeUnit", notes),
            candidate_labels: items(core.get("candidateLabels"), "candidateLabel", notes),
        },
        image_contributions: items(value.get("imageContributions"), "imageContribution", notes),
        evidence_candidates: EvidenceCandidates {
            text: items(
                value.pointer("/evidenceCandidates/text"),
                "textEvidence",
                notes,
            ),
            vision: items(
                value.pointer("/evidenceCandidates/vision"),
                "visionEvidence",
                notes,
            ),
        },
    })
}

/// IDs are local to one answer, so malformed ones are renamed consistently
/// instead of discarding the answer.
fn normalize_ids(draft: &mut Draft, notes: &mut Vec<Value>) {
    fn plan<'a>(
        ids: impl Iterator<Item = &'a String>,
        prefix: &str,
        notes: &mut Vec<Value>,
        kind: &str,
    ) -> BTreeMap<String, String> {
        let ids: Vec<&String> = ids.collect();
        let taken: BTreeSet<&str> = ids.iter().map(|id| id.as_str()).collect();
        let mut renamed = BTreeMap::new();
        let mut next = 0;
        for id in ids {
            if local_id(id) || renamed.contains_key(id) {
                continue;
            }
            let fresh = loop {
                next += 1;
                let candidate = format!("{prefix}{next}");
                if !taken.contains(candidate.as_str()) {
                    break candidate;
                }
            };
            note(notes, kind, id, "id_renamed");
            renamed.insert(id.clone(), fresh);
        }
        renamed
    }
    fn apply(values: &mut [String], renamed: &BTreeMap<String, String>) {
        for value in values {
            if let Some(fresh) = renamed.get(value) {
                *value = fresh.clone();
            }
        }
    }
    let units = plan(
        draft.core.knowledge_units.iter().map(|u| &u.id),
        "ux",
        notes,
        "knowledgeUnit",
    );
    let evidence_ids: Vec<String> = draft
        .evidence_candidates
        .text
        .iter()
        .map(|e| e.id.clone())
        .chain(
            draft
                .evidence_candidates
                .vision
                .iter()
                .map(|e| e.id.clone()),
        )
        .collect();
    let evidence = plan(evidence_ids.iter(), "evx", notes, "evidence");
    let core = &mut draft.core;
    for unit in &mut core.knowledge_units {
        apply(std::slice::from_mut(&mut unit.id), &units);
        apply(&mut unit.evidence_refs, &evidence);
    }
    apply(std::slice::from_mut(&mut core.main_topic_unit_id), &units);
    apply(&mut core.summary.unit_ids, &units);
    for label in &mut core.candidate_labels {
        apply(&mut label.unit_ids, &units);
    }
    for contribution in &mut draft.image_contributions {
        apply(&mut contribution.unit_ids, &units);
        apply(&mut contribution.evidence_refs, &evidence);
    }
    for candidate in &mut draft.evidence_candidates.text {
        apply(std::slice::from_mut(&mut candidate.id), &evidence);
    }
    for candidate in &mut draft.evidence_candidates.vision {
        apply(std::slice::from_mut(&mut candidate.id), &evidence);
    }
}

/// A reference that names a source segment or image instead of an evidence entry
/// points to the entries quoting that source.
fn map_source_refs(draft: &mut Draft, notes: &mut Vec<Value>) {
    let candidates = &draft.evidence_candidates;
    let mut by_source: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (id, source) in candidates
        .text
        .iter()
        .map(|c| (&c.id, &c.input_citation_id))
        .chain(
            candidates
                .vision
                .iter()
                .map(|c| (&c.id, &c.input_citation_id)),
        )
    {
        by_source
            .entry(source.as_str())
            .or_default()
            .push(id.clone());
    }
    let ids: BTreeSet<&str> = by_source.values().flatten().map(String::as_str).collect();
    let mut remap = |refs: &mut Vec<String>, owner: &str| {
        let mut mapped = Vec::new();
        for reference in refs.drain(..) {
            match by_source.get(reference.as_str()) {
                Some(entries) if !ids.contains(reference.as_str()) => {
                    note(notes, "reference", owner, "source_ref_mapped_to_evidence");
                    mapped.extend(entries.iter().cloned());
                }
                _ => mapped.push(reference),
            }
        }
        dedupe(&mut mapped);
        *refs = mapped;
    };
    for unit in &mut draft.core.knowledge_units {
        remap(&mut unit.evidence_refs, &unit.id);
    }
    for contribution in &mut draft.image_contributions {
        remap(
            &mut contribution.evidence_refs,
            &contribution.input_citation_id,
        );
    }
}

/// Admit what the material supports. A defect in one item drops that item and is
/// recorded in `admissionNotes`; only a missing summary or no supported unit fails
/// the answer. Stored quotations are always literal slices of the frozen material.
fn accept_draft(
    draft: Value,
    snapshot: &Snapshot,
    generation: &Generation,
    short_source_ids: bool,
    excluded_units: &BTreeSet<String>,
) -> Result<Value, &'static str> {
    validate_snapshot(snapshot)?;
    validate_generation(generation)?;
    let mut notes = Vec::new();
    let mut draft = read_draft(draft, &mut notes)?;
    if !sized(&draft.core.summary.text, 1, 160) {
        return Err("invalid_draft");
    }
    normalize_ids(&mut draft, &mut notes);
    map_source_refs(&mut draft, &mut notes);

    let mut unit_ids = BTreeSet::new();
    let mut units = Vec::new();
    for mut unit in std::mem::take(&mut draft.core.knowledge_units) {
        dedupe(&mut unit.evidence_refs);
        if excluded_units.contains(&unit.id) {
            note(&mut notes, "knowledgeUnit", &unit.id, "unsupported_literal");
        } else if !sized(&unit.statement, 1, 280) || unit.evidence_refs.is_empty() {
            note(&mut notes, "knowledgeUnit", &unit.id, "invalid_unit");
        } else if !unit_ids.insert(unit.id.clone()) {
            note(&mut notes, "knowledgeUnit", &unit.id, "duplicate_id");
        } else if units.len() == MAX_UNITS {
            note(&mut notes, "knowledgeUnit", &unit.id, "over_limit");
        } else {
            units.push(unit);
        }
    }

    let used: BTreeSet<String> = units
        .iter()
        .flat_map(|unit| unit.evidence_refs.iter().cloned())
        .chain(
            draft
                .image_contributions
                .iter()
                .flat_map(|c| c.evidence_refs.iter().cloned()),
        )
        .collect();
    let mut seen = BTreeSet::new();
    let mut evidence: Vec<(String, Value)> = Vec::new();
    let mut image_of: BTreeMap<String, String> = BTreeMap::new();
    for candidate in &draft.evidence_candidates.text {
        if !seen.insert(candidate.id.clone()) {
            note(&mut notes, "textEvidence", &candidate.id, "duplicate_id");
            continue;
        }
        if !used.contains(&candidate.id) {
            continue;
        }
        let cited = if short_source_ids {
            candidate
                .input_citation_id
                .strip_prefix('s')
                .and_then(|id| id.parse::<usize>().ok().filter(|n| n.to_string() == id))
                .and_then(|n| n.checked_sub(1))
                .filter(|index| *index < snapshot.texts.len())
        } else {
            snapshot
                .texts
                .iter()
                .position(|s| s.citation_id == candidate.input_citation_id)
        };
        let located = sized(&candidate.quote, 1, 500)
            .then(|| locate_quote(snapshot, cited, &candidate.quote))
            .flatten()
            .filter(|(_, _, quote, _)| sized(quote, 1, 500));
        let Some((index, byte, quote, how)) = located else {
            note(
                &mut notes,
                "textEvidence",
                &candidate.id,
                "quote_not_in_material",
            );
            continue;
        };
        if how != "exact" {
            note(&mut notes, "textEvidence", &candidate.id, how);
        }
        let source = &snapshot.texts[index];
        let start = source
            .start
            .checked_add(source.text[..byte].chars().count())
            .ok_or("invalid_text_quote")?;
        let end = start
            .checked_add(quote.chars().count())
            .ok_or("invalid_text_quote")?;
        evidence.push((
            candidate.id.clone(),
            json!({
                "id": candidate.id, "inputCitationId": source.citation_id,
                "sourceRef": source.source_ref, "sourceVersion": source.source_version,
                "sourceKind": "material", "start": start, "end": end, "quote": quote
            }),
        ));
    }
    for candidate in std::mem::take(&mut draft.evidence_candidates.vision) {
        if !seen.insert(candidate.id.clone()) {
            note(&mut notes, "visionEvidence", &candidate.id, "duplicate_id");
            continue;
        }
        if !used.contains(&candidate.id) {
            continue;
        }
        let (model_quote, observation) = (
            meaningful(candidate.model_quote),
            meaningful(candidate.observation),
        );
        let Some(source) = snapshot
            .images
            .iter()
            .find(|s| s.citation_id == candidate.input_citation_id)
            .filter(|_| model_quote.is_some() || observation.is_some())
        else {
            note(
                &mut notes,
                "visionEvidence",
                &candidate.id,
                "invalid_image_evidence",
            );
            continue;
        };
        image_of.insert(candidate.id.clone(), source.citation_id.clone());
        evidence.push((
            candidate.id.clone(),
            json!({
                "id": candidate.id, "inputCitationId": source.citation_id,
                "sourceRef": source.source_ref, "sourceVersion": source.source_version,
                "sourceKind": "material", "anchorKind": "image", "blobSha256": source.blob_sha256,
                "modelQuote": model_quote, "observation": observation,
                "speakerRole": candidate.speaker_role, "verification": "model_read"
            }),
        ));
    }
    for (id, _) in evidence.drain(evidence.len().min(MAX_EVIDENCE)..) {
        note(&mut notes, "evidence", &id, "over_limit");
    }
    let bound: BTreeSet<String> = evidence.iter().map(|(id, _)| id.clone()).collect();

    units.retain_mut(|unit| {
        unit.evidence_refs.retain(|id| bound.contains(id));
        unit.evidence_refs.truncate(MAX_REFS);
        let supported = !unit.evidence_refs.is_empty();
        if !supported {
            note(
                &mut notes,
                "knowledgeUnit",
                &unit.id,
                "no_supported_evidence",
            );
        }
        supported
    });
    if units.is_empty() {
        return Err("invalid_draft");
    }
    let unit_ids: BTreeSet<&str> = units.iter().map(|u| u.id.as_str()).collect();

    let mut contributed = BTreeSet::new();
    let mut contributions = Vec::new();
    for mut contribution in std::mem::take(&mut draft.image_contributions) {
        if !snapshot
            .images
            .iter()
            .any(|s| s.citation_id == contribution.input_citation_id)
            || !sized(&contribution.summary, 1, 500)
            || contribution.summary.trim().is_empty()
            || !contributed.insert(contribution.input_citation_id.clone())
        {
            note(
                &mut notes,
                "imageContribution",
                &contribution.input_citation_id,
                "invalid_contribution",
            );
            continue;
        }
        contribution
            .unit_ids
            .retain(|id| unit_ids.contains(id.as_str()));
        dedupe(&mut contribution.unit_ids);
        contribution.unit_ids.truncate(MAX_UNITS);
        let image = contribution.input_citation_id.clone();
        contribution
            .evidence_refs
            .retain(|id| image_of.get(id) == Some(&image));
        dedupe(&mut contribution.evidence_refs);
        contribution.evidence_refs.truncate(MAX_REFS);
        contributions.push(contribution);
    }

    let core = &mut draft.core;
    let declared = units.iter().find(|u| u.id == core.main_topic_unit_id);
    let main_topic = match declared {
        Some(unit) if unit.unit_type == UnitType::Discussion => unit.id.clone(),
        _ => {
            let fallback = units
                .iter()
                .find(|u| u.unit_type == UnitType::Discussion)
                .or(declared)
                .unwrap_or(&units[0]);
            note(
                &mut notes,
                "mainTopic",
                &core.main_topic_unit_id,
                "main_topic_reassigned",
            );
            fallback.id.clone()
        }
    };
    core.summary
        .unit_ids
        .retain(|id| unit_ids.contains(id.as_str()));
    dedupe(&mut core.summary.unit_ids);
    core.summary.unit_ids.truncate(MAX_REFS);
    if core.summary.unit_ids.is_empty() {
        core.summary.unit_ids.push(main_topic.clone());
    }
    let mut labels = Vec::new();
    for mut label in std::mem::take(&mut core.candidate_labels) {
        label.unit_ids.retain(|id| unit_ids.contains(id.as_str()));
        dedupe(&mut label.unit_ids);
        label.unit_ids.truncate(MAX_REFS);
        if !sized(&label.label, 1, 80)
            || !sized(&label.retrieval_use, 1, 180)
            || label.unit_ids.is_empty()
        {
            note(&mut notes, "candidateLabel", &label.label, "invalid_label");
        } else if labels.len() == MAX_LABELS {
            note(&mut notes, "candidateLabel", &label.label, "over_limit");
        } else {
            labels.push(label);
        }
    }
    let candidate_labels: Vec<_> = labels
        .iter()
        .filter(|label| !matches!(label.dimension, LabelDimension::AuthorPosition))
        .collect();
    // Category arrays are views over knowledgeUnits; a long view is shortened, not rejected.
    let mut project = |kind: UnitType, max: usize| -> Vec<String> {
        let mut ids: Vec<String> = units
            .iter()
            .filter(|u| u.unit_type == kind)
            .map(|u| u.id.clone())
            .collect();
        for id in ids.drain(ids.len().min(max)..) {
            note(&mut notes, "categoryView", &id, "over_limit");
        }
        ids
    };
    let audience = project(UnitType::AudienceContext, 4);
    let author_position = project(UnitType::AuthorPosition, 4);
    let provision = project(UnitType::Provision, 8);
    let reader_value = project(UnitType::ReaderValue, 4);

    let referenced: BTreeSet<&str> = units
        .iter()
        .flat_map(|u| &u.evidence_refs)
        .chain(contributions.iter().flat_map(|c| &c.evidence_refs))
        .map(String::as_str)
        .collect();
    let evidence: Vec<Value> = evidence
        .into_iter()
        .filter(|(id, _)| referenced.contains(id.as_str()))
        .map(|(_, value)| value)
        .collect();
    let card = json!({
        "schemaVersion": "content-card.v0.2", "workRef": snapshot.work_ref,
        "analysisRef": generation.analysis_ref,
        "sourceSnapshot": {
            "materialSnapshotIdentity": snapshot.material_snapshot_identity,
            "materialAsOf": snapshot.material_as_of, "includedKinds": snapshot.included_kinds,
            "coverageMode": snapshot.coverage_mode
        },
        "core": {
            "summary": core.summary, "contentType": core.content_type,
            "mainTopicUnitId": main_topic,
            "audienceUnitIds": audience, "authorPositionUnitIds": author_position,
            "provisionUnitIds": provision, "readerValueUnitIds": reader_value,
            "knowledgeUnits": units, "candidateLabels": candidate_labels
        },
        "deep": null, "evidence": evidence,
        "generation": {
            "methodVersion": METHOD, "skillVersionRef": generation.skill_version_ref,
            "promptVersionRef": generation.prompt_version_ref, "modelConfigRef": generation.model_config_ref,
            "providerReturnedModel": generation.provider_returned_model, "createdAt": generation.created_at
        }
    });
    let mut result = json!({"card": card, "imageContributions": contributions});
    if !notes.is_empty() {
        result["admissionNotes"] = json!(notes);
    }
    Ok(result)
}
