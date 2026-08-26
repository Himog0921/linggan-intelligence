mod collection;
mod evidence_page;
mod shell;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use linggan_contracts::{
    EvidenceQuery, parse_local_producer_attempt, parse_local_producer_submission,
    parse_local_task_spec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    DiscoveryIngressError, LocalAttemptOutcome, LocalProducerError, LocalSubmissionOutcome,
    LocalTaskOutcome, MediaUploadFinalizeClaim, ProducerRuntimeError, RuntimeAttemptOutcome,
    RuntimeSubmissionOutcome, RuntimeTaskOutcome, admit_media_blob, begin_media_upload,
    claim_media_upload_finalize, complete_media_upload, create_manual_task, create_producer_task,
    ingest_discovery_package, local_discovery_schema_is_ready, local_producer_schema_is_ready,
    producer_runtime_has_packages, producer_runtime_schema_is_ready, read_discovery_library,
    read_local_media_blob, read_media_upload_session, read_runtime_library,
    record_media_download_failure, record_media_upload_chunk, release_media_upload_finalize,
    start_local_attempt, start_producer_attempt, submit_local_package, submit_producer_package,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Error, ErrorKind, Seek, SeekFrom, Write},
    net::{Ipv4Addr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

const LOCAL_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
const LOCAL_PORT: u16 = 3000;
const FULL_PRODUCER_RUNTIME_DATA_STATE: &str = "LINGGAN_BROWSER_PRODUCER_RUNTIME";
const FULL_PRODUCER_RUNTIME_SCHEMA: &str = "PLUGIN_RUNTIME_001_SCHEMA_READY";
// The Browser Producer obtains these three paths from /health before it starts a
// durable outbox delivery. Keep the router and published contract on the same
// constants so a renamed server route cannot leave the plugin delivering to a
// stale endpoint.
const LOCAL_PRODUCER_TASK_CREATION_PATH: &str = "/api/local/producer/tasks";
const LOCAL_PRODUCER_ATTEMPT_START_PATH: &str = "/api/local/producer/runtime-attempts";
const LOCAL_PRODUCER_SUBMISSION_PATH: &str = "/api/local/producer/runtime-submissions";
const LIDS_TOKENS: &str = include_str!("local_web/lids_tokens.css");
const SHELL_CSS: &str = include_str!("local_web/shell.css");
const COLLECTION_WORKSPACE_CSS: &str = include_str!("local_web/collection_workspace.css");
const COLLECTION_WORKSPACE_JS: &str = include_str!("local_web/collection_workspace.js");
const EVIDENCE_LIBRARY_CSS: &str = include_str!("local_web/evidence_library.css");
#[cfg(test)]
const LIDS_TOKEN_DOCUMENT: &str = include_str!("../../../docs/design/lids/tokens.md");

#[derive(Clone)]
struct LocalWebState {
    database: LocalDatabaseState,
    active_media_sessions: Arc<tokio::sync::Mutex<BTreeSet<uuid::Uuid>>>,
}

#[derive(Clone)]
enum LocalDatabaseState {
    NotConfigured,
    DatabaseUnavailable,
    SchemaUnavailable,
    Ready(Arc<Database>),
}

impl LocalDatabaseState {
    fn database(&self) -> Option<&Database> {
        match self {
            Self::Ready(database) => Some(database),
            Self::NotConfigured | Self::DatabaseUnavailable | Self::SchemaUnavailable => None,
        }
    }

    async fn health_state(&self) -> (&'static str, &'static str, &'static str, &'static str) {
        match self {
            Self::NotConfigured => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "NOT_CONFIGURED",
                "NOT_CHECKED",
            ),
            Self::DatabaseUnavailable => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "CONFIGURED_UNAVAILABLE",
                "LOCAL_001_DATABASE_UNAVAILABLE",
            ),
            Self::SchemaUnavailable => (
                "SOURCE_INCOMPLETE",
                "NOT_CONNECTED",
                "CONFIGURED_UNAVAILABLE",
                "LOCAL_001_SCHEMA_UNAVAILABLE",
            ),
            Self::Ready(database) => match local_discovery_schema_is_ready(database).await {
                Ok(true) => match producer_runtime_schema_is_ready(database).await {
                    Ok(true) => (
                        FULL_PRODUCER_RUNTIME_DATA_STATE,
                        "MANUAL_RUNTIME_PACKAGES",
                        "READY",
                        FULL_PRODUCER_RUNTIME_SCHEMA,
                    ),
                    Ok(false) => match local_producer_schema_is_ready(database).await {
                        Ok(true) => (
                            "LOCAL_TRUSTED_PRODUCER",
                            "MANUAL_DISCOVERY_ONLY",
                            "READY",
                            "LOCAL_003_SCHEMA_READY",
                        ),
                        Ok(false) => (
                            "LOCAL_DISCOVERY_READ_PROJECTION",
                            "DISCOVERY_ONLY",
                            "READY",
                            "LOCAL_001_SCHEMA_READY",
                        ),
                        Err(_) => (
                            "SOURCE_INCOMPLETE",
                            "NOT_CONNECTED",
                            "CONFIGURED_UNAVAILABLE",
                            "LOCAL_003_DATABASE_UNAVAILABLE",
                        ),
                    },
                    Err(_) => (
                        "SOURCE_INCOMPLETE",
                        "NOT_CONNECTED",
                        "CONFIGURED_UNAVAILABLE",
                        "PLUGIN_RUNTIME_001_DATABASE_UNAVAILABLE",
                    ),
                },
                Ok(false) => (
                    "SOURCE_INCOMPLETE",
                    "NOT_CONNECTED",
                    "CONFIGURED_UNAVAILABLE",
                    "LOCAL_001_SCHEMA_UNAVAILABLE",
                ),
                Err(_) => (
                    "SOURCE_INCOMPLETE",
                    "NOT_CONNECTED",
                    "CONFIGURED_UNAVAILABLE",
                    "LOCAL_001_DATABASE_UNAVAILABLE",
                ),
            },
        }
    }
}

