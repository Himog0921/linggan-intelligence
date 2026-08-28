mod collection;
mod collection_intake;
mod collection_targets_view;
mod evidence_page;
#[cfg(test)]
mod material_cursor_tests;
mod material_projection;
#[cfg(test)]
mod material_projection_media_fixture;
#[cfg(test)]
mod material_projection_tests;
mod shell;
mod station_view;
mod target_drawer;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use linggan_contracts::{
    EvidenceQuery, LifecycleState, parse_local_producer_attempt, parse_local_producer_submission,
    parse_local_task_spec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::{
    AcquisitionChainError, AuthorizationGrant, CheckInOutcome, DiscoveryIngressError,
    DispatchDecision, InstallationCheckIn, LeaseError, LocalAttemptOutcome, LocalProducerError,
    LocalSubmissionOutcome, LocalTaskOutcome, MaterialReadError, MediaUploadFinalizeClaim,
    ProducerRuntimeError, RuntimeAttemptOutcome, RuntimeSubmissionOutcome, RuntimeTaskOutcome,
    StoreOutcome, admit_media_blob, begin_media_upload, check_in_installation, claim_installation,
    claim_media_upload_finalize, close_claim_window, complete_lease_for_task,
    complete_media_upload, count_targets, create_manual_task, create_producer_task,
    decide_dispatch, dispatch_schema_is_ready, enrich_target_from_author_profile,
    grant_authorization, ingest_discovery_package, issue_work_order_lease, list_targets,
    list_targets_in_state, local_discovery_schema_is_ready, local_producer_schema_is_ready,
    open_claim_window, producer_runtime_has_packages, producer_runtime_schema_is_ready,
    read_archive_completeness, read_discovery_library, read_local_media_blob,
    read_media_upload_session, read_runtime_library, read_station_overview, read_target,
    record_media_download_failure, record_media_upload_chunk, register_station,
    release_media_upload_finalize, request_and_admit, retire_station, set_group_for_many,
    set_monitoring_for_many, set_target_monitoring, start_local_attempt, start_producer_attempt,
    station_schema_is_ready, store_pending_target, submit_local_package, submit_producer_package,
    target_monitoring_enabled,
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

/// COLLECTION-001 的写入面：观察目标、四段授权链、工位与租约。
///
/// 与主路由表分开，因为它们共享一条纪律——每一个都只写本机记录，没有一个会访问平台。
fn collection_api_routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/collection/targets",
            get(collection_targets_json).post(collection_target_intake),
        )
        .route(
            "/api/local/collection/authorizations",
            post(collection_grant_authorization),
        )
        .route(
            "/api/local/collection/archive-requests",
            post(collection_archive_request),
        )
        .route(
            "/api/local/collection/work-order-leases",
            post(collection_issue_lease),
        )
        .route("/api/local/stations", post(station_register))
        .route(
            "/api/local/stations/claim-window",
            post(station_claim_window),
        )
        .route(STATION_CHECK_IN_PATH, post(station_check_in))
        .route("/api/local/stations/claims", post(station_claim))
        .route(DISPATCH_CLAIM_PATH, post(dispatch_claim))
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
        .merge(collection_api_routes())
        .route("/corpus", get(corpus_entry))
        .route("/corpus/evidence", get(evidence_library))
        .route("/collection", get(collection_entry))
        .route("/collection/targets", get(collection_targets))
        .route("/collection/targets/new", post(collection_target_create))
        .route(
            "/collection/targets/monitoring",
            post(collection_target_toggle_monitoring),
        )
        .route(
            "/collection/targets/archive",
            post(collection_target_deep_archive),
        )
        .route("/collection/targets/batch", post(collection_targets_batch))
        .route("/collection/operations", get(collection_operations))
        .route("/collection/attention", get(collection_attention))
        .route("/collection/tasks", get(collection_tasks))
        .route("/collection/runtime", get(collection_runtime))
        .route(
            "/collection/runtime/stations",
            post(collection_runtime_register_station),
        )
        .route(
            "/collection/runtime/claim-window",
            post(collection_runtime_open_window),
        )
        .route(
            "/collection/runtime/close-window",
            post(collection_runtime_close_window),
        )
        .route(
            "/collection/runtime/retire",
            post(collection_runtime_retire_station),
        )
        .route("/collection/runtime/claims", post(collection_runtime_claim))
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

/// Each primary responsibility owns an entry route, and that route is the single place its
/// default surface is decided. The global header links here rather than to a sub-surface: a
/// header that names a sub-surface is a second copy of that decision, and the two drift the
/// first time the default moves.
async fn corpus_entry() -> Redirect {
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
    // The plugin learns where to check in from /health rather than hardcoding a path, the
    // same way it already learns the producer routes. A route is advertised only once its
    // schema is applied: advertising it earlier would invite a call that cannot succeed.
    let station_routes = match state.database.database() {
        Some(database) if station_schema_is_ready(database).await.unwrap_or(false) => json!({
            "checkIn": STATION_CHECK_IN_PATH
        }),
        _ => Value::Null,
    };
    // 派发路由同样只在其 schema 就绪后通告。通告它不等于闸门开着——闸门是另一回事，
    // 由 claim 的回答给出。
    let dispatch_routes = match state.database.database() {
        Some(database) if dispatch_schema_is_ready(database).await.unwrap_or(false) => json!({
            "claim": DISPATCH_CLAIM_PATH
        }),
        _ => Value::Null,
    };
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
            "localProducer": local_producer_routes,
            "station": station_routes,
            "dispatch": dispatch_routes
        }
    }))
}

async fn evidence_library(
    State(state): State<LocalWebState>,
    Query(params): Query<material_projection::EvidenceLibraryParams>,
) -> Html<String> {
    match state.database.database() {
        None => Html(evidence_library_html()),
        Some(database) => match material_projection::local_query(&params) {
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
    Query(params): Query<material_projection::EvidenceLibraryParams>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(query) = material_projection::local_query(&params) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_local_evidence_query",
        );
    };
    match read_evidence_library(database, &query).await {
        Ok(projection) => {
            match material_projection::compose_json(database, &query, projection).await {
                Ok(response) => Json(response).into_response(),
                Err(MaterialReadError::InvalidCursor | MaterialReadError::UnsupportedSort) => {
                    local_read_json_error(
                        axum::http::StatusCode::BAD_REQUEST,
                        "invalid_material_query_cursor_or_sort",
                    )
                }
                Err(MaterialReadError::Database(error)) => {
                    eprintln!("material read projection unavailable: {error}");
                    local_read_json_error(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "material_read_projection_unavailable",
                    )
                }
            }
        }
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_projection_unavailable",
        ),
    }
}

