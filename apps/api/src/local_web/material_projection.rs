//! Loopback-only Evidence Library JSON composition.
//!
//! The existing `cards` response remains available during the UI transition. `items` is the
//! authoritative multi-material surface; this adapter never triggers platform acquisition.

use linggan_contracts::EvidenceQuery;
use linggan_evidence::{
    DiscoveryLibraryProjection, material_projection_schema_is_ready, read_material_library,
};
use linggan_storage_postgres::Database;
use serde_json::Value;

pub(super) async fn compose_json(
    database: &Database,
    query: &EvidenceQuery,
    discovery: DiscoveryLibraryProjection,
) -> Result<Value, sqlx::Error> {
    let mut value = serde_json::to_value(discovery).expect("the discovery projection serializes");
    let object = value
        .as_object_mut()
        .expect("the discovery projection is an object");
    if !material_projection_schema_is_ready(database).await? {
        object.insert("items".to_owned(), Value::Array(Vec::new()));
        object.insert(
            "queryScope".to_owned(),
            Value::String("accepted_discovery_legacy_only".to_owned()),
        );
        object.insert("asOf".to_owned(), Value::Null);
        object.insert("cursor".to_owned(), Value::Null);
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
    Ok(value)
}
