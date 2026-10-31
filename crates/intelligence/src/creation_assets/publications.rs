//! Explicit links to existing Operations publication records. No platform publication occurs here.
use super::{
    AssetsError,
    requests::{Claim, claim, command_hash, lock},
    types::*,
};
use crate::content_workbench as workbench;
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

fn work_error(error: workbench::WorkbenchError) -> AssetsError {
    match error {
        workbench::WorkbenchError::NotFound => AssetsError::NotFound,
        workbench::WorkbenchError::Source => AssetsError::SourceUnavailable,
        other => AssetsError::Workbench(other),
    }
}

/// Operations imports update account before publication. Take shared row locks in that order.
async fn eligible_in(
    tx: &mut Transaction<'_, Postgres>,
    publication: Uuid,
    account: Uuid,
    domain: Uuid,
) -> Result<bool, AssetsError> {
    let account_ok: Option<Uuid> = sqlx::query_scalar("SELECT account_ref FROM linggan_ops_account WHERE account_ref=$1 AND domain_ref=$2 AND ledger_state='in_ledger' AND platform='xhs' FOR SHARE")
        .bind(account).bind(domain).fetch_optional(&mut **tx).await?;
    if account_ok.is_none() {
        return Ok(false);
    }
    let publication_ok: Option<Uuid> = sqlx::query_scalar("SELECT publication_ref FROM linggan_ops_publication WHERE publication_ref=$1 AND account_ref=$2 AND platform='xhs' AND merged_into_ref IS NULL AND missing_since_batch_ref IS NULL AND (source_system<>'linggan_native' OR native_record_state='active') FOR SHARE")
        .bind(publication).bind(account).fetch_optional(&mut **tx).await?;
    Ok(publication_ok.is_some())
}

pub async fn publication_candidates(
    db: &Database,
    work: Uuid,
) -> Result<Vec<PublicationCandidate>, AssetsError> {
    let envelope = workbench::read_work(db, work).await.map_err(work_error)?;
    let Some(account) = envelope.revision.document.account_ref else {
        return Ok(vec![]);
    };
    let rows = sqlx::query("SELECT p.publication_ref,p.title,COALESCE(p.native_published_on,p.published_business_date)::text AS published_on,p.publication_url FROM linggan_ops_publication p JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE p.account_ref=$1 AND a.domain_ref=$2 AND a.ledger_state='in_ledger' AND a.platform='xhs' AND p.platform='xhs' AND p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL AND (p.source_system<>'linggan_native' OR p.native_record_state='active') AND NOT EXISTS (SELECT 1 FROM linggan_creation_publication_link l WHERE l.publication_ref=p.publication_ref AND NOT EXISTS (SELECT 1 FROM linggan_creation_publication_unlink u WHERE u.link_ref=l.link_ref)) ORDER BY COALESCE(p.native_published_on,p.published_business_date) DESC NULLS LAST,p.publication_ref LIMIT 100")
        .bind(account).bind(envelope.domain_ref).fetch_all(db.pool()).await?;
    rows.iter()
        .map(|row| {
            Ok(PublicationCandidate {
                publication_ref: row.try_get("publication_ref")?,
                title: row.try_get("title")?,
                published_on: row.try_get("published_on")?,
                url: row.try_get("publication_url")?,
            })
        })
        .collect()
}