/// COLLECTION-001 · accept an observation target from the browser.
///
/// No Acquisition Authorization is required here, and that is deliberate: storing identity
/// consumes no platform access. Deep archiving — going back to read 200 works — is what needs
/// authorising, and that route does not exist yet.
async fn collection_target_intake(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(intake) = serde_json::from_slice::<collection_intake::TargetIntake>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "target_intake_invalid",
        );
    };
    let identity = match collection_intake::parse_intake(&intake) {
        Ok(identity) => identity,
        Err(rejection) => {
            return local_read_json_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                rejection.code(),
            );
        }
    };

    match store_pending_target(
        database,
        &identity,
        collection_intake::INTAKE_SOURCE,
        intake.display_name.as_deref(),
        intake.identity_facts.as_ref(),
    )
    .await
    {
        Ok((target, outcome)) => Json(serde_json::json!({
            // Two pushes of the same creator is a normal outcome, not a failure — but the
            // caller must be able to tell which happened rather than assume it created one.
            "outcome": match outcome {
                StoreOutcome::Stored => "stored",
                StoreOutcome::AlreadyPresent => "already_present",
            },
            "targetRef": target.target_ref,
            "lifecycleState": target.lifecycle_state,
            // Storing proves storage. Nothing here has been archived or authorised.
            "acquisition": "NOT_AUTHORISED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "collection_targets_unavailable",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GrantBody {
    target_kind: String,
    #[serde(default)]
    lane: Option<String>,
    purpose: String,
    #[serde(default)]
    max_targets: Option<i32>,
    #[serde(default)]
    max_works_per_target: Option<i32>,
    valid_for_days: i32,
}

/// COLLECTION-001 · a person grants an acquisition authorization.
///
/// Only a person may grant (contract §2). The route exists so that granting is an explicit,
/// recorded act rather than a config file nobody reviews.
async fn collection_grant_authorization(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(grant) = serde_json::from_slice::<GrantBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "authorization_grant_invalid",
        );
    };
    let lane = grant.lane.as_deref().unwrap_or("deep_archive");
    match grant_authorization(
        database,
        &AuthorizationGrant {
            platform: linggan_contracts::OPEN_PLATFORM,
            target_kind: &grant.target_kind,
            lane,
            purpose: &grant.purpose,
            max_targets: grant.max_targets,
            max_works_per_target: grant.max_works_per_target,
            valid_for_days: grant.valid_for_days,
        },
    )
    .await
    {
        Ok(authorization_ref) => Json(serde_json::json!({
            "authorizationRef": authorization_ref,
            "grantedBy": "person",
            // A grant permits; it does not schedule, queue or execute anything.
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "authorization_grant_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StationBody {
    display_name: String,
    #[serde(default)]
    daily_work_quota: Option<i32>,
}

/// COLLECTION-001 · register a work station. Only a person creates one.
///
/// The station is the durable side of the pair: name, quota and authorization live here, so a
/// plugin reinstall never resets them.
async fn station_register(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(station) = serde_json::from_slice::<StationBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "station_invalid",
        );
    };
    // 200 notes per station per day (Mog's decision), held on the station, not the install.
    let quota = station.daily_work_quota.unwrap_or(200);
    match register_station(database, &station.display_name, quota).await {
        Ok(station_ref) => Json(serde_json::json!({
            "stationRef": station_ref,
            "registeredBy": "person",
            "dailyWorkQuota": quota,
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "station_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimWindowBody {
    station_ref: uuid::Uuid,
    #[serde(default)]
    valid_for_hours: Option<i32>,
}

/// COLLECTION-001 · open a claim window so new installs bind themselves to this station.
///
/// Always time-bounded. During plugin development one window covers a day of reinstalls; a
/// window that never closed would be an open door on a public domain.
async fn station_claim_window(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(window) = serde_json::from_slice::<ClaimWindowBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "claim_window_invalid",
        );
    };
    let hours = window.valid_for_hours.unwrap_or(24);
    match open_claim_window(database, window.station_ref, hours).await {
        Ok(()) => Json(serde_json::json!({
            "stationRef": window.station_ref,
            "validForHours": hours,
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "claim_window_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckInBody {
    install_key: String,
    plugin_version: String,
    #[serde(default)]
    browser_label: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

/// COLLECTION-001 · a plugin install reports in.
///
/// It never creates a station. An unclaimed install is recorded and left idle — present, but
/// not something work can be handed to.
async fn station_check_in(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(check_in) = serde_json::from_slice::<CheckInBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "check_in_invalid",
        );
    };
    let result = check_in_installation(
        database,
        &InstallationCheckIn {
            install_key: &check_in.install_key,
            plugin_version: &check_in.plugin_version,
            browser_label: check_in.browser_label.as_deref(),
            capabilities: serde_json::json!(check_in.capabilities),
        },
    )
    .await;
    match result {
        Ok(outcome) => Json(check_in_payload(&outcome)).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "check_in_rejected",
        ),
    }
}

fn check_in_payload(outcome: &CheckInOutcome) -> serde_json::Value {
    match outcome {
        CheckInOutcome::Claimed {
            installation_ref,
            station_ref,
            superseded,
        } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "claimed",
            "claimKind": "claim_window",
            "stationRef": station_ref,
            // A reinstall replaces the previous install on the same station rather than
            // registering a second station. The replaced one stays visible on purpose.
            "supersededInstallationRef": superseded,
            "execution": "NOT_STARTED",
        }),
        CheckInOutcome::AwaitingClaim { installation_ref } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "awaiting_claim",
            "stationRef": serde_json::Value::Null,
            "execution": "NOT_STARTED",
        }),
        CheckInOutcome::Heartbeat { installation_ref } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "heartbeat",
            "execution": "NOT_STARTED",
        }),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaimBody {
    installation_ref: uuid::Uuid,
    station_ref: uuid::Uuid,
}

/// COLLECTION-001 · a person points one waiting install at one station.
async fn station_claim(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(claim) = serde_json::from_slice::<ClaimBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "claim_invalid",
        );
    };
    match claim_installation(database, claim.installation_ref, claim.station_ref).await {
        Ok(superseded) => Json(serde_json::json!({
            "installationRef": claim.installation_ref,
            "stationRef": claim.station_ref,
            "claimKind": "person",
            "supersededInstallationRef": superseded,
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "claim_rejected",
        ),
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArchiveRequestBody {
    target_ref: uuid::Uuid,
    purpose: String,
    #[serde(default)]
    requested_by: Option<String>,
    /// 哪条观察 lane。深度建档与轻巡检的风险与成本不同，因此授权也分开——写死一个 lane
    /// 会让另一条 lane 的申请永远找不到匹配的授权。
    #[serde(default)]
    lane: Option<String>,
}

/// COLLECTION-001 · request deep archiving for one target, and run admission on it.
///
/// The response reports what admission concluded, including the refusals. A request that
/// produced no Work Order is a normal, recorded outcome — not an error to be retried.
async fn collection_archive_request(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<ArchiveRequestBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "archive_request_invalid",
        );
    };
    let requested_by = request.requested_by.as_deref().unwrap_or("person");
    let lane = request.lane.as_deref().unwrap_or("deep_archive");
    match request_and_admit(
        database,
        request.target_ref,
        lane,
        &request.purpose,
        requested_by,
    )
    .await
    {
        Ok(result) => Json(serde_json::json!({
            "requestRef": result.request_ref,
            "decisionRef": result.decision_ref,
            "admission": result.outcome.code(),
            "unansweredQuestion": result.outcome.unanswered_question().map(|q| q.number()),
            "questionText": result.outcome.unanswered_question().map(|q| q.describe()),
            "workOrderRef": result.work_order_ref,
            // A Work Order is a written instruction. Nothing has run.
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            match error {
                AcquisitionChainError::UnknownTarget => "unknown_target",
                AcquisitionChainError::TargetNotRequestable { .. } => "target_not_requestable",
                AcquisitionChainError::SchemaUnavailable => "acquisition_chain_unavailable",
                AcquisitionChainError::Database(_) => "acquisition_chain_unavailable",
            },
        ),
    }
}

/// COLLECTION-001 · the pending observation targets, read-only.
///
/// Read-only on purpose: storing a target is harmless, but every route that would *create*
/// one has to pass through Acquisition Request → Authorization → Admission first (INV-36),
/// and none of those exist yet. A write route here would be the four-stage chain collapsed
/// into one call — exactly what the rules forbid.
async fn collection_targets_json(State(state): State<LocalWebState>) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    match list_targets_in_state(database, LifecycleState::PendingDecision, 200).await {
        Ok(targets) => Json(serde_json::json!({
            "lifecycleState": "pending_decision",
            "storedTargetCount": targets.len(),
            "targets": targets
                .iter()
                .map(|target| serde_json::json!({
                    "targetRef": target.target_ref,
                    "platform": target.platform,
                    "targetKind": target.target_kind,
                    "identityKey": target.identity_key,
                    "displayName": target.display_name,
                    "source": target.source,
                    "firstStoredAt": target.first_stored_at,
                }))
                .collect::<Vec<_>>(),
            // Storing a target proves it was saved and nothing else.
            "acquisition": "NOT_AUTHORISED",
        }))
        .into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "collection_targets_unavailable",
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
        Err(ProducerRuntimeError::ScheduledTaskNotServerIssued) => local_producer_error(
            axum::http::StatusCode::FORBIDDEN,
            "scheduled_tasks_are_server_issued_only",
        ),
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
        Ok(outcome) => {
            // 采集包被接纳后收尾：结束租约并记下这个目标真的拿回了材料。没有这一步，
            // 「派过」与「成了」永远分不开，下一轮巡检也没有依据判断上一轮是成是败。
            //
            // 手动采集不带租约，此处返回 None，不是错误。
            let _ = complete_lease_for_task(database, submission.task_id()).await;
            // 采到的博主资料回填观察目标：采集与观察目标此前是两条不相交的线，人在列表里
            // 看着一串十六进制 ID，认不出那是谁。
            let package = submission.capture_package();
            let _ = enrich_target_from_author_profile(
                database,
                package.package_kind(),
                package.platform(),
                package.records(),
            )
            .await;
            Json(outcome).into_response()
        }
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
    evidence_page::render_read_unavailable(&evidence_library_html())
}

fn evidence_query_invalid_html() -> String {
    evidence_page::render_query_invalid(&evidence_library_html())
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
    /// 观察目标的筛选。只改读取范围，不消耗任何平台访问。
    filter: Option<String>,
    /// 上一次动作的失败原因。失败必须看得见，否则跳转回来什么都不说，会让人以为成功了。
    error: Option<String>,
    /// 抽屉打开的是哪个目标，以及停在哪个 tab。**放在 URL 里而不是 JS 状态里**：
    /// 刷新与分享都不丢，而这一页的用途正是「打开一个目标细看，然后发给别人」。
    dtab: Option<String>,
}

/// DESIGN-006: the entry lands on the one surface whose contents expire. Arriving on the
/// target list meant opening with the most static thing in Collection — a list that does not
/// change for a week — while anything actually waiting sat two tabs away.
async fn collection_entry() -> Redirect {
    Redirect::temporary("/collection/attention")
}

async fn collection_targets(
    State(state): State<LocalWebState>,
    Query(params): Query<CollectionParams>,
) -> Html<String> {
    // 计数不受当前筛选影响：tab 上的数字要回答「切过去有多少」。
    let counts = match state.database.database() {
        Some(database) => count_targets(database).await.ok(),
        None => None,
    };
    let base = collection::render(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        params.drawer.as_deref(),
        params.filter.as_deref(),
        counts.as_ref(),
    );
    // Without a database the page still renders its honest empty state rather than an error:
    // "we cannot read targets right now" and "there are no targets" are different claims, and
    // the empty state already makes only the weaker one.
    let Some(database) = state.database.database() else {
        return Html(base);
    };
    // 一次查完所有目标的档案完整度：列表最多两百行，逐行发查询会让页面打开一次跑
    // 两百次数据库。
    let completeness = read_archive_completeness(database, linggan_contracts::OPEN_PLATFORM)
        .await
        .unwrap_or_default();
    // 抽屉独立查目标，不从筛过的列表里找——被筛掉的目标不该显示成「未找到」。
    let drawer_target = match params
        .drawer
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    {
        Some(target_ref) => read_target(database, target_ref).await.ok().flatten(),
        None => None,
    };
    match list_targets(database, params.filter.as_deref(), 200).await {
        Ok(targets) => {
            let list = collection_targets_view::render_stored_targets(
                &base,
                &targets,
                &completeness,
                params.error.as_deref(),
            );
            let drawer = target_drawer::render(
                drawer_target.as_ref(),
                &completeness,
                params.drawer.as_deref(),
                params.dtab.as_deref(),
            );
            Html(format!("{list}{drawer}"))
        }
        Err(_) => Html(base),
    }
}

async fn collection_operations(Query(params): Query<CollectionParams>) -> Html<String> {
    Html(collection::render(
        collection::Section::Operations,
        collection::OperationsMode::parse(params.mode.as_deref()),
        None,
        None,
        None,
    ))
}

async fn collection_attention() -> Html<String> {
    Html(collection::render(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    ))
}

async fn collection_tasks() -> Html<String> {
    Html(collection::render(
        collection::Section::Tasks,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    ))
}

#[derive(serde::Deserialize)]
struct RuntimeSurfaceParams {
    #[serde(default)]
    error: Option<String>,
}

async fn collection_runtime(
    State(state): State<LocalWebState>,
    Query(params): Query<RuntimeSurfaceParams>,
) -> Html<String> {
    let base = collection::render(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
        None,
        None,
    );
    // Without a database the page keeps its honest empty state: "we cannot read stations right
    // now" and "no station is registered" are different claims.
    let Some(database) = state.database.database() else {
        return Html(base);
    };
    match read_station_overview(database).await {
        Ok((stations, unclaimed)) => Html(station_view::render_stations(
            &base,
            &stations,
            &unclaimed,
            params.error.as_deref(),
        )),
        Err(_) => Html(base),
    }
}

/// COLLECTION-001 · the person-facing station actions on the 执行工位 surface.
///
/// These write local records only — no platform is contacted and no capture begins, which is
/// why this page may carry real buttons while every platform-consuming control stays absent.
///
/// Each one redirects back with 303 so a refresh re-reads the page instead of re-submitting.
const RUNTIME_SURFACE: &str = "/collection/runtime";

/// 把失败原因带回页面。
///
/// 此前这些表单处理用 `let _ =` 吞掉错误后照常跳转：登记一个重名工位会得到一次跳转、
/// 一切如常的假象，而工位并没有建成。「点了没反应」已经够糟，「点了看起来成功了其实没有」
/// 更糟——它会让人以为系统里有一台并不存在的工位。
fn runtime_surface_with_error(code: &str) -> String {
    format!("{RUNTIME_SURFACE}?error={code}")
}

/// Where a plugin install reports in. Advertised through `/health` so the plugin never has to
/// hardcode it.
const STATION_CHECK_IN_PATH: &str = "/api/local/stations/installations";

/// 工位来问「现在有我能做的活吗」。同样通过 `/health` 通告，插件不写死。
const DISPATCH_CLAIM_PATH: &str = "/api/local/dispatch/claim";

#[derive(serde::Deserialize)]
struct StationForm {
    display_name: String,
}

async fn collection_runtime_register_station(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<StationForm>,
) -> Redirect {
    if let Some(database) = state.database.database() {
        // 200 notes per station per day (Mog's decision), held on the station so a plugin
        // reinstall never resets it.
        if register_station(database, form.display_name.trim(), 200)
            .await
            .is_err()
        {
            return Redirect::to(&runtime_surface_with_error("station_rejected"));
        }
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct ClaimWindowForm {
    station_ref: uuid::Uuid,
    valid_for_hours: i32,
}

async fn collection_runtime_open_window(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<ClaimWindowForm>,
) -> Redirect {
    if let Some(database) = state.database.database()
        && open_claim_window(database, form.station_ref, form.valid_for_hours)
            .await
            .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("claim_window_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct CloseWindowForm {
    station_ref: uuid::Uuid,
}

async fn collection_runtime_close_window(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<CloseWindowForm>,
) -> Redirect {
    if let Some(database) = state.database.database()
        && close_claim_window(database, form.station_ref)
            .await
            .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("close_window_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct RetireForm {
    station_ref: uuid::Uuid,
}

async fn collection_runtime_retire_station(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<RetireForm>,
) -> Redirect {
    if let Some(database) = state.database.database()
        && retire_station(database, form.station_ref, "在执行工位页停用")
            .await
            .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("retire_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct ClaimForm {
    installation_ref: uuid::Uuid,
    station_ref: uuid::Uuid,
}

async fn collection_runtime_claim(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<ClaimForm>,
) -> Redirect {
    if let Some(database) = state.database.database()
        && claim_installation(database, form.installation_ref, form.station_ref)
            .await
            .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("claim_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LeaseBody {
    work_order_ref: uuid::Uuid,
    #[serde(default)]
    valid_for_minutes: Option<i32>,
}

/// COLLECTION-001 · turn one admitted work order into a bounded, revocable permission.
///
/// Issuing is not executing: no platform is contacted and no plugin is told anything. The
/// lease records who may run this work order, inside which frozen identity, until when.
///
/// Authorisation is re-checked here rather than trusted from admission — time passes between
/// the two, and a station can go offline or a risk pause can start in between.
async fn collection_issue_lease(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<LeaseBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "lease_request_invalid",
        );
    };
    // A default that expires: an unbounded permission cannot be taken back once handed out.
    let minutes = request.valid_for_minutes.unwrap_or(30);
    match issue_work_order_lease(database, request.work_order_ref, minutes).await {
        Ok(lease) => Json(serde_json::json!({
            "leaseRef": lease.lease_ref,
            "taskIds": lease.task_ids,
            // 逐篇详情不在首批里，原因写在响应里而不是留给人去猜。
            "detailStepDeferred": linggan_evidence::DETAIL_STEP_DEFERRED_REASON,
            "stationRef": lease.station_ref,
            "expiresAt": lease.expires_at,
            // A lease permits; nothing has run and no plugin has been told anything.
            "execution": "NOT_STARTED",
        }))
        .into_response(),
        Err(error) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            lease_error_code(&error),
        ),
    }
}

/// Each refusal names the specific gate that stopped it. "lease_rejected" would send the
/// reader looking through five different checks.
fn lease_error_code(error: &LeaseError) -> &'static str {
    match error {
        LeaseError::SchemaUnavailable => "lease_schema_unavailable",
        LeaseError::UnknownWorkOrder => "work_order_not_found",
        LeaseError::AlreadyLeased => "work_order_already_leased",
        LeaseError::NoStation => "work_order_names_no_station",
        LeaseError::StationUnavailable => "station_unavailable",
        LeaseError::AuthorizationLapsed => "authorization_lapsed",
        LeaseError::RiskPaused { .. } => "risk_paused",
        LeaseError::TaskSpecInvalid(_) => "task_spec_invalid",
        LeaseError::Database(_) => "lease_write_failed",
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchClaimBody {
    install_key: String,
}

/// COLLECTION-001 · a station asks whether there is work it may run right now.
///
/// Only one answer permits a platform to be touched. Every other answer names the specific
/// gate that stopped it: a flat "nothing for you" cannot distinguish "quota spent" from
/// "risk paused" from "real execution was never authorised", and those need different acts.
async fn dispatch_claim(State(state): State<LocalWebState>, body: Bytes) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<DispatchClaimBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_claim_invalid",
        );
    };
    match decide_dispatch(database, &request.install_key).await {
        Ok(decision) => Json(dispatch_payload(&decision)).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "dispatch_claim_rejected",
        ),
    }
}

fn dispatch_payload(decision: &DispatchDecision) -> serde_json::Value {
    let mut payload = serde_json::json!({
        "decision": decision.code(),
        // The plugin must key off this and nothing else. A task body without permission is
        // still not permission.
        "mayExecute": decision.permits_execution(),
        // 节奏由服务端给：插件不自定间隔，否则想调就得重新发一版插件，而十个插件会各自
        // 按自己的常量敲门，服务端对总量毫无控制。
        "nextPollAfterSeconds": decision.next_poll_after_seconds(),
    });
    match decision {
        DispatchDecision::Dispatch {
            task_id,
            lease_ref,
            task_spec,
        } => {
            payload["taskId"] = serde_json::json!(task_id);
            payload["leaseRef"] = serde_json::json!(lease_ref);
            payload["taskSpec"] = task_spec.clone();
        }
        DispatchDecision::RiskPaused { reason } => {
            payload["reason"] = serde_json::json!(reason);
        }
        DispatchDecision::DailyQuotaReached { quota, used } => {
            payload["reason"] = serde_json::json!(format!(
                "这台工位今天已入库 {used} 篇，达到每日 {quota} 篇上限。工位没有离线，\
                 明天窗口重置后自然恢复。"
            ));
        }
        DispatchDecision::InstallationNotClaimed => {
            payload["reason"] =
                serde_json::json!("这个插件安装还没有归位到任何工位，不属于任何工位的产能。");
        }
        DispatchDecision::NothingWaiting => {
            payload["reason"] = serde_json::json!("没有等待派发的任务。");
        }
    }
    payload
}

#[derive(serde::Deserialize)]
struct NewTargetForm {
    target_kind: String,
    identity: String,
}

/// COLLECTION-001 · 从页面加入一个观察目标。
///
/// 只写本机记录：不访问任何平台，也不会让任何采集开始。加入观察与「开始采集」是两件事，
/// 后者仍然要走申请 → 授权 → 准入 → 工单 → 租约 → 闸门。
async fn collection_target_create(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<NewTargetForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to("/collection/targets?error=read_model_not_connected");
    };
    let raw = form.identity.trim();
    // 创作者用主页链接就够了——平台 ID 藏在 URL 里，让人自己去扒是把工具的活推给使用者。
    let identity = match form.target_kind.as_str() {
        "creator" => creator_id_from(raw),
        _ => Some(raw.to_owned()),
    };
    let Some(identity) = identity.filter(|value| !value.is_empty()) else {
        return Redirect::to("/collection/targets?error=identity_unrecognised");
    };
    let intake = collection_intake::TargetIntake {
        platform: linggan_contracts::OPEN_PLATFORM.to_owned(),
        target_kind: form.target_kind.clone(),
        identity,
        // 同一个词的两种排序是两个观察面，因此关键词默认落在综合排序上并写明。
        ranking: (form.target_kind == "keyword").then(|| "comprehensive".to_owned()),
        // 粘进来的是链接时不要拿整条 URL 当名字：它又长又带追踪参数，在列表里认不出人。
        // 真名要等采集回来才知道，在那之前留空比塞一条 URL 诚实。
        display_name: (!raw.starts_with("http")).then(|| raw.to_owned()),
        identity_facts: None,
    };
    let parsed_display_name = intake.display_name.clone();
    match collection_intake::parse_intake(&intake) {
        // 页面添加记 manual，不冒充插件推送——来源是「谁把它加进来的」这个事实。
        Ok(parsed) => match store_pending_target(
            database,
            &parsed,
            linggan_contracts::TargetSource::Manual,
            parsed_display_name.as_deref(),
            None,
        )
        .await
        {
            Ok(_) => Redirect::to("/collection/targets"),
            Err(_) => Redirect::to("/collection/targets?error=store_failed"),
        },
        Err(_) => Redirect::to("/collection/targets?error=identity_unrecognised"),
    }
}

/// 从小红书创作者主页链接里取出平台 ID。
///
/// 链接形如 `https://www.xiaohongshu.com/user/profile/<24 位十六进制>?...`。**平台 ID 才是
/// 身份，URL 不是**：URL 会带追踪参数、会变形，同一个人两个链接会变成两个观察目标。
/// 取不到就明说取不到，不拿整条 URL 凑数。
fn creator_id_from(raw: &str) -> Option<String> {
    let candidate = raw
        .rsplit("/user/profile/")
        .next()
        .unwrap_or(raw)
        .split(['?', '#', '/'])
        .next()
        .unwrap_or("")
        .trim();
    let looks_like_platform_id = candidate.len() >= 16
        && candidate.len() <= 32
        && candidate.chars().all(|c| c.is_ascii_hexdigit());
    looks_like_platform_id.then(|| candidate.to_owned())
}

#[derive(serde::Deserialize)]
struct MonitoringForm {
    /// 行内按钮用的字段名与批量勾选的 `target_ref` **必须不同**：两者在同一个表单里，
    /// 点单行按钮时勾选的行也会一起提交，同名会让单行动作莫名其妙作用到一批目标上。
    row_target_ref: uuid::Uuid,
}

/// COLLECTION-001 · 切换一个观察目标的巡检开关。
///
/// 只写本机记录：打开巡检不等于立刻采集——调度器仍要按间隔到期、准入仍要过六问、
/// 额度与风险暂停仍然管用。
async fn collection_target_toggle_monitoring(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<MonitoringForm>,
) -> Redirect {
    if let Some(database) = state.database.database() {
        let enabled = target_monitoring_enabled(database, form.row_target_ref)
            .await
            .unwrap_or(false);
        if set_target_monitoring(database, form.row_target_ref, !enabled, None)
            .await
            .is_err()
        {
            return Redirect::to("/collection/targets?error=monitoring_toggle_failed");
        }
    }
    Redirect::to("/collection/targets")
}

/// COLLECTION-001 · 从页面发起一次深度建档。
///
/// 它**不绕过授权链**：走的是与定时巡检、与 API 完全相同的一条路——申请、准入六问、
/// 工单、租约。按钮只是把「人现在想要这个」表达出来，能不能做仍由准入回答。
///
/// 失败原因原样带回页面：没有覆盖深度建档的授权、目标已在建档中、工位不在岗，这三种
/// 情况的处置完全不同，压成一句「失败」等于让人自己去猜。
async fn collection_target_deep_archive(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<MonitoringForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to("/collection/targets?error=read_model_not_connected");
    };
    let outcome = request_and_admit(
        database,
        form.row_target_ref,
        "deep_archive",
        "从观察目标页发起深度建档",
        "person",
    )
    .await;
    let Ok(outcome) = outcome else {
        return Redirect::to("/collection/targets?error=archive_not_requestable");
    };
    let Some(work_order_ref) = outcome.work_order_ref else {
        // 准入没通过。把它的结论原样带回去——refuse 与 defer 的处置完全不同。
        return Redirect::to(&format!(
            "/collection/targets?error=archive_{}",
            outcome.outcome.code()
        ));
    };
    // 深度建档给 60 分钟：200 条作品的清单加逐篇详情，比一次巡检重得多。
    if issue_work_order_lease(database, work_order_ref, 60)
        .await
        .is_err()
    {
        return Redirect::to("/collection/targets?error=archive_lease_failed");
    }
    Redirect::to("/collection/targets")
}

/// COLLECTION-001 · 对勾选的来源做批量操作。
///
/// 手工解析表单而不是用 `Form<T>`：`serde_urlencoded` **不支持同名字段收成数组**
/// （已知限制，不是用法问题），而勾选框正是靠重复的 `target_ref` 表达「选了哪几行」。
/// 与其为此引一个新依赖，不如就地解析这一个请求。
///
/// 批量是**明确指定开或关**，不是逐个取反：取反会让一次操作里有的开有的关，人点了
/// 「批量开启巡检」却得到一半被关掉，那不是他要的。
///
/// 分组只写本机记录，不影响任何采集行为——它是人自己的分类方式。
/// 解析 `application/x-www-form-urlencoded`，**保留同名字段的全部取值**。
///
/// 只做这一件事，因此不引新依赖：`serde_urlencoded` 丢掉重复键，而勾选框正是靠重复键
/// 表达「选了哪几行」。
fn parse_form_pairs(body: &[u8]) -> Vec<(String, String)> {
    String::from_utf8_lossy(body)
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

/// 表单编码把空格写成 `+`，其余非 ASCII 写成 `%XX`。
///
/// **全程按字节做，不对字符串切片**。此前用 `&raw[index + 1..index + 3]` 取那两位十六
/// 进制，而 `index` 是字节下标——一个裸 `%` 后面跟中文（比如分组名写成 `%中文`）就会
/// 切在 UTF-8 字符中间，直接 panic 掉整个请求。这不是理论风险：分组名是自由文本。
fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                match (hex_value(bytes[index + 1]), hex_value(bytes[index + 2])) {
                    (Some(high), Some(low)) => {
                        out.push(high << 4 | low);
                        index += 3;
                    }
                    // 不是合法的 %XX 就当普通字符原样留下，而不是丢掉它。
                    _ => {
                        out.push(bytes[index]);
                        index += 1;
                    }
                }
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

async fn collection_targets_batch(State(state): State<LocalWebState>, body: Bytes) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to("/collection/targets?error=read_model_not_connected");
    };
    let mut action = String::new();
    let mut group_name: Option<String> = None;
    let mut target_refs: Vec<uuid::Uuid> = Vec::new();
    for (key, value) in parse_form_pairs(&body) {
        match key.as_str() {
            "action" => action = value,
            "group_name" => group_name = Some(value),
            // 认不出的 uuid 直接忽略：一个坏值不该让整批操作失败，而它也不会被误当成
            // 别的目标——解析不出来就不在名单里。
            "target_ref" => {
                if let Ok(parsed) = uuid::Uuid::parse_str(&value) {
                    target_refs.push(parsed);
                }
            }
            _ => {}
        }
    }
    if target_refs.is_empty() {
        return Redirect::to("/collection/targets?error=batch_nothing_selected");
    }
    let outcome = match action.as_str() {
        "monitor_on" => set_monitoring_for_many(database, &target_refs, true).await,
        "monitor_off" => set_monitoring_for_many(database, &target_refs, false).await,
        "set_group" => set_group_for_many(database, &target_refs, group_name.as_deref()).await,
        _ => return Redirect::to("/collection/targets?error=batch_unknown_action"),
    };
    if outcome.is_err() {
        return Redirect::to("/collection/targets?error=batch_failed");
    }
    Redirect::to("/collection/targets")
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
    <title>证据库 · Linggan Intelligence</title>
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
          <div class="v7-side-foot"><span class="v7-side-dot"></span><!-- EVIDENCE_READ_STATUS_START --><span class="v7-zh-status">来源材料尚未完整接通</span><span class="v7-tech-key">SOURCE INCOMPLETE</span><br><span class="v7-zh-status">仅展示本机页面结构</span><span class="v7-tech-key">LOCAL PRESENTATION</span><br><span class="v7-zh-status">当前尚未读取任何材料</span><span class="v7-tech-key">NO MATERIAL READ</span><!-- EVIDENCE_READ_STATUS_END --></div>
        </aside>

        <main class="v7-main" aria-labelledby="page-title">
          <section class="v7-page-header">
            <h1 class="v7-sr-only" id="page-title">证据库</h1>
            <form class="v7-search-row" method="get"><label class="v7-search"><span class="v7-cmd">⌘ 检索</span><!-- EVIDENCE_SEARCH_INPUT_START --><input disabled aria-disabled="true" placeholder="等待受控材料读投影接通……"><!-- EVIDENCE_SEARCH_INPUT_END --><kbd>⌘ K</kbd></label><details class="v7-dd" aria-label="搜索范围"><summary><em>类型</em><strong>全部</strong><i aria-hidden="true">▾</i></summary><div class="v7-dd-menu"><button type="button" disabled aria-current="true">全部</button><button type="button" disabled>作品</button><button type="button" disabled>评论</button><button type="button" disabled>转录</button><button type="button" disabled>作者</button></div></details><div class="v7-page-actions"><button class="v7-btn" disabled aria-disabled="true">复制查询</button><button class="v7-btn" disabled aria-disabled="true">保存当前视图</button><button class="v7-btn v7-primary" disabled aria-disabled="true">发起研究</button></div></form>
            <div class="v7-views-bar">
              <div class="v7-view-group"><span class="v7-view-label">系统视图 <em>SYSTEM VIEWS</em></span><div class="v7-view-strip"><button class="v7-view-pill" disabled aria-current="true"><i>01</i><span>最新发现</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>02</i><span>待补采</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>03</i><span>高互动</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>04</i><span>评论密集</span><span class="v7-n">—</span></button><button class="v7-view-pill" disabled><i>05</i><span>最近异常</span><span class="v7-n">—</span></button><button class="v7-view-pill v7-more" disabled><i>+</i><span>更多</span><span class="v7-n">⌄</span></button></div></div>
              <div class="v7-view-group v7-my"><span class="v7-view-label">我的视图 <em>MY VIEWS</em></span><div class="v7-view-strip"><button class="v7-view-pill" disabled><i>A</i><span>ADHD 作业</span></button><button class="v7-view-pill" disabled><i>B</i><span>低粉爆文</span></button><button class="v7-view-pill" disabled><i>C</i><span>家长原声研究</span></button></div></div>
            </div>
            <div class="v7-controls-row"><div class="v7-filters"><span class="v7-filter-lead">筛选 <em>FILTER</em></span><button class="v7-chip" disabled><em>平台</em> 全部 <span class="v7-tech-key">ALL</span></button><button class="v7-chip" disabled><em>窗口</em> 未知 <span class="v7-tech-key">UNKNOWN</span></button><button class="v7-chip" disabled><em>类型</em> 全部 <span class="v7-tech-key">ALL</span></button><button class="v7-chip" disabled><em>来源</em> 未知 <span class="v7-tech-key">UNKNOWN</span></button><button class="v7-chip" disabled><em>状态</em> 未知 <span class="v7-tech-key">UNKNOWN</span></button></div><div class="v7-controls"><button class="v7-control" disabled><small>分组</small><strong>不分组</strong></button><button class="v7-control" disabled><small>排序</small><strong>未知 <span class="v7-tech-key">UNKNOWN</span></strong></button><button class="v7-control" disabled><small>密度</small><strong>标准</strong></button><div class="v7-seg"><span class="v7-seg-label">视图</span><button disabled aria-current="true">研究</button><button disabled>表格</button><button disabled>封面</button></div></div></div>
            <!-- EVIDENCE_QUERY_LINE_START --><div class="v7-query-line"><div>页面结构已就绪 · 材料读投影尚未接通</div><div><b>来源信息尚未完整接通 <span class="v7-tech-key">SOURCE INCOMPLETE</span></b> · <span>当前没有可用查询 <span class="v7-tech-key">NO QUERY AVAILABLE</span></span></div></div><!-- EVIDENCE_QUERY_LINE_END -->
          </section>

            <section class="v7-workspace" aria-label="证据库工作区">
            <section class="v7-results" aria-label="事实材料列表">
              <div class="v7-fact-strap"><span>事实层 / 证据 <span class="v7-tech-key">FACT LAYER / EVIDENCE</span></span><i aria-hidden="true"></i><b>原始内容资产</b></div>
              <!-- EVIDENCE_RESULTS_HEAD_START --><div class="v7-results-head"><div class="v7-results-left"><input class="v7-check" type="checkbox" disabled aria-label="选择全部材料"><span>当前没有已接纳材料 <span class="v7-tech-key">NO ACCEPTED MATERIAL AVAILABLE</span></span></div><div>本地读投影尚未接通 <span class="v7-tech-key">READ MODEL NOT CONNECTED</span></div></div><!-- EVIDENCE_RESULTS_HEAD_END -->
              <!-- EVIDENCE_RESULTS_START --><div class="v7-results-empty">
                <article class="v7-empty-row"><input class="v7-check" type="checkbox" disabled aria-label="无材料"><div class="v7-empty-mark">?</div><div class="v7-empty-main"><div class="v7-empty-title">来源信息尚未完整接通 <span class="v7-tech-key">SOURCE INCOMPLETE</span></div><div class="v7-empty-copy">页面还没有连接到受控的本地材料读投影，因此不能列出内容、评论、转录或来源对象。</div><div class="v7-empty-boundary"><b>当前没有已接纳材料 <span class="v7-tech-key">NO ACCEPTED MATERIAL AVAILABLE</span></b>这不是世界中不存在内容，也不是库内数量为零。</div><div class="v7-empty-facts"><span>事实层级 <strong>未知 <span class="v7-tech-key">UNKNOWN</span></strong></span><span>覆盖情况 <strong>未知 <span class="v7-tech-key">UNKNOWN</span></strong></span><span>页面投影 <strong>未接通 <span class="v7-tech-key">NOT CONNECTED</span></strong></span></div></div><div class="v7-empty-metric"><div><b>—</b><span>材料数 <span class="v7-tech-key">ITEMS</span></span></div><div><b>—</b><span>观察数 <span class="v7-tech-key">OBS</span></span></div><div><b>—</b><span>来源数 <span class="v7-tech-key">SOURCE</span></span></div></div></article>
                <section class="v7-empty-panel" aria-labelledby="empty-title"><h2 id="empty-title">没有可展示的本地材料</h2><p>当前本机服务只提供此页面的视觉和信息边界；它没有读取数据库、历史内容工作台或插件结果。</p><dl class="v7-empty-grid"><div><dt>现在知道什么</dt><dd>页面可由本机服务提供；材料读取合同未接通。</dd></div><div><dt>现在不知道什么</dt><dd>材料、来源、观察时间、采集记录与覆盖情况均为未知。</dd></div><div><dt>下一步</dt><dd>需在独立范围内建立受控只读投影。</dd></div></dl></section>
              </div><!-- EVIDENCE_RESULTS_END -->
            </section>
            <aside class="v7-inspect" aria-labelledby="inspector-title">
              <div class="v7-inspector-head"><div class="v7-inspector-identity"><div class="v7-iid">未选择材料 <span class="v7-tech-key">#NO_SELECTION</span></div><h2 class="v7-ititle" id="inspector-title">尚未选择材料</h2><div class="v7-imeta"><span>内容对象未知 <span class="v7-tech-key">CONTENT ITEM UNKNOWN</span></span><span>·</span><span>观察未知 <span class="v7-tech-key">OBSERVATION UNKNOWN</span></span><span>·</span><span>采集记录未知 <span class="v7-tech-key">CAPTURE UNKNOWN</span></span></div></div><div class="v7-inspector-ops"><div class="v7-inspector-primary"><button disabled aria-disabled="true">↗ 原文</button><button disabled aria-disabled="true">⟳ 补采</button><button disabled aria-disabled="true">＋ 研究</button></div><div class="v7-inspector-window"><button disabled aria-disabled="true">固定面板 <span class="v7-tech-key">PIN</span></button><button disabled aria-disabled="true">加宽面板 <span class="v7-tech-key">WIDE</span></button><button disabled aria-label="关闭检查器">×</button></div></div><div class="v7-tabs" aria-label="材料详情页签"><button disabled aria-current="page">概览</button><button disabled>正文</button><button disabled>评论</button><button disabled>历史</button><button disabled>来源</button><button disabled>关系</button></div></div>
              <div class="v7-inspector-body"><section class="v7-section"><h3>原始内容 <em>RAW CONTENT</em> <span>未知 <span class="v7-tech-key">UNKNOWN</span></span></h3><div class="v7-body-copy">没有选中内容对象，也没有可显示的受限材料。此处不能推断标题、正文、作者或平台状态。</div></section><section class="v7-section"><h3>最强命中 <em>STRONGEST MATCH</em> <span>没有材料 <span class="v7-tech-key">NO MATERIAL</span></span></h3><div class="v7-quote">没有材料可供匹配或引用。<small>不显示示例原文、评论或转录。</small></div></section><section class="v7-section"><h3>平台与本地事实 <em>PLATFORM / LOCAL FACTS</em> <span>未知 <span class="v7-tech-key">UNKNOWN</span></span></h3><div class="v7-readout"><div><b>—</b><span>平台点赞 <span class="v7-tech-key">PLATFORM LIKES</span></span></div><div><b>—</b><span>平台评论 <span class="v7-tech-key">PLATFORM COMMENTS</span></span></div><div><b>—</b><span>本地评论 <span class="v7-tech-key">LOCAL COMMENTS</span></span></div><div><b>—</b><span>观察记录 <span class="v7-tech-key">OBSERVATIONS</span></span></div></div></section><section class="v7-section"><h3>状态矩阵 <em>STATE MATRIX</em></h3><div class="v7-matrix"><div class="v7-matrix-row"><div class="v7-k">事实来源</div><div class="v7-v"><i class="v7-dot"></i>来源信息尚未完整接通 <span class="v7-tech-key">SOURCE INCOMPLETE</span></div></div><div class="v7-matrix-row"><div class="v7-k">读投影</div><div class="v7-v"><i class="v7-dot"></i>未接通 <span class="v7-tech-key">NOT CONNECTED</span></div></div><div class="v7-matrix-row"><div class="v7-k">覆盖</div><div class="v7-v"><i class="v7-dot"></i>未知 <span class="v7-tech-key">UNKNOWN</span></div></div></div></section></div>
            </aside>
          </section>
        </main>
      </div>
    </div>
  </body>
</html>"#;
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "<!-- EVIDENCE_HEADER_BOUNDARY_START -->本机服务 / 读投影未接通 <span class=\"v7-tech-key\">LOCAL HOST / NO READ MODEL</span><!-- EVIDENCE_HEADER_BOUNDARY_END -->",
        "语料 <span class=\"v7-slash\">/</span> <b>证据库</b> <span class=\"v7-slash\">/</span> <span class=\"v7-context-current\">材料状态</span>",
        "<span class=\"v7-kpi\"><em>内容</em><b>未知 <span class=\"v7-tech-key\">UNKNOWN</span></b></span><span class=\"v7-kpi\"><em>评论</em><b>未知 <span class=\"v7-tech-key\">UNKNOWN</span></b></span><span class=\"v7-kpi\"><em>创作者</em><b>未知 <span class=\"v7-tech-key\">UNKNOWN</span></b></span><i class=\"v7-vr\" aria-hidden=\"true\"></i><!-- EVIDENCE_HEADER_META_STATE_START --><span class=\"v7-query-meta\">本地读投影未接通 <span class=\"v7-tech-key\">READ MODEL NOT CONNECTED</span></span><span>来源信息尚未完整接通 <span class=\"v7-tech-key\">SOURCE INCOMPLETE</span></span><!-- EVIDENCE_HEADER_META_STATE_END --><span>本机时区 <span class=\"v7-tech-key\">UTC+08</span></span>",
    );
    base.replace(
        "<!-- GLOBAL_HEADER_START --><!-- GLOBAL_HEADER_END -->",
        &header,
    )
}

#[cfg(test)]
mod tests;
