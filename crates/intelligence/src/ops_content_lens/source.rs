use super::{
    AnalysisInput, CountSource, CreationLink, EffectiveLabel, LabelDimension, LabelMethod,
    LensDefinitionError, PublicationInput, SnapshotScope, SourcedCount,
};
use crate::ops_ledger::read::LATEST_VIEWS_CTE;
use sqlx::{PgConnection, Row};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

/// Blind labels use only these two columns. As in the Ops ledger, the latest adoption
/// decides the title, including a latest conflict/empty superseding an older adoption.
pub const TITLED_PUBLICATIONS_CTE: &str = r#"titled_publications AS (
    SELECT p.publication_ref,
        (SELECT CASE WHEN f.disposition='adopted' THEN f.value_text END
         FROM linggan_ops_publication_field_adoption f
         WHERE f.publication_ref=p.publication_ref AND f.field_name='title'
           AND f.publication_version IS NOT DISTINCT FROM p.native_version
         ORDER BY f.observed_at::timestamptz DESC,f.created_at DESC,f.adoption_ref DESC LIMIT 1) AS title
    FROM linggan_ops_publication p LEFT JOIN linggan_ops_account a USING(account_ref)
    WHERE p.platform='xhs' AND linggan_ops_publication_visible(p,a,NULL,NULL)
)"#;

pub(super) async fn read_input(
    conn: &mut PgConnection,
    scope: &SnapshotScope,
    method: LabelMethod,
) -> Result<AnalysisInput, LensDefinitionError> {
    let sql = format!(
        r#"WITH {TITLED_PUBLICATIONS_CTE},{LATEST_VIEWS_CTE}
        SELECT p.publication_ref,p.account_ref,coalesce(p.published_business_date,p.native_published_on)::text AS day,
          CASE WHEN t.title ~ '[^[:space:]]' THEN linggan_ops_lens_title_key(t.title) END AS key,
          v.report_ref,v.view_count,comment.adoption_ref,comment.value
        FROM titled_publications t JOIN linggan_ops_publication p USING(publication_ref)
        LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref
        LEFT JOIN latest_views v USING(publication_ref)
        LEFT JOIN LATERAL (SELECT f.adoption_ref,CASE WHEN f.disposition='adopted' THEN f.value_count END AS value
          FROM linggan_ops_publication_field_adoption f WHERE f.publication_ref=p.publication_ref AND f.field_name='comment_count'
            AND f.publication_version IS NOT DISTINCT FROM p.native_version
          ORDER BY f.observed_at::timestamptz DESC,f.created_at DESC,f.adoption_ref DESC LIMIT 1) comment ON true
        WHERE ($1::uuid IS NULL OR a.domain_ref=$1)
          AND ($2::text IS NULL OR coalesce(p.published_business_date,p.native_published_on)>=$2::text::date)
          AND ($3::text IS NULL OR coalesce(p.published_business_date,p.native_published_on)<=$3::text::date)
        ORDER BY p.publication_ref"#
    );
    let raw = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(scope.domain_ref)
        .bind(&scope.from)
        .bind(&scope.to)
        .fetch_all(&mut *conn)
        .await?;
    let total_publications = raw.len() as u64;
    let mut publications = BTreeMap::new();
    let mut titles = BTreeSet::new();
    for row in raw {
        let Some(key) = row.try_get::<Option<String>, _>("key")? else {
            continue;
        };
        titles.insert(key.clone());
        let reading = row.try_get::<Option<i64>, _>("view_count")?;
        let comments = row.try_get::<Option<i64>, _>("value")?;
        publications.insert(
            row.try_get("publication_ref")?,
            PublicationInput {
                title_key: key,
                account_ref: row.try_get("account_ref")?,
                business_date: row.try_get("day")?,
                labels: BTreeMap::new(),
                creation_links: BTreeMap::new(),
                views: SourcedCount {
                    value: reading.map(|v| v as u64),
                    source: row
                        .try_get::<Option<Uuid>, _>("report_ref")?
                        .map(CountSource::ViewReport),
                },
                comments: SourcedCount {
                    value: comments.map(|v| v as u64),
                    source: row
                        .try_get::<Option<Uuid>, _>("adoption_ref")?
                        .map(CountSource::FieldAdoption),
                },
            },
        );
    }
    let keys = titles.into_iter().collect::<Vec<_>>();
    let labels=sqlx::query("SELECT title_key,dimension,value,run_ref,adjudication_ref FROM linggan_ops_lens_effective_label
        WHERE taxonomy_ref=$1 AND task_config_ref=$2 AND prompt_version_ref=$3 AND title_key=ANY($4)")
        .bind(method.taxonomy_ref).bind(method.task_config_ref).bind(method.prompt_version_ref).bind(&keys).fetch_all(&mut *conn).await?;
    let mut by_title = BTreeMap::<String, BTreeMap<LabelDimension, EffectiveLabel>>::new();
    for row in labels {
        let dim = match row.try_get::<&str, _>("dimension")? {
            "moment" => LabelDimension::Moment,
            "offer" => LabelDimension::Offer,
            "situation" => LabelDimension::Situation,
            "audience" => LabelDimension::Audience,
            _ => return Err(LensDefinitionError::InvalidDefinition),
        };
        by_title
            .entry(row.try_get("title_key")?)
            .or_default()
            .insert(
                dim,
                EffectiveLabel {
                    value: row.try_get("value")?,
                    run_ref: row.try_get("run_ref")?,
                    adjudication_ref: row.try_get("adjudication_ref")?,
                },
            );
    }
    // A primary run with labels but no qualified audit must not masquerade as an unlabeled title.
    let pending: Vec<_> = keys
        .iter()
        .filter(|k| !by_title.contains_key(*k))
        .cloned()
        .collect();
    let unaudited:bool=sqlx::query_scalar("SELECT EXISTS(SELECT FROM linggan_ops_lens_label_run r JOIN linggan_ops_lens_label l USING(run_ref)
       WHERE r.run_kind='primary' AND r.taxonomy_ref=$1 AND r.task_config_ref=$2 AND r.prompt_version_ref=$3 AND l.title_key=ANY($4))")
        .bind(method.taxonomy_ref).bind(method.task_config_ref).bind(method.prompt_version_ref).bind(&pending).fetch_one(&mut *conn).await?;
    if unaudited || (!keys.is_empty() && by_title.is_empty()) {
        return Err(LensDefinitionError::MissingAgreement);
    }
    for row in publications.values_mut() {
        if let Some(labels) = by_title.get(&row.title_key) {
            row.labels = labels.clone();
        }
    }
    let links=sqlx::query("SELECT publication_ref,link_ref,work_ref,revision_ref FROM linggan_creation_publication_link l
        WHERE publication_ref=ANY($1) AND NOT EXISTS (SELECT FROM linggan_creation_publication_unlink u WHERE u.link_ref=l.link_ref)")
        .bind(publications.keys().copied().collect::<Vec<Uuid>>()).fetch_all(&mut *conn).await?;
    for row in links {
        let id: Uuid = row.try_get("publication_ref")?;
        publications
            .get_mut(&id)
            .expect("links selected from scope")
            .creation_links
            .insert(
                row.try_get("link_ref")?,
                CreationLink {
                    work_ref: row.try_get("work_ref")?,
                    revision_ref: row.try_get("revision_ref")?,
                },
            );
    }
    Ok(AnalysisInput {
        domain_ref: scope.domain_ref,
        from: scope.from.clone(),
        to: scope.to.clone(),
        method,
        total_publications,
        with_title: publications.len() as u64,
        pending_titles: pending.len() as u64,
        publications,
    })
}
