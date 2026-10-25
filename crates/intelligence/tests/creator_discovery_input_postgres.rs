//! Canonical creator preparation reaches a synthetic local Pi child in full.
//! This is a data-consumer proof; the retired production scheduler remains closed.
#[path = "../../evidence/tests/support/research_dossier_fixture.rs"]
mod dossier;
use linggan_evidence::creator_discovery::load_topic_materials;
use linggan_intelligence::{
    creator_discovery_input::{citation_map, prepare},
    creator_discovery_worker::run_once,
    model_secrets::SyntheticModelSecrets,
    model_settings::ModelError,
    pi_adapter::{PI_PROTOCOL, PiAdapter, PiRequest},
};
use serde_json::{Value, json};

#[tokio::test]
#[ignore = "isolated PostgreSQL and fixed synthetic child only; no provider or production dispatch"]
async fn canonical_creator_input_keeps_full_content_and_exact_provenance_without_reviving_retired_worker()
 {
    let db = dossier::fixture::proof_database("creator_dossier_input").await;
    let work = dossier::work(&db).await;
    dossier::ordinary_ocr(&db).await;
    let body = format!(
        "CREATOR_EXPECT_2000:{}CREATOR_FULL_TAIL🙂",
        "家庭实践过程。".repeat(2000)
    );
    let detail = |likes| json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"作者完整材料","bodyText":body,"authorId":"synthetic-author","likes":likes}});
    dossier::fixture::submit_package_at(
        &db,
        "content_detail",
        json!({"contentExternalId":"dossier-note"}),
        detail(10),
        "2026-10-20T00:00:00Z",
    )
    .await;
    let data = load_topic_materials(&db, dossier::DOMAIN).await.unwrap();
    let source = data.works.iter().find(|w| w.work_ref == work).unwrap();
    let (input, fragments) = prepare(&data.domain, source, &data.as_of);
    assert_eq!(input["materialContract"], "research-dossier.v2");
    assert_eq!(input["asOf"], data.as_of);
    assert!(input.get("fragments").is_none());
    let materials = input["materials"].as_array().unwrap();
    assert_eq!(materials.len(), fragments.len());
    let body_input = materials.iter().find(|m| m["field"] == "body").unwrap();
    assert_eq!(body_input["content"]["text"], body);
    assert!(body.chars().count() > 12000);
    assert!(body_input["readingRef"].is_string() && body_input["unitRef"].is_string());
    assert!(
        materials
            .iter()
            .any(|m| m["field"] == "ocr" && m["speaker"] == "unconfirmed")
    );
    for material in materials {
        assert_eq!(material["content"]["ref"], material["blockRef"]);
        assert!(material["sourceRef"].is_string() && material["sourceVersion"].is_string());
        assert!(material["issues"].is_array());
        assert!(
            material.get("text").is_none(),
            "canonical content appears once"
        );
    }
    let map = citation_map(&fragments, &data.as_of);
    for (position, local) in map.as_array().unwrap().iter().enumerate() {
        assert_eq!(local["citation"], materials[position]["citation"]);
        assert_eq!(local["sourceRef"], materials[position]["sourceRef"]);
        assert_eq!(local["blockRef"], materials[position]["blockRef"]);
        assert_eq!(local["asOf"], data.as_of);
        assert!(local.get("text").is_none());
    }
    let node: std::path::PathBuf = std::env::var_os("CREATOR_PROOF_NODE")
        .expect("owned proof Node required")
        .into();
    assert!(
        node.is_file(),
        "CREATOR_PROOF_NODE must name the owned Node executable"
    );
    let adapter = PiAdapter::configured_with_test_command(
        node,
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/creator_discovery_adapter.mjs"),
    );
    assert!(matches!(
        run_once(&db, &SyntheticModelSecrets, &adapter).await,
        Err(ModelError::LegacyRetired)
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_creator_discovery_work_analysis"
        )
        .fetch_one(db.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0
    );

    let response = adapter
        .call(&PiRequest {
            version: PI_PROTOCOL,
            operation: "analyze".into(),
            api: "openai-completions".into(),
            base_url: "http://127.0.0.1:1".into(),
            local_endpoint: true,
            api_key: "synthetic-no-secret".into(),
            model_id: "synthetic-creator-proof".into(),
            timeout_ms: 30000,
            max_output_tokens: 1000,
            system: linggan_intelligence::creator_discovery_analysis::SYSTEM.into(),
            prompt: input.to_string(),
            images: vec![],
            execution: None,
        })
        .await
        .unwrap();
    assert!(response.ok);
    let output: Value = serde_json::from_str(response.text.as_deref().unwrap()).unwrap();
    assert_eq!(
        output["relevance"]["evidenceFragmentIds"][0],
        body_input["citation"]
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_model_invocation")
            .fetch_one(db.pool())
            .await
            .unwrap(),
        0,
        "the direct synthetic child is a preparation proof, not a production invocation"
    );

    let old = fragments.iter().find(|f| f.field == "body").unwrap();
    dossier::fixture::submit_package_at(
        &db,
        "content_detail",
        json!({"contentExternalId":"dossier-note"}),
        detail(999),
        "2026-10-21T00:00:00Z",
    )
    .await;
    let refreshed = load_topic_materials(&db, dossier::DOMAIN).await.unwrap();
    let current = refreshed.works.iter().find(|w| w.work_ref == work).unwrap();
    assert_ne!(
        current.fingerprint, source.fingerprint,
        "new exact source revisions cannot reuse old citation ownership"
    );
    let latest = current
        .fragments
        .iter()
        .find(|f| f.field == "body")
        .unwrap();
    assert_eq!(latest.text, old.text);
    assert_ne!(latest.source_ref, old.source_ref);
    assert_ne!(latest.fragment_id, old.fragment_id);
    assert_eq!(body_input["sourceRef"], json!(old.source_ref));
    assert_eq!(body_input["content"]["text"], body);
}

