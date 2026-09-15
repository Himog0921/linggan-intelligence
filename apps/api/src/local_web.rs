mod collection;
mod collection_dispatch;
mod collection_intake;
mod collection_targets_view;
mod collection_tasks_view;
mod comment_research;
#[cfg(test)]
#[path = "../../../crates/evidence/tests/support/material_fixture.rs"]
mod comment_research_api_fixture;
mod creator_lifecycle_api;
#[cfg(test)]
mod creator_lifecycle_tests;
#[cfg(test)]
mod evidence_page;
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
mod shell;
mod station_view;
mod target_drawer;
#[cfg(test)]
mod target_inspector_performance_tests;
mod topic_workspace;
#[cfg(test)]
mod topic_workspace_tests;

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
use linggan_evidence::cross_industry_read::{
    CrossIndustryReadError, read_cross_industry_comments, read_cross_industry_samples,
};
use linggan_evidence::observation_domain::{
    ObservationDomain, read_observation_domains, resolve_current_domain,
};
use linggan_evidence::{
    AcquisitionChainError, AuthorizationGrant, CheckInOutcome, CreatorLifecycleQuery,
    DiscoveryIngressError, InstallationCheckIn, LeaseError, LocalAttemptOutcome,
    LocalProducerError, LocalSubmissionOutcome, LocalTaskOutcome, MaterialDeepeningTarget,
    MediaUploadFinalizeClaim, MonitorCommandActor, MonitorCommandKind, MonitorRuleCommand,
    MonitorRuleCommandError, MonitorRuleDraft, MonitorRuleMode, ObservationTarget,
    ObservationTargetAvatar, ProducerRuntimeError, RequestLeaseError, RuntimeAttemptOutcome,
    RuntimeCapacityOverview, RuntimeSubmissionOutcome, RuntimeTaskOutcome, StationCapability,
    StationOverview, StoreOutcome, TargetCounts, TargetDeletionOutcome, UnclaimedInstallation,
    WorkResourceReadError, admit_media_blob, apply_monitor_rule_command, begin_media_upload,
    bind_observation_account, check_in_installation, claim_installation, claim_media_acquisition,
    claim_media_upload_finalize, close_claim_window, complete_media_upload, count_targets,
    create_manual_task, create_producer_task, delete_observation_target, dispatch_schema_is_ready,
    grant_authorization, ingest_discovery_package, issue_work_order_lease,
    keyword_baselines_qualified, list_targets, list_targets_in_state,
    local_discovery_schema_is_ready, local_producer_schema_is_ready,
    media_acquisition_schema_is_ready, open_claim_window, producer_runtime_has_packages,
    producer_runtime_schema_is_ready, read_archive_completeness, read_blocked_materials,
    read_collection_task_timeline, read_creator_directory, read_creator_lifecycle,
    read_cross_industry_hits, read_discovery_library, read_keyword_hits, read_media_upload_session,
    read_runtime_capacity, read_runtime_library, read_scheduler_heartbeat,
    read_station_capabilities, read_station_overview, read_target, read_target_avatars,
    read_target_deletion_preview, read_target_inspector, read_target_observation_summaries,
    record_media_acquisition_failure, record_media_download_failure, record_media_upload_chunk,
    register_station, release_media_upload_finalize, rename_station, request_and_admit,
    request_and_admit_material_targets, request_progressive_archive, retire_materials,
    retire_station, set_group_for_many, set_station_accepting, start_local_attempt,
    start_producer_attempt, station_schema_is_ready, store_pending_target, submit_local_package,
    submit_producer_package, sync_target_from_author_profile, toggle_target_patrol,
};
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
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
const LIDS_TOKENS: &str = include_str!("local_web/lids_tokens.css");
const SHELL_CSS: &str = include_str!("local_web/shell.css");
const COLLECTION_WORKSPACE_CSS: &str = include_str!("local_web/collection_workspace.css");
const TARGET_DRAWER_CSS: &str = include_str!("local_web/target_drawer.css");
const COLLECTION_WORKSPACE_JS: &str = include_str!("local_web/collection_workspace.js");
const EVIDENCE_LIBRARY_CSS: &str = include_str!("local_web/evidence_library.css");
const EVIDENCE_OBSERVATION_JS: &str = include_str!("local_web/evidence_observation.js");
const EVIDENCE_LIBRARY_JS: &str = include_str!("local_web/evidence_library.js");
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
        .merge(creator_lifecycle_api::routes())
        .merge(topic_workspace::routes())
        .merge(comment_research::routes())
        .merge(model_settings::routes())
        .route("/corpus", get(corpus_entry))
        .route("/corpus/evidence", get(evidence_library))
        .route("/collection", get(collection_entry))
        .route("/collection/targets", get(collection_targets))
        .route("/collection/targets/new", post(collection_target_create))
        .route(
            "/collection/targets/rules",
            post(collection_target_rule_command),
        )
        .route(
            "/collection/targets/monitoring",
            post(collection_target_toggle_monitoring),
        )
        .route(
            "/collection/targets/archive",
            post(collection_target_deep_archive),
        )
        .route(
            "/collection/targets/retire-materials",
            post(collection_retire_materials),
        )
        .route(
            "/collection/targets/patrol-toggle",
            post(collection_target_patrol_toggle),
        )
        .route(
            "/collection/targets/monitor-rules/retire",
            post(collection_monitor_rule_retire),
        )
        .route("/collection/targets/delete", post(collection_target_delete))
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
            "/collection/runtime/stations/name",
            post(collection_runtime_rename_station),
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
        .route(
            "/collection/runtime/accepting",
            post(collection_runtime_set_accepting),
        )
        .route(
            "/collection/runtime/account-bindings",
            post(collection_runtime_bind_account),
        )
        .route("/assets/evidence-library.css", get(stylesheet))
        .route(
            "/assets/evidence-observation.js",
            get(evidence_observation_script),
        )
        .route("/assets/evidence-library.js", get(evidence_library_script))
        .route(
            "/assets/topic-workspace.css",
            get(topic_workspace::stylesheet),
        )
        .route("/assets/topic-workspace.js", get(topic_workspace::script))
        .route(
            "/assets/collection-workspace.css",
            get(collection_stylesheet),
        )
        .route("/assets/collection-workspace.js", get(collection_script))
        .with_state(state)
}

fn material_api_routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/local/media/{materialization_ref}/{sha256}",
            get(local_media_routes::materialization),
        )
        .route(
            "/api/local/derivative/{derivative_ref}",
            get(local_media_routes::derivative),
        )
        .route("/api/local/work-resources", get(evidence_library_json))
        // 跨行业是**另一条查询路径**，不是给上面那个接口加参数。规格的接口红线：
        // 不得为跨行业给证据库接口增加任何参数或字段——两条路不共享，隔离才不
        // 依赖任何人记得在某处加一个条件。
        .route(
            "/api/local/cross-industry/samples",
            get(cross_industry_samples_json),
        )
        .route(
            "/api/local/cross-industry/comments",
            get(cross_industry_comments_json),
        )
        .route(
            "/api/local/evidence-library/legacy",
            get(material_projection::legacy_json),
        )
        .route(
            "/api/local/work-resources/{public_ref}/comments",
            get(material_projection::research_comments_json).layer(axum::middleware::from_fn(
                comment_research::local_research_guard,
            )),
        )
        .route(
            "/api/local/work-resources/{public_ref}/reobserve",
            post(material_projection::reobserve_json),
        )
        .route(
            "/api/local/work-resources/{public_ref}/reobserve/{lease_ref}",
            get(material_projection::reobservation_status_json),
        )
        .route(
            "/api/local/work-resources/{public_ref}",
            get(material_projection::detail_json),
        )
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
            "failure": collection_dispatch::FAILURE_PATH
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
        "listener": "loopback-only",
        "dataState": data_state,
        "evidenceReadModel": evidence_read_model,
        "database": {
            "state": database_state,
            "schema": schema_state
        },
        "scheduler": scheduler,
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

#[derive(serde::Deserialize)]
struct CorpusSurfaceParams {
    /// 当前观察领域。语料页每个子页共用这一个参数——换领域是换观察对象，不是换页面。
    #[serde(default)]
    domain: Option<String>,
}

async fn evidence_library(
    State(state): State<LocalWebState>,
    Query(params): Query<CorpusSurfaceParams>,
) -> Html<String> {
    let collection_state = match state.database.database() {
        Some(database) => match count_targets(database, None).await {
            Ok(counts) if counts.total > 0 => Some("观察中"),
            Ok(_) => Some("无观察目标"),
            Err(_) => Some("状态未知"),
        },
        None => None,
    };
    // 领域读不出来时给空列表：切换器随之隐藏，页面照常以本领域呈现。缺一个切换器远好过
    // 显示一个点不动的假控件。
    let domains = match state.database.database() {
        Some(database) => read_observation_domains(database).await.unwrap_or_default(),
        None => Vec::new(),
    };
    let current = resolve_current_domain(&domains, params.domain.as_deref());
    Html(evidence_library_html(collection_state, &domains, current))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CrossIndustryQuery {
    domain: uuid::Uuid,
}

/// 读一个外部领域的样本。
///
/// 本领域走的是 `/api/local/work-resources`，不是这里。传入本领域会得到一个明确的
/// 拒绝而不是空列表：空列表会被读成「这个领域还没采过」，而真相是问错了地方。
/// 读一个外部领域的评论原声。
///
/// 本领域的评论在证据侧（`/api/local/comment-research`），不在这里。两条查询路径不
/// 共享接口，隔离因此不依赖任何人记得在某处加条件。
async fn cross_industry_comments_json(
    State(state): State<LocalWebState>,
    Query(query): Query<CrossIndustryQuery>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    match read_cross_industry_comments(database, query.domain).await {
        Ok(payload) => Json(payload).into_response(),
        Err(CrossIndustryReadError::HomeDomainHasNoSamples) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "home_domain_reads_evidence",
        ),
        Err(CrossIndustryReadError::SchemaUnavailable) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "cross_industry_schema_unavailable",
        ),
        Err(CrossIndustryReadError::Database(_)) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "cross_industry_read_unavailable",
        ),
    }
}

