mod ai_settings;
#[cfg(test)]
mod ai_settings_browser_tests;
#[cfg(test)]
mod ai_settings_tests;
mod collection;
mod collection_command_api;
mod collection_dispatch;
mod collection_intake;
mod collection_management;
mod collection_management_api;
#[cfg(test)]
mod collection_readonly_preview_tests;
mod collection_run_api;
mod collection_shell_api;
#[cfg(test)]
mod collection_single_run_tests;
mod collection_target_presentation;
mod collection_targets_api;
mod collection_targets_view;
mod collection_task_actions;
mod collection_task_diagnostics_api;
mod comment_study;
mod content_workbench_ai_api;
mod content_workbench_api;
#[cfg(test)]
mod content_workbench_api_tests;
#[cfg(test)]
mod content_workbench_browser_tests;
mod content_workbench_sources_api;
mod corpus_shell_api;
mod creator_lifecycle_api;
#[cfg(test)]
mod creator_lifecycle_tests;
mod creators;
#[cfg(test)]
mod creators_browser_tests;
#[cfg(test)]
mod evidence_api_proof_tests;
#[cfg(test)]
mod evidence_browser_tests;
#[cfg(test)]
mod evidence_fixture;
#[cfg(test)]
mod evidence_guard_tests;
#[cfg(test)]
mod evidence_seed;
#[cfg(test)]
mod full_schema_fixture;
mod local_asset_delivery;
mod local_media_routes;
#[cfg(test)]
mod material_asset_route_fixture;
#[cfg(test)]
mod material_cursor_tests;
#[cfg(test)]
mod material_media_delivery_tests;
mod material_projection;
#[cfg(test)]
mod material_projection_media_fixture;
#[cfg(test)]
mod material_projection_tests;
#[cfg(test)]
mod material_replica_fallback_tests;
mod model_settings;
mod ocr_audit;
mod ocr_recognition;
#[cfg(test)]
mod ocr_recognition_browser_tests;
mod offline_preview;
#[cfg(test)]
mod offline_preview_tests;
mod ops;
mod ops_api;
mod ops_auth;
#[cfg(test)]
mod ops_auth_tests;
#[cfg(test)]
mod ops_browser_tests;
mod ops_export;
mod ops_overview_api;
mod research_dossier;
#[cfg(test)]
mod research_dossier_tests;
mod station_view;
mod target_drawer;
#[cfg(test)]
mod topic_browser_tests;
mod topic_map;
mod topic_workspace;
#[cfg(test)]
mod topic_workspace_tests;
mod web_app;
#[cfg(test)]
mod web_app_tests;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderValue, header},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
};
use linggan_contracts::{
    LifecycleState, parse_local_producer_attempt, parse_local_producer_submission,
    parse_local_task_spec, parse_producer_attempt, parse_producer_submission,
    parse_producer_task_spec,
};
use linggan_evidence::observation_domain::{ObservationDomain, read_observation_domains};
use linggan_evidence::{
    AcquisitionChainError, AuthorizationGrant, COLLECTION_RUNTIME_REQUIREMENTS, CheckInOutcome,
    DiscoveryIngressError, InstallationCheckIn, LeaseError, LocalAttemptOutcome,
    LocalProducerError, LocalSubmissionOutcome, LocalTaskOutcome, MaterialDeepeningTarget,
    MediaUploadFinalizeClaim, MonitorCommandActor, MonitorCommandKind, MonitorRuleCommand,
    MonitorRuleCommandError, MonitorRuleDraft, MonitorRuleMode, ProducerRuntimeError,
    RequestLeaseError, RuntimeAttemptOutcome, RuntimeReadiness, RuntimeSubmissionOutcome,
    RuntimeTaskOutcome, StoreOutcome, TargetDeletionOutcome, WorkResourceReadError,
    admit_media_blob, apply_monitor_rule_command, assign_target_domain, begin_media_upload,
    bind_observation_account, check_in_installation, claim_installation, claim_media_acquisition,
    claim_media_upload_finalize, close_claim_window, complete_media_upload, count_targets,
    create_manual_task, create_producer_task, delete_observation_target, dispatch_schema_is_ready,
    grant_authorization, ingest_discovery_package, issue_work_order_lease,
    keyword_baselines_qualified, list_targets_in_state, local_discovery_schema_is_ready,
    local_producer_schema_is_ready, media_acquisition_schema_is_ready, open_claim_window,
    probe_runtime_readiness, producer_runtime_schema_is_ready, read_media_upload_session,
    read_runtime_capacity, read_scheduler_heartbeat, read_station_capabilities,
    read_station_overview, record_media_acquisition_failure, record_media_download_failure,
    record_media_upload_chunk, register_station, release_media_upload_finalize, rename_station,
    request_and_admit_for_domain, request_and_admit_material_targets_for_domain,
    request_progressive_archive, retire_materials, retire_station, set_group_for_many,
    set_station_accepting, start_local_attempt, start_producer_attempt, station_schema_is_ready,
    store_pending_target, submit_local_package, submit_producer_package,
    sync_target_from_author_profile, toggle_target_patrol,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    io::{BufReader, Error, ErrorKind, Read, Seek, SeekFrom, Write},
    net::{Ipv4Addr, SocketAddr},
    path::{Path as FsPath, PathBuf},
    sync::Arc,
};

const LOCAL_HOST: Ipv4Addr = Ipv4Addr::LOCALHOST;
const LOCAL_PORT: u16 = 3000;
const FULL_PRODUCER_RUNTIME_DATA_STATE: &str = "LINGGAN_BROWSER_PRODUCER_RUNTIME";
const FULL_PRODUCER_RUNTIME_SCHEMA: &str = "PLUGIN_RUNTIME_002_SCHEMA_READY";
// The Browser Producer obtains these three paths from /health before it starts a
// durable outbox delivery. Keep the router and published contract on the same
// constants so a renamed server route cannot leave the plugin delivering to a
// stale endpoint.
const LOCAL_PRODUCER_TASK_CREATION_PATH: &str = "/api/local/producer/tasks";
const LOCAL_PRODUCER_ATTEMPT_START_PATH: &str = "/api/local/producer/runtime-attempts";
const LOCAL_PRODUCER_SUBMISSION_PATH: &str = "/api/local/producer/runtime-submissions";
const MEDIA_ACQUISITION_CLAIM_PATH: &str = "/api/local/producer/media-acquisitions/claim";
#[cfg(test)]
const LIDS_TOKEN_DOCUMENT: &str = include_str!("../../../docs/design/lids/tokens.md");

#[derive(Clone)]
struct LocalWebState {
    database: LocalDatabaseState,
    active_media_sessions: Arc<tokio::sync::Mutex<BTreeSet<uuid::Uuid>>>,
    account_digest_key: Option<Arc<Vec<u8>>>,
}

#[derive(Clone)]
enum LocalDatabaseState {
    NotConfigured,
    DatabaseUnavailable,
    /// 连得上，但本地读取面的基础 schema 不在，所以这个进程读不了任何东西。连接留着：
    /// `/health` 的就绪判定要拿它问清楚**为什么**不在（台账有没有、缺哪个迁移），
    /// 而不是在这里替它猜一个原因。
    SchemaUnavailable(Arc<Database>),
    Ready(Arc<Database>),
}

impl LocalDatabaseState {
    fn database(&self) -> Option<&Database> {
        match self {
            Self::Ready(database) => Some(database),
            Self::NotConfigured | Self::DatabaseUnavailable | Self::SchemaUnavailable(_) => None,
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
            Self::SchemaUnavailable(_) => (
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
        account_digest_key: None,
    })
}

#[cfg(test)]
fn app_with_database(database: Database) -> Router {
    router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(database)),
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        account_digest_key: Some(Arc::new(
            b"local-web-test-account-digest-key-32-plus".to_vec(),
        )),
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
        .route(
            "/api/local/collection/material-deepening",
            post(collection_material_deepening),
        )
        .route("/api/local/stations", post(station_register))
        .route(
            "/api/local/stations/claim-window",
            post(station_claim_window),
        )
        .route(STATION_CHECK_IN_PATH, post(station_check_in))
        .route("/api/local/stations/claims", post(station_claim))
        .merge(collection_dispatch::routes())
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
            MEDIA_ACQUISITION_CLAIM_PATH,
            post(claim_media_acquisition_route),
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
        .merge(material_api_routes())
        .merge(collection_api_routes())
        .merge(
            Router::new()
                .merge(collection_shell_api::routes())
                .merge(collection_task_diagnostics_api::routes())
                .merge(collection_command_api::routes())
                .merge(collection_targets_api::routes())
                .merge(collection_management_api::routes())
                .layer(axum::middleware::from_fn(
                    comment_study::local_comment_study_guard,
                )),
        )
        .merge(creator_lifecycle_api::routes())
        .merge(collection_run_api::routes())
        .merge(creators::routes())
        .merge(corpus_shell_api::routes())
        .merge(topic_workspace::routes())
        .merge(topic_map::routes(state.database.database().cloned()))
        .merge(comment_study::routes())
        .merge(model_settings::routes())
        .merge(ai_settings::routes())
        .merge(offline_preview::routes())
        .merge(ops_auth::routes())
        .merge(web_app::routes())
        .merge(web_app::ops_routes())
        .merge(ops::routes())
        .merge(content_workbench_api::routes())
        .merge(content_workbench_sources_api::routes())
        .merge(content_workbench_ai_api::routes())
        .route("/corpus", get(corpus_entry))
        .route("/collection", get(collection_entry))
        .with_state(state)
}

fn material_api_routes() -> Router<LocalWebState> {
    Router::new()
        .merge(ocr_recognition::routes())
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier",
            get(research_dossier::read).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier/note-material",
            get(research_dossier::note_material).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier/creator-preview",
            get(research_dossier::creator_preview).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier/comments",
            get(research_dossier::comments).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier/relayer-plan",
            get(research_dossier::plan).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/research-dossier/relayer",
            post(research_dossier::relayer).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/media/{materialization_ref}/{sha256}",
            get(local_media_routes::materialization),
        )
        .route(
            "/api/local/derivative/{derivative_ref}",
            get(local_media_routes::derivative),
        )
        .route(
            "/api/local/work-resources",
            get(evidence_library_json).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/comments",
            get(material_projection::research_comments_json).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/reobserve",
            post(material_projection::reobserve_json).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/reobserve/{lease_ref}",
            get(material_projection::reobservation_status_json).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}",
            get(material_projection::detail_json).layer(axum::middleware::from_fn(
                comment_study::local_comment_study_guard,
            )),
        )
}

