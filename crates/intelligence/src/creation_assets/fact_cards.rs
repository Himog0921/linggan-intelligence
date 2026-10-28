//! Product fact cards: the organisation's wording for the product. Revisions are immutable and
//! only a confirmed revision drives the checks; an older confirmed one stays in force until a
//! newer one is confirmed.
use super::requests::{Claim, claim, command_hash, lock, sql};
use super::subject::CardTerms;
use super::{AssetsError, types::*};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use std::collections::BTreeMap;

const SELECT: &str = "SELECT c.card_ref,c.code,c.revision_number,c.name,c.allowed_names,c.banned_names,c.what,c.who,c.can_say,c.cannot_say,c.must_note,k.confirmed_by,k.confirmed_at::text AS confirmed_at,c.saved_at::text AS saved_at,c.saved_by FROM linggan_creation_fact_card c LEFT JOIN linggan_creation_fact_card_confirmation k ON k.card_ref=c.card_ref";

fn from_row(row: &PgRow) -> Result<FactCard, AssetsError> {
    Ok(FactCard {
        code: row.try_get("code")?,
        revision: row.try_get("revision_number")?,
        name: row.try_get("name")?,
        allowed_names: row.try_get("allowed_names")?,
        banned_names: row.try_get("banned_names")?,
        what: row.try_get("what")?,
        who: row.try_get("who")?,
        can_say: row.try_get("can_say")?,
        cannot_say: row.try_get("cannot_say")?,
        must_note: row.try_get("must_note")?,
        confirmed_by: row.try_get("confirmed_by")?,
        confirmed_at: row.try_get("confirmed_at")?,
        saved_at: row.try_get("saved_at")?,
        saved_by: row.try_get("saved_by")?,
    })
}

const LATEST: &str = "c.revision_number=(SELECT max(revision_number) FROM linggan_creation_fact_card WHERE code=c.code)";
const LATEST_CONFIRMED: &str = "k.card_ref IS NOT NULL AND c.revision_number=(SELECT max(c2.revision_number) FROM linggan_creation_fact_card c2 JOIN linggan_creation_fact_card_confirmation k2 ON k2.card_ref=c2.card_ref WHERE c2.code=c.code)";

async fn confirmed_in(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<FactCard>, AssetsError> {
    let rows = sqlx::query(sql(format!(
        "{SELECT} WHERE {LATEST_CONFIRMED} ORDER BY c.code"
    )))
    .fetch_all(&mut **tx)
    .await?;
    rows.iter().map(from_row).collect()
}

/// The names of every confirmed card, or `None` while no card is confirmed.
pub(super) async fn confirmed_terms_in(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<Option<CardTerms>, AssetsError> {
    let cards = confirmed_in(tx).await?;
    let Some(first) = cards.first() else {
        return Ok(None);
    };
    let mut terms = CardTerms {
        canonical: first.name.clone(),
        ..CardTerms::default()
    };
    for card in &cards {
        for name in std::iter::once(&card.name).chain(&card.allowed_names) {
            if !terms.allowed.contains(name) {
                terms.allowed.push(name.clone());
            }
        }
        for name in &card.banned_names {
            if !terms.banned.contains(name) {
                terms.banned.push(name.clone());
            }
        }
        terms.revisions.push(CardRevision {
            code: card.code.clone(),
            revision: card.revision,
        });
    }
    Ok(Some(terms))
}

pub async fn list_fact_cards(db: &Database) -> Result<Vec<FactCardView>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let latest = sqlx::query(sql(format!("{SELECT} WHERE {LATEST} ORDER BY c.code")))
        .fetch_all(&mut *tx)
        .await?;
    let mut confirmed: BTreeMap<String, FactCard> = confirmed_in(&mut tx)
        .await?
        .into_iter()
        .map(|card| (card.code.clone(), card))
        .collect();
    tx.commit().await?;
    latest
        .iter()
        .map(|row| {
            let latest = from_row(row)?;
            Ok(FactCardView {
                confirmed: confirmed.remove(&latest.code),
                code: latest.code.clone(),
                latest,
            })
        })
        .collect()
}

fn clean_names(names: &[String]) -> Result<Vec<String>, AssetsError> {
    let mut out: Vec<String> = Vec::new();
    for name in names {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 40 || name.chars().any(char::is_control) {
            return Err(AssetsError::Invalid("names"));
        }
        if !out.iter().any(|seen| seen == name) {
            out.push(name.to_string());
        }
    }
    if out.len() > 20 {
        return Err(AssetsError::Invalid("names"));
    }
    Ok(out)
}

fn normalized(cmd: &FactCardCommand) -> Result<FactCardCommand, AssetsError> {
    let mut cmd = cmd.clone();
    cmd.name = cmd.name.trim().to_string();
    if cmd.expected_revision < 0
        || cmd.name.is_empty()
        || cmd.name.chars().count() > 80
        || cmd.name.chars().any(char::is_control)
    {
        return Err(AssetsError::Invalid("name"));
    }
    cmd.allowed_names = clean_names(&cmd.allowed_names)?;
    cmd.banned_names = clean_names(&cmd.banned_names)?;
    if cmd
        .banned_names
        .iter()
        .any(|banned| *banned == cmd.name || cmd.allowed_names.contains(banned))
    {
        return Err(AssetsError::Invalid("names_overlap"));
    }
    for text in [
        &mut cmd.what,
        &mut cmd.who,
        &mut cmd.can_say,
        &mut cmd.cannot_say,
        &mut cmd.must_note,
    ] {
        *text = text.trim().to_string();
        if text.chars().count() > 2000 {
            return Err(AssetsError::Invalid("text_size"));
        }
    }
    Ok(cmd)
}

async fn by_request(
    tx: &mut Transaction<'_, Postgres>,
    request: uuid::Uuid,
    confirmation: bool,
) -> Result<FactCard, AssetsError> {
    let key = if confirmation {
        "k.request_ref"
    } else {
        "c.request_ref"
    };
    let row = sqlx::query(sql(format!("{SELECT} WHERE {key}=$1")))
        .bind(request)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AssetsError::NotFound)?;
    from_row(&row)
}

