//! Pick qualified source voices using the existing catalog and its shared like selector.
use super::{CardError, model};
use crate::{
    comment_cleaning::CLEANER_VERSION,
    comment_study_catalog,
    comment_study_source::{StudySourceRole, selected_source_in},
};
use sqlx::{Postgres, Row, Transaction};
use std::collections::BTreeMap;
use uuid::Uuid;

// This is the fixed maximum in the card response contract, not a metric threshold.
const MAX_SAMPLE_VOICES: usize = 3;

pub(super) async fn select(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    problem: Uuid,
    taxonomy: i64,
    as_of: &str,
    tz: &str,
    basis: &str,
) -> Result<(Vec<model::Voice>, BTreeMap<String, usize>), CardError> {
    let sql = format!(
        "{}, {}, eligible AS MATERIALIZED (\
         SELECT * FROM linggan_opportunity_eligible_comments($1,$2::timestamptz,$6,$7)) \
         SELECT q.material_ref,q.content_public_ref,q.comment_external_id,q.exclusion_reason,\
           likes.like_count,original.body_text IS NOT DISTINCT FROM current.body_text AS same_body \
         FROM qualified q JOIN eligible ON eligible.content_ref=q.content_public_ref \
           AND eligible.comment_external_id=q.comment_external_id \
         JOIN linggan_opportunity_comment_mention mention ON mention.content_ref=q.content_public_ref \
           AND mention.comment_external_id=q.comment_external_id AND mention.problem_ref=$8 \
           AND mention.taxonomy_version=$9 AND mention.excluded_reason IS DISTINCT FROM 'marketing' \
         JOIN linggan_comment_problem_membership membership ON membership.request_ref=mention.source_request_ref \
           AND membership.signal_ordinal=mention.source_signal_ordinal AND membership.created_at<=$2::timestamptz \
         JOIN linggan_material_comment original ON original.material_ref=mention.source_material_ref \
         LEFT JOIN linggan_material_comment current ON current.material_ref=q.material_ref \
         LEFT JOIN like_heads likes ON likes.content_public_ref=q.content_public_ref \
           AND likes.comment_external_id=q.comment_external_id \
         ORDER BY likes.like_count DESC NULLS LAST,eligible.first_observed_at,\
                  q.content_public_ref,q.comment_external_id",
        comment_study_catalog::facts_sql(),
        comment_study_catalog::like_heads_sql()
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(domain)
        .bind(as_of)
        .bind(CLEANER_VERSION)
        .bind(Option::<Uuid>::None)
        .bind(Option::<String>::None)
        .bind(tz)
        .bind(basis)
        .bind(problem)
        .bind(taxonomy)
        .fetch_all(&mut **tx)
        .await?;
    let mut voices = Vec::new();
    let mut excluded = BTreeMap::new();
    for row in rows {
        if let Some(reason) = row.try_get::<Option<String>, _>("exclusion_reason")? {
            *excluded.entry(reason).or_default() += 1;
            continue;
        }
        if voices.len() == MAX_SAMPLE_VOICES {
            continue;
        }
        if !row.try_get::<bool, _>("same_body")? {
            *excluded.entry("sourceBodyChanged".to_owned()).or_default() += 1;
            continue;
        }
        let content_ref: Uuid = row.try_get("content_public_ref")?;
        let source_ref: Uuid = row.try_get("material_ref")?;
        let Some(source) = selected_source_in(
            tx,
            domain,
            content_ref,
            source_ref,
            StudySourceRole::Primary,
            as_of,
        )
        .await?
        else {
            *excluded.entry("sourceUnavailable".to_owned()).or_default() += 1;
            continue;
        };
        voices.push(model::Voice {
            comment_ref: model::CommentRef {
                content_ref,
                comment_external_id: row.try_get("comment_external_id")?,
            },
            text: Some(source.raw_text),
            likes: row.try_get("like_count")?,
            work_id: content_ref,
            tombstone: false,
        });
    }
    Ok((voices, excluded))
}
