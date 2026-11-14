//! Pure L1 draft admission and source-bound Content Card assembly; no I/O or model calls.
use linggan_contracts::is_rfc3339_timestamp;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Draft {
    core: CoreDraft,
    image_contributions: Vec<ImageContribution>,
    evidence_candidates: EvidenceCandidates,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CoreDraft {
    summary: Summary,
    content_type: ContentType,
    main_topic_unit_id: String,
    knowledge_units: Vec<KnowledgeUnit>,
    candidate_labels: Vec<CandidateLabel>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageContribution {
    input_citation_id: String,
    status: ContributionStatus,
    summary: String,
    unit_ids: Vec<String>,
    evidence_refs: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EvidenceCandidates {
    text: Vec<TextCandidate>,
    vision: Vec<VisionCandidate>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TextCandidate {
    id: String,
    input_citation_id: String,
    quote: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VisionCandidate {
    id: String,
    input_citation_id: String,
    // deserialize_with makes the nullable fields mandatory, rather than silently defaulting.
    #[serde(deserialize_with = "nullable_text")]
    model_quote: Option<String>,
    #[serde(deserialize_with = "nullable_text")]
    observation: Option<String>,
    speaker_role: SpeakerRole,
}
fn nullable_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
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
fn references(values: &[String], min: usize, max: usize) -> bool {
    (min..=max).contains(&values.len())
        && values.iter().all(|v| local_id(v))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}
fn meaningful_optional(value: &Option<String>) -> bool {
    value
        .as_ref()
        .is_none_or(|s| sized(s, 1, 500) && !s.trim().is_empty())
}
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

fn read_draft(mut value: Value) -> Result<Draft, &'static str> {
    // Only ignore observed metadata with no content. The typed draft still rejects
    // every other extra field, including model-supplied source identity and offsets.
    if let Some(object) = value.as_object_mut()
        && object.get("type").and_then(Value::as_str) == Some("json_object")
    {
        object.remove("type");
    }
    if let Some(core) = value.get_mut("core").and_then(Value::as_object_mut) {
        for key in ["knowledgeUnitsNote", "knowledgeUnits_note"] {
            if core.get(key).is_some_and(Value::is_null) {
                core.remove(key);
            }
        }
    }
    if let Some(images) = value
        .pointer_mut("/evidenceCandidates/vision")
        .and_then(Value::as_array_mut)
    {
        for image in images {
            if let Some(role) = image.get_mut("speakerRole")
                && role.as_str() == Some("unconfirmed")
            {
                *role = json!("unknown");
            }
        }
    }
    serde_json::from_value(value).map_err(|_| "invalid_draft")
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

fn quote_in_source<'a>(
    source: &'a str,
    quote: &str,
    allow_transcript_wrap: bool,
) -> Result<(usize, &'a str), &'static str> {
    // A transcript can wrap a fragment immediately after a comma. The
    // stored quotation still comes from one unchanged, contiguous slice.
    let check_wrap =
        allow_transcript_wrap && !quote.contains(['\r', '\n']) && source.contains('\n');
    // Scan overlaps, and count both exact and wrap-equivalent matches.
    let first = quote.chars().next().ok_or("invalid_text_quote")?;
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
    let hit = candidates.next().ok_or("invalid_text_quote")?;
    if candidates.next().is_some() {
        return Err("invalid_text_quote");
    }
    Ok(hit)
}

/// Accept the new draft protocol only. The caller retains the untouched raw response.
/// Structure/source admission is not semantic certification or permission to use the sources.
pub fn accept(
    draft: Value,
    snapshot: &Snapshot,
    generation: &Generation,
) -> Result<Value, &'static str> {
    let draft = read_draft(draft)?;
    validate_snapshot(snapshot)?;
    validate_generation(generation)?;
    let core = &draft.core;
    if !(1..=12).contains(&core.knowledge_units.len())
        || core.candidate_labels.len() > 12
        || !sized(&core.summary.text, 1, 160)
        || !references(&core.summary.unit_ids, 1, 8)
        || !local_id(&core.main_topic_unit_id)
        || draft.evidence_candidates.text.len() + draft.evidence_candidates.vision.len() > 64
    {
        return Err("invalid_draft");
    }
    let mut units = BTreeMap::new();
    for unit in &core.knowledge_units {
        if !local_id(&unit.id)
            || !sized(&unit.statement, 1, 280)
            || !references(&unit.evidence_refs, 1, 8)
            || units.insert(unit.id.as_str(), unit).is_some()
        {
            return Err("invalid_draft");
        }
    }
    if units
        .get(core.main_topic_unit_id.as_str())
        .map(|u| &u.unit_type)
        != Some(&UnitType::Discussion)
    {
        return Err("invalid_main_topic");
    }
    let existing_units = |ids: &[String]| ids.iter().all(|id| units.contains_key(id.as_str()));
    if !existing_units(&core.summary.unit_ids) {
        return Err("invalid_reference");
    }
    for label in &core.candidate_labels {
        if !sized(&label.label, 1, 80)
            || !sized(&label.retrieval_use, 1, 180)
            || !references(&label.unit_ids, 1, 8)
        {
            return Err("invalid_draft");
        }
        if !existing_units(&label.unit_ids) {
            return Err("invalid_reference");
        }
    }
    let candidate_labels: Vec<_> = core
        .candidate_labels
        .iter()
        .filter(|label| !matches!(label.dimension, LabelDimension::AuthorPosition))
        .collect();
    let project = |kind: UnitType, max: usize| -> Result<Vec<&str>, &'static str> {
        let ids: Vec<_> = core
            .knowledge_units
            .iter()
            .filter(|u| u.unit_type == kind)
            .map(|u| u.id.as_str())
            .collect();
        if ids.len() > max {
            return Err("invalid_draft");
        }
        Ok(ids)
    };
    let audience = project(UnitType::AudienceContext, 4)?;
    let author_position = project(UnitType::AuthorPosition, 4)?;
    let provision = project(UnitType::Provision, 8)?;
    let reader_value = project(UnitType::ReaderValue, 4)?;

    let used: BTreeSet<&str> = core
        .knowledge_units
        .iter()
        .flat_map(|unit| &unit.evidence_refs)
        .chain(
            draft
                .image_contributions
                .iter()
                .flat_map(|contribution| &contribution.evidence_refs),
        )
        .map(String::as_str)
        .collect();
    let mut evidence = Vec::new();
    let mut evidence_sources: BTreeMap<&str, Option<&str>> = BTreeMap::new();
    for candidate in &draft.evidence_candidates.text {
        if !local_id(&candidate.id)
            || !sized(&candidate.input_citation_id, 1, 500)
            || !sized(&candidate.quote, 1, 500)
            || evidence_sources.insert(&candidate.id, None).is_some()
        {
            return Err("invalid_draft");
        }
        if !used.contains(candidate.id.as_str()) {
            continue;
        }
        let source = snapshot
            .texts
            .iter()
            .find(|s| s.citation_id == candidate.input_citation_id)
            .ok_or("invalid_evidence_source")?;
        let (byte, quote) =
            quote_in_source(&source.text, &candidate.quote, source.kind == "transcript")?;
        if !sized(quote, 1, 500) {
            return Err("invalid_text_quote");
        }
        let start = source
            .start
            .checked_add(source.text[..byte].chars().count())
            .ok_or("invalid_text_quote")?;
        let end = start
            .checked_add(quote.chars().count())
            .ok_or("invalid_text_quote")?;
        evidence.push((
            candidate.id.as_str(),
            json!({
                "id": candidate.id, "inputCitationId": source.citation_id,
                "sourceRef": source.source_ref, "sourceVersion": source.source_version,
                "sourceKind": "material", "start": start, "end": end, "quote": quote
            }),
        ));
    }
    for candidate in &draft.evidence_candidates.vision {
        if !local_id(&candidate.id)
            || !sized(&candidate.input_citation_id, 1, 500)
            || !meaningful_optional(&candidate.model_quote)
            || !meaningful_optional(&candidate.observation)
            || (candidate.model_quote.is_none() && candidate.observation.is_none())
            || evidence_sources
                .insert(&candidate.id, Some(&candidate.input_citation_id))
                .is_some()
        {
            return Err("invalid_draft");
        }
        if !used.contains(candidate.id.as_str()) {
            continue;
        }
        let source = snapshot
            .images
            .iter()
            .find(|s| s.citation_id == candidate.input_citation_id)
            .ok_or("invalid_evidence_source")?;
        evidence.push((
            candidate.id.as_str(),
            json!({
                "id": candidate.id, "inputCitationId": source.citation_id,
                "sourceRef": source.source_ref, "sourceVersion": source.source_version,
                "sourceKind": "material", "anchorKind": "image", "blobSha256": source.blob_sha256,
                "modelQuote": candidate.model_quote, "observation": candidate.observation,
                "speakerRole": candidate.speaker_role, "verification": "model_read"
            }),
        ));
    }
    for unit in &core.knowledge_units {
        for id in &unit.evidence_refs {
            if !evidence_sources.contains_key(id.as_str()) {
                return Err("invalid_reference");
            }
        }
    }
    if draft.image_contributions.len() != snapshot.images.len() {
        return Err("invalid_image_contributions");
    }
    let mut contributed = BTreeSet::new();
    for contribution in &draft.image_contributions {
        if !snapshot
            .images
            .iter()
            .any(|s| s.citation_id == contribution.input_citation_id)
            || !contributed.insert(contribution.input_citation_id.as_str())
            || !sized(&contribution.summary, 1, 500)
            || contribution.summary.trim().is_empty()
            || !references(&contribution.unit_ids, 0, 12)
            || !references(&contribution.evidence_refs, 0, 8)
            || (contribution.status == ContributionStatus::Used
                && contribution.evidence_refs.is_empty())
        {
            return Err("invalid_image_contributions");
        }
        if !existing_units(&contribution.unit_ids) {
            return Err("invalid_reference");
        }
        for id in &contribution.evidence_refs {
            if evidence_sources.get(id.as_str()).copied().flatten()
                != Some(contribution.input_citation_id.as_str())
            {
                return Err("invalid_image_contributions");
            }
        }
    }
    let evidence: Vec<Value> = evidence.into_iter().map(|(_, value)| value).collect();
    if !(1..=64).contains(&evidence.len()) {
        return Err("invalid_draft");
    }
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
            "mainTopicUnitId": core.main_topic_unit_id,
            "audienceUnitIds": audience, "authorPositionUnitIds": author_position,
            "provisionUnitIds": provision, "readerValueUnitIds": reader_value,
            "knowledgeUnits": core.knowledge_units, "candidateLabels": candidate_labels
        },
        "deep": null, "evidence": evidence,
        "generation": {
            "methodVersion": METHOD, "skillVersionRef": generation.skill_version_ref,
            "promptVersionRef": generation.prompt_version_ref, "modelConfigRef": generation.model_config_ref,
            "providerReturnedModel": generation.provider_returned_model, "createdAt": generation.created_at
        }
    });
    Ok(json!({"card": card, "imageContributions": draft.image_contributions}))
}