async fn local_entry() -> Redirect {
    Redirect::temporary("/login")
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
        account_digest_key: configured_account_digest_key(),
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

fn configured_account_digest_key() -> Option<Arc<Vec<u8>>> {
    let value = std::env::var("LINGGAN_ACCOUNT_DIGEST_KEY").ok()?;
    let bytes = value.into_bytes();
    (bytes.len() >= 32).then(|| Arc::new(bytes))
}

async fn health(State(state): State<LocalWebState>) -> Json<Value> {
    let (data_state, evidence_read_model, database_state, schema_state) =
        state.database.health_state().await;
    // The plugin learns where to check in from /health rather than hardcoding a path, the
    // same way it already learns the producer routes. A route is advertised only once its
    // schema is applied: advertising it earlier would invite a call that cannot succeed.
    let station_routes = match state.database.database() {
        Some(database) if station_schema_is_ready(database).await.unwrap_or(false) => {
            let account_observation_ready =
                linggan_evidence::collection_control_schema_is_ready(database)
                    .await
                    .unwrap_or(false);
            let (eligibility_report, account_observation) = if !account_observation_ready {
                (Value::Null, "schema_unavailable")
            } else if state.account_digest_key.is_none() {
                // Explicit login/cooling/restriction facts do not need an identity digest and
                // must remain reportable. The endpoint rejects only authenticated identity
                // observations while the key is unavailable.
                (
                    Value::String(collection_dispatch::ACCOUNT_ELIGIBILITY_PATH.to_owned()),
                    "identity_key_missing",
                )
            } else {
                (
                    Value::String(collection_dispatch::ACCOUNT_ELIGIBILITY_PATH.to_owned()),
                    "ready",
                )
            };
            json!({
                "checkIn": STATION_CHECK_IN_PATH,
                "eligibilityReport": eligibility_report,
                "accountObservation": account_observation,
                "credentialActivation": collection_dispatch::CREDENTIAL_ACTIVATION_PATH
            })
        }
        _ => Value::Null,
    };
    // 派发路由同样只在其 schema 就绪后通告。通告它不等于闸门开着——闸门是另一回事，
    // 由 claim 的回答给出。
    let dispatch_routes = match state.database.database() {
        Some(database) if dispatch_schema_is_ready(database).await.unwrap_or(false) => json!({
            "claim": collection_dispatch::CLAIM_PATH,
            "failure": collection_dispatch::FAILURE_PATH,
            "detailPageSessionGrant": collection_dispatch::DETAIL_PAGE_SESSION_GRANT_PATH,
            "detailPageSessionNavigation": collection_dispatch::DETAIL_PAGE_SESSION_NAVIGATION_PATH,
            "detailPageRiskSignal": collection_dispatch::DETAIL_PAGE_RISK_SIGNAL_PATH,
            // 通道交付身份的握手版本。插件只在本字段出现时才在授权请求里带上它；
            // 没通告就不请求，请求了但服务端认不出则该次授权被拒。
            "detailPageSessionLanePreparationContract":
                collection_dispatch::DETAIL_PAGE_SESSION_LANE_PREPARATION_CONTRACT
        }),
        _ => Value::Null,
    };
    let media_acquisition_ready = match state.database.database() {
        Some(database) => media_acquisition_schema_is_ready(database)
            .await
            .unwrap_or(false),
        None => false,
    };
    let local_producer_routes = if data_state == FULL_PRODUCER_RUNTIME_DATA_STATE
        && database_state == "READY"
        && schema_state == FULL_PRODUCER_RUNTIME_SCHEMA
        && media_acquisition_ready
    {
        json!({
            "taskCreation": LOCAL_PRODUCER_TASK_CREATION_PATH,
            "attemptStart": LOCAL_PRODUCER_ATTEMPT_START_PATH,
            "submission": LOCAL_PRODUCER_SUBMISSION_PATH,
            "mediaAcquisitionClaim": MEDIA_ACQUISITION_CLAIM_PATH
        })
    } else {
        Value::Null
    };
    // 「这台机器现在能不能接活」用与 worker 同一套判据（`runtime_readiness`），不在页面这层
    // 另解释一遍 `database.state`——同一件事两处各判一次，迟早会出现 worker 说未就绪、
    // 页面说没问题。`database.state` 与 `scheduler` 的含义都不变，这里是新增的第三个回答。
    let readiness = match &state.database {
        LocalDatabaseState::Ready(database) | LocalDatabaseState::SchemaUnavailable(database) => {
            probe_runtime_readiness(database, &COLLECTION_RUNTIME_REQUIREMENTS).await
        }
        // 地址没给是配置错误，等多久都不会好；连不上是环境问题，等它回来。
        LocalDatabaseState::NotConfigured => RuntimeReadiness::not_configured(),
        LocalDatabaseState::DatabaseUnavailable => RuntimeReadiness::unreachable("connect_failed"),
    };
    let scheduler = match state.database.database() {
        Some(database) => match read_scheduler_heartbeat(database).await {
            Ok(Some(heartbeat)) => json!({
                "state": heartbeat.state,
                "lastTickCompletedAt": heartbeat.last_tick_completed_at,
                "lastOutcome": heartbeat.last_outcome,
                "dispatchedCount": heartbeat.dispatched_count,
                "skippedCount": heartbeat.skipped_count,
                "lastError": heartbeat.last_error,
            }),
            Ok(None) | Err(_) => json!({
                "state": "unknown",
                "lastTickCompletedAt": Value::Null,
                "lastOutcome": "unknown"
            }),
        },
        None => json!({
            "state": "unknown",
            "lastTickCompletedAt": Value::Null,
            "lastOutcome": "unknown"
        }),
    };
    Json(json!({
        "service": "linggan-local-web",
        "collectionUpgradePhase": linggan_evidence::collection_upgrade_phase(),
        "listener": "loopback-only",
        "dataState": data_state,
        "evidenceReadModel": evidence_read_model,
        "database": {
            "state": database_state,
            "schema": schema_state
        },
        "readiness": {
            "state": readiness.state.code(),
            "detail": readiness.detail,
            "checkedAt": readiness.checked_at
        },
        "scheduler": scheduler,
        "webApp": web_app::health(),
        "routes": {
            "evidenceLibrary": "/corpus/evidence",
            "workResources": "/api/local/work-resources",
            "discoveryIngress": "/api/local/discovery-packages",
            "localProducer": local_producer_routes,
            "station": station_routes,
            "dispatch": dispatch_routes
        }
    }))
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
    let Some(domain_ref) = params.domain else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "domain_required");
    };
    let Ok(query) = material_projection::local_query(&params) else {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_local_evidence_query",
        );
    };
    let domain_exists: bool = match sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)",
    )
    .bind(domain_ref)
    .fetch_one(database.pool())
    .await
    {
        Ok(exists) => exists,
        Err(_) => match linggan_evidence::validate_work_resource_query(database, &query).await {
            Err(WorkResourceReadError::InvalidCursor | WorkResourceReadError::UnsupportedSort) => {
                return local_read_json_error(
                    axum::http::StatusCode::BAD_REQUEST,
                    "invalid_material_query_cursor_or_sort",
                );
            }
            Err(_) | Ok(()) => {
                return local_read_json_error(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "domain_read_unavailable",
                );
            }
        },
    };
    if !domain_exists {
        return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "unknown_domain");
    }
    match material_projection::compose_json(database, &query).await {
        Ok(response) => Json(response).into_response(),
        Err(WorkResourceReadError::InvalidCursor | WorkResourceReadError::UnsupportedSort) => {
            local_read_json_error(
                axum::http::StatusCode::BAD_REQUEST,
                "invalid_material_query_cursor_or_sort",
            )
        }
        Err(WorkResourceReadError::ProjectionUnavailable) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "material_projection_schema_unavailable",
        ),
        Err(WorkResourceReadError::Database(error)) => {
            eprintln!("material read projection unavailable: {error}");
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "material_read_projection_unavailable",
            )
        }
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
        None,
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
    installation_credential: Option<String>,
    #[serde(default)]
    browser_label: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
    /// 最近一次页面结构自检的受限快照，按平台分组。字段**可选**：没有记录时不带它，
    /// 「本机还没检查过」与「检查过、一切正常」是两件事。它认不出形状时不会让报到失败——
    /// 诊断从来不是报到的前提。
    #[serde(default)]
    selector_health: Option<serde_json::Value>,
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
            selector_health: check_in.selector_health.as_ref(),
            installation_credential: check_in.installation_credential.as_deref(),
        },
    )
    .await;
    match result {
        Ok(outcome) => Json(check_in_payload(&outcome)).into_response(),
        Err(linggan_evidence::StationError::Control(
            linggan_evidence::CollectionControlError::InvalidCredential,
        )) => local_read_json_error(
            axum::http::StatusCode::UNAUTHORIZED,
            "installation_credential_invalid",
        ),
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
            station_display_name,
            accepting_tasks,
            superseded,
            credential,
        } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "claimed",
            "claimKind": "claim_window",
            "stationRef": station_ref,
            "stationDisplayName": station_display_name,
            "stationAccepting": accepting_tasks,
            // A reinstall replaces the previous install on the same station rather than
            // registering a second station. The replaced one stays visible on purpose.
            "supersededInstallationRef": superseded,
            "installationCredentialRef": credential.as_ref().map(|value| value.credential_ref),
            "installationCredential": credential.as_ref().map(|value| value.raw_credential.expose_once()),
            "execution": "NOT_STARTED",
        }),
        CheckInOutcome::AwaitingClaim {
            installation_ref,
            credential,
        } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "awaiting_claim",
            "stationRef": serde_json::Value::Null,
            "stationDisplayName": serde_json::Value::Null,
            "stationAccepting": serde_json::Value::Null,
            "installationCredentialRef": credential.as_ref().map(|value| value.credential_ref),
            "installationCredential": credential.as_ref().map(|value| value.raw_credential.expose_once()),
            "execution": "NOT_STARTED",
        }),
        CheckInOutcome::Heartbeat {
            installation_ref,
            station_ref,
            station_display_name,
            accepting_tasks,
            credential,
        } => serde_json::json!({
            "installationRef": installation_ref,
            "state": "heartbeat",
            "stationRef": station_ref,
            "stationDisplayName": station_display_name,
            "stationAccepting": accepting_tasks,
            "installationCredentialRef": credential.as_ref().map(|value| value.credential_ref),
            "installationCredential": credential.as_ref().map(|value| value.raw_credential.expose_once()),
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
        Ok(outcome) => Json(serde_json::json!({
            "installationRef": claim.installation_ref,
            "stationRef": claim.station_ref,
            "stationDisplayName": outcome.station_display_name,
            "stationAccepting": outcome.accepting_tasks,
            "claimKind": "person",
            "supersededInstallationRef": outcome.superseded,
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
    domain_ref: Option<uuid::Uuid>,
    purpose: String,
    #[serde(default)]
    requested_by: Option<String>,
    /// 哪条观察 lane。深度建档与轻巡检的风险与成本不同，因此授权也分开——写死一个 lane
    /// 会让另一条 lane 的申请永远找不到匹配的授权。
    #[serde(default)]
    lane: Option<String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterialDeepeningRequestBody {
    target_ref: uuid::Uuid,
    domain_ref: Option<uuid::Uuid>,
    purpose: String,
    materials: Vec<MaterialDeepeningRequestItem>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MaterialDeepeningRequestItem {
    content_public_ref: uuid::Uuid,
    #[serde(default = "default_comment_limit")]
    comment_limit: i32,
    #[serde(default = "default_reply_expand_limit")]
    reply_expand_limit: i32,
    #[serde(default = "default_true")]
    acquire_media: bool,
    #[serde(default = "default_true")]
    allow_ocr: bool,
    #[serde(default = "default_true")]
    allow_asr: bool,
}

const fn default_comment_limit() -> i32 {
    30
}

const fn default_reply_expand_limit() -> i32 {
    2
}

const fn default_true() -> bool {
    true
}

/// Freeze and queue one exact material-deepening run.
///
/// The endpoint is loopback-only with the rest of this API.  It deliberately accepts public
/// material identities rather than a search such as "latest 12": the selected set must not drift
/// between approval and Work Order creation. It never assigns a station or issues a Lease: the
/// shared claim queue is the only authority that turns queued work into browser execution.
async fn collection_material_deepening(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Ok(request) = serde_json::from_slice::<MaterialDeepeningRequestBody>(&body) else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "material_deepening_request_invalid",
        );
    };
    let Some(domain_ref) = request.domain_ref else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "domain_required",
        );
    };
    let targets: Vec<MaterialDeepeningTarget> = request
        .materials
        .into_iter()
        .map(|material| MaterialDeepeningTarget {
            content_public_ref: material.content_public_ref,
            comment_limit: material.comment_limit,
            reply_expand_limit: material.reply_expand_limit,
            acquire_media: material.acquire_media,
            allow_ocr: material.allow_ocr,
            allow_asr: material.allow_asr,
        })
        .collect();
    let outcome = match request_and_admit_material_targets_for_domain(
        database,
        request.target_ref,
        domain_ref,
        &request.purpose,
        "person",
        &targets,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            let code = match error {
                AcquisitionChainError::UnknownTarget => "unknown_target",
                // 与「状态不允许」分开报：这条的处置是去给目标指定领域，不是等状态流转。
                AcquisitionChainError::TargetDomainUnassigned => "target_domain_unassigned",
                AcquisitionChainError::TargetNotRequestable { .. } => "target_not_requestable",
                AcquisitionChainError::KeywordArchiveIncomplete => "keyword_archive_incomplete",
                AcquisitionChainError::InvalidMaterialTargets => "material_targets_invalid",
                AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { .. } => {
                    "progressive_archive_authorization_too_small"
                }
                AcquisitionChainError::SchemaUnavailable
                | AcquisitionChainError::Database(_)
                | AcquisitionChainError::ProgressiveArchiveAuthorizationMissing
                | AcquisitionChainError::ProgressiveArchivePurposeMismatch
                | AcquisitionChainError::ProgressiveArchiveNotReady { .. } => {
                    "acquisition_chain_unavailable"
                }
            };
            return local_read_json_error(axum::http::StatusCode::UNPROCESSABLE_ENTITY, code);
        }
    };
    let Some(work_order_ref) = outcome.work_order_ref else {
        return Json(json!({
            "requestRef": outcome.request_ref,
            "decisionRef": outcome.decision_ref,
            "domainRef": domain_ref,
            "admission": outcome.outcome.code(),
            "workOrderRef": null,
            "execution": "NOT_STARTED",
        }))
        .into_response();
    };
    Json(json!({
        "requestRef": outcome.request_ref,
        "decisionRef": outcome.decision_ref,
        "domainRef": domain_ref,
        "admission": outcome.outcome.code(),
        "workOrderRef": work_order_ref,
        "materialCount": targets.len(),
        "execution": "QUEUED",
    }))
    .into_response()
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
    let Some(domain_ref) = request.domain_ref else {
        return local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "domain_required",
        );
    };
    let requested_by = request.requested_by.as_deref().unwrap_or("person");
    let lane = request.lane.as_deref().unwrap_or("deep_archive");
    match request_and_admit_for_domain(
        database,
        request.target_ref,
        domain_ref,
        lane,
        &request.purpose,
        requested_by,
    )
    .await
    {
        Ok(result) => Json(serde_json::json!({
            "requestRef": result.request_ref,
            "decisionRef": result.decision_ref,
            "domainRef": domain_ref,
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
                // 与「状态不允许」分开报：这条的处置是去给目标指定领域，不是等状态流转。
                AcquisitionChainError::TargetDomainUnassigned => "target_domain_unassigned",
                AcquisitionChainError::TargetNotRequestable { .. } => "target_not_requestable",
                AcquisitionChainError::KeywordArchiveIncomplete => "keyword_archive_incomplete",
                AcquisitionChainError::InvalidMaterialTargets => "material_targets_invalid",
                AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { .. } => {
                    "progressive_archive_authorization_too_small"
                }
                AcquisitionChainError::ProgressiveArchiveAuthorizationMissing
                | AcquisitionChainError::ProgressiveArchivePurposeMismatch
                | AcquisitionChainError::ProgressiveArchiveNotReady { .. } => {
                    "acquisition_chain_unavailable"
                }
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
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "scheduled_task_not_claimed_by_producer",
        ),
        Err(ProducerRuntimeError::ScheduledLaneAwaitingClaim) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "scheduled_lane_waiting_for_claim",
        ),
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
            // Scheduled task/lease completion was committed with Package and Receipt inside the
            // producer transaction. This route must not create a second completion boundary.
            // Package/Receipt has committed first.  The target projection is then derived from
            // the accepted public author facts; failure is returned as retryable so the browser
            // outbox never reports the submission delivered while the requested target is absent.
            let package = submission.capture_package();
            let target_sync = match sync_target_from_author_profile(
                database,
                package.package_kind(),
                package.platform(),
                package.records(),
            )
            .await
            {
                Ok(target_sync) => target_sync,
                Err(_) => {
                    return local_producer_error(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "author_target_sync_pending",
                    );
                }
            };
            let mut response = serde_json::to_value(outcome).unwrap_or_else(|_| {
                json!({
                    "delivery": "acknowledged"
                })
            });
            if let Some(object) = response.as_object_mut() {
                object.insert(
                    "targetSync".to_owned(),
                    serde_json::to_value(target_sync).unwrap_or_else(|_| {
                        json!({
                            "state": "notApplicable",
                            "reason": "target_sync_serialization_failed"
                        })
                    }),
                );
            }
            Json(response).into_response()
        }
        Err(ProducerRuntimeError::RoutingNotFound) => {
            local_producer_error(axum::http::StatusCode::NOT_FOUND, "attempt_not_found")
        }
        Err(ProducerRuntimeError::AttemptIdentityMismatch) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "attempt_identity_mismatch",
        ),
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "scheduled_submission_lease_not_live",
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
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MediaAcquisitionClaimWire {
    install_key: String,
    installation_credential: String,
}

async fn claim_media_acquisition_route(
    State(state): State<LocalWebState>,
    Json(input): Json<MediaAcquisitionClaimWire>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "producer_not_connected",
        );
    };
    if input.install_key.trim().is_empty() {
        return local_producer_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "media_acquisition_claim_invalid",
        );
    }
    match claim_media_acquisition(database, &input.install_key, &input.installation_credential)
        .await
    {
        Ok(decision) => Json(decision).into_response(),
        Err(linggan_evidence::MediaAcquisitionError::InvalidCredential) => local_producer_error(
            axum::http::StatusCode::UNAUTHORIZED,
            "installation_credential_invalid",
        ),
        Err(_) => local_producer_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "media_acquisition_unavailable",
        ),
    }
}

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
    work_ref: Option<uuid::Uuid>,
    claim_generation: Option<i32>,
    install_key: Option<String>,
    installation_credential: Option<String>,
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
        Ok(download_attempt_ref) => match (
            input.work_ref,
            input.claim_generation,
            input.install_key.as_deref(),
            input.installation_credential.as_deref(),
        ) {
            (Some(work_ref), Some(claim_generation), Some(install_key), Some(credential)) => {
                match record_media_acquisition_failure(
                    database,
                    work_ref,
                    observation_ref,
                    install_key,
                    credential,
                    claim_generation,
                    &input.terminal_reason,
                )
                .await
                {
                    Ok(work) => Json(json!({
                        "downloadAttemptRef": download_attempt_ref,
                        "delivery": "acknowledged",
                        "work": work,
                    }))
                    .into_response(),
                    Err(linggan_evidence::MediaAcquisitionError::InvalidCredential) => {
                        local_producer_error(
                            axum::http::StatusCode::UNAUTHORIZED,
                            "installation_credential_invalid",
                        )
                    }
                    Err(_) => local_producer_error(
                        axum::http::StatusCode::SERVICE_UNAVAILABLE,
                        "media_acquisition_failure_not_recorded",
                    ),
                }
            }
            (Some(_), Some(_), _, _) => local_producer_error(
                axum::http::StatusCode::UNAUTHORIZED,
                "installation_credential_required",
            ),
            _ => Json(json!({
                "downloadAttemptRef": download_attempt_ref,
                "delivery": "acknowledged"
            }))
            .into_response(),
        },
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
        // 会话自称已物化，却没有下载尝试引用——这是**这一行自己的状态**出了问题，不是服务
        // 暂时不可用。两者必须分开说：归到 `media_upload_state_unavailable` 会让插件一直重试
        // 一个重试不好的东西；归到这里，它至少能停下来把这条会话报出来。
        Err(ProducerRuntimeError::MaterializedSessionWithoutDownloadAttempt) => {
            return local_producer_error(
                axum::http::StatusCode::UNPROCESSABLE_ENTITY,
                "media_upload_materialized_without_download_attempt",
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
    match verify_media_file(&temporary_path, &session).or_else(|error| {
        if error.kind() == ErrorKind::NotFound {
            verify_media_file(&final_path, &session)
        } else {
            Err(error)
        }
    }) {
        Ok(()) => {}
        Err(_) => {
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

fn verify_media_file(
    path: &FsPath,
    session: &linggan_evidence::MediaUploadSession,
) -> std::io::Result<()> {
    let file = fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || i64::try_from(metadata.len()).ok() != Some(session.expected_byte_size)
    {
        return Err(Error::new(ErrorKind::InvalidData, "media size mismatch"));
    }
    let mut reader = BufReader::new(file);
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_i64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(i64::try_from(read).map_err(|_| Error::other("media size overflow"))?)
            .ok_or_else(|| Error::other("media size overflow"))?;
        if total > session.expected_byte_size {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "media exceeds declared size",
            ));
        }
        digest.update(&buffer[..read]);
    }
    let actual_hash = digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if total != session.expected_byte_size || actual_hash != session.expected_sha256 {
        return Err(Error::new(ErrorKind::InvalidData, "media hash mismatch"));
    }
    Ok(())
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
        || !(1..=local_media_max_bytes()).contains(&expected_byte_size)
        || mime_type.is_empty()
        || mime_type.len() > 255
        || !mime_type.is_ascii()
        || mime_type.bytes().any(|byte| byte.is_ascii_control())
    {
        return None;
    }
    Some((expected_sha256, mime_type, expected_byte_size))
}

fn media_storage_key(sha256: &str) -> String {
    format!("blobs/{}/{}", &sha256[..2], sha256)
}

fn local_media_max_bytes() -> i64 {
    std::env::var("LINGGAN_LOCAL_MEDIA_MAX_BYTES")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(256 * 1024 * 1024)
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
    classify_database(database).await
}

/// 连上之后的分类：这个进程的本地读取面能不能用。不能用的原因不在这里猜——连接留着，
/// `/health` 的就绪判定会拿它问出确切原因。
async fn classify_database(database: Database) -> LocalDatabaseState {
    if local_discovery_schema_is_ready(&database)
        .await
        .unwrap_or(false)
    {
        LocalDatabaseState::Ready(Arc::new(database))
    } else {
        LocalDatabaseState::SchemaUnavailable(Arc::new(database))
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

/// Collection sub-surface handlers. The section is part of the path and the Operations mode
/// is a query parameter, so every view is a real, shareable, refresh-safe address.
struct CollectionSurfaceReads {
    surface_state: collection::SurfaceState,
}

async fn read_collection_surface(database: &Database) -> CollectionSurfaceReads {
    let counts_read = async { count_targets(database, None).await.ok() };
    let (counts, heartbeat) = tokio::join!(counts_read, read_scheduler_heartbeat(database));
    let scheduler_state = match heartbeat {
        Ok(Some(heartbeat)) if heartbeat.state == "running" => collection::SchedulerState::Running,
        Ok(Some(_)) => collection::SchedulerState::Stale,
        Ok(None) | Err(_) => collection::SchedulerState::Unreadable,
    };
    let surface_state = collection::SurfaceState {
        vacant_stations: None,
        unclaimed_installations: None,
        total_targets: counts.as_ref().map(|counts| counts.total),
        monitoring_targets: counts.as_ref().map(|counts| counts.monitoring),
        archiving_targets: counts.as_ref().map(|counts| counts.archiving),
        scheduler_state,
    };
    CollectionSurfaceReads { surface_state }
}

/// Daily collection work begins with the preserved observation-target workspace.
async fn collection_entry() -> Redirect {
    Redirect::temporary("/collection/targets")
}

fn collection_target_kind_label(kind: &str) -> &'static str {
    match kind {
        "creator" => "博主",
        "keyword" => "关键词",
        _ => "类型未知",
    }
}

fn collection_target_lifecycle_label(state: &str) -> &'static str {
    match state {
        "pending_decision" => "待决定",
        "archiving" => "建档中",
        "archived" => "已建档",
        "monitoring" => "巡检中",
        "paused" => "已暂停",
        "dismissed" => "已停用",
        _ => "状态未知",
    }
}

fn collection_domain_lane_label(lane: &str) -> &'static str {
    match lane {
        "discovery" => "发现",
        "detail" => "详情",
        "comments" => "评论",
        "replies" => "回复",
        "media_slots" => "媒体槽位",
        "media_bytes" => "媒体字节",
        "ocr" => "图片文字识别",
        "asr" => "语音转写",
        _ => "未归类通道",
    }
}

