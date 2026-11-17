//! Read-only opportunity endpoints. They never start a worker, model or collection.
use super::{LocalWebState, comment_study, local_read_json_error};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State, rejection::QueryRejection},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use linggan_intelligence::opportunity::{
    CardError, EvidenceQuery, OpportunityListQuery, OpportunityReadError, read_card,
    read_card_in_domain, read_evidence, read_evidence_in_domain, read_opportunities,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use uuid::Uuid;

#[derive(Clone)]
struct OpportunityAccess {
    enabled: bool,
    cursor_key: [u8; 32],
}

impl OpportunityAccess {
    fn new(enabled: bool) -> Self {
        let mut cursor_key = [0_u8; 32];
        cursor_key[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        cursor_key[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        Self {
            enabled,
            cursor_key,
        }
    }

    fn signature(&self, token: &str) -> String {
        linggan_evidence::hmac_sha256_hex(
            &self.cursor_key,
            format!("opportunity-page-v1:{token}").as_bytes(),
        )
    }

    fn sign(&self, token: &str) -> String {
        format!("{token}.{}", self.signature(token))
    }

    fn verify(&self, value: &str) -> Result<String, OpportunityReadError> {
        let (token, supplied) = value
            .split_once('.')
            .ok_or(OpportunityReadError::InvalidCursor)?;
        if token.is_empty() || token.len() > 2048 || supplied.len() != 64 {
            return Err(OpportunityReadError::InvalidCursor);
        }
        let expected = self.signature(token);
        if !bool::from(expected.as_bytes().ct_eq(supplied.as_bytes())) {
            return Err(OpportunityReadError::InvalidCursor);
        }
        Ok(token.to_owned())
    }
}

pub(super) fn routes(enabled: bool) -> Router<LocalWebState> {
    let access = Arc::new(OpportunityAccess::new(enabled));
    let protected = Router::new()
        .route(
            "/api/local/opportunity/domains/{domain_ref}/opportunities",
            get(list),
        )
        .route(
            "/api/local/opportunity/problems/{problem_ref}/card",
            get(card),
        )
        .route(
            "/api/local/opportunity/problems/{problem_ref}/evidence",
            get(evidence),
        )
        .route(
            "/api/local/opportunity/domains/{domain_ref}/problems/{problem_ref}/card",
            get(scoped_card),
        )
        .route(
            "/api/local/opportunity/domains/{domain_ref}/problems/{problem_ref}/evidence",
            get(scoped_evidence),
        )
        .route("/api/local/opportunity/{*rest}", any(not_found))
        .layer(middleware::from_fn_with_state(
            access.clone(),
            feature_guard,
        ));
    Router::new()
        .route("/api/local/opportunity/status", get(status))
        .merge(protected)
        .layer(Extension(access))
        .layer(middleware::from_fn(
            comment_study::local_comment_study_guard,
        ))
}

async fn feature_guard(
    State(access): State<Arc<OpportunityAccess>>,
    request: Request,
    next: Next,
) -> Response {
    if !access.enabled {
        return local_read_json_error(StatusCode::NOT_FOUND, "opportunity_card_disabled");
    }
    next.run(request).await
}

async fn status(Extension(access): Extension<Arc<OpportunityAccess>>) -> Json<Value> {
    Json(json!({"enabled":access.enabled}))
}

async fn not_found() -> Response {
    local_read_json_error(StatusCode::NOT_FOUND, "opportunity_not_found")
}

fn parse_id(value: &str) -> Result<Uuid, OpportunityReadError> {
    Uuid::parse_str(value).map_err(|_| OpportunityReadError::InvalidQuery)
}

fn decode_cursor(
    access: &OpportunityAccess,
    cursor: &mut Option<String>,
) -> Result<(), OpportunityReadError> {
    if let Some(value) = cursor.take() {
        *cursor = Some(access.verify(&value)?);
    }
    Ok(())
}

fn reply(access: &OpportunityAccess, result: Result<Value, OpportunityReadError>) -> Response {
    match result {
        Ok(mut value) => {
            if let Some(token) = value.pointer("/page/nextCursor").and_then(Value::as_str) {
                let signed = access.sign(token);
                value["page"]["nextCursor"] = json!(signed);
            }
            Json(value).into_response()
        }
        Err(error) => failure(error),
    }
}

fn failure(error: OpportunityReadError) -> Response {
    let (status, code) = match error {
        OpportunityReadError::InvalidQuery => (StatusCode::BAD_REQUEST, "invalid_request"),
        OpportunityReadError::InvalidLimit => (StatusCode::BAD_REQUEST, "invalid_limit"),
        OpportunityReadError::InvalidCursor => (StatusCode::BAD_REQUEST, "invalid_cursor"),
        OpportunityReadError::CursorScopeMismatch => {
            (StatusCode::BAD_REQUEST, "cursor_scope_mismatch")
        }
        OpportunityReadError::PageChanged => (StatusCode::CONFLICT, "opportunity_page_changed"),
        OpportunityReadError::NotFound => (StatusCode::NOT_FOUND, "opportunity_not_found"),
        OpportunityReadError::LineageConflict => (
            StatusCode::CONFLICT,
            "opportunity_problem_lineage_ambiguous",
        ),
        OpportunityReadError::SourceUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_card_source_unavailable",
        ),
        OpportunityReadError::PeriodSnapshotUnavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_period_snapshot_unavailable",
        ),
        OpportunityReadError::Snapshot(CardError::SourcesBusy) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_card_sources_busy",
        ),
        OpportunityReadError::Snapshot(CardError::RestrictionVisibility) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_card_restriction_visibility_unavailable",
        ),
        OpportunityReadError::Snapshot(CardError::SourceLockPermission) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_card_source_lock_permission_missing",
        ),
        OpportunityReadError::Database(_)
        | OpportunityReadError::Snapshot(_)
        | OpportunityReadError::InvalidModel => (
            StatusCode::SERVICE_UNAVAILABLE,
            "opportunity_read_unavailable",
        ),
    };
    local_read_json_error(status, code)
}

