//! Explicit, private visual parity preview against the current local database.
//! This starts only an Axum router: no main, scheduler, worker, migration or seed path runs.
//! PostgreSQL startup options make every pooled transaction read-only; an outer HTTP guard
//! rejects every non-GET/HEAD request before any route or command receipt code can run.
use super::*;
use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    middleware::Next,
};
use sqlx::{ConnectOptions, postgres::PgConnectOptions};
use std::{collections::BTreeMap, fs, path::PathBuf, str::FromStr};
use tower::ServiceExt;

fn runtime_environment() -> BTreeMap<String, String> {
    let path = std::env::var_os("WEB_REACT_COLLECTION_RUNTIME_ENV")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").expect("local home directory"))
                .join("Library/Application Support/Linggan Intelligence/runtime-main/.env")
        });
    let source =
        fs::read_to_string(path).unwrap_or_else(|_| panic!("runtime environment cannot be read"));
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (name, value) = line
                .strip_prefix("export ")
                .unwrap_or(line)
                .split_once('=')?;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .or_else(|| {
                    value
                        .strip_prefix('\'')
                        .and_then(|value| value.strip_suffix('\''))
                })
                .unwrap_or(value);
            Some((name.trim().to_owned(), value.to_owned()))
        })
        .collect()
}

async fn read_only_database(env: &BTreeMap<String, String>) -> Database {
    let required = |name: &str| {
        env.get(name)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| panic!("runtime database configuration is incomplete"))
    };
    let port = required("POSTGRES_PORT")
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .expect("runtime database loopback port");
    let options = PgConnectOptions::new()
        .host("127.0.0.1")
        .port(port)
        .username(required("POSTGRES_USER"))
        .password(required("POSTGRES_PASSWORD"))
        .database(required("POSTGRES_DB"))
        .options([
            ("default_transaction_read_only", "on"),
            ("application_name", "web-react-collection-readonly-preview"),
        ]);
    // sqlx 0.9's to_url_lossy omits startup options. Copy only this explicitly bounded
    // option string into the URL before passing through the canonical Database pool owner.
    let mut url = options.to_url_lossy();
    url.query_pairs_mut().append_pair(
        "options",
        options.get_options().expect("read-only startup options"),
    );
    let reparsed = PgConnectOptions::from_str(url.as_str())
        .unwrap_or_else(|_| panic!("read-only pool options are invalid"));
    assert!(
        reparsed
            .get_options()
            .is_some_and(|value| value.contains("default_transaction_read_only=on"))
    );
    let database = Database::connect(url.as_str())
        .await
        .unwrap_or_else(|_| panic!("read-only preview database cannot connect"));
    // Hold all four so the assertion checks every pooled connection, including later opens.
    let mut held = Vec::new();
    for _ in 0..4 {
        let mut connection = database
            .pool()
            .acquire()
            .await
            .expect("read-only pool connection");
        let setting: String = sqlx::query_scalar("SHOW default_transaction_read_only")
            .fetch_one(&mut *connection)
            .await
            .expect("read-only setting can be read");
        assert_eq!(setting, "on", "preview pool must be read-only");
        held.push(connection);
    }
    drop(held);
    database
}

async fn read_only_http(request: Request<Body>, next: Next) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return (StatusCode::METHOD_NOT_ALLOWED, "Read-only visual preview").into_response();
    }
    next.run(request).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "private read-only current-data visual preview; runs until interrupted"]
async fn readonly_collection_frontend_preview() {
    assert_eq!(
        std::env::var("WEB_REACT_COLLECTION_READONLY_PREVIEW").as_deref(),
        Ok("1"),
        "preview requires explicit opt-in"
    );
    let env = runtime_environment();
    assert_eq!(
        std::env::var("LINGGAN_COLLECTION_UPGRADE_PHASE")
            .ok()
            .as_deref(),
        env.get("LINGGAN_COLLECTION_UPGRADE_PHASE")
            .map(String::as_str),
        "the nonsecret collection read phase must match runtime; pass LINGGAN_COLLECTION_UPGRADE_PHASE explicitly when launching preview"
    );
    let database = read_only_database(&env).await;
    let account_available = env
        .get("LINGGAN_ACCOUNT_DIGEST_KEY")
        .is_some_and(|value| value.len() >= 32);
    let application = router(LocalWebState {
        database: LocalDatabaseState::Ready(Arc::new(database)),
        active_media_sessions: Arc::new(tokio::sync::Mutex::new(BTreeSet::new())),
        // Only presence affects GET projection. Do not copy the actual runtime identity key.
        account_digest_key: account_available.then(|| Arc::new(vec![0x42; 32])),
    })
    .layer(axum::middleware::from_fn(read_only_http));
    let forbidden = application
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/collection/commands/runtime-register")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("guard probe request"),
        )
        .await
        .expect("guard probe reply");
    assert_eq!(
        forbidden.status(),
        StatusCode::METHOD_NOT_ALLOWED,
        "outer middleware must reject commands before execution"
    );
    let port = std::env::var("WEB_REACT_COLLECTION_READONLY_PORT")
        .unwrap_or_else(|_| "3107".to_owned())
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0 && *port != 3000)
        .expect("dedicated preview port other than 3000");
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port))
        .await
        .expect("dedicated loopback preview listener");
    println!("COLLECTION_READONLY_PREVIEW_URL=http://127.0.0.1:{port}/collection/targets");
    println!(
        "COLLECTION_READONLY_PREVIEW_GUARDS=all_pool_connections_read_only;GET_HEAD_only;no_background_services"
    );
    axum::serve(listener, application)
        .await
        .expect("read-only visual preview server");
}
