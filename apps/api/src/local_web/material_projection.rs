//! Loopback-only Evidence Library JSON composition.
//!
//! `items` is the authoritative multi-material surface. Legacy cards are served only by the
//! explicit compatibility endpoint and are never mixed into this response.

use super::*;
use linggan_contracts::EvidenceQuery;
use linggan_evidence::{
    WorkResourceReadError, read_authorized_research_comments, read_work_resource,
    read_work_resources, work_resource_schema_is_ready,
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
}

pub(super) async fn legacy_json(
    State(state): State<LocalWebState>,
    Query(params): Query<EvidenceLibraryParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(query) = local_query(&params) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_local_evidence_query",
        );
    };
    match super::read_evidence_library(database, &query).await {
        Ok(mut projection) => {
            let truncated = projection.cards.len() > 50;
            if truncated {
                projection.cards.truncate(50);
            }
            let returned = projection.cards.len();
            Json(json!({
                "compatibilityState":"EXPLICIT_LEGACY_DISCOVERY","cards":projection.cards,
                "returned":returned,"truncated":truncated,"nextCursor":Value::Null,
                "excludedUnknownPublishedAt":projection.excluded_unknown_published_at,
                "timeView":projection.time_view
            }))
            .into_response()
        }
        Err(error) => {
            eprintln!("legacy evidence projection unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "legacy_read_projection_unavailable",
            )
        }
    }
}

#[derive(Deserialize)]
pub(super) struct MaterialChannelParams {
    cursor: Option<String>,
    q: Option<String>,
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
    match read_work_resource(database, public_ref).await {
        Ok(Some(item)) => {
            let comments_url = format!("/api/local/work-resources/{public_ref}/comments");
            Json(json!({"item":item,"channels":{
                "comments":{"url":comments_url,"receipt":item.inspector.get("commentsReceipt")},
                "media":{"receipt":item.inspector.get("mediaSlotsReceipt")},
                "derivatives":{"receipt":item.inspector.get("derivativesReceipt")},
                "provenance":{"receipt":item.inspector.pointer("/provenance/receipt")}
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
        "cursor": params.cursor
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
                        Value::String(format!("/api/local/work-resources/{public_ref}"))
                    }),
                );
            }
        }
    }
    Ok(value)
}
