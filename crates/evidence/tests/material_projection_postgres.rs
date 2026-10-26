#[allow(dead_code)]
#[path = "support/research_dossier_fixture.rs"]
mod dossier_fixture;
#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{
    coverage_layer, proof_database, submit_custom_package, submit_package, submit_package_at,
};
use linggan_contracts::{EvidenceQuery, TargetIdentity, TargetSource};
use linggan_evidence::{
    TargetDeletionOutcome, admit_media_blob, delete_observation_target,
    read_target_deletion_preview, read_work_resource, read_work_resources, store_pending_target,
};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn blank_platform_title_falls_back_only_to_an_accepted_front_cover_headline() {
    let database = proof_database("material_cover_ocr_title_fallback").await;
    let content_id = "note-cover-ocr-fallback";
    let slot_key = format!("xhs:{content_id}:cover:1");
    let observation_ref = Uuid::new_v4();
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":content_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id},
            "payload":{}
        }),
    )
    .await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":content_id}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":observation_ref,
            "slot":{"role":"cover","ordinal":1},
            "observation":{"externalUri":"https://media.example/cover-ocr-fallback.jpg","candidateUris":["https://media.example/cover-ocr-fallback.jpg"],"observedAt":"2026-09-20T10:00:00Z"},
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
        }),
    )
    .await;
    let blob_sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let materialization = admit_media_blob(
        &database,
        observation_ref,
        blob_sha256,
        "image/jpeg",
        4096,
        "blobs/aa/cover-ocr-fallback",
    )
    .await
    .unwrap()
    .materialization_ref;
    let content_ref:Uuid=sqlx::query_scalar("SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1").bind(content_id).fetch_one(database.pool()).await.unwrap();
    let mut tx = database.pool().begin().await.unwrap();
    linggan_evidence::material_reading::publish_in(&mut tx,linggan_evidence::material_reading::NewReading {
        content_public_ref:content_ref,unit_key:format!("image:{slot_key}"),kind:"image".into(),slot_key:Some(slot_key.clone()),page:None,source_version:materialization.to_string(),source_state:"present".into(),derivative_ref:None,producer_ref:Some(Uuid::new_v4()),generated_by:"cloud_vision".into(),rule_version:"synthetic-cover-current.v1".into(),
        blocks:vec![serde_json::json!({"kind":"heading","text":"封面里的作者标题","speaker":"unconfirmed","role":"main_title"})],issues:vec![],source_location:serde_json::json!({"blobSha256":blob_sha256,"materializationRef":materialization,"titleClaim":{"status":"present","text":"封面里的作者标题","eligible":true}}),publication:linggan_evidence::material_reading::Publication::Automatic{expected_current:None}
    }).await.unwrap();
    tx.commit().await.unwrap();
    let item = read_work_resource(&database, content_ref)
        .await
        .expect("the shared read projection succeeds")
        .expect("the work remains readable");
    assert_eq!(item.display.title.as_deref(), Some("封面里的作者标题"));
    assert_eq!(item.display.title_state, "KNOWN");
    assert_eq!(item.display.title_source, "cover_ocr");
    assert_eq!(item.display.title_media_display_ordinal, None);
    assert!(
        item.evidence_fragment.is_none(),
        "cover OCR may supply a disclosed title fallback but must not become the default evidence quote"
    );
    assert_eq!(
        item.inspector.pointer("/displayTitle/source"),
        Some(&serde_json::json!("cover_ocr")),
        "the fallback is disclosed instead of being presented as a platform field"
    );
    assert_eq!(item.media["ocr"]["state"], "ACQUIRED");
    assert_eq!(
        item.lane_summaries
            .iter()
            .find(|lane| lane.lane == "ocr")
            .unwrap()
            .state,
        "ACQUIRED"
    );
    assert_topic_overview_matches_detail(&database, content_ref).await;

    let current = linggan_evidence::research_dossier::read(&database, content_ref)
        .await
        .unwrap()
        .unwrap();
    let image = current
        .materials
        .iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap();
    let unit: Uuid = serde_json::from_value(image["ref"].clone()).unwrap();
    let saved=linggan_evidence::material_reading::review_reading(&database,unit,&linggan_evidence::material_reading::ReviewReadingRequest {
        command_ref:Uuid::new_v4(),unit_ref:unit.to_string(),source_version:image["sourceVersion"].as_str().unwrap().into(),expected_current_revision_ref:serde_json::from_value(image["currentRevisionRef"].clone()).unwrap(),action:"edit".into(),actor:"synthetic-cover-human".into(),reason:"核对完整合成封面标题".into(),edited_blocks:Some(vec![serde_json::json!({"kind":"heading","text":"人工核对封面标题","speaker":"unconfirmed"})]),verified:true,resolved_issue_keys:vec![]
    }).await.unwrap();
    let next_observation = Uuid::new_v4();
    submit_package(&database,"media_slots",serde_json::json!({"contentExternalId":content_id}),serde_json::json!({"kind":"media_slot","slotKey":slot_key,"observationRef":next_observation,"slot":{"role":"cover","ordinal":1},"observation":{"externalUri":"https://media.example/cover-reobserved.jpg","candidateUris":["https://media.example/cover-reobserved.jpg"],"observedAt":"2026-10-20T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":content_id}})).await;
    let same = admit_media_blob(
        &database,
        next_observation,
        blob_sha256,
        "image/jpeg",
        4096,
        "blobs/aa/cover-ocr-fallback",
    )
    .await
    .unwrap()
    .materialization_ref;
    assert_ne!(
        same, materialization,
        "same verified bytes have a genuinely new materialization"
    );
    let same_read = linggan_evidence::research_dossier::read(&database, content_ref)
        .await
        .unwrap()
        .unwrap();
    let human = same_read
        .materials
        .iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap();
    assert_eq!(human["currentRevisionRef"], saved["currentRevisionRef"]);
    assert_eq!(human["sourceVersion"], materialization.to_string());
    assert_eq!(same_read.note["displayTitle"], "人工核对封面标题");
    let shared = read_work_resource(&database, content_ref)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(shared.display.title.as_deref(), Some("人工核对封面标题"));
    assert_topic_overview_matches_detail(&database, content_ref).await;
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await
        .unwrap();

    let changed_observation = Uuid::new_v4();
    submit_package(&database,"media_slots",serde_json::json!({"contentExternalId":content_id}),serde_json::json!({"kind":"media_slot","slotKey":slot_key,"observationRef":changed_observation,"slot":{"role":"cover","ordinal":1},"observation":{"externalUri":"https://media.example/cover-changed.jpg","candidateUris":["https://media.example/cover-changed.jpg"],"observedAt":"2026-10-21T10:00:00Z"},"sourceObject":{"platform":"xhs","type":"content","externalId":content_id}})).await;
    admit_media_blob(
        &database,
        changed_observation,
        &"b".repeat(64),
        "image/jpeg",
        4096,
        "blobs/bb/cover-changed",
    )
    .await
    .unwrap();
    assert!(
        read_work_resource(&database, content_ref)
            .await
            .unwrap()
            .unwrap()
            .display
            .title
            .is_none(),
        "changed bytes cannot borrow the old human title"
    );
    let changed = linggan_evidence::research_dossier::read(&database, content_ref)
        .await
        .unwrap()
        .unwrap();
    assert!(changed.note["displayTitle"].is_null());
    let domain: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    let frozen =
        linggan_evidence::read_work_resource_overviews(&database, domain, &[content_ref], &cutoff)
            .await
            .unwrap()
            .pop()
            .unwrap();
    assert_eq!(
        frozen.display.title.as_deref(),
        Some("人工核对封面标题"),
        "historical asOf still uses its then-current verified bytes"
    );
    linggan_evidence::record_materialization_disposition(
        &database,
        materialization,
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic",
        "original reading source withdrawn",
    )
    .await
    .unwrap();
    let hidden =
        linggan_evidence::read_work_resource_overviews(&database, domain, &[content_ref], &cutoff)
            .await
            .unwrap()
            .pop()
            .unwrap();
    assert!(
        hidden.display.title.is_none(),
        "current source restrictions also gate historical shared titles"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn content_detail_submission_forms_a_typed_material_with_field_sources_and_unknowns() {
    let database = proof_database("material_detail_slice").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-1"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-material-1"},
            "payload":{
                "title":"逐字段来源标题",
                "bodyText":"可检索正文",
                "authorId":"author-material-1",
                "publishedAt":1713501296000_i64,
                "publishedAtText":"1713501296",
                "publishedAtSourceField":"publishTime",
                "publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    let row = sqlx::query(
        "SELECT title, title_state, body_text, body_state, creator_display_name_state, \
                published_at::text AS published_at,published_at_source_field, \
                published_at_source_kind,published_at_precision,published_at_parser_version, \
                package_ref, record_ordinal \
         FROM linggan_material_content_detail",
    )
    .fetch_one(database.pool())
    .await
    .expect("a qualified detail record forms one typed material row");

    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("逐字段来源标题")
    );
    assert_eq!(row.get::<String, _>("title_state"), "KNOWN");
    assert_eq!(
        row.get::<Option<String>, _>("body_text").as_deref(),
        Some("可检索正文")
    );
    assert_eq!(row.get::<String, _>("body_state"), "KNOWN");
    assert_eq!(
        row.get::<String, _>("creator_display_name_state"),
        "UNKNOWN"
    );
    assert!(row.get::<Option<String>, _>("published_at").is_some());
    assert_eq!(
        row.get::<String, _>("published_at_source_field"),
        "publishTime"
    );
    assert_eq!(
        row.get::<String, _>("published_at_source_kind"),
        "platform_epoch"
    );
    assert_eq!(row.get::<String, _>("published_at_precision"), "second");
    assert_eq!(
        row.get::<String, _>("published_at_parser_version"),
        "xhs-detail-time-v2"
    );
    assert_ne!(row.get::<uuid::Uuid, _>("package_ref"), uuid::Uuid::nil());
    assert_eq!(row.get::<i32, _>("record_ordinal"), 0);

    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("shared query validates");
    let page = read_work_resources(&database, &query)
        .await
        .expect("shared work resource interface reads the exact timestamp");
    assert_eq!(page.items[0].display.published_at_state, "KNOWN");
    assert_eq!(
        page.items[0].display.published_at_source_field.as_deref(),
        Some("publishTime")
    );

    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-material-2"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"attacker-selected-other-note"},
            "payload":{
                "title":"不得跨目标接纳",
                "creatorName":"不得猜成 authorName"
            }
        }),
    )
    .await;
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .expect("detail count reads");
    assert_eq!(
        detail_count, 1,
        "mismatched source identity is not projected"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn content_author_attribution_is_retained_after_observation_target_deletion() {
    let database = proof_database("content_author_target_deletion").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"retained-author-work"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"retained-author-work"},
            "payload":{"authorId":"retained-author","title":"保留作者归属"}
        }),
    )
    .await;

    let identity =
        TargetIdentity::creator("xhs", "retained-author").expect("creator identity is valid");
    let (target, _) = store_pending_target(
        &database,
        &identity,
        TargetSource::Manual,
        Some("保留作者"),
        None,
        None,
    )
    .await
    .expect("observation target is stored");
    let preview = read_target_deletion_preview(&database, target.target_ref)
        .await
        .expect("deletion preview is readable")
        .expect("target exists");
    assert_eq!((preview.retained_works, preview.retained_details), (1, 1));

    let before: (String, String) = sqlx::query_as(
        "SELECT author_external_id,attribution_source \
         FROM linggan_material_content_author WHERE platform='xhs' AND author_external_id='retained-author'",
    )
    .fetch_one(database.pool())
    .await
    .expect("content detail supplies author attribution");
    assert_eq!(
        before,
        ("retained-author".to_owned(), "content_detail".to_owned())
    );
    assert_eq!(
        delete_observation_target(&database, target.target_ref, "保留作者")
            .await
            .expect("deletion only clears target control state"),
        TargetDeletionOutcome::Deleted
    );
    let after: (String, String) = sqlx::query_as(
        "SELECT author_external_id,attribution_source \
         FROM linggan_material_content_author WHERE platform='xhs' AND author_external_id='retained-author'",
    )
    .fetch_one(database.pool())
    .await
    .expect("attribution remains after target deletion");
    assert_eq!(after, before);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn profile_discovery_supplies_author_attribution_only_for_its_scanned_creator() {
    let database = proof_database("content_author_profile_discovery").await;
    let target = serde_json::json!({"authorExternalId":"profile-only-author"});
    submit_custom_package(
        &database,
        "xhs",
        &["profile_discovery"],
        target.clone(),
        "profile_discovery",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("profile_discovery",1)]}),
        vec![serde_json::json!({
            "kind":"profile_discovery_card",
            "resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":"profile-only-work"},
            "payload":{"title":"主页发现的作品"}
        })],
    )
    .await;

    let attribution: (String, String) = sqlx::query_as(
        "SELECT author_external_id,attribution_source \
         FROM linggan_material_content_author \
         WHERE platform='xhs' AND content_public_ref=( \
             SELECT public_ref FROM linggan_material_content \
             WHERE platform='xhs' AND content_external_id='profile-only-work' \
         )",
    )
    .fetch_one(database.pool())
    .await
    .expect("profile discovery only attributes its own scanned creator");
    assert_eq!(
        attribution,
        (
            "profile-only-author".to_owned(),
            "profile_discovery".to_owned()
        )
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn newer_partial_detail_keeps_prior_known_fields_and_current_metrics_are_field_wise() {
    let database = proof_database("material_field_wise_current").await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-08-30T10:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let first_package = submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-field-wise-current"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-field-wise-current"},
            "payload":{
                "title":"首次已知标题","bodyText":"首次已知正文","authorId":"author-field-wise","authorName":"首次作者",
                "likes":10,"comments":20,"collects":3,"shares":1,
                "publishedAt":1713501296000_i64,"publishedAtText":"1713501296",
                "publishedAtSourceField":"publishTime","publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second","publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-08-30T11:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-field-wise-current"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-field-wise-current"},
            "payload":{"likes":12}
        }),
    )
    .await;
    sqlx::query(
        "CREATE OR REPLACE FUNCTION scope_001_now() RETURNS timestamptz LANGUAGE sql VOLATILE AS $$ SELECT timestamptz '2026-08-30T12:00:00Z' $$",
    )
    .execute(database.pool())
    .await
    .unwrap();
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-field-wise-current"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-field-wise-current"},
            "payload":{"comments":24}
        }),
    )
    .await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-field-wise-current"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-field-wise-current"},
            "payload":{
                "publishedAt":1787283600000_i64,"publishedAtText":"3小时前",
                "publishedAtSourceField":"time","publishedAtSourceKind":"visible_text",
                "publishedAtPrecision":"relative","publishedAtReferenceObservedAt":"2026-08-30T12:00:00Z",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;

    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id='note-field-wise-current'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let material = read_work_resource(&database, content_ref)
        .await
        .unwrap()
        .expect("the append-only versions project as one stable work");
    assert_eq!(material.display.title.as_deref(), Some("首次已知标题"));
    assert_eq!(
        material.display.creator_display_name.as_deref(),
        Some("首次作者")
    );
    assert!(material.display.published_at.is_some());
    assert_eq!(material.display.engagement.like_count, Some(12));
    assert_eq!(material.display.engagement.comment_count, Some(24));
    assert_eq!(
        material.display.published_at_source_text.as_deref(),
        Some("1713501296"),
        "a qualified exact Published Current must retain the source text from that same row"
    );
    assert_eq!(material.display.published_at_source_kind, "platform_epoch");
    assert_eq!(material.display.published_at_precision, "second");
    assert_eq!(
        material.display.published_at_parser_version.as_deref(),
        Some("xhs-detail-time-v2")
    );
    assert_eq!(
        material
            .inspector
            .pointer("/detailCurrent/title/source/packageRef"),
        Some(&serde_json::json!(first_package)),
        "the preserved title remains traceable to its own earlier package"
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/current/value"),
        Some(&serde_json::json!(12)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/previous/value"),
        Some(&serde_json::json!(10)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/delta"),
        Some(&serde_json::json!(2)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/detailCurrent/publishedAt/state"),
        Some(&serde_json::json!("KNOWN")),
        "a newer relative source text must not downgrade an earlier qualified platform epoch"
    );
    assert_eq!(
        material
            .inspector
            .pointer("/detailCurrent/publishedAt/source/packageRef"),
        Some(&serde_json::json!(first_package)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/detailCurrent/publishedAt/sourceText"),
        Some(&serde_json::json!("1713501296")),
        "Inspector must not combine an older exact value with newer relative source text"
    );
    let exact_material_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT material_ref FROM linggan_material_content_detail WHERE package_ref=$1 AND record_ordinal=0",
    )
    .bind(first_package)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        material
            .inspector
            .pointer("/detailCurrent/publishedAt/source/materialRef"),
        Some(&serde_json::json!(exact_material_ref)),
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn work_resource_list_detail_and_inspector_keep_each_field_on_its_typed_source() {
    let database = proof_database("material_field_source_parity").await;
    let external_id = "note-field-source-parity";
    let title_package = submit_package_at(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"title":"逐字段标题"}
        }),
        "2026-08-28T08:00:00Z",
    )
    .await;
    let body_package = submit_package_at(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"bodyText":"逐字段正文"}
        }),
        "2026-08-28T09:00:00Z",
    )
    .await;
    let creator_package = submit_package_at(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"authorId":"author-source-parity","authorName":"逐字段作者"}
        }),
        "2026-08-28T10:00:00Z",
    )
    .await;
    submit_package_at(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":external_id}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":external_id},
            "payload":{"likes":7}
        }),
        "2026-08-28T11:00:00Z",
    )
    .await;

    let source_rows = sqlx::query(
        "SELECT package_ref,material_ref,record_ordinal FROM linggan_material_content_detail \
         WHERE package_ref=ANY($1::uuid[]) ORDER BY package_ref",
    )
    .bind(vec![title_package, body_package, creator_package])
    .fetch_all(database.pool())
    .await
    .unwrap();
    let source_material = |package_ref| {
        source_rows
            .iter()
            .find(|row| row.get::<uuid::Uuid, _>("package_ref") == package_ref)
            .map(|row| row.get::<uuid::Uuid, _>("material_ref"))
            .expect("each typed field source is retained")
    };
    let title_material = source_material(title_package);
    let body_material = source_material(body_package);
    let creator_material = source_material(creator_package);

    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .unwrap();
    let list = read_work_resources(&database, &query).await.unwrap();
    let list_item = list
        .items
        .iter()
        .find(|item| item.identity.content_external_id == external_id)
        .expect("the Work appears in the shared list");
    let detail = read_work_resource(&database, list_item.identity.public_ref)
        .await
        .unwrap()
        .expect("the same Work appears in detail");

    assert_eq!(detail.display.title, list_item.display.title);
    assert_eq!(
        detail.display.creator_display_name,
        list_item.display.creator_display_name
    );
    assert_eq!(detail.display.engagement.like_count, Some(7));
    for (index, expected) in [title_material, body_material, creator_material]
        .into_iter()
        .enumerate()
    {
        let pointer = format!("/overview/fields/{index}/sourceRefs/0");
        assert_eq!(
            list_item.inspector.pointer(&pointer),
            Some(&serde_json::json!(expected)),
            "list Inspector field source {index} must follow its typed Current source"
        );
        assert_eq!(
            detail.inspector.pointer(&pointer),
            Some(&serde_json::json!(expected)),
            "detail Inspector field source {index} must match the list"
        );
    }
    for package_ref in [title_package, body_package, creator_package] {
        assert!(
            detail
                .inspector
                .pointer("/provenance/packageRefs")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|refs| refs.contains(&serde_json::json!(package_ref))),
            "aggregate provenance must include the package owning each displayed field"
        );
        assert!(
            detail
                .inspector
                .pointer("/provenance/recordRefs")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|refs| refs.contains(&serde_json::json!({
                    "packageRef":package_ref,"recordOrdinal":0
                }))),
            "aggregate provenance must include the record owning each displayed field"
        );
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn engagement_current_uses_full_history_not_the_bounded_timeline_window() {
    let database = proof_database("material_engagement_current_long_history").await;
    let content_external_id = "note-long-engagement-history";
    for likes in 1_i64..=101 {
        submit_package(
            &database,
            "content_detail",
            serde_json::json!({"contentExternalId":content_external_id}),
            serde_json::json!({
                "kind":"content_detail",
                "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id},
                "payload":{"likes":likes}
            }),
        )
        .await;
    }
    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content WHERE platform='xhs' AND content_external_id=$1",
    )
    .bind(content_external_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let material = read_work_resource(&database, content_ref)
        .await
        .unwrap()
        .expect("one stable work survives more than 100 observations");
    assert_eq!(
        material
            .inspector
            .pointer("/engagementTimeline")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(100),
        "the historical presentation remains bounded from its newest edge"
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/current/value"),
        Some(&serde_json::json!(101)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/previous/value"),
        Some(&serde_json::json!(100)),
    );
    assert_eq!(
        material
            .inspector
            .pointer("/engagementCurrent/metrics/likeCount/delta"),
        Some(&serde_json::json!(1)),
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn work_resource_uses_one_media_contract_and_selects_a_local_cover_fallback() {
    let database = proof_database("material_unified_media_resource").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-media-resource"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-resource"},
            "payload":{"title":"统一媒体读模型"}
        }),
    )
    .await;
    let observation_ref = uuid::Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-resource"}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":"xhs:note-media-resource:image:1",
            "slot":{"role":"image","ordinal":1},
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-media-resource"},
            "observation":{"externalUri":"https://media.example/body-1.jpg","candidateUris":["https://media.example/body-1.jpg"]},
            "observationRef":observation_ref
        }),
    )
    .await;
    admit_media_blob(
        &database,
        observation_ref,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/jpeg",
        4,
        "blobs/aa/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .await
    .expect("body image materializes through the existing asset chain");

    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("shared query validates");
    let page = read_work_resources(&database, &query)
        .await
        .expect("work resource projects one media contract");
    let media = &page.items[0].media;
    assert_eq!(
        media
            .pointer("/contractVersion")
            .and_then(serde_json::Value::as_str),
        Some("linggan.media-resource.v1")
    );
    assert_eq!(
        media
            .pointer("/cover/selectedBy")
            .and_then(serde_json::Value::as_str),
        Some("first_body_image")
    );
    assert_eq!(
        media
            .pointer("/cover/fallbackUsed")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert!(
        media
            .pointer("/cover/localAssetUrl")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("/api/local/media/"))
    );
    assert_eq!(
        media
            .pointer("/images/0/relationship")
            .and_then(serde_json::Value::as_str),
        Some("content.image")
    );
    assert_eq!(
        media
            .pointer("/avatar/state")
            .and_then(serde_json::Value::as_str),
        Some("NOT_OBSERVED")
    );
    let reference = page.items[0].identity.public_ref;
    assert_topic_overview_matches_detail(&database, reference).await;
    for state in ["BYTES_CLEANED", "WITHDRAWN_OR_RESTRICTED"] {
        sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,materialization_ref,state,authority_ref,reason,effective_at)VALUES($1,CASE WHEN $2='BYTES_CLEANED' THEN NULL ELSE 'xhs:note-media-resource:image:1' END,CASE WHEN $2='BYTES_CLEANED' THEN (SELECT materialization_ref FROM linggan_media_materialization WHERE blob_sha256='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa' ORDER BY verified_at DESC LIMIT 1) ELSE NULL END,$2,'synthetic-overview-proof','synthetic media gate',clock_timestamp())")
            .bind(Uuid::new_v4()).bind(state).execute(database.pool()).await.unwrap();
        assert_topic_overview_matches_detail(&database, reference).await;
        let compact = read_work_resource(&database, reference)
            .await
            .unwrap()
            .unwrap();
        assert!(compact.media["cover"]["localAssetUrl"].is_null());
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn author_avatar_uses_the_media_chain_and_projects_beside_content_media() {
    let database = proof_database("material_author_avatar_resource").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-author-avatar"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-author-avatar"},
            "payload":{"title":"头像与作品媒体同链","authorId":"author-avatar-1","authorName":"作品作者"}
        }),
    )
    .await;
    let avatar_observation_ref = uuid::Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-author-avatar"}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":"xhs:author:author-avatar-1:avatar:1",
            "slot":{"role":"avatar","ordinal":1},
            "sourceObject":{"platform":"xhs","type":"author","externalId":"author-avatar-1"},
            "contextContentExternalId":"note-author-avatar",
            "observation":{"externalUri":"https://media.example/avatar.jpg","candidateUris":["https://media.example/avatar.jpg"]},
            "observationRef":avatar_observation_ref
        }),
    )
    .await;
    admit_media_blob(
        &database,
        avatar_observation_ref,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "image/jpeg",
        4,
        "blobs/bb/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    )
    .await
    .expect("author avatar materializes through the existing asset chain");
    let avatar_processing_jobs: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_media_processing_job WHERE slot_key='xhs:author:author-avatar-1:avatar:1'",
    )
    .fetch_one(database.pool())
    .await
    .expect("avatar processing queue is readable");
    assert_eq!(
        avatar_processing_jobs, 0,
        "identity avatars do not enter OCR or ASR processing"
    );

    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("shared query validates");
    let page = read_work_resources(&database, &query)
        .await
        .expect("work resource projects the author avatar");
    let avatar = page.items[0]
        .media
        .pointer("/avatar")
        .expect("avatar is part of the unified resource");
    assert_eq!(
        avatar
            .get("relationship")
            .and_then(serde_json::Value::as_str),
        Some("author.avatar")
    );
    assert_eq!(
        avatar.get("state").and_then(serde_json::Value::as_str),
        Some("ACQUIRED")
    );
    assert!(
        avatar
            .get("localAssetUrl")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| value.starts_with("/api/local/media/"))
    );
    let relation: (String, String) = sqlx::query_as(
        "SELECT subject_kind,relationship_kind FROM linggan_media_resource_relation WHERE slot_key='xhs:author:author-avatar-1:avatar:1'",
    )
    .fetch_one(database.pool())
    .await
    .expect("one authoritative author-avatar relation exists");
    assert_eq!(relation, ("author".to_owned(), "author.avatar".to_owned()));
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn relative_detail_time_remains_source_text_only_even_with_a_derived_millis_value() {
    let database = proof_database("material_relative_time_slice").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-relative-time"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-relative-time"},
            "payload":{
                "title":"相对时间",
                "publishedAt":1787283600000_i64,
                "publishedAtText":"3小时前",
                "publishedAtSourceField":"time",
                "publishedAtSourceKind":"visible_text",
                "publishedAtPrecision":"relative",
                "publishedAtReferenceObservedAt":"2026-08-21T12:00:00Z",
                "publishedAtParserVersion":"xhs-detail-time-v2"
            }
        }),
    )
    .await;
    let query: EvidenceQuery = serde_json::from_value(serde_json::json!({
        "scope":"all_accepted_material","window":"latest_accepted_discovery","sort":"latest_discovery"
    }))
    .expect("shared query validates");
    let page = read_work_resources(&database, &query)
        .await
        .expect("shared work resource interface reads source text");
    let display = &page.items[0].display;
    assert_eq!(display.published_at, None);
    assert_eq!(display.published_at_state, "SOURCE_TEXT_ONLY");
    assert_eq!(display.published_at_source_text.as_deref(), Some("3小时前"));
    assert_eq!(display.published_at_source_kind, "visible_text");
    assert_eq!(display.published_at_precision, "relative");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn exact_time_requires_a_supported_detail_field_and_parser_version() {
    let database = proof_database("material_unqualified_exact_time").await;
    submit_package(
        &database,
        "content_detail",
        serde_json::json!({"contentExternalId":"note-unqualified-exact-time"}),
        serde_json::json!({
            "kind":"content_detail",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-unqualified-exact-time"},
            "payload":{
                "title":"未资格化时间",
                "publishedAt":1713501296000_i64,
                "publishedAtText":"1713501296",
                "publishedAtSourceField":"inventedField",
                "publishedAtSourceKind":"platform_epoch",
                "publishedAtPrecision":"second",
                "publishedAtParserVersion":"invented-parser"
            }
        }),
    )
    .await;

    let row = sqlx::query(
        "SELECT published_at::text AS published_at,published_at_source_field, \
                published_at_source_kind,published_at_precision,published_at_parser_version \
         FROM linggan_material_content_detail",
    )
    .fetch_one(database.pool())
    .await
    .expect("unqualified source remains a typed detail row");
    assert_eq!(row.get::<Option<String>, _>("published_at"), None);
    assert_eq!(
        row.get::<Option<String>, _>("published_at_source_field"),
        None
    );
    assert_eq!(row.get::<String, _>("published_at_source_kind"), "unknown");
    assert_eq!(row.get::<String, _>("published_at_precision"), "unknown");
    assert_eq!(
        row.get::<Option<String>, _>("published_at_parser_version"),
        None
    );
}

