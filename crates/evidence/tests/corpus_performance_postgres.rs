//! Public-seam regression proof; timings are descriptive, statement counts are the guard.
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{coverage_layer, proof_database, submit_custom_package, submit_package};
use linggan_contracts::EvidenceQuery;
use linggan_evidence::{read_work_resource, read_work_resources};
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;

const DOMAIN: &str = "00000000-0000-4000-8000-000000000001";

fn query(extra: Value) -> EvidenceQuery {
    let mut value = json!({"scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"});
    value
        .as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(value).expect("bounded query validates")
}

async fn seed(database: &Database, count: usize) -> Vec<Uuid> {
    let records = (0..count).map(|i| json!({
        "kind":"discovery_card",
        "sourceObject":{"platform":"xhs","type":"content","externalId":format!("perf-{i:03}")},
        "payload":{"title":format!("synthetic work {i}"),"authorId":"synthetic-author","likes":i}
    })).collect();
    let target = json!({"query":"synthetic performance"});
    let package = submit_custom_package(
        database,
        "xhs",
        &["discovery_search"],
        target.clone(),
        "discovery_search",
        "xhs",
        json!({"target":target,"layers":[coverage_layer("discovery_search",count as i64)]}),
        records,
    )
    .await;
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref) SELECT gen_random_uuid(),public_ref,$1,'primary','legacy_domain_migration',$2 FROM linggan_material_content")
        .bind(DOMAIN.parse::<Uuid>().unwrap()).bind(package).execute(database.pool()).await.unwrap();
    sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content ORDER BY content_external_id",
    )
    .fetch_all(database.pool())
    .await
    .unwrap()
}

async fn statement_count(database: &Database, refs: &[Uuid]) -> i64 {
    sqlx::query("SELECT public.pg_stat_statements_reset()")
        .execute(database.pool())
        .await
        .unwrap();
    let page = read_work_resources(
        database,
        &query(json!({"domainRef":DOMAIN,"publicRefs":refs})),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), refs.len());
    let counts: Vec<(String,i64)> = sqlx::query_as("SELECT query,calls::bigint FROM public.pg_stat_statements WHERE dbid=(SELECT oid FROM pg_database WHERE datname=current_database()) AND query NOT LIKE '%pg_stat_statements%' AND query NOT LIKE 'SET %' AND query NOT IN ('BEGIN','COMMIT','ROLLBACK','SELECT $1','SELECT 1')")
        .fetch_all(database.pool()).await.unwrap();
    counts.iter().map(|(_, calls)| calls).sum()
}