async fn cross_industry_samples_json(
    State(state): State<LocalWebState>,
    Query(query): Query<CrossIndustryQuery>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    match read_cross_industry_samples(database, query.domain).await {
        Ok(payload) => Json(payload).into_response(),
        Err(CrossIndustryReadError::HomeDomainHasNoSamples) => local_read_json_error(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "home_domain_reads_evidence",
        ),
        Err(CrossIndustryReadError::SchemaUnavailable) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "cross_industry_schema_unavailable",
        ),
        Err(CrossIndustryReadError::Database(_)) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "cross_industry_read_unavailable",
        ),
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
    let outcome = match request_and_admit_material_targets(
        database,
        request.target_ref,
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
            "admission": outcome.outcome.code(),
            "workOrderRef": null,
            "execution": "NOT_STARTED",
        }))
        .into_response();
    };
    Json(json!({
        "requestRef": outcome.request_ref,
        "decisionRef": outcome.decision_ref,
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
                // 与「状态不允许」分开报：这条的处置是去给目标指定领域，不是等状态流转。
                AcquisitionChainError::TargetDomainUnassigned => "target_domain_unassigned",
                AcquisitionChainError::TargetNotRequestable { .. } => "target_not_requestable",
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
        Err(ProducerRuntimeError::ScheduledTaskNotClaimed) => local_producer_error(
            axum::http::StatusCode::CONFLICT,
            "scheduled_task_not_claimed_by_producer",
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

#[cfg(test)]
fn evidence_read_unavailable_html() -> String {
    // 读不到领域时页面仍以本领域呈现，选择器不渲染——这两个错误页正是那个状态。
    evidence_page::render_read_unavailable(&evidence_library_html(None, &[], None))
}

#[cfg(test)]
fn evidence_query_invalid_html() -> String {
    evidence_page::render_query_invalid(&evidence_library_html(None, &[], None))
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
    /// 要删除哪个观察目标。只是打开确认面板，不删任何东西——删除只走 POST。
    delete: Option<String>,
    /// 观察目标的筛选。只改读取范围，不消耗任何平台访问。
    filter: Option<String>,
    /// 当前观察领域。`all` 或缺省表示全部领域——采集是运维视角，默认看全貌，
    /// 这也正是加入领域之前的既有行为。
    domain: Option<String>,
    /// 当前唯一排序口径也属于列表返回上下文；打开/关闭抽屉不得把它丢掉。
    sort: Option<String>,
    /// 上一次动作的失败原因。失败必须看得见，否则跳转回来什么都不说，会让人以为成功了。
    error: Option<String>,
    /// 刚排进队列的那张工单，前面还有几个。只给右下角那条回执用——它回答的是
    /// 「我刚点的那下什么时候轮到」，看过即可，不常驻在列表上。
    ahead: Option<i64>,
    /// 抽屉打开的是哪个目标，以及停在哪个 tab。**放在 URL 里而不是 JS 状态里**：
    /// 刷新与分享都不丢，而这一页的用途正是「打开一个目标细看，然后发给别人」。
    dtab: Option<String>,
    /// Creator works keep their inner List / Performance view in the URL. Existing
    /// lifecycle links without `wview` are normalized to Performance below.
    wview: Option<String>,
    /// Lifecycle 只按窗口与指标读取。选中作品仅是页面状态，不进入 Rust read query。
    life_window: Option<String>,
    life_metric: Option<String>,
    /// 图表阅读方式与趋势粒度是页面本地状态：它们不进入 lifecycle read contract。
    life_chart: Option<String>,
    life_grain: Option<String>,
    life_work: Option<String>,
    /// The target drawer owns this bounded, read-only catalogue filter.
    catalog_query: Option<String>,
    catalog_filter: Option<String>,
    /// 规则 modal 的 target ref；与 drawer 并列，避免把规则命令状态塞进 JS。
    rule: Option<String>,
    /// 面板打开哪一条规则的口径。缺省=最早那条（博主永远只有一条）；`new`=新开一条。
    ///
    /// 放进 URL 而不是 JS 状态：刷新和分享都不丢，而且「加一条规则」与「编辑第二条」
    /// 是两个不同的页面状态，靠一个 target ref 区分不开——此前正因为区分不开，
    /// 「加一条规则」拿到的是最早那条的表单。
    rule_slot: Option<String>,
    /// 成功命令的 durable receipt ref，用于刷新后仍显示刚刚的回执。
    rule_receipt: Option<String>,
    /// 规则提交失败后，服务端将用户刚刚提交的受限字段值带回 modal；这些值只用于
    /// 修复表单，不参与任何读取或执行事实。
    rule_automatic_enabled: Option<String>,
    rule_fixed_interval_seconds: Option<String>,
    rule_surface_key: Option<String>,
    rule_ranking_key: Option<String>,
    rule_task_contract_version: Option<String>,
}

/// Collection 的共享页头只读现有事实，不创造第二套状态口径。每一项独立保留：某个
/// read model 暂时失败时，页面仍可显示另外三项已知事实，而不是整块退回「未接通」。
struct CollectionSurfaceReads {
    surface_state: collection::SurfaceState,
    counts: Option<TargetCounts>,
    capacity: Option<RuntimeCapacityOverview>,
    roster: Option<(Vec<StationOverview>, Vec<UnclaimedInstallation>)>,
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
    CollectionSurfaceReads {
        surface_state,
        counts,
        capacity: None,
        roster: None,
    }
}

async fn render_simple_collection_surface(
    state: &LocalWebState,
    section: collection::Section,
    mode: collection::OperationsMode,
) -> Html<String> {
    let reads = match state.database.database() {
        Some(database) => Some(read_collection_surface(database).await),
        None => None,
    };
    Html(collection::render(
        section,
        mode,
        None,
        None,
        reads.as_ref().map(|reads| &reads.surface_state),
    ))
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
    let database = state.database.database();
    let reads = match database {
        Some(database) => Some(read_collection_surface(database).await),
        None => None,
    };
    // 领域读不出来时给空列表：选择器随之隐藏，列表照常以全部领域呈现。
    let domains = match database {
        Some(database) => read_observation_domains(database).await.unwrap_or_default(),
        None => Vec::new(),
    };
    let current_domain = linggan_evidence::observation_domain::resolve_collection_domain(
        &domains,
        params.domain.as_deref(),
    );
    let picker = corpus_domain_picker(
        &domains,
        current_domain,
        "/collection/targets",
        Some("全部领域"),
    );
    let nav_domain = collection_nav_domain(&domains, current_domain);
    // 筛选 tab 的计数按当前领域算。它回答的是「在这个领域里切过去有多少」——跨领域去数，
    // 站在考研自习下会看到「创作者 2」而列表里只有 1 个，那个 2 说的是别的世界的事。
    // 左栏底部的总数仍是全局的：那是模块级事实，不随当前领域变。
    let domain_counts = match database {
        Some(database) => count_targets(database, current_domain.map(|domain| domain.domain_ref))
            .await
            .ok(),
        None => None,
    };
    let counts = domain_counts
        .as_ref()
        .or_else(|| reads.as_ref().and_then(|reads| reads.counts.as_ref()));
    let base = collection::render_in_domain(
        collection::Section::Targets,
        collection::OperationsMode::Now,
        params.filter.as_deref(),
        counts,
        reads.as_ref().map(|reads| &reads.surface_state),
        collection::DomainBar {
            picker: &picker,
            nav_domain: nav_domain.as_deref(),
            domains: &domains,
        },
    );
    let list_context = target_drawer::TargetListContext {
        filter: params.filter.as_deref(),
        sort: params.sort.as_deref(),
        domain: nav_domain.as_deref(),
    };
    // Without a database the page still renders its honest empty state rather than an error:
    // "we cannot read targets right now" and "there are no targets" are different claims, and
    // the empty state already makes only the weaker one.
    let Some(database) = database else {
        let drawer =
            collection::render_unreadable_target_drawer(params.drawer.as_deref(), list_context);
        let rule_overlay = params
            .rule
            .as_deref()
            .and_then(|value| uuid::Uuid::parse_str(value).ok())
            .map(|target_ref| {
                collection::collection_control_rule_view::render_monitor_rule_unavailable(
                    target_ref,
                    collection::collection_control_rule_view::MonitorRuleUnavailableState::SchemaUnavailable,
                    list_context,
                )
            })
            .unwrap_or_default();
        return Html(target_drawer::attach_to_collection_document(
            &base,
            &format!("{drawer}{rule_overlay}"),
        ));
    };
    // 一次查完所有目标的档案完整度：列表最多两百行，逐行发查询会让页面打开一次跑
    // 两百次数据库。
    let completeness = read_archive_completeness(database, linggan_contracts::OPEN_PLATFORM)
        .await
        .ok();
    // 抽屉独立查目标，不从筛过的列表里找——被筛掉的目标不该显示成「未找到」。
    let drawer_target_ref = params
        .drawer
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok());
    let drawer_target = match drawer_target_ref {
        Some(target_ref) => read_target(database, target_ref).await.map_err(|_| ()),
        None => Ok(None),
    };
    let drawer_inspector = match drawer_target_ref {
        Some(target_ref) => read_target_inspector(database, target_ref)
            .await
            .map_err(|_| ()),
        None => Ok(None),
    };
    let drawer_avatar = match drawer_target.as_ref() {
        Ok(Some(target)) if target.target_kind == "creator" => {
            match read_target_avatars(database, std::slice::from_ref(target)).await {
                Ok(mut avatars) => avatars
                    .remove(&target.target_ref)
                    .or(Some(ObservationTargetAvatar::NotObserved)),
                Err(_) => Some(ObservationTargetAvatar::Unavailable),
            }
        }
        Ok(Some(_)) | Ok(None) | Err(()) => None,
    };
    let has_legacy_lifecycle_query = params.life_window.is_some()
        || params.life_metric.is_some()
        || params.life_chart.is_some()
        || params.life_grain.is_some()
        || params.life_work.is_some();
    // 早期的作品表现链接只有 `life_*`，没有今天的 `dtab=works&wview=performance`。
    // 它们必须仍然落到表现视图；否则无效查询会在概览里被静默吞掉，用户既看不到图也看
    // 不到为什么图不可用。
    let drawer_tab = if params.dtab.is_none() && has_legacy_lifecycle_query {
        target_drawer::TargetDrawerTab::Baseline
    } else {
        target_drawer::TargetDrawerTab::parse(params.dtab.as_deref())
    };
    let works_view =
        target_drawer::TargetWorksView::parse(params.wview.as_deref(), has_legacy_lifecycle_query);
    let chart_view = target_drawer::LifeChartView::parse(params.life_chart.as_deref());
    let trend_grain = target_drawer::LifeTrendGrain::parse(params.life_grain.as_deref());
    let lifecycle_query = CreatorLifecycleQuery::parse_optional(
        params.life_window.as_deref(),
        params.life_metric.as_deref(),
    );
    // A selected Work is display state only, but it is still reflected into generated links.
    // Keep it in the UUID contract before any renderer sees it; arbitrary query text must never
    // become an HTML attribute through a refresh-safe drawer URL.
    let selected_lifecycle_work = params
        .life_work
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
        .map(|value| value.to_string());
    let lifecycle = if should_read_target_lifecycle(
        drawer_target.as_ref().ok().and_then(Option::as_ref),
        drawer_tab,
        works_view,
    ) {
        match (drawer_target_ref, lifecycle_query.as_ref()) {
            (Some(target_ref), Ok(query)) => {
                read_creator_lifecycle(database, target_ref, query).await
            }
            (None, _) | (_, Err(_)) => Ok(None),
        }
    } else {
        Ok(None)
    };
    let creator_catalog = match drawer_target.as_ref().ok().and_then(Option::as_ref) {
        Some(target) if target.target_kind == "creator" => {
            read_creator_directory(database, target.target_ref)
                .await
                .ok()
        }
        _ => None,
    };
    // 命中作品按**领域**决定读哪一侧：本领域的材料在证据库，外部领域的在跨行业语料
    // （`0044` 的隔离）。只读证据侧的话，跨行业目标的作品页永远是空的——采回来 204 篇，
    // 界面上一篇看不到，而且看不出是没采到还是读错了地方。
    let keyword_catalog = match drawer_target.as_ref().ok().and_then(Option::as_ref) {
        Some(target) if target.target_kind == "keyword" => {
            if target_is_cross_industry(database, target.target_ref).await {
                read_cross_industry_hits(database, target.target_ref)
                    .await
                    .ok()
            } else {
                read_keyword_hits(database, target.target_ref).await.ok()
            }
        }
        _ => None,
    };
    // 只有真的点了删除才去读预览：列表每次渲染都读一遍，等于为一个多数时候不显示的
    // 面板付一次查询。
    let deletion_target = params
        .delete
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok());
    let deletion_preview = match deletion_target {
        Some(target_ref) => read_target_deletion_preview(database, target_ref)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let list = match list_targets(
        database,
        params.filter.as_deref(),
        current_domain.map(|domain| domain.domain_ref),
        200,
    )
    .await
    {
        Ok(targets) => {
            let avatars = match read_target_avatars(database, &targets).await {
                Ok(avatars) => avatars,
                // A failed avatar read is not evidence that no avatar was observed. Render the
                // conservative unavailable state instead of quietly falling back to either a
                // remote URL or an untrue “not observed” label.
                Err(_) => targets
                    .iter()
                    .filter(|target| target.target_kind == "creator")
                    .map(|target| (target.target_ref, ObservationTargetAvatar::Unavailable))
                    .collect::<HashMap<_, _>>(),
            };
            let observation = read_target_observation_summaries(database, &targets)
                .await
                .ok();
            // 关键词「建过档没有」是查出来的（它不能存成生命周期状态），所以这里批量问
            // 一次，而不是每行查一次。查不到时传 None——页面据此显示「读不到」而不是
            // 催人去做一件可能已经做过的事。
            let keyword_refs = targets
                .iter()
                .filter(|target| target.target_kind == "keyword")
                .map(|target| target.target_ref)
                .collect::<Vec<_>>();
            let keyword_archives = keyword_baselines_qualified(database, &keyword_refs)
                .await
                .ok();
            // 建档分两段：先拿链接，再补详情。第二段是不是还欠着，同样批量问一次。
            let keyword_details_pending =
                linggan_evidence::keyword_targets_pending_detail(database, &keyword_refs)
                    .await
                    .ok();
            // 命中多少篇、补到多少篇详情：两侧一起数（本领域在证据侧，外部领域在跨行业
            // 语料），只数一侧另一侧会显示成 0——而 0 与「还没采」在界面上长得一样。
            let keyword_counts =
                linggan_evidence::read_keyword_catalog_counts(database, &keyword_refs)
                    .await
                    .ok();
            collection_targets_view::render_stored_targets_with_observation(
                &base,
                &targets,
                &avatars,
                completeness.as_ref(),
                observation.as_ref(),
                params.error.as_deref(),
                params.ahead,
                deletion_preview.as_ref(),
                deletion_target,
                collection_targets_view::TargetListFacts {
                    keyword_archives: keyword_archives.as_ref(),
                    keyword_details_pending: keyword_details_pending.as_ref(),
                    keyword_counts: keyword_counts.as_ref(),
                },
                list_context,
            )
        }
        Err(_) => base,
    };
    // The selected target lookup is independent from the list lookup. A filtered or failed
    // list must not erase a target that was read successfully, and an unreadable target must
    // not be flattened into "not found".
    // 打开了某个目标才读它的巡检规则：一个关键词可以有好几条，看不见就管不了。
    // 读不到时传 `None`，界面说读不到——不把读不到显示成「一条都没有」。
    let monitor_rules = match drawer_target.as_ref().ok().and_then(Option::as_ref) {
        Some(target) => linggan_evidence::read_target_monitor_rules(database, target.target_ref)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    // 只有真的打开了某个目标才去读它的待判断作品：列表页每次渲染都读一遍，等于为一个
    // 多数时候不显示的区块付一次查询。读不到就当作没有待判断项——那时不渲染确认入口，
    // 而不是把「读不到」渲染成「没问题」。
    let retirable = match drawer_target.as_ref() {
        Ok(Some(target)) if target.target_kind == "creator" => {
            read_blocked_materials(database, target.target_ref)
                .await
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    // 关键词「建过档没有」是主操作的输入之一，抽屉现在与列表行用同一个判断，就必须拿到
    // 同一份事实。抽屉目标可能被列表筛掉（列表最多两百行），所以在这里单独问一次，而不是
    // 从列表那一批里找——找不到会把「这一屏没有它」说成「它没建过档」。
    let drawer_keyword_archive = match drawer_target.as_ref().ok().and_then(Option::as_ref) {
        Some(target) if target.target_kind == "keyword" => {
            let refs = [target.target_ref];
            let archived = keyword_baselines_qualified(database, &refs).await.ok();
            // 建档分两段：先拿链接，再补详情。第二段是不是还欠着，同样要问。
            let pending = linggan_evidence::keyword_targets_pending_detail(database, &refs)
                .await
                .ok();
            collection_targets_view::keyword_archive_read(
                archived.as_ref(),
                pending.as_ref(),
                target.target_ref,
            )
        }
        // 创作者的主操作不看这一项；`Unavailable` 在这里是「不适用」，不是读失败。
        _ => target_drawer::KeywordArchiveRead::Unavailable,
    };
    let drawer = match drawer_target.as_ref() {
        Ok(target) => target_drawer::render_with_catalog_view_with_chart(
            target.as_ref(),
            drawer_avatar.as_ref(),
            completeness.as_ref(),
            params.drawer.as_deref(),
            drawer_tab,
            match lifecycle_query.as_ref() {
                Err(_) => target_drawer::LifecycleView::QueryInvalid,
                Ok(query)
                    if should_read_target_lifecycle(target.as_ref(), drawer_tab, works_view) =>
                {
                    match lifecycle.as_ref() {
                        Ok(Some(projection)) => {
                            target_drawer::LifecycleView::Projection(projection)
                        }
                        Ok(None) | Err(_) => target_drawer::LifecycleView::ReadUnavailable {
                            window: query.window,
                            metric: query.metric,
                        },
                    }
                }
                Ok(query) => target_drawer::LifecycleView::NotRead {
                    window: query.window,
                    metric: query.metric,
                },
            },
            match drawer_inspector.as_ref() {
                Ok(Some(projection)) => target_drawer::TargetInspectorView::Projection(projection),
                Ok(None) if target.is_some() => target_drawer::TargetInspectorView::ReadUnavailable,
                Ok(None) => target_drawer::TargetInspectorView::NotRead,
                Err(()) => target_drawer::TargetInspectorView::ReadUnavailable,
            },
            works_view,
            chart_view,
            trend_grain,
            match target.as_ref().map(|target| target.target_kind.as_str()) {
                Some("creator") => target_drawer::TargetCatalogView::Creator(
                    creator_catalog.as_ref().and_then(Option::as_ref),
                ),
                Some("keyword") => target_drawer::TargetCatalogView::Keyword(
                    keyword_catalog.as_ref().and_then(Option::as_ref),
                ),
                _ => target_drawer::TargetCatalogView::Unavailable,
            },
            params.catalog_query.as_deref(),
            params.catalog_filter.as_deref(),
            selected_lifecycle_work.as_deref(),
            monitor_rules.as_deref(),
            &retirable,
            drawer_keyword_archive,
            list_context,
        ),
        Err(()) => {
            collection::render_unreadable_target_drawer(params.drawer.as_deref(), list_context)
        }
    };
    let rule_overlay = match params
        .rule
        .as_deref()
        .and_then(|value| uuid::Uuid::parse_str(value).ok())
    {
        Some(target_ref) => match collection::collection_control_rule_view::read_monitor_rule_panel(
            database,
            target_ref,
            monitor_rule_selection(params.rule_slot.as_deref()),
            params
                .rule_receipt
                .as_deref()
                .and_then(|value| uuid::Uuid::parse_str(value).ok()),
        )
        .await
        {
            Ok(collection::collection_control_rule_view::MonitorRulePanelRead::Found(panel)) => {
                let form = rule_form_from_query(&panel, &params);
                let form = if params.error.is_some() {
                    form.with_new_command_identity()
                } else {
                    form
                };
                let form_error = monitor_rule_form_error(params.error.as_deref());
                collection::collection_control_rule_view::render_monitor_rule_modal(
                    &panel,
                    &form,
                    form_error.as_ref(),
                    list_context,
                )
            }
            Ok(collection::collection_control_rule_view::MonitorRulePanelRead::TargetNotFound) =>
                collection::collection_control_rule_view::render_monitor_rule_unavailable(
                    target_ref,
                    collection::collection_control_rule_view::MonitorRuleUnavailableState::TargetNotFound,
                    list_context,
                ),
            Ok(collection::collection_control_rule_view::MonitorRulePanelRead::SchemaUnavailable) =>
                collection::collection_control_rule_view::render_monitor_rule_unavailable(
                    target_ref,
                    collection::collection_control_rule_view::MonitorRuleUnavailableState::SchemaUnavailable,
                    list_context,
                ),
            Err(_) => collection::collection_control_rule_view::render_monitor_rule_unavailable(
                target_ref,
                collection::collection_control_rule_view::MonitorRuleUnavailableState::ReadUnavailable,
                list_context,
            ),
        },
        None => String::new(),
    };
    Html(target_drawer::attach_to_collection_document(
        &list,
        &format!("{drawer}{rule_overlay}"),
    ))
}

/// The lifecycle query can inspect up to its explicit scan budget, so the Collection page only
/// issues it for the one surface that consumes the result. The independent JSON API stays
/// available for intentional reads.
fn should_read_target_lifecycle(
    target: Option<&ObservationTarget>,
    active_tab: target_drawer::TargetDrawerTab,
    works_view: target_drawer::TargetWorksView,
) -> bool {
    target
        .map(|target| target.target_kind == "creator")
        .unwrap_or(false)
        && active_tab == target_drawer::TargetDrawerTab::Baseline
        && works_view == target_drawer::TargetWorksView::Performance
}

async fn collection_operations(
    State(state): State<LocalWebState>,
    Query(params): Query<CollectionParams>,
) -> Html<String> {
    let mode = collection::OperationsMode::parse(params.mode.as_deref());
    let Some(database) = state.database.database() else {
        return render_simple_collection_surface(&state, collection::Section::Operations, mode)
            .await;
    };
    let reads = read_collection_surface(database).await;
    let base = collection::render_in_domain(
        collection::Section::Operations,
        mode,
        None,
        None,
        Some(&reads.surface_state),
        collection::DomainBar {
            domains: &[],
            picker: "",
            nav_domain: params
                .domain
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        },
    );
    match collection::collection_control_surface_view::read_collection_control_surface(database, 100).await {
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::Ready(projection)) =>
            Html(collection::collection_control_surface_view::render_operations(&base, &projection, mode)),
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::SchemaUnavailable)
        | Err(_) => Html(base),
    }
}

async fn collection_attention(
    State(state): State<LocalWebState>,
    Query(domain): Query<CollectionDomainParam>,
) -> Html<String> {
    let Some(database) = state.database.database() else {
        return render_simple_collection_surface(
            &state,
            collection::Section::Attention,
            collection::OperationsMode::Now,
        )
        .await;
    };
    let reads = read_collection_surface(database).await;
    let base = collection::render_in_domain(
        collection::Section::Attention,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&reads.surface_state),
        collection::DomainBar {
            domains: &[],
            picker: "",
            nav_domain: domain.nav(),
        },
    );
    match collection::collection_control_surface_view::read_collection_control_surface(database, 100).await {
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::Ready(projection)) =>
            Html(collection::collection_control_surface_view::render_attention(&base, &projection)),
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::SchemaUnavailable)
        | Err(_) => Html(base),
    }
}

async fn collection_tasks(
    State(state): State<LocalWebState>,
    Query(domain): Query<CollectionDomainParam>,
) -> Html<String> {
    let Some(database) = state.database.database() else {
        return render_simple_collection_surface(
            &state,
            collection::Section::Tasks,
            collection::OperationsMode::Now,
        )
        .await;
    };
    let (reads, timeline) = tokio::join!(
        read_collection_surface(database),
        read_collection_task_timeline(database, 100),
    );
    let base = collection::render_in_domain(
        collection::Section::Tasks,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&reads.surface_state),
        collection::DomainBar {
            domains: &[],
            picker: "",
            nav_domain: domain.nav(),
        },
    );
    match timeline {
        Ok(timeline) => {
            let tasks = collection_tasks_view::render_tasks(&base, &timeline);
            match collection::collection_control_surface_view::read_collection_control_surface(database, 100).await {
                Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::Ready(projection)) =>
                    Html(collection::collection_control_surface_view::render_tasks_control(&tasks, &projection)),
                Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::SchemaUnavailable) =>
                    Html(collection::collection_control_surface_view::render_tasks_control_unavailable(
                        &tasks,
                        collection::collection_control_surface_view::TaskControlUnavailable::SchemaUnavailable,
                    )),
                Err(_) => Html(collection::collection_control_surface_view::render_tasks_control_unavailable(
                    &tasks,
                    collection::collection_control_surface_view::TaskControlUnavailable::ReadFailed,
                )),
            }
        }
        // The base says the narrower, truthful thing: this task read model is not available.
        Err(_) => Html(base),
    }
}