#[derive(Debug, Deserialize)]
struct EvidenceLibraryParams {
    q: Option<String>,
    window: Option<String>,
}

#[cfg(test)]
fn app() -> Router {
    router(LocalWebState {
        database: LocalDatabaseState::NotConfigured,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
    })
}

#[cfg(test)]
fn app_with_database(database: Database) -> Router {
    router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(database)),
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
    })
}

fn router(state: LocalWebState) -> Router {
    Router::new()
        .route("/", get(local_entry))
        .route("/health", get(health))
        .route("/api/local/discovery-packages", post(discovery_ingress))
        .route(
            "/api/local/producer/manual-tasks",
            post(create_manual_task_route),
        )
        .route(
            LOCAL_PRODUCER_TASK_CREATION_PATH,
            post(create_producer_task_route),
        )
        .route(
            "/api/local/producer/attempts",
            post(start_local_attempt_route),
        )
        .route(
            "/api/local/producer/submissions",
            post(submit_local_package_route),
        )
        .route(
            LOCAL_PRODUCER_ATTEMPT_START_PATH,
            post(start_producer_attempt_route),
        )
        .route(
            LOCAL_PRODUCER_SUBMISSION_PATH,
            post(submit_producer_package_route),
        )
        .route(
            "/api/local/producer/media-observations/{observation_ref}/uploads",
            post(start_media_upload_route),
        )
        .route(
            "/api/local/producer/media-observations/{observation_ref}/download-failures",
            post(record_media_download_failure_route),
        )
        .route(
            "/api/local/producer/media-uploads/{session_ref}/chunks",
            axum::routing::patch(append_media_upload_chunk_route),
        )
        .route(
            "/api/local/producer/media-uploads/{session_ref}/finalize",
            post(finalize_media_upload_route),
        )
        .route(
            "/api/local/media/{sha256}",
            get(read_local_media_blob_route),
        )
        .route("/api/local/evidence-library", get(evidence_library_json))
        .route("/corpus/evidence", get(evidence_library))
        .route("/collection", get(collection_entry))
        .route("/collection/targets", get(collection_targets))
        .route("/collection/operations", get(collection_operations))
        .route("/collection/attention", get(collection_attention))
        .route("/collection/tasks", get(collection_tasks))
        .route("/collection/runtime", get(collection_runtime))
        .route("/assets/evidence-library.css", get(stylesheet))
        .route(
            "/assets/collection-workspace.css",
            get(collection_stylesheet),
        )
        .route("/assets/collection-workspace.js", get(collection_script))
        .with_state(state)
}

async fn local_entry() -> Redirect {
    Redirect::temporary("/corpus/evidence")
}

pub async fn serve() -> Result<(), std::io::Error> {
    let application = router(LocalWebState {
        database: configured_database_state().await,
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
    });
    let local_port = configured_local_port()?;
    let address = SocketAddr::from((LOCAL_HOST, local_port));
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Linggan local host listening on http://localhost:{local_port}");
    axum::serve(listener, application).await
}

fn configured_local_port() -> Result<u16, std::io::Error> {
    match std::env::var("LINGGAN_LOCAL_PORT") {
        Ok(value) => match value.parse::<u16>() {
            Ok(port) if port > 0 => Ok(port),
            _ => Err(Error::new(
                ErrorKind::InvalidInput,
                "LINGGAN_LOCAL_PORT must be a valid non-zero loopback TCP port",
            )),
        },
        Err(_) => Ok(LOCAL_PORT),
    }
}

async fn health(State(state): State<LocalWebState>) -> Json<Value> {
    let (data_state, evidence_read_model, database_state, schema_state) =
        state.database.health_state().await;
    let local_producer_routes = if data_state == FULL_PRODUCER_RUNTIME_DATA_STATE
        && database_state == "READY"
        && schema_state == FULL_PRODUCER_RUNTIME_SCHEMA
    {
        json!({
            "taskCreation": LOCAL_PRODUCER_TASK_CREATION_PATH,
            "attemptStart": LOCAL_PRODUCER_ATTEMPT_START_PATH,
            "submission": LOCAL_PRODUCER_SUBMISSION_PATH
        })
    } else {
        Value::Null
    };
    Json(json!({
        "service": "linggan-local-web",
        "listener": "loopback-only",
        "dataState": data_state,
        "evidenceReadModel": evidence_read_model,
        "database": {
            "state": database_state,
            "schema": schema_state
        },
        "routes": {
            "evidenceLibrary": "/corpus/evidence",
            "discoveryIngress": "/api/local/discovery-packages",
            "localProducer": local_producer_routes
        }
    }))
}

async fn evidence_library(
    State(state): State<LocalWebState>,
    Query(params): Query<EvidenceLibraryParams>,
) -> Html<String> {
    match state.database.database() {
        None => Html(evidence_library_html()),
        Some(database) => match local_query(&params) {
            Ok(query) => match read_evidence_library(database, &query).await {
                Ok(projection) => Html(evidence_page::render_read_projection(
                    &evidence_library_html(),
                    &projection,
                    params.q.as_deref(),
                )),
                Err(_) => Html(evidence_read_unavailable_html()),
            },
            Err(()) => Html(evidence_query_invalid_html()),
        },
    }
}