/// 列表面的互动数是**文本**，不是数字。
///
/// 既有 discovery 用例全都写 `"likes":321` 这种数字形态，恰好避开了真实形状，于是
/// 「列表面互动数全部落空」这件事一直没有测试能发现：线上 1100+ 条发现记录的点赞、
/// 评论、收藏 `*_state` 全是 `UNKNOWN`。这条用例按插件真实回传的形态断言。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn card_engagement_text_including_the_ten_thousand_unit_projects_as_counts() {
    let database = proof_database("material_card_engagement_slice").await;
    submit_package(
        &database,
        "discovery_search",
        serde_json::json!({"query":"考研自习"}),
        serde_json::json!({
            "kind":"discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-engagement-text"},
            // 插件从卡片上读到什么就回传什么：高赞是「万」，没有数字时是占位文案。
            "payload":{"title":"文本互动数","likes":"2.9万","comments":"312","collects":"1万","shares":"赞"}
        }),
    )
    .await;
    let row = sqlx::query(
        "SELECT like_count,like_count_state,comment_count,comment_count_state, \
                collect_count,collect_count_state,share_count,share_count_state \
         FROM linggan_material_discovery_finding",
    )
    .fetch_one(database.pool())
    .await
    .expect("typed discovery finding exists");
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(29_000));
    assert_eq!(row.get::<String, _>("like_count_state"), "KNOWN");
    assert_eq!(row.get::<Option<i64>, _>("comment_count"), Some(312));
    assert_eq!(row.get::<String, _>("comment_count_state"), "KNOWN");
    assert_eq!(row.get::<Option<i64>, _>("collect_count"), Some(10_000));
    assert_eq!(row.get::<String, _>("collect_count_state"), "KNOWN");
    // 「赞」是这张卡片上没有数字可读，不是没有人点赞——必须留在未知，不能变成 0。
    assert_eq!(row.get::<Option<i64>, _>("share_count"), None);
    assert_eq!(row.get::<String, _>("share_count_state"), "UNKNOWN");
}