/// 只取当前领域的轻量参数。
///
/// 用在那些还不按领域过滤内容的采集子页上：它们不需要读领域表，也不该渲染选择器
/// （点了不起作用的控件比没有更糟），但必须把地址上的领域原样带进导航链接——否则
/// 从观察目标走一趟别的子页再回来，当前领域就没了。
///
/// 这里刻意不校验取值：无效的领域到了观察目标页会被解析回落成「全部领域」，带着一个
/// 认不出的值走一段路是无害的，而为此在每个子页各查一次库不值得。
#[derive(serde::Deserialize)]
struct CollectionDomainParam {
    domain: Option<String>,
}

impl CollectionDomainParam {
    fn nav(&self) -> Option<&str> {
        self.domain
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }
}

#[derive(serde::Deserialize)]
struct RuntimeSurfaceParams {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    domain: Option<String>,
}

async fn collection_runtime(
    State(state): State<LocalWebState>,
    Query(params): Query<RuntimeSurfaceParams>,
) -> Html<String> {
    // Without a database the page keeps its honest empty state: "we cannot read stations right
    // now" and "no station is registered" are different claims.
    let Some(database) = state.database.database() else {
        return Html(collection::render_in_domain(
            collection::Section::Runtime,
            collection::OperationsMode::Now,
            None,
            None,
            None,
            collection::DomainBar {
                // 这条渲染路径不提供领域清单：新建入口在观察目标页，不在这里。
                domains: &[],
                picker: "",
                nav_domain: params
                    .domain
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty()),
            },
        ));
    };
    // 三份读物一起决定这一页能说什么：工位现状、准入第 5 问的判定、上下文行的事实。
    // 任何一份读不到，对应的那部分就说「读不到」——**不退回写死的「未接通」**，
    // 那是这一页此前最大的问题：一句写下时为真、之后永不更新的状态。
    let (mut reads, capacity, roster) = tokio::join!(
        read_collection_surface(database),
        read_runtime_capacity(database),
        read_station_overview(database),
    );
    reads.capacity = capacity.ok();
    reads.roster = roster.ok();
    reads.surface_state.vacant_stations = reads
        .capacity
        .as_ref()
        .map(|capacity| capacity.registered_stations - capacity.staffed_stations);
    // 只数最近报到过的安装。一台机器每升级一次插件就留下一条旧安装，它们永远不会
    // 被认领；把它们算进上下文行，那个数字就只会往上涨，读起来像「有 N 台机器掉队
    // 了」，而实际掉队的是 0 台。历史残留仍在页面里，只是不冒充待处理项。
    let now_minutes = station_view::recent_installation_cutoff();
    reads.surface_state.unclaimed_installations = reads.roster.as_ref().map(|(_, unclaimed)| {
        unclaimed
            .iter()
            .filter(|installation| {
                station_view::installation_is_recent(&installation.first_seen_at, now_minutes)
            })
            .count() as i64
    });
    if reads.surface_state.total_targets.is_none() {
        reads.surface_state.total_targets = reads
            .capacity
            .as_ref()
            .map(|capacity| capacity.patrol.total_targets);
        reads.surface_state.monitoring_targets = reads
            .capacity
            .as_ref()
            .map(|capacity| capacity.patrol.monitoring_targets);
    }
    let base = collection::render_in_domain(
        collection::Section::Runtime,
        collection::OperationsMode::Now,
        None,
        None,
        Some(&reads.surface_state),
        collection::DomainBar {
            domains: &[],
            picker: "",
            nav_domain: params
                .domain
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
        },
    );
    // 能力矩阵按工位逐台读。工位是个位数，一台一次查询换来的是「这一项到底跑成过没有」
    // 这个问题有据可答；读不到的那台留空，由页面说「读不到」，不冒充「没有能力」。
    let mut capabilities: BTreeMap<uuid::Uuid, Vec<StationCapability>> = BTreeMap::new();
    if let Some((stations, _)) = reads.roster.as_ref() {
        for station in stations.iter() {
            if let Ok(rows) = read_station_capabilities(database, station.station_ref).await {
                capabilities.insert(station.station_ref, rows);
            }
        }
    }
    // 通道判定与工位控制事实此前渲染成页面顶部一个独立区块，于是同一台工位在一页里
    // 出现两次、同一条通道有两个名字。现在它们作为**输入**交给这一页：通道判定进
    // 判断区，工位控制事实进工位表对应的那一行。读不到时页面说读不到，不退回空值。
    let control = match collection::collection_control_surface_view::read_collection_control_surface(
        database, 100,
    )
    .await
    {
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::Ready(
            projection,
        )) => Some(projection),
        Ok(
            collection::collection_control_surface_view::CollectionControlSurfaceRead::SchemaUnavailable,
        )
        | Err(_) => None,
    };
    let control = control
        .as_ref()
        .map(|projection| station_view::RuntimeControl {
            lanes: &projection.runtime_lanes,
            resources: &projection.runtime_resources,
            account_observation_available: state.account_digest_key.is_some(),
        });
    let rendered = match reads.roster.as_ref() {
        Some((stations, unclaimed)) => station_view::render_runtime(
            &base,
            reads.capacity.as_ref(),
            stations,
            unclaimed,
            &capabilities,
            control.as_ref(),
            now_minutes,
            params.error.as_deref(),
        ),
        None => station_view::render_runtime_with_unreadable_roster(
            &base,
            reads.capacity.as_ref(),
            control.as_ref(),
            now_minutes,
            params.error.as_deref(),
        ),
    };
    Html(rendered)
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

