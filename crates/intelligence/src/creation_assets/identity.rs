//! Account identity: what the operators know about an account, as append-only revisions.
use super::requests::{Claim, claim, command_hash, lock, sql};
use super::{AssetsError, types::*};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use uuid::Uuid;

pub(super) const COLUMNS: &str = "i.account_ref,i.revision_number,i.org_relation,i.bio_discloses_relation,i.bio_checked_at::text AS bio_checked_at,i.bio_checked_by,i.has_real_person,i.backing_person_role,i.can_provide_experience,EXISTS(SELECT 1 FROM linggan_creation_account_person p WHERE p.account_ref=i.account_ref) AS has_known_person,i.saved_at::text AS saved_at,i.saved_by";

pub(super) fn from_row(row: &PgRow) -> Result<Identity, AssetsError> {
    Ok(Identity {
        account_ref: row.try_get("account_ref")?,
        revision: row.try_get("revision_number")?,
        org_relation: OrgRelation::from_db(&row.try_get::<String, _>("org_relation")?)
            .ok_or(AssetsError::Invalid("org_relation"))?,
        bio_discloses_relation: row.try_get("bio_discloses_relation")?,
        bio_checked_at: row.try_get("bio_checked_at")?,
        bio_checked_by: row.try_get("bio_checked_by")?,
        has_real_person: row.try_get("has_real_person")?,
        backing_person_role: row.try_get("backing_person_role")?,
        can_provide_experience: row.try_get("can_provide_experience")?,
        has_known_person: row.try_get("has_known_person")?,
        saved_at: row.try_get("saved_at")?,
        saved_by: row.try_get("saved_by")?,
    })
}

pub(super) async fn latest_in(
    tx: &mut Transaction<'_, Postgres>,
    account: Uuid,
) -> Result<Option<Identity>, AssetsError> {
    let row = sqlx::query(sql(format!(
        "SELECT {COLUMNS} FROM linggan_creation_account_identity i WHERE i.account_ref=$1 ORDER BY i.revision_number DESC LIMIT 1"
    )))
    .bind(account)
    .fetch_optional(&mut **tx)
    .await?;
    row.as_ref().map(from_row).transpose()
}

pub async fn read_identity(db: &Database, account: Uuid) -> Result<Option<Identity>, AssetsError> {
    let mut tx = db.pool().begin().await?;
    let identity = latest_in(&mut tx, account).await?;
    tx.commit().await?;
    Ok(identity)
}

fn normalized(cmd: &IdentityCommand) -> Result<IdentityCommand, AssetsError> {
    let mut cmd = cmd.clone();
    if cmd.expected_revision < 0 {
        return Err(AssetsError::Invalid("expected_revision"));
    }
    cmd.backing_person_role = match cmd.backing_person_role.take() {
        None => None,
        Some(role) => {
            let role = role.trim().to_string();
            if cmd.has_real_person != Some(true)
                || role.is_empty()
                || role.chars().count() > 40
                || role.chars().any(char::is_control)
            {
                return Err(AssetsError::Invalid("backing_person_role"));
            }
            Some(role)
        }
    };
    Ok(cmd)
}

async fn by_request(
    tx: &mut Transaction<'_, Postgres>,
    request: Uuid,
) -> Result<Identity, AssetsError> {
    let row = sqlx::query(sql(format!(
        "SELECT {COLUMNS} FROM linggan_creation_account_identity i WHERE i.request_ref=$1"
    )))
    .bind(request)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(AssetsError::NotFound)?;
    from_row(&row)
}