fn collection_domain_lane_state_label(state: &str) -> &'static str {
    match state {
        "UNKNOWN" => "状态未知",
        "NOT_REQUESTED" => "本次未申请",
        "NOT_OBSERVED" => "尚未观察",
        "OBSERVED" => "已完成观察",
        "PARTIAL" => "部分取得",
        "FAILED" => "执行失败",
        "RISK_CONTROL" => "风险控制停止",
        "SEARCHABLE" => "可检索",
        "REMOTE_ONLY" => "仅有远程候选",
        "LOCALIZED" => "已有本地材料",
        "PROCESSING" => "处理中",
        _ => "状态未归类",
    }
}

fn runtime_control_for_page<'a>(
    projection: Option<
        &'a collection::collection_control_surface_view::CollectionControlSurfaceProjection,
    >,
    recovery_lanes: Option<
        &'a [collection::collection_control_surface_view::RuntimeLaneControlView],
    >,
    account_observation_available: bool,
) -> Option<station_view::RuntimeControl<'a>> {
    projection
        .map(|projection| station_view::RuntimeControl {
            lanes: &projection.runtime_lanes,
            resources: &projection.runtime_resources,
            account_observation_available,
        })
        .or_else(|| {
            recovery_lanes.map(|lanes| station_view::RuntimeControl {
                lanes,
                resources: &[],
                account_observation_available,
            })
        })
}

