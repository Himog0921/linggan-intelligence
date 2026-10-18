//! Loopback-only Topic Workspace page and JSON composition.

use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_evidence::read_work_resource;
use linggan_intelligence::{
    TopicWorkspaceError, TopicWorkspaceImport, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};
use serde_json::{Value, json};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/topic-workspaces/{canonical_key}/materials/{work_ref}",
            get(read_material_json),
        )
        .route("/api/local/topic-workspaces", post(import_json))
        .route(
            "/api/local/topic-workspaces/{canonical_key}",
            get(read_json),
        )
}

async fn import_json(
    State(state): State<LocalWebState>,
    request: Result<Json<TopicWorkspaceImport>, JsonRejection>,
) -> Response {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Topic workspace import request rejected: {error}");
            return json_error(
                StatusCode::BAD_REQUEST,
                TopicWorkspaceApiOutcome::Rejected,
                "invalid_topic_workspace_request",
            );
        }
    };
    let Some(database) = state.database.database() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_read_model_not_connected",
        );
    };
    match topic_workspace_schema_is_ready(database).await {
        Ok(true) => {}
        Ok(false) => {
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                TopicWorkspaceApiOutcome::Unavailable,
                "topic_schema_not_ready",
            );
        }
        Err(error) => return unavailable(error),
    }
    match import_topic_workspace(database, &request).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => topic_error(error),
    }
}

async fn read_json(
    State(state): State<LocalWebState>,
    Path(canonical_key): Path<String>,
) -> Response {
    let Some(database) = state.database.database() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_read_model_not_connected",
        );
    };
    match read_topic_workspace(database, &canonical_key).await {
        Ok(Some(workspace)) => match compose_materials(database, workspace).await {
            Ok(payload) => Json(payload).into_response(),
            Err(error) => error,
        },
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            TopicWorkspaceApiOutcome::NotFound,
            "topic_workspace_not_found",
        ),
        Err(error) => topic_error(error),
    }
}

/// A frozen workspace already exposes these exact members. This detail path preserves
/// that membership boundary without inventing an ObservationDomain UUID mapping.
async fn read_material_json(
    State(state): State<LocalWebState>,
    Path((canonical_key, work_ref)): Path<(String, uuid::Uuid)>,
) -> Response {
    let Some(database) = state.database.database() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_read_model_not_connected",
        );
    };
    let workspace = match read_topic_workspace(database, &canonical_key).await {
        Ok(Some(workspace)) => workspace,
        Ok(None) => {
            return json_error(
                StatusCode::NOT_FOUND,
                TopicWorkspaceApiOutcome::NotFound,
                "topic_workspace_not_found",
            );
        }
        Err(error) => return topic_error(error),
    };
    if !workspace
        .members
        .iter()
        .any(|member| member.work_public_ref == work_ref)
    {
        return json_error(
            StatusCode::NOT_FOUND,
            TopicWorkspaceApiOutcome::NotFound,
            "topic_material_not_in_pack",
        );
    }
    match read_work_resource(database, work_ref).await {
        Ok(Some(work)) => Json(work).into_response(),
        Ok(None) => json_error(
            StatusCode::CONFLICT,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_material_pack_reference_unavailable",
        ),
        Err(error) => {
            eprintln!("Topic member read unavailable: {error}");
            json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                TopicWorkspaceApiOutcome::Unavailable,
                "work_resource_read_unavailable",
            )
        }
    }
}

async fn compose_materials(
    database: &linggan_storage_postgres::Database,
    workspace: linggan_intelligence::TopicWorkspace,
) -> Result<Value, Response> {
    let mut materials = Vec::with_capacity(workspace.members.len());
    for member in &workspace.members {
        let work = read_work_resource(database, member.work_public_ref)
            .await
            .map_err(|error| {
                eprintln!("Topic Work Resource read unavailable: {error}");
                json_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    TopicWorkspaceApiOutcome::Unavailable,
                    "work_resource_read_unavailable",
                )
            })?
            .ok_or_else(|| {
                json_error(
                    StatusCode::CONFLICT,
                    TopicWorkspaceApiOutcome::Unavailable,
                    "topic_material_pack_reference_unavailable",
                )
            })?;
        materials.push(json!({
            "classification":member,
            "detailUrl":format!("/api/local/topic-workspaces/{}/materials/{}", workspace.canonical_key, member.work_public_ref),
            "workResource":work
        }));
    }
    Ok(json!({"topic":workspace,"materials":materials}))
}