/// Writes the next identity revision. A repeat of the same command replays; a revision other than
/// the latest one is stale.
pub async fn put_identity(
    db: &Database,
    account: Uuid,
    cmd: &IdentityCommand,
) -> Result<IdentityReceipt, AssetsError> {
    let cmd = normalized(cmd)?;
    let mut meaning = serde_json::json!({
        "account": account, "expected": cmd.expected_revision, "relation": cmd.org_relation,
        "bio": cmd.bio_discloses_relation, "person": cmd.has_real_person,
        "role": cmd.backing_person_role, "experience": cmd.can_provide_experience,
    });
    // Preserve existing request fingerprints for callers that do not specify a new action.
    if let Some(change) = cmd.person_change {
        meaning["personChange"] = serde_json::to_value(change)?;
    }
    let sha = command_hash("identity", &meaning)?;
    let mut tx = db.pool().begin().await?;
    if let Claim::Replay = claim(&mut tx, cmd.request_id, "identity", &sha).await? {
        let identity = by_request(&mut tx, cmd.request_id).await?;
        tx.commit().await?;
        return Ok(IdentityReceipt {
            identity,
            replayed: true,
        });
    }
    lock(&mut tx, "experience-gate").await?;
    lock(&mut tx, &format!("identity:{account}")).await?;
    let known: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_ops_account WHERE account_ref=$1 AND ledger_state='in_ledger')",
    )
    .bind(account)
    .fetch_one(&mut *tx)
    .await?;
    if !known {
        return Err(AssetsError::NotFound);
    }
    let current = latest_in(&mut tx, account).await?;
    if current.as_ref().map_or(0, |i| i.revision) != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let existing_subject: Option<Uuid> = sqlx::query_scalar(
        "SELECT subject_ref FROM linggan_creation_account_person WHERE account_ref=$1",
    )
    .bind(account)
    .fetch_optional(&mut *tx)
    .await?;
    if cmd.person_change.is_some() && cmd.has_real_person != Some(true) {
        return Err(AssetsError::Invalid("person_change"));
    }
    let ambiguity = existing_subject.is_some()
        && cmd.has_real_person == Some(true)
        && current.as_ref().is_some_and(|old| {
            old.has_real_person != Some(true) || old.backing_person_role != cmd.backing_person_role
        });
    if ambiguity && cmd.person_change.is_none() {
        return Err(AssetsError::Invalid("person_change_required"));
    }
    let (subject, assignment) = if cmd.has_real_person == Some(true) {
        match (existing_subject, cmd.person_change) {
            (Some(_), Some(PersonChange::Replacement)) | (None, _) => {
                let subject = Uuid::new_v4();
                sqlx::query("INSERT INTO linggan_creation_person_subject(subject_ref,account_ref) VALUES($1,$2)")
                    .bind(subject).bind(account).execute(&mut *tx).await?;
                sqlx::query("INSERT INTO linggan_creation_account_person(account_ref,subject_ref) VALUES($1,$2) ON CONFLICT (account_ref) DO UPDATE SET subject_ref=EXCLUDED.subject_ref")
                    .bind(account).bind(subject).execute(&mut *tx).await?;
                (
                    Some(subject),
                    if existing_subject.is_some() {
                        "replaced"
                    } else {
                        "created"
                    },
                )
            }
            (Some(subject), Some(PersonChange::Correction)) => (Some(subject), "corrected"),
            (Some(subject), None) => (Some(subject), "unchanged"),
        }
    } else {
        (existing_subject, "unavailable")
    };
    // A bio check applies to the specific relationship, not just the yes/no answer.
    let carried = current
        .as_ref()
        .filter(|i| {
            i.org_relation == cmd.org_relation
                && i.bio_discloses_relation == cmd.bio_discloses_relation
        })
        .and_then(|i| i.bio_checked_at.clone());
    let identity_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_creation_account_identity(identity_ref,account_ref,revision_number,request_ref,org_relation,bio_discloses_relation,bio_checked_at,bio_checked_by,has_real_person,backing_person_role,can_provide_experience,saved_by)
         VALUES($1,$2,$3,$4,$5,$6::boolean,
                CASE WHEN $6::boolean IS NULL THEN NULL ELSE COALESCE($7::timestamptz,scope_001_now()) END,
                CASE WHEN $6::boolean IS NULL THEN NULL ELSE 'local_machine' END,
                $8,$9,$10,'local_machine')",
    )
    .bind(identity_ref)
    .bind(account)
    .bind(cmd.expected_revision + 1)
    .bind(cmd.request_id)
    .bind(cmd.org_relation.as_str())
    .bind(cmd.bio_discloses_relation)
    .bind(carried)
    .bind(cmd.has_real_person)
    .bind(&cmd.backing_person_role)
    .bind(cmd.can_provide_experience)
    .execute(&mut *tx)
    .await?;
    if let Some(subject) = subject {
        sqlx::query("INSERT INTO linggan_creation_person_assignment(identity_ref,account_ref,subject_ref,action,recorded_by) VALUES($1,$2,$3,$4,'local_machine')")
            .bind(identity_ref).bind(account).bind(subject).bind(assignment).execute(&mut *tx).await?;
    }
    let identity = by_request(&mut tx, cmd.request_id).await?;
    tx.commit().await?;
    Ok(IdentityReceipt {
        identity,
        replayed: false,
    })
}

/// XHS accounts in the ledger that can be written for, each with its latest identity revision.
pub async fn list_accounts(
    db: &Database,
    domain: Option<Uuid>,
) -> Result<Vec<AccountView>, AssetsError> {
    let rows = sqlx::query("SELECT a.account_ref AS account_ref_a,a.account_name,a.platform,a.status AS account_status,a.operator_name,
                i.account_ref,i.revision_number,i.org_relation,i.bio_discloses_relation,i.bio_checked_at::text AS bio_checked_at,i.bio_checked_by,i.has_real_person,i.backing_person_role,i.can_provide_experience,EXISTS(SELECT 1 FROM linggan_creation_account_person p WHERE p.account_ref=i.account_ref) AS has_known_person,i.saved_at::text AS saved_at,i.saved_by
         FROM linggan_ops_account a
         LEFT JOIN LATERAL (SELECT * FROM linggan_creation_account_identity WHERE account_ref=a.account_ref ORDER BY revision_number DESC LIMIT 1) i ON true
         WHERE a.platform='xhs' AND a.ledger_state='in_ledger' AND a.status IN ('active','warming') AND ($1::uuid IS NULL OR a.domain_ref=$1)
         ORDER BY a.account_name,a.account_ref")
    .bind(domain)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            let identity = match row.try_get::<Option<i32>, _>("revision_number")? {
                Some(_) => Some(from_row(row)?),
                None => None,
            };
            Ok(AccountView {
                account_ref: row.try_get("account_ref_a")?,
                account_name: row.try_get("account_name")?,
                platform: row.try_get("platform")?,
                status: row.try_get("account_status")?,
                operator_name: row.try_get("operator_name")?,
                identity,
            })
        })
        .collect()
}
