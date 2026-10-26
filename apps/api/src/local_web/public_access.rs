//! Explicit HTTPS entry for the internal team. The listener remains on loopback; a proxy may
//! forward one configured host, while every other Host keeps the existing local-only boundary.
use super::{LocalWebState, ops_auth};
use axum::{
    extract::{Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::{io, sync::Arc};

#[derive(Clone, Default)]
pub(super) struct PublicAccess {
    origin: Option<Arc<str>>,
    host: Option<Arc<str>>,
}

impl PublicAccess {
    pub(super) fn from_environment() -> io::Result<Self> {
        match std::env::var("LINGGAN_PUBLIC_ORIGIN") {
            Ok(value) => {
                if std::env::var("LINGGAN_OPS_COOKIE_SECURE").as_deref() != Ok("1") {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "LINGGAN_OPS_COOKIE_SECURE=1 is required with LINGGAN_PUBLIC_ORIGIN",
                    ));
                }
                Self::parse(&value)
            }
            Err(std::env::VarError::NotPresent) => Ok(Self::default()),
            Err(_) => Err(invalid_origin()),
        }
    }

    fn parse(value: &str) -> io::Result<Self> {
        let uri: axum::http::Uri = value.parse().map_err(|_| invalid_origin())?;
        let authority = uri.authority().ok_or_else(invalid_origin)?;
        if uri.scheme_str() != Some("https")
            || value != format!("https://{authority}")
            || authority.port_u16().is_some()
            || authority.as_str() != authority.host()
            || !authority.host().contains('.')
            || authority.host().parse::<std::net::IpAddr>().is_ok()
        {
            return Err(invalid_origin());
        }
        Ok(Self {
            origin: Some(Arc::from(value)),
            host: Some(Arc::from(authority.as_str())),
        })
    }

    fn is_public_host(&self, host: &str) -> bool {
        self.host.as_deref() == Some(host)
    }

    pub(super) fn allows_public_origin(&self, headers: &HeaderMap) -> bool {
        let Some(origin) = self.origin.as_deref() else {
            return false;
        };
        if !headers
            .get(header::HOST)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|host| self.is_public_host(host))
        {
            return false;
        }
        if headers
            .get("sec-fetch-site")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| !matches!(value, "same-origin" | "none"))
        {
            return false;
        }
        headers
            .get(header::ORIGIN)
            .is_none_or(|value| value.to_str().is_ok_and(|value| value == origin))
    }

    fn allows_public_request(&self, method: &Method, headers: &HeaderMap) -> bool {
        if !self.allows_public_origin(headers) {
            return false;
        }
        method == Method::GET
            || method == Method::HEAD
            || method == Method::OPTIONS
            || headers
                .get(header::ORIGIN)
                .and_then(|value| value.to_str().ok())
                == self.origin.as_deref()
    }
}

fn invalid_origin() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "LINGGAN_PUBLIC_ORIGIN must be one HTTPS hostname without a path or port",
    )
}

fn is_local_host(host: &str) -> bool {
    let Some((name, port)) = host.rsplit_once(':') else {
        return false;
    };
    matches!(name, "localhost" | "127.0.0.1") && port.parse::<u16>().is_ok_and(|port| port > 0)
}

fn anonymous_path(method: &Method, path: &str) -> bool {
    (method == Method::GET && matches!(path, "/" | "/login" | "/invite/accept"))
        || (method == Method::GET && path.starts_with("/ui-assets/"))
        || (method == Method::POST
            && matches!(
                path,
                "/api/local/auth/login" | "/api/local/auth/invitations/accept"
            ))
}