/// COLLECTION-001 · the person-facing station actions on the 执行工位 surface.
///
/// These write local records only — no platform is contacted and no capture begins, which is
/// why this page may carry real buttons while every platform-consuming control stays absent.
///
/// Each one redirects back with 303 so a refresh re-reads the page instead of re-submitting.
const STATION_CHECK_IN_PATH: &str = "/api/local/stations/installations";

/// 把失败原因带回页面。
///
/// 此前这些表单处理用 `let _ =` 吞掉错误后照常跳转：登记一个重名工位会得到一次跳转、
/// 一切如常的假象，而工位并没有建成。「点了没反应」已经够糟，「点了看起来成功了其实没有」
/// 更糟——它会让人以为系统里有一台并不存在的工位。
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
        LeaseError::InvalidLeaseDuration => "lease_duration_invalid",
        LeaseError::NoStation => "work_order_names_no_station",
        LeaseError::StationUnavailable => "station_unavailable",
        LeaseError::FrozenControlMissing => "lease_frozen_control_missing",
        // Not a refusal: every step this order froze has already been done, so it was closed
        // instead of being handed a permit with no work behind it.
        LeaseError::WorkOrderAlreadySatisfied => "work_order_already_satisfied",
        // Also not an ordinary refusal: nothing left in this order can run **yet** — every
        // remaining step is stopped, either on missing execution input or on an exhausted
        // page-read budget. It was cancelled rather than left in the queue to be re-claimed
        // and re-expanded into nothing every round; which of the two stopped a given range —
        // and whether it can come back on its own — lives on the execution-input ledger.
        LeaseError::OnlyStoppedMembersRemain => "work_order_only_stopped_members_remain",
        LeaseError::ControlBlocked { reason_code } => match reason_code.as_str() {
            "domain_paused_or_unscoped" => "domain_paused_or_unscoped",
            "risk_paused" => "risk_paused",
            "station_unavailable" => "station_unavailable",
            "station_not_accepting" => "station_not_accepting",
            "installation_credential_missing" => "installation_credential_missing",
            "plugin_version_unsupported" => "plugin_version_unsupported",
            "installation_stale" => "installation_stale",
            "capability_missing" => "capability_missing",
            "account_unbound" => "account_unbound",
            "account_binding_changed" => "account_binding_changed",
            "account_binding_expired" => "account_binding_expired",
            // Retained solely to render historical dispatch rows written before observation
            // freshness became diagnostic metadata; current capacity evaluation never emits it.
            "account_eligibility_stale" => "account_eligibility_stale",
            "account_cooling" => "account_cooling",
            "account_needs_login" => "account_needs_login",
            "account_restricted" => "account_restricted",
            "account_unknown" => "account_unknown",
            "account_busy" => "account_busy",
            "station_busy" => "station_busy",
            "station_daily_budget_reached" => "station_daily_budget_reached",
            "rule_missing" => "rule_missing",
            "rule_revision_changed" => "rule_revision_changed",
            "monitoring_paused" => "monitoring_paused",
            _ => "capacity_unknown",
        },
        LeaseError::AuthorizationLapsed => "authorization_lapsed",
        LeaseError::RiskPaused { .. } => "risk_paused",
        LeaseError::TaskSpecInvalid(_) => "task_spec_invalid",
        LeaseError::Database(_) => "lease_write_failed",
    }
}

#[derive(serde::Deserialize)]
struct NewTargetForm {
    target_kind: String,
    identity: String,
    /// 这个目标新建时关联的现有领域，由弹窗明确选择。
    domain: Option<String>,
}

/// COLLECTION-001 · 从页面加入一个观察目标。
///
/// 只写本机记录：不访问任何平台，也不会让任何采集开始。加入观察与「开始采集」是两件事，
/// 后者仍然要走申请 → 授权 → 准入 → 工单 → 租约 → 闸门。
/// 回到观察目标列表，把当前领域带回去。
///
/// 建完目标被甩回「全部领域」，人得再切一次才能看到刚建的东西——地址上的领域在提交
/// 那一刻就该跟着走。
fn back_to_targets(domain: Option<&str>, error: Option<&str>) -> String {
    let mut href = "/collection/targets".to_owned();
    let mut sep = '?';
    if let Some(domain) = domain {
        href.push(sep);
        href.push_str(&format!(
            "domain={}",
            target_drawer::percent_encode_component(domain)
        ));
        sep = '&';
    }
    if let Some(error) = error {
        href.push(sep);
        href.push_str(&format!("error={error}"));
    }
    href
}

async fn collection_target_create(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<NewTargetForm>,
) -> Redirect {
    // Target identity is global. The person selects the Domain relation explicitly; a paused or
    // unknown Domain cannot receive a new relation.
    let domain_param = form
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|value| {
            !value.is_empty()
                && !value.eq_ignore_ascii_case(linggan_evidence::observation_domain::ALL_DOMAINS)
        })
        .map(str::to_owned);
    let domain = domain_param
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok());
    // 没选领域就不建。前端也会挡一道，但真正的闸门在这里——前端禁用是提示，不是保证。
    let Some(domain) = domain else {
        return Redirect::to(&back_to_targets(
            domain_param.as_deref(),
            Some("target_domain_required"),
        ));
    };
    let Some(database) = state.database.database() else {
        return Redirect::to(&back_to_targets(
            domain_param.as_deref(),
            Some("read_model_not_connected"),
        ));
    };
    let domain_active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1 AND status='active')",
    )
    .bind(domain)
    .fetch_one(database.pool())
    .await
    .unwrap_or(false);
    if !domain_active {
        return Redirect::to(&back_to_targets(
            domain_param.as_deref(),
            Some("target_domain_unavailable"),
        ));
    }
    let raw = form.identity.trim();
    // 创作者用主页链接就够了——平台 ID 藏在 URL 里，让人自己去扒是把工具的活推给使用者。
    let identity = match form.target_kind.as_str() {
        "creator" => creator_id_from(raw),
        _ => Some(raw.to_owned()),
    };
    let Some(identity) = identity.filter(|value| !value.is_empty()) else {
        return Redirect::to(&back_to_targets(
            domain_param.as_deref(),
            Some("identity_unrecognised"),
        ));
    };
    let intake = collection_intake::TargetIntake {
        platform: linggan_contracts::OPEN_PLATFORM.to_owned(),
        target_kind: form.target_kind.clone(),
        identity,
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
            // 领域已在上面确认过，这里是确定值而不是「可能没有」。
            Some(domain),
        )
        .await
        {
            // 建完回到刚才那个领域，不把人甩回全部领域——新目标就在那里等着看。
            Ok(_) => Redirect::to(&back_to_targets(domain_param.as_deref(), None)),
            Err(_) => Redirect::to(&back_to_targets(
                domain_param.as_deref(),
                Some("store_failed"),
            )),
        },
        Err(_) => Redirect::to(&back_to_targets(
            domain_param.as_deref(),
            Some("identity_unrecognised"),
        )),
    }
}

#[derive(Debug, Deserialize)]
struct MonitorRuleWire {
    return_scope: Option<String>,
    return_domain: Option<String>,
    target_ref: uuid::Uuid,
    expected_revision: i32,
    idempotency_key: uuid::Uuid,
    command_kind: String,
    automatic_enabled: Option<String>,
    creator_follow_details: Option<String>,
    fixed_interval_seconds: Option<String>,
    surface_key: Option<String>,
    ranking_key: Option<String>,
    /// 采样口径。原始字符串，解析在 handler 里做——失败时把人填过的字面值原样送回。
    scroll_rounds: Option<String>,
    top_by_likes: Option<String>,
    published_within_days: Option<String>,
    task_contract_version: Option<String>,
    return_filter: Option<String>,
    return_sort: Option<String>,
    /// 从检查器的规则台点过来的（每条规则各自的暂停／启用），点完要回到那张表。
    ///
    /// 不带这两项的提交来自规则弹窗，点完留在弹窗里看回执。**两个入口对「点完该回哪」的
    /// 答案不同**：在表上点暂停却被丢进一个规则编辑弹窗，人会以为自己误点了别的东西。
    return_drawer: Option<String>,
    return_dtab: Option<String>,
    /// 这次提交是在编辑哪一条口径（或 `new`）。**必须原样带回**：校验失败时服务端按查询串
    /// 重建表单，丢了它面板就回落到「最早那条规则」，于是排序是人选的、版本号却是另一条
    /// 规则的——人改完重试永远撞 `stale_revision`，而界面只说「版本已过期」。
    rule_slot: Option<String>,
}

