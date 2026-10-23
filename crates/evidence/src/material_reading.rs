//! The single canonical, immutable material reading history used by research dossiers.
//! Producers append observations; a separately recorded selection owns current content.
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgConnection, Row};
use std::collections::BTreeMap;
use uuid::Uuid;

pub mod producers;
mod read;
mod review;
mod source;
mod write;
pub use read::{read_candidates, read_current_in, read_versions_in};
pub use review::review_reading;
pub use write::publish_in;

#[derive(Clone, Copy, Debug)]
pub enum Publication {
    Automatic { expected_current: Option<Uuid> },
    Candidate,
}

#[derive(Debug)]
pub struct NewReading {
    pub content_public_ref: Uuid,
    pub unit_key: String,
    pub kind: String,
    pub slot_key: Option<String>,
    pub page: Option<i32>,
    pub source_version: String,
    pub source_state: String,
    pub derivative_ref: Option<Uuid>,
    pub producer_ref: Option<Uuid>,
    pub generated_by: String,
    pub rule_version: String,
    pub blocks: Vec<Value>,
    pub issues: Vec<Value>,
    pub source_location: Value,
    pub publication: Publication,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedReading {
    pub unit_ref: Uuid,
    pub reading_ref: Uuid,
    pub current: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewReadingRequest {
    pub command_ref: Uuid,
    pub expected_current_revision_ref: Option<Uuid>,
    pub candidate_revision_ref: Uuid,
    pub action: String,
    pub edited_blocks: Option<Vec<Value>>,
    pub verified: bool,
    #[serde(default)]
    pub resolved_issue_keys: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadingError {
    #[error("invalid_reading_command")]
    Invalid,
    #[error("reading_not_found")]
    NotFound,
    #[error("reading_revision_conflict")]
    Conflict,
    #[error("reading_source_changed")]
    SourceChanged,
    #[error("reading_storage_unavailable")]
    Storage(#[from] sqlx::Error),
}

fn invalid(message: impl Into<String>) -> sqlx::Error {
    sqlx::Error::Protocol(message.into())
}

fn validate_content(blocks: &[Value], issues: &[Value]) -> Result<(), sqlx::Error> {
    use std::collections::BTreeSet;
    crate::research_dossier::content::validate_blocks(blocks).map_err(invalid)?;
    crate::research_dossier::content::validate_issues(&json!(issues)).map_err(invalid)?;
    let refs: BTreeSet<_> = blocks.iter().filter_map(|b| b["ref"].as_str()).collect();
    let mut keys = BTreeSet::new();
    for issue in issues.iter().chain(
        blocks
            .iter()
            .flat_map(|b| b["issues"].as_array().into_iter().flatten()),
    ) {
        let key = issue["key"]
            .as_str()
            .ok_or_else(|| invalid("reading_issue_key_missing"))?;
        if !keys.insert(key) {
            return Err(invalid("duplicate_reading_issue_key"));
        }
        if issue["blockRef"]
            .as_str()
            .is_some_and(|r| !refs.contains(r))
        {
            return Err(invalid("reading_issue_block_missing"));
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Unit {
    id: Uuid,
    work: Uuid,
    kind: String,
    slot: Option<String>,
    page: Option<i32>,
    source: String,
    state: String,
    current: Option<Uuid>,
    revision: i64,
    adoption: String,
    verification: String,
}

impl Unit {
    fn from_row(r: &sqlx::postgres::PgRow) -> Self {
        Self {
            id: r.get("unit_ref"),
            work: r.get("content_public_ref"),
            kind: r.get("kind"),
            slot: r.get("slot_key"),
            page: r.get("page_ordinal"),
            source: r.get("source_version"),
            state: r.get("source_state"),
            current: r.get("current_reading_ref"),
            revision: r.get("revision"),
            adoption: r.get("adoption"),
            verification: r.get("verification"),
        }
    }
}

async fn lock_work(conn: &mut PgConnection, work: Uuid) -> Result<(), sqlx::Error> {
    // Admission and normalization share this lock with their current-source selection.
    sqlx::query("SELECT pg_advisory_xact_lock(420,hashtext($1::text))")
        .bind(work.to_string())
        .execute(conn)
        .await?;
    Ok(())
}

async fn record_selection(
    conn: &mut PgConnection,
    unit: &Unit,
    event: &str,
    command: Option<Uuid>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO linggan_material_reading_selection(selection_ref,unit_ref,revision,event,source_version,source_state,reading_ref,page_ordinal,adoption,verification,command_ref) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)")
        .bind(Uuid::new_v4()).bind(unit.id).bind(unit.revision).bind(event)
        .bind(&unit.source).bind(&unit.state).bind(unit.current).bind(unit.page)
        .bind(&unit.adoption).bind(&unit.verification).bind(command).execute(conn).await?;
    Ok(())
}

async fn update_unit(conn: &mut PgConnection, unit: &mut Unit) -> Result<(), sqlx::Error> {
    unit.revision += 1;
    sqlx::query("UPDATE linggan_material_unit SET source_version=$2,source_state=$3,current_reading_ref=$4,revision=$5,adoption=$6,verification=$7,page_ordinal=$8,updated_at=scope_001_now() WHERE unit_ref=$1")
        .bind(unit.id).bind(&unit.source).bind(&unit.state).bind(unit.current).bind(unit.revision)
        .bind(&unit.adoption).bind(&unit.verification).bind(unit.page).execute(conn).await?;
    Ok(())
}

fn normalize_refs(work: Uuid, reading: Uuid, blocks: &mut [Value], issues: &mut [Value]) {
    let mut mappings = BTreeMap::new();
    for (index, block) in blocks.iter_mut().enumerate() {
        let reference = format!("{work}.{reading}.{index}");
        if let Some(previous) = block["ref"].as_str() {
            mappings.insert(previous.to_owned(), reference.clone());
        }
        block["ref"] = json!(reference);
    }
    for issue in issues {
        rewrite_issue_ref(issue, &mappings);
    }
    for block in blocks {
        if let Some(issues) = block.get_mut("issues").and_then(Value::as_array_mut) {
            for issue in issues {
                rewrite_issue_ref(issue, &mappings);
            }
        }
    }
}

fn rewrite_issue_ref(issue: &mut Value, mappings: &BTreeMap<String, String>) {
    if let Some(reference) = issue["blockRef"].as_str().and_then(|r| mappings.get(r)) {
        issue["blockRef"] = json!(reference);
    }
}
