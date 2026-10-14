//! Read-fragment parity and failures use the formal handler, never a second renderer.
use super::*;

fn state(database: LocalDatabaseState) -> LocalWebState {
    LocalWebState {
        database,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: None,
    }
}

#[tokio::test]
async fn collection_drawer_fragment_preserves_unreadable_and_command_navigation() {
    let params = CollectionParams {
        drawer: Some("a1000000-0000-4000-8000-000000000001".into()),
        drawer_fragment: true,
        ..Default::default()
    };
    let fragment = collection_targets(
        State(state(LocalDatabaseState::NotConfigured)),
        Query(params),
    )
    .await
    .0;
    assert!(fragment.contains("id=\"c-drawer\""));
    assert!(!fragment.to_ascii_lowercase().contains("<!doctype"));
    assert!(!fragment.contains("data-target-row"));
    let params = CollectionParams {
        drawer: Some("a1000000-0000-4000-8000-000000000001".into()),
        rule: Some("a1000000-0000-4000-8000-000000000001".into()),
        drawer_fragment: true,
        ..Default::default()
    };
    let full = collection_targets(
        State(state(LocalDatabaseState::NotConfigured)),
        Query(params),
    )
    .await
    .0;
    assert!(full.to_ascii_lowercase().contains("<!doctype"));
    assert!(full.contains("id=\"c-monitor-rule\""));
}

#[tokio::test]
#[ignore = "requires disposable LOCAL-001 PostgreSQL"]
async fn collection_drawer_fragment_matches_full_target_and_missing_reads() {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
    let db = linggan_storage_postgres::testing::isolated_proof_schema(
        &url,
        "collection_read_fragment",
        full_schema_fixture::FULL_MIGRATIONS,
    )
    .await
    .unwrap();
    sqlx::raw_sql("INSERT INTO collection_observation_target (target_ref,platform,target_kind,identity_key,display_name,source,lifecycle_state) VALUES ('a1000000-0000-4000-8000-000000000001','xhs','creator','fragment-creator','合成读取目标','manual','pending_decision'); INSERT INTO observation_domain_target(domain_ref,target_ref,role) VALUES ('00000000-0000-4000-8000-000000000001','a1000000-0000-4000-8000-000000000001','primary');")
        .execute(db.pool()).await.unwrap();
    let state = state(LocalDatabaseState::Ready(Arc::new(db)));
    for drawer in [
        "a1000000-0000-4000-8000-000000000001",
        "a1000000-0000-4000-8000-000000000099",
    ] {
        for tab in ["overview", "works", "patrol"] {
            let params = CollectionParams {
                drawer: Some(drawer.into()),
                dtab: Some(tab.into()),
                domain: Some("all".into()),
                ..Default::default()
            };
            let full = collection_targets(State(state.clone()), Query(params))
                .await
                .0;
            let params = CollectionParams {
                drawer: Some(drawer.into()),
                dtab: Some(tab.into()),
                domain: Some("all".into()),
                drawer_fragment: true,
                ..Default::default()
            };
            let fragment = collection_targets(State(state.clone()), Query(params))
                .await
                .0;
            assert!(fragment.starts_with("<aside id=\"c-drawer\""));
            assert!(
                full.contains(&fragment),
                "{drawer}/{tab}: same renderer and facts"
            );
            assert!(!fragment.contains("data-target-row"));
            assert!(!fragment.contains("<script"));
        }
    }
}