#[tokio::test]
#[ignore = "requires scripts/test-corpus-performance-postgres.sh and isolated pg_stat_statements"]
async fn enrichment_statement_count_does_not_grow_per_work() {
    let database = proof_database("corpus_perf_query_count").await;
    let refs = seed(&database, 50).await;
    // Count application statements and schema probes; exclude pool pings, transaction
    // control (including SET TRANSACTION), and measurement queries. Utility tracking
    // differs for a cached SET plan; it must not add noise to the read-query guard.
    // Warm the public seam and connection setup before resetting database stats.
    read_work_resources(
        &database,
        &query(json!({"domainRef":DOMAIN,"publicRefs":refs})),
    )
    .await
    .unwrap();
    let one = statement_count(&database, &refs[..1]).await;
    let fifty = statement_count(&database, &refs).await;
    assert_eq!(
        fifty, one,
        "enrichment must use fixed batch queries, not N+1: one={one}, fifty={fifty}"
    );
    assert!(fifty <= 40, "fixed statement budget exceeded: {fifty}");
    println!("CORPUS-PERFORMANCE-089 statements: one={one}, fifty={fifty}");
    for item in read_work_resources(
        &database,
        &query(json!({"domainRef":DOMAIN,"publicRefs":refs})),
    )
    .await
    .unwrap()
    .items
    {
        let detail = read_work_resource(&database, item.identity.public_ref)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(&item.display).unwrap(),
            serde_json::to_value(&detail.display).unwrap()
        );
        assert_eq!(item.media, detail.media);
        assert_eq!(
            serde_json::to_value(&item.lane_summaries).unwrap(),
            serde_json::to_value(&detail.lane_summaries).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&item.evidence_fragment).unwrap(),
            serde_json::to_value(&detail.evidence_fragment).unwrap()
        );
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL performance harness"]
async fn keyset_snapshot_search_and_scan_budget_remain_bounded() {
    let database = proof_database("corpus_perf_keyset").await;
    let refs = seed(&database, 260).await;
    let first = read_work_resources(&database, &query(json!({})))
        .await
        .unwrap();
    assert_eq!(first.items.len(), 50);
    assert_eq!(first.scanned_count, 51);
    assert!(!first.scan_limited);
    let as_of = first.as_of.clone();
    let mut seen: HashSet<_> = first.items.iter().map(|i| i.identity.public_ref).collect();
    // New material after the first-page snapshot must not leak into its continuation.
    submit_package(&database,"content_detail",json!({"contentExternalId":"perf-000-new"}),json!({
        "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"perf-000-new"},"payload":{"title":"newer snapshot"}
    })).await;
    let mut cursor = first.cursor;
    while let Some(next) = cursor {
        let page = read_work_resources(&database, &query(json!({"cursor":next})))
            .await
            .unwrap();
        assert_eq!(page.as_of, as_of);
        assert!(page.items.len() <= 50);
        for item in &page.items {
            assert!(
                seen.insert(item.identity.public_ref),
                "cursor must not duplicate a work"
            );
        }
        cursor = page.cursor;
    }
    assert_eq!(seen, refs.iter().copied().collect());
    let searched = read_work_resources(&database, &query(json!({"text":"synthetic work 259"})))
        .await
        .unwrap();
    assert_eq!(
        searched.items.len(),
        1,
        "SQL text filtering must occur before the identity limit"
    );
    assert_eq!(searched.items[0].identity.content_external_id, "perf-259");
    let restricted_query = query(json!({"restriction":"WITHDRAWN_OR_RESTRICTED"}));
    let limited = read_work_resources(&database, &restricted_query)
        .await
        .unwrap();
    assert!(limited.items.is_empty());
    assert_eq!(limited.scanned_count, 200);
    assert!(limited.scan_limited && limited.truncated && limited.cursor.is_some());
    let tail = read_work_resources(
        &database,
        &query(json!({"restriction":"WITHDRAWN_OR_RESTRICTED","cursor":limited.cursor})),
    )
    .await
    .unwrap();
    assert!(tail.items.is_empty() && !tail.scan_limited && tail.cursor.is_none());
    assert_eq!(tail.scanned_count, 61);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL performance harness"]
async fn media_slot_limits_and_receipts_are_per_work_in_a_batch() {
    let database = proof_database("corpus_perf_media_limit").await;
    seed(&database, 2).await;
    for content_id in ["perf-000", "perf-001"] {
        let target = json!({"contentExternalId":content_id});
        let records = (1..=70).map(|ordinal| json!({
            "kind":"media_slot","slotKey":format!("xhs:{content_id}:image:{ordinal}"),
            "observationRef":Uuid::new_v4(),"slot":{"role":"image","ordinal":ordinal},
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id},
            "observation":{"externalUri":format!("https://media.example/{content_id}/{ordinal}.jpg"),"candidateUris":[format!("https://media.example/{content_id}/{ordinal}.jpg")]}
        })).collect();
        submit_custom_package(
            &database,
            "xhs",
            &["media_slots"],
            target.clone(),
            "media_slots",
            "xhs",
            json!({"target":target,"layers":[coverage_layer("media_slots",70)]}),
            records,
        )
        .await;
    }
    let page = read_work_resources(&database, &query(json!({"domainRef":DOMAIN})))
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
    for item in page.items {
        assert_eq!(item.media["images"].as_array().unwrap().len(), 64);
        let receipt = item.inspector.get("mediaSlotsReceipt").unwrap();
        assert_eq!(receipt["total"], 70);
        assert_eq!(receipt["returned"], 64);
        assert_eq!(receipt["truncated"], true);
        assert!(receipt["nextCursor"].as_str().is_some());
        for slot in item.media["images"].as_array().unwrap() {
            assert!(
                slot["slotKey"]
                    .as_str()
                    .unwrap()
                    .contains(&item.identity.content_external_id)
            );
        }
        let detail = read_work_resource(&database, item.identity.public_ref)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(item.media, detail.media);
        assert_eq!(receipt, detail.inspector.get("mediaSlotsReceipt").unwrap());
    }
}
