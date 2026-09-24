//! Loopback-only Evidence Library JSON composition.
//!
//! `items` is the authoritative multi-material surface. Legacy cards are served only by the
//! explicit compatibility endpoint and are never mixed into this response.

use super::*;
use linggan_contracts::EvidenceQuery;
use linggan_evidence::{
    ContentReobservationError, WorkResourceReadError, content_reobservation_in_domain,
    read_authorized_research_comments, read_content_reobservation,
    read_content_reobservation_eligibility_in_domain, read_work_resource, read_work_resources,
    work_resource_schema_is_ready,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Deserialize)]
pub(super) struct EvidenceLibraryParams {
    pub(super) q: Option<String>,
    pub(super) window: Option<String>,
    pub(super) sort: Option<String>,
    pub(super) lane: Option<String>,
    #[serde(rename = "laneState")]
    pub(super) lane_state: Option<String>,
    #[serde(rename = "mediaKind")]
    pub(super) media_kind: Option<String>,
    pub(super) restriction: Option<String>,
    pub(super) cursor: Option<String>,
    pub(super) domain: Option<uuid::Uuid>,
}

#[derive(Deserialize)]
pub(super) struct MaterialChannelParams {
    cursor: Option<String>,
    q: Option<String>,
    domain: Option<uuid::Uuid>,
}

async fn material_is_in_domain(
    database: &Database,
    public_ref: uuid::Uuid,
    domain_ref: uuid::Uuid,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage \
         WHERE content_public_ref=$1 AND domain_ref=$2)",
    )
    .bind(public_ref)
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await
}

pub(super) async fn research_comments_json(
    State(state): State<LocalWebState>,
    Path(public_ref): Path<String>,
    Query(params): Query<MaterialChannelParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(public_ref) = uuid::Uuid::parse_str(&public_ref) else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    let Some(domain_ref) = params.domain else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "domain_required");
    };
    match material_is_in_domain(database, public_ref, domain_ref).await {
        Ok(true) => {}
        Ok(false) => {
            return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found");
        }
        Err(_) => {
            return local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "domain_material_read_unavailable",
            );
        }
    }
    let after = match params.cursor {
        Some(value) => match uuid::Uuid::parse_str(&value) {
            Ok(value) => Some(value),
            Err(_) => {
                return local_read_json_error(
                    axum::http::StatusCode::BAD_REQUEST,
                    "invalid_material_channel_cursor",
                );
            }
        },
        None => None,
    };
    let text = params.q.as_deref().filter(|value| !value.trim().is_empty());
    if text.is_some_and(|value| value.chars().count() > 200) {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "material_comment_query_too_long",
        );
    }
    match read_authorized_research_comments(database, public_ref, after, text).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => {
            eprintln!("authorized research comment read unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "material_comment_channel_unavailable",
            )
        }
    }
}

pub(super) async fn detail_json(
    State(state): State<LocalWebState>,
    Path(public_ref): Path<String>,
    Query(params): Query<MaterialChannelParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(public_ref) = uuid::Uuid::parse_str(&public_ref) else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    let Some(domain_ref) = params.domain else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "domain_required");
    };
    match material_is_in_domain(database, public_ref, domain_ref).await {
        Ok(true) => {}
        Ok(false) => {
            return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found");
        }
        Err(_) => {
            return local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "domain_material_read_unavailable",
            );
        }
    }
    match read_work_resource(database, public_ref).await {
        Ok(Some(item)) => {
            let comments_url =
                format!("/api/local/work-resources/{public_ref}/comments?domain={domain_ref}");
            let reobservation_url =
                format!("/api/local/work-resources/{public_ref}/reobserve?domain={domain_ref}");
            let eligibility = match read_content_reobservation_eligibility_in_domain(
                database, public_ref, domain_ref,
            )
            .await
            {
                Ok(Some(eligibility)) => eligibility,
                Ok(None) => {
                    return local_read_json_error(
                        axum::http::StatusCode::NOT_FOUND,
                        "material_not_found",
                    );
                }
                Err(error) => {
                    eprintln!("reobservation eligibility unavailable: {error}");
                    return local_read_json_error(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "reobservation_eligibility_unavailable",
                    );
                }
            };
            Json(json!({"item":item,"channels":{
                "comments":{"url":comments_url,"receipt":item.inspector.get("commentsReceipt")},
                "media":{"receipt":item.inspector.get("mediaSlotsReceipt")},
                "derivatives":{"receipt":item.inspector.get("derivativesReceipt")},
                "provenance":{"receipt":item.inspector.pointer("/provenance/receipt")},
                "reobservation":{
                    "url":if eligibility.eligible { Value::String(reobservation_url) } else { Value::Null },
                    "available":eligibility.eligible,
                    "supported":eligibility.supported,
                    "eligible":eligibility.eligible,
                    "reason":eligibility.reason,
                    "requires":"TARGET_LINKED_ACTIVE_DEEP_ARCHIVE_AUTHORIZATION",
                    "mediaPolicy":"EXISTING_ASSETS_REUSED"
                }
            }}))
            .into_response()
        }
        Ok(None) => local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found"),
        Err(error) => {
            eprintln!("material detail unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "material_detail_unavailable",
            )
        }
    }
}

