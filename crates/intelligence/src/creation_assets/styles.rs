//! Writing styles: how an account writes, named once and reused across manuscripts. Revisions are
//! immutable; the account's current style is the newest revision of its newest style.
use super::requests::{Claim, claim, command_hash, lock, sql};
use super::{AssetsError, types::*};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

const SELECT: &str = "SELECT s.code,s.account_ref,s.revision_number,s.name,s.source_note,s.structure,s.rhythm,s.wording,s.skip,s.saved_at::text AS saved_at FROM linggan_creation_style s";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StyleCommand {
    pub request_id: Uuid,
    pub account_ref: Uuid,
    /// 0 adds a style; anything else adds the next revision of `code`.
    pub expected_revision: i32,
    pub name: String,
    pub source_note: String,
    pub structure: Vec<String>,
    pub rhythm: Vec<String>,
    pub wording: Vec<String>,
    pub skip: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationStyle {
    pub code: String,
    pub account_ref: Uuid,
    pub revision: i32,
    pub name: String,
    pub source_note: String,
    pub structure: Vec<String>,
    pub rhythm: Vec<String>,
    pub wording: Vec<String>,
    pub skip: Vec<String>,
    pub saved_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleReceipt {
    pub style: CreationStyle,
    pub replayed: bool,
}

fn from_row(row: &PgRow) -> Result<CreationStyle, AssetsError> {
    Ok(CreationStyle {
        code: row.try_get("code")?,
        account_ref: row.try_get("account_ref")?,
        revision: row.try_get("revision_number")?,
        name: row.try_get("name")?,
        source_note: row.try_get("source_note")?,
        structure: row.try_get("structure")?,
        rhythm: row.try_get("rhythm")?,
        wording: row.try_get("wording")?,
        skip: row.try_get("skip")?,
        saved_at: row.try_get("saved_at")?,
    })
}

fn lines(items: &[String]) -> Result<Vec<String>, AssetsError> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        let item = item.trim();
        if item.is_empty() || item.chars().count() > 200 || item.chars().any(char::is_control) {
            return Err(AssetsError::Invalid("style_line"));
        }
        if !out.iter().any(|seen| seen == item) {
            out.push(item.to_string());
        }
    }
    if out.len() > 12 {
        return Err(AssetsError::Invalid("style_line"));
    }
    Ok(out)
}

fn normalized(cmd: &StyleCommand) -> Result<StyleCommand, AssetsError> {
    let mut cmd = cmd.clone();
    cmd.name = cmd.name.trim().to_string();
    cmd.source_note = cmd.source_note.trim().to_string();
    if cmd.account_ref.is_nil()
        || cmd.expected_revision < 0
        || cmd.name.is_empty()
        || cmd.name.chars().count() > 80
        || cmd.name.chars().any(char::is_control)
        || cmd.source_note.chars().count() > 200
    {
        return Err(AssetsError::Invalid("style_name"));
    }
    cmd.structure = lines(&cmd.structure)?;
    cmd.rhythm = lines(&cmd.rhythm)?;
    cmd.wording = lines(&cmd.wording)?;
    cmd.skip = lines(&cmd.skip)?;
    Ok(cmd)
}

