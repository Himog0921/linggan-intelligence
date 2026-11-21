use super::*;
use crate::ops_content_lens::{AnalysisInput, CountSource, SourcedCount, metric_definition};
use linggan_storage_postgres::testing::isolated_proof_schema;
use serde_json::json;

#[path = "../../../../../apps/api/src/local_web/full_schema_fixture.rs"]
mod fixture;

fn synthetic_taxonomy() -> Value {
    fn entries(keys: &[&str]) -> Vec<Value> {
        keys.iter().map(|key| json!({"key":key,"name":"样例","definition":"合成定义","include":"纳入条件","confusable_with":"区分说明","example_ids":[]})).collect()
    }
    json!({"_meta":{"general_rules":["合成规则"]}, "moment":entries(MOMENT),
        "situation":entries(SITUATION), "offer":entries(OFFER), "audience":entries(AUDIENCE)})
}

#[test]
fn matches_v1_vocabulary_and_rejects_ambiguous_definitions() {
    let mut body = synthetic_taxonomy();
    assert_eq!(
        [MOMENT.len(), OFFER.len(), SITUATION.len(), AUDIENCE.len()],
        [17, 14, 17, 5]
    );
    let taxonomy: Taxonomy = serde_json::from_value(body.clone()).unwrap();
    taxonomy.validate(true).unwrap();
    body["moment"][0]["key"] = json!("other");
    let duplicate: Taxonomy = serde_json::from_value(body).unwrap();
    assert!(duplicate.validate(true).is_err());
}

#[tokio::test]
#[ignore = "requires the disposable ops ledger proof database"]
async fn definitions_and_shared_publication_example() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = isolated_proof_schema(&url, "ops_content_lens_l1a", fixture::FULL_MIGRATIONS)
        .await
        .unwrap();
    let bytes = match std::env::var_os("OPS_LENS_PROOF_TAXONOMY_FILE") {
        Some(file) => std::fs::read(file).unwrap(),
        None => serde_json::to_vec(&synthetic_taxonomy()).unwrap(),
    };
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    let (first, repeat) = tokio::join!(load_taxonomy(&db, &bytes), load_taxonomy(&db, &bytes));
    let (first, repeat) = (first.unwrap(), repeat.unwrap());
    assert_eq!(first.version, 1);
    assert_eq!(first.definition_ref, repeat.definition_ref);
    let mut changed = body.clone();
    changed["_meta"]["general_rules"] = json!(["修订定义"]);
    assert_eq!(
        load_taxonomy(&db, &serde_json::to_vec(&changed).unwrap())
            .await
            .unwrap()
            .version,
        2
    );
    assert_eq!(load_taxonomy(&db, &bytes).await.unwrap().version, 1);
    let current: Uuid = sqlx::query_scalar(
        "SELECT definition_ref FROM linggan_ops_lens_definition_current WHERE kind='taxonomy'",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(current, first.definition_ref);
    let (_, metric) = metric_definition(&db, 1).await.unwrap();
    assert_eq!(
        (
            metric.min_n_rate,
            metric.permutation_seed,
            metric.audit_sample_seed
        ),
        (20, 42, 43)
    );
    let (key1, key2): (String, String) = sqlx::query_as(
        "SELECT linggan_ops_lens_title_key('样例 A!'),linggan_ops_lens_title_key('样例A')",
    )
    .fetch_one(db.pool())
    .await
    .unwrap();
    assert_eq!(key1, key2);

    let mut input: AnalysisInput = serde_json::from_value(json!({
        "domainRef":null,"from":null,"to":null,
        "method":{"taxonomyRef":current,"taskConfigRef":Uuid::from_u128(1),"promptVersionRef":Uuid::from_u128(2)},
        "totalPublications":2,"withTitle":2,"pendingTitles":1,"publications":{}
    })).unwrap();
    for publication in [1, 2] {
        input.publications.insert(Uuid::from_u128(publication), serde_json::from_value(json!({
            "titleKey":key1,"accountRef":null,"businessDate":"2026-11-01","labels":{},
            "views":{"source":null,"value":null},"comments":{"source":null,"value":null},"creationLinks":{}
        })).unwrap());
    }
    // Values and references as selected by the future L1d ledger reader.
    for (receipt, views) in [(10, 900), (11, 1200)] {
        input
            .publications
            .get_mut(&Uuid::from_u128(1))
            .unwrap()
            .views = SourcedCount {
            source: Some(CountSource::ViewReport(Uuid::from_u128(receipt))),
            value: Some(views),
        };
    }
    let identity = input.input_hash().unwrap();
    let serialized = serde_json::to_value(&input).unwrap();
    let decoded: AnalysisInput = serde_json::from_value(serialized).unwrap();
    let available: Vec<_> = decoded
        .publications
        .values()
        .filter_map(|p| p.views.value)
        .collect();
    assert_eq!(decoded.publications.len(), 2);
    assert_eq!(available, [1200]);
    assert_eq!(
        available
            .iter()
            .filter(|v| **v >= metric.views_threshold_mid)
            .count(),
        1
    );
    assert!(available.len() < metric.min_n_rate as usize);
    assert_eq!(decoded.input_hash().unwrap(), identity);
    input.pending_titles = 0;
    assert_ne!(input.input_hash().unwrap(), identity);

    sqlx::raw_sql(include_str!("../schema_test.sql"))
        .execute(db.pool())
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE linggan_ops_lens_label SET moment='other'")
            .execute(db.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query(
            "UPDATE linggan_ops_lens_definition_current SET definition_ref=$1 WHERE kind='metric'"
        )
        .bind(current)
        .execute(db.pool())
        .await
        .is_err()
    );
}

#[path = "../snapshot_test.rs"]
mod snapshot_test;
