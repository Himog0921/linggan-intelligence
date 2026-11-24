//! Reconcile committed ledger changes with the immutable board snapshots.
use super::{
    LensDefinitionError, SnapshotReceipt, SnapshotScope,
    label::{IncrementalLabels, start_pending},
    snapshot::{SnapshotMode, build_with_mode},
};
use crate::ai_foundation::store::FoundationError;
use linggan_storage_postgres::Database;
use serde::Serialize;
use std::collections::BTreeSet;
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshReceipt {
    pub snapshots: Vec<SnapshotReceipt>,
    pub awaiting_agreement: bool,
    pub labels: IncrementalLabels,
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error(transparent)]
    Snapshot(#[from] LensDefinitionError),
    #[error(transparent)]
    Labels(#[from] FoundationError),
    #[error("ops_lens_refresh_database_error")]
    Database(#[from] sqlx::Error),
}

/// Advances committed refresh requests and admits full incremental label batches.
/// The caller controls the Content Lens switch. AI foundation checks admission; this
/// function never invokes a provider. Any interrupted projection remains retryable.
///
/// # Errors
/// Returns a database, snapshot calculation or unexpected label admission error.
pub async fn refresh_pending(database: &Database) -> Result<RefreshReceipt, RefreshError> {
    let changes: Vec<(Uuid, i64, Option<Uuid>)> = sqlx::query_as(
        "SELECT q.publication_ref,q.generation,a.domain_ref
         FROM linggan_ops_lens_refresh q
         JOIN linggan_ops_publication p USING(publication_ref)
         LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref
         WHERE q.snapshot_generation<q.generation",
    )
    .fetch_all(database.pool())
    .await?;
    let mut snapshots = Vec::new();
    let mut awaiting_agreement = false;
    if !changes.is_empty() {
        let domains: BTreeSet<_> = changes.iter().filter_map(|row| row.2).collect();
        let mut refreshed_domains = BTreeSet::new();
        for domain_ref in std::iter::once(None).chain(domains.into_iter().map(Some)) {
            let scope = SnapshotScope {
                domain_ref,
                ..SnapshotScope::default()
            };
            match build_with_mode(
                database,
                scope,
                "ops_title_label",
                SnapshotMode::IncludePending,
            )
            .await
            {
                Ok(receipt) => {
                    refreshed_domains.insert(domain_ref);
                    snapshots.push(receipt);
                }
                Err(LensDefinitionError::MissingAgreement | LensDefinitionError::NoMethod) => {
                    awaiting_agreement = true;
                    if domain_ref.is_none() {
                        break;
                    }
                }
                // A domain that no longer has an eligible account has no valid board scope.
                Err(LensDefinitionError::InvalidScope) if domain_ref.is_some() => {
                    refreshed_domains.insert(domain_ref);
                }
                Err(error) => return Err(error.into()),
            }
        }
        if refreshed_domains.contains(&None) {
            acknowledge(
                database,
                changes
                    .iter()
                    .filter(|row| refreshed_domains.contains(&row.2)),
            )
            .await?;
        }
    }
    let labels = start_pending(database).await?;
    Ok(RefreshReceipt {
        snapshots,
        awaiting_agreement,
        labels,
    })
}

pub(crate) async fn mark_publication_dirty(
    conn: &mut sqlx::PgConnection,
    publication: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO linggan_ops_lens_refresh(publication_ref)
         SELECT p.publication_ref FROM linggan_ops_publication p
         WHERE p.publication_ref=$1 AND p.platform='xhs'
           AND linggan_ops_publication_visible(p,NULL::linggan_ops_account,NULL,NULL)
         ON CONFLICT (publication_ref) DO UPDATE SET generation=linggan_ops_lens_refresh.generation+1",
    )
    .bind(publication)
    .execute(conn)
    .await?;
    Ok(())
}

async fn acknowledge<'a>(
    database: &Database,
    changes: impl Iterator<Item = &'a (Uuid, i64, Option<Uuid>)>,
) -> Result<(), sqlx::Error> {
    let (publications, generations): (Vec<_>, Vec<_>) = changes.map(|row| (row.0, row.1)).unzip();
    // The reads above may have covered newer data, but we only acknowledge what
    // we observed before snapshot generation. A concurrent write cannot be lost.
    sqlx::query(
        "UPDATE linggan_ops_lens_refresh q
         SET snapshot_generation=changes.generation
         FROM unnest($1::uuid[],$2::bigint[]) AS changes(publication_ref,generation)
         WHERE q.publication_ref=changes.publication_ref
           AND q.snapshot_generation<changes.generation",
    )
    .bind(publications)
    .bind(generations)
    .execute(database.pool())
    .await?;
    Ok(())
}