async fn latest_revision_in(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<Option<i32>, AssetsError> {
    Ok(sqlx::query_scalar(
        "SELECT max(revision_number) FROM linggan_creation_fact_card WHERE code=$1",
    )
    .bind(code)
    .fetch_one(&mut **tx)
    .await?)
}

/// Adds a card (`code` is `None`, `expected_revision` 0) or its next revision. The new revision
/// is unconfirmed whatever its predecessor was.
pub async fn put_fact_card(
    db: &Database,
    code: Option<&str>,
    cmd: &FactCardCommand,
) -> Result<FactCardReceipt, AssetsError> {
    let cmd = normalized(cmd)?;
    if code.is_none() != (cmd.expected_revision == 0) {
        return Err(AssetsError::Invalid("expected_revision"));
    }
    let sha = command_hash(
        "fact_card",
        &(
            code,
            &cmd.expected_revision,
            &cmd.name,
            &cmd.allowed_names,
            &cmd.banned_names,
            &cmd.what,
            &cmd.who,
            &cmd.can_say,
            &cmd.cannot_say,
            &cmd.must_note,
        ),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "fact_card", &sha).await? {
        let card = by_request(&mut tx, cmd.request_id, false).await?;
        tx.commit().await?;
        return Ok(FactCardReceipt {
            card,
            replayed: true,
        });
    }
    let (code, revision) = match code {
        None => {
            let code: String = sqlx::query_scalar(
                "SELECT 'CP-'||lpad(nextval('linggan_creation_fact_card_code_seq')::text,4,'0')",
            )
            .fetch_one(&mut *tx)
            .await?;
            lock(&mut tx, &format!("fact_card:{code}")).await?;
            (code, 1)
        }
        Some(code) => {
            lock(&mut tx, &format!("fact_card:{code}")).await?;
            let latest = latest_revision_in(&mut tx, code)
                .await?
                .ok_or(AssetsError::NotFound)?;
            if latest != cmd.expected_revision {
                return Err(AssetsError::Stale);
            }
            (code.to_string(), latest + 1)
        }
    };
    sqlx::query(
        "INSERT INTO linggan_creation_fact_card(card_ref,code,revision_number,request_ref,name,allowed_names,banned_names,what,who,can_say,cannot_say,must_note,saved_by)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'local_machine')",
    )
    .bind(uuid::Uuid::new_v4())
    .bind(&code)
    .bind(revision)
    .bind(cmd.request_id)
    .bind(&cmd.name)
    .bind(&cmd.allowed_names)
    .bind(&cmd.banned_names)
    .bind(&cmd.what)
    .bind(&cmd.who)
    .bind(&cmd.can_say)
    .bind(&cmd.cannot_say)
    .bind(&cmd.must_note)
    .execute(&mut *tx)
    .await?;
    let card = by_request(&mut tx, cmd.request_id, false).await?;
    tx.commit().await?;
    Ok(FactCardReceipt {
        card,
        replayed: false,
    })
}

/// Confirms the latest revision of a card. Only a confirmed revision reaches the checks.
pub async fn confirm_fact_card(
    db: &Database,
    code: &str,
    cmd: &ConfirmCommand,
) -> Result<FactCardReceipt, AssetsError> {
    if cmd.expected_revision < 1 {
        return Err(AssetsError::Invalid("expected_revision"));
    }
    let sha = command_hash("fact_card_confirm", &(code, cmd.expected_revision))?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "fact_card_confirm", &sha).await? {
        let card = by_request(&mut tx, cmd.request_id, true).await?;
        tx.commit().await?;
        return Ok(FactCardReceipt {
            card,
            replayed: true,
        });
    }
    lock(&mut tx, &format!("fact_card:{code}")).await?;
    let latest = latest_revision_in(&mut tx, code)
        .await?
        .ok_or(AssetsError::NotFound)?;
    if latest != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let card_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT card_ref FROM linggan_creation_fact_card WHERE code=$1 AND revision_number=$2",
    )
    .bind(code)
    .bind(latest)
    .fetch_one(&mut *tx)
    .await?;
    let already: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_fact_card_confirmation WHERE card_ref=$1)",
    )
    .bind(card_ref)
    .fetch_one(&mut *tx)
    .await?;
    if already {
        return Err(AssetsError::Invalid("already_confirmed"));
    }
    sqlx::query(
        "INSERT INTO linggan_creation_fact_card_confirmation(card_ref,request_ref,confirmed_by) VALUES($1,$2,'local_machine')",
    )
    .bind(card_ref)
    .bind(cmd.request_id)
    .execute(&mut *tx)
    .await?;
    let card = by_request(&mut tx, cmd.request_id, true).await?;
    tx.commit().await?;
    Ok(FactCardReceipt {
        card,
        replayed: false,
    })
}
