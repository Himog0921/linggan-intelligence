use super::*;
use axum::{
    extract::rejection::{JsonRejection, QueryRejection},
    http::{HeaderMap, StatusCode},
};
use linggan_intelligence::ops_content_lens::{
    BoardStatus, LensDefinitionError, PageRequest, SnapshotLookup, SnapshotScope, board_status,
    build_snapshot, publications, read_snapshot, spot_check,
};

pub(super) fn routes(enabled: bool) -> Router<LocalWebState> {
    let status_path = "/api/local/ops/content-lens/status";
    let snapshot_path = "/api/local/ops/content-lens/snapshot";
    let members_path = "/api/local/ops/content-lens/publications";
    let spot_path = "/api/local/ops/content-lens/spot-check";
    let runs_path = "/api/local/ops/content-lens/runs";
    let snapshots_path = "/api/local/ops/content-lens/snapshots";
    if !enabled {
        return Router::new()
            .route(
                status_path,
                get(|| async {
                    Json(BoardStatus {
                        enabled: false,
                        run: None,
                    })
                }),
            )
            .route(snapshot_path, get(disabled))
            .route(members_path, get(disabled))
            .route(spot_path, get(disabled))
            .route(runs_path, post(disabled))
            .route(snapshots_path, post(disabled));
    }
    Router::new()
        .route(status_path, get(status))
        .route(snapshot_path, get(snapshot))
        .route(members_path, get(members))
        .route(spot_path, get(spot))
        .route(runs_path, post(start))
        .route(snapshots_path, post(generate))
}

async fn disabled() -> Response {
    local_read_json_error(StatusCode::NOT_FOUND, "ops_content_lens_disabled")
}

async fn admin_db<'a>(
    state: &'a LocalWebState,
    headers: &HeaderMap,
) -> Result<&'a linggan_storage_postgres::Database, Response> {
    super::ops_overview_api::require_admin(state, headers).await?;
    state
        .database
        .database()
        .ok_or_else(super::ops_api::not_connected)
}

fn read_response<T: serde::Serialize>(result: Result<T, LensDefinitionError>) -> Response {
    match result {
        Ok(data) => Json(data).into_response(),
        Err(LensDefinitionError::SnapshotNotFound) => {
            local_read_json_error(StatusCode::NOT_FOUND, "ops_lens_snapshot_not_found")
        }
        Err(LensDefinitionError::InvalidScope) => {
            local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid")
        }
        Err(_) => {
            local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "ops_lens_read_unavailable")
        }
    }
}

async fn status(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    match admin_db(&state, &headers).await {
        Ok(db) => read_response(board_status(db).await),
        Err(response) => response,
    }
}
type ReadQuery<T> = Result<Query<T>, QueryRejection>;

async fn snapshot(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    query: ReadQuery<SnapshotLookup>,
) -> Response {
    let db = match admin_db(&state, &headers).await {
        Ok(db) => db,
        Err(response) => return response,
    };
    let Ok(Query(query)) = query else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid");
    };
    read_response(read_snapshot(db, query).await)
}

async fn members(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    query: ReadQuery<PageRequest>,
) -> Response {
    let db = match admin_db(&state, &headers).await {
        Ok(db) => db,
        Err(response) => return response,
    };
    let Ok(Query(query)) = query else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid");
    };
    read_response(publications(db, query).await)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SpotQuery {
    snapshot_ref: uuid::Uuid,
}

async fn spot(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    query: ReadQuery<SpotQuery>,
) -> Response {
    let db = match admin_db(&state, &headers).await {
        Ok(db) => db,
        Err(response) => return response,
    };
    let Ok(Query(query)) = query else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid");
    };
    read_response(spot_check(db, query.snapshot_ref).await)
}

async fn generate(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    body: Result<Json<SnapshotScope>, JsonRejection>,
) -> Response {
    if let Err(response) = super::ops_overview_api::require_admin(&state, &headers).await {
        return response;
    }
    let Some(db) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let Ok(Json(scope)) = body else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid");
    };
    match build_snapshot(db, scope).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(LensDefinitionError::InvalidScope) => {
            local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_scope_invalid")
        }
        Err(LensDefinitionError::MissingAgreement) => {
            local_read_json_error(StatusCode::CONFLICT, "ops_lens_missing_agreement")
        }
        Err(LensDefinitionError::NoMethod) => {
            local_read_json_error(StatusCode::CONFLICT, "ops_lens_no_current_method")
        }
        Err(_) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "ops_lens_snapshot_unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use std::collections::BTreeSet;
    use tower::ServiceExt;

    #[tokio::test]
    async fn snapshot_write_obeys_feature_and_admin_guards() {
        for (enabled, status, code) in [
            (false, StatusCode::NOT_FOUND, "ops_content_lens_disabled"),
            (
                true,
                StatusCode::SERVICE_UNAVAILABLE,
                "ops_auth_unavailable",
            ),
        ] {
            let app = routes(enabled).with_state(LocalWebState {
                database: LocalDatabaseState::NotConfigured,
                active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
                account_digest_key: None,
            });
            let response = app
                .oneshot(
                    Request::post("/api/local/ops/content-lens/snapshots")
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
            let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["code"],
                code
            );
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StartRun {
    command_ref: uuid::Uuid,
}

async fn start(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    body: Result<Json<StartRun>, JsonRejection>,
) -> Response {
    if let Err(response) = super::ops_overview_api::require_admin(&state, &headers).await {
        return response;
    }
    let Some(db) = state.database.database() else {
        return super::ops_api::not_connected();
    };
    let Ok(Json(request)) = body else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "ops_lens_invalid_run");
    };
    super::ai_settings::respond(
        linggan_intelligence::ops_content_lens::label::start(db, request.command_ref).await,
    )
}
