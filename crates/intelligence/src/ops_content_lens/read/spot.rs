use super::{LensDefinitionError, frozen};
use crate::ops_content_lens::LabelDimension as Dim;
use crate::ops_content_lens::{LabelDimension, TITLED_PUBLICATIONS_CTE};
use linggan_storage_postgres::Database;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotCheck {
    pub snapshot_ref: Uuid,
    pub items: Vec<SpotItem>,
    pub vocabulary: BTreeMap<Dim, Vec<Choice>>,
}
#[derive(Serialize, Deserialize)]
pub struct Choice {
    key: String,
    name: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotItem {
    pub title_key: String,
    pub effective: BTreeMap<Dim, String>,
    pub run_ref: Uuid,
    pub publication_ref: Uuid,
    pub title: Option<String>,
    pub moment: String,
    pub offer: String,
    pub situation: String,
    pub audience: String,
    pub confidence: String,
}

pub async fn spot_check(db: &Database, id: Uuid) -> Result<SpotCheck, LensDefinitionError> {
    let (input, _) = frozen(db, id).await?;
    // Each title is counted once. Take its frozen primary label, even when a human
    // later adjudicated a dimension or a newer primary method labeled the same title.
    let mut titles = BTreeMap::new();
    for (publication_ref, row) in &input.publications {
        if let Some(label) = row.labels.get(&LabelDimension::Moment) {
            titles
                .entry(row.title_key.as_str())
                .or_insert((label.run_ref, *publication_ref));
        }
    }
    let mut keys = Vec::with_capacity(titles.len());
    let mut runs = Vec::with_capacity(titles.len());
    for (key, (run, _)) in &titles {
        keys.push(*key);
        runs.push(*run);
    }
    let rows=sqlx::query("WITH candidates AS (
        SELECT l.*,md5($3::text||l.title_key) AS draw
        FROM unnest($1::text[],$2::uuid[]) x(title_key,run_ref)
        JOIN linggan_ops_lens_label l USING(title_key,run_ref)
      ), low AS (SELECT title_key FROM candidates WHERE confidence='low' ORDER BY draw,title_key LIMIT 15)
      SELECT c.title_key,c.run_ref,c.moment,c.offer,c.situation,c.audience,c.confidence
      FROM candidates c LEFT JOIN low USING(title_key)
      ORDER BY (low.title_key IS NOT NULL) DESC,c.draw,c.title_key LIMIT 30")
        .bind(&keys).bind(&runs).bind(id.to_string()).fetch_all(db.pool()).await?;
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let title_key: String = row.try_get("title_key")?;
        let (_, publication_ref) = titles[title_key.as_str()];
        items.push(SpotItem {
            effective: input.publications[&publication_ref]
                .labels
                .iter()
                .map(|(dim, l)| (dim.clone(), l.value.clone()))
                .collect(),
            title_key,
            publication_ref,
            title: None,
            run_ref: row.try_get("run_ref")?,
            moment: row.try_get("moment")?,
            offer: row.try_get("offer")?,
            situation: row.try_get("situation")?,
            audience: row.try_get("audience")?,
            confidence: row.try_get("confidence")?,
        });
    }
    // A current title can be shown beside a frozen machine label only if its key still
    // matches. A sibling publication of the same title may retain a valid display.
    let selected: std::collections::BTreeSet<_> =
        items.iter().map(|r| r.title_key.as_str()).collect();
    let (refs, keys): (Vec<_>, Vec<_>) = input
        .publications
        .iter()
        .filter(|(_, r)| selected.contains(r.title_key.as_str()))
        .map(|(id, r)| (*id, r.title_key.as_str()))
        .unzip();
    let sql=format!("WITH {TITLED_PUBLICATIONS_CTE}
        SELECT DISTINCT ON(x.key) x.key,t.publication_ref,t.title FROM unnest($1::uuid[],$2::text[]) x(ref,key)
        JOIN titled_publications t ON t.publication_ref=x.ref AND linggan_ops_lens_title_key(t.title)=x.key
        ORDER BY x.key,t.publication_ref");
    let rows: Vec<(String, Uuid, String)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(refs)
        .bind(keys)
        .fetch_all(db.pool())
        .await?;
    let mut names: BTreeMap<_, _> = rows
        .into_iter()
        .map(|(key, id, title)| (key, (id, title)))
        .collect();
    for item in &mut items {
        if let Some((id, title)) = names.remove(&item.title_key) {
            item.publication_ref = id;
            item.title = Some(title);
        }
    }
    let (vocabulary,): (sqlx::types::Json<BTreeMap<Dim, Vec<Choice>>>,) = sqlx::query_as(
        "SELECT body - '_meta' FROM linggan_ops_lens_definition WHERE definition_ref=$1",
    )
    .bind(input.method.taxonomy_ref)
    .fetch_one(db.pool())
    .await?;
    Ok(SpotCheck {
        vocabulary: vocabulary.0,
        snapshot_ref: id,
        items,
    })
}
