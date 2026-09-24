//! P2 command adapter, NOT composed into the live router before UI/dispatch cutover.
//! Tests exercise this exact router; no alternate public URL or enable flag is added.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Path, RawQuery, Request, State, rejection::JsonRejection},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
};
use linggan_intelligence::{
    comment_study_policy::{ActivateStudyPolicyCommand, StudyPolicyContractError,
        StudyPolicyStoreError, activate_study_policy},
    comment_study_run::{StudyStartError, TrustedStudyOrigin, preview_study_selection, start_study_run},
    comment_study_selection::{SelectionPreviewCommand, StartStudyRunCommand, StudySelectionError},
    comment_study_source::ADHD_DOMAIN_REF,
};
use serde_json::json;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/comment-study/selection-preview", post(preview))
        .route("/api/local/comment-study/runs", post(start))
        .route("/api/local/comment-study/policies/{policy_ref}/activate", post(activate))
        .route("/api/local/comment-study/policy", post(retired))
        .layer(middleware::from_fn(command_guard))
}

async fn command_guard(request: Request, next: Next) -> Response {
    // Reuse the existing Host/Origin predicate, not a second access policy.
    let mut response = if super::super::allowed_origin(request.headers()) {
        next.run(request).await
    } else {
        error(StatusCode::FORBIDDEN, "local_access_denied", "不允许从此来源访问本机研究。", false, None)
    };
    response.headers_mut().insert(header::CACHE_CONTROL, header::HeaderValue::from_static("no-store"));
    response.headers_mut().insert(header::X_CONTENT_TYPE_OPTIONS, header::HeaderValue::from_static("nosniff"));
    response
}

async fn preview(
    State(state): State<LocalWebState>, RawQuery(query): RawQuery,
    body: Result<Json<SelectionPreviewCommand>, JsonRejection>,
) -> Response {
    if query.is_some_and(|q| !q.is_empty()) { return invalid(None); }
    let Ok(Json(command)) = body else { return invalid(None); };
    let command = match command.normalize() {
        Ok(command) => command,
        Err(e) => return failure(e.into(), None),
    };
    let LocalDatabaseState::Ready(db) = &state.database else { return database_unavailable(&state, None); };
    match preview_study_selection(db, command).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => failure(e, None),
    }
}

async fn start(
    State(state): State<LocalWebState>, RawQuery(query): RawQuery,
    body: Result<Json<StartStudyRunCommand>, JsonRejection>,
) -> Response {
    if query.is_some_and(|q| !q.is_empty()) { return invalid(None); }
    let Ok(Json(command)) = body else { return invalid(None); };
    let reference = (!command.request_ref.is_nil()).then_some(command.request_ref);
    let command = match command.normalize() {
        Ok(command) => command,
        Err(e) => return failure(e.into(), reference),
    };
    let LocalDatabaseState::Ready(db) = &state.database else { return database_unavailable(&state, reference); };
    match start_study_run(db, command, TrustedStudyOrigin::Manual).await {
        Ok(receipt) => {
            let status = if receipt.outcome == "created" && !receipt.idempotent_replay {
                StatusCode::CREATED
            } else { StatusCode::OK };
            (status, Json(receipt)).into_response()
        }
        Err(e) => failure(e, reference),
    }
}

async fn activate(
    State(state): State<LocalWebState>, Path(reference): Path<String>, RawQuery(query): RawQuery,
    body: Result<Json<ActivateStudyPolicyCommand>, JsonRejection>,
) -> Response {
    if query.is_some_and(|q| !q.is_empty()) { return invalid(None); }
    let Ok(reference) = Uuid::parse_str(&reference) else { return invalid(None); };
    let Ok(Json(command)) = body else { return invalid(None); };
    if reference.is_nil() || command.validate().is_err() { return invalid(None); }
    let LocalDatabaseState::Ready(db) = &state.database else { return database_unavailable(&state, None); };
    // The approved release has one supported domain and the existing singleton default.
    // Never infer scope from an arbitrary policy UUID or accept undeclared JSON fields.
    let domain = Uuid::parse_str(ADHD_DOMAIN_REF).expect("static study domain");
    match activate_study_policy(db, domain, reference, command).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => failure(e.into(), None),
    }
}

async fn retired() -> Response {
    error(StatusCode::GONE, "policy_endpoint_retired", "旧策略写入已退役，请保存方法版本。", false, None)
}

