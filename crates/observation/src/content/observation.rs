//! Reading the frozen `content-detail.synthetic.v1` payload and appending one observation.

use serde_json::Value;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::processing::ProcessingError;

const PAYLOAD_SCHEMA_VERSION: &str = "content-detail.synthetic.v1";

/// One field as the payload stated it. `observed = false` means the producer explicitly did not
/// observe the field; it never becomes an empty string.
pub(crate) struct ObservedField {
    pub(crate) observed: bool,
    pub(crate) value: Option<String>,
}

/// The parts of a validated payload the observation needs.
pub(crate) struct ParsedPayload {
    pub(crate) source_external_id: Option<String>,
    pub(crate) title: ObservedField,
    pub(crate) body: ObservedField,
}

/// Validates the payload subtree against its closed contract. Anything outside it is a per-record
/// contract failure, never an ingress failure and never a silent default.
pub(crate) fn parse_payload(payload: &Value) -> Option<ParsedPayload> {
    let object = payload.as_object()?;
    if object.len() != 3 {
        return None;
    }
    if object.get("schemaVersion")?.as_str()? != PAYLOAD_SCHEMA_VERSION {
        return None;
    }

    let source_external_id = match object.get("sourceExternalId")? {
        Value::Null => None,
        Value::String(value) => Some(value.clone()),
        _ => return None,
    };

    let fields = object.get("fields")?.as_object()?;
    if fields.len() != 2 {
        return None;
    }
    Some(ParsedPayload {
        source_external_id,
        title: parse_field(fields.get("title")?)?,
        body: parse_field(fields.get("body")?)?,
    })
}

fn parse_field(field: &Value) -> Option<ObservedField> {
    let object = field.as_object()?;
    if object.len() != 2 {
        return None;
    }
    match (object.get("observed")?, object.get("value")?) {
        (Value::Bool(true), Value::String(value)) => Some(ObservedField {
            observed: true,
            value: Some(value.clone()),
        }),
        (Value::Bool(false), Value::Null) => Some(ObservedField {
            observed: false,
            value: None,
        }),
        _ => None,
    }
}

/// Appends the observation this record supports. `observed_at` is copied from the record itself
/// so no processing, receive or insert time can stand in for the producer's observation instant.
pub(crate) async fn append_observation(
    transaction: &mut Transaction<'_, Postgres>,
    source_content_id: i64,
    capture_record_id: i64,
    payload: &ParsedPayload,
    observation_ref: Uuid,
) -> Result<(), ProcessingError> {
    sqlx::query(
        "INSERT INTO content_observation \
             (observation_ref, source_content_id, capture_record_id, observed_at, \
              observed_at_precision, parser_version, title_observed, title_value, \
              body_observed, body_value) \
         SELECT $1, $2, r.id, r.observed_at, r.observed_at_precision, \
                'content-detail-processor-v1', $4, $5, $6, $7 \
         FROM capture_record r WHERE r.id = $3",
    )
    .bind(observation_ref)
    .bind(source_content_id)
    .bind(capture_record_id)
    .bind(payload.title.observed)
    .bind(payload.title.value.as_deref())
    .bind(payload.body.observed)
    .bind(payload.body.value.as_deref())
    .execute(&mut **transaction)
    .await
    .map_err(ProcessingError::internal)?;
    Ok(())
}
