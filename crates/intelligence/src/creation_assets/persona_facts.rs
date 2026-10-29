//! Subject-specific persona facts. Text and request fingerprints are erasable on withdrawal.
use super::requests::{command_hash, lock};
use super::{AssetsError, types::*};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<PersonaFact, AssetsError> {
    let row = sqlx::query("SELECT f.fact_ref,f.account_ref,f.fact_key,f.version,f.current_revision,f.state,COALESCE((SELECT p.subject_ref=f.subject_ref FROM linggan_creation_account_person p WHERE p.account_ref=f.account_ref),false) AND (SELECT i.has_real_person FROM linggan_creation_account_identity i WHERE i.account_ref=f.account_ref ORDER BY i.revision_number DESC LIMIT 1) IS TRUE AS current_person,r.text,r.source,r.status,r.confirmation_method FROM linggan_creation_persona_fact f LEFT JOIN linggan_creation_persona_fact_revision r ON r.fact_ref=f.fact_ref AND r.revision=f.current_revision WHERE f.fact_ref=$1")
        .bind(id).fetch_optional(&mut **tx).await?.ok_or(AssetsError::NotFound)?;
    let current_person: bool = row.try_get("current_person")?;
    let state: String = row.try_get("state")?;
    let visible = current_person && state == "active";
    Ok(PersonaFact {
        id: row.try_get("fact_ref")?,
        account_ref: row.try_get("account_ref")?,
        key: if visible {
            row.try_get("fact_key")?
        } else {
            None
        },
        version: row.try_get("version")?,
        current_revision: row.try_get("current_revision")?,
        current_person,
        state,
        text: if visible { row.try_get("text")? } else { None },
        source: if visible {
            row.try_get("source")?
        } else {
            None
        },
        status: if visible {
            row.try_get("status")?
        } else {
            None
        },
        confirmation_method: if visible {
            row.try_get("confirmation_method")?
        } else {
            None
        },
    })
}

async fn previous(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    kind: &str,
    hash: &str,
) -> Result<Option<PersonaFactReceipt>, AssetsError> {
    lock(tx, &format!("persona-request:{request}")).await?;
    let cancelled: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_persona_fact_request_cancel WHERE request_ref=$1)").bind(request).fetch_one(&mut **tx).await?;
    if cancelled {
        return Err(AssetsError::Invalid("request_cancelled"));
    }
    let row = sqlx::query("SELECT fact_ref,kind,command_sha256,result_version FROM linggan_creation_persona_fact_request WHERE request_ref=$1")
        .bind(request).fetch_optional(&mut **tx).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored: Option<String> = row.try_get("command_sha256")?;
    if stored.is_none() {
        return Err(AssetsError::NotFound);
    }
    if row.try_get::<String, _>("kind")? != kind || stored.as_deref() != Some(hash) {
        return Err(AssetsError::RequestConflict);
    }
    lock(tx, "experience-gate").await?;
    let fact = snapshot(tx, row.try_get("fact_ref")?).await?;
    if kind != "withdraw" && (!fact.current_person || fact.state != "active") {
        return Err(AssetsError::SourceUnavailable);
    }
    Ok(Some(PersonaFactReceipt {
        fact,
        result_version: row.try_get("result_version")?,
        replayed: true,
    }))
}

pub async fn persona_request_status(
    db: &Database,
    request: Uuid,
) -> Result<Option<serde_json::Value>, AssetsError> {
    let row = sqlx::query("SELECT kind,fact_ref,result_version FROM linggan_creation_persona_fact_request WHERE request_ref=$1").bind(request).fetch_optional(db.pool()).await?;
    row.map(|row| -> Result<_,AssetsError> { Ok(serde_json::json!({"kind":row.try_get::<String,_>("kind")?,"factRef":row.try_get::<Uuid,_>("fact_ref")?,"version":row.try_get::<i32,_>("result_version")?})) }).transpose()
}

pub async fn cancel_persona_request(
    db: &Database,
    request: Uuid,
) -> Result<serde_json::Value, AssetsError> {
    let mut tx = db.pool().begin().await?;
    lock(&mut tx, &format!("persona-request:{request}")).await?;
    let committed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_persona_fact_request WHERE request_ref=$1)",
    )
    .bind(request)
    .fetch_one(&mut *tx)
    .await?;
    if !committed {
        sqlx::query("INSERT INTO linggan_creation_persona_fact_request_cancel(request_ref) VALUES($1) ON CONFLICT DO NOTHING").bind(request).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(serde_json::json!({"outcome":if committed {"committed"} else {"cancelled"}}))
}