fn invalid(reference: Option<Uuid>) -> Response {
    error(StatusCode::BAD_REQUEST, "invalid_request", "研究命令参数不符合约定。", false, reference)
}

fn database_unavailable(state: &LocalWebState, reference: Option<Uuid>) -> Response {
    if matches!(&state.database, LocalDatabaseState::SchemaUnavailable(_)) {
        failure(StudyStartError::SchemaUnavailable, reference)
    } else {
        error(StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable",
            "数据库暂不可用；重试开始请求时请沿用原请求编号。", true, reference)
    }
}

fn failure(cause: StudyStartError, reference: Option<Uuid>) -> Response {
    use StudyPolicyContractError as C;
    use StudyPolicyStoreError as P;
    use StudySelectionError as S;
    use StudyStartError as E;
    let (status, code, message, retryable) = match cause {
        E::Selection(S::InvalidRequest) | E::Policy(P::Contract(C::InvalidRequest)) => return invalid(reference),
        E::Selection(S::InvalidLimit) =>
            (StatusCode::BAD_REQUEST, "invalid_limit", "本次研究预算超出允许范围。", false),
        E::Selection(S::UnsupportedDomain) | E::Policy(P::Contract(C::UnsupportedDomain)) =>
            (StatusCode::BAD_REQUEST, "unsupported_domain", "当前评论研究不支持这个领域。", false),
        E::NotFound | E::Policy(P::NotFound) =>
            (StatusCode::NOT_FOUND, "resource_not_found", "未找到可访问的方法或所选材料。", false),
        E::IdempotencyConflict =>
            (StatusCode::CONFLICT, "idempotency_conflict", "请求编号已用于其他命令；新意图须使用新编号。", false),
        E::Policy(P::ActiveConflict) =>
            (StatusCode::CONFLICT, "control_version_conflict", "默认方法已变化，请刷新后再确认。", false),
        E::Policy(P::Unrecorded) =>
            (StatusCode::CONFLICT, "policy_unrecorded", "历史方法未完整记录，请新建完整版本。", false),
        E::Policy(P::ModelDisabled | P::ModelUnavailable | P::Contract(_)) =>
            (StatusCode::CONFLICT, "policy_unavailable", "方法版本或模型配置当前不可用。", false),
        E::SchemaUnavailable | E::Policy(P::SchemaUnavailable) =>
            (StatusCode::SERVICE_UNAVAILABLE, "study_schema_unavailable", "研究升级尚未就绪；不会自动建表。", false),
        E::Busy =>
            (StatusCode::CONFLICT, "study_busy", "研究正在处理并发命令，可使用原请求编号重试。", true),
        E::QueryTimeout =>
            (StatusCode::SERVICE_UNAVAILABLE, "query_timeout", "研究查询超时，可使用原请求编号重试。", true),
        E::BuildUnrecorded =>
            (StatusCode::SERVICE_UNAVAILABLE, "model_runtime_unavailable", "当前构建未完整记录，不能开始研究。", false),
        E::Database(sqlx::Error::Database(e)) | E::Policy(P::Database(sqlx::Error::Database(e))) => {
            return database_failure(e.code().as_deref(), reference);
        }
        E::ReceiptInvalid | E::Selection(S::InvalidSnapshot) =>
            (StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable", "研究记录完整性检查未通过。", false),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable",
            "数据库暂不可用；重试开始请求时请沿用原请求编号。", true),
    };
    error(status, code, message, retryable, reference)
}

fn database_failure(code: Option<&str>, reference: Option<Uuid>) -> Response {
    match code {
        Some("40001" | "40P01" | "55P03") => failure(StudyStartError::Busy, reference),
        Some("57014") => failure(StudyStartError::QueryTimeout, reference),
        Some("42P01" | "42703" | "42883") => failure(StudyStartError::SchemaUnavailable, reference),
        _ => error(StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable",
            "数据库暂不可用；重试开始请求时请沿用原请求编号。", true, reference),
    }
}

fn error(status: StatusCode, code: &str, message: &str, retryable: bool, reference: Option<Uuid>) -> Response {
    (status, Json(json!({"error":{"code":code,"message":message,"retryable":retryable,"details":{}},
        "requestRef":reference}))).into_response()
}

#[cfg(test)]
#[path = "comment_study_command_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "comment_study_command_postgres.rs"]
mod postgres_tests;
