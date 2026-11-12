//! Exact, current Operations publication facts for qualified graph works.
use super::*;
use linggan_storage_postgres::Database;
use serde_json::json;
use sqlx::Row;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;

pub(super) async fn read(
    db: &Database,
    works: &[Uuid],
    domain: Option<Uuid>,
) -> Result<BTreeMap<Uuid, Vec<Value>>, TopicMapError> {
    let mut tx = db.pool().begin().await?;
    let result = read_in(&mut tx, works, domain).await?;
    tx.commit().await?;
    Ok(result)
}

pub(super) async fn read_in(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    works: &[Uuid],
    domain: Option<Uuid>,
) -> Result<BTreeMap<Uuid, Vec<Value>>, TopicMapError> {
    let query = format!(
        "WITH {} SELECT DISTINCT projection.content_public_ref,p.publication_ref,p.account_ref,a.account_name,p.platform,coalesce(p.published_business_date,p.native_published_on)::text AS published_on,views.view_count FROM linggan_ops_publication_material_projection projection JOIN linggan_ops_publication p ON p.publication_ref=projection.publication_ref AND p.native_version=projection.publication_version LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref LEFT JOIN latest_views views ON views.publication_ref=p.publication_ref WHERE projection.content_public_ref=ANY($1) AND linggan_ops_publication_visible(p,a,NULL::uuid[],NULL::uuid) AND ($2::uuid IS NULL OR coalesce(p.native_business_domain_ref,a.domain_ref)=$2) AND (p.source_system='linggan_native' OR (p.legacy_effective_publication AND p.review_status<>'draft')) ORDER BY projection.content_public_ref,p.publication_ref",
        crate::ops_ledger::read::LATEST_VIEWS_CTE
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(query))
        .bind(works)
        .bind(domain)
        .fetch_all(&mut **tx)
        .await?;
    let mut result = BTreeMap::<Uuid, Vec<Value>>::new();
    for row in rows {
        let platform: String = row.get("platform");
        let views: Option<i64> = row.get("view_count");
        let hit = platform == "xhs"
            && views.is_some_and(|v| v >= crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD);
        result.entry(row.get("content_public_ref")).or_default().push(json!({
            "publicationRef":row.get::<Uuid,_>("publication_ref"),"accountRef":row.get::<Option<Uuid>,_>("account_ref"),
            "accountName":row.get::<Option<String>,_>("account_name"),"platform":platform,
            "publishedOn":row.get::<Option<String>,_>("published_on"),"views":views,"hit":hit
        }));
    }
    Ok(result)
}

pub(super) fn summarize(facts: &BTreeMap<Uuid, Vec<Value>>, works: &[&TopicMapWork]) -> Value {
    let entries: Vec<_> = works
        .iter()
        .filter_map(|w| facts.get(&w.work_ref).map(|p| (w.work_ref, p)))
        .collect();
    let publications: BTreeSet<_> = entries
        .iter()
        .flat_map(|(_, p)| p.iter().filter_map(|p| p["publicationRef"].as_str()))
        .collect();
    json!({"publishedWorkCount":entries.len(),"publicationCount":publications.len(),
        "hitWorkCount":entries.iter().filter(|(_,p)|p.iter().any(|p|p["hit"]==true)).count(),
        "unknownViewsWorkCount":entries.iter().filter(|(_,p)|p.iter().any(|p|p["platform"]=="xhs"&&p["views"].is_null())).count(),
        "hitRule":{"platform":"xhs","metric":"views","threshold":crate::ops_ledger::XHS_HIT_VIEW_THRESHOLD},
        "works":entries.iter().map(|(work,p)|json!({"workRef":work,"publications":p})).collect::<Vec<_>>()})
}