/// URL 上那个口径参数说的是哪一条规则。
///
/// 只认闭集：五个榜、`primary`（博主的主页目录与迁移留下的那条）、以及 `new`。**不认的值
/// 一律当作「最早那条」**，不悄悄变成新开一条——后者会让一个拼错的链接变成「再加一条规则」，
/// 那是个有副作用的误解。
fn monitor_rule_redirect(
    form: &MonitorRuleWire,
    error: Option<&str>,
    receipt_ref: Option<uuid::Uuid>,
) -> Redirect {
    if form.return_scope.as_deref() == Some("tasks") && form.command_kind == "manual_observe" {
        return Redirect::to(&collection_task_actions::return_path(
            form.target_ref,
            form.return_domain.as_deref(),
            receipt_ref,
            error,
        ));
    }
    // 从规则台点来的，回规则台。
    if let Some(drawer) = form
        .return_drawer
        .as_deref()
        .map(str::trim)
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    {
        let mut pairs = Vec::new();
        if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) =
            form.return_filter.as_deref()
        {
            pairs.push(format!("filter={filter}"));
        }
        if form.return_sort.as_deref() == Some("last") {
            pairs.push("sort=last".to_owned());
        }
        pairs.push(format!("drawer={drawer}"));
        pairs.push(format!(
            "dtab={}",
            match form.return_dtab.as_deref().map(str::trim) {
                Some(tab @ ("patrol" | "works" | "archive" | "comments")) => tab,
                _ => "patrol",
            }
        ));
        // 成功也要说一句。规则台没有回执面板，什么都不说的话「改成功了」与「被拒了」
        // 在页面上长得一模一样——这一行的状态字本来就可能因为别的原因没变。
        pairs.push(format!(
            "error={}",
            error.unwrap_or(match form.command_kind.as_str() {
                "pause" => "monitor_rule_paused",
                "resume" => "monitor_rule_resumed",
                _ => "monitor_rule_saved",
            })
        ));
        return Redirect::to(&format!("/collection/targets?{}", pairs.join("&")));
    }
    let mut params = vec![format!("rule={}", form.target_ref)];
    // 原样带回这次是在编辑哪一条。成功时回执会进一步把面板指向真正写进去的那条规则
    // （见 `read_monitor_rule_panel`）；失败时没有回执，就靠这个参数留在同一个模式上。
    if let Some(
        slot @ ("new" | "most_liked" | "most_collected" | "most_commented" | "latest"
        | "comprehensive" | "primary"),
    ) = form.rule_slot.as_deref().map(str::trim)
    {
        params.push(format!("rule_slot={slot}"));
    }
    if let Some(receipt_ref) = receipt_ref {
        params.push(format!("rule_receipt={receipt_ref}"));
    }
    if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) =
        form.return_filter.as_deref()
    {
        params.push(format!("filter={filter}"));
    }
    if form.return_sort.as_deref() == Some("last") {
        params.push("sort=last".to_owned());
    }
    if let Some(error) = error {
        // Keep the bounded form snapshot only after a rejected command. A successful
        // pause/resume/manual command must reload the server-owned active rule, rather than
        // letting its submitted checkbox values shadow the durable result on the next GET.
        // The current form has only one cadence control; do not retain compatibility fields
        // for hidden modes, calendars, or fallback intervals in the address bar.
        push_rule_query(
            &mut params,
            "rule_automatic_enabled",
            Some(if form.automatic_enabled.is_some() {
                "1"
            } else {
                "0"
            }),
        );
        push_rule_query(
            &mut params,
            "rule_creator_follow_details",
            Some(if form.creator_follow_details.is_some() {
                "1"
            } else {
                "0"
            }),
        );
        push_rule_query(
            &mut params,
            "rule_fixed_interval_seconds",
            form.fixed_interval_seconds.as_deref(),
        );
        push_rule_query(&mut params, "rule_surface_key", form.surface_key.as_deref());
        push_rule_query(&mut params, "rule_ranking_key", form.ranking_key.as_deref());
        push_rule_query(
            &mut params,
            "rule_task_contract_version",
            form.task_contract_version.as_deref(),
        );
        params.push(format!("error={error}"));
    }
    let fragment = format!("#monitor-rule-{}", form.target_ref);
    Redirect::to(&format!(
        "/collection/targets?{}{}",
        params.join("&"),
        fragment
    ))
}

fn push_rule_query(params: &mut Vec<String>, key: &str, value: Option<&str>) {
    let Some(value) = value else {
        return;
    };
    params.push(format!("{key}={}", encode_query_value(value)));
}

fn encode_query_value(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                vec![byte as char]
            } else {
                let hex = format!("%{byte:02X}");
                hex.chars().collect()
            }
        })
        .collect()
}

fn parse_rule_interval(raw: Option<&str>) -> Option<i32> {
    raw?.trim().parse().ok()
}

/// 解析一项采样口径。
///
/// 空白表示「不设这一项」，与 0 是两件事：0 次下拉是一个真实的口径（只看首屏），
/// 不填则是没有约定。解析不出来的也按不填处理——数据库的范围 CHECK 会挡住越界值，
/// 这里不复述那套边界，免得两处规则日后各说各话。
fn parse_sampling_value(raw: Option<&str>) -> Option<i32> {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())?
        .parse()
        .ok()
}

fn monitor_rule_error_code(error: &MonitorRuleCommandError) -> &'static str {
    match error {
        MonitorRuleCommandError::SchemaUnavailable => "read_model_not_connected",
        MonitorRuleCommandError::UnknownTarget => "target_not_found",
        MonitorRuleCommandError::InvalidManualObserveCommand => "target_not_requestable",
        // 只有列表上那个「一起开关」会产生它；单条命令这一路走不到。显式写出而不留兜底：
        // 留了兜底，将来新增错误变体会悄悄变成一句通用文案。
        MonitorRuleCommandError::NotEveryRuleSwitched { .. } => "patrol_toggle_partial",
        MonitorRuleCommandError::ManualObserveNotAdmitted { reason_code } => {
            match reason_code.as_str() {
                "authorization_missing" => "authorization_missing",
                "authorization_expired_or_revoked" => "authorization_expired_or_revoked",
                "station_not_accepting" => "station_not_accepting",
                "account_needs_login" => "account_needs_login",
                _ => "manual_observe_rejected",
            }
        }
        MonitorRuleCommandError::Acquisition(_) => "manual_observe_rejected",
        MonitorRuleCommandError::Lease(error) => lease_error_code(error),
        MonitorRuleCommandError::Database(_) => "database_unavailable",
    }
}

/// Versioned rule commands are the only UI write path for monitoring. The old boolean route is
/// retained only as a compatibility redirect below; it never mutates the target directly.
async fn collection_target_rule_command(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<MonitorRuleWire>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return monitor_rule_redirect(&form, Some("read_model_not_connected"), None);
    };
    let Ok(kind) = (match form.command_kind.as_str() {
        "save_rule" => Ok(MonitorCommandKind::SaveRule),
        "pause" => Ok(MonitorCommandKind::Pause),
        "resume" => Ok(MonitorCommandKind::Resume),
        "stop" => Ok(MonitorCommandKind::Stop),
        "manual_observe" => Ok(MonitorCommandKind::ManualObserve),
        _ => Err(()),
    }) else {
        return monitor_rule_redirect(&form, Some("invalid_mode"), None);
    };
    // **只有存规则才带草稿。** 非 SaveRule 的命令带草稿会被判 `invalid_mode`
    // （`validate_monitor_command` 明确要求 `draft.is_none()`），而弹窗里的「暂停未来自动调度」
    // 与保存共用同一个表单、提交的是同一批字段——于是那个按钮**从来没有生效过**：点下去
    // 只拿到一句「模式不合法」，规则一动不动。
    let draft = if kind != MonitorCommandKind::SaveRule {
        None
    } else {
        let interval = parse_rule_interval(form.fixed_interval_seconds.as_deref());
        let sampling = form.surface_key.as_deref().map(str::trim) == Some("keyword_search");
        // The page has one scheduling control: an anchored fixed interval.
        // Legacy wire fields are deliberately ignored instead of letting a
        // hidden weekday/window/fallback combination create a second cadence.
        Some(MonitorRuleDraft {
            mode: MonitorRuleMode::Fixed,
            automatic_enabled: form.automatic_enabled.is_some(),
            creator_follow_details: form.creator_follow_details.is_some()
                && form.surface_key.as_deref() == Some("creator_profile"),
            run_on_weekdays: true,
            run_on_weekends: true,
            all_day: true,
            window_start_minute: None,
            window_end_minute: None,
            fixed_interval_seconds: interval,
            fallback_interval_seconds: interval
                .unwrap_or(linggan_evidence::DEFAULT_MONITOR_INTERVAL_SECONDS),
            surface_key: form.surface_key.clone().unwrap_or_default(),
            ranking_key: form
                .ranking_key
                .clone()
                .filter(|value| !value.trim().is_empty()),
            // 口径只属于关键词搜索面。创作者主页没有排序也没有「取前 N」可言——即使表单
            // 里塞了值也丢掉，不让它写进去等着被数据库的 CHECK 拒绝。
            scroll_rounds: sampling
                .then(|| parse_sampling_value(form.scroll_rounds.as_deref()))
                .flatten(),
            top_by_likes: sampling
                .then(|| parse_sampling_value(form.top_by_likes.as_deref()))
                .flatten(),
            published_within_days: sampling
                .then(|| parse_sampling_value(form.published_within_days.as_deref()))
                .flatten(),
            task_contract_version: form
                .task_contract_version
                .clone()
                .unwrap_or_else(|| linggan_contracts::PRODUCER_TASK_SPEC_VERSION.to_owned()),
        })
    };
    let command = MonitorRuleCommand {
        target_ref: form.target_ref,
        expected_revision: form.expected_revision,
        idempotency_key: form.idempotency_key,
        kind,
        actor: MonitorCommandActor::Person,
        source: "targets_ui",
        draft,
        // 暂停／恢复／停止作用在表单说的那一条口径上。存规则不看它——新规则落到哪条口径
        // 由草稿里的排序决定。
        slot_key: (kind != MonitorCommandKind::SaveRule)
            .then(|| {
                form.rule_slot
                    .as_deref()
                    .map(str::trim)
                    .filter(|slot| {
                        matches!(
                            *slot,
                            "most_liked"
                                | "most_collected"
                                | "most_commented"
                                | "latest"
                                | "comprehensive"
                                | "primary"
                        )
                    })
                    .map(str::to_owned)
            })
            .flatten(),
    };
    match apply_monitor_rule_command(database, &command).await {
        // **`Ok` 不等于「命令被接受了」。** 这套命令系统里 Rust 的 `Err` 只留给基础设施故障；
        // 「版本过期」「被拒」「重放」都是耐久事实，走 `Ok(receipt)` 带回执。只看 `Ok`/`Err`
        // 的话，规则台上那个按钮点下去撞了 `stale_revision` 也会跳回去一句不说——页面上这一行
        // 状态原封不动、没有任何文字，人分不清「点了没反应」和「点了但被拒」。
        //
        // 弹窗那条路靠 `rule_receipt` 把回执带回去逐字渲染，不需要这个码；规则台没有回执面板，
        // 只有列表那张消息表，所以把结果翻成一个码给它。
        Ok(receipt) => monitor_rule_redirect(
            &form,
            monitor_rule_outcome_code(&receipt),
            Some(receipt.receipt_ref),
        ),
        Err(error) => monitor_rule_redirect(&form, Some(monitor_rule_error_code(&error)), None),
    }
}