async fn evidence_library_json(
    State(state): State<LocalWebState>,
    Query(params): Query<EvidenceLibraryParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(query) = local_query(&params) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_local_evidence_query",
        );
    };
    match read_evidence_library(database, &query).await {
        Ok(projection) => Json(projection).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_projection_unavailable",
        ),
    }
}

async fn read_evidence_library(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<linggan_evidence::DiscoveryLibraryProjection, sqlx::Error> {
    if producer_runtime_schema_is_ready(database).await?
        && producer_runtime_has_packages(database).await?
    {
        read_runtime_library(database, query).await
    } else {
        read_discovery_library(database, query).await
    }
}

async fn discovery_ingress(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return ingress_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "ingress_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return ingress_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "discovery_contract_invalid",
        );
    };
    match ingest_discovery_package(database, body).await {
        Ok(outcome) => Json(outcome).into_response(),
        Err(DiscoveryIngressError::Contract(_)) => ingress_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "discovery_contract_invalid",
        ),
        Err(DiscoveryIngressError::Internal(_)) => ingress_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "ingress_not_committed",
        ),
    }
}

async fn create_manual_task_route(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        );
    };
    let Ok(task) = parse_local_task_spec(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        );
    };
    match create_manual_task(database, &task).await {
        Ok(LocalTaskOutcome::Conflict { .. }) => {
            local_producer_error(axum::http::StatusCode::CONFLICT, "task_spec_conflict")
        }
        Ok(outcome) => Json(outcome).into_response(),
        Err(LocalProducerError::Internal(_)) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_committed",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        ),
    }
}

async fn create_producer_task_route(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        );
    };
    let Ok(task) = parse_producer_task_spec(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        );
    };
    match create_producer_task(database, &task).await {
        Ok(RuntimeTaskOutcome::Conflict { .. }) => {
            local_producer_error(axum::http::StatusCode::CONFLICT, "task_spec_conflict")
        }
        Ok(outcome) => Json(outcome).into_response(),
        Err(ProducerRuntimeError::Internal(_)) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_committed",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "task_spec_invalid",
        ),
    }
}

async fn start_local_attempt_route(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        );
    };
    let Ok(attempt) = parse_local_producer_attempt(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        );
    };
    match start_local_attempt(database, &attempt).await {
        Ok(LocalAttemptOutcome::Conflict { .. }) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_identity_conflict",
        ),
        Ok(outcome) => Json(outcome).into_response(),
        Err(LocalProducerError::RoutingNotFound) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "task_not_found")
        }
        Err(LocalProducerError::Internal(_)) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_committed",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        ),
    }
}

async fn submit_local_package_route(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "submission_invalid",
        );
    };
    let Ok(submission) = parse_local_producer_submission(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "submission_invalid",
        );
    };
    match submit_local_package(database, &submission).await {
        Ok(LocalSubmissionOutcome::Conflict { .. }) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_terminal_submission_conflict",
        ),
        Ok(outcome) => Json(outcome).into_response(),
        Err(LocalProducerError::RoutingNotFound) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "attempt_not_found")
        }
        Err(LocalProducerError::AttemptIdentityMismatch) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_identity_mismatch",
        ),
        Err(LocalProducerError::DiscoveryContract | LocalProducerError::Contract(_)) => {
            local_producer_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "submission_invalid",
            )
        }
        Err(LocalProducerError::DiscoveryIngress(_) | LocalProducerError::Internal(_)) => {
            local_producer_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "submission_not_acknowledged",
            )
        }
    }
}

async fn start_producer_attempt_route(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        );
    };
    let Ok(attempt) = parse_producer_attempt(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        );
    };
    match start_producer_attempt(database, &attempt).await {
        Ok(RuntimeAttemptOutcome::Conflict { .. }) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_identity_conflict",
        ),
        Ok(outcome) => Json(outcome).into_response(),
        Err(ProducerRuntimeError::RoutingNotFound) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "task_not_found")
        }
        Err(ProducerRuntimeError::Internal(_)) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_committed",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "attempt_invalid",
        ),
    }
}

async fn submit_producer_package_route(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(body) = std::str::from_utf8(&body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "submission_invalid",
        );
    };
    let Ok(submission) = parse_producer_submission(body) else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "submission_invalid",
        );
    };
    match submit_producer_package(database, &submission).await {
        Ok(RuntimeSubmissionOutcome::Conflict { .. }) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_terminal_submission_conflict",
        ),
        Ok(outcome) => Json(outcome).into_response(),
        Err(ProducerRuntimeError::RoutingNotFound) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "attempt_not_found")
        }
        Err(ProducerRuntimeError::AttemptIdentityMismatch) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_identity_mismatch",
        ),
        Err(ProducerRuntimeError::Internal(_)) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "submission_not_acknowledged",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "submission_invalid",
        ),
    }
}

