//! Current comment catalog and research workflow routes share the local origin guard.
use super::{LocalDatabaseState, LocalWebState};
use axum::{
    Json, Router,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use serde_json::json;
#[path = "comment_study_api.rs"]
mod catalog_api;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .merge(catalog_api::routes())
        .merge(catalog_api::command_routes())
        .layer(middleware::from_fn(local_comment_study_guard))
}
pub(super) async fn local_comment_study_guard(request: Request, next: Next) -> Response {
    if !allowed_local_or_public_origin(&request) {
        return error(StatusCode::FORBIDDEN, "local_comment_study_origin_rejected");
    }
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    response
}

fn allowed_local_or_public_origin(request: &Request) -> bool {
    allowed_origin(request.headers())
        || request
            .extensions()
            .get::<super::public_access::PublicAccess>()
            .is_some_and(|access| access.allows_public_origin(request.headers()))
}

fn allowed_origin(headers: &HeaderMap) -> bool {
    let Some(host) = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let Some((name, port)) = host.rsplit_once(':') else {
        return false;
    };
    if !matches!(name, "127.0.0.1" | "localhost") || port.parse::<u16>().is_err() {
        return false;
    }
    if headers
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !matches!(value, "same-origin" | "none"))
    {
        return false;
    }
    headers.get(header::ORIGIN).is_none_or(|origin| {
        origin
            .to_str()
            .is_ok_and(|value| value == format!("http://{host}"))
    })
}

fn error(status: StatusCode, code: &str) -> Response {
    (status, Json(json!({"error":code}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_site_requests_are_rejected() {
        let mut h = HeaderMap::new();
        h.insert(header::HOST, "127.0.0.1:3000".parse().unwrap());
        h.insert(header::ORIGIN, "https://other.example".parse().unwrap());
        assert!(!allowed_origin(&h));
        h.insert(header::ORIGIN, "http://127.0.0.1:3000".parse().unwrap());
        assert!(allowed_origin(&h));
    }
}