async fn list(
    State(state): State<LocalWebState>,
    Extension(access): Extension<Arc<OpportunityAccess>>,
    Path(domain): Path<String>,
    query: Result<Query<OpportunityListQuery>, QueryRejection>,
) -> Response {
    let (Ok(domain), Ok(Query(mut query))) = (parse_id(&domain), query) else {
        return failure(OpportunityReadError::InvalidQuery);
    };
    if let Err(error) = decode_cursor(&access, &mut query.cursor) {
        return failure(error);
    }
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    reply(&access, read_opportunities(database, domain, &query).await)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CardQuery {
    as_of: Option<String>,
}

async fn card(
    State(state): State<LocalWebState>,
    Extension(access): Extension<Arc<OpportunityAccess>>,
    Path(problem): Path<String>,
    query: Result<Query<CardQuery>, QueryRejection>,
) -> Response {
    let (Ok(problem), Ok(Query(query))) = (parse_id(&problem), query) else {
        return failure(OpportunityReadError::InvalidQuery);
    };
    if query.as_of.is_some() {
        return local_read_json_error(
            StatusCode::NOT_IMPLEMENTED,
            "opportunity_as_of_not_supported",
        );
    }
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    reply(&access, read_card(database, problem).await)
}

async fn evidence(
    State(state): State<LocalWebState>,
    Extension(access): Extension<Arc<OpportunityAccess>>,
    Path(problem): Path<String>,
    query: Result<Query<EvidenceQuery>, QueryRejection>,
) -> Response {
    let (Ok(problem), Ok(Query(mut query))) = (parse_id(&problem), query) else {
        return failure(OpportunityReadError::InvalidQuery);
    };
    if let Err(error) = decode_cursor(&access, &mut query.cursor) {
        return failure(error);
    }
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    reply(&access, read_evidence(database, problem, &query).await)
}

async fn scoped_card(
    State(state): State<LocalWebState>,
    Extension(access): Extension<Arc<OpportunityAccess>>,
    Path((domain, problem)): Path<(String, String)>,
    query: Result<Query<CardQuery>, QueryRejection>,
) -> Response {
    let (Ok(domain), Ok(problem), Ok(Query(query))) =
        (parse_id(&domain), parse_id(&problem), query)
    else {
        return failure(OpportunityReadError::InvalidQuery);
    };
    if query.as_of.is_some() {
        return local_read_json_error(
            StatusCode::NOT_IMPLEMENTED,
            "opportunity_as_of_not_supported",
        );
    }
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    reply(
        &access,
        read_card_in_domain(database, domain, problem).await,
    )
}

async fn scoped_evidence(
    State(state): State<LocalWebState>,
    Extension(access): Extension<Arc<OpportunityAccess>>,
    Path((domain, problem)): Path<(String, String)>,
    query: Result<Query<EvidenceQuery>, QueryRejection>,
) -> Response {
    let (Ok(domain), Ok(problem), Ok(Query(mut query))) =
        (parse_id(&domain), parse_id(&problem), query)
    else {
        return failure(OpportunityReadError::InvalidQuery);
    };
    if let Err(error) = decode_cursor(&access, &mut query.cursor) {
        return failure(error);
    }
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    reply(
        &access,
        read_evidence_in_domain(database, domain, problem, &query).await,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_web::LocalDatabaseState;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use std::collections::BTreeSet;
    use tower::ServiceExt;

    fn app(enabled: bool) -> Router {
        routes(enabled).with_state(LocalWebState {
            database: LocalDatabaseState::NotConfigured,
            active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
            account_digest_key: None,
        })
    }

    async fn call(app: Router, method: &str, uri: &str) -> (StatusCode, Value) {
        let response = app
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .header("Host", "127.0.0.1:3001")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&body).unwrap())
    }

    #[tokio::test]
    async fn disabled_routes_and_status_are_independent_of_database() {
        let app = app(false);
        assert_eq!(
            call(app.clone(), "GET", "/api/local/opportunity/status").await,
            (StatusCode::OK, json!({"enabled":false}))
        );
        for uri in [
            "/api/local/opportunity/domains/not-a-uuid/opportunities?limit=broken",
            "/api/local/opportunity/problems/not-a-uuid/card?asOf=anything",
            "/api/local/opportunity/problems/not-a-uuid/evidence?kind=broken",
            "/api/local/opportunity/domains/not-a-uuid/problems/not-a-uuid/card",
            "/api/local/opportunity/domains/not-a-uuid/problems/not-a-uuid/evidence?kind=comments",
            "/api/local/opportunity/not-a-route",
        ] {
            let (status, body) = call(app.clone(), "GET", uri).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
            assert_eq!(body["code"], "opportunity_card_disabled");
        }
    }

    #[tokio::test]
    async fn enabled_status_and_historical_request_are_explicit() {
        let app = app(true);
        assert_eq!(
            call(app.clone(), "GET", "/api/local/opportunity/status").await,
            (StatusCode::OK, json!({"enabled":true}))
        );
        let (status, body) = call(app,"GET",
            "/api/local/opportunity/problems/00000000-0000-4000-8000-000000000001/card?asOf=2026-01-01T00%3A00%3A00Z").await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(body["code"], "opportunity_as_of_not_supported");
    }

    #[test]
    fn cursor_signature_rejects_changed_or_other_process_tokens() {
        let access = OpportunityAccess::new(true);
        let token = access.sign("synthetic-page");
        assert_eq!(access.verify(&token).unwrap(), "synthetic-page");
        assert!(
            access
                .verify(&token.replace("synthetic", "changed"))
                .is_err()
        );
        assert!(OpportunityAccess::new(true).verify(&token).is_err());
    }

    async fn insert_synthetic_cards(
        db: &linggan_storage_postgres::Database,
        domain: Uuid,
    ) -> (Uuid, Value) {
        use sha2::{Digest, Sha256};
        let mut payload: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/intelligence/tests/opportunity_metrics_postgres/p1-card.snapshot.json"
        )))
        .unwrap();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let index = json!([{"periodStart":"2026-11-02","protocolVersion":null,"evidenceTier":"significant"}]);
        for id in [first, second] {
            sqlx::query("INSERT INTO linggan_opportunity_problem(problem_ref,source_ref,source_definition_ref,domain_ref,label) VALUES($1,$1,$2,$3,'synthetic problem')")
                .bind(id).bind(Uuid::new_v4()).bind(domain).execute(db.pool()).await.unwrap();
            payload["problem"]["id"] = json!(id);
            let fingerprint: String =
                Sha256::digest(serde_json::to_vec(&(&payload, &index)).unwrap())
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
            sqlx::query("INSERT INTO linggan_opportunity_opportunity_card(problem_ref,payload,as_of,taxonomy_version,metric_def_version,input_fingerprint) VALUES($1,$2,$3::timestamptz,1,1,$4)")
                .bind(id).bind(&payload).bind(payload["asOf"].as_str().unwrap()).bind(&fingerprint)
                .execute(db.pool()).await.unwrap();
            sqlx::query("INSERT INTO linggan_opportunity_card_period_index(problem_ref,taxonomy_version,metric_def_version,card_fingerprint,period_index) VALUES($1,1,1,$2,$3)")
                .bind(id).bind(&fingerprint).bind(&index)
                .execute(db.pool()).await.unwrap();
        }
        (first, payload)
    }

    #[tokio::test]
    #[ignore = "owned PostgreSQL proof; synthetic cards only"]
    async fn enabled_routes_return_cards_and_guard_changed_pages() {
        let url = std::env::var("OPPORTUNITY_PROOF_DATABASE_URL").expect("owned proof URL");
        let db = linggan_storage_postgres::testing::isolated_proof_schema(
            &url,
            "opportunity_t15_http",
            super::super::full_schema_fixture::FULL_MIGRATIONS,
        )
        .await
        .unwrap();
        let domain = Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
        let (first, mut payload) = insert_synthetic_cards(&db, domain).await;
        let app = routes(true).with_state(LocalWebState {
            database: LocalDatabaseState::Ready(Arc::new(db.clone())),
            active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
            account_digest_key: None,
        });
        let path = format!("/api/local/opportunity/problems/{first}/card");
        let (status, card) = call(app.clone(), "GET", &path).await;
        assert_eq!(status, StatusCode::OK, "{card}");
        payload["problem"]["id"] = json!(first);
        assert_eq!(
            card, payload,
            "the HTTP card matches the approved P1 snapshot"
        );
        let scoped = format!("/api/local/opportunity/domains/{domain}/problems/{first}");
        assert_eq!(
            call(app.clone(), "GET", &format!("{scoped}/card")).await.0,
            StatusCode::OK
        );
        let other_domain = "00000000-0000-4000-8000-000000000002";
        let other = format!("/api/local/opportunity/domains/{other_domain}/problems/{first}");
        assert_eq!(
            call(app.clone(), "GET", &format!("{other}/card")).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            call(
                app.clone(),
                "GET",
                &format!("{other}/evidence?kind=creators")
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let comparisons = format!(
            "/api/local/opportunity/domains/{domain}/opportunities?includeWeeklyComparison=true&limit=1"
        );
        let (status, observed) = call(app.clone(), "GET", &comparisons).await;
        assert_eq!(status, StatusCode::OK, "{observed}");
        assert_eq!(
            observed["weeklyComparisons"][0]["groups"][0]["status"],
            "incomparable"
        );
        let creators =
            format!("/api/local/opportunity/problems/{first}/evidence?kind=creators&limit=1");
        let (status, result) = call(app.clone(), "GET", &creators).await;
        assert_eq!(status, StatusCode::OK, "{result}");
        assert_eq!(result["items"].as_array().unwrap().len(), 1);

        let historical = format!(
            "/api/local/opportunity/domains/{domain}/opportunities?period=2026-11-02&tier=significant&limit=1"
        );
        let (status, matched) = call(app.clone(), "GET", &historical).await;
        assert_eq!(status, StatusCode::OK, "{matched}");
        assert_eq!(
            matched["periodMatches"][0]["groups"][0]["evidenceTier"],
            "significant"
        );
        assert_eq!(
            matched["items"][0]["demand"]["groups"][0]["evidenceTier"],
            "established"
        );

        let list = format!("/api/local/opportunity/domains/{domain}/opportunities?limit=1");
        let (status, page) = call(app.clone(), "GET", &list).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert_eq!(page["page"]["hasMore"], true);
        let next = page["page"]["nextCursor"].as_str().unwrap().to_owned();
        let (status, second_page) =
            call(app.clone(), "GET", &format!("{list}&cursor={next}")).await;
        assert_eq!(status, StatusCode::OK, "{second_page}");
        assert_eq!(second_page["items"].as_array().unwrap().len(), 1);
        sqlx::query("UPDATE linggan_opportunity_opportunity_card SET input_fingerprint=repeat('b',64) WHERE problem_ref=$1")
            .bind(first).execute(db.pool()).await.unwrap();
        let (status, changed) = call(app.clone(), "GET", &format!("{list}&cursor={next}")).await;
        assert_eq!(status, StatusCode::CONFLICT, "{changed}");
        assert_eq!(changed["code"], "opportunity_page_changed");
        let (status, stale) = call(app.clone(), "GET", &historical).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{stale}");
        assert_eq!(stale["code"], "opportunity_period_snapshot_unavailable");

        sqlx::query("UPDATE linggan_opportunity_opportunity_card SET payload=jsonb_set(payload,'{demand,sampleVoices,0,commenterHash}',to_jsonb('private'::text),true) WHERE problem_ref=$1")
            .bind(first).execute(db.pool()).await.unwrap();
        let (status, body) = call(app, "GET", &path).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(!body.to_string().contains("private"));
    }
}