/// This creates only the existing server-authorized acquisition chain. It never manufactures a
/// producer task or calls a platform; a claimed Browser Producer may later execute the lease.
pub(super) async fn reobserve_json(
    State(state): State<LocalWebState>,
    Path(public_ref): Path<String>,
    Query(params): Query<MaterialChannelParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(public_ref) = uuid::Uuid::parse_str(&public_ref) else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_material_ref");
    };
    let Some(domain_ref) = params.domain else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "domain_required");
    };
    match material_is_in_domain(database, public_ref, domain_ref).await {
        Ok(true) => {}
        Ok(false) => {
            return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found");
        }
        Err(_) => {
            return local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "domain_material_read_unavailable",
            );
        }
    }
    match content_reobservation_in_domain(database, public_ref, domain_ref).await {
        Ok(operation) => {
            let status_url = operation.lease_ref.map(|lease_ref| {
                format!("/api/local/work-resources/{public_ref}/reobserve/{lease_ref}?domain={domain_ref}")
            });
            Json(json!({"operation":operation,"statusUrl":status_url})).into_response()
        }
        Err(ContentReobservationError::WorkResourceNotFound) => {
            local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found")
        }
        Err(ContentReobservationError::PlatformNotSupported) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "reobservation_platform_not_supported",
        ),
        Err(ContentReobservationError::AuthorizedTargetMissing) => local_read_json_error(
            axum::http::StatusCode::CONFLICT,
            "reobservation_authorization_not_linked",
        ),
        Err(ContentReobservationError::Acquisition(error)) => {
            eprintln!("reobservation admission unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::CONFLICT,
                "reobservation_admission_unavailable",
            )
        }
        Err(ContentReobservationError::Lease(error)) => {
            eprintln!("reobservation lease unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::CONFLICT,
                "reobservation_lease_unavailable",
            )
        }
        Err(ContentReobservationError::Database(error)) => {
            eprintln!("reobservation database unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "reobservation_unavailable",
            )
        }
    }
}

pub(super) async fn reobservation_status_json(
    State(state): State<LocalWebState>,
    Path((public_ref, lease_ref)): Path<(String, String)>,
    Query(params): Query<MaterialChannelParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let (Ok(public_ref), Ok(lease_ref)) = (
        uuid::Uuid::parse_str(&public_ref),
        uuid::Uuid::parse_str(&lease_ref),
    ) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_reobservation_reference",
        );
    };
    let Some(domain_ref) = params.domain else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "domain_required");
    };
    match material_is_in_domain(database, public_ref, domain_ref).await {
        Ok(true) => {}
        Ok(false) => {
            return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "material_not_found");
        }
        Err(_) => {
            return local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "domain_material_read_unavailable",
            );
        }
    }
    match read_content_reobservation(database, public_ref, lease_ref).await {
        Ok(Some(operation)) => Json(json!({"operation":operation})).into_response(),
        Ok(None) => {
            local_read_json_error(axum::http::StatusCode::NOT_FOUND, "reobservation_not_found")
        }
        Err(error) => {
            eprintln!("reobservation status unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "reobservation_status_unavailable",
            )
        }
    }
}

