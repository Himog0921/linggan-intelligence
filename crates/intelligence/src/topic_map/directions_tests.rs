use super::*;
use crate::topic_map::{TopicMapQuery, read_topic_map};
#[allow(dead_code)]
#[path = "../topic_map_research/tests/saved_source_fixture.rs"]
mod support;

#[tokio::test]
#[ignore = "isolated PostgreSQL"]
async fn published_directions_survive_drafts_and_retries_and_keep_distinct_work_denominators() {
    let (db, _) = support::setup(&format!("directions_{}", Uuid::new_v4().simple())).await;
    let work = support::work(&db, "synthetic-direction-work", "合成方向材料").await;
    let query = TopicMapQuery {
        domain_ref: Some(support::D),
        ..Default::default()
    };
    let mut snap = read_topic_map(&db, &query).await.unwrap();
    let source = snap.works.iter_mut().find(|w| w.work_ref == work).unwrap();
    source.own = false;
    source.author_external_id = Some("parent-benchmark".into());
    source.likes = Some(200);
    let root = Uuid::new_v4();
    let leaf = Uuid::new_v4();
    for (id, parent, direct) in [(root, None, vec![]), (leaf, Some(root), vec![work])] {
        snap.topics.push(TopicMapTopic {
            topic_ref: id,
            canonical_key: id.to_string(),
            domain_ref: Some(support::D),
            definition_ref: Uuid::new_v4(),
            definition_version: 1,
            display_name: "合成方向".into(),
            definition_text: "合成来源定义".into(),
            lifecycle_state: "provisional".into(),
            parent_topic_ref: parent,
            binding_version: Some(1),
            direct_work_refs: direct,
            work_refs: vec![work],
            statistics: Value::Null,
            journey: Value::Null,
            core: Value::Null,
        });
    }
    let mut draft = propose(&snap.topics.iter().collect::<Vec<_>>());
    assert_eq!(draft.directions.len(), 1);
    draft.benchmark_authors.insert("parent-benchmark".into());
    let command = |id: Uuid, rev: i64, publish: bool, catalog: Catalog| Save {
        command_ref: id,
        expected_revision: rev,
        preview_hash: None,
        domain_ref: support::D,
        publish,
        catalog,
    };
    let original = Uuid::new_v4();
    let saved = save(&db, command(original, 0, false, draft.clone()), &snap)
        .await
        .unwrap();
    assert_eq!(
        save(&db, command(original, 0, false, draft.clone()), &snap)
            .await
            .unwrap(),
        saved
    );
    assert!(read(&db, support::D, &snap).await.unwrap()["published"].is_null());
    let impact = preview(&db, command(Uuid::new_v4(), 1, true, draft.clone()), &snap)
        .await
        .unwrap();
    assert_eq!(impact["changedWorks"], 1);
    save(
        &db,
        Save {
            preview_hash: impact["previewHash"].as_str().map(str::to_owned),
            ..command(Uuid::new_v4(), 1, true, draft.clone())
        },
        &snap,
    )
    .await
    .unwrap();
    let stale = save(&db, command(Uuid::new_v4(), 1, true, draft.clone()), &snap).await;
    assert!(matches!(stale, Err(TopicMapError::Conflict)));
    let first = read(&db, support::D, &snap).await.unwrap();
    assert_eq!(first["published"]["directions"][0]["externalHits"], 1);
    assert_eq!(first["published"]["directions"][0]["ownKnown"], 0);
    let mut updated = draft.clone();
    updated.directions.push(Direction {
        direction_ref: leaf,
        label: "更具体的方向".into(),
        definition: "独立子主题".into(),
        topic_refs: vec![leaf],
        text_source: None,
        text_sources: Vec::new(),
        evidence_work_refs: Vec::new(),
        candidate_source: None,
    });
    save(
        &db,
        command(Uuid::new_v4(), 2, false, updated.clone()),
        &snap,
    )
    .await
    .unwrap();
    let next = read(&db, support::D, &snap).await.unwrap();
    assert_eq!(next["publishedRevision"], 2);
    assert_eq!(next["published"], first["published"]);
    let impact = preview(&db, command(Uuid::new_v4(), 3, true, updated), &snap)
        .await
        .unwrap();
    assert_eq!(
        (
            impact["before"].as_u64(),
            impact["after"].as_u64(),
            impact["changedWorks"].as_u64()
        ),
        (Some(1), Some(1), Some(1))
    );
    snap.topics
        .iter_mut()
        .find(|t| t.topic_ref == root)
        .unwrap()
        .core = json!({"sourceState":"source_unavailable"});
    let redacted = read(&db, support::D, &snap).await.unwrap();
    assert_eq!(
        redacted["published"]["directions"][0]["label"],
        "来源已变化的方向"
    );
    assert!(
        preview(&db, command(Uuid::new_v4(), 3, true, draft), &snap)
            .await
            .is_err()
    );
}
