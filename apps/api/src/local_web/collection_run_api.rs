//! Collection user-run endpoints on the existing loopback router.
use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_evidence::collection_run::{self, CreateRun, RunCommand, RunError, RunQuery};
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/executions", get(executions))
        .route("/api/collection/runs", get(list).post(create))
        .route("/api/local/collection/run-control", post(probe))
        .route("/api/collection/runs/targets", get(targets))
        .route(
            "/api/collection/runs/requests/{identity}",
            get(request_receipt),
        )
        .route("/api/collection/runs/{run}", get(read))
        .route("/api/collection/runs/{run}/works", get(works))
        .route("/api/collection/runs/{run}/commands", post(command))
        .route(
            "/api/collection/runs/{run}/commands/{identity}",
            get(command_receipt),
        )
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Probe {
    install_key: String,
    installation_credential: String,
    task_id: Uuid,
}
async fn probe(State(state): State<LocalWebState>, Json(q): Json<Probe>) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    match linggan_evidence::probe_collection_run_control(
        db,
        &q.install_key,
        &q.installation_credential,
        q.task_id,
    )
    .await
    {
        Ok(v) => Json(v).into_response(),
        Err(linggan_evidence::CollectionControlError::InvalidCredential) => {
            local_read_json_error(StatusCode::UNAUTHORIZED, "installation_credential_invalid")
        }
        Err(_) => {
            local_read_json_error(StatusCode::CONFLICT, "collection_run_action_not_authorized")
        }
    }
}
fn result(r: Result<serde_json::Value, RunError>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(RunError::Invalid(code)) => {
            local_read_json_error(StatusCode::UNPROCESSABLE_ENTITY, code)
        }
        Err(RunError::Conflict) => {
            local_read_json_error(StatusCode::CONFLICT, "collection_run_conflict")
        }
        Err(RunError::NotFound) => {
            local_read_json_error(StatusCode::NOT_FOUND, "collection_run_not_found")
        }
        Err(error) => {
            eprintln!("collection run local operation failed: {error}");
            local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "collection_run_unavailable",
            )
        }
    }
}
/// 单次采集的推进间隔：一段评论交卷后，最多等这么久就排下一段。
const RUN_ADVANCE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15);

