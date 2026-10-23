use super::{Error, Scope};
use linggan_storage_postgres::Database;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) async fn begin<'a>(
    database: &'a Database,
    scope: &Scope,
) -> Result<(Transaction<'a, Postgres>, String), Error> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    validate_target(&mut tx, scope).await?;
    let valid: bool = sqlx::query_scalar(
        "SELECT CASE WHEN $1::text IS NULL THEN true \
         WHEN $1 ~ '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]{1,6})?(Z|[+-][0-9]{2}:[0-9]{2})$' \
         THEN isfinite($1::timestamptz) AND $1::timestamptz<=scope_001_now() ELSE false END",
    ).bind(scope.knowledge_cutoff.as_deref()).fetch_one(&mut *tx).await.map_err(time_error)?;
    if !valid {
        return Err(Error::Invalid("invalid_knowledge_cutoff"));
    }
    // Machine timestamp. A single cutoff freezes every read in this inspection response.
    let cutoff: String = sqlx::query_scalar(
        "SELECT to_char(COALESCE($1::timestamptz,scope_001_now()) AT TIME ZONE 'UTC','YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')",
    )
    .bind(scope.knowledge_cutoff.as_deref())
    .fetch_one(&mut *tx)
    .await?;
    Ok((tx, cutoff))
}

fn time_error(error: sqlx::Error) -> Error {
    if let sqlx::Error::Database(database) = &error {
        if matches!(
            database.code().as_deref(),
            Some("22007" | "22008" | "22P02")
        ) {
            return Error::Invalid("invalid_knowledge_cutoff");
        }
    }
    Error::Database(error)
}

pub(crate) async fn validate_target(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
) -> Result<(), Error> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collection_observation_target target \
         WHERE target.target_ref=$1 AND ($2::uuid IS NULL OR EXISTS( \
             SELECT 1 FROM observation_domain_target relation \
             JOIN observation_domain domain USING(domain_ref) \
             WHERE relation.target_ref=target.target_ref AND domain.domain_ref=$2) \
           OR EXISTS(SELECT 1 FROM collection_work_order work_order \
             JOIN collection_work_order_domain_usage usage USING(work_order_ref) \
             WHERE work_order.target_ref=target.target_ref AND usage.domain_ref=$2)))",
    )
    .bind(scope.target_ref)
    .bind(scope.domain_ref)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(Error::NotFound);
    }
    Ok(())
}

pub(crate) async fn require_work(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    work: Uuid,
    cutoff: &str,
) -> Result<(), Error> {
    let belongs: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM collection_observation_target target \
         JOIN linggan_material_content content ON content.public_ref=$3 \
         WHERE target.target_ref=$1 AND content.platform=target.platform \
         AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage \
             JOIN linggan_runtime_capture_package p ON p.package_ref=usage.package_ref \
             JOIN linggan_runtime_submission_receipt receipt ON receipt.package_ref=p.package_ref \
             WHERE usage.content_public_ref=content.public_ref \
               AND ($2::uuid IS NULL OR usage.domain_ref=$2) \
               AND p.accepted_at <= $4::timestamptz AND receipt.material_admission='ACCEPTED') \
         AND ((target.target_kind='creator' AND EXISTS( \
             SELECT 1 FROM linggan_material_content_author author \
             WHERE author.content_public_ref=content.public_ref \
               AND author.platform=target.platform AND author.author_external_id=target.identity_key)) \
           OR (target.target_kind='keyword' AND EXISTS(SELECT 1 FROM linggan_material_discovery_finding finding \
             JOIN linggan_runtime_capture_package package USING(package_ref) \
             JOIN linggan_runtime_submission_receipt receipt USING(package_ref) \
             JOIN linggan_runtime_record_disposition disposition \
               ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal \
             JOIN collection_work_order_lease_task task ON task.task_id=package.task_id \
             JOIN collection_work_order_lease lease USING(lease_ref) \
             JOIN collection_work_order work_order USING(work_order_ref) \
             WHERE finding.content_public_ref=content.public_ref \
               AND work_order.target_ref=target.target_ref AND package.accepted_at <= $4::timestamptz \
               AND receipt.material_admission='ACCEPTED' \
               AND disposition.disposition='accepted_for_library_discovery' \
               AND EXISTS(SELECT 1 FROM collection_work_order_domain_usage work_usage \
                 WHERE work_usage.work_order_ref=work_order.work_order_ref \
                   AND ($2::uuid IS NULL OR work_usage.domain_ref=$2))))))",
    ).bind(scope.target_ref).bind(scope.domain_ref).bind(work).bind(cutoff).fetch_one(&mut **tx).await?;
    if !belongs {
        return Err(Error::NotFound);
    }
    Ok(())
}

pub(crate) fn check_offset(offset: usize) -> Result<i64, Error> {
    if offset > 100_000 {
        return Err(Error::Invalid("inspection_offset_out_of_range"));
    }
    i64::try_from(offset).map_err(|_| Error::Invalid("inspection_offset_out_of_range"))
}
