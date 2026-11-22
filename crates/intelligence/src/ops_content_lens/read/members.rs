use super::{LensDefinitionError, frozen};
use crate::ops_content_lens::{
    LabelFilter, SampleMetric, Subset, TITLED_PUBLICATIONS_CTE, select_publications,
};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    pub snapshot_ref: Uuid,
    pub moment: Option<String>,
    pub offer: Option<String>,
    pub situation: Option<String>,
    pub metric: Option<SampleMetric>,
    pub subset: Option<Subset>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicationPage {
    pub snapshot_ref: Uuid,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
    pub items: Vec<Publication>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Publication {
    pub publication_ref: Uuid,
    pub title: Option<String>,
    pub account_name: Option<String>,
    pub views: Option<u64>,
    pub comments: Option<u64>,
    pub business_date: Option<String>,
}

type Display = (Option<String>, Option<String>);

// Read the same visible title as the ledger. A frozen member whose live record is no longer
// visible still contributes its frozen numbers; title and account are then unavailable.
pub(super) async fn display(
    db: &Database,
    ids: &[Uuid],
) -> Result<BTreeMap<Uuid, Display>, LensDefinitionError> {
    let sql=format!("WITH {TITLED_PUBLICATIONS_CTE}
      SELECT t.publication_ref,t.title,a.account_name
      FROM titled_publications t JOIN linggan_ops_publication p USING(publication_ref)
      LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref WHERE t.publication_ref=ANY($1)");
    let rows: Vec<(Uuid, Option<String>, Option<String>)> =
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(ids)
            .fetch_all(db.pool())
            .await?;
    Ok(rows
        .into_iter()
        .map(|(id, title, account)| (id, (title, account)))
        .collect())
}

pub async fn publications(
    db: &Database,
    request: PageRequest,
) -> Result<PublicationPage, LensDefinitionError> {
    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(25);
    let subset = request.subset.unwrap_or(Subset::All);
    let metric = request.metric.unwrap_or(SampleMetric::Publications);
    if !(1..=100).contains(&limit)
        || (matches!(
            metric,
            SampleMetric::TitledPublications | SampleMetric::TitledViews
        ) && (!matches!(subset, Subset::All)
            || request.moment.is_some()
            || request.offer.is_some()
            || request.situation.is_some()))
    {
        return Err(LensDefinitionError::InvalidScope);
    }
    let (input, definition) = frozen(db, request.snapshot_ref).await?;
    let filter = LabelFilter {
        moment: request.moment,
        offer: request.offer,
        situation: request.situation,
    };
    let members = select_publications(&input, &filter, metric, subset, &definition);
    let total = members.len();
    let page = members.get(offset..).unwrap_or_default();
    let ids = &page[..limit.min(page.len())];
    let mut display = display(db, ids).await?;
    let items = ids
        .iter()
        .map(|id| {
            let row = &input.publications[id];
            let (title, account_name) = display.remove(id).unwrap_or_default();
            Publication {
                publication_ref: *id,
                title,
                account_name,
                views: row.views.value,
                comments: row.comments.value,
                business_date: row.business_date.clone(),
            }
        })
        .collect();
    Ok(PublicationPage {
        snapshot_ref: request.snapshot_ref,
        offset,
        limit,
        total,
        items,
    })
}
