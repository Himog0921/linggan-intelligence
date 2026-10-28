//! The bridge sentence library. Wording is append-only revisions; review state is append-only
//! status events, and a revision without any event is waiting for review. Nothing is ever
//! generated here: sentences are written and approved by people.
use super::checks::{blocks_entry, check_sentence};
use super::fact_cards::confirmed_terms_in;
use super::identity::latest_in as latest_identity_in;
use super::requests::{Claim, claim, command_hash, lock, sql};
use super::{AssetsError, types::*};
use crate::content_workbench::BridgeGroup;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

const SELECT: &str = "SELECT s.sentence_ref,s.code,s.revision_number,s.bridge_group,s.body,s.origin,s.entry_findings,s.saved_at::text AS saved_at,s.saved_by,COALESCE(st.status,'review') AS status,st.note AS status_note,
 (SELECT count(*) FROM linggan_creation_work w JOIN linggan_creation_revision r ON r.revision_ref=w.current_revision_ref WHERE r.document->'bridge'->'sentence'->>'sentenceCode'=s.code AND COALESCE((r.document->'bridge'->>'skip')::boolean,false)=false) AS used_count
 FROM linggan_creation_bridge_sentence s
 LEFT JOIN LATERAL (SELECT status,note FROM linggan_creation_bridge_sentence_status WHERE sentence_ref=s.sentence_ref ORDER BY seq DESC LIMIT 1) st ON true";
const LATEST: &str = "s.revision_number=(SELECT max(revision_number) FROM linggan_creation_bridge_sentence WHERE code=s.code)";

fn from_row(row: &PgRow) -> Result<Sentence, AssetsError> {
    Ok(Sentence {
        code: row.try_get("code")?,
        revision: row.try_get("revision_number")?,
        group: group_from_db(&row.try_get::<String, _>("bridge_group")?)
            .ok_or(AssetsError::Invalid("group"))?,
        text: row.try_get("body")?,
        status: SentenceStatus::from_db(&row.try_get::<String, _>("status")?)
            .ok_or(AssetsError::Invalid("status"))?,
        status_note: row.try_get("status_note")?,
        origin: SentenceOrigin::from_db(&row.try_get::<String, _>("origin")?)
            .ok_or(AssetsError::Invalid("origin"))?,
        entry_findings: serde_json::from_value(row.try_get("entry_findings")?)?,
        used_count: row.try_get("used_count")?,
        saved_at: row.try_get("saved_at")?,
        saved_by: row.try_get("saved_by")?,
    })
}

/// Latest revision of every sentence, optionally narrowed by voice and status, in code order.
pub async fn list_sentences(
    db: &Database,
    group: Option<BridgeGroup>,
    status: Option<SentenceStatus>,
) -> Result<Vec<Sentence>, AssetsError> {
    let rows = sqlx::query(sql(format!(
        "{SELECT} WHERE {LATEST} AND ($1::text IS NULL OR s.bridge_group=$1) AND ($2::text IS NULL OR COALESCE(st.status,'review')=$2) ORDER BY s.code"
    )))
    .bind(group.map(group_str))
    .bind(status.map(SentenceStatus::as_str))
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(from_row).collect()
}

fn clean_text(text: &str) -> Result<String, AssetsError> {
    let text = text.trim().to_string();
    if text.is_empty() || text.chars().count() > 500 || text.chars().any(char::is_control) {
        return Err(AssetsError::Invalid("text"));
    }
    Ok(text)
}

/// Entry checks for a sentence that is about to be added; a cure promise refuses the entry.
async fn entry_findings_in(
    tx: &mut Transaction<'_, Postgres>,
    text: &str,
) -> Result<Vec<Finding>, AssetsError> {
    let cards = confirmed_terms_in(tx).await?;
    let (findings, _) = check_sentence(text, cards.as_ref());
    if blocks_entry(&findings) {
        return Err(AssetsError::Blocked(findings));
    }
    Ok(findings)
}

