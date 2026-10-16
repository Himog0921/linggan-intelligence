use super::*;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use linggan_storage_postgres::testing::isolated_proof_schema;
use tower::ServiceExt;

#[test]
fn creator_archive_errors_do_not_report_database_failures_as_duplicate_work() {
    assert_eq!(
        creator_archive_database_kind(&sqlx::Error::RowNotFound),
        "database_row_not_found"
    );
    assert_eq!(
        creator_archive_database_kind(&sqlx::Error::PoolTimedOut),
        "database_pool_timeout"
    );
    let database =
        RequestLeaseError::Acquisition(AcquisitionChainError::Database(sqlx::Error::RowNotFound));
    assert_eq!(
        creator_archive_error_receipt(&database),
        "archive_request_failed"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Lease(LeaseError::Database(
            sqlx::Error::PoolTimedOut,
        ))),
        "archive_request_failed"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Lease(LeaseError::SchemaUnavailable)),
        "archive_request_failed"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Lease(LeaseError::NoStation)),
        "archive_lease_failed"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::InvalidMaterialTargets,
        )),
        "archive_request_failed"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchivePurposeMismatch,
        )),
        "archive_purpose_mismatch"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveNotReady {
                reason: "detail_batch_in_flight",
            },
        )),
        "archive_in_progress"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveNotReady {
                reason: "archive_baseline_complete",
            },
        )),
        "archive_baseline_complete"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveNotReady {
                reason: "detail_gap_not_schedulable",
            },
        )),
        "archive_gap_not_schedulable"
    );
    assert_eq!(
        creator_archive_error_receipt(&RequestLeaseError::Acquisition(
            AcquisitionChainError::TargetNotRequestable {
                state: "monitoring".to_owned(),
            },
        )),
        "archive_not_requestable"
    );
}

