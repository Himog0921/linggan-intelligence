//! `content-current-policy-v1`: recomputing one immutable current revision from the observations
//! this content actually holds, then publishing the pointer.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use crate::processing::ProcessingError;

const POLICY_VERSION: &str = "content-current-policy-v1";

/// One observation as the policy sees it.
struct ObservationInput {
    id: i64,
    observed_at: String,
    title: Option<String>,
    body: Option<String>,
}

/// What the policy decided for one field, and which observations it fixes as the reason.
struct FieldResolution {
    state: &'static str,
    value: Option<String>,
    /// `(observation id, role)`; empty for `unknown`.
    sources: Vec<(i64, &'static str)>,
}

/// Recomputes and publishes the current revision for one content. Callers pass the refs for the
/// revision and for the title and body field sources, in that order.
pub(crate) async fn republish_current(
    transaction: &mut Transaction<'_, Postgres>,
    source_content_id: i64,
    revision_ref: Uuid,
    title_source_ref: Uuid,
    body_source_ref: Uuid,
) -> Result<(), ProcessingError> {
    let observations = load_observations(transaction, source_content_id).await?;
    let title = resolve_field(&observations, |observation| observation.title.as_deref());
    let body = resolve_field(&observations, |observation| observation.body.as_deref());
    let watermark = build_watermark(transaction, source_content_id).await?;

    let revision_id = sqlx::query(
        "INSERT INTO content_current_revision \
             (revision_ref, source_content_id, policy_version, title_state, title_value, \
              body_state, body_value, watermark) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(revision_ref)
    .bind(source_content_id)
    .bind(POLICY_VERSION)
    .bind(title.state)
    .bind(title.value.as_deref())
    .bind(body.state)
    .bind(body.value.as_deref())
    .bind(&watermark)
    .fetch_one(&mut **transaction)
    .await
    .map(|row| row.get::<i64, _>("id"))
    .map_err(ProcessingError::internal)?;

    insert_field_sources(
        transaction,
        revision_id,
        source_content_id,
        "title",
        &title,
        title_source_ref,
    )
    .await?;
    insert_field_sources(
        transaction,
        revision_id,
        source_content_id,
        "body",
        &body,
        body_source_ref,
    )
    .await?;

    // Readers only ever follow a published pointer, so the revision is complete before this runs.
    sqlx::query("UPDATE source_content SET current_revision_id = $1 WHERE id = $2")
        .bind(revision_id)
        .bind(source_content_id)
        .execute(&mut **transaction)
        .await
        .map_err(ProcessingError::internal)?;
    Ok(())
}

/// Policy v1: consider only observations that observed the field, keep the latest observation
/// instant, and take the value only when that latest set agrees. Same instant with different
/// values is `unresolved`; nothing qualified is `unknown`. Receive, accept and insert order are
/// never consulted.
fn resolve_field(
    observations: &[ObservationInput],
    field: impl Fn(&ObservationInput) -> Option<&str>,
) -> FieldResolution {
    let qualified: Vec<&ObservationInput> = observations
        .iter()
        .filter(|observation| field(observation).is_some())
        .collect();
    let Some(latest) = qualified
        .iter()
        .map(|observation| observation.observed_at.as_str())
        .max()
    else {
        return FieldResolution {
            state: "unknown",
            value: None,
            sources: Vec::new(),
        };
    };

    let newest: Vec<&&ObservationInput> = qualified
        .iter()
        .filter(|observation| observation.observed_at == latest)
        .collect();
    let mut distinct: Vec<&str> = newest
        .iter()
        .filter_map(|observation| field(observation))
        .collect();
    distinct.sort_unstable();
    distinct.dedup();

    if distinct.len() == 1 {
        FieldResolution {
            state: "selected",
            value: Some(distinct[0].to_owned()),
            sources: newest
                .iter()
                .map(|observation| (observation.id, "selected_support"))
                .collect(),
        }
    } else {
        FieldResolution {
            state: "unresolved",
            value: None,
            sources: newest
                .iter()
                .map(|observation| (observation.id, "conflicting_candidate"))
                .collect(),
        }
    }
}

async fn load_observations(
    transaction: &mut Transaction<'_, Postgres>,
    source_content_id: i64,
) -> Result<Vec<ObservationInput>, ProcessingError> {
    let rows = sqlx::query(
        "SELECT id, \
                to_char(observed_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS observed_at, \
                title_value, body_value \
         FROM content_observation WHERE source_content_id = $1 ORDER BY id",
    )
    .bind(source_content_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;

    Ok(rows
        .iter()
        .map(|row| ObservationInput {
            id: row.get("id"),
            observed_at: row.get("observed_at"),
            title: row.get("title_value"),
            body: row.get("body_value"),
        })
        .collect())
}

/// The exact inputs this recomputation locked, as fixed public refs. A deferred constraint
/// trigger re-derives the same value and rejects any drift.
async fn build_watermark(
    transaction: &mut Transaction<'_, Postgres>,
    source_content_id: i64,
) -> Result<Value, ProcessingError> {
    let rows = sqlx::query(
        "SELECT p.package_ref, d.delivery_ref, p.accepted_receipt_ref, r.record_ref, o.observation_ref \
         FROM content_observation o \
         JOIN capture_record r ON r.id = o.capture_record_id \
         JOIN capture_package p ON p.id = r.package_id \
         JOIN capture_ingress_delivery d ON d.id = p.accepted_delivery_id \
         WHERE o.source_content_id = $1",
    )
    .bind(source_content_id)
    .fetch_all(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;

    let mut packages: BTreeMap<String, (String, String, Vec<String>, Vec<String>)> =
        BTreeMap::new();
    for row in &rows {
        let package_ref = row.get::<Uuid, _>("package_ref").to_string();
        let entry = packages.entry(package_ref).or_insert_with(|| {
            (
                row.get::<Uuid, _>("delivery_ref").to_string(),
                row.get::<Uuid, _>("accepted_receipt_ref").to_string(),
                Vec::new(),
                Vec::new(),
            )
        });
        entry.2.push(row.get::<Uuid, _>("record_ref").to_string());
        entry
            .3
            .push(row.get::<Uuid, _>("observation_ref").to_string());
    }

    Ok(Value::Array(
        packages
            .into_iter()
            .map(
                |(package_ref, (delivery_ref, receipt_ref, records, observations))| {
                    json!({
                        "packageRef": package_ref,
                        "originalAcceptedDeliveryRef": delivery_ref,
                        "acceptedReceiptRef": receipt_ref,
                        "recordRefs": sorted_unique(records),
                        "observationRefs": sorted_unique(observations),
                    })
                },
            )
            .collect(),
    ))
}

fn sorted_unique(mut refs: Vec<String>) -> Vec<String> {
    refs.sort();
    refs.dedup();
    refs
}

async fn insert_field_sources(
    transaction: &mut Transaction<'_, Postgres>,
    revision_id: i64,
    source_content_id: i64,
    field_kind: &str,
    resolution: &FieldResolution,
    first_ref: Uuid,
) -> Result<(), ProcessingError> {
    for (index, (observation_id, role)) in resolution.sources.iter().enumerate() {
        // The frozen manifest fixes one ref per field for the single-observation proof; further
        // sources of the same field get their own generated refs.
        let field_source_ref = if index == 0 {
            first_ref
        } else {
            Uuid::new_v4()
        };
        sqlx::query(
            "INSERT INTO content_current_revision_field_source \
                 (field_source_ref, revision_id, source_content_id, field_kind, observation_id, role) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(field_source_ref)
        .bind(revision_id)
        .bind(source_content_id)
        .bind(field_kind)
        .bind(observation_id)
        .bind(*role)
        .execute(&mut **transaction)
        .await
        .map_err(ProcessingError::internal)?;
    }
    Ok(())
}