// These three routes are the resumable media lane.  They are intentionally separate from the
// text package route: slow bytes and a retrying download must never delay an already-captured
// note, comment, or discovery package.
async fn start_media_upload_route(
    State(state): State<LocalWebState>,
    Path(observation_ref): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(observation_ref) = uuid::Uuid::parse_str(&observation_ref) else {
        return local_producer_error(
            axum::http::StatusCode::BAD_REQUEST,
            "media_observation_invalid",
        );
    };
    let Some((expected_sha256, mime_type, expected_byte_size)) = media_upload_headers(&headers)
    else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "media_upload_contract_invalid",
        );
    };
    let temporary_storage_key = format!("uploads/{observation_ref}/{expected_sha256}.part");
    match begin_media_upload(
        database,
        observation_ref,
        &expected_sha256,
        &mime_type,
        expected_byte_size,
        &temporary_storage_key,
    )
    .await
    {
        Ok(session) => Json(json!({
            "sessionRef": session.session_ref,
            "nextOffset": session.next_offset,
            "state": session.state,
        }))
        .into_response(),
        Err(ProducerRuntimeError::MediaObservationNotFound) => local_producer_error(
            axum::http::StatusCode::NOT_FOUND,
            "media_observation_not_found",
        ),
        Err(ProducerRuntimeError::MediaBlobConflict) => {
            local_producer_error(axum::http::StatusCode::CONFLICT, "media_upload_conflict")
        }
        Err(_) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "media_upload_state_unavailable",
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaDownloadFailureWire {
    attempted_uri: String,
    terminal_reason: String,
}

async fn record_media_download_failure_route(
    State(state): State<LocalWebState>,
    Path(observation_ref): Path<String>,
    Json(input): Json<MediaDownloadFailureWire>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(observation_ref) = uuid::Uuid::parse_str(&observation_ref) else {
        return local_producer_error(
            axum::http::StatusCode::BAD_REQUEST,
            "media_observation_invalid",
        );
    };
    if input.attempted_uri.trim().is_empty() {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "media_download_failure_invalid",
        );
    }
    match record_media_download_failure(
        database,
        observation_ref,
        &input.attempted_uri,
        &input.terminal_reason,
    )
    .await
    {
        Ok(download_attempt_ref) => {
            Json(json!({"downloadAttemptRef": download_attempt_ref, "delivery": "acknowledged"}))
                .into_response()
        }
        Err(ProducerRuntimeError::MediaObservationNotFound) => local_producer_error(
            axum::http::StatusCode::NOT_FOUND,
            "media_observation_not_found",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "media_download_failure_not_recorded",
        ),
    }
}

async fn append_media_upload_chunk_route(
    State(state): State<LocalWebState>,
    Path(session_ref): Path<String>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(session_ref) = uuid::Uuid::parse_str(&session_ref) else {
        return local_producer_error(
            axum::http::StatusCode::BAD_REQUEST,
            "media_upload_session_invalid",
        );
    };
    let _session_guard = match acquire_media_session_guard(&state, session_ref).await {
        Some(guard) => guard,
        None => return local_producer_error(axum::http::StatusCode::CONFLICT, "media_upload_busy"),
    };
    let Some(offset) = headers
        .get("x-linggan-media-offset")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<i64>().ok())
    else {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "media_chunk_offset_invalid",
        );
    };
    let Ok(byte_count) = i64::try_from(body.len()) else {
        return local_producer_error(
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            "media_chunk_size_invalid",
        );
    };
    let session = match read_media_upload_session(database, session_ref).await {
        Ok(Some(value)) => value,
        Ok(None) => {
            return local_producer_error(
                axum::http::StatusCode::NOT_FOUND,
                "media_upload_session_not_found",
            );
        }
        Err(_) => {
            return local_producer_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "media_upload_state_unavailable",
            );
        }
    };
    if session.state == "materialized" {
        return Json(json!({"sessionRef": session_ref, "nextOffset": session.next_offset, "state": session.state})).into_response();
    }
    if session.state != "receiving" || session.next_offset != offset || byte_count < 1 {
        return local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "media_chunk_offset_conflict",
        );
    }
    let temporary_path = local_media_root().join(&session.temporary_storage_key);
    if let Err(error) = write_media_chunk(&temporary_path, offset, &body) {
        eprintln!("Linggan local media chunk write failed: {error}");
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "media_chunk_not_written",
        );
    }
    match record_media_upload_chunk(database, session_ref, offset, byte_count).await {
        Ok(updated) => Json(json!({"sessionRef": updated.session_ref, "nextOffset": updated.next_offset, "state": updated.state})).into_response(),
        Err(ProducerRuntimeError::MediaBlobConflict) => {
            let _ = truncate_media_file(&temporary_path, offset);
            local_producer_error(axum::http::StatusCode::CONFLICT, "media_chunk_offset_conflict")
        }
        Err(_) => {
            let _ = truncate_media_file(&temporary_path, offset);
            local_producer_error(axum::http::StatusCode::SERVICE_UNAVAILABLE, "media_chunk_state_not_updated")
        }
    }
}

async fn finalize_media_upload_route(
    State(state): State<LocalWebState>,
    Path(session_ref): Path<String>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    let Ok(session_ref) = uuid::Uuid::parse_str(&session_ref) else {
        return local_producer_error(
            axum::http::StatusCode::BAD_REQUEST,
            "media_upload_session_invalid",
        );
    };
    let _session_guard = match acquire_media_session_guard(&state, session_ref).await {
        Some(guard) => guard,
        None => return local_producer_error(axum::http::StatusCode::CONFLICT, "media_upload_busy"),
    };
    let session = match claim_media_upload_finalize(database, session_ref).await {
        Ok(MediaUploadFinalizeClaim::Materialized(admission)) => {
            return Json(admission).into_response();
        }
        Ok(MediaUploadFinalizeClaim::Incomplete) => {
            return local_producer_error(
                axum::http::StatusCode::CONFLICT,
                "media_upload_incomplete",
            );
        }
        Ok(MediaUploadFinalizeClaim::Busy) => {
            return local_producer_error(
                axum::http::StatusCode::CONFLICT,
                "media_upload_finalizing",
            );
        }
        Ok(MediaUploadFinalizeClaim::Ready(session)) => session,
        Err(ProducerRuntimeError::MediaObservationNotFound) => {
            return local_producer_error(
                axum::http::StatusCode::NOT_FOUND,
                "media_upload_session_not_found",
            );
        }
        Err(_) => {
            return local_producer_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "media_upload_state_unavailable",
            );
        }
    };
    finalize_claimed_media_upload(database, session_ref, session).await
}