#[tokio::test]
async fn lifecycle_api_keeps_a_closed_query_contract() {
    let invalid_enum = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?window=last_2160_hours")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid_enum.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unused_selection = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?selected_work=00000000-0000-0000-0000-000000000002")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unused_selection.status(), StatusCode::BAD_REQUEST);

    let not_connected = app()
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000001/lifecycle?window=all&metric=likes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_connected.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn lifecycle_api_returns_a_minimal_target_projection() {
    let database = proof_database("creator_lifecycle_api").await;
    let target_ref = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO collection_observation_target \
         (target_ref,platform,target_kind,identity_key,display_name,source) \
         VALUES ($1,'xhs','keyword','ADHD','ADHD','manual')",
    )
    .bind(target_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let response = app_with_database(database.clone())
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/local/collection/targets/{target_ref}/lifecycle?window=all&metric=comments"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        payload.pointer("/status").and_then(Value::as_str),
        Some("NOT_APPLICABLE")
    );
    assert!(payload.pointer("/analysis").is_none());
    for forbidden in [
        "bodyText",
        "commentId",
        "commentExternalId",
        "media",
        "evidence",
        "monitoringValue",
        "opportunityScore",
        "selectedWork",
        "title",
        "publishedAt",
        "publishedLocalDate",
        "publishedAtEpochMs",
        "metricValue",
        "authorExternalId",
    ] {
        assert_no_json_key(&payload, forbidden);
    }

    // Page DOM and URL normalization moved to the React browser proof. The public lifecycle
    // route above retains its minimal payload contract; its missing-object behavior is below.

    let missing = app_with_database(database)
        .oneshot(
            Request::builder()
                .uri("/api/local/collection/targets/00000000-0000-0000-0000-000000000003/lifecycle")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

async fn proof_database(schema: &str) -> Database {
    let url = std::env::var("LOCAL_001_PROOF_DATABASE_URL").expect("proof URL is supplied");
    isolated_proof_schema(&url, schema, full_schema_fixture::FULL_MIGRATIONS)
        .await
        .expect("migrations apply")
}

fn assert_no_json_key(value: &Value, forbidden: &str) {
    match value {
        Value::Object(object) => {
            assert!(
                !object.contains_key(forbidden),
                "lifecycle response must not copy Corpus facts or invent value fields: {forbidden}"
            );
            object
                .values()
                .for_each(|child| assert_no_json_key(child, forbidden));
        }
        Value::Array(values) => values
            .iter()
            .for_each(|child| assert_no_json_key(child, forbidden)),
        _ => {}
    }
}

#[test]
fn corpus_work_deep_link_does_not_fall_back_when_the_work_is_off_page() {
    assert!(EVIDENCE_LIBRARY_JS.contains("function directWorkItem(publicRef)"));
    assert!(EVIDENCE_LIBRARY_JS.contains("detailUrl: `${API_ROOT}/${encodeURIComponent(publicRef)}?domain=${encodeURIComponent(CORPUS_DOMAIN.ref)}"));
    assert!(EVIDENCE_LIBRARY_JS.contains(
        "'&collectionRunRef='+encodeURIComponent(creatorBacklink.get('collectionRunRef'))"
    ));
    assert!(EVIDENCE_LIBRARY_JS.contains("revealUrlSelection = false"));
    assert!(
        EVIDENCE_LIBRARY_JS.contains(
            "const selectionSource = revealUrlSelection && requestedRef ? 'url' : 'auto'"
        )
    );
    assert!(
        EVIDENCE_LIBRARY_JS
            .contains("await selectItem(directWorkItem(requestedRef), selectionSource)")
    );
    assert!(
        EVIDENCE_LIBRARY_JS.contains(
            "if (selectionSource === 'user' || selectionSource === 'url') openInspector()"
        )
    );
    assert_eq!(
        EVIDENCE_LIBRARY_JS
            .matches("loadList({ keepSelection: true, revealUrlSelection: true })")
            .count(),
        2,
        "initial URL restoration and popstate must reveal the addressed Work without adding history"
    );
    assert!(EVIDENCE_LIBRARY_JS.contains("const requestedRef = restoreRef || model.selectedRef"));
    assert!(!EVIDENCE_LIBRARY_JS.contains(
        "model.items.find((item) => item.identity?.publicRef === (restoreRef || model.selectedRef))\n          || model.items[0]"
    ));
}

#[tokio::test]
async fn target_route_never_reflects_an_invalid_selected_work() {
    let response = app()
        .oneshot(
            Request::builder()
                .header(header::HOST,"127.0.0.1:8080")
                .uri("/collection/targets?drawer=00000000-0000-0000-0000-000000000001&life_work=%22%3E%3Csvg%20onload%3Dalert%281%29%3E")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(!html.contains("life_work="));
    assert!(!html.contains("onload=alert"));
    assert!(!html.contains("\"><svg onload"));
}

#[test]
fn archive_return_path_drops_untrusted_navigation_fields() {
    let target_ref = uuid::Uuid::from_u128(606);
    let unsafe_form = TargetArchiveForm {
        archive_action: None,
        row_target_ref: target_ref,
        domain_ref: None,
        return_domain: None,
        return_filter: Some("\"><svg onload=alert(1)>".to_owned()),
        return_sort: Some("https://example.invalid".to_owned()),
    };
    assert_eq!(
        target_archive_return_path(&unsafe_form, Some("archive_not_requestable")),
        format!(
            "/collection/targets?drawer={target_ref}&dtab=archive&error=archive_not_requestable#target-archive"
        )
    );

    let valid_form = TargetArchiveForm {
        archive_action: None,
        row_target_ref: target_ref,
        domain_ref: None,
        return_domain: None,
        return_filter: Some("creator".to_owned()),
        return_sort: Some("last".to_owned()),
    };
    assert_eq!(
        target_archive_return_path(&valid_form, None),
        format!(
            "/collection/targets?filter=creator&sort=last&drawer={target_ref}&dtab=archive#target-archive"
        )
    );
}
