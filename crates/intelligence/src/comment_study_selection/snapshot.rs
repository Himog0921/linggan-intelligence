//! One SELECT snapshot, consumed in bounded FETCH windows. No provider calls or writes.
use super::*;
use crate::comment_cleaning::CLEANER_VERSION;
use crate::comment_study_run::{StudyStartError, bounded_context_manifest};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::time::Duration;

pub(crate) struct FrozenSelection {
    pub as_of: String,
    pub requested_works: Vec<Uuid>,
    pub contexts: BTreeMap<Uuid, Value>,
    pub rows: Vec<SelectionCandidate>,
    pub inputs: BTreeMap<usize, PreparedStudyInput>,
    pub selection: StudySelection,
    pub index_coverage: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SnapshotRow {
    comment_key: CommentKey,
    source_ref: Uuid,
    source_rank: u64,
    exclusion_reason: Option<String>,
    readable: bool,
    indexed: bool,
    in_progress: bool,
    latest: Option<SelectionHistory>,
    raw_text: Option<String>,
    raw_sha256: Option<String>,
    research_text: Option<String>,
    parent: Option<Value>,
}

type InputOrder = (u64, Uuid, String);
struct Scan {
    contexts: BTreeMap<Uuid, Value>,
    rows: Vec<SelectionCandidate>,
    retained: BTreeMap<InputOrder, (usize, PreparedStudyInput)>,
    eligible_ranks: BTreeMap<Uuid, u64>,
    indexed: u64,
    pending: u64,
}

pub(crate) fn requested_works(scope: &StudyScope) -> Vec<Uuid> {
    match scope {
        StudyScope::Works { work_refs } => work_refs.clone(),
        StudyScope::Comments { comment_keys } => comment_keys.iter().map(|k| k.work_ref)
            .collect::<BTreeSet<_>>().into_iter().collect(),
    }
}

pub(crate) fn freeze_sql() -> String {
    // The very same work-context SQL used by P1; no second SQL/Rust context builder.
    let context = include_str!("../comment_study_source/context.sql").trim_end().trim_end_matches(';')
        .replace("$1", "$4").replace("$2::timestamptz", "$2::text::timestamptz");
    format!("DECLARE cs_study_freeze INSENSITIVE NO SCROLL CURSOR WITHOUT HOLD FOR {}",
        include_str!("snapshot.sql")
            .replace("/*SHARED_WORK_CONTEXT_QUERY*/", &context)
            .replace("/*SOURCE_ELIGIBILITY_CASE*/", &crate::comment_study_source::gate::sql_case()))
}

pub(crate) async fn read_frozen_selection(
    tx: &mut Transaction<'_, Postgres>, command: &SelectionPreviewCommand, as_of: String,
) -> Result<FrozenSelection, StudyStartError> {
    tokio::time::timeout(Duration::from_secs(15), scan(tx, command, as_of)).await
        .map_err(|_| StudyStartError::QueryTimeout)?
}

async fn scan(
    tx: &mut Transaction<'_, Postgres>, command: &SelectionPreviewCommand, as_of: String,
) -> Result<FrozenSelection, StudyStartError> {
    let works = requested_works(&command.scope);
    let keys = match &command.scope {
        StudyScope::Comments { comment_keys } => Some(json!(comment_keys)),
        StudyScope::Works { .. } => None,
    };
    sqlx::query(sqlx::AssertSqlSafe(freeze_sql())).bind(command.domain_ref).bind(&as_of)
        .bind(CLEANER_VERSION).bind(&works).bind(keys).execute(&mut **tx).await?;
    let mut scan = Scan { contexts: BTreeMap::new(), rows: Vec::new(), retained: BTreeMap::new(),
        eligible_ranks: BTreeMap::new(), indexed: 0, pending: 0 };
    loop {
        // FETCH advances one cursor; it never opens another source snapshot.
        let page = sqlx::query("FETCH FORWARD 128 FROM cs_study_freeze").fetch_all(&mut **tx).await?;
        if page.is_empty() { break; }
        for row in page {
            let payload: Value = row.try_get("payload")?;
            match row.try_get::<i32, _>("kind")? {
                0 => scan.context(payload, command.limits.context_character_budget as usize)?,
                1 => scan.comment(payload, command)?,
                _ => return Err(StudySelectionError::InvalidSnapshot.into()),
            }
        }
    }
    sqlx::query("CLOSE cs_study_freeze").execute(&mut **tx).await?;
    if scan.contexts.keys().copied().collect::<Vec<_>>() != works {
        return Err(StudyStartError::NotFound);
    }
    if let StudyScope::Comments { comment_keys } = &command.scope
        && scan.rows.len() != comment_keys.len()
    { return Err(StudyStartError::NotFound); }
    let selection = choose_study_targets(command, &scan.rows)?;
    let inputs: BTreeMap<_, _> = scan.retained.into_values().collect();
    if inputs.len() != selection.target_count
        || selection.selected_indices.iter().any(|i| !inputs.contains_key(i))
    { return Err(StudySelectionError::InvalidSnapshot.into()); }
    Ok(FrozenSelection { index_coverage: json!({"state":if scan.pending==0 {"ready"} else {"partial"},
        "indexedCount":scan.indexed,"pendingCount":scan.pending,"asOf":as_of}),
        as_of, requested_works: works, contexts: scan.contexts, rows: scan.rows, inputs, selection })
}

impl Scan {
    fn context(&mut self, payload: Value, budget: usize) -> Result<(), StudyStartError> {
        let work: Uuid = serde_json::from_value(payload["workRef"].clone())
            .map_err(|_| StudySelectionError::InvalidSnapshot)?;
        if !payload["fragments"].is_array() || self.contexts.contains_key(&work) {
            return Err(StudySelectionError::InvalidSnapshot.into());
        }
        let manifest = json!({"contract":"comment-study.context.v1","workRef":work,
            "cleanerVersion":CLEANER_VERSION,"sources":payload["fragments"]});
        self.contexts.insert(work, bounded_context_manifest(&manifest, budget));
        Ok(())
    }

