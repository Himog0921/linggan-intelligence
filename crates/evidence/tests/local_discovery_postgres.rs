use linggan_contracts::{
    DiscoveryContractError, EvidenceQuery, is_rfc3339_timestamp, parse_discovery_package,
};
use linggan_evidence::{
    DiscoveryIngressError, DiscoveryIngressOutcome, ingest_discovery_package,
    read_discovery_library,
};
use linggan_storage_postgres::{Database, testing::isolated_proof_schema};
use sqlx::Row;

const MIGRATIONS: &str = concat!(
    include_str!("../../../database/migrations/0001_scope_001_capture_evidence.sql"),
    "\n",
    include_str!("../../../database/migrations/0002_local_001_discovery.sql"),
);

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn admission_preserves_partial_cards_replays_exact_input_and_rejects_invalid_input() {
    let database = proof_database("local_discovery_admission").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let package = package(&observed_at, Some(&observed_at));

    let accepted = ingest_discovery_package(&database, &package)
        .await
        .expect("a valid partial discovery package is admitted");
    assert!(matches!(
        accepted,
        DiscoveryIngressOutcome::Accepted {
            visible_cards: 2,
            ..
        }
    ));
    assert_counts(&database, &[1, 1, 2, 2, 1]).await;

    let replay = ingest_discovery_package(&database, &package)
        .await
        .expect("byte-identical package is a replay, not an overwrite");
    assert!(matches!(
        replay,
        DiscoveryIngressOutcome::Replay {
            visible_cards: 2,
            ..
        }
    ));
    assert_counts(&database, &[1, 2, 2, 2, 1]).await;

    let invalid = package.replace(
        "\"title\":\"标题命中 ADHD\"",
        "\"title\":\"标题命中 ADHD\",\"comments\":[]",
    );
    assert!(matches!(
        ingest_discovery_package(&database, &invalid).await,
        Err(DiscoveryIngressError::Contract(_))
    ));
    assert_counts(&database, &[1, 2, 2, 2, 1]).await;

    let mutation = sqlx::query("UPDATE local_discovery_package SET query_text = 'other'")
        .execute(database.pool())
        .await;
    assert!(
        mutation.is_err(),
        "accepted discovery facts must be append-only"
    );
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn read_projection_uses_known_published_time_and_counts_unknowns_only_within_the_query_candidate_set()
 {
    let database = proof_database("local_discovery_read").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    ingest_discovery_package(&database, &package(&observed_at, Some(&observed_at)))
        .await
        .expect("valid package is admitted");
    let query: EvidenceQuery = serde_json::from_str(
        r#"{"text":"ADHD","scope":"all_accepted_material","window":"last_30_days","sort":"latest_discovery"}"#,
    )
    .expect("fixed local retrieval query is valid");

    let projection = read_discovery_library(&database, &query)
        .await
        .expect("accepted discovery data is locally readable");
    assert_eq!(projection.cards.len(), 1);
    assert_eq!(projection.excluded_unknown_published_at, 0);
    assert_eq!(
        projection.cards[0].cover_presentation_state,
        "MEDIA_NOT_ACQUIRED"
    );
    let serialized = serde_json::to_string(&projection).expect("projection serializes");
    assert!(!serialized.contains("xhscdn"));
    assert!(!serialized.contains("https://"));
    assert!(serialized.contains("MEDIA_NOT_ACQUIRED"));
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn read_projection_counts_matching_unknown_published_time_within_the_query_candidate_set() {
    let database = proof_database("local_discovery_query_scoped_unknown").await;
    let observed_at = producer_fixture_observed_at(&database).await;
    let package = package(&observed_at, Some(&observed_at)).replace(
        "\"title\":\"无发布时间卡片\"",
        "\"title\":\"ADHD 未知发布时间卡片\"",
    );
    ingest_discovery_package(&database, &package)
        .await
        .expect("valid package is admitted");
    let query: EvidenceQuery = serde_json::from_str(
        r#"{"text":"ADHD","scope":"all_accepted_material","window":"last_30_days","sort":"latest_discovery"}"#,
    )
    .expect("fixed local retrieval query is valid");

    let projection = read_discovery_library(&database, &query)
        .await
        .expect("accepted discovery data is locally readable");
    assert_eq!(projection.cards.len(), 1);
    assert_eq!(projection.excluded_unknown_published_at, 1);
    assert_eq!(projection.cards[0].platform_content_id, "note-a");
}

#[tokio::test]
#[ignore = "requires ./scripts/test-local-001-discovery-postgres.sh and an isolated PostgreSQL proof database"]
async fn postgres_display_text_is_rejected_and_the_fixture_uses_explicit_rfc3339() {
    let database = proof_database("local_discovery_time_format").await;
    let (postgres_display_text, rfc3339_fixture_time) = database_clock_strings(&database).await;

    assert!(
        !is_rfc3339_timestamp(&postgres_display_text),
        "PostgreSQL display text must never be passed through as a producer timestamp"
    );
    assert!(
        is_rfc3339_timestamp(&rfc3339_fixture_time),
        "the test fixture must explicitly format a producer timestamp with an RFC 3339 offset"
    );
    assert!(matches!(
        parse_discovery_package(&package(
            &postgres_display_text,
            Some(&rfc3339_fixture_time)
        )),
        Err(DiscoveryContractError::InvalidObservedAt)
    ));
    assert!(
        parse_discovery_package(&package(&rfc3339_fixture_time, Some(&rfc3339_fixture_time)))
            .is_ok(),
        "the fixture's explicitly formatted timestamp remains contract-valid"
    );
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL")
        .expect("test script must provide the isolated proof database URL");
    isolated_proof_schema(&url, schema, MIGRATIONS)
        .await
        .expect("isolated migration applies")
}

async fn producer_fixture_observed_at(database: &Database) -> String {
    database_clock_strings(database).await.1
}

async fn database_clock_strings(database: &Database) -> (String, String) {
    let row = sqlx::query(
        "SELECT scope_001_now()::text AS postgres_display_text, \
                to_char(scope_001_now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS rfc3339_fixture_time",
    )
        .fetch_one(database.pool())
        .await
        .expect("database returns both a display value and an explicitly formatted fixture value");
    (
        row.get("postgres_display_text"),
        row.get("rfc3339_fixture_time"),
    )
}

async fn assert_counts(database: &Database, expected: &[i64; 5]) {
    let statements = [
        "SELECT count(*) AS count FROM local_discovery_package",
        "SELECT count(*) AS count FROM local_discovery_ingress_delivery",
        "SELECT count(*) AS count FROM local_discovery_content_item",
        "SELECT count(*) AS count FROM local_discovery_occurrence",
        "SELECT count(*) AS count FROM local_discovery_coverage",
    ];
    for (index, statement) in statements.into_iter().enumerate() {
        let count: i64 = sqlx::query(statement)
            .fetch_one(database.pool())
            .await
            .expect("count query succeeds")
            .get("count");
        assert_eq!(
            count, expected[index],
            "unexpected table count at index {index}"
        );
    }
}

fn package(observed_at: &str, known_published_at: Option<&str>) -> String {
    let first_published = known_published_at
        .map(|value| format!("\"publishedAtSourceText\":\"{value}\","))
        .unwrap_or_default();
    format!(
        r#"{{
          "contractVersion":"xhs.discovery.visible-card.v1",
          "acquisitionSpec":{{"platform":"xhs","query":"ADHD","sort":"comprehensive","target":{{"basis":"maximum_quota","unit":"visible_search_card","maximumQuota":20}}}},
          "observedAt":"{observed_at}",
          "coverage":{{"unit":"visible_search_card","visibleCards":2,"stoppedReason":"risk_control"}},
          "cards":[
            {{"content":{{"platformContentId":"note-a","title":"标题命中 ADHD","creatorDisplayName":"A娃家长",{first_published}"coverCandidate":{{"observedExternalUri":"https://xhscdn.example/cover-a"}}}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":1}}}},
            {{"content":{{"platformContentId":"note-b","title":"无发布时间卡片","creatorDisplayName":"另一位家长"}},"occurrence":{{"query":"ADHD","sort":"comprehensive","observedAt":"{observed_at}","resultPosition":2}}}}
          ]
        }}"#,
    )
}