#[path = "../../evidence/tests/support/standard_dossier_fixture.rs"]
mod standard;

#[tokio::test]
#[ignore = "isolated PostgreSQL and no provider"]
async fn standard_dossier_partial_and_video_share_creator_workbench_and_offline_blocks() {
    use linggan_evidence::{dossier_input, research_dossier};
    use linggan_intelligence::{
        content_workbench_sources::materials, note_interpretation_offline as offline,
    };
    use uuid::Uuid;
    let db = dossier::fixture::proof_database("standard_consumers").await;
    for (name, images, video, partial) in [
        ("consumer-partial", 5, false, true),
        ("consumer-video", 1, true, false),
    ] {
        let work = standard::scenario(&db, name, None, images, video, partial, false).await;
        let d = research_dossier::read(&db, work).await.unwrap().unwrap();
        let input = dossier_input::read(&db, standard::DOMAIN, work)
            .await
            .unwrap();
        let preview = offline::prepare_input(&offline::Input {
            material: input,
            direction_version_ref: Uuid::new_v4(),
            skill_version_ref: Uuid::new_v4(),
            direction_refs: vec![Uuid::new_v4()],
        })
        .unwrap();
        assert_eq!(preview.reading_title, research_dossier::reading_title(&d));
        assert_eq!(
            preview
                .fragments
                .iter()
                .map(|f| f.logical_position.as_str())
                .collect::<Vec<_>>(),
            research_dossier::ordered_refs(&d)
        );
        let data = linggan_evidence::creator_discovery::load_topic_materials_for_works(
            &db,
            standard::DOMAIN,
            &[work],
        )
        .await
        .unwrap();
        assert_eq!(
            data.works[0].title.as_deref(),
            d.note["displayTitle"].as_str()
        );
        let (provider, fragments) = prepare(&data.domain, &data.works[0], &data.as_of);
        assert_eq!(
            fragments
                .iter()
                .map(|f| f.fragment_id.as_str())
                .collect::<Vec<_>>(),
            research_dossier::ordered_refs(&d)
        );
        assert_eq!(
            provider["readingTitle"],
            research_dossier::reading_title(&d)
        );
        assert_eq!(
            provider["materials"].as_array().unwrap().len(),
            fragments.len()
        );
        let q=serde_json::from_value(json!({"domainRef":standard::DOMAIN,"sourceKind":"note","sourceRef":work,"workRef":work})).unwrap();
        let document = materials::read_document(&db, &q).await.unwrap();
        assert_eq!(document["title"], d.note["displayTitle"]);
        assert_eq!(
            document["fragments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|f| f["blockRef"].as_str().unwrap())
                .collect::<Vec<_>>(),
            research_dossier::ordered_refs(&d)
        );
        assert_eq!(
            document["context"]["dossier"]["readingView"],
            d.reading_view
        );
    }
}