#[derive(serde::Deserialize)]
struct StationForm {
    display_name: String,
}

async fn collection_runtime_register_station(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<StationForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    // 200 notes per station per day (Mog's decision), held on the station so a plugin
    // reinstall never resets it.
    if register_station(database, form.display_name.trim(), 200)
        .await
        .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("station_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct StationNameForm {
    station_ref: uuid::Uuid,
    display_name: String,
}

/// The Runtime owns the one canonical station name. A matched plugin receives
/// that name only through a later check-in response; it never proposes or
/// changes its own identity.
async fn collection_runtime_rename_station(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<StationNameForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    if rename_station(database, form.station_ref, &form.display_name)
        .await
        .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("station_name_rejected"));
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
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    if open_claim_window(database, form.station_ref, form.valid_for_hours)
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
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    if close_claim_window(database, form.station_ref)
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
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    if retire_station(database, form.station_ref, "在执行工位页停用")
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
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    if claim_installation(database, form.installation_ref, form.station_ref)
        .await
        .is_err()
    {
        return Redirect::to(&runtime_surface_with_error("claim_rejected"));
    }
    Redirect::to(RUNTIME_SURFACE)
}

#[derive(serde::Deserialize)]
struct AcceptingForm {
    station_ref: uuid::Uuid,
    accepting: bool,
}