/// 回执翻成列表那张消息表认的码。`None` 表示「这次真的改了，没什么要额外说的」。
///
/// 只给**从规则台点过来**的那条路用（带 `return_drawer` 的那种）——弹窗有自己的回执面板，
/// 会把 outcome 与 reason 逐字渲染出来，再叠一个码是重复说同一件事。
fn monitor_rule_outcome_code(
    receipt: &linggan_evidence::MonitorRuleCommandReceipt,
) -> Option<&'static str> {
    match receipt.outcome {
        linggan_evidence::MonitorCommandOutcomeKind::Applied => None,
        linggan_evidence::MonitorCommandOutcomeKind::Replay => {
            Some("monitor_rule_command_replayed")
        }
        linggan_evidence::MonitorCommandOutcomeKind::StaleRevision => {
            Some("monitor_rule_command_stale")
        }
        _ => Some("monitor_rule_command_rejected"),
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

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum TargetArchiveAction {
    Gaps,
}

#[derive(serde::Deserialize)]
struct TargetArchiveForm {
    row_target_ref: uuid::Uuid,
    archive_action: Option<TargetArchiveAction>,
    /// A concrete acquisition Domain selected by the collection navigation context.
    domain_ref: Option<uuid::Uuid>,
    /// Closed list state only. These fields are navigation context, never acquisition input.
    return_domain: Option<String>,
    return_filter: Option<String>,
    return_sort: Option<String>,
}

/// 刚排进队列的那张工单前面还有几个。读不到就不说——编一个数字比不说更糟。
async fn queued_ahead_for(database: &Database, target_ref: uuid::Uuid) -> Option<i64> {
    linggan_evidence::read_target_queue_positions(database, &[target_ref])
        .await
        .ok()?
        .get(&target_ref)
        .filter(|position| position.ready > 0)
        .map(|position| position.ahead)
}

fn target_archive_return_path(form: &TargetArchiveForm, error: Option<&str>) -> String {
    target_archive_return_path_with_queue(form, error, None)
}

fn target_archive_return_path_with_queue(
    form: &TargetArchiveForm,
    error: Option<&str>,
    ahead: Option<i64>,
) -> String {
    let mut pairs = Vec::new();
    if let Some(domain) = form.return_domain.as_deref().filter(|domain| {
        *domain == linggan_evidence::observation_domain::ALL_DOMAINS
            || uuid::Uuid::parse_str(domain).is_ok()
    }) {
        pairs.push(format!("domain={domain}"));
    }
    if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) =
        form.return_filter.as_deref()
    {
        pairs.push(format!("filter={filter}"));
    }
    if matches!(form.return_sort.as_deref(), Some("last")) {
        pairs.push("sort=last".to_owned());
    }
    pairs.push(format!("drawer={}", form.row_target_ref));
    pairs.push("dtab=archive".to_owned());
    if let Some(error) = error {
        pairs.push(format!("error={error}"));
    }
    if let Some(ahead) = ahead {
        pairs.push(format!("ahead={ahead}"));
    }
    format!("/collection/targets?{}#target-archive", pairs.join("&"))
}

/// COLLECTION-001 · 人确认一批作品在平台上已经不存在了。
///
/// 手工解析表单而不是用 `Form<T>`：`serde_urlencoded` 不支持把同名字段收成数组（已知限制，
/// 不是用法问题），而勾选框正是靠重复的 `content_public_ref` 表达「选了哪几篇」。批量入口
/// 已经因为同一个原因手工解析，这里沿用同一套做法而不是为它引一个新依赖。
///
/// 一次判断只写一条结论：不删作品、不改那次读取失败、不触发任何采集。
async fn collection_retire_materials(State(state): State<LocalWebState>, body: Bytes) -> Redirect {
    let mut target_ref: Option<uuid::Uuid> = None;
    let mut contents: Vec<uuid::Uuid> = Vec::new();
    let mut return_domain: Option<String> = None;
    let mut return_filter: Option<String> = None;
    let mut return_sort: Option<String> = None;
    for (key, value) in parse_form_pairs(&body) {
        match key.as_str() {
            "row_target_ref" => target_ref = uuid::Uuid::parse_str(&value).ok(),
            "content_public_ref" => {
                if let Ok(parsed) = uuid::Uuid::parse_str(&value) {
                    contents.push(parsed);
                }
            }
            "return_domain" => return_domain = Some(value),
            "return_filter" => return_filter = Some(value),
            "return_sort" => return_sort = Some(value),
            _ => {}
        }
    }
    let Some(target_ref) = target_ref else {
        return Redirect::to("/collection/targets?error=material_retirement_invalid");
    };
    let form = TargetArchiveForm {
        row_target_ref: target_ref,
        archive_action: None,
        domain_ref: None,
        return_domain,
        return_filter,
        return_sort,
    };
    let Some(database) = state.database.database() else {
        return Redirect::to(&target_retire_return_path(
            &form,
            Some("read_model_not_connected"),
        ));
    };
    if contents.is_empty() {
        // 一篇都没勾就提交，是「什么都没选」而不是「全部确认」。默默当成全选会让人在
        // 一次误点里失去三个判断。
        return Redirect::to(&target_retire_return_path(
            &form,
            Some("material_retirement_empty"),
        ));
    }
    match retire_materials(database, target_ref, &contents, "page_gone").await {
        Ok(0) => Redirect::to(&target_retire_return_path(
            &form,
            Some("material_retirement_none"),
        )),
        Ok(_) => Redirect::to(&target_retire_return_path(
            &form,
            Some("material_retirement_done"),
        )),
        Err(_) => Redirect::to(&target_retire_return_path(
            &form,
            Some("material_retirement_failed"),
        )),
    }
}

/// 回到抽屉里发起这次判断的那一段，而不是回到列表顶部——人做完一个决定要看到它的结果。
fn target_retire_return_path(form: &TargetArchiveForm, error: Option<&str>) -> String {
    let mut pairs = Vec::new();
    if let Some(domain) = form.return_domain.as_deref().filter(|domain| {
        *domain == linggan_evidence::observation_domain::ALL_DOMAINS
            || uuid::Uuid::parse_str(domain).is_ok()
    }) {
        pairs.push(format!("domain={domain}"));
    }
    if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) =
        form.return_filter.as_deref()
    {
        pairs.push(format!("filter={filter}"));
    }
    pairs.push(format!("drawer={}", form.row_target_ref));
    pairs.push("dtab=overview".to_owned());
    if let Some(error) = error {
        pairs.push(format!("error={error}"));
    }
    format!("/collection/targets?{}#archive-problems", pairs.join("&"))
}

/// 行内写入只能回到它出发的列表上下文。这里逐项白名单，而不是接受一个 `return_url`：
/// 既不会把用户送到别的领域，也不会把表单变成开放跳转入口。
fn target_list_return_path(
    return_domain: Option<&str>,
    return_filter: Option<&str>,
    return_sort: Option<&str>,
    return_focus: Option<&str>,
    delete: Option<uuid::Uuid>,
    error: &str,
) -> String {
    let mut pairs = Vec::new();
    if let Some(domain) = return_domain.filter(|domain| {
        *domain == linggan_evidence::observation_domain::ALL_DOMAINS
            || uuid::Uuid::parse_str(domain).is_ok()
    }) {
        pairs.push(format!("domain={domain}"));
    }
    if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) = return_filter {
        pairs.push(format!("filter={filter}"));
    }
    if return_sort == Some("last") {
        pairs.push("sort=last".to_owned());
    }
    if let Some(target_ref) = delete {
        pairs.push(format!("delete={target_ref}"));
    }
    pairs.push(format!("error={error}"));
    let mut path = format!("/collection/targets?{}", pairs.join("&"));
    if let Some(focus) = return_focus
        .and_then(|value| value.strip_prefix("target-"))
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    {
        path.push_str(&format!("#target-{focus}"));
    }
    path
}