async fn finalize_claimed_media_upload(
    database: &Database,
    session_ref: uuid::Uuid,
    session: linggan_evidence::MediaUploadSession,
) -> Response {
    let root = local_media_root();
    let temporary_path = root.join(&session.temporary_storage_key);
    let storage_key = media_storage_key(&session.expected_sha256);
    let final_path = root.join(&storage_key);
    // A crash may happen after the atomic rename and before its database receipt. Retry from a
    // verified final blob in that narrow interval instead of making `finalizing` terminal.
    let bytes_to_verify = fs::read(&temporary_path).or_else(|error| {
        if error.kind() == ErrorKind::NotFound {
            fs::read(&final_path)
        } else {
            Err(error)
        }
    });
    match bytes_to_verify {
        Ok(bytes)
            if i64::try_from(bytes.len()).ok() == Some(session.expected_byte_size)
                && sha256_bytes(&bytes) == session.expected_sha256 => {}
        _ => {
            let _ = release_media_upload_finalize(database, session_ref).await;
            return local_producer_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "media_upload_integrity_invalid",
            );
        }
    };
    let created = match atomically_promote_media_upload(&temporary_path, &final_path) {
        Ok(created) => created,
        Err(error) => {
            let _ = release_media_upload_finalize(database, session_ref).await;
            eprintln!("Linggan local media promotion failed: {error}");
            return local_producer_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "media_upload_not_materialized",
            );
        }
    };
    let admission = admit_media_blob(
        database,
        session.media_observation_ref,
        &session.expected_sha256,
        &session.mime_type,
        session.expected_byte_size,
        &storage_key,
    )
    .await;
    match admission {
        Ok(outcome) => {
            match complete_media_upload(database, session_ref, outcome.download_attempt_ref).await {
                Ok(()) => Json(outcome).into_response(),
                Err(_) => local_producer_error(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "media_upload_receipt_not_recorded",
                ),
            }
        }
        Err(error) => {
            if created {
                let _ = remove_unreferenced_media(&root, &final_path);
            }
            let _ = release_media_upload_finalize(database, session_ref).await;
            match error {
                ProducerRuntimeError::MediaObservationNotFound => local_producer_error(
                    axum::http::StatusCode::NOT_FOUND,
                    "media_observation_not_found",
                ),
                ProducerRuntimeError::MediaBlobConflict => local_producer_error(
                    axum::http::StatusCode::CONFLICT,
                    "media_blob_metadata_conflict",
                ),
                _ => local_producer_error(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "media_admission_not_committed",
                ),
            }
        }
    }
}

struct MediaSessionGuard {
    sessions: Arc<tokio::sync::Mutex<BTreeSet<uuid::Uuid>>>,
    session_ref: uuid::Uuid,
}

impl Drop for MediaSessionGuard {
    fn drop(&mut self) {
        let sessions = Arc::clone(&self.sessions);
        let session_ref = self.session_ref;
        tokio::spawn(async move {
            sessions.lock().await.remove(&session_ref);
        });
    }
}

async fn acquire_media_session_guard(
    state: &LocalWebState,
    session_ref: uuid::Uuid,
) -> Option<MediaSessionGuard> {
    let mut sessions = state.active_media_sessions.lock().await;
    if !sessions.insert(session_ref) {
        return None;
    }
    Some(MediaSessionGuard {
        sessions: Arc::clone(&state.active_media_sessions),
        session_ref,
    })
}

async fn read_local_media_blob_route(
    State(state): State<LocalWebState>,
    Path(sha256): Path<String>,
) -> Response {
    if !is_sha256(&sha256) {
        return local_producer_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found");
    }
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    match read_local_media_blob(database, &sha256).await {
        Ok(Some((mime_type, storage_key))) => {
            match fs::read(local_media_root().join(storage_key)) {
                Ok(bytes) => {
                    let Ok(content_type) = HeaderValue::from_str(&mime_type) else {
                        return local_producer_error(
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "local_media_metadata_invalid",
                        );
                    };
                    (
                        [
                            (header::CONTENT_TYPE, content_type),
                            (
                                header::CACHE_CONTROL,
                                HeaderValue::from_static("private, max-age=31536000, immutable"),
                            ),
                        ],
                        bytes,
                    )
                        .into_response()
                }
                Err(_) => local_producer_error(
                    axum::http::StatusCode::NOT_FOUND,
                    "local_media_bytes_unavailable",
                ),
            }
        }
        Ok(None) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found")
        }
        Err(_) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "local_media_metadata_unavailable",
        ),
    }
}

