//! Local guarded audit commands; no image bytes or provider credentials accepted from a browser.
use super::{
    LocalWebState,
    ai_settings::{invalid, respond, unavailable},
};
use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use linggan_intelligence::ocr_quality_audit::{report, review, source, store};
use uuid::Uuid;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ocr-audit", get(read))
        .route("/api/local/ocr-audit/settings", post(save))
        .route("/api/local/ocr-audit/runs", post(start).get(recent))
        .route("/api/local/ocr-audit/runs/{id}", get(run))
        .route("/api/local/ocr-audit/samples/{id}", get(sample))
        .route("/api/local/ocr-audit/readings/{id}", get(reading))
        .route("/api/local/ocr-audit/reviews", post(review))
        .route("/api/local/ocr-audit/probe-image", get(probe_image))
}
async fn read(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    respond(store::read_settings(db).await)
}
async fn recent(State(s): State<LocalWebState>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    respond(report::recent(db).await)
}
async fn run(State(s): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    respond(report::report(db, id).await)
}
async fn sample(State(s): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    respond(report::detail(db, id).await)
}
async fn reading(State(s): State<LocalWebState>, Path(id): Path<Uuid>) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    respond(report::latest_for_reading(db, id).await)
}
async fn save(State(s): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    let Ok(r) = serde_json::from_slice::<store::SaveSettings>(&body) else {
        return invalid();
    };
    respond(store::save_settings(db, &r).await)
}
async fn start(State(s): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    let Ok(r) = serde_json::from_slice::<store::StartRun>(&body) else {
        return invalid();
    };
    respond(store::start_run(db, &r).await)
}
async fn review(State(s): State<LocalWebState>, body: Bytes) -> Response {
    let Some(db) = s.database.database() else {
        return unavailable();
    };
    let Ok(r) = serde_json::from_slice::<review::Review>(&body) else {
        return invalid();
    };
    respond(review::review(db, &r).await)
}
async fn probe_image() -> Response {
    (
        [("Content-Type", "image/png"), ("Cache-Control", "no-store")],
        source::probe_image_bytes(),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[ignore = "owned preseeded synthetic PostgreSQL and built UI only"]
    async fn owned_audit_ui_reads_and_appends_a_human_opinion() {
        let uri = std::env::var("LOCAL_001_PROOF_DATABASE_URL").unwrap();
        assert!(uri.contains("@127.0.0.1:") && uri.contains("/linggan_intelligence_ocr_"));
        let directory = PathBuf::from(std::env::var_os("OCR_AUDIT_PROOF_OUTPUT").unwrap());
        let mut fixture: serde_json::Value =
            serde_json::from_slice(&std::fs::read(directory.join("fixture.json")).unwrap())
                .unwrap();
        assert_eq!(fixture["schema"], "ocr_quality_audit_owned");
        let db = linggan_storage_postgres::Database::connect_within_schema(
            &uri,
            "ocr_quality_audit_owned",
        )
        .await
        .unwrap();
        let app = super::super::app_with_database(db.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        fixture["origin"] = serde_json::json!(format!("http://{}", listener.local_addr().unwrap()));
        std::fs::write(directory.join("fixture.json"), fixture.to_string()).unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let status = tokio::task::spawn_blocking(|| {
            std::process::Command::new(std::env::var_os("AI_FOUNDATION_PROOF_NODE").unwrap())
                .arg(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../web/test/e2e/ocr-audit-real-chain.mjs"),
                )
                .status()
                .unwrap()
        })
        .await
        .unwrap();
        server.abort();
        let _ = server.await;
        assert!(status.success());
        let rows: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM linggan_ocr_audit_review WHERE actor='SYNTHETIC-BROWSER-NOT-MOG'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(rows, 1);
    }
}
