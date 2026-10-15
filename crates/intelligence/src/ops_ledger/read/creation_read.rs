//! Creation reads retain the operations mirror's current-export and duplicate qualifications.
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationPublication {
    pub source_ref: Uuid,
    pub title: Option<String>,
    pub body: Option<String>,
    pub account_ref: Option<Uuid>,
    pub account_name: Option<String>,
    pub source_version: String,
}

pub async fn read_creation_publications(
    tx: &mut Transaction<'_, Postgres>,
    domain: Option<Uuid>,
    account: Option<Uuid>,
    reference: Option<Uuid>,
) -> Result<Vec<CreationPublication>, sqlx::Error> {
    let rows = sqlx::query("SELECT p.publication_ref,p.title,p.body,p.account_ref,a.account_name,p.last_changed_batch_ref,p.source_updated_at::text AS source_updated_at FROM linggan_ops_publication p LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL AND ($1::uuid IS NULL OR a.domain_ref=$1) AND ($2::uuid IS NULL OR p.account_ref=$2) AND ($3::uuid IS NULL OR p.publication_ref=$3) ORDER BY p.publication_ref")
        .bind(domain).bind(account).bind(reference).fetch_all(&mut **tx).await?;
    Ok(rows
        .iter()
        .map(|r| {
            let title: Option<String> = r.get("title");
            let body: Option<String> = r.get("body");
            let version = crate::research_text::content_hash(
                &serde_json::json!([
                    r.get::<Uuid, _>("last_changed_batch_ref"),
                    r.get::<String, _>("source_updated_at"),
                    title,
                    body
                ])
                .to_string(),
            );
            CreationPublication {
                source_ref: r.get("publication_ref"),
                title,
                body,
                account_ref: r.get("account_ref"),
                account_name: r.get("account_name"),
                source_version: version,
            }
        })
        .collect())
}

pub async fn creation_publication_directory_in(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    account: Option<Uuid>,
    query: Option<&str>,
    offset: i64,
) -> Result<(Vec<Uuid>, i64), sqlx::Error> {
    let filter = "FROM linggan_ops_publication p JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE p.merged_into_ref IS NULL AND p.missing_since_batch_ref IS NULL AND a.domain_ref=$1 AND ($2::uuid IS NULL OR p.account_ref=$2) AND ($3::text IS NULL OR lower(COALESCE(p.title,'')) LIKE '%'||lower($3)||'%' OR lower(COALESCE(p.body,'')) LIKE '%'||lower($3)||'%')";
    let total = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {filter}")))
        .bind(domain)
        .bind(account)
        .bind(query)
        .fetch_one(&mut **tx)
        .await?;
    let refs = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT p.publication_ref {filter} ORDER BY p.publication_ref LIMIT 31 OFFSET $4"
    )))
    .bind(domain)
    .bind(account)
    .bind(query)
    .bind(offset)
    .fetch_all(&mut **tx)
    .await?;
    Ok((refs, total))
}