pub(super) async fn guard(
    State((state, access)): State<(LocalWebState, PublicAccess)>,
    mut request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let is_public = access.is_public_host(host);
    if access.origin.is_some() {
        if is_public {
            if request
                .headers()
                .get("x-forwarded-proto")
                .and_then(|value| value.to_str().ok())
                != Some("https")
            {
                return StatusCode::FORBIDDEN.into_response();
            }
            if !access.allows_public_request(request.method(), request.headers()) {
                return StatusCode::FORBIDDEN.into_response();
            }
            let path = request.uri().path();
            if path == "/health"
                || path == "/api/local/discovery-packages"
                || path.starts_with("/api/local/producer/")
                || path.starts_with("/api/local/stations")
                || path.starts_with("/api/local/dispatch/")
            {
                return StatusCode::FORBIDDEN.into_response();
            }
            if !anonymous_path(request.method(), path) {
                if let Err(response) = ops_auth::current_member(&state, request.headers()).await {
                    return response;
                }
            }
        } else if !is_local_host(host)
            || request.headers().contains_key("x-forwarded-proto")
            || request.headers().contains_key("cf-connecting-ip")
            || request.headers().contains_key("forwarded")
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }
    request.extensions_mut().insert(access);
    let mut response = next.run(request).await;
    if is_public && response.headers().get(header::CACHE_CONTROL).is_none() {
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("private, no-store"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{HeaderValue, Request},
    };
    use tower::ServiceExt;

    #[test]
    fn public_origin_is_exact_https_and_does_not_accept_cross_site_requests() {
        for invalid in [
            "http://intelligence.example.com",
            "https://localhost",
            "https://intelligence.example.com:443",
            "https://intelligence.example.com/path",
        ] {
            assert!(PublicAccess::parse(invalid).is_err(), "{invalid}");
        }
        let access = PublicAccess::parse("https://intelligence.example.com").unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            header::HOST,
            HeaderValue::from_static("intelligence.example.com"),
        );
        assert!(access.allows_public_request(&Method::GET, &headers));
        assert!(!access.allows_public_request(&Method::POST, &headers));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://intelligence.example.com"),
        );
        assert!(access.allows_public_request(&Method::POST, &headers));
        headers.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://elsewhere.example.com"),
        );
        assert!(!access.allows_public_request(&Method::POST, &headers));
    }

    #[test]
    fn only_login_and_static_assets_are_anonymous_on_the_public_host() {
        assert!(anonymous_path(&Method::GET, "/login"));
        assert!(anonymous_path(&Method::POST, "/api/local/auth/login"));
        assert!(!anonymous_path(&Method::GET, "/corpus/evidence"));
        assert!(!anonymous_path(&Method::GET, "/api/local/work-resources"));
        assert!(!anonymous_path(&Method::GET, "/api/local/media/id/hash"));
    }

    #[tokio::test]
    async fn public_router_denies_machine_and_untrusted_hosts_before_any_handler() {
        let app = super::super::router_with_public_access(
            LocalWebState {
                database: super::super::LocalDatabaseState::NotConfigured,
                active_media_sessions: Arc::new(tokio::sync::Mutex::new(Default::default())),
                account_digest_key: None,
            },
            PublicAccess::parse("https://intelligence.example.com").unwrap(),
        );
        for (host, forwarded, path, expected) in [
            (
                "intelligence.example.com",
                "https",
                "/health",
                StatusCode::FORBIDDEN,
            ),
            (
                "intelligence.example.com",
                "https",
                "/api/local/stations",
                StatusCode::FORBIDDEN,
            ),
            (
                "intelligence.example.com",
                "https",
                "/api/local/discovery-packages",
                StatusCode::FORBIDDEN,
            ),
            (
                "intelligence.example.com",
                "http",
                "/login",
                StatusCode::FORBIDDEN,
            ),
            (
                "elsewhere.example.com",
                "https",
                "/login",
                StatusCode::FORBIDDEN,
            ),
            ("localhost:3001", "https", "/health", StatusCode::FORBIDDEN),
        ] {
            let request = Request::builder()
                .uri(path)
                .header(header::HOST, host)
                .header("x-forwarded-proto", forwarded)
                .body(Body::empty())
                .unwrap();
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), expected, "{host} {path}");
        }
        let local = Request::builder()
            .uri("/health")
            .header(header::HOST, "localhost:3001")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            app.clone().oneshot(local).await.unwrap().status(),
            StatusCode::OK
        );
        let public_login = Request::builder()
            .uri("/login")
            .header(header::HOST, "intelligence.example.com")
            .header("x-forwarded-proto", "https")
            .body(Body::empty())
            .unwrap();
        assert_ne!(
            app.oneshot(public_login).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
    }
}