pub(super) fn local_query(params: &EvidenceLibraryParams) -> Result<EvidenceQuery, ()> {
    let window = match params
        .window
        .as_deref()
        .unwrap_or("latest_accepted_discovery")
    {
        "latest_accepted_discovery" => "latest_accepted_discovery",
        "last_7_days" => "last_7_days",
        "last_30_days" => "last_30_days",
        _ => return Err(()),
    };
    let sort = match params.sort.as_deref().unwrap_or("latest_discovery") {
        "latest_discovery" => "latest_discovery",
        "relevance" => "relevance",
        _ => return Err(()),
    };
    serde_json::from_value(json!({
        "text": params.q,
        "scope": "all_accepted_material",
        "window": window,
        "sort": sort,
        "lane": params.lane,
        "laneState": params.lane_state,
        "mediaKind": params.media_kind,
        "restriction": params.restriction,
        "cursor": params.cursor,
        "domainRef": params.domain
    }))
    .map_err(|_| ())
}

pub(super) async fn compose_json(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<Value, WorkResourceReadError> {
    if !work_resource_schema_is_ready(database).await? {
        return match read_work_resources(database, query).await {
            Err(error @ WorkResourceReadError::InvalidCursor)
            | Err(error @ WorkResourceReadError::UnsupportedSort) => Err(error),
            Err(_) | Ok(_) => Err(WorkResourceReadError::ProjectionUnavailable),
        };
    }
    let material = read_work_resources(database, query).await?;
    let mut value = serde_json::to_value(material).map_err(|error| {
        WorkResourceReadError::Database(sqlx::Error::Protocol(error.to_string()))
    })?;
    if let Some(items) = value.get_mut("items").and_then(Value::as_array_mut) {
        for item in items {
            let public_ref = item
                .pointer("/identity/publicRef")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if let Some(object) = item.as_object_mut() {
                object.remove("inspector");
                object.insert(
                    "detailUrl".to_owned(),
                    public_ref.map_or(Value::Null, |public_ref| {
                        Value::String(format!(
                            "/api/local/work-resources/{public_ref}?domain={}",
                            query
                                .domain_ref()
                                .map(|value| value.to_string())
                                .unwrap_or_default()
                        ))
                    }),
                );
            }
        }
    }
    if let (Some(domain_ref), Some(items)) = (
        query.domain_ref(),
        value.get_mut("items").and_then(Value::as_array_mut),
    ) {
        let public_refs = items
            .iter()
            .filter_map(|item| {
                item.pointer("/identity/publicRef")
                    .and_then(Value::as_str)
                    .and_then(|value| uuid::Uuid::parse_str(value).ok())
            })
            .collect::<Vec<_>>();
        let usages: Vec<(uuid::Uuid, bool, Vec<String>)> = sqlx::query_as(
            "SELECT content_public_ref,bool_or(role='primary'),array_agg(DISTINCT basis_kind ORDER BY basis_kind) \
             FROM linggan_material_domain_usage \
             WHERE domain_ref=$1 AND content_public_ref=ANY($2) \
             GROUP BY content_public_ref",
        )
        .bind(domain_ref)
        .bind(&public_refs)
        .fetch_all(database.pool())
        .await?;
        for item in items {
            let Some(public_ref) = item
                .pointer("/identity/publicRef")
                .and_then(Value::as_str)
                .and_then(|value| uuid::Uuid::parse_str(value).ok())
            else {
                continue;
            };
            if let Some((_, has_primary, basis_kinds)) =
                usages.iter().find(|row| row.0 == public_ref)
                && let Some(object) = item.as_object_mut()
            {
                object.insert(
                    "domainUsage".to_owned(),
                    json!({"role":if *has_primary {"primary"} else {"reference"},"basisKinds":basis_kinds}),
                );
            }
        }
    }
    Ok(value)
}