async fn collection_runtime_set_accepting(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<AcceptingForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    match set_station_accepting(database, form.station_ref, form.accepting, "person").await {
        Ok(()) => Redirect::to(RUNTIME_SURFACE),
        Err(_) => Redirect::to(&runtime_surface_with_error("station_acceptance_rejected")),
    }
}

#[derive(serde::Deserialize)]
struct AccountBindingForm {
    installation_ref: uuid::Uuid,
    account_ref: uuid::Uuid,
}

async fn collection_runtime_bind_account(
    State(state): State<LocalWebState>,
    axum::extract::Form(form): axum::extract::Form<AccountBindingForm>,
) -> Redirect {
    let Some(database) = state.database.database() else {
        return Redirect::to(&runtime_surface_with_error("read_model_not_connected"));
    };
    match bind_observation_account(database, form.account_ref, form.installation_ref, "person")
        .await
    {
        Ok(_) => Redirect::to(RUNTIME_SURFACE),
        Err(_) => Redirect::to(&runtime_surface_with_error("account_binding_rejected")),
    }
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
        LeaseError::InvalidLeaseDuration => "lease_duration_invalid",
        LeaseError::NoStation => "work_order_names_no_station",
        LeaseError::StationUnavailable => "station_unavailable",
        LeaseError::FrozenControlMissing => "lease_frozen_control_missing",
        // Not a refusal: every step this order froze has already been done, so it was closed
        // instead of being handed a permit with no work behind it.
        LeaseError::WorkOrderAlreadySatisfied => "work_order_already_satisfied",
        LeaseError::ControlBlocked { reason_code } => match reason_code.as_str() {
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
    /// 这个目标归属的领域，由新建弹窗当场选定——**不再是「当前正在看的领域」**。
    /// 值为 `__new__` 时表示同时新建一个领域，名字在 `new_domain_name` 里。
    domain: Option<String>,
    /// 新领域的名字。只在 `domain == "__new__"` 时有意义。
    new_domain_name: Option<String>,
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
    // 领域必须由人明确选定，不能由「当前在看哪个领域」推断出来。
    //
    // 此前在「全部领域」视图下新建时表单不带这一项，服务端按本领域处置——于是一个本想
    // 用作跨行业参照的关键词被静默归进了本领域，它采回来的材料**直接写进证据侧**。
    // 2026-09-11 真实发生过一次：两个「数学思维」目标因此把 413 条笔记写进了 ADHD
    // 证据库，而跨行业语料表里一条都没有。领域不是一个可以猜的默认值，猜错的代价是
    // 参照物混进证据，且任何读证据的地方都不会再提醒你。
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
    // 选了「＋ 新建一个领域」时先把领域建出来，再用它建目标。两件事同一个动作里完成，
    // 但**不共用一个事务**：领域建成而目标没建成时，多一个空领域是可解释的；反过来
    // 目标挂在一个不存在的领域上则连外键都过不去。
    let domain = if domain_param.as_deref() == Some("__new__") {
        let Some(database) = state.database.database() else {
            return Redirect::to(&back_to_targets(None, Some("read_model_not_connected")));
        };
        let name = form.new_domain_name.as_deref().unwrap_or_default();
        match linggan_evidence::observation_domain::create_observation_domain(database, name).await
        {
            Ok(created) => Some(created.domain_ref),
            Err(linggan_evidence::observation_domain::ObservationDomainError::EmptyName) => {
                return Redirect::to(&back_to_targets(None, Some("domain_name_required")));
            }
            Err(linggan_evidence::observation_domain::ObservationDomainError::DuplicateName) => {
                return Redirect::to(&back_to_targets(None, Some("domain_name_taken")));
            }
            Err(_) => {
                return Redirect::to(&back_to_targets(None, Some("domain_create_failed")));
            }
        }
    } else {
        domain
    };
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
    target_ref: uuid::Uuid,
    expected_revision: i32,
    idempotency_key: uuid::Uuid,
    command_kind: String,
    automatic_enabled: Option<String>,
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
fn monitor_rule_selection(
    slot: Option<&str>,
) -> collection::collection_control_rule_view::MonitorRuleSelection<'_> {
    use collection::collection_control_rule_view::MonitorRuleSelection;
    match slot.map(str::trim) {
        Some("new") => MonitorRuleSelection::NewRule,
        Some(
            value @ ("most_liked" | "most_collected" | "most_commented" | "latest"
            | "comprehensive" | "primary"),
        ) => MonitorRuleSelection::Slot(value),
        _ => MonitorRuleSelection::CurrentRule,
    }
}

fn monitor_rule_redirect(
    form: &MonitorRuleWire,
    error: Option<&str>,
    receipt_ref: Option<uuid::Uuid>,
) -> Redirect {
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

fn rule_bool_query(value: Option<&str>, fallback: bool) -> bool {
    value.map_or(fallback, |value| matches!(value, "1" | "true" | "on"))
}

fn rule_form_from_query(
    panel: &collection::collection_control_rule_view::MonitorRulePanel,
    params: &CollectionParams,
) -> collection::collection_control_rule_view::MonitorRuleFormState {
    let mut form =
        collection::collection_control_rule_view::MonitorRuleFormState::from_panel(panel);
    form.automatic_enabled = rule_bool_query(
        params.rule_automatic_enabled.as_deref(),
        form.automatic_enabled,
    );
    if let Some(value) = params.rule_fixed_interval_seconds.as_deref() {
        form.fixed_interval_seconds = value.to_owned();
    }
    if let Some(value) = params.rule_surface_key.as_deref() {
        form.surface_key = value.to_owned();
    }
    if let Some(value) = params.rule_ranking_key.as_deref() {
        form.ranking_key = value.to_owned();
    }
    if let Some(value) = params.rule_task_contract_version.as_deref() {
        form.task_contract_version = value.to_owned();
    }
    form
}

fn monitor_rule_form_error(
    code: Option<&str>,
) -> Option<collection::collection_control_rule_view::MonitorRuleFormError> {
    let code = code?;
    let (code, fields) = match code {
        "invalid_mode" => (
            "invalid_mode",
            vec![
                collection::collection_control_rule_view::MonitorRuleFieldError {
                    field: "mode",
                    message: "请选择一个受支持的运行方式。",
                },
            ],
        ),
        "read_model_not_connected" => ("read_model_not_connected", Vec::new()),
        "target_not_found" => ("target_not_found", Vec::new()),
        "baseline_not_ready" => ("baseline_not_ready", Vec::new()),
        "stale_revision" => ("stale_revision", Vec::new()),
        "identity_conflict" => ("identity_conflict", Vec::new()),
        "authorization_missing" => ("authorization_missing", Vec::new()),
        "authorization_expired_or_revoked" => ("authorization_expired_or_revoked", Vec::new()),
        "station_not_accepting" => ("station_not_accepting", Vec::new()),
        "account_needs_login" => ("account_needs_login", Vec::new()),
        "target_not_requestable" => ("target_not_requestable", Vec::new()),
        "database_unavailable" => ("database_unavailable", Vec::new()),
        "manual_observe_rejected" => ("manual_observe_rejected", Vec::new()),
        _ => ("command_rejected", Vec::new()),
    };
    Some(collection::collection_control_rule_view::MonitorRuleFormError { code, fields })
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
struct TargetArchiveForm {
    row_target_ref: uuid::Uuid,
    /// Closed list state only. These fields are navigation context, never acquisition input.
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
/// 这个观察目标的材料落在跨行业语料那一侧吗？
///
/// 读不出来时按**本领域**处理，与 `0041` 对既有目标的处置一致：历史上的采集全发生在
/// 只有一个领域的时候，把它们算成别的领域会改写历史。这里只影响「读哪张表」，读错的
/// 后果是看不到作品，不会把材料写错地方——写入侧的领域判定另有一套，并且有复合外键兜底。
async fn target_is_cross_industry(database: &Database, target_ref: uuid::Uuid) -> bool {
    sqlx::query_scalar::<_, bool>(
        "SELECT COALESCE(domain.is_own_domain,true)=false \
         FROM collection_observation_target target \
         LEFT JOIN observation_domain domain USING(domain_ref) \
         WHERE target.target_ref=$1",
    )
    .bind(target_ref)
    .fetch_optional(database.pool())
    .await
    .ok()
    .flatten()
    .unwrap_or(false)
}

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
        //
        // 同一个按钮按所处的段落做不同的事：还没翻完搜索面就去翻，翻完了就去补下一批
        // 详情。分成两个按钮反而要人先判断「现在到哪一步了」——那是系统自己查得出来的。
        let advanced = linggan_evidence::advance_keyword_archive_detail(
            database,
            form.row_target_ref,
            "从观察目标页发起关键词历史建档",
            "person",
        )
        .await;
        match advanced {
            Ok(linggan_evidence::KeywordDetailAdvance::Queued { .. }) => {
                let ahead = queued_ahead_for(database, form.row_target_ref).await;
                return Redirect::to(&target_archive_return_path_with_queue(
                    &form,
                    Some("keyword_detail_requested"),
                    ahead,
                ));
            }
            Ok(linggan_evidence::KeywordDetailAdvance::Skipped("detail_batch_in_flight")) => {
                return Redirect::to(&target_archive_return_path(
                    &form,
                    Some("archive_in_progress"),
                ));
            }
            Ok(linggan_evidence::KeywordDetailAdvance::Skipped("no_missing_detail")) => {
                return Redirect::to(&target_archive_return_path(
                    &form,
                    Some("keyword_detail_complete"),
                ));
            }
            // 准入没放行时把它的结论原样带回去，与下面那一段同一种处理。
            Ok(linggan_evidence::KeywordDetailAdvance::Skipped(code)) => {
                return Redirect::to(&target_archive_return_path(
                    &form,
                    Some(&format!("archive_{code}")),
                ));
            }
            Err(AcquisitionChainError::TargetDomainUnassigned) => {
                return Redirect::to(&target_archive_return_path(
                    &form,
                    Some("target_domain_unassigned"),
                ));
            }
            // 详情这一段读不出来，不代表第一段发不出去。继续往下走，由第一段自己的
            // 结论说话——在这里断言「建档不可用」会掩盖真实原因。
            Err(_) => {}
        }
        let requested = linggan_evidence::request_and_admit(
            database,
            form.row_target_ref,
            "deep_archive",
            "从观察目标页发起关键词历史建档",
            "person",
        )
        .await;
        return Redirect::to(&match requested {
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
            // 缺领域要说清是缺领域。它此前落进一句「上一次动作没有完成」——信息量最低的
            // 那条兜底文案，既不说原因也不说下一步。
            Err(AcquisitionChainError::TargetDomainUnassigned) => {
                target_archive_return_path(&form, Some("target_domain_unassigned"))
            }
            Err(_) => target_archive_return_path(&form, Some("archive_unavailable")),
        });
    }
    let outcome = request_progressive_archive(
        database,
        form.row_target_ref,
        "从观察目标页发起深度建档",
        "person",
    )
    .await;
    let outcome = match outcome {
        Ok(outcome) => outcome,
        Err(RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveAuthorizationTooSmall { .. },
        )) => {
            return Redirect::to(&target_archive_return_path(
                &form,
                Some("archive_authorization_below_200"),
            ));
        }
        Err(RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveAuthorizationMissing,
        )) => {
            return Redirect::to(&target_archive_return_path(&form, Some("archive_refuse")));
        }
        Err(RequestLeaseError::Acquisition(
            AcquisitionChainError::ProgressiveArchiveNotReady { reason },
        )) => {
            let code = if reason == "detail_batch_in_flight" {
                "archive_in_progress"
            } else if reason == "no_missing_accepted_work" {
                "archive_nothing_to_continue"
            } else {
                "archive_not_requestable"
            };
            return Redirect::to(&target_archive_return_path(&form, Some(code)));
        }
        // 「缺领域」不是「状态不允许」：后者的文案是「目标可能已有同类任务在进行，
        // 或当前状态不允许再次发起」，会把人引向一个永远不会到来的状态流转。
        Err(RequestLeaseError::Acquisition(AcquisitionChainError::TargetDomainUnassigned)) => {
            return Redirect::to(&target_archive_return_path(
                &form,
                Some("target_domain_unassigned"),
            ));
        }
        Err(_) => {
            let code = { "archive_not_requestable" };
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
        Some("archive_requested"),
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

async fn collection_stylesheet() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        format!("{LIDS_TOKENS}\n{SHELL_CSS}\n{COLLECTION_WORKSPACE_CSS}\n{TARGET_DRAWER_CSS}"),
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

async fn evidence_library_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/javascript; charset=utf-8"),
        )],
        EVIDENCE_LIBRARY_JS,
    )
        .into_response()
}