async fn by_request(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<CreationStyle, AssetsError> {
    let row = sqlx::query(sql(format!("{SELECT} WHERE s.request_ref=$1")))
        .bind(request)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(AssetsError::NotFound)?;
    from_row(&row)
}

async fn latest_in(
    tx: &mut Transaction<'_, Postgres>,
    code: &str,
) -> Result<Option<CreationStyle>, AssetsError> {
    let row = sqlx::query(sql(format!(
        "{SELECT} WHERE s.code=$1 ORDER BY s.revision_number DESC LIMIT 1"
    )))
    .bind(code)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(from_row).transpose()
}

/// The newest revision of every style, newest first. `domain` limits the list to accounts in it.
pub async fn list_styles(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<Vec<CreationStyle>, AssetsError> {
    let rows = sqlx::query(sql(format!(
        "{SELECT} JOIN linggan_ops_account a ON a.account_ref=s.account_ref \
         WHERE s.revision_number=(SELECT max(r.revision_number) FROM linggan_creation_style r WHERE r.code=s.code) \
         AND ($1::uuid IS NULL OR a.domain_ref=$1) \
         ORDER BY s.saved_at DESC,s.code DESC LIMIT 200"
    )))
    .bind(domain)
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(from_row).collect()
}

/// Adds a style (`code` is `None`, `expected_revision` 0) or its next revision. The account of a
/// style never changes once it exists.
pub async fn put_style(
    db: &Database,
    code: Option<&str>,
    cmd: &StyleCommand,
) -> Result<StyleReceipt, AssetsError> {
    let cmd = normalized(cmd)?;
    if code.is_none() != (cmd.expected_revision == 0) {
        return Err(AssetsError::Invalid("expected_revision"));
    }
    let sha = command_hash(
        "style",
        &(
            code,
            &cmd.account_ref,
            &cmd.expected_revision,
            &cmd.name,
            &cmd.source_note,
            &cmd.structure,
            &cmd.rhythm,
            &cmd.wording,
            &cmd.skip,
        ),
    )?;
    let kind = if code.is_none() {
        "style_create"
    } else {
        "style_revise"
    };
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, kind, &sha).await? {
        let style = by_request(&mut tx, cmd.request_id).await?;
        tx.commit().await?;
        return Ok(StyleReceipt {
            style,
            replayed: true,
        });
    }
    let (code, revision) = match code {
        None => {
            let known: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM linggan_ops_account WHERE account_ref=$1)",
            )
            .bind(cmd.account_ref)
            .fetch_one(&mut *tx)
            .await?;
            if !known {
                return Err(AssetsError::NotFound);
            }
            let code: String = sqlx::query_scalar(
                "SELECT 'FG-'||lpad(nextval('linggan_creation_style_code_seq')::text,4,'0')",
            )
            .fetch_one(&mut *tx)
            .await?;
            lock(&mut tx, &format!("style:{code}")).await?;
            (code, 1)
        }
        Some(code) => {
            lock(&mut tx, &format!("style:{code}")).await?;
            let latest = latest_in(&mut tx, code)
                .await?
                .ok_or(AssetsError::NotFound)?;
            if latest.revision != cmd.expected_revision {
                return Err(AssetsError::Stale);
            }
            if latest.account_ref != cmd.account_ref {
                return Err(AssetsError::Invalid("style_account"));
            }
            (code.to_string(), latest.revision + 1)
        }
    };
    sqlx::query(
        "INSERT INTO linggan_creation_style(style_ref,code,account_ref,revision_number,request_ref,name,source_note,structure,rhythm,wording,skip,saved_by)
         VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'local_machine')",
    )
    .bind(Uuid::new_v4())
    .bind(&code)
    .bind(cmd.account_ref)
    .bind(revision)
    .bind(cmd.request_id)
    .bind(&cmd.name)
    .bind(&cmd.source_note)
    .bind(&cmd.structure)
    .bind(&cmd.rhythm)
    .bind(&cmd.wording)
    .bind(&cmd.skip)
    .execute(&mut *tx)
    .await?;
    let style = by_request(&mut tx, cmd.request_id).await?;
    tx.commit().await?;
    Ok(StyleReceipt {
        style,
        replayed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> StyleCommand {
        StyleCommand {
            request_id: Uuid::new_v4(),
            account_ref: Uuid::new_v4(),
            expected_revision: 0,
            name: "她的原声".into(),
            source_note: String::new(),
            structure: vec!["先说结论".into()],
            rhythm: vec![],
            wording: vec![],
            skip: vec![],
        }
    }

    #[test]
    fn a_style_needs_a_name_and_at_most_twelve_lines_per_kind() {
        assert!(normalized(&command()).is_ok());
        let blank = StyleCommand {
            name: "   ".into(),
            ..command()
        };
        assert!(normalized(&blank).is_err());
        let long = StyleCommand {
            structure: (0..13).map(|i| format!("第{i}条")).collect(),
            ..command()
        };
        assert!(normalized(&long).is_err());
    }

    #[test]
    fn repeated_lines_are_kept_once_and_trimmed() {
        let style = normalized(&StyleCommand {
            wording: vec![" 她说 ".into(), "她说".into()],
            ..command()
        })
        .expect("valid");
        assert_eq!(style.wording, vec!["她说".to_string()]);
    }
}
