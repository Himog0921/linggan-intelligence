//! Loopback-only Evidence Library JSON composition.
//!
//! The existing `cards` response remains available during the UI transition. `items` is the
//! authoritative multi-material surface; this adapter never triggers platform acquisition.

use linggan_contracts::EvidenceQuery;
use linggan_evidence::{
    DiscoveryLibraryProjection, MaterialReadError, material_projection_schema_is_ready,
    read_material_library,
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
    discovery: DiscoveryLibraryProjection,
) -> Result<Value, MaterialReadError> {
    let mut value = serde_json::to_value(discovery).expect("the discovery projection serializes");
    let object = value
        .as_object_mut()
        .expect("the discovery projection is an object");
    if query.lane().is_some()
        || query.lane_state().is_some()
        || query.media_kind().is_some()
        || query.restriction().is_some()
    {
        object.insert("cards".to_owned(), Value::Array(Vec::new()));
    }
    if !material_projection_schema_is_ready(database).await? {
        object.insert("items".to_owned(), Value::Array(Vec::new()));
        object.insert(
            "queryScope".to_owned(),
            Value::String("accepted_discovery_legacy_only".to_owned()),
        );
        object.insert("asOf".to_owned(), Value::Null);
        object.insert("cursor".to_owned(), Value::Null);
        object.insert("truncated".to_owned(), Value::Bool(false));
        return Ok(value);
    }
    let material = read_material_library(database, query).await?;
    object.insert(
        "items".to_owned(),
        serde_json::to_value(material.items).expect("material items serialize"),
    );
    object.insert(
        "queryScope".to_owned(),
        Value::String(material.query_scope.to_owned()),
    );
    object.insert("asOf".to_owned(), Value::String(material.as_of));
    object.insert(
        "cursor".to_owned(),
        material.cursor.map_or(Value::Null, Value::String),
    );
    object.insert("truncated".to_owned(), Value::Bool(material.truncated));
    Ok(value)
}
