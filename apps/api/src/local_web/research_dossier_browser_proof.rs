//! Real React → Axum → owned PostgreSQL proof with synthetic source material only.
use super::*;
use std::path::PathBuf;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "owned PostgreSQL, production React build and browser only"]
async fn dossier_v2_browser_proof() {
    assert_eq!(std::env::var("DOSSIER_UI_PROOF").as_deref(), Ok("1"));
    let directory = PathBuf::from(std::env::var_os("WEB_APP_PROOF_DIR").unwrap());
    std::fs::create_dir_all(&directory).unwrap();
    let db = super::super::evidence_fixture::isolated("dossier_ui").await;
    let work = dossier_fixture::work(&db).await;
    let long_tail = "完整正文末尾🙂";
    let body = format!(
        "# 合成完整底稿\n\n{}{}\n\n- 条件一不可遗漏\n- 条件二有边界\n\n| 条件 | 读数 |\n| --- | --- |\n| 不成立 | 0.5 mg |\n\n> 他人的引述，归属未确认",
        "完整原文。".repeat(25000),
        long_tail
    );
    dossier_fixture::fixture::submit_package(&db,"content_detail",json!({"contentExternalId":"dossier-note"}),json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"dossier-note"},"payload":{"title":"合成底稿验收","bodyText":body,"authorId":"synthetic-author","likedCount":0,"hashtags":["#合成验收"]}})).await;
    let image_bytes = super::super::evidence_fixture::png_bytes(240, 320, [232, 238, 247]);
    dossier_fixture::current_image_reading_with_bytes(&db, Some(image_bytes)).await;
    let read = dossier::read(&db, work).await.unwrap().unwrap();
    let image = read
        .materials
        .iter()
        .find(|m| m["kind"] == "image")
        .unwrap()
        .clone();
    let mut tx = db.pool().begin().await.unwrap();
    let rich=reading::publish_in(&mut tx,reading::NewReading{
        content_public_ref:work,unit_key:format!("image:{}",image["slotKey"].as_str().unwrap()),kind:"image".into(),slot_key:image["slotKey"].as_str().map(str::to_owned),page:Some(1),source_version:image["sourceVersion"].as_str().unwrap().into(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"cloud_vision".into(),rule_version:"dossier-content.v2:synthetic-browser".into(),
        blocks:vec![json!({"ref":"image-heading","kind":"heading","text":"图片完整读本","speaker":"unconfirmed"}),json!({"ref":"image-table","kind":"table","columns":["条件","读数"],"rows":[["不适用","0.5 mg"]],"speaker":"unconfirmed","bbox":[0.1,0.2,0.7,0.2]}),json!({"ref":"image-list","kind":"list","ordered":true,"start":2,"items":["不可删除否定","条件有先后"],"speaker":"unconfirmed"}),json!({"ref":"image-dialogue","kind":"dialogue","turns":[{"speaker":"other","text":"他人的经验"},{"speaker":"author","text":"作者补充适用范围"}]})],
        issues:vec![json!({"key":"image-word","code":"glyph_uncertain","message":"表格的一字待核对","resolution":"reading_edit","blockRef":"image-table"}),json!({"key":"image-source","code":"source_boundary","message":"来源范围仍需单独确认","resolution":"source_refresh"})],source_location:json!({"materializationRef":image["sourceVersion"],"originalUrl":image["originalUrl"]}),publication:reading::Publication::Automatic{expected_current:serde_json::from_value(image["currentRevisionRef"].clone()).unwrap()}
    }).await.unwrap();
    assert!(rich.current);
    tx.commit().await.unwrap();
    let mut scenarios = vec![];
    for (name, body, images, video, partial, empty) in [
        ("multi-image", None, 3, false, false, false),
        (
            "body-plus-images",
            Some("一句话正文🙂"),
            2,
            false,
            false,
            false,
        ),
        ("video-transcript", None, 1, true, false, false),
        ("partial-images", None, 5, false, true, false),
        ("originals-only", None, 1, false, false, true),
    ] {
        let work = standard_fixture::scenario(&db, name, body, images, video, partial, empty).await;
        scenarios.push(json!({"name":name,"work":work,"domain":standard_fixture::DOMAIN,"dataSource":"synthetic saved-result replay; no recognition/model"}));
    }
    if let Some(path) = std::env::var_os("STANDARD_DOSSIER_SAVED_MANIFEST") {
        assert!(
            std::env::var_os("GITHUB_ACTIONS").is_none(),
            "private replay is local only"
        );
        let entries: Vec<Value> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for entry in entries {
            let work = standard_fixture::replay_saved(&db, &entry).await;
            scenarios.push(json!({"name":entry["name"],"work":work,"domain":standard_fixture::DOMAIN,"dataSource":entry["dataSource"],"sourceWorkRef":entry["sourceWorkRef"]}));
        }
    }
    // Saved-result replay includes a completed lifecycle; no recognizer is invoked.
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason) SELECT gen_random_uuid(),j.job_ref,'succeeded','offline_saved_result_replay' FROM linggan_media_processing_job j JOIN linggan_material_unit u ON u.slot_key=j.slot_key AND u.current_reading_ref IS NOT NULL AND u.kind=CASE j.processor_kind WHEN 'image_ocr' THEN 'image' WHEN 'asr' THEN 'audio' WHEN 'video_frame_ocr' THEN 'video_frame' ELSE '' END").execute(db.pool()).await.unwrap();
    let before:Value=sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_model_invocation),(SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_runtime_capture_package))").fetch_one(db.pool()).await.unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = json!({"origin":format!("http://{}",listener.local_addr().unwrap()),"domain":dossier_fixture::DOMAIN,"work":work,
        "imageUnitRef":image["ref"],"currentReadingRef":rich.reading_ref,"longTail":long_tail,"scenarios":scenarios});
    std::fs::write(directory.join("fixture.json"), fixture.to_string()).unwrap();
    let app = app_with_database(db.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let status = tokio::task::spawn_blocking(|| {
        std::process::Command::new(std::env::var_os("WEB_APP_PROOF_NODE").unwrap())
            .arg(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../web/test/e2e/research-dossier-v2.mjs"),
            )
            .status()
            .unwrap()
    })
    .await
    .unwrap();
    server.abort();
    let _ = server.await;
    assert!(status.success(), "actual browser proof failed");
    let after:Value=sqlx::query_scalar("SELECT jsonb_build_array((SELECT count(*) FROM linggan_model_invocation),(SELECT count(*) FROM linggan_media_processing_job),(SELECT count(*) FROM linggan_runtime_capture_package))").fetch_one(db.pool()).await.unwrap();
    assert_eq!(
        before, after,
        "review cannot dispatch a provider or recollect source material"
    );
    let commands: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_reading_command")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        commands, 1,
        "one explicit complete edit is recorded without a candidate adoption workflow"
    );
    let final_read = dossier::read(&db, work).await.unwrap().unwrap();
    let final_image = final_read
        .materials
        .iter()
        .find(|m| m["ref"] == image["ref"])
        .unwrap();
    assert_eq!(final_image["reading"]["adoption"], "human_edited");
    assert!(
        final_image["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == "image-source")
    );
    assert!(
        !final_image["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|i| i["key"] == "image-word")
    );
    db.pool().close().await;
}