fn local_media_root() -> PathBuf {
    std::env::var("LINGGAN_LOCAL_MEDIA_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".linggan-local/media"))
}

fn media_upload_headers(headers: &axum::http::HeaderMap) -> Option<(String, String, i64)> {
    let expected_sha256 = headers
        .get("x-linggan-media-sha256")?
        .to_str()
        .ok()?
        .trim()
        .to_ascii_lowercase();
    let mime_type = headers
        .get("x-linggan-media-mime")?
        .to_str()
        .ok()?
        .trim()
        .to_owned();
    let expected_byte_size = headers
        .get("x-linggan-media-size")?
        .to_str()
        .ok()?
        .parse::<i64>()
        .ok()?;
    if !is_sha256(&expected_sha256)
        || !(1..=256 * 1024 * 1024).contains(&expected_byte_size)
        || (!mime_type.starts_with("image/")
            && !mime_type.starts_with("video/")
            && !mime_type.starts_with("audio/"))
    {
        return None;
    }
    Some((expected_sha256, mime_type, expected_byte_size))
}

fn media_storage_key(sha256: &str) -> String {
    format!("blobs/{}/{}", &sha256[..2], sha256)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

fn write_media_chunk(path: &FsPath, offset: i64, bytes: &[u8]) -> Result<(), std::io::Error> {
    let offset = u64::try_from(offset)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "media_chunk_offset_invalid"))?;
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "media_upload_temp_path_invalid"))?;
    fs::create_dir_all(parent)?;
    let existing_size = fs::metadata(path)
        .map(|value| value.len())
        .or_else(|error| {
            if error.kind() == ErrorKind::NotFound {
                Ok(0)
            } else {
                Err(error)
            }
        })?;
    if existing_size != offset {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "media_chunk_file_offset_conflict",
        ));
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    file.write_all(bytes)?;
    file.sync_data()?;
    Ok(())
}

fn truncate_media_file(path: &FsPath, offset: i64) -> Result<(), std::io::Error> {
    if !path.exists() {
        return Ok(());
    }
    let offset = u64::try_from(offset)
        .map_err(|_| Error::new(ErrorKind::InvalidInput, "media_chunk_offset_invalid"))?;
    let file = std::fs::OpenOptions::new().write(true).open(path)?;
    file.set_len(offset)
}

fn atomically_promote_media_upload(
    temporary_path: &FsPath,
    final_path: &FsPath,
) -> Result<bool, std::io::Error> {
    if final_path.exists() {
        let _ = fs::remove_file(temporary_path);
        return Ok(false);
    }
    let parent = final_path
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "media_storage_path_invalid"))?;
    fs::create_dir_all(parent)?;
    match fs::rename(temporary_path, final_path) {
        Ok(()) => Ok(true),
        Err(_error) if final_path.exists() => {
            let _ = fs::remove_file(temporary_path);
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

fn remove_unreferenced_media(root: &FsPath, path: &FsPath) -> Result<(), std::io::Error> {
    if path.starts_with(root) && path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

async fn configured_database_state() -> LocalDatabaseState {
    let Ok(url) = std::env::var("LINGGAN_LOCAL_DATABASE_URL") else {
        return LocalDatabaseState::NotConfigured;
    };
    let database = match Database::connect(&url).await {
        Ok(database) => database,
        Err(_) => return LocalDatabaseState::DatabaseUnavailable,
    };
    if local_discovery_schema_is_ready(&database)
        .await
        .unwrap_or(false)
    {
        LocalDatabaseState::Ready(Arc::new(database))
    } else {
        LocalDatabaseState::SchemaUnavailable
    }
}

fn local_query(params: &EvidenceLibraryParams) -> Result<EvidenceQuery, ()> {
    let window = match params.window.as_deref().unwrap_or("last_30_days") {
        "last_7_days" => "last_7_days",
        "last_30_days" => "last_30_days",
        _ => return Err(()),
    };
    serde_json::from_value(json!({
        "text": params.q,
        "scope": "all_accepted_material",
        "window": window,
        "sort": "latest_discovery"
    }))
    .map_err(|_| ())
}

fn ingress_json_error(status: axum::http::StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({ "admission": "not_accepted", "code": code })),
    )
        .into_response()
}

fn local_producer_error(status: axum::http::StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({ "delivery": "not_acknowledged", "code": code })),
    )
        .into_response()
}

fn local_read_json_error(status: axum::http::StatusCode, code: &'static str) -> Response {
    (
        status,
        Json(json!({ "operation": "local_read", "outcome": "unavailable", "code": code })),
    )
        .into_response()
}

fn evidence_read_unavailable_html() -> String {
    evidence_library_html()
        .replace("SOURCE_INCOMPLETE", "READ_PROJECTION_UNAVAILABLE")
        .replace(
            "页面还没有连接到受控的本地材料读投影",
            "页面无法从受控本地读投影读取卡片；没有显示任何旧系统或远程数据",
        )
        .to_owned()
}

fn evidence_query_invalid_html() -> String {
    evidence_library_html()
        .replace("SOURCE_INCOMPLETE", "LOCAL_QUERY_INVALID")
        .replace(
            "页面还没有连接到受控的本地材料读投影",
            "当前只接受本地 EvidenceQuery；没有触发平台搜索或补采",
        )
        .to_owned()
}

async fn stylesheet() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        format!("{LIDS_TOKENS}\n{SHELL_CSS}\n{EVIDENCE_LIBRARY_CSS}"),
    )
        .into_response()
}

/// Collection sub-surface handlers. The section is part of the path and the Operations mode
/// is a query parameter, so every view is a real, shareable, refresh-safe address.
#[derive(Deserialize)]
struct CollectionParams {
    mode: Option<String>,
    drawer: Option<String>,
}

/// DESIGN-006: the entry lands on the one surface whose contents expire. Arriving on the
/// target list meant opening with the most static thing in Collection — a list that does not
/// change for a week — while anything actually waiting sat two tabs away.
async fn collection_entry() -> Redirect {
    Redirect::temporary("/collection/attention")
}

async fn collection_targets(Query(params): Query<CollectionParams>) -> Html<String> {
    Html(collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        params.drawer.as_deref(),
    ))
}

async fn collection_operations(Query(params): Query<CollectionParams>) -> Html<String> {
    Html(collection::render(
        collection::Section::Operations,
        collection::OperationsMode::parse(params.mode.as_deref()),
        None,
    ))
}

