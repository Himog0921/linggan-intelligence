//! Temporary real-API preview for an independently controlled browser. Synthetic only.
use super::super::{app_with_database, counts, standard_fixture};
use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

async fn read_only(request: Request, next: Next) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    next.run(request).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL and manually controlled browser; never launches a browser or worker"]
async fn dossier_reading_manual_preview() {
    assert_eq!(std::env::var("DOSSIER_UI_PROOF").as_deref(), Ok("1"));
    let directory =
        PathBuf::from(std::env::var_os("WEB_APP_PROOF_DIR").expect("preview directory supplied"));
    std::fs::create_dir_all(&directory).unwrap();
    let stop = directory.join("stop");
    if stop.exists() {
        std::fs::remove_file(&stop).unwrap();
    }
    let db = super::super::super::evidence_fixture::isolated("dossier_preview").await;
    let demo = standard_fixture::approved_demo(&db).await;
    let mut scenarios = vec![
        json!({"name":"approved-demo","work":demo,"domain":standard_fixture::DOMAIN,
        "dataSource":"approved synthetic demo text; 26 saved ASR segments, 292 seconds; no model invoked"}),
    ];
    for (name, body, images, video, partial, empty) in [
        (
            "twelve-images",
            Some("合成演示：把一项任务写清楚。"),
            12,
            false,
            false,
            false,
        ),
        ("thirteen-images", None, 13, false, false, false),
        ("partial-images", None, 12, false, true, false),
        ("video-transcript", None, 1, true, false, false),
        ("originals-only", None, 1, false, false, true),
    ] {
        let work = standard_fixture::scenario(&db, name, body, images, video, partial, empty).await;
        scenarios.push(
            json!({"name":name,"work":work,"domain":standard_fixture::DOMAIN,
            "dataSource":"fully synthetic saved-result replay; no recognizer/model invoked"}),
        );
    }
    // Replay represents saved completed results; it does not run the queued jobs.
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) SELECT gen_random_uuid(),j.job_ref,'succeeded','offline_saved_result_replay' FROM linggan_media_processing_job j JOIN linggan_material_unit u ON u.slot_key=j.slot_key AND u.current_reading_ref IS NOT NULL AND u.kind=CASE j.processor_kind WHEN 'image_ocr' THEN 'image' WHEN 'asr' THEN 'audio' WHEN 'video_frame_ocr' THEN 'video_frame' ELSE '' END").execute(db.pool()).await.unwrap();
    let before = counts(&db).await;
    let port = std::env::var("DOSSIER_PREVIEW_PORT")
        .ok()
        .map(|p| p.parse::<u16>().unwrap())
        .unwrap_or(0);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let fixture = json!({"origin":origin,"domain":standard_fixture::DOMAIN,"work":scenarios[0]["work"],
        "synthetic":true,"readOnly":true,"stopFile":stop,"scenarios":scenarios});
    std::fs::write(
        directory.join("fixture.json"),
        serde_json::to_vec_pretty(&fixture).unwrap(),
    )
    .unwrap();
    println!(
        "DOSSIER_MANUAL_PREVIEW {origin}; manifest={}",
        directory.join("fixture.json").display()
    );
    let seconds = std::env::var("DOSSIER_PREVIEW_SECONDS")
        .ok()
        .map(|n| n.parse::<u64>().unwrap())
        .unwrap_or(900)
        .clamp(1, 900);
    let app = app_with_database(db.clone()).layer(middleware::from_fn(read_only));
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let start = Instant::now();
            while !stop.exists() && start.elapsed() < Duration::from_secs(seconds) {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await
        .unwrap();
    assert_eq!(
        before,
        counts(&db).await,
        "manual reading must not mutate captures, jobs, readings, commands, snapshots or model invocations"
    );
    std::fs::write(
        directory.join("read-only-check.json"),
        json!({"passed":true,"before":before,"after":counts(&db).await}).to_string(),
    )
    .unwrap();
    db.pool().close().await;
}
