//! Same worker execution entry and existing media admission, lease and dossier readers.
//! Private holdout data is supplied only on the local machine and never printed.
#![allow(dead_code)]
#[path = "../../../../crates/evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "../../src/bin/media_worker_support/mod.rs"]
mod support;
use linggan_evidence::*;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use std::path::PathBuf;
use uuid::Uuid;

async fn database(schema: &str) -> Database {
    fixture::proof_database(schema).await
}
pub(crate) async fn admit(
    db: &Database,
    work: &str,
    ordinal: u32,
    bytes: &[u8],
    mime: &str,
) -> Uuid {
    admit_with_slot_role(db, work, ordinal, bytes, mime, "image").await
}
pub(crate) async fn admit_with_slot_role(
    db: &Database,
    work: &str,
    ordinal: u32,
    bytes: &[u8],
    mime: &str,
    slot_role: &str,
) -> Uuid {
    admit_image_source(db, work, ordinal, bytes, mime, slot_role).await
}

/// One complete platform list must contain all its pages in the same package.
pub(crate) async fn admit_known_pages(
    db: &Database,
    work: &str,
    count: u32,
    bytes: &[u8],
    mime: &str,
) -> Uuid {
    fixture::submit_package(db, "content_detail", json!({"contentExternalId":work}), json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":work},"payload":{"title":"Synthetic known page metadata","bodyText":"Native body remains independent."}})).await;
    let observations: Vec<_> = (1..=count)
        .map(|ordinal| (ordinal, Uuid::new_v4()))
        .collect();
    let records: Vec<_> = observations.iter().map(|(ordinal, observation)| {
        let uri = format!("https://media.example/{work}/{ordinal}.png");
        json!({"kind":"media_slot","slotKey":format!("xhs:{work}:image:{ordinal}"),"observationRef":observation,
            "slot":{"role":"image","ordinal":ordinal,"displayOrdinal":ordinal,"displayOrderBasis":"platform_explicit"},
            "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-21T00:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":work}})
    }).collect();
    fixture::submit_custom_package(db, "xhs", &["media_slots"], json!({"contentExternalId":work}), "media_slots", "xhs",
        json!({"target":{"contentExternalId":work,"imageInventory":{"state":"complete","source":"platform_image_list","expectedPages":count}},"layers":[fixture::coverage_layer("media_slots",i64::from(count))]}), records).await;
    let hash = support::sha256_hex(bytes);
    let key = format!("blobs/{hash}");
    std::fs::write(support::prepare_output(&key).unwrap(), bytes).unwrap();
    for (_, observation) in observations {
        admit_media_blob(db, observation, &hash, mime, bytes.len() as i64, &key)
            .await
            .unwrap();
    }
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap()
}

async fn admit_image_source(
    db: &Database,
    work: &str,
    ordinal: u32,
    bytes: &[u8],
    mime: &str,
    slot_role: &str,
) -> Uuid {
    let present: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM linggan_material_content WHERE content_external_id=$1)",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap();
    if !present {
        fixture::submit_package(db,"content_detail",json!({"contentExternalId":work}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":work},"payload":{"title":"Synthetic replay metadata","bodyText":"Native body must remain independent."}})).await;
    }
    let observation = Uuid::new_v4();
    let (slot, source_type, source_external_id, context) = match slot_role {
        "avatar" => (
            format!("xhs:author:synthetic-author:avatar:{ordinal}"),
            "author",
            "synthetic-author".to_owned(),
            Some(work),
        ),
        "comment_image" => (
            format!("xhs:comment:synthetic-comment-{ordinal}:comment_image:{ordinal}"),
            "comment",
            format!("synthetic-comment-{ordinal}"),
            Some(work),
        ),
        _ => (
            format!("xhs:{work}:{slot_role}:{ordinal}"),
            "content",
            work.to_owned(),
            None,
        ),
    };
    let uri = format!("https://media.example/{work}/{ordinal}.jpg");
    let mut record = json!({"kind":"media_slot","slotKey":slot,"observationRef":observation,"slot":{"role":slot_role,"ordinal":ordinal},"observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-10-21T00:00:00Z"},"sourceObject":{"platform":"xhs","type":source_type,"externalId":source_external_id}});
    if let Some(context) = context {
        record["contextContentExternalId"] = json!(context);
    }
    let target = json!({"contentExternalId":work,
        "imageInventory":{"state":"observed_set","source":"dom_observed_set","expectedPages":null}});
    fixture::submit_custom_package(
        db,
        "xhs",
        &["media_slots"],
        json!({"contentExternalId":work}),
        "media_slots",
        "xhs",
        json!({"target":target,"layers":[fixture::coverage_layer("media_slots",1)]}),
        vec![record],
    )
    .await;
    let hash = support::sha256_hex(bytes);
    let key = format!("blobs/{hash}");
    let path = support::prepare_output(&key).unwrap();
    std::fs::write(path, bytes).unwrap();
    admit_media_blob(db, observation, &hash, mime, bytes.len() as i64, &key)
        .await
        .unwrap();
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
    )
    .bind(work)
    .fetch_one(db.pool())
    .await
    .unwrap()
}