async fn collection_target_assign_domain(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Redirect {
    let mut target_ref: Option<uuid::Uuid> = None;
    let mut domain_ref: Option<uuid::Uuid> = None;
    let mut return_filter: Option<String> = None;
    let mut return_sort: Option<String> = None;
    let mut return_domain: Option<String> = None;
    let mut return_focus: Option<String> = None;
    for (key, value) in parse_form_pairs(&body) {
        match key.as_str() {
            "row_target_ref" => target_ref = uuid::Uuid::parse_str(&value).ok(),
            "domain_ref" => domain_ref = uuid::Uuid::parse_str(&value).ok(),
            "return_filter" => return_filter = Some(value),
            "return_sort" => return_sort = Some(value),
            "return_domain" => return_domain = Some(value),
            "return_focus" => return_focus = Some(value),
            _ => {}
        }
    }
    let back = |code| {
        target_list_return_path(
            return_domain.as_deref(),
            return_filter.as_deref(),
            return_sort.as_deref(),
            return_focus.as_deref(),
            None,
            code,
        )
    };
    let (Some(target_ref), Some(domain_ref)) = (target_ref, domain_ref) else {
        return Redirect::to(&back("target_domain_assignment_invalid"));
    };
    let Some(database) = state.database.database() else {
        return Redirect::to(&back("read_model_not_connected"));
    };
    let code = match assign_target_domain(database, target_ref, domain_ref).await {
        Ok(
            linggan_evidence::TargetDomainAssignmentOutcome::Assigned
            | linggan_evidence::TargetDomainAssignmentOutcome::AlreadyAssigned,
        ) => "target_domain_assigned",
        Err(linggan_evidence::CollectionTargetError::DomainAlreadyAssigned) => {
            "target_domain_already_assigned"
        }
        Err(linggan_evidence::CollectionTargetError::UnknownDomain) => "target_domain_unavailable",
        Err(linggan_evidence::CollectionTargetError::UnknownTarget) => {
            "target_domain_target_missing"
        }
        Err(_) => "target_domain_assignment_failed",
    };
    Redirect::to(&back(code))
}

/// COLLECTION-001 · 从列表上开关一个目标的自动巡查。
///
/// 停止观察不是删除：语料与档案全部保留，已经在跑的活跑完，只是不再排新的巡检。
async fn collection_target_patrol_toggle(
    State(state): State<LocalWebState>,
    body: Bytes,
) -> Redirect {
    let mut target_ref: Option<uuid::Uuid> = None;
    let mut enable = false;
    let mut return_filter: Option<String> = None;
    let mut return_sort: Option<String> = None;
    let mut return_domain: Option<String> = None;
    let mut return_focus: Option<String> = None;
    for (key, value) in parse_form_pairs(&body) {
        match key.as_str() {
            "row_target_ref" => target_ref = uuid::Uuid::parse_str(&value).ok(),
            "enable" => enable = value == "true",
            "return_filter" => return_filter = Some(value),
            "return_sort" => return_sort = Some(value),
            "return_domain" => return_domain = Some(value),
            "return_focus" => return_focus = Some(value),
            _ => {}
        }
    }
    let Some(target_ref) = target_ref else {
        return Redirect::to("/collection/targets?error=patrol_toggle_invalid");
    };
    let back = |code| {
        target_list_return_path(
            return_domain.as_deref(),
            return_filter.as_deref(),
            return_sort.as_deref(),
            return_focus.as_deref(),
            None,
            code,
        )
    };
    let Some(database) = state.database.database() else {
        return Redirect::to(&back("read_model_not_connected"));
    };
    match toggle_target_patrol(database, target_ref, enable).await {
        Ok(_) if enable => Redirect::to(&back("patrol_resumed")),
        Ok(_) => Redirect::to(&back("patrol_paused")),
        Err(MonitorRuleCommandError::UnknownTarget) => Redirect::to(&back("patrol_toggle_no_rule")),
        Err(MonitorRuleCommandError::Acquisition(
            AcquisitionChainError::KeywordArchiveIncomplete,
        )) => Redirect::to(&back("keyword_archive_incomplete")),
        Err(MonitorRuleCommandError::Acquisition(AcquisitionChainError::SchemaUnavailable)) => {
            Redirect::to(&back("keyword_patrol_archive_unreadable"))
        }
        // 一起开关时有规则没翻过来——多半是那一条在这一页打开之后被改过（另一个标签页，
        // 或者一次巡检推进了它的版本）。不报成功：目标行是诚实的，撒谎的会是那句横幅。
        Err(MonitorRuleCommandError::NotEveryRuleSwitched { .. }) => {
            Redirect::to(&back("patrol_toggle_partial"))
        }
        Err(_) => Redirect::to(&back("patrol_toggle_failed")),
    }
}

/// COLLECTION-001 · 彻底删除一个观察目标。
///
/// 删掉的是控制面——「我要盯着这个人」这个决定，以及它产生的申请、准入、工单、租约。
/// 采集事实（作品、详情、评论、采集包、回执）在数据库层禁止删除，也不需要跟着走：作者
/// 归属推自那些 append-only 事实，删完之后作品仍然属于这个博主、仍然检索得到。
///
/// 要求把名字原样打一遍。不可逆的操作，点两下太容易了。
async fn collection_target_delete(State(state): State<LocalWebState>, body: Bytes) -> Redirect {
    let mut target_ref: Option<uuid::Uuid> = None;
    let mut confirm_name = String::new();
    let mut return_filter: Option<String> = None;
    let mut return_sort: Option<String> = None;
    let mut return_domain: Option<String> = None;
    let mut return_focus: Option<String> = None;
    for (key, value) in parse_form_pairs(&body) {
        match key.as_str() {
            "row_target_ref" => target_ref = uuid::Uuid::parse_str(&value).ok(),
            "confirm_name" => confirm_name = value,
            "return_filter" => return_filter = Some(value),
            "return_sort" => return_sort = Some(value),
            "return_domain" => return_domain = Some(value),
            "return_focus" => return_focus = Some(value),
            _ => {}
        }
    }
    let Some(target_ref) = target_ref else {
        return Redirect::to(&target_list_return_path(
            return_domain.as_deref(),
            return_filter.as_deref(),
            return_sort.as_deref(),
            return_focus.as_deref(),
            None,
            "target_delete_invalid",
        ));
    };
    let back = |delete, error| {
        target_list_return_path(
            return_domain.as_deref(),
            return_filter.as_deref(),
            return_sort.as_deref(),
            return_focus.as_deref(),
            delete,
            error,
        )
    };
    let Some(database) = state.database.database() else {
        return Redirect::to(&back(Some(target_ref), "read_model_not_connected"));
    };
    match delete_observation_target(database, target_ref, &confirm_name).await {
        Ok(TargetDeletionOutcome::Deleted) => Redirect::to(&back(None, "target_deleted")),
        Ok(TargetDeletionOutcome::NameMismatch) => {
            Redirect::to(&back(Some(target_ref), "target_delete_name_mismatch"))
        }
        Ok(TargetDeletionOutcome::BlockedByProtectedFacts { .. }) => {
            Redirect::to(&back(Some(target_ref), "target_delete_blocked"))
        }
        Ok(TargetDeletionOutcome::UnknownTarget) => {
            Redirect::to(&back(None, "target_delete_missing"))
        }
        Err(_) => Redirect::to(&back(Some(target_ref), "target_delete_failed")),
    }
}

/// COLLECTION-001 · 切换一个观察目标的巡检开关。
///
/// 只写本机记录：打开巡检不等于立刻采集——调度器仍要按间隔到期、准入仍要过六问、
/// 额度与风险暂停仍然管用。
async fn collection_target_toggle_monitoring(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<MonitoringForm>,
) -> Redirect {
    let _ = state;
    // The pre-Package-2 boolean endpoint must not remain a second authority for monitoring.
    // Send old bookmarks to the versioned control surface where expected_revision and
    // idempotency are required; no state is changed by this compatibility path.
    Redirect::to(&format!(
        "/collection/targets?rule={}&error=rule_command_required#monitor-rule-{}",
        form.row_target_ref, form.row_target_ref
    ))
}

/// COLLECTION-001 · 从页面发起一次深度建档。
///
/// 它**不绕过授权链**：走的是与定时巡检、与 API 完全相同的一条路——申请、准入六问、
/// 工单、租约。按钮只是把「人现在想要这个」表达出来，能不能做仍由准入回答。
///
/// 失败原因原样带回页面：没有覆盖深度建档的授权、目标已在建档中、工位不在岗，这三种
/// 情况的处置完全不同，压成一句「失败」等于让人自己去猜。
#[derive(serde::Deserialize)]
struct MonitorRuleRetireForm {
    row_target_ref: uuid::Uuid,
    rule_ref: uuid::Uuid,
    return_filter: Option<String>,
    return_sort: Option<String>,
}

/// COLLECTION-001 · 停用一条巡检规则。
///
/// **不删**：它签发过的工单与材料还挂在它的版本上。停用只是不再排期。
async fn collection_monitor_rule_retire(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<MonitorRuleRetireForm>,
) -> Redirect {
    let code = match state.database.database() {
        None => "read_model_not_connected",
        Some(database) => match linggan_evidence::retire_monitor_rule(
            database,
            form.row_target_ref,
            form.rule_ref,
        )
        .await
        {
            Ok(()) => "monitor_rule_retired",
            Err(linggan_evidence::MonitorRuleRetireError::UnknownRule) => "monitor_rule_unknown",
            Err(linggan_evidence::MonitorRuleRetireError::LastRuleOfAMonitoredTarget) => {
                "monitor_rule_is_the_last_one"
            }
            Err(_) => "monitor_rule_retire_failed",
        },
    };
    let mut pairs = Vec::new();
    if let Some(filter @ ("creator" | "keyword" | "archiving" | "monitoring")) =
        form.return_filter.as_deref()
    {
        pairs.push(format!("filter={filter}"));
    }
    if matches!(form.return_sort.as_deref(), Some("last")) {
        pairs.push("sort=last".to_owned());
    }
    pairs.push(format!("drawer={}", form.row_target_ref));
    pairs.push("dtab=patrol".to_owned());
    pairs.push(format!("error={code}"));
    Redirect::to(&format!("/collection/targets?{}", pairs.join("&")))
}

/// 一个关键词**建过档没有**：列表行、抽屉和建档按钮共用的一份判据。
///
/// 按钮上写什么由它决定，按下之后该走哪一段也由它决定。读会发生两次——点击在渲染之后，
/// 这一点改不掉——但两次读的必须是同一份判据：两处**各判一次**，迟早会出一件「行上说还
/// 没建档，点下去却在补详情」的事。读不到就如实返回 `Unavailable`，由调用方决定怎么说；
/// 这里不把它压成任何一种已知状态，压成哪一种都是替人做主。
async fn read_keyword_archive(
    database: &Database,
    target_ref: uuid::Uuid,
) -> target_drawer::KeywordArchiveRead {
    let refs = [target_ref];
    let archived = keyword_baselines_qualified(database, &refs).await.ok();
    // 建档分两段：先拿链接，再补详情。第二段是不是还欠着，同样要问。
    let pending = linggan_evidence::keyword_targets_pending_detail(database, &refs)
        .await
        .ok();
    collection_targets_view::keyword_archive_read(archived.as_ref(), pending.as_ref(), target_ref)
}

/// 采集链路回一个错误时，这一下该说哪一句话。**两段共用这一处。**
///
/// 「当前状态不接受这次请求」与「请求没送出去」是两件事，处置也相反：前者重试一辈子都
/// 一样，后者过一会儿就好。此前两者共用 `archive_unavailable`，于是永久的状态前置被写成
/// 了「稍后可以重试」。
///
/// 分类写在两个分支里，就一定会有一段漏掉：第一版只给第一段分了类，第二段遇到同一个拒绝
/// （关键词被排除之后，详情补采同样不被准入接受）仍然说「稍后可以重试」。所以这里连同兜底
/// 一起收进来——调用方不再自己 match，也就没有再漏一处的地方。
fn keyword_archive_error_receipt(error: &AcquisitionChainError) -> &'static str {
    match error {
        AcquisitionChainError::UnknownTarget => "keyword_archive_target_missing",
        // 缺领域要说清是缺领域。它此前落进一句「上一次动作没有完成」——信息量最低的那条
        // 兜底文案，既不说原因也不说下一步。
        AcquisitionChainError::TargetDomainUnassigned => "target_domain_unassigned",
        // 准入按状态拒绝：这是前置，不是暂时故障。写「稍后重试」等于让人反复做一件永远
        // 不会成的事。
        AcquisitionChainError::TargetNotRequestable { .. } => "keyword_archive_not_requestable",
        AcquisitionChainError::KeywordArchiveIncomplete => "keyword_archive_incomplete",
        // 其余（目标已经不在了、schema 没就绪、数据库故障）是真的没送出去，如实说。
        _ => "archive_unavailable",
    }
}

fn creator_archive_error_receipt(error: &RequestLeaseError) -> &'static str {
    match error {
        RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { .. },
        ) => "archive_authorization_below_200",
        RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveAuthorizationMissing,
        ) => "archive_refuse",
        RequestLeaseError::Acquisition(AcquisitionChainError::ProgressiveArchiveNotReady {
            reason: "detail_batch_in_flight",
        }) => "archive_in_progress",
        RequestLeaseError::Acquisition(AcquisitionChainError::ProgressiveArchiveNotReady {
            reason: "no_missing_accepted_work",
        }) => "archive_nothing_to_continue",
        RequestLeaseError::Acquisition(AcquisitionChainError::ProgressiveArchiveNotReady {
            reason: "archive_baseline_complete",
        }) => "archive_baseline_complete",
        RequestLeaseError::Acquisition(AcquisitionChainError::ProgressiveArchiveNotReady {
            reason: "detail_gap_not_schedulable",
        }) => "archive_gap_not_schedulable",
        RequestLeaseError::Acquisition(AcquisitionChainError::TargetDomainUnassigned) => {
            "target_domain_unassigned"
        }
        RequestLeaseError::Acquisition(AcquisitionChainError::UnknownTarget) => {
            "archive_target_missing"
        }
        RequestLeaseError::Acquisition(AcquisitionChainError::TargetNotRequestable { .. }) => {
            "archive_not_requestable"
        }
        RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchivePurposeMismatch,
        ) => "archive_purpose_mismatch",
        RequestLeaseError::Lease(LeaseError::Database(_) | LeaseError::SchemaUnavailable) => {
            "archive_request_failed"
        }
        RequestLeaseError::Lease(_) => "archive_lease_failed",
        RequestLeaseError::Acquisition(
            AcquisitionChainError::SchemaUnavailable
            | AcquisitionChainError::KeywordArchiveIncomplete
            | AcquisitionChainError::ProgressiveArchiveNotReady { .. }
            | AcquisitionChainError::InvalidMaterialTargets
            | AcquisitionChainError::Database(_),
        ) => "archive_request_failed",
    }
}

