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

/// 关键词任务带上采样口径后，整包被判为 `task_package_contract_mismatch` 而全部隔离。
///
/// 真实形状：任务 target 有 `query` + 四项口径，插件回执的 `coverage.target` 只回显
/// `query` 与 surface 事实。既有的 discovery 集成用例全都只用 `{"query": ...}` 这种
/// 不带口径的 target，所以 2026-09-08 起线上每一次关键词采集整包隔离，没有任何一条
/// 测试变红。这条用例补上那个缺口。
#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn a_keyword_search_carrying_sampling_directives_projects_instead_of_quarantining() {
    let database = proof_database("material_keyword_sampling_slice").await;
    let task_target = serde_json::json!({
        "query":"考研自习","ranking":"most_liked",
        "topByLikes":20,"scrollRounds":3,"publishedWithinDays":7
    });
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
        serde_json::json!({"query":"考研自习","ranking":"most_liked"}),
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
