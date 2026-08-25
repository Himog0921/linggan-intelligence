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
fn discovery_rejects_quota_reached_before_the_fixed_visible_card_quota() {
    let input = mutated(|package| {
        package["coverage"]["stoppedReason"] = serde_json::json!("quota_reached");
    });
    let error = parse_discovery_package(&input).expect_err(
        "two visible cards cannot truthfully report reaching the fixed quota of twenty",
    );

    assert!(matches!(
        error,
        DiscoveryContractError::QuotaReachedBeforeMaximumQuota
    ));
    assert!(
        parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY).is_ok(),
        "risk_control remains a truthful allowed partial stop reason"
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
fn discovery_rejects_blank_identity_or_missing_or_invalid_observation_time() {
    for mutation in [
        Box::new(|package: &mut serde_json::Value| {
            package["cards"][0]["content"]["platformContentId"] = serde_json::json!("");
        }) as Box<dyn FnOnce(&mut serde_json::Value)>,
        Box::new(|package: &mut serde_json::Value| {
            package["observedAt"] = serde_json::json!("");
            package["cards"][0]["occurrence"]["observedAt"] = serde_json::json!("");
        }),
        Box::new(|package: &mut serde_json::Value| {
            package["observedAt"] = serde_json::json!("2026-99-40T28:70:70Z");
            package["cards"][0]["occurrence"]["observedAt"] =
                serde_json::json!("2026-99-40T28:70:70Z");
        }),
    ] {
        let error = parse_discovery_package(&mutated(mutation)).expect_err(
            "a visible card needs a stable identity and a concrete valid observation time",
        );
        assert!(
            matches!(
                error,
                DiscoveryContractError::MissingPlatformContentIdentity
                    | DiscoveryContractError::InvalidObservedAt
            ),
            "the incomplete discovery fact must be rejected specifically"
        );
    }
}

#[test]
fn discovery_rejects_two_cards_with_the_same_visible_position() {
    let input = mutated(|package| {
        package["cards"][1]["occurrence"]["resultPosition"] = serde_json::json!(1);
    });
    let error = parse_discovery_package(&input)
        .expect_err("two cards cannot both claim the same visible search result position");
    assert!(matches!(
        error,
        DiscoveryContractError::DuplicateResultPosition
    ));
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
fn discovery_contract_keeps_unknown_published_time_for_the_read_projection() {
    let package = parse_discovery_package(PARTIAL_VISIBLE_DISCOVERY).expect("fixture is valid");
    assert_eq!(
        package.cards()[1].content().published_at_source_text(),
        None,
        "the discovery contract preserves absent source publication time as unknown"
    );
    // Result filtering belongs to the local read projection. Its PostgreSQL proof separately
    // verifies that this unknown value is excluded from a PublishedWindow query.
}