/// The check shown while a sentence is typed, before anything is saved.
pub async fn check_sentence_text(
    db: &Database,
    req: &SentenceCheckRequest,
) -> Result<SentenceCheckReport, AssetsError> {
    if req.text.chars().count() > 2000 {
        return Err(AssetsError::Invalid("text"));
    }
    let mut tx = db.pool().begin().await?;
    let cards = confirmed_terms_in(&mut tx).await?;
    tx.commit().await?;
    let (findings, not_run) = check_sentence(req.text.trim(), cards.as_ref());
    Ok(SentenceCheckReport {
        ruleset_version: super::words::RULESET_VERSION.into(),
        blocked: blocks_entry(&findings),
        findings,
        not_run,
    })
}

async fn by_request(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    status_event: bool,
) -> Result<Sentence, AssetsError> {
    let sql_text = if status_event {
        format!(
            "{SELECT} WHERE s.sentence_ref=(SELECT sentence_ref FROM linggan_creation_bridge_sentence_status WHERE request_ref=$1)"
        )
    } else {
        format!("{SELECT} WHERE s.request_ref=$1")
    };
    let row = sqlx::query(sql(sql_text))
        .bind(request)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AssetsError::NotFound)?;
    from_row(&row)
}

async fn latest_in(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<Sentence, AssetsError> {
    let row = sqlx::query(sql(format!("{SELECT} WHERE s.code=$1 AND {LATEST}")))
        .bind(code)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AssetsError::NotFound)?;
    from_row(&row)
}

struct NewRevision<'a> {
    code: &'a str,
    revision: i32,
    request: Uuid,
    group: BridgeGroup,
    text: &'a str,
    origin: SentenceOrigin,
    findings: &'a [Finding],
}