async fn record(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
    kind: &str,
    hash: &str,
    fact: &PersonaFact,
) -> Result<(), AssetsError> {
    sqlx::query("INSERT INTO linggan_creation_persona_fact_request(request_ref,fact_ref,kind,command_sha256,result_version) VALUES($1,$2,$3,$4,$5)")
        .bind(request).bind(fact.id).bind(kind).bind(hash).bind(fact.version).execute(&mut **tx).await?;
    Ok(())
}

pub async fn list_persona_facts(
    db: &Database,
    account: Uuid,
) -> Result<Vec<PersonaFact>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    lock(&mut tx, "experience-gate").await?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT fact_ref FROM linggan_creation_persona_fact WHERE account_ref=$1 ORDER BY created_at DESC,fact_ref LIMIT 200")
        .bind(account).fetch_all(&mut *tx).await?;
    let mut items = Vec::with_capacity(ids.len());
    for id in ids {
        items.push(snapshot(&mut tx, id).await?);
    }
    tx.commit().await?;
    Ok(items)
}

pub async fn persona_fact_history(
    db: &Database,
    id: Uuid,
) -> Result<Vec<PersonaFactRevision>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    lock(&mut tx, "experience-gate").await?;
    let fact = snapshot(&mut tx, id).await?;
    if !fact.current_person || fact.state != "active" {
        return Err(AssetsError::SourceUnavailable);
    }
    let rows = sqlx::query("SELECT revision,text,source,status,applied,recorded_at::text AS recorded_at FROM linggan_creation_persona_fact_revision WHERE fact_ref=$1 ORDER BY revision DESC LIMIT 200")
        .bind(id).fetch_all(&mut *tx).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(PersonaFactRevision {
            revision: row.try_get("revision")?,
            text: row.try_get("text")?,
            source: row.try_get("source")?,
            status: row.try_get("status")?,
            applied: row.try_get("applied")?,
            recorded_at: row.try_get("recorded_at")?,
        });
    }
    tx.commit().await?;
    Ok(out)
}

pub async fn put_persona_fact(
    db: &Database,
    cmd: &PersonaFactPut,
) -> Result<PersonaFactReceipt, AssetsError> {
    let key = cmd.key.trim();
    let text = cmd.text.trim();
    if key.is_empty()
        || key.chars().count() > 40
        || text.is_empty()
        || text.chars().count() > 500
        || cmd.expected_version < 0
        || !matches!(
            cmd.source.as_str(),
            "oral" | "manuscript" | "manual" | "inferred"
        )
        || !matches!(
            cmd.conflict_decision.as_deref(),
            None | Some("keep_old") | Some("use_new")
        )
    {
        return Err(AssetsError::Invalid("persona_fact"));
    }
    let hash = command_hash(
        "persona_put",
        &(
            cmd.account_ref,
            cmd.expected_person_ref,
            key,
            cmd.expected_version,
            text,
            &cmd.source,
            &cmd.conflict_decision,
        ),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Some(receipt) = previous(&mut tx, cmd.request_id, "put", &hash).await? {
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, "experience-gate").await?;
    lock(&mut tx, &format!("identity:{}", cmd.account_ref)).await?;
    let identity = super::identity::latest_in(&mut tx, cmd.account_ref).await?;
    if identity.is_none_or(|value| value.has_real_person != Some(true)) {
        return Err(AssetsError::Invalid("account_has_no_confirmed_person"));
    }
    let subject: Uuid = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(cmd.account_ref)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AssetsError::Invalid("person_unavailable"))?;
    if subject != cmd.expected_person_ref {
        return Err(AssetsError::Stale);
    }
    let existing: Option<Uuid> = sqlx::query_scalar("SELECT fact_ref FROM linggan_creation_persona_fact WHERE subject_ref=$1 AND fact_key=$2 AND state='active'")
        .bind(subject).bind(key).fetch_optional(&mut *tx).await?;
    let status = if cmd.source == "inferred" {
        "inferred"
    } else {
        "new"
    };
    let id = if let Some(id) = existing {
        let old = snapshot(&mut tx, id).await?;
        if old.version != cmd.expected_version {
            return Err(AssetsError::Stale);
        }
        if old.text.as_deref() == Some(text) && old.source.as_deref() == Some(&cmd.source) {
            record(&mut tx, cmd.request_id, "put", &hash, &old).await?;
            tx.commit().await?;
            return Ok(PersonaFactReceipt {
                result_version: old.version,
                fact: old,
                replayed: false,
            });
        }
        if old.text.as_deref() != Some(text) && cmd.conflict_decision.is_none() {
            return Err(AssetsError::Invalid("persona_conflict_decision"));
        }
        let revision = old
            .version
            .checked_add(1)
            .ok_or(AssetsError::Invalid("persona_version"))?;
        let applied = cmd.conflict_decision.as_deref() != Some("keep_old");
        sqlx::query("INSERT INTO linggan_creation_persona_fact_revision(fact_ref,revision,text,source,status,applied) VALUES($1,$2,$3,$4,$5,$6)")
            .bind(id).bind(revision).bind(text).bind(&cmd.source).bind(status).bind(applied).execute(&mut *tx).await?;
        sqlx::query("UPDATE linggan_creation_persona_fact SET version=$2,current_revision=CASE WHEN $3 THEN $2 ELSE current_revision END WHERE fact_ref=$1")
            .bind(id).bind(revision).bind(applied).execute(&mut *tx).await?;
        id
    } else {
        if cmd.expected_version != 0 {
            return Err(AssetsError::Stale);
        }
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO linggan_creation_persona_fact(fact_ref,account_ref,subject_ref,fact_key,version,current_revision,state) VALUES($1,$2,$3,$4,1,1,'active')")
            .bind(id).bind(cmd.account_ref).bind(subject).bind(key).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO linggan_creation_persona_fact_revision(fact_ref,revision,text,source,status,applied) VALUES($1,1,$2,$3,$4,true)")
            .bind(id).bind(text).bind(&cmd.source).bind(status).execute(&mut *tx).await?;
        id
    };
    let fact = snapshot(&mut tx, id).await?;
    record(&mut tx, cmd.request_id, "put", &hash, &fact).await?;
    tx.commit().await?;
    Ok(PersonaFactReceipt {
        result_version: fact.version,
        fact,
        replayed: false,
    })
}

