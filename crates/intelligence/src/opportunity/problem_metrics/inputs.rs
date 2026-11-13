//! A single as-of input set shared by current and shadow metric calculations.
use super::{MetricCalculationError, MetricDefinition, ProtocolVersion};
use crate::opportunity::{CommenterDigest, PeriodBasis};
use serde::Serialize;
use sqlx::{Postgres, Row, Transaction};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

pub(super) type CommentKey = (Uuid, String);

#[derive(Serialize)]
pub(super) struct EligibleComment {
    pub(super) period: String,
    pub(super) protocol: ProtocolVersion,
    pub(super) hash: Option<String>,
}

#[derive(Default)]
pub(super) struct MetricInputs {
    pub(super) eligible: BTreeMap<CommentKey, EligibleComment>,
    pub(super) denominators: BTreeMap<ProtocolVersion, BTreeMap<String, u64>>,
    pub(super) mentions: BTreeMap<Uuid, BTreeSet<CommentKey>>,
}

fn protocol(value: Option<serde_json::Value>) -> Result<ProtocolVersion, MetricCalculationError> {
    match value {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => {
            let version = value.as_i64().and_then(|v| i32::try_from(v).ok());
            match version {
                Some(version) if version > 0 => Ok(Some(version)),
                _ => Err(MetricCalculationError::InvalidProtocol),
            }
        }
    }
}

pub(super) async fn read_inputs(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    definition: &MetricDefinition,
    taxonomy_version: i64,
    digest: &CommenterDigest,
) -> Result<MetricInputs, MetricCalculationError> {
    let basis = match definition.params.period_basis {
        PeriodBasis::WorkPublishedCohort => "work_published_cohort",
        PeriodBasis::CommentPublished => "comment_published",
    };
    let rows = sqlx::query(
        "SELECT eligible.content_ref, eligible.comment_external_id, eligible.commenter_external_id,
                eligible.period_start::text AS period_start,
                package.payload->'collectionProtocol'->'version' AS protocol_version
         FROM linggan_opportunity_eligible_comments($1, $2::timestamptz, $3, $4) eligible
         LEFT JOIN linggan_material_comment_first_admission admission
           ON admission.content_public_ref=eligible.content_ref
          AND admission.comment_external_id=eligible.comment_external_id
         LEFT JOIN linggan_runtime_capture_package package
           ON package.package_ref=admission.first_package_ref AND package.accepted_at <= $2::timestamptz
          AND EXISTS (
            SELECT 1 FROM linggan_material_comment observation
            WHERE observation.content_public_ref=eligible.content_ref
              AND observation.comment_external_id=eligible.comment_external_id
              AND observation.package_ref=package.package_ref
              AND observation.created_at <= $2::timestamptz
          )",
    )
    .bind(domain)
    .bind(as_of)
    .bind(&definition.params.period_tz)
    .bind(basis)
    .fetch_all(&mut **tx)
    .await?;
    let mut inputs = MetricInputs::default();
    for row in rows {
        let protocol = protocol(row.try_get("protocol_version")?)?;
        let period: String = row.try_get("period_start")?;
        let author: Option<String> = row.try_get("commenter_external_id")?;
        let key = (
            row.try_get("content_ref")?,
            row.try_get("comment_external_id")?,
        );
        let comment = EligibleComment {
            period: period.clone(),
            protocol,
            hash: digest.hash(author.as_deref()),
        };
        if inputs.eligible.insert(key, comment).is_some() {
            return Err(MetricCalculationError::InvalidCounts);
        }
        *inputs
            .denominators
            .entry(protocol)
            .or_default()
            .entry(period)
            .or_default() += 1;
    }
    if inputs.denominators.is_empty() {
        inputs.denominators.insert(None, BTreeMap::new());
    }
    read_mentions(tx, domain, as_of, taxonomy_version, digest, &mut inputs).await?;
    Ok(inputs)
}

async fn read_mentions(
    tx: &mut Transaction<'_, Postgres>,
    domain: Uuid,
    as_of: &str,
    taxonomy_version: i64,
    digest: &CommenterDigest,
    inputs: &mut MetricInputs,
) -> Result<(), MetricCalculationError> {
    let rows = sqlx::query(
        "SELECT mention.problem_ref, mention.content_ref, mention.comment_external_id,
                mention.commenter_digest_version
         FROM linggan_opportunity_comment_mention mention
         JOIN linggan_opportunity_problem problem USING(problem_ref)
         JOIN linggan_comment_problem_membership source
           ON source.request_ref=mention.source_request_ref
          AND source.signal_ordinal=mention.source_signal_ordinal
         WHERE problem.domain_ref=$1 AND mention.taxonomy_version=$2
           AND source.created_at <= $3::timestamptz
           AND mention.excluded_reason IS DISTINCT FROM 'marketing'",
    )
    .bind(domain)
    .bind(taxonomy_version)
    .bind(as_of)
    .fetch_all(&mut **tx)
    .await?;
    for row in rows {
        let version: String = row.try_get("commenter_digest_version")?;
        if version != digest.digest_version() {
            return Err(MetricCalculationError::DigestVersionMismatch);
        }
        let key = (
            row.try_get("content_ref")?,
            row.try_get("comment_external_id")?,
        );
        if inputs.eligible.contains_key(&key) {
            inputs
                .mentions
                .entry(row.try_get("problem_ref")?)
                .or_default()
                .insert(key);
        }
    }
    Ok(())
}