    fn comment(&mut self, payload: Value, command: &SelectionPreviewCommand) -> Result<(), StudyStartError> {
        let raw: SnapshotRow = serde_json::from_value(payload)
            .map_err(|_| StudySelectionError::InvalidSnapshot)?;
        if raw.readable { if raw.indexed { self.indexed += 1; } else { self.pending += 1; } }
        let mut flags = [false; 7];
        if let Some(reason) = &raw.exclusion_reason {
            let index = EXCLUSIONS[..7].iter().position(|code| code == reason)
                .ok_or(StudySelectionError::InvalidSnapshot)?;
            flags[index] = true;
        }
        let input = if raw.exclusion_reason.is_none() && !raw.in_progress {
            let context = self.contexts.get(&raw.comment_key.work_ref)
                .ok_or(StudySelectionError::InvalidSnapshot)?;
            let mut parent = raw.parent;
            // Overlong parent text is unusable context, never a truncated quote.
            if let Some(p) = &mut parent
                && p["commentText"].as_str().is_some_and(|v| v.chars().count()>16000)
            { p["commentText"] = Value::Null; }
            let input = prepare_study_input(&raw.comment_key, raw.source_ref,
                raw.raw_text.as_deref().ok_or(StudySelectionError::InvalidSnapshot)?, context, parent.as_ref())?;
            if raw.raw_sha256.as_deref() != Some(input.raw_sha256.as_str())
                || raw.research_text.as_deref() != Some(input.research_text.as_str())
            { return Err(StudySelectionError::InvalidSnapshot.into()); }
            Some(input)
        } else { None };
        let candidate = SelectionCandidate { comment_key:raw.comment_key, source_ref:raw.source_ref,
            source_rank:raw.source_rank, source_flags:flags, in_progress:raw.in_progress, latest:raw.latest,
            input_fingerprint:input.as_ref().map(|i|i.input_fingerprint.clone()) };
        if let Some(input) = input && selected_by_mode(command.mode, &candidate) {
            let rank = self.eligible_ranks.entry(candidate.comment_key.work_ref).or_default();
            let order = (*rank, candidate.comment_key.work_ref, candidate.comment_key.comment_external_id.clone());
            *rank += 1;
            self.retained.insert(order, (self.rows.len(), input));
            if self.retained.len()>command.limits.comment_budget as usize { self.retained.pop_last(); }
        }
        // Store metadata only; raw pages and non-selected prepared text are dropped here.
        self.rows.push(candidate);
        Ok(())
    }
}
