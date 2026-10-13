use axum::{
    http::{HeaderValue, header},
    response::{Html, IntoResponse, Response},
};

const PAGE: &str = include_str!("login.html");

/// Public development entry; it neither authenticates nor creates a session.
pub(super) async fn page() -> Response {
    let mut response = Html(PAGE).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

#[cfg(test)]
mod tests {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn login_page_works_without_a_database_and_does_not_set_a_session() {
        let response = super::super::app()
            .oneshot(
                Request::builder()
                    .uri("/login")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("set-cookie"));
        let body = to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap();
        let html = std::str::from_utf8(&body).unwrap();
        assert!(html.contains("开发模式无需真实账号"));
        assert!(html.contains("window.location.assign('/corpus')"));
        assert!(html.contains("connect-src 'none'; form-action 'none'"));
        assert!(html.contains("station-template"));
    }

    #[tokio::test]
    async fn login_page_does_not_accept_credentials() {
        let response = super::super::app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .body(Body::from("email=example&password=example"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