async fn insert_revision(
    tx: &mut Transaction<'_, Postgres>,
    new: NewRevision<'_>,
) -> Result<(), AssetsError> {
    sqlx::query(
        "INSERT INTO linggan_creation_bridge_sentence(sentence_ref,code,revision_number,request_ref,bridge_group,body,origin,entry_findings,saved_by)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,'local_machine')",
    )
    .bind(Uuid::new_v4())
    .bind(new.code)
    .bind(new.revision)
    .bind(new.request)
    .bind(group_str(new.group))
    .bind(new.text)
    .bind(new.origin.as_str())
    .bind(serde_json::to_value(new.findings)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Adds a sentence to the library as revision 1, waiting for review.
pub async fn create_sentence(
    db: &Database,
    cmd: &SentenceCreate,
) -> Result<SentenceReceipt, AssetsError> {
    let text = clean_text(&cmd.text)?;
    let sha = command_hash(
        "sentence_create",
        &(group_str(cmd.group), &text, cmd.origin.as_str()),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "sentence_create", &sha).await? {
        let sentence = by_request(&mut tx, cmd.request_id, false).await?;
        tx.commit().await?;
        return Ok(SentenceReceipt {
            sentence,
            replayed: true,
        });
    }
    let findings = entry_findings_in(&mut tx, &text).await?;
    let code: String = sqlx::query_scalar(
        "SELECT 'XJ-'||lpad(nextval('linggan_creation_bridge_sentence_code_seq')::text,4,'0')",
    )
    .fetch_one(&mut *tx)
    .await?;
    lock(&mut tx, &format!("sentence:{code}")).await?;
    insert_revision(
        &mut tx,
        NewRevision {
            code: &code,
            revision: 1,
            request: cmd.request_id,
            group: cmd.group,
            text: &text,
            origin: cmd.origin,
            findings: &findings,
        },
    )
    .await?;
    let sentence = by_request(&mut tx, cmd.request_id, false).await?;
    tx.commit().await?;
    Ok(SentenceReceipt {
        sentence,
        replayed: false,
    })
}

/// Rewrites a sentence as its next revision; any rewrite goes back to review.
pub async fn edit_sentence(
    db: &Database,
    code: &str,
    cmd: &SentenceEdit,
) -> Result<SentenceReceipt, AssetsError> {
    let text = clean_text(&cmd.text)?;
    if cmd.expected_revision < 1 {
        return Err(AssetsError::Invalid("expected_revision"));
    }
    let sha = command_hash(
        "sentence_edit",
        &(code, cmd.expected_revision, group_str(cmd.group), &text),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "sentence_edit", &sha).await? {
        let sentence = by_request(&mut tx, cmd.request_id, false).await?;
        tx.commit().await?;
        return Ok(SentenceReceipt {
            sentence,
            replayed: true,
        });
    }
    lock(&mut tx, &format!("sentence:{code}")).await?;
    let current = latest_in(&mut tx, code).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let findings = entry_findings_in(&mut tx, &text).await?;
    insert_revision(
        &mut tx,
        NewRevision {
            code,
            revision: current.revision + 1,
            request: cmd.request_id,
            group: cmd.group,
            text: &text,
            origin: current.origin,
            findings: &findings,
        },
    )
    .await?;
    let sentence = by_request(&mut tx, cmd.request_id, false).await?;
    tx.commit().await?;
    Ok(SentenceReceipt {
        sentence,
        replayed: false,
    })
}

fn next_status(from: SentenceStatus, action: ReviewAction) -> Option<SentenceStatus> {
    use SentenceStatus as S;
    match (action, from) {
        (ReviewAction::Approve, S::Review | S::Fix) => Some(S::Ok),
        (ReviewAction::RequestFix, S::Review | S::Ok) => Some(S::Fix),
        (ReviewAction::Disable, S::Review | S::Ok | S::Fix) => Some(S::Off),
        (ReviewAction::Enable, S::Off) => Some(S::Review),
        _ => None,
    }
}

/// Approves, sends back, disables or re-enables the latest revision of a sentence.
pub async fn review_sentence(
    db: &Database,
    code: &str,
    cmd: &SentenceReview,
) -> Result<SentenceReceipt, AssetsError> {
    let note = cmd
        .note
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    if cmd.expected_revision < 1
        || note
            .as_deref()
            .is_some_and(|n| n.chars().count() > 500 || n.chars().any(char::is_control))
        || (cmd.action == ReviewAction::RequestFix && note.is_none())
    {
        return Err(AssetsError::Invalid("review"));
    }
    let sha = command_hash(
        "sentence_review",
        &(code, cmd.expected_revision, cmd.action, &note),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "sentence_review", &sha).await? {
        let sentence = by_request(&mut tx, cmd.request_id, true).await?;
        tx.commit().await?;
        return Ok(SentenceReceipt {
            sentence,
            replayed: true,
        });
    }
    lock(&mut tx, &format!("sentence:{code}")).await?;
    let current = latest_in(&mut tx, code).await?;
    if current.revision != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let to = next_status(current.status, cmd.action).ok_or(AssetsError::Invalid("transition"))?;
    let sentence_ref: Uuid = sqlx::query_scalar(
        "SELECT sentence_ref FROM linggan_creation_bridge_sentence WHERE code=$1 AND revision_number=$2",
    )
    .bind(code)
    .bind(current.revision)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO linggan_creation_bridge_sentence_status(status_ref,sentence_ref,seq,request_ref,status,note,set_by)
         VALUES($1,$2,(SELECT COALESCE(max(seq),0)+1 FROM linggan_creation_bridge_sentence_status WHERE sentence_ref=$2),$3,$4,$5,'local_machine')",
    )
    .bind(Uuid::new_v4())
    .bind(sentence_ref)
    .bind(cmd.request_id)
    .bind(to.as_str())
    .bind(&note)
    .execute(&mut *tx)
    .await?;
    let sentence = by_request(&mut tx, cmd.request_id, true).await?;
    tx.commit().await?;
    Ok(SentenceReceipt {
        sentence,
        replayed: false,
    })
}

/// The approved sentences an account may use, in the order "换一句" cycles through them. Empty
/// unless the account is confirmed, has a relation, and says so in its bio.
pub async fn candidates_for_account(
    db: &Database,
    account: Uuid,
) -> Result<Candidates, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let identity = latest_identity_in(&mut tx, account).await?;
    tx.commit().await?;
    let state = bridge_state_of(identity.as_ref(), true, false);
    let group = identity.as_ref().and_then(|i| i.org_relation.group());
    let items = match (state, group) {
        (BridgeState::Ready, Some(group)) => {
            list_sentences(db, Some(group), Some(SentenceStatus::Ok)).await?
        }
        _ => vec![],
    };
    Ok(Candidates {
        state,
        group,
        items,
    })
}
