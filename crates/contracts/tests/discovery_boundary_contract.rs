use linggan_contracts::{
    CoverPresentationState, DiscoveryContractError, EvidenceQuery, PublishedWindow,
    parse_discovery_package,
};

const PARTIAL_VISIBLE_DISCOVERY: &str = r#"
{
  "contractVersion": "xhs.discovery.visible-card.v1",
  "acquisitionSpec": {
    "platform": "xhs",
    "query": "ADHD",
    "sort": "comprehensive",
    "target": {
      "basis": "maximum_quota",
      "unit": "visible_search_card",
      "maximumQuota": 20
    }
  },
  "observedAt": "2026-08-25T09:30:00+08:00",
  "coverage": {
    "unit": "visible_search_card",
    "visibleCards": 2,
    "stoppedReason": "risk_control"
  },
  "cards": [
    {
      "content": {
        "platformContentId": "note-a",
        "title": "ADHD 作业怎么开始",
        "creatorDisplayName": "家长甲",
        "publishedAtSourceText": "3小时前",
        "coverCandidate": {
          "observedExternalUri": "https://ci.xiaohongshu.com/cover-a"
        }
      },
      "occurrence": {
        "query": "ADHD",
        "sort": "comprehensive",
        "observedAt": "2026-08-25T09:30:00+08:00",
        "resultPosition": 1
      }
    },
    {
      "content": {
        "platformContentId": "note-b",
        "title": "A娃家庭的一天",
        "creatorDisplayName": "家长乙"
      },
      "occurrence": {
        "query": "ADHD",
        "sort": "comprehensive",
        "observedAt": "2026-08-25T09:30:00+08:00",
        "resultPosition": 2
      }
    }
  ]
}
"#;

fn mutated(value: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut package: serde_json::Value =
        serde_json::from_str(PARTIAL_VISIBLE_DISCOVERY).expect("fixture must be valid JSON");
    value(&mut package);
    serde_json::to_string(&package).expect("mutation must be valid JSON")
}

#[test]
fn acquisition_and_local_evidence_query_are_disjoint_contracts() {
    let query: EvidenceQuery = serde_json::from_str(
        r#"{
          "text":"A娃",
          "scope":"all_accepted_material",
          "window":"last_30_days",
          "sort":"relevance"
        }"#,
    )
    .expect("a local EvidenceQuery must not need platform fields");

    assert_eq!(query.window(), PublishedWindow::Last30Days);
    assert!(
        serde_json::from_str::<EvidenceQuery>(
            r#"{
          "text":"A娃",
          "scope":"all_accepted_material",
          "window":"last_30_days",
          "sort":"relevance",
          "platform":"xhs"
        }"#
        )
        .is_err(),
        "a local EvidenceQuery must reject a platform command field"
    );
    assert!(
        serde_json::from_str::<EvidenceQuery>(
            r#"{
          "text":"A娃",
          "scope":"all_accepted_material",
          "window":"last_30_days",
          "sort":"relevance",
          "acquisitionSpec":{"platform":"xhs"}
        }"#
        )
        .is_err(),
        "a local EvidenceQuery must reject an embedded acquisition command"
    );

    let error = parse_discovery_package(&mutated(|package| {
        package["acquisitionSpec"]["evidenceQuery"] = serde_json::json!({"text": "A娃"});
    }))
    .expect_err("an AcquisitionSpec cannot embed a local EvidenceQuery");
    assert!(matches!(error, DiscoveryContractError::SchemaInvalid(_)));
}

#[test]
fn discovery_package_accepts_partial_visible_cards_without_inventing_missing_objects() {
    let package = parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY)
        .expect("two actually visible cards remain valid even when the maximum quota is twenty");

    assert_eq!(package.acquisition_spec().maximum_quota(), 20);
    assert_eq!(package.coverage().visible_cards(), 2);
    assert_eq!(package.cards().len(), 2);
    assert_eq!(
        package
            .cards()
            .iter()
            .map(|card| card.content().platform_content_id())
            .collect::<Vec<_>>(),
        vec!["note-a", "note-b"],
        "the contract returns only observed cards; it exposes no manufactured remainder"
    );
}

#[test]
fn discovery_payload_rejects_detail_comments_media_bytes_ocr_and_asr() {
    for forbidden_key in ["body", "comments", "mediaBytes", "ocrText", "asrTranscript"] {
        let input = mutated(|package| {
            package["cards"][0]["content"][forbidden_key] = serde_json::json!("forbidden");
        });
        let error = parse_discovery_package(&input)
            .expect_err("a discovery-only card must reject any deeper capture material");
        assert!(
            matches!(error, DiscoveryContractError::SchemaInvalid(_)),
            "{forbidden_key} must fail as an undeclared discovery-card field"
        );
    }
}

#[test]
fn search_position_is_only_a_discovery_occurrence_fact() {
    let input = mutated(|package| {
        package["cards"][0]["position"] = serde_json::json!(1);
    });
    let error =
        parse_discovery_package(&input).expect_err("position outside occurrence must be rejected");
    assert!(matches!(error, DiscoveryContractError::SchemaInvalid(_)));

    let package = parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY).expect("fixture is valid");
    assert_eq!(package.cards()[0].occurrence().result_position(), 1);
    assert_eq!(package.cards()[0].occurrence().query(), "ADHD");
    assert_eq!(
        package.cards()[0].occurrence().observed_at(),
        package.observed_at(),
        "position must remain attached to its query/sort/observation context"
    );
}

#[test]
fn remote_cover_is_only_a_candidate_and_never_a_display_url() {
    let package = parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY).expect("fixture is valid");
    let candidate = package.cards()[0]
        .content()
        .cover_candidate()
        .expect("fixture has an observed cover candidate");

    assert_eq!(
        candidate.presentation_state(),
        CoverPresentationState::MediaNotAcquired
    );
    assert_eq!(
        candidate.observed_external_uri(),
        "https://ci.xiaohongshu.com/cover-a",
        "the URI remains provenance only"
    );

    let input = mutated(|package| {
        package["cards"][0]["content"]["coverCandidate"]["displayUrl"] =
            serde_json::json!("https://ci.xiaohongshu.com/cover-a");
    });
    let error =
        parse_discovery_package(&input).expect_err("a candidate must reject a page display URL");
    assert!(matches!(error, DiscoveryContractError::SchemaInvalid(_)));
}

#[test]
fn published_window_never_defaults_unknown_source_time_into_results() {
    let package = parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY).expect("fixture is valid");
    assert_eq!(
        package.cards()[1].content().published_at_source_text(),
        None,
        "a card without a source publication-time assertion is unknown, not recent"
    );
}
