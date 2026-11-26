//! Automatic AI uses the same frozen collection members and accepted details as the producer.
use super::store::FoundationError;
use linggan_evidence::selected_collection_run::{self, ExecutionScope, SelectedCollectionRun};
use linggan_storage_postgres::Database;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

/// None means the configured collection run is unavailable. An absent configuration uses the normal queue.
#[derive(Default)]
pub(super) struct Filter {
    pub collection_run_ref: Option<Uuid>,
    pub domain: Option<Uuid>,
    pub works: Option<Vec<Uuid>>,
}

pub(super) async fn filter(db: &Database) -> Result<Option<Filter>, sqlx::Error> {
    if !selected_collection_run::configured() {
        return Ok(Some(Filter::default()));
    }
    let mut connection = db.pool().acquire().await?;
    filter_in(&mut connection).await
}

pub(super) async fn filter_in(
    connection: &mut PgConnection,
) -> Result<Option<Filter>, sqlx::Error> {
    let scope = selected_collection_run::load_in(connection).await?;
    filter_scope_in(connection, scope).await
}

pub(super) async fn lock_filter_in(
    connection: &mut PgConnection,
) -> Result<Option<Filter>, sqlx::Error> {
    let scope = selected_collection_run::lock_in(connection).await?;
    filter_scope_in(connection, scope).await
}

async fn filter_scope_in(
    connection: &mut PgConnection,
    scope: ExecutionScope,
) -> Result<Option<Filter>, sqlx::Error> {
    match scope {
        ExecutionScope::Unrestricted => Ok(Some(Filter::default())),
        ExecutionScope::Blocked => Ok(None),
        ExecutionScope::Selected(scope) => Ok(Some(Filter {
            collection_run_ref: Some(scope.run_ref),
            domain: Some(scope.domain_ref),
            works: Some(accepted_works_in(connection, &scope).await?),
        })),
    }
}

async fn accepted_works_in(
    connection: &mut PgConnection,
    scope: &SelectedCollectionRun,
) -> Result<Vec<Uuid>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT DISTINCT detail.content_public_ref
         FROM collection_run_package package
         JOIN linggan_material_content_detail detail USING(package_ref)
         WHERE package.run_ref=$1 AND package.package_kind='content_detail'
           AND detail.content_public_ref=ANY($2)
         ORDER BY detail.content_public_ref",
    )
    .bind(scope.run_ref)
    .bind(&scope.work_refs)
    .fetch_all(connection)
    .await
}

pub(super) async fn queue_automatic_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    work: Uuid,
) -> Result<Option<Uuid>, FoundationError> {
    match selected_collection_run::load_in(tx).await? {
        ExecutionScope::Unrestricted => Ok(None),
        ExecutionScope::Selected(scope) if scope.domain_ref == domain => {
            if scope.work_refs.contains(&work)
                && accepted_works_in(tx, &scope).await?.contains(&work)
            {
                return Ok(Some(scope.run_ref));
            }
            Err(scope_denied())
        }
        _ => Err(scope_denied()),
    }
}

pub(super) fn scope_denied() -> FoundationError {
    FoundationError::AdmissionDenied(vec!["selected_collection_run_not_eligible".into()])
}