/// The v2 task target contains only the search term; sampling is a separate instruction.
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_search_with_identity_only_target_projects_instead_of_quarantining() {
    let database = proof_database("material_keyword_sampling_slice").await;
    let task_target = serde_json::json!({"query":"考研自习"});
    submit_custom_package(
        &database,
        "xhs",
        &["discovery_search"],
        task_target,
        "discovery_search",
        "xhs",
        serde_json::json!({
            "target":{
                "basis":"current_visible_surface",
                "surface":"target_driven_surface",
                "query":"考研自习"
            },
            "layers":[coverage_layer("discovery_search",1)]
        }),
        vec![serde_json::json!({
            "kind":"discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-keyword-sampling"},
            "payload":{"title":"二战考研半夜被自习室赶出来了","likes":2704}
        })],
    )
    .await;

    let quarantined: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_record_disposition \
         WHERE reason='task_package_contract_mismatch'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        quarantined, 0,
        "采样口径是下发的执行指令，不该参与包与任务的身份比对"
    );
    let row = sqlx::query(
        "SELECT finding.title,finding.like_count,content.content_external_id \
         FROM linggan_material_discovery_finding finding \
         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref",
    )
    .fetch_one(database.pool())
    .await
    .expect("带口径的关键词搜索结果进入发现面");
    assert_eq!(
        row.get::<String, _>("content_external_id"),
        "note-keyword-sampling"
    );
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(2704));
}