fn creator_archive_database_kind(error: &sqlx::Error) -> &'static str {
    match error {
        sqlx::Error::Database(_) => "database_server",
        sqlx::Error::RowNotFound => "database_row_not_found",
        sqlx::Error::PoolTimedOut => "database_pool_timeout",
        sqlx::Error::PoolClosed => "database_pool_closed",
        sqlx::Error::Io(_) => "database_io",
        sqlx::Error::Protocol(_) => "database_protocol",
        _ => "database_other",
    }
}

fn log_creator_archive_failure(
    error: &RequestLeaseError,
    target_ref: uuid::Uuid,
    filling_gaps: bool,
    failure_ref: uuid::Uuid,
) {
    let (kind, sqlstate, constraint) = match error {
        RequestLeaseError::Acquisition(AcquisitionChainError::Database(error)) => (
            creator_archive_database_kind(error),
            error.as_database_error().and_then(|value| value.code()),
            error
                .as_database_error()
                .and_then(|value| value.constraint()),
        ),
        RequestLeaseError::Acquisition(AcquisitionChainError::SchemaUnavailable) => {
            ("schema_unavailable", None, None)
        }
        RequestLeaseError::Acquisition(AcquisitionChainError::ProgressiveArchiveNotReady {
            ..
        }) => ("unrecognized_not_ready", None, None),
        RequestLeaseError::Acquisition(AcquisitionChainError::KeywordArchiveIncomplete) => {
            ("keyword_archive_incomplete", None, None)
        }
        RequestLeaseError::Acquisition(AcquisitionChainError::InvalidMaterialTargets) => {
            ("invalid_material_targets", None, None)
        }
        RequestLeaseError::Lease(LeaseError::Database(error)) => (
            creator_archive_database_kind(error),
            error.as_database_error().and_then(|value| value.code()),
            error
                .as_database_error()
                .and_then(|value| value.constraint()),
        ),
        RequestLeaseError::Lease(LeaseError::SchemaUnavailable) => {
            ("lease_schema_unavailable", None, None)
        }
        RequestLeaseError::Lease(_) => ("lease", None, None),
        _ => ("acquisition", None, None),
    };
    eprintln!(
        "creator archive request failed: failure_ref={failure_ref} target_ref={target_ref} action={} kind={kind} sqlstate={sqlstate:?} constraint={constraint:?}",
        if filling_gaps { "gaps" } else { "archive" },
    );
}

async fn collection_target_deep_archive(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<TargetArchiveForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to(&target_archive_return_path(
            &form,
            Some("read_model_not_connected"),
        ));
    };
    // 建档对两类目标是同一个用户意图，走的却必须是两条链。
    //
    // 创作者走**渐进式建档**：作品数可能上千，要分批、可续跑，并且需要一份足够大的授权。
    // 关键词没有这回事——它的建档就是把搜索面按点赞翻一遍，一轮就该结束；把它塞进渐进式
    // 那条链，只会因为「拿不到 200 篇的渐进授权」而被拒，而那条拒绝对关键词毫无意义。
    let target_kind: Option<String> = sqlx::query_scalar(
        "SELECT target_kind FROM collection_observation_target WHERE target_ref=$1",
    )
    .bind(form.row_target_ref)
    .fetch_optional(database.pool())
    .await
    .ok()
    .flatten();
    if target_kind.as_deref() == Some("keyword") {
        // 关键词建档也是两段：**先拿链接，再补详情**，与创作者渐进式建档同一个形状。
        // 同一个按钮按所处的段落做不同的事，而「现在到哪一段」不是在这里再猜一次的：
        // 它就是列表行和抽屉决定这颗按钮叫什么时读的那份事实。
        //
        // 此前这里是「先试第二段，挑不出候选就当完成」——一个还没翻过搜索面的词，第二段
        // 必然挑不出候选，于是它回一句「详情已补齐」就结束，**第一段（真正该跑的那一段）
        // 永远发不出去**。空的候选集有两种意思：「都补完了」和「一条链接都还没有」，
        // 只有建档态分得开它们。
        return match read_keyword_archive(database, form.row_target_ref).await {
            // 两段都做完了：这一单没有可发的采集。如实说，并指出下一步是开始每周巡检。
            target_drawer::KeywordArchiveRead::Complete => Redirect::to(
                &target_archive_return_path(&form, Some("keyword_detail_complete")),
            ),
            // 第二段：链接有了、详情还欠着，补下一批。
            target_drawer::KeywordArchiveRead::DetailPending => {
                let advanced = if let Some(domain_ref) = form.domain_ref {
                    linggan_evidence::advance_keyword_archive_detail_for_domain(
                        database,
                        form.row_target_ref,
                        domain_ref,
                        "从观察目标页发起关键词历史建档",
                        "person",
                    )
                    .await
                } else {
                    linggan_evidence::advance_keyword_archive_detail(
                        database,
                        form.row_target_ref,
                        "从观察目标页发起关键词历史建档",
                        "person",
                    )
                    .await
                };
                Redirect::to(&match advanced {
                    Ok(linggan_evidence::KeywordDetailAdvance::Queued { .. }) => {
                        let ahead = queued_ahead_for(database, form.row_target_ref).await;
                        target_archive_return_path_with_queue(
                            &form,
                            Some("keyword_detail_requested"),
                            ahead,
                        )
                    }
                    Ok(linggan_evidence::KeywordDetailAdvance::Skipped(
                        "detail_batch_in_flight",
                    )) => target_archive_return_path(&form, Some("archive_in_progress")),
                    // 建档态说还欠详情，来挑的时候却没有可挑的了：缺口在这两步之间合上
                    // （补采 tick 刚跑完，或那一批已经排在在途工单上）。**这一句在这里才是
                    // 真的**——它上面已经确认过链接是有的。
                    Ok(linggan_evidence::KeywordDetailAdvance::Skipped("no_missing_detail")) => {
                        target_archive_return_path(&form, Some("keyword_detail_complete"))
                    }
                    // 准入没放行时把它的结论原样带回去，与下面那一段同一种处理。
                    Ok(linggan_evidence::KeywordDetailAdvance::Skipped(code)) => {
                        target_archive_return_path(&form, Some(&format!("archive_{code}")))
                    }
                    // 第二段读不出来就去发第一段？不行：建档态已经说过这个词的链接是有的，
                    // 为一次读失败再去翻一遍搜索面，等于为一个已知存在的底座又真实访问一次
                    // 平台。这里是「没做成」，如实说，不换一件更大的事做。
                    Err(error) => target_archive_return_path(
                        &form,
                        Some(keyword_archive_error_receipt(&error)),
                    ),
                })
            }
            // 第一段：搜索面还没翻完。**这一段不经过第二段**——一个作品链接都还没有的词，
            // 第二段能问出来的只有「没有可补的详情」，那不是「已补齐」。
            target_drawer::KeywordArchiveRead::NotArchived => {
                let requested = if let Some(domain_ref) = form.domain_ref {
                    linggan_evidence::request_and_admit_for_domain(
                        database,
                        form.row_target_ref,
                        domain_ref,
                        "deep_archive",
                        "从观察目标页发起关键词历史建档",
                        "person",
                    )
                    .await
                } else {
                    linggan_evidence::request_and_admit(
                        database,
                        form.row_target_ref,
                        "deep_archive",
                        "从观察目标页发起关键词历史建档",
                        "person",
                    )
                    .await
                };
                Redirect::to(&match requested {
                    Ok(outcome) if outcome.work_order_ref.is_some() => {
                        target_archive_return_path_with_queue(
                            &form,
                            Some("archive_requested"),
                            queued_ahead_for(database, form.row_target_ref).await,
                        )
                    }
                    // 准入没放行时**把它的结论原样带回去**，与创作者那一路同一种处理：
                    // refuse（没有授权）、defer（暂时没有执行资源）、merge（已经有在途的同类
                    // 工作）后果完全不同。压成一句「没有授权」会让人去申请一份根本不缺的授权，
                    // 而真实情况——已经在建了——完全没有传达。
                    Ok(outcome) => target_archive_return_path(
                        &form,
                        Some(&format!("archive_{}", outcome.outcome.code())),
                    ),
                    Err(error) => target_archive_return_path(
                        &form,
                        Some(keyword_archive_error_receipt(&error)),
                    ),
                })
            }
            // 建档态读不到就不发采集：这里**没有更安全的默认分支**——当成没建过会让人重做
            // 一次真实的平台访问，当成建好了会把一个半成品底座推进巡检。两件都不做。
            target_drawer::KeywordArchiveRead::Unavailable => Redirect::to(
                &target_archive_return_path(&form, Some("keyword_archive_unreadable")),
            ),
        };
    }
    let filling_gaps = matches!(form.archive_action, Some(TargetArchiveAction::Gaps));
    let outcome = if filling_gaps {
        let requested = if let Some(domain_ref) = form.domain_ref {
            linggan_evidence::request_creator_directory_gaps_for_domain(
                database,
                form.row_target_ref,
                domain_ref,
                "从观察目标页发起深度建档",
                "person",
            )
            .await
        } else {
            linggan_evidence::request_creator_directory_gaps(
                database,
                form.row_target_ref,
                "从观察目标页发起深度建档",
                "person",
            )
            .await
        };
        requested.map_err(RequestLeaseError::Acquisition)
    } else {
        if let Some(domain_ref) = form.domain_ref {
            linggan_evidence::request_progressive_archive_for_domain(
                database,
                form.row_target_ref,
                domain_ref,
                "从观察目标页发起深度建档",
                "person",
            )
            .await
        } else {
            request_progressive_archive(
                database,
                form.row_target_ref,
                "从观察目标页发起深度建档",
                "person",
            )
            .await
        }
    };
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(error) => {
            let code = creator_archive_error_receipt(&error);
            if code == "archive_request_failed" {
                let failure_ref = uuid::Uuid::new_v4();
                log_creator_archive_failure(&error, form.row_target_ref, filling_gaps, failure_ref);
                let receipt = format!("archive_request_failed_{failure_ref}");
                return Redirect::to(&target_archive_return_path(&form, Some(&receipt)));
            }
            return Redirect::to(&target_archive_return_path(&form, Some(code)));
        }
    };
    let Some(_work_order_ref) = outcome.work_order_ref else {
        // 准入没通过。把它的结论原样带回去——refuse 与 defer 的处置完全不同。
        let code = format!("archive_{}", outcome.outcome.code());
        return Redirect::to(&target_archive_return_path(&form, Some(&code)));
    };
    Redirect::to(&target_archive_return_path(
        &form,
        Some(if filling_gaps {
            "creator_detail_requested"
        } else {
            "archive_requested"
        }),
    ))
}

/// COLLECTION-001 · 对勾选的来源做批量操作。
///
/// 手工解析表单而不是用 `Form<T>`：`serde_urlencoded` **不支持同名字段收成数组**
/// （已知限制，不是用法问题），而勾选框正是靠重复的 `target_ref` 表达「选了哪几行」。
/// 与其为此引一个新依赖，不如就地解析这一个请求。
///
/// 规则只由目标级的版本化命令保存；批量入口不改巡检状态。分组只写本机记录，
/// 不影响任何采集行为——它是人自己的分类方式。
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
        "set_group" => set_group_for_many(database, &target_refs, group_name.as_deref()).await,
        _ => return Redirect::to("/collection/targets?error=batch_unknown_action"),
    };
    if outcome.is_err() {
        return Redirect::to("/collection/targets?error=batch_failed");
    }
    Redirect::to("/collection/targets")
}

#[cfg(test)]
mod tests;