/// 本服务自己推进单次采集（Mog 2026-10-23）。巡查 worker 停用时，页面上建的单次采集（含证据库的评论深采）
/// 由这里定时推进：只调用 `advance_runs`——按已接纳的交付冻结范围、准入下一段，不访问平台、不调模型，
/// 也不碰巡查、建档补采等其它自动任务。它和 worker 同时运行也安全：每次采集一把咨询锁，抢不到就留给对方。
pub(super) fn spawn_run_advancer(database: linggan_storage_postgres::Database) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(RUN_ADVANCE_INTERVAL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(error) =
                linggan_evidence::collection_run_progress::advance_runs(&database).await
            {
                eprintln!("linggan local runtime: collection run advancement failed: {error}");
            }
        }
    });
}
fn disconnected() -> Response {
    local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected")
}
async fn create(State(state): State<LocalWebState>, Json(input): Json<CreateRun>) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    let domain = input.domain_ref;
    // The owner validates the domain and includes it in the durable request hash, including
    // replay. Return the confirmed scope alongside the identity for the React receipt parser.
    result(
        collection_run::create_run(db, &input)
            .await
            .map(|mut receipt| {
                receipt["domainRef"] = serde_json::json!(domain);
                receipt
            }),
    )
}
async fn list(State(state): State<LocalWebState>, Query(q): Query<RunQuery>) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(collection_run::list_runs(db, &q).await)
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Domain {
    domain_ref: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WorksQuery {
    domain_ref: Uuid,
    page: Option<i64>,
}
async fn works(
    State(state): State<LocalWebState>,
    Path(run): Path<Uuid>,
    Query(q): Query<WorksQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(collection_run::read_run_works(db, run, q.domain_ref, q.page.unwrap_or(1)).await)
}
async fn read(
    State(state): State<LocalWebState>,
    Path(run): Path<Uuid>,
    Query(q): Query<Domain>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(collection_run::read_run(db, run, q.domain_ref).await)
}
async fn command(
    State(state): State<LocalWebState>,
    Path(run): Path<Uuid>,
    Json(input): Json<RunCommand>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(collection_run::command_run(db, run, &input).await)
}
async fn request_receipt(
    State(state): State<LocalWebState>,
    Path(identity): Path<Uuid>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    let found:Result<Option<serde_json::Value>,sqlx::Error>=sqlx::query_scalar("SELECT jsonb_build_object('runRef',run_ref,'domainRef',domain_ref,'replayed',true) FROM collection_run WHERE idempotency_key=$1").bind(identity).fetch_optional(db.pool()).await;
    result(
        found
            .map_err(RunError::Database)
            .and_then(|r| r.ok_or(RunError::NotFound)),
    )
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TargetQuery {
    domain_ref: Uuid,
    q: Option<String>,
    page: Option<i64>,
    selected: Option<String>,
}
async fn targets(State(state): State<LocalWebState>, Query(q): Query<TargetQuery>) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(read_targets(db, q).await)
}
async fn read_targets(
    db: &linggan_storage_postgres::Database,
    q: TargetQuery,
) -> Result<serde_json::Value, RunError> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let term = q.q.as_deref().unwrap_or("");
    let page = q.page.unwrap_or(1).clamp(1, 1000000);
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM collection_observation_target t JOIN observation_domain_target d USING(target_ref) WHERE d.domain_ref=$1 AND t.purpose_kind='long_term' AND t.lifecycle_state<>'dismissed' AND t.platform='xhs' AND ($2='' OR strpos(lower(COALESCE(t.display_name,'')||' '||t.identity_key),lower($2))>0)").bind(q.domain_ref).bind(term).fetch_one(&mut *tx).await?;
    let mut items:Vec<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('target_ref',t.target_ref,'name',COALESCE(t.display_name,t.identity_key),'target_kind',t.target_kind) FROM collection_observation_target t JOIN observation_domain_target d USING(target_ref) WHERE d.domain_ref=$1 AND t.purpose_kind='long_term' AND t.lifecycle_state<>'dismissed' AND t.platform='xhs' AND ($2='' OR strpos(lower(COALESCE(t.display_name,'')||' '||t.identity_key),lower($2))>0) ORDER BY t.first_stored_at DESC,t.target_ref LIMIT 20 OFFSET $3").bind(q.domain_ref).bind(term).bind((page-1)*20).fetch_all(&mut *tx).await?;
    if page == 1 {
        if let Some(selected) = q.selected.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
            let row:Option<serde_json::Value>=sqlx::query_scalar("SELECT jsonb_build_object('target_ref',t.target_ref,'name',COALESCE(t.display_name,t.identity_key),'target_kind',t.target_kind) FROM collection_observation_target t JOIN observation_domain_target d USING(target_ref) WHERE t.target_ref=$1 AND d.domain_ref=$2 AND t.purpose_kind='long_term' AND t.lifecycle_state<>'dismissed' AND t.platform='xhs'").bind(selected).bind(q.domain_ref).fetch_optional(&mut *tx).await?;
            if let Some(row) = row {
                if !items.iter().any(|v| v["target_ref"] == row["target_ref"]) {
                    items.push(row);
                }
            }
        }
    }
    tx.commit().await?;
    Ok(serde_json::json!({"items":items,"total":total,"page":page,"pageSize":20}))
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExecutionQuery {
    domain_ref: Option<Uuid>,
    run_ref: Option<Uuid>,
    page: Option<i64>,
}
async fn executions(
    State(state): State<LocalWebState>,
    Query(q): Query<ExecutionQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    result(collection_run::executions(db, q.domain_ref, q.run_ref, q.page.unwrap_or(1)).await)
}

// Reads the existing durable command receipt. Absence is unconfirmed, never a replay instruction.
async fn command_receipt(
    State(state): State<LocalWebState>,
    Path((run, identity)): Path<(Uuid, Uuid)>,
    Query(q): Query<Domain>,
) -> Response {
    let Some(db) = state.database.database() else {
        return disconnected();
    };
    let found: Result<Option<serde_json::Value>, sqlx::Error> = sqlx::query_scalar(
        "SELECT c.receipt FROM collection_run_command c JOIN collection_run r USING(run_ref) WHERE c.command_ref=$1 AND c.run_ref=$2 AND r.domain_ref=$3"
    ).bind(identity).bind(run).bind(q.domain_ref).fetch_optional(db.pool()).await;
    result(
        found
            .map_err(RunError::Database)
            .and_then(|r| r.ok_or(RunError::NotFound)),
    )
}