/// 豁免只对口径生效：搜索词本身对不上，仍然整包隔离，不进素材库。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_search_reporting_another_term_is_still_quarantined() {
    let database = proof_database("material_keyword_identity_slice").await;
    submit_custom_package(
        &database,
        "xhs",
        &["discovery_search"],
        serde_json::json!({"query":"考研自习"}),
        "discovery_search",
        "xhs",
        serde_json::json!({
            "target":{
                "basis":"current_visible_surface",
                "surface":"target_driven_surface",
                "query":"adhd"
            },
            "layers":[coverage_layer("discovery_search",1)]
        }),
        vec![serde_json::json!({
            "kind":"discovery_card","resultPosition":1,
            "sourceObject":{"platform":"xhs","type":"content","externalId":"note-keyword-elsewhere"},
            "payload":{"title":"另一个词的结果","likes":1}
        })],
    )
    .await;

    let quarantined: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_runtime_record_disposition \
         WHERE reason='task_package_contract_mismatch'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(quarantined, 1, "搜索词对不上仍然必须整包隔离");
    let findings: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_discovery_finding")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(findings, 0, "隔离的记录不得进入发现面");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn discovery_forms_the_same_work_identity_without_claiming_detail() {
    let database = proof_database("material_discovery_slice").await;
    submit_package(&database,"discovery_search",serde_json::json!({"query":"ADHD"}),serde_json::json!({
        "kind":"discovery_card","resultPosition":1,"sourceObject":{"platform":"xhs","type":"content","externalId":"note-discovery-material"},
        "payload":{
            "title":"发现面标题","authorName":"发现面作者",
            "coverUrl":"https://sns-webpic-qc.xhscdn.com/observed-cover.webp",
            "likes":321,"comments":17,"collects":43,"shares":5
        }
    })).await;
    let row=sqlx::query("SELECT finding.title,finding.title_state,content.content_external_id, \
                                finding.cover_source_url,finding.cover_source_state, \
                                finding.like_count,finding.comment_count,finding.collect_count,finding.share_count \
                         FROM linggan_material_discovery_finding finding \
                         JOIN linggan_material_content content ON content.public_ref=finding.content_public_ref")
        .fetch_one(database.pool()).await.expect("typed discovery finding exists");
    assert_eq!(
        row.get::<String, _>("content_external_id"),
        "note-discovery-material"
    );
    assert_eq!(
        row.get::<Option<String>, _>("title").as_deref(),
        Some("发现面标题")
    );
    assert_eq!(
        row.get::<Option<String>, _>("cover_source_url").as_deref(),
        Some("https://sns-webpic-qc.xhscdn.com/observed-cover.webp")
    );
    assert_eq!(row.get::<String, _>("cover_source_state"), "KNOWN");
    assert_eq!(row.get::<Option<i64>, _>("like_count"), Some(321));
    assert_eq!(row.get::<Option<i64>, _>("comment_count"), Some(17));
    assert_eq!(row.get::<Option<i64>, _>("collect_count"), Some(43));
    assert_eq!(row.get::<Option<i64>, _>("share_count"), Some(5));
    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        detail_count, 0,
        "discovery never manufactures detail material"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn admission_quarantines_task_package_and_record_identity_attacks_without_poisoning_siblings()
{
    let database = proof_database("material_admission_attacks").await;
    let target = serde_json::json!({"contentExternalId":"note-admission-1"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        target.clone(),
        "content_detail",
        "douyin",
        serde_json::json!({"target":target,"layers":[coverage_layer("content_detail",4)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"douyin","type":"content","externalId":"note-admission-1"},
            "payload":{"title":"platform mismatch must not project"}
        })],
    )
    .await;
    let target = serde_json::json!({"contentExternalId":"note-admission-2"});
    submit_custom_package(
        &database,
        "xhs",
        &["comments"],
        target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("content_detail",1)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-admission-2"},
            "payload":{"title":"capability mismatch must not project"}
        })],
    )
    .await;
    let task_target = serde_json::json!({"contentExternalId":"note-admission-3"});
    let coverage_target = serde_json::json!({"contentExternalId":"attacker-other-target"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        task_target,
        "content_detail",
        "xhs",
        serde_json::json!({"target":coverage_target,"layers":[coverage_layer("content_detail",1)]}),
        vec![serde_json::json!({
            "kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"attacker-other-target"},
            "payload":{"title":"target mismatch must not project"}
        })],
    )
    .await;

    let healthy_target = serde_json::json!({"contentExternalId":"note-admission-healthy"});
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        healthy_target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":healthy_target,"layers":[coverage_layer("content_detail",4)]}),
        vec![
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-admission-healthy"},"payload":{"title":"healthy sibling"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"douyin","type":"content","externalId":"note-admission-healthy"},"payload":{"title":"cross platform"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"author","externalId":"note-admission-healthy"},"payload":{"title":"wrong subject type"}}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","externalId":"note-admission-healthy"},"payload":{"title":"missing subject type"}}),
        ],
    )
    .await;

    let detail_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_content_detail")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(detail_count, 1, "only the healthy record projects");
    let task_binding_quarantines: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='task_package_contract_mismatch'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(task_binding_quarantines, 3);
    let record_quarantines: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='typed_detail_identity_invalid'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(record_quarantines, 3);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn coverage_selects_the_exact_capability_and_reconciles_producer_acquired() {
    let database = proof_database("material_coverage_attack").await;
    let target = serde_json::json!({"contentExternalId":"note-coverage"});
    let mut wrong_lane = coverage_layer("comments", 99);
    wrong_lane["observed"] = serde_json::json!(99);
    let mut detail_lane = coverage_layer("content_detail", 2);
    detail_lane["observed"] = serde_json::json!(2);
    submit_custom_package(
        &database,
        "xhs",
        &["content_detail"],
        target.clone(),
        "content_detail",
        "xhs",
        serde_json::json!({"target":target,"layers":[wrong_lane,detail_lane]}),
        vec![serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":"note-coverage"},"payload":{"title":"one retained of two acquired"}})],
    ).await;
    let row = sqlx::query("SELECT observed,producer_acquired,retained FROM linggan_material_lane_observation WHERE lane='detail'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(row.get::<Option<i32>, _>("observed"), Some(2));
    assert_eq!(row.get::<Option<i32>, _>("producer_acquired"), Some(2));
    assert_eq!(row.get::<Option<i32>, _>("retained"), Some(1));
}

async fn assert_topic_overview_matches_detail(
    database: &linggan_storage_postgres::Database,
    reference: Uuid,
) {
    let domain: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)SELECT gen_random_uuid(),public_ref,$2,'primary','legacy_domain_migration',first_package_ref FROM linggan_material_content WHERE public_ref=$1 AND NOT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2)")
        .bind(reference).bind(domain).execute(database.pool()).await.unwrap();
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(database.pool())
        .await
        .unwrap();
    let full = read_work_resource(database, reference)
        .await
        .unwrap()
        .unwrap();
    let compact =
        linggan_evidence::read_work_resource_overviews(database, domain, &[reference], &cutoff)
            .await
            .unwrap()
            .pop()
            .unwrap();
    assert_eq!(
        serde_json::to_value(&compact.display).unwrap(),
        serde_json::to_value(&full.display).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&compact.preview).unwrap(),
        serde_json::to_value(&full.preview).unwrap()
    );
    assert_eq!(compact.media["cover"], full.media["cover"]);
    assert_eq!(
        compact.inspector["derivatives"],
        full.inspector["derivatives"]
    );
    assert_eq!(
        compact.inspector.pointer("/overview/fields"),
        full.inspector.pointer("/overview/fields")
    );
    assert_eq!(
        serde_json::to_value(&compact.evidence_fragment).unwrap(),
        serde_json::to_value(&full.evidence_fragment).unwrap()
    );
    assert_eq!(
        compact.summary.restriction_state,
        full.summary.restriction_state
    );
    assert_eq!(compact.media["detailState"], "NOT_LOADED");
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn batched_media_replicas_preserve_cutoff_fallback_and_global_dispositions() {
    let db = proof_database("material_batched_replicas").await;
    let domain: Uuid = "00000000-0000-4000-8000-000000000001".parse().unwrap();
    let mut refs = Vec::new();
    let mut latest = Vec::new();
    for work in 0..2 {
        let id = format!("synthetic-batch-{work}");
        submit_package(&db, "content_detail", serde_json::json!({"contentExternalId":id}),
            serde_json::json!({"kind":"content_detail","sourceObject":{"platform":"xhs","type":"content","externalId":id},"payload":{"title":"合成批次媒体"}})).await;
        for slot in 1..=2 {
            let observation = Uuid::new_v4();
            submit_package(&db, "media_slots", serde_json::json!({"contentExternalId":id}),
                serde_json::json!({"kind":"media_slot","slotKey":format!("xhs:{id}:image:{slot}"),"slot":{"role":"image","ordinal":slot},"sourceObject":{"platform":"xhs","type":"content","externalId":id},"observation":{"externalUri":"https://media.example/synthetic.jpg","candidateUris":["https://media.example/synthetic.jpg"]},"observationRef":observation})).await;
            let old = format!("{:064x}", 10 + work * 2 + slot);
            admit_media_blob(
                &db,
                observation,
                &old,
                "image/jpeg",
                4,
                &format!("blobs/synthetic/{old}"),
            )
            .await
            .unwrap();
            latest.push((observation, format!("{:064x}", 20 + work * 2 + slot)));
        }
        let reference: Uuid = sqlx::query_scalar(
            "SELECT public_ref FROM linggan_material_content WHERE content_external_id=$1",
        )
        .bind(id)
        .fetch_one(db.pool())
        .await
        .unwrap();
        refs.push(reference);
        sqlx::query("INSERT INTO linggan_material_domain_usage(usage_ref,content_public_ref,domain_ref,role,basis_kind,package_ref)SELECT gen_random_uuid(),public_ref,$2,'primary','legacy_domain_migration',first_package_ref FROM linggan_material_content WHERE public_ref=$1")
            .bind(reference).bind(domain).execute(db.pool()).await.unwrap();
    }
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    for (observation, sha) in &latest {
        admit_media_blob(
            &db,
            *observation,
            sha,
            "image/jpeg",
            4,
            &format!("blobs/synthetic/{sha}"),
        )
        .await
        .unwrap();
    }
    let frozen = linggan_evidence::read_work_resource_overviews(&db, domain, &refs, &cutoff)
        .await
        .unwrap();
    for (index, reference) in refs.iter().enumerate() {
        let item = frozen
            .iter()
            .find(|w| w.identity.public_ref == *reference)
            .unwrap();
        assert!(
            item.media["cover"]["localAssetUrl"]
                .as_str()
                .unwrap()
                .ends_with(&format!("{:064x}", 11 + index * 2))
        );
    }
    // A cleaned newer replica falls back to the older qualified bytes; the other
    // work's slot restriction applies to every generation and cannot bleed across slots.
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,materialization_ref,state,authority_ref,reason,effective_at)SELECT gen_random_uuid(),materialization_ref,'BYTES_CLEANED','synthetic-batch','synthetic cleanup',clock_timestamp() FROM linggan_media_materialization WHERE blob_sha256=$1")
        .bind(&latest[0].1).execute(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_material_media_disposition_event(event_ref,slot_key,state,authority_ref,reason,effective_at)VALUES(gen_random_uuid(),'xhs:synthetic-batch-1:image:1','WITHDRAWN_OR_RESTRICTED','synthetic-batch','synthetic restriction',clock_timestamp())")
        .execute(db.pool()).await.unwrap();
    let now: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let batch = linggan_evidence::read_work_resource_overviews(&db, domain, &refs, &now)
        .await
        .unwrap();
    for (index, reference) in refs.iter().enumerate() {
        let compact = batch
            .iter()
            .find(|w| w.identity.public_ref == *reference)
            .unwrap();
        let full = read_work_resource(&db, *reference).await.unwrap().unwrap();
        assert_eq!(compact.media["cover"], full.media["cover"]);
        let expected = if index == 0 {
            format!("{:064x}", 11)
        } else {
            latest[3].1.clone()
        };
        assert!(
            compact.media["cover"]["localAssetUrl"]
                .as_str()
                .unwrap()
                .ends_with(&expected)
        );
        if index == 0 {
            assert!(
                full.inspector["limitations"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == "NEWER_MATERIALIZATION_DISPOSED")
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn shared_ocr_state_uses_canonical_reading_gap_empty_and_task_receipts_at_the_same_cutoff() {
    use linggan_evidence::material_reading::{NewReading, Publication, publish_in};
    let db = proof_database("shared_canonical_ocr_states").await;
    let work = dossier_fixture::work(&db).await;
    dossier_fixture::current_image_reading(&db).await;
    let dossier = linggan_evidence::research_dossier::read(&db, work)
        .await
        .unwrap()
        .unwrap();
    let image = dossier
        .materials
        .iter()
        .find(|unit| unit["kind"] == "image")
        .unwrap();
    let initial: Uuid = serde_json::from_value(image["currentRevisionRef"].clone()).unwrap();
    let slot = image["slotKey"].as_str().unwrap();
    let source = image["sourceVersion"].as_str().unwrap();
    let first = read_work_resource(&db, work).await.unwrap().unwrap();
    assert_eq!(first.media["ocr"]["state"], "ACQUIRED");
    assert_eq!(
        first.media["ocr"]["resources"][0]["readingRef"],
        image["currentRevisionRef"]
    );
    assert_eq!(
        first
            .lane_summaries
            .iter()
            .find(|lane| lane.lane == "ocr")
            .unwrap()
            .state,
        "ACQUIRED"
    );
    for forbidden in ["blocks", "displayText", "ocrLayering"] {
        assert!(first.media["ocr"]["resources"][0].get(forbidden).is_none());
    }
    assert!(
        !first.media["ocr"]
            .to_string()
            .contains("用途不清的背景文字"),
        "media status must not transport another OCR full text"
    );
    assert_topic_overview_matches_detail(&db, work).await;
    let cutoff: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(db.pool())
        .await
        .unwrap();
    let mut expected = Some(initial);
    for (state, blocks, label) in [
        (
            "present",
            vec![
                serde_json::json!({"kind":"paragraph","text":"仍然可读的周围文字"}),
                serde_json::json!({"kind":"gap","text":"此处未读清"}),
            ],
            "PARTIAL",
        ),
        ("known_empty", vec![], "KNOWN_EMPTY"),
        ("present", vec![], "QUEUED"),
    ] {
        let mut tx = db.pool().begin().await.unwrap();
        let result = publish_in(
            &mut tx,
            NewReading {
                content_public_ref: work,
                unit_key: format!("image:{slot}"),
                kind: "image".into(),
                slot_key: Some(slot.into()),
                page: Some(1),
                source_version: source.into(),
                source_state: state.into(),
                derivative_ref: None,
                producer_ref: Some(Uuid::new_v4()),
                generated_by: "cloud_vision".into(),
                rule_version: format!("dossier-content.v2:state-{label}"),
                blocks,
                issues: vec![],
                source_location: serde_json::json!({"blobSha256":"a".repeat(64)}),
                publication: Publication::Automatic {
                    expected_current: expected,
                },
            },
        )
        .await
        .unwrap();
        assert!(result.current);
        tx.commit().await.unwrap();
        expected = Some(result.reading_ref);
        let current = read_work_resource(&db, work).await.unwrap().unwrap();
        assert_eq!(current.media["ocr"]["state"], label);
        assert_eq!(
            current
                .lane_summaries
                .iter()
                .find(|lane| lane.lane == "ocr")
                .unwrap()
                .state,
            label
        );
    }
    let job:Uuid=sqlx::query_scalar("SELECT job_ref FROM linggan_media_processing_job WHERE slot_key=$1 AND processor_kind='image_ocr' ORDER BY created_at DESC LIMIT 1").bind(slot).fetch_one(db.pool()).await.unwrap();
    sqlx::query("INSERT INTO linggan_media_processing_job_event(event_ref,job_ref,state,reason)VALUES($1,$2,'failed','synthetic_latest_failure')").bind(Uuid::new_v4()).bind(job).execute(db.pool()).await.unwrap();
    assert_eq!(
        read_work_resource(&db, work).await.unwrap().unwrap().media["ocr"]["state"],
        "FAILED",
        "a terminal job without readable canonical blocks cannot become acquired"
    );
    let frozen = linggan_evidence::read_work_resource_overviews(
        &db,
        dossier_fixture::DOMAIN,
        &[work],
        &cutoff,
    )
    .await
    .unwrap()
    .pop()
    .unwrap();
    assert_eq!(
        frozen
            .lane_summaries
            .iter()
            .find(|lane| lane.lane == "ocr")
            .unwrap()
            .state,
        "ACQUIRED",
        "later job/reading states do not rewrite an older asOf"
    );
    linggan_evidence::record_materialization_disposition(
        &db,
        source.parse().unwrap(),
        linggan_evidence::MaterialMediaDisposition::WithdrawnOrRestricted,
        "synthetic",
        "current source restriction",
    )
    .await
    .unwrap();
    let restricted = read_work_resource(&db, work).await.unwrap().unwrap();
    assert_eq!(restricted.media["ocr"]["state"], "WITHDRAWN_OR_RESTRICTED");
    assert!(
        restricted.media["ocr"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|resource| resource["readingRef"].is_null())
    );
}