async fn evidence_observation_script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/javascript; charset=utf-8"),
        )],
        EVIDENCE_OBSERVATION_JS,
    )
        .into_response()
}

/// 面包屑里的领域选择器。
///
/// 它站在面包屑那一层，而不是内容区里，因为领域是**当前观察对象**而非页面内的一个筛选项：
/// 选中它，语料下的每个子页都跟着换数据源，页面结构一律不变。
///
/// 只有一个领域（或读不出来）时整个控件不渲染——一个永远只有一项的下拉是噪音。
/// 导航链接该不该带领域参数，与选择器渲不渲染用的是同一个判断：少于两个领域时
/// 页面上没有可切换的东西，链接再带一个参数只会让地址假装有得选。
fn corpus_nav_domain(
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
) -> Option<String> {
    if domains.len() < 2 {
        return None;
    }
    current.map(|domain| domain.domain_ref.to_string())
}

/// 采集侧的导航领域参数。与语料侧的差别只有一处：全部领域也要显式带上，否则从
/// 「全部领域」点进另一个子页会悄悄变回默认，与本次修的那类缺陷同源。
fn collection_nav_domain(
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
) -> Option<String> {
    if domains.len() < 2 {
        return None;
    }
    Some(current.map_or_else(
        || linggan_evidence::observation_domain::ALL_DOMAINS.to_owned(),
        |domain| domain.domain_ref.to_string(),
    ))
}