async fn collection_attention() -> Html<String> {
    Html(collection::render(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
    ))
}

async fn collection_tasks() -> Html<String> {
    Html(collection::render(
        collection::Section::Tasks,
        collection::OperationsMode::Now,
        None,
    ))
}

async fn collection_runtime() -> Html<String> {
    Html(collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
    ))
}

async fn collection_stylesheet() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        format!("{LIDS_TOKENS}\n{SHELL_CSS}\n{COLLECTION_WORKSPACE_CSS}"),
    )
        .into_response()
}

async fn collection_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/javascript; charset=utf-8"),
        )],
        COLLECTION_WORKSPACE_JS,
    )
        .into_response()
}

fn evidence_library_html() -> String {
    let base = r#"<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>Evidence Library · Linggan Intelligence</title>
    <link rel="stylesheet" href="/assets/evidence-library.css">
  </head>
  <body>
    <div class="v7-app">
      <!-- GLOBAL_HEADER_START --><!-- GLOBAL_HEADER_END -->

      <div class="v7-shell">
        <aside class="v7-side" aria-label="语料导航">
          <div class="v7-nav-label" data-readout="CORPUS">语料与证据</div>
          <button class="v7-side-nav" disabled aria-disabled="true" aria-current="page"><i>01</i><span>证据库</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>02</i><span>评论</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>03</i><span>创作者</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>04</i><span>已存查询</span></button>
          <button class="v7-side-nav" disabled aria-disabled="true"><i>05</i><span>来源</span></button>
          <div class="v7-side-foot"><span class="v7-side-dot"></span>SOURCE INCOMPLETE<br><span class="v7-mono">local presentation only · no material read</span></div>
        </aside>

        <main class="v7-main" aria-labelledby="page-title">
          <section class="v7-page-header">
            <h1 class="v7-sr-only" id="page-title">证据库</h1>
            <form class="v7-search-row" method="get"><label class="v7-search"><span class="v7-cmd">⌘ FIND</span><!-- EVIDENCE_SEARCH_INPUT_START --><input disabled aria-disabled="true" placeholder="等待受控材料读投影接通……"><!-- EVIDENCE_SEARCH_INPUT_END --><kbd>⌘ K</kbd></label><details class="v7-dd" aria-label="搜索范围"><summary><em>类型</em><strong>全部</strong><i aria-hidden="true">▾</i></summary><div class="v7-dd-menu"><button type="button" disabled aria-current="true">全部</button><button type="button" disabled>作品</button><button type="button" disabled>评论</button><button type="button" disabled>转录</button><button type="button" disabled>作者</button></div></details><div class="v7-page-actions"><button class="v7-btn" disabled aria-disabled="true">复制查询</button><button class="v7-btn" disabled aria-disabled="true">保存当前视图</button><button class="v7-btn v7-primary" disabled aria-disabled="true">发起研究</button></div></form>
            <div class="v7-views-bar">
              <div class="v7-view-group"><span class="v7-view-label">系统视图 <em>SYSTEM VIEWS</em></span><div class="v7-view-strip"><button class="v7-view-pill" disabled aria-current="true"><i>01</i><span>最新发现</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>02</i><span>待补采</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>03</i><span>高互动</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>04</i><span>评论密集</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>05</i><span>最近异常</span><span class="v7-n">—</span></button><button class="v7-view-pill v7-more" disabled><i>+</i><span>更多</span><span class="v7-n">⌄</span></button></div></div>
              <div class="v7-view-group v7-my"><span class="v7-view-label">我的视图 <em>MY VIEWS</em></span><div class="v7-view-strip"><button class="v7-view-pill" disabled><i>A</i><span>ADHD 作业</span></button><button class="v7-view-pill" disabled><i>B</i><span>低粉爆文</span></button><button class="v7-view-pill" disabled><i>C</i><span>家长原声研究</span></button></div></div>
            </div>
            <div class="v7-controls-row"><div class="v7-filters"><span class="v7-filter-lead">筛选 <em>FILTER</em></span><button class="v7-chip" disabled><em>平台</em> ALL</button><button class="v7-chip" disabled><em>窗口</em> UNKNOWN</button><button class="v7-chip" disabled><em>类型</em> ALL</button><button class="v7-chip" disabled><em>来源</em> UNKNOWN</button><button class="v7-chip" disabled><em>状态</em> UNKNOWN</button></div><div class="v7-controls"><button class="v7-control" disabled><small>分组</small><strong>不分组</strong></button><button class="v7-control" disabled><small>排序</small><strong>UNKNOWN</strong></button><button class="v7-control" disabled><small>密度</small><strong>标准</strong></button><div class="v7-seg"><span class="v7-seg-label">视图</span><button disabled aria-current="true">研究</button><button disabled>表格</button><button disabled>封面</button></div></div></div>
            <div class="v7-query-line"><div>页面结构已就绪 · 材料读投影尚未接通</div><div><b>SOURCE_INCOMPLETE</b> · <span>NO QUERY AVAILABLE</span></div></div>
          </section>

          <section class="v7-workspace" aria-label="Evidence Library 工作区">
            <section class="v7-results" aria-label="事实材料列表">
              <div class="v7-fact-strap"><span>FACT LAYER / EVIDENCE</span><i aria-hidden="true"></i><b>原始内容资产</b></div>
              <div class="v7-results-head"><div class="v7-results-left"><input class="v7-check" type="checkbox" disabled aria-label="选择全部材料"><span>NO ACCEPTED MATERIAL AVAILABLE</span></div><div>READ MODEL NOT CONNECTED</div></div>
              <!-- EVIDENCE_RESULTS_START --><div class="v7-results-empty">
                <article class="v7-empty-row"><input class="v7-check" type="checkbox" disabled aria-label="无材料"><div class="v7-empty-mark">?</div><div class="v7-empty-main"><div class="v7-empty-title">SOURCE_INCOMPLETE</div><div class="v7-empty-copy">页面还没有连接到受控的本地材料读投影，因此不能列出 Content、评论、转录或来源对象。</div><div class="v7-empty-boundary"><b>NO_ACCEPTED_MATERIAL_AVAILABLE</b>这不是世界中不存在内容，也不是库内数量为零。</div><div class="v7-empty-facts"><span>TRUTH <strong>UNKNOWN</strong></span><span>COVERAGE <strong>UNKNOWN</strong></span><span>DISPLAY <strong>NOT CONNECTED</strong></span></div></div><div class="v7-empty-metric"><div><b>—</b><span>ITEMS</span></div><div><b>—</b><span>OBS</span></div><div><b>—</b><span>SOURCE</span></div></div></article>
                <section class="v7-empty-panel" aria-labelledby="empty-title"><h2 id="empty-title">没有可展示的本地材料</h2><p>当前本地 host 只提供此页面的视觉和信息边界；它没有读取数据库、历史内容工作台或插件结果。</p><dl class="v7-empty-grid"><div><dt>现在知道什么</dt><dd>页面可被本地 host 提供；材料读取合同未接通。</dd></div><div><dt>现在不知道什么</dt><dd>材料、来源、观察时间、Capture 与 Coverage 均为未知。</dd></div><div><dt>下一步</dt><dd>001B 另立范围后才能建立受控只读投影。</dd></div></dl></section>
              </div><!-- EVIDENCE_RESULTS_END -->
            </section>
            <aside class="v7-inspect" aria-labelledby="inspector-title">
              <div class="v7-inspector-head"><div class="v7-inspector-identity"><div class="v7-iid">#NO_SELECTION</div><h2 class="v7-ititle" id="inspector-title">尚未选择材料</h2><div class="v7-imeta"><span>CONTENT ITEM UNKNOWN</span><span>·</span><span>OBSERVATION UNKNOWN</span><span>·</span><span>CAPTURE UNKNOWN</span></div></div><div class="v7-inspector-ops"><div class="v7-inspector-primary"><button disabled aria-disabled="true">↗ 原文</button><button disabled aria-disabled="true">⟳ 补采</button><button disabled aria-disabled="true">＋ 研究</button></div><div class="v7-inspector-window"><button disabled aria-disabled="true">PIN</button><button disabled aria-disabled="true">WIDE</button><button disabled aria-disabled="true">×</button></div></div><div class="v7-tabs" aria-label="材料详情页签"><button disabled aria-current="page">概览</button><button disabled>正文</button><button disabled>评论</button><button disabled>历史</button><button disabled>来源</button><button disabled>关系</button></div></div>
              <div class="v7-inspector-body"><section class="v7-section"><h3>原始内容 <em>RAW CONTENT</em> <span>UNKNOWN</span></h3><div class="v7-body-copy">没有选中 ContentItem，也没有可显示的受限材料。此处不能推断标题、正文、作者或平台状态。</div></section><section class="v7-section"><h3>最强命中 <em>STRONGEST MATCH</em> <span>NO MATERIAL</span></h3><div class="v7-quote">没有材料可供匹配或引用。<small>不显示示例原文、评论或转录。</small></div></section><section class="v7-section"><h3>平台与本地事实 <em>PLATFORM / LOCAL FACTS</em> <span>UNKNOWN</span></h3><div class="v7-readout"><div><b>—</b><span>PLATFORM LIKES</span></div><div><b>—</b><span>PLATFORM COMMENTS</span></div><div><b>—</b><span>LOCAL COMMENTS</span></div><div><b>—</b><span>OBSERVATIONS</span></div></div></section><section class="v7-section"><h3>状态矩阵 <em>STATE MATRIX</em></h3><div class="v7-matrix"><div class="v7-matrix-row"><div class="v7-k">事实来源</div><div class="v7-v"><i class="v7-dot"></i>SOURCE_INCOMPLETE</div></div><div class="v7-matrix-row"><div class="v7-k">读投影</div><div class="v7-v"><i class="v7-dot"></i>NOT CONNECTED</div></div><div class="v7-matrix-row"><div class="v7-k">覆盖</div><div class="v7-v"><i class="v7-dot"></i>UNKNOWN</div></div></div></section></div>
            </aside>
          </section>
        </main>
      </div>
    </div>
  </body>
</html>"#;
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "LOCAL HOST / NO READ MODEL",
        "语料 <span class=\"v7-slash\">/</span> <b>证据库</b> <span class=\"v7-slash\">/</span> <span class=\"v7-context-current\">材料状态</span>",
        "<span class=\"v7-kpi\"><em>内容</em><b>UNKNOWN</b></span><span class=\"v7-kpi\"><em>评论</em><b>UNKNOWN</b></span><span class=\"v7-kpi\"><em>创作者</em><b>UNKNOWN</b></span><i class=\"v7-vr\" aria-hidden=\"true\"></i><span class=\"v7-query-meta\">READ MODEL NOT CONNECTED</span><span>SOURCE INCOMPLETE</span><span>UTC+08</span>",
    );
    base.replace(
        "<!-- GLOBAL_HEADER_START --><!-- GLOBAL_HEADER_END -->",
        &header,
    )
}

#[cfg(test)]
mod tests;