fn topic_error(error: TopicWorkspaceError) -> Response {
    match error {
        TopicWorkspaceError::InvalidRequest(_) => json_error(
            StatusCode::BAD_REQUEST,
            TopicWorkspaceApiOutcome::Rejected,
            "invalid_topic_workspace_request",
        ),
        TopicWorkspaceError::UnknownWorkResource(_) => json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            TopicWorkspaceApiOutcome::Rejected,
            "unknown_work_resource",
        ),
        TopicWorkspaceError::IdempotencyConflict => json_error(
            StatusCode::CONFLICT,
            TopicWorkspaceApiOutcome::Conflict,
            "topic_idempotency_conflict",
        ),
        TopicWorkspaceError::VersionConflict { .. } => json_error(
            StatusCode::CONFLICT,
            TopicWorkspaceApiOutcome::Conflict,
            "topic_version_conflict",
        ),
        TopicWorkspaceError::CorruptProjection | TopicWorkspaceError::Database(_) => {
            unavailable(error)
        }
    }
}

fn unavailable(error: TopicWorkspaceError) -> Response {
    eprintln!("Topic workspace unavailable: {error}");
    json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        TopicWorkspaceApiOutcome::Unavailable,
        "topic_workspace_unavailable",
    )
}

#[derive(Clone, Copy)]
enum TopicWorkspaceApiOutcome {
    Unavailable,
    Rejected,
    Conflict,
    NotFound,
}

impl TopicWorkspaceApiOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Rejected => "rejected",
            Self::Conflict => "conflict",
            Self::NotFound => "not_found",
        }
    }
}

fn json_error(
    status: StatusCode,
    outcome: TopicWorkspaceApiOutcome,
    code: &'static str,
) -> Response {
    (
        status,
        Json(json!({
            "operation":"topic_workspace",
            "outcome":outcome.as_str(),
            "code":code
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn error_responses_preserve_their_real_outcome() {
        let unknown_work = uuid::Uuid::nil();
        let cases = [
            (
                topic_error(TopicWorkspaceError::InvalidRequest("invalid")),
                StatusCode::BAD_REQUEST,
                "rejected",
                "invalid_topic_workspace_request",
            ),
            (
                topic_error(TopicWorkspaceError::UnknownWorkResource(unknown_work)),
                StatusCode::UNPROCESSABLE_ENTITY,
                "rejected",
                "unknown_work_resource",
            ),
            (
                topic_error(TopicWorkspaceError::IdempotencyConflict),
                StatusCode::CONFLICT,
                "conflict",
                "topic_idempotency_conflict",
            ),
            (
                topic_error(TopicWorkspaceError::VersionConflict {
                    expected: Some(1),
                    actual: Some(2),
                }),
                StatusCode::CONFLICT,
                "conflict",
                "topic_version_conflict",
            ),
            (
                json_error(
                    StatusCode::NOT_FOUND,
                    TopicWorkspaceApiOutcome::NotFound,
                    "topic_workspace_not_found",
                ),
                StatusCode::NOT_FOUND,
                "not_found",
                "topic_workspace_not_found",
            ),
            (
                topic_error(TopicWorkspaceError::CorruptProjection),
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "topic_workspace_unavailable",
            ),
        ];

        for (response, status, outcome, code) in cases {
            assert_eq!(response.status(), status);
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("error body reads");
            let body: Value = serde_json::from_slice(&body).expect("error body is JSON");
            assert_eq!(body.pointer("/operation"), Some(&json!("topic_workspace")));
            assert_eq!(body.pointer("/outcome"), Some(&json!(outcome)));
            assert_eq!(body.pointer("/code"), Some(&json!(code)));
        }
    }
}