/// 面包屑上的领域选择器。
///
/// `all_domains_label` 传 `Some` 时多一个「全部领域」项，`current` 为 `None` 即选中它。
/// 语料侧传 `None`——阅读时一次只看一个世界，混着看没有意义；采集侧传 `Some`——运维
/// 视角需要一眼看到所有领域在跑什么。
fn corpus_domain_picker(
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
    action: &str,
    all_domains_label: Option<&str>,
) -> String {
    if current.is_none() && all_domains_label.is_none() {
        return String::new();
    }
    if domains.len() < 2 {
        return String::new();
    }
    let mut options = String::new();
    let mut current_label = String::new();
    let mut current_meta = String::new();
    if let Some(label) = all_domains_label {
        let selected = current.is_none();
        if selected {
            current_label = label.to_owned();
            current_meta = "全部领域".to_owned();
        }
        options.push_str(&format!(
            r#"<a href="{action}?domain={all}"{current} aria-label="{label}，全部领域"><span>{label}</span><small>全部领域</small></a>"#,
            action = html_escape(action),
            all = linggan_evidence::observation_domain::ALL_DOMAINS,
            current = if selected { " aria-current=\"page\"" } else { "" },
            label = html_escape(label),
        ));
    }
    for domain in domains {
        let selected = current.is_some_and(|current| current.domain_ref == domain.domain_ref);
        // 本领域只带一个角标，不单列一类：它在这个列表里是普通一项。
        let meta = if domain.is_own_domain {
            "本领域".to_owned()
        } else if let Some(count) = domain.sample_count {
            format!("{count} 条样本")
        } else {
            "样本数未知".to_owned()
        };
        if selected {
            current_label = domain.name.clone();
            current_meta = meta.clone();
        }
        options.push_str(&format!(
            r#"<a href="{action}?domain={domain_ref}"{current} aria-label="{name}，{meta}"><span>{name}</span><small>{meta}</small></a>"#,
            action = html_escape(action),
            domain_ref = domain.domain_ref,
            current = if selected { " aria-current=\"page\"" } else { "" },
            name = html_escape(&domain.name),
            meta = html_escape(&meta),
        ));
    }
    // 领域切换只带 domain：此前领域下的检索词、筛选和选中项不能跟去描述另一批材料。
    // 使用链接而不是原生 select，菜单的打开态、焦点和当前项才能由 LIDS 接管。
    format!(
        r#"<details class="v7-domain-picker">
             <summary aria-label="切换当前观察领域"><span>{current_label}</span><small>{current_meta}</small><i aria-hidden="true"></i></summary>
             <nav aria-label="可选观察领域">{options}</nav>
           </details>"#,
        current_label = html_escape(&current_label),
        current_meta = html_escape(&current_meta),
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn evidence_library_header(
    collection_state: Option<&str>,
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
) -> String {
    let picker = corpus_domain_picker(domains, current, "/corpus/evidence", None);
    let crumb = if picker.is_empty() {
        "语料 <span class=\"v7-slash\">/</span> <b>证据库</b> <span class=\"v7-slash\">/</span> <span class=\"v7-context-current\">作品材料集合</span>".to_owned()
    } else {
        format!(
            "语料 <span class=\"v7-slash\">/</span> {picker} <span class=\"v7-slash\">/</span> <b>证据库</b>"
        )
    };
    shell::global_header(
        shell::PrimarySurface::Corpus,
        "本机材料投影 <span class=\"v7-tech-key\">LOCAL MATERIAL PROJECTION</span>",
        &crumb,
        "<span class=\"v7-kpi\"><em>事实层</em><b>只读</b></span><span class=\"v7-kpi\"><em>材料入口</em><b>默认投影</b></span><i class=\"v7-vr\" aria-hidden=\"true\"></i><span class=\"v7-query-meta\">不混读旧发现卡片 <span class=\"v7-tech-key\">NO LEGACY FALLBACK</span></span><span>本机时区 <span class=\"v7-tech-key\">UTC+08</span></span>",
        collection_state,
    )
}

fn evidence_library_html(
    collection_state: Option<&str>,
    domains: &[ObservationDomain],
    current: Option<&ObservationDomain>,
) -> String {
    let base = r#"<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="color-scheme" content="light">
    <title>证据库 · Linggan Intelligence</title>
    <link rel="stylesheet" href="/assets/evidence-library.css">
    <script src="/assets/evidence-observation.js" defer></script>
    <script src="/assets/evidence-library.js" defer></script>
  </head>
  <body data-corpus-domain="__CORPUS_DOMAIN_REF__" data-corpus-domain-own="__CORPUS_DOMAIN_OWN__" data-corpus-domain-name="__CORPUS_DOMAIN_NAME__">
    <div class="v7-app">
      <!-- GLOBAL_HEADER_START --><!-- GLOBAL_HEADER_END -->
      <div class="v7-shell">
        <!-- CORPUS_SIDE_NAV_START --><!-- CORPUS_SIDE_NAV_END -->

        <main class="v7-main ev-main" aria-labelledby="page-title">
          <h1 class="v7-sr-only" id="page-title">证据库</h1>

          <section class="ev-command" aria-label="材料检索与筛选">
            <form id="ev-query-form" class="ev-query-form">
              <div class="ev-find">
                <span class="ev-find-mark" aria-hidden="true">FIND</span>
                <input id="ev-search" name="q" type="search" autocomplete="off" aria-label="检索作品与证据" placeholder="标题、作者或原声片段……">
              </div>
              <div class="ev-query-tools">
                <div class="ev-popover-anchor">
                  <button class="ev-button ev-filter-toggle" id="ev-filter-toggle" type="button" aria-expanded="false" aria-controls="ev-filter-panel">
                    <span>筛选</span><b id="ev-filter-count">0</b>
                  </button>
                  <div class="ev-popover ev-filter-panel" id="ev-filter-panel" hidden>
                    <div class="ev-field" data-ev-select="ev-window" data-ev-name="window"><span class="ev-field-label" id="ev-window-label">读取窗口</span><div class="ev-popover-anchor"><button class="ev-select-toggle" id="ev-window" type="button" aria-expanded="false" aria-haspopup="listbox" aria-labelledby="ev-window-label ev-window-value" aria-controls="ev-window-list"><span class="ev-select-value" id="ev-window-value">最新已接纳发现</span><i class="ev-caret" aria-hidden="true"></i></button><div class="ev-popover ev-select-list" id="ev-window-list" role="listbox" aria-labelledby="ev-window-label" hidden><button type="button" role="option" data-ev-option="latest_accepted_discovery" aria-selected="true">最新已接纳发现</button><button type="button" role="option" data-ev-option="last_7_days" aria-selected="false">近 7 天来源发布时间</button><button type="button" role="option" data-ev-option="last_30_days" aria-selected="false">近 30 天来源发布时间</button></div></div></div>
                    <div class="ev-field" data-ev-select="ev-lane" data-ev-name="lane"><span class="ev-field-label" id="ev-lane-label">材料通道</span><div class="ev-popover-anchor"><button class="ev-select-toggle" id="ev-lane" type="button" aria-expanded="false" aria-haspopup="listbox" aria-labelledby="ev-lane-label ev-lane-value" aria-controls="ev-lane-list"><span class="ev-select-value" id="ev-lane-value">全部材料通道</span><i class="ev-caret" aria-hidden="true"></i></button><div class="ev-popover ev-select-list" id="ev-lane-list" role="listbox" aria-labelledby="ev-lane-label" hidden><button type="button" role="option" data-ev-option="" aria-selected="true">全部材料通道</button><button type="button" role="option" data-ev-option="discovery" aria-selected="false">发现</button><button type="button" role="option" data-ev-option="detail" aria-selected="false">详情</button><button type="button" role="option" data-ev-option="comments" aria-selected="false">评论</button><button type="button" role="option" data-ev-option="replies" aria-selected="false">回复</button><button type="button" role="option" data-ev-option="author" aria-selected="false">作者</button><button type="button" role="option" data-ev-option="media_slots" aria-selected="false">媒体槽位</button><button type="button" role="option" data-ev-option="media_bytes" aria-selected="false">媒体字节</button><button type="button" role="option" data-ev-option="ocr" aria-selected="false">图片文字</button><button type="button" role="option" data-ev-option="asr" aria-selected="false">视频转录</button></div></div></div>
                    <div class="ev-field" data-ev-select="ev-lane-state" data-ev-name="laneState"><span class="ev-field-label" id="ev-lane-state-label">通道状态</span><div class="ev-popover-anchor"><button class="ev-select-toggle" id="ev-lane-state" type="button" aria-expanded="false" aria-haspopup="listbox" aria-labelledby="ev-lane-state-label ev-lane-state-value" aria-controls="ev-lane-state-list"><span class="ev-select-value" id="ev-lane-state-value">全部状态</span><i class="ev-caret" aria-hidden="true"></i></button><div class="ev-popover ev-select-list" id="ev-lane-state-list" role="listbox" aria-labelledby="ev-lane-state-label" hidden><button type="button" role="option" data-ev-option="" aria-selected="true">全部状态</button><button type="button" role="option" data-ev-option="UNKNOWN" aria-selected="false">当前未知</button><button type="button" role="option" data-ev-option="NOT_REQUESTED" aria-selected="false">尚未请求</button><button type="button" role="option" data-ev-option="QUEUED" aria-selected="false">已排队</button><button type="button" role="option" data-ev-option="NOT_OBSERVED" aria-selected="false">尚未观察</button><button type="button" role="option" data-ev-option="OBSERVED" aria-selected="false">已观察</button><button type="button" role="option" data-ev-option="PARTIAL" aria-selected="false">部分取得</button><button type="button" role="option" data-ev-option="ACQUIRED" aria-selected="false">已取得</button><button type="button" role="option" data-ev-option="PROCESSING" aria-selected="false">处理中</button><button type="button" role="option" data-ev-option="NOT_ENABLED" aria-selected="false">处理器未启用</button><button type="button" role="option" data-ev-option="SEARCHABLE" aria-selected="false">可检索</button><button type="button" role="option" data-ev-option="FAILED" aria-selected="false">执行失败</button><button type="button" role="option" data-ev-option="RISK_CONTROL" aria-selected="false">风险控制停止</button><button type="button" role="option" data-ev-option="BYTES_CLEANED" aria-selected="false">字节已清理</button><button type="button" role="option" data-ev-option="WITHDRAWN_OR_RESTRICTED" aria-selected="false">撤回或受限</button></div></div></div>
                    <div class="ev-field" data-ev-select="ev-media-kind" data-ev-name="mediaKind"><span class="ev-field-label" id="ev-media-kind-label">媒体类型</span><div class="ev-popover-anchor"><button class="ev-select-toggle" id="ev-media-kind" type="button" aria-expanded="false" aria-haspopup="listbox" aria-labelledby="ev-media-kind-label ev-media-kind-value" aria-controls="ev-media-kind-list"><span class="ev-select-value" id="ev-media-kind-value">全部类型</span><i class="ev-caret" aria-hidden="true"></i></button><div class="ev-popover ev-select-list" id="ev-media-kind-list" role="listbox" aria-labelledby="ev-media-kind-label" hidden><button type="button" role="option" data-ev-option="" aria-selected="true">全部类型</button><button type="button" role="option" data-ev-option="cover" aria-selected="false">封面</button><button type="button" role="option" data-ev-option="image" aria-selected="false">正文图片</button><button type="button" role="option" data-ev-option="video" aria-selected="false">视频</button><button type="button" role="option" data-ev-option="live_photo" aria-selected="false">实况图片</button></div></div></div>
                    <div class="ev-filter-foot"><button class="ev-button ev-button--ghost" id="ev-reset" type="button">清空筛选</button></div>
                  </div>
                </div>
                <div class="ev-popover-anchor">
                  <button class="ev-button ev-sort-toggle" id="ev-sort-toggle" type="button" aria-expanded="false" aria-haspopup="listbox" aria-controls="ev-sort-list">
                    <span class="ev-sort-value" id="ev-sort-value">最近观察</span><i class="ev-caret" aria-hidden="true"></i>
                  </button>
                  <div class="ev-popover ev-sort-list" id="ev-sort-list" role="listbox" aria-label="结果排序" hidden>
                    <button type="button" role="option" data-ev-sort="latest_discovery" aria-selected="true">最近观察 <span class="v7-tech-key">LATEST DISCOVERY</span></button>
                    <button type="button" role="option" data-ev-sort="relevance" aria-selected="false">相关度 <span class="v7-tech-key">RELEVANCE</span></button>
                  </div>
                </div>
                <button class="ev-button ev-button--primary" type="submit"><span>读取材料</span></button>
              </div>
            </form>

            <div class="ev-deck-foot">
              <div class="ev-quickviews" role="group" aria-label="按材料状态快速筛选">
                <span class="ev-deck-label">系统视图 <span class="v7-tech-key">SYSTEM VIEWS</span></span>
                <button type="button" data-ev-view="all" aria-pressed="true">全部材料</button>
                <button type="button" data-ev-view="partial" aria-pressed="false">部分取得</button>
                <button type="button" data-ev-view="risk" aria-pressed="false">风险停止</button>
                <button type="button" data-ev-view="cleaned" aria-pressed="false">媒体已清理</button>
                <button type="button" data-ev-view="restricted" aria-pressed="false">撤回或受限</button>
              </div>
              <div class="ev-layout-control">
                <span class="ev-deck-label ev-saved-views" aria-disabled="true">我的视图 <span class="v7-tech-key">MY VIEWS</span> · 暂无已保存视图 <span class="v7-tech-key">SAVED VIEWS NOT CONNECTED</span></span>
                <div class="ev-layout-switch" role="group" aria-label="结果排版">
                  <button type="button" data-ev-layout="research" aria-pressed="true">研读</button>
                  <button type="button" data-ev-layout="table" aria-pressed="false">表格</button>
                  <button type="button" data-ev-layout="cover" aria-pressed="false">封面</button>
                </div>
              </div>
            </div>
          </section>

          <div class="ev-read-receipt" id="ev-read-receipt" role="status" aria-live="polite" hidden></div>

          <section class="ev-bench" id="ev-bench" data-inspector="normal" aria-label="证据审查工作台">
            <section class="ev-results" aria-labelledby="results-title">
              <h2 class="v7-sr-only" id="results-title">作品材料结果</h2>
              <div class="ev-feedback" id="ev-feedback" role="status" aria-live="polite"></div>
              <div class="ev-table-head" id="ev-table-head" aria-hidden="true" hidden><span>作品</span><span>作者</span><span>材料</span><span class="ev-head-metrics" id="ev-head-metrics" aria-label="互动数据"></span><span>发布时间</span><span>最近观察</span></div>
              <div class="ev-work-list" id="ev-work-list" role="listbox" aria-label="作品材料集合"></div>
              <div class="ev-list-footer"><button class="ev-button ev-button--secondary" id="ev-next-list" type="button" hidden>继续读取作品</button></div>
            </section>

            <aside class="ev-inspector" id="ev-inspector" aria-labelledby="ev-inspector-title">
              <header class="ev-inspector-head">
                <button class="ev-back" id="ev-back-to-list" type="button">← 返回当前作品</button>
                <h2 class="v7-sr-only" id="ev-inspector-title">请选择一个作品材料集合</h2>
                <div class="ev-action-row">
                  <button class="ev-button ev-button--primary" id="ev-open-source" type="button" disabled><span>打开原文</span></button>
                  <button class="ev-button" id="ev-request-media" type="button" disabled>补采</button>
                  <span class="ev-inspector-ref" id="ev-inspector-ref"></span>
                  <div class="ev-panel-tools">
                    <div class="ev-width-switch" role="group" aria-label="检查器宽度">
                      <button type="button" data-ev-width="normal" aria-pressed="true" title="标准宽度"><i class="ev-width-glyph ev-width-glyph--normal" aria-hidden="true"></i><span class="v7-sr-only">标准</span></button>
                      <button type="button" data-ev-width="wide" aria-pressed="false" title="加宽"><i class="ev-width-glyph ev-width-glyph--wide" aria-hidden="true"></i><span class="v7-sr-only">加宽</span></button>
                      <button type="button" data-ev-width="focus" aria-pressed="false" title="专注"><i class="ev-width-glyph ev-width-glyph--focus" aria-hidden="true"></i><span class="v7-sr-only">专注</span></button>
                    </div>
                    <button class="ev-panel-close" id="ev-close-inspector" type="button" title="关闭检查器"><span aria-hidden="true">×</span><span class="v7-sr-only">关闭检查器</span></button>
                  </div>
                </div>
                <div class="ev-inspector-feedback" id="ev-inspector-feedback" role="status"></div>
                <div class="ev-tabs" role="tablist" aria-label="作品证据详情">
                  <button type="button" role="tab" id="ev-tab-overview" data-ev-tab="overview" aria-controls="ev-panel-overview" aria-selected="true">概览</button>
                  <button type="button" role="tab" id="ev-tab-evidence" data-ev-tab="evidence" aria-controls="ev-panel-evidence" aria-selected="false" tabindex="-1">证据</button>
                  <button type="button" role="tab" id="ev-tab-materials" data-ev-tab="materials" aria-controls="ev-panel-materials" aria-selected="false" tabindex="-1">材料</button>
                  <button type="button" role="tab" id="ev-tab-trace" data-ev-tab="trace" aria-controls="ev-panel-trace" aria-selected="false" tabindex="-1">来源轨迹</button>
                </div>
              </header>
              <div class="ev-inspector-body">
                <div role="tabpanel" id="ev-panel-overview" data-ev-panel="overview" aria-labelledby="ev-tab-overview"></div>
                <div role="tabpanel" id="ev-panel-evidence" data-ev-panel="evidence" aria-labelledby="ev-tab-evidence" hidden></div>
                <div role="tabpanel" id="ev-panel-materials" data-ev-panel="materials" aria-labelledby="ev-tab-materials" hidden></div>
                <div role="tabpanel" id="ev-panel-trace" data-ev-panel="trace" aria-labelledby="ev-tab-trace" hidden></div>
              </div>
            </aside>
          </section>

          <div class="ev-lightbox" id="ev-lightbox" role="dialog" aria-modal="true" aria-labelledby="ev-lightbox-title" hidden>
            <div class="ev-lightbox-head">
              <div><strong id="ev-lightbox-title">本地媒体对象</strong><span class="ev-lightbox-ref" id="ev-lightbox-ref"></span></div>
              <div class="ev-lightbox-tools">
                <div class="ev-zoom" role="group" aria-label="缩放">
                  <button type="button" id="ev-zoom-out" aria-label="缩小"><span aria-hidden="true">−</span></button>
                  <button type="button" id="ev-zoom-reset" class="ev-zoom-value">100%</button>
                  <button type="button" id="ev-zoom-in" aria-label="放大"><span aria-hidden="true">+</span></button>
                </div>
                <button class="ev-lightbox-close" id="ev-lightbox-close" type="button"><span aria-hidden="true">×</span><span class="v7-sr-only">关闭查看器 Esc</span></button>
              </div>
            </div>
            <div class="ev-lightbox-stage">
              <button class="ev-lightbox-step" id="ev-lightbox-prev" type="button" aria-label="上一张"><span aria-hidden="true">←</span></button>
              <figure class="ev-lightbox-figure">
                <div class="ev-lightbox-frame" id="ev-lightbox-frame"><img id="ev-lightbox-image" alt=""></div>
                <figcaption id="ev-lightbox-caption"></figcaption>
              </figure>
              <button class="ev-lightbox-step" id="ev-lightbox-next" type="button" aria-label="下一张"><span aria-hidden="true">→</span></button>
            </div>
          </div>
        </main>
      </div>
    </div>
  </body>
</html>"#;
    let header = evidence_library_header(collection_state, domains, current);
    let side_nav = shell::corpus_side_nav(
        shell::CorpusPage::Evidence,
        corpus_nav_domain(domains, current).as_deref(),
        r#"<span class="v7-side-dot"></span><span class="v7-zh-status">只读本机材料投影</span><br><span class="v7-zh-status">列表与详情不触发采集</span>"#,
    );
    // 当前领域随页面一起下发，前端据此决定读哪条查询路径。放在 body 属性上而不是
    // 让前端自己解析地址：地址里的 domain 可能是无效值，回落判定由服务端做过一次了，
    // 前端再判一次就会出现两处规则，早晚不一致。
    base.replace(
        "<!-- GLOBAL_HEADER_START --><!-- GLOBAL_HEADER_END -->",
        &header,
    )
    .replace(
        "<!-- CORPUS_SIDE_NAV_START --><!-- CORPUS_SIDE_NAV_END -->",
        &side_nav,
    )
    .replace(
        "__CORPUS_DOMAIN_REF__",
        &current
            .map(|d| d.domain_ref.to_string())
            .unwrap_or_default(),
    )
    .replace(
        "__CORPUS_DOMAIN_OWN__",
        if current.is_none_or(|d| d.is_own_domain) {
            "true"
        } else {
            "false"
        },
    )
    .replace(
        "__CORPUS_DOMAIN_NAME__",
        &current.map(|d| html_escape(&d.name)).unwrap_or_default(),
    )
}

#[cfg(test)]
mod tests;