pub async fn link_publication(
    db: &Database,
    work: Uuid,
    cmd: &PublicationLinkCommand,
) -> Result<PublicationLinkReceipt, AssetsError> {
    let sha = command_hash(
        "publication_link",
        &(work, cmd.expected_revision, cmd.publication_ref),
    )?;
    let mut tx = db.pool().begin().await?;
    let claimed = claim(&mut tx, cmd.request_id, "publication_link", &sha).await?;
    if matches!(claimed, Claim::Replay) {
        let row = sqlx::query("SELECT link_ref,revision_ref,publication_ref FROM linggan_creation_publication_link l WHERE request_ref=$1 AND work_ref=$2 AND NOT EXISTS(SELECT 1 FROM linggan_creation_publication_unlink u WHERE u.link_ref=l.link_ref)")
            .bind(cmd.request_id).bind(work).fetch_optional(&mut *tx).await?.ok_or(AssetsError::Stale)?;
        let receipt = PublicationLinkReceipt {
            link_ref: row.try_get("link_ref")?,
            revision_id: row.try_get("revision_ref")?,
            publication_ref: row.try_get("publication_ref")?,
            replayed: true,
        };
        lock(&mut tx, &format!("publication:{}", receipt.publication_ref)).await?;
        let active: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM linggan_creation_publication_unlink WHERE link_ref=$1)")
            .bind(receipt.link_ref).fetch_one(&mut *tx).await?;
        if !active {
            return Err(AssetsError::Stale);
        }
        let (account, domain): (Option<Uuid>, Uuid) = sqlx::query_as("SELECT (r.document->>'accountRef')::uuid,w.domain_ref FROM linggan_creation_publication_link l JOIN linggan_creation_revision r ON r.revision_ref=l.revision_ref JOIN linggan_creation_work w ON w.work_ref=l.work_ref WHERE l.link_ref=$1")
            .bind(receipt.link_ref).fetch_one(&mut *tx).await?;
        let Some(account) = account else {
            return Err(AssetsError::SourceUnavailable);
        };
        if !eligible_in(&mut tx, receipt.publication_ref, account, domain).await? {
            return Err(AssetsError::SourceUnavailable);
        }
        tx.commit().await?;
        return Ok(receipt);
    }
    workbench::work_lock(&mut tx, work.to_string())
        .await
        .map_err(work_error)?;
    let (domain, revision) = workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(work_error)?;
    if revision.id != cmd.expected_revision {
        return Err(AssetsError::Stale);
    }
    let account = revision
        .document
        .account_ref
        .ok_or(AssetsError::Invalid("account"))?;
    lock(&mut tx, &format!("publication:{}", cmd.publication_ref)).await?;
    if !eligible_in(&mut tx, cmd.publication_ref, account, domain).await? {
        return Err(AssetsError::SourceUnavailable);
    }
    let linked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_publication_link l WHERE l.publication_ref=$1 AND NOT EXISTS (SELECT 1 FROM linggan_creation_publication_unlink u WHERE u.link_ref=l.link_ref))")
        .bind(cmd.publication_ref).fetch_one(&mut *tx).await?;
    if linked {
        return Err(AssetsError::RequestConflict);
    }
    let link_ref = Uuid::new_v4();
    sqlx::query("INSERT INTO linggan_creation_publication_link(link_ref,request_ref,work_ref,revision_ref,publication_ref,linked_by) VALUES($1,$2,$3,$4,$5,'local_machine')")
        .bind(link_ref).bind(cmd.request_id).bind(work).bind(revision.id).bind(cmd.publication_ref).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(PublicationLinkReceipt {
        link_ref,
        revision_id: revision.id,
        publication_ref: cmd.publication_ref,
        replayed: false,
    })
}

/// Active links, including records whose Operations qualification later changed.
pub async fn publication_links(
    db: &Database,
    work: Uuid,
) -> Result<Vec<PublicationLinkView>, AssetsError> {
    let _ = workbench::read_work(db, work).await.map_err(work_error)?;
    let rows = sqlx::query("SELECT l.link_ref,l.revision_ref,l.publication_ref,p.title,COALESCE(p.native_published_on,p.published_business_date)::text AS published_on,COALESCE((p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL AND (p.source_system<>'linggan_native' OR p.native_record_state='active') AND p.platform='xhs' AND p.account_ref::text=r.document->>'accountRef' AND a.domain_ref=w.domain_ref AND a.ledger_state='in_ledger' AND a.platform='xhs'),false) AS available FROM linggan_creation_publication_link l JOIN linggan_ops_publication p ON p.publication_ref=l.publication_ref JOIN linggan_creation_revision r ON r.revision_ref=l.revision_ref JOIN linggan_creation_work w ON w.work_ref=l.work_ref LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE l.work_ref=$1 AND NOT EXISTS(SELECT 1 FROM linggan_creation_publication_unlink u WHERE u.link_ref=l.link_ref) ORDER BY l.linked_at DESC,l.link_ref DESC")
        .bind(work).fetch_all(db.pool()).await?;
    rows.iter()
        .map(|row| {
            Ok(PublicationLinkView {
                link_ref: row.try_get("link_ref")?,
                revision_id: row.try_get("revision_ref")?,
                publication_ref: row.try_get("publication_ref")?,
                title: row.try_get("title")?,
                published_on: row.try_get("published_on")?,
                available: row.try_get("available")?,
            })
        })
        .collect()
}

pub async fn unlink_publication(
    db: &Database,
    work: Uuid,
    link_ref: Uuid,
    request_id: Uuid,
) -> Result<bool, AssetsError> {
    let sha = command_hash("publication_unlink", &(work, link_ref))?;
    let mut tx = db.pool().begin().await?;
    let claim_state = claim(&mut tx, request_id, "publication_unlink", &sha).await?;
    let publication: Uuid = sqlx::query_scalar("SELECT publication_ref FROM linggan_creation_publication_link WHERE link_ref=$1 AND work_ref=$2")
        .bind(link_ref).bind(work).fetch_optional(&mut *tx).await?.ok_or(AssetsError::NotFound)?;
    lock(&mut tx, &format!("publication:{publication}")).await?;
    if matches!(claim_state, Claim::Replay) {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_creation_publication_unlink WHERE link_ref=$1 AND request_ref=$2)")
            .bind(link_ref).bind(request_id).fetch_one(&mut *tx).await?;
        if !exists {
            return Err(AssetsError::RequestConflict);
        }
        tx.commit().await?;
        return Ok(true);
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_creation_publication_unlink WHERE link_ref=$1)",
    )
    .bind(link_ref)
    .fetch_one(&mut *tx)
    .await?;
    if exists {
        return Err(AssetsError::RequestConflict);
    }
    sqlx::query("INSERT INTO linggan_creation_publication_unlink(link_ref,request_ref,unlinked_by) VALUES($1,$2,'local_machine')")
        .bind(link_ref).bind(request_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(false)
}