pub async fn confirm_persona_fact(
    db: &Database,
    id: Uuid,
    cmd: &PersonaFactConfirm,
) -> Result<PersonaFactReceipt, AssetsError> {
    if !cmd.subject_confirmed
        || !matches!(
            cmd.confirmation_method.as_str(),
            "subject_present" | "subject_statement"
        )
    {
        return Err(AssetsError::Invalid("subject_consent"));
    }
    let hash = command_hash(
        "persona_confirm",
        &(
            id,
            cmd.expected_version,
            cmd.subject_confirmed,
            &cmd.confirmation_method,
        ),
    )?;
    let mut tx = db.pool().begin().await?;
    if let Some(receipt) = previous(&mut tx, cmd.request_id, "confirm", &hash).await? {
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, "experience-gate").await?;
    let old = snapshot(&mut tx, id).await?;
    if !old.current_person || old.state != "active" {
        return Err(AssetsError::SourceUnavailable);
    }
    if old.version != cmd.expected_version {
        return Err(AssetsError::Stale);
    }
    let revision = old
        .version
        .checked_add(1)
        .ok_or(AssetsError::Invalid("persona_version"))?;
    sqlx::query("INSERT INTO linggan_creation_persona_fact_revision(fact_ref,revision,text,source,status,applied,confirmation_method) VALUES($1,$2,$3,$4,'confirmed',true,$5)")
        .bind(id).bind(revision).bind(old.text.ok_or(AssetsError::SourceUnavailable)?).bind(old.source.ok_or(AssetsError::SourceUnavailable)?).bind(&cmd.confirmation_method).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE linggan_creation_persona_fact SET version=$2,current_revision=$2 WHERE fact_ref=$1",
    )
    .bind(id)
    .bind(revision)
    .execute(&mut *tx)
    .await?;
    let fact = snapshot(&mut tx, id).await?;
    record(&mut tx, cmd.request_id, "confirm", &hash, &fact).await?;
    tx.commit().await?;
    Ok(PersonaFactReceipt {
        result_version: fact.version,
        fact,
        replayed: false,
    })
}

pub async fn withdraw_persona_fact(
    db: &Database,
    id: Uuid,
    request: Uuid,
) -> Result<PersonaFactReceipt, AssetsError> {
    let hash = command_hash("persona_withdraw", &id)?;
    let mut tx = db.pool().begin().await?;
    if let Some(receipt) = previous(&mut tx, request, "withdraw", &hash).await? {
        tx.commit().await?;
        return Ok(receipt);
    }
    lock(&mut tx, "experience-gate").await?;
    let old = snapshot(&mut tx, id).await?;
    if old.state != "active" {
        return Err(AssetsError::Invalid("persona_state"));
    }
    sqlx::query("DELETE FROM linggan_creation_persona_fact_revision WHERE fact_ref=$1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE linggan_creation_persona_fact_request SET command_sha256=NULL WHERE fact_ref=$1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE linggan_creation_persona_fact SET state='withdrawn',fact_key=NULL,withdrawn_at=scope_001_now() WHERE fact_ref=$1").bind(id).execute(&mut *tx).await?;
    let fact = snapshot(&mut tx, id).await?;
    record(&mut tx, request, "withdraw", &hash, &fact).await?;
    tx.commit().await?;
    Ok(PersonaFactReceipt {
        result_version: fact.version,
        fact,
        replayed: false,
    })
}
