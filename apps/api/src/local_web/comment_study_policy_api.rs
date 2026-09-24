//! Method storage endpoints; never activate a policy or start research.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::{Path, Query, State, rejection::{JsonRejection, QueryRejection}},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_intelligence::{
    comment_study_catalog::StudyCatalogError,
    comment_study_policy::{CreateStudyPolicyCommand, StudyPolicyContractError, StudyPolicyQuery,
        StudyPolicyStoreError, create_study_policy, read_study_policies, read_study_policy},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/comment-study/policies", get(list).post(create))
        .route("/api/local/comment-study/policies/{policy_ref}", get(detail))
}

async fn list(
    State(state): State<LocalWebState>, query: Result<Query<StudyPolicyQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid(); };
    let LocalDatabaseState::Ready(db) = &state.database else { return unavailable(); };
    response(read_study_policies(db, &query).await, StatusCode::OK)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DetailQuery { domain: Uuid }

async fn detail(
    State(state): State<LocalWebState>, Path(reference): Path<String>,
    query: Result<Query<DetailQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else { return invalid(); };
    let Ok(reference) = Uuid::parse_str(&reference) else { return invalid(); };
    let LocalDatabaseState::Ready(db) = &state.database else { return unavailable(); };
    response(read_study_policy(db, query.domain, reference).await, StatusCode::OK)
}

async fn create(
    State(state): State<LocalWebState>, body: Result<Json<CreateStudyPolicyCommand>, JsonRejection>,
) -> Response {
    let Ok(Json(command)) = body else { return invalid(); };
    if let Err(error) = command.validate() {
        return response(Err(error.into()), StatusCode::CREATED);
    }
    let LocalDatabaseState::Ready(db) = &state.database else { return unavailable(); };
    response(create_study_policy(db, command).await, StatusCode::CREATED)
}

fn invalid() -> Response {
    error(StatusCode::BAD_REQUEST, "invalid_request", "研究方法参数不符合约定。", false)
}
fn unavailable() -> Response {
    error(StatusCode::SERVICE_UNAVAILABLE, "catalog_unavailable", "研究方法暂时无法读取或保存。", true)
}

fn response(result: Result<Value, StudyPolicyStoreError>, success: StatusCode) -> Response {
    use StudyPolicyContractError as C;
    use StudyPolicyStoreError as E;
    let failure = match result { Ok(value) => return (success, Json(value)).into_response(), Err(e) => e };
    let (status, code, message, retryable) = match failure {
        E::NotFound => (StatusCode::NOT_FOUND,"resource_not_found","未找到可访问的方法版本。",false),
        E::Unrecorded => (StatusCode::CONFLICT,"policy_unrecorded","历史方法未完整记录，请新建完整版本。",false),
        E::ModelDisabled | E::ModelUnavailable =>
            (StatusCode::CONFLICT,"policy_unavailable","方法引用的模型配置不存在或已停用。",false),
        E::SchemaUnavailable =>
            (StatusCode::SERVICE_UNAVAILABLE,"study_schema_unavailable","方法存储升级尚未就绪；不会自动迁移数据库。",false),
        E::Contract(C::UnsupportedDomain) =>
            (StatusCode::BAD_REQUEST,"unsupported_domain","当前评论研究不支持这个领域。",false),
        E::Contract(C::InvalidRequest) => return invalid(),
        E::Contract(_) =>
            (StatusCode::CONFLICT,"policy_unavailable","方法记录不完整、校验失败或版本不受支持。",false),
        E::Catalog(StudyCatalogError::InvalidCursor) =>
            (StatusCode::BAD_REQUEST,"invalid_cursor","翻页位置无效，请刷新列表。",false),
        E::Catalog(StudyCatalogError::CursorScopeMismatch) =>
            (StatusCode::BAD_REQUEST,"cursor_scope_mismatch","翻页位置与当前方法目录不匹配。",false),
        E::Catalog(StudyCatalogError::InvalidLimit) =>
            (StatusCode::BAD_REQUEST,"invalid_limit","每页数量必须在1–100之间。",false),
        E::Database(sqlx::Error::Database(ref e)) if e.code().as_deref() == Some("55P03") =>
            (StatusCode::CONFLICT,"study_busy","方法配置正在更新，本次没有保存。",true),
        E::Database(sqlx::Error::Database(ref e)) if e.code().as_deref() == Some("57014") =>
            (StatusCode::SERVICE_UNAVAILABLE,"query_timeout","本次方法查询或保存超时。",true),
        _ => return unavailable(),
    };
    error(status, code, message, retryable)
}

pub(super) fn error(status: StatusCode, code: &str, message: &str, retryable: bool) -> Response {
    (status, Json(json!({"error":{"code":code,"message":message,"retryable":retryable,"details":{}},
        "requestRef":null}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::{Body, to_bytes}, http::{Request, header}, middleware};
    use std::{collections::BTreeSet, sync::Arc};
    use tower::ServiceExt;

    fn app() -> Router {
        routes().layer(middleware::from_fn(crate::local_web::comment_study::local_comment_study_guard))
            .with_state(LocalWebState { database:LocalDatabaseState::NotConfigured,
                active_media_sessions:Arc::new(tokio::sync::Mutex::new(BTreeSet::new())), account_digest_key:None })
    }

    #[tokio::test]
    async fn policy_routes_apply_origin_guard_before_storage_and_set_no_store() {
        let denied = app().oneshot(Request::builder().uri("/api/local/comment-study/policies")
            .header(header::HOST,"localhost:3000").header(header::ORIGIN,"https://other.invalid")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        let body: Value = serde_json::from_slice(&to_bytes(denied.into_body(),4096).await.unwrap()).unwrap();
        assert_eq!(body["error"], "local_comment_study_origin_rejected");
        let valid = app().oneshot(Request::builder().uri(format!("/api/local/comment-study/policies?domain={}",
            linggan_intelligence::comment_study_source::ADHD_DOMAIN_REF)).header(header::HOST,"localhost:3000")
            .body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(valid.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(valid.headers()[header::CACHE_CONTROL], "no-store");
    }

    #[tokio::test]
    async fn invalid_json_content_type_and_extra_fields_never_reach_storage() {
        for (content_type, body) in [("text/plain","{}"),("application/json","{"),
            ("application/json",r#"{"methodHash":"client-forged"}"#)] {
            let response = app().oneshot(Request::builder().method("POST").uri("/api/local/comment-study/policies")
                .header(header::HOST,"localhost:3000").header(header::CONTENT_TYPE,content_type)
                .body(Body::from(body)).unwrap()).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn errors_do_not_leak_database_or_model_content() {
        let value = response(Err(StudyPolicyStoreError::Database(sqlx::Error::Protocol(
            "secret-and-body-must-not-leak".into()))), StatusCode::OK);
        let bytes = to_bytes(value.into_body(),4096).await.unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("secret-and-body"));
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body["error"]["code"],"catalog_unavailable");
        assert_eq!(body["error"]["details"],json!({}));
        assert!(body.get("items").is_none());
    }
}
