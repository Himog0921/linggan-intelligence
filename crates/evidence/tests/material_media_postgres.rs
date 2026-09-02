#[path = "support/material_fixture.rs"]
mod fixture;

use fixture::{coverage_layer, proof_database, submit_custom_package, submit_package};
use linggan_evidence::{
    ObservationTargetAvatar, admit_media_blob, list_targets, read_target_avatars,
    read_work_resource, record_media_derivative_completion, sync_target_from_author_profile,
};
use sqlx::Row;

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn media_slots_preserve_candidates_generation_unknown_order_and_concurrency() {
    let database = proof_database("material_media_red").await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-1"}),
        media_record("note-media-1", "image", 1, "primary"),
    )
    .await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-live"}),
        media_record("note-media-live", "live_photo", 1, "live-unknown"),
    )
    .await;

    let origin = sqlx::query("SELECT purpose,display_ordinal,display_order_state,source_generation FROM linggan_material_media_origin WHERE slot_key='xhs:note-media-1:image:1'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(origin.get::<String, _>("purpose"), "body_image");
    assert_eq!(origin.get::<Option<i32>, _>("display_ordinal"), None);
    assert_eq!(origin.get::<String, _>("display_order_state"), "UNKNOWN");
    assert_eq!(origin.get::<i32, _>("source_generation"), 1);
    let relationship: (String, i32) = sqlx::query_as(
        "SELECT relationship_kind,relationship_ordinal \
         FROM linggan_media_resource_relation WHERE slot_key='xhs:note-media-1:image:1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(relationship, ("content.image".into(), 1));
    let live: (String,String,String) = sqlx::query_as("SELECT composite_state,live_photo_still_state,live_photo_motion_state FROM linggan_material_media_origin WHERE purpose='live_photo'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(live, ("PARTIAL".into(), "UNKNOWN".into(), "UNKNOWN".into()));

    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-live-components"}),
        live_photo_record_with_components("note-media-live-components"),
    )
    .await;
    let observed_live: (String, String, String) = sqlx::query_as(
        "SELECT composite_state,live_photo_still_state,live_photo_motion_state \
         FROM linggan_material_media_origin \
         WHERE slot_key='xhs:note-media-live-components:live_photo:1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        observed_live,
        ("PARTIAL".into(), "OBSERVED".into(), "OBSERVED".into()),
        "observing both Live Photo candidates is not the same as acquiring both byte components",
    );

    let concurrent_a = submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-concurrent"}),
        media_record("note-media-concurrent", "image", 1, "a"),
    );
    let concurrent_b = submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-concurrent"}),
        media_record("note-media-concurrent", "image", 1, "b"),
    );
    tokio::join!(concurrent_a, concurrent_b);
    let generations: Vec<i32> = sqlx::query_scalar("SELECT source_generation FROM linggan_material_media_origin WHERE slot_key='xhs:note-media-concurrent:image:1' ORDER BY source_generation")
        .fetch_all(database.pool()).await.unwrap();
    assert_eq!(generations, vec![1, 2]);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn comment_images_use_the_shared_media_chain_and_work_resource_read_model() {
    let database = proof_database("material_comment_image_chain").await;
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-comment-image"}),
        comment_image_record("note-comment-image", "comment-image-1", 1),
    )
    .await;

    let origin: (String, String) = sqlx::query_as(
        "SELECT origin.purpose,relation.relationship_kind \
         FROM linggan_material_media_origin origin \
         JOIN linggan_media_resource_relation relation USING(slot_key) \
         WHERE origin.slot_key='xhs:comment:comment-image-1:comment_image:1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(origin, ("comment_image".into(), "comment.image".into()));

    let content_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT public_ref FROM linggan_material_content \
         WHERE content_external_id='note-comment-image'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let work = read_work_resource(&database, content_ref)
        .await
        .unwrap()
        .expect("comment image remains readable through the work-level media contract");
    assert_eq!(work.media["commentImages"]["state"], "OBSERVED");
    assert_eq!(
        work.media["commentImages"]["items"][0]["relationship"],
        "comment.image"
    );
    assert_eq!(
        work.media["commentImages"]["items"][0]["subjectExternalId"],
        "comment-image-1"
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn standalone_author_avatar_uses_media_lifecycle_without_inventing_a_work() {
    let database = proof_database("author_profile_avatar_media").await;
    let author = serde_json::json!({
        "userId":"author-profile-1",
        "name":"头像作者",
        "avatar":"https://media.example/author-profile-1.png",
        "description":"仅采集了博主页"
    });
    submit_package(
        &database,
        "author_profile",
        serde_json::json!({"authorExternalId":"author-profile-1"}),
        serde_json::json!({
            "kind":"author_profile",
            "sourceObject":{"platform":"xhs","type":"author","externalId":"author-profile-1"},
            "payload":author
        }),
    )
    .await;
    let target = sync_target_from_author_profile(
        &database,
        "author_profile",
        "xhs",
        &[serde_json::json!({"payload":author})],
    )
    .await
    .expect("accepted author profile creates observation target");
    assert!(matches!(
        target,
        linggan_evidence::TargetSyncOutcome::Created { .. }
    ));

    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"authorExternalId":"author-profile-1"}),
        author_avatar_record("author-profile-1"),
    )
    .await;

    let origin: (Option<uuid::Uuid>, String, String) = sqlx::query_as(
        "SELECT origin.content_public_ref,origin.purpose,relation.relationship_kind \
         FROM linggan_material_media_origin origin \
         JOIN linggan_media_resource_relation relation USING(slot_key) \
         WHERE origin.slot_key='xhs:author:author-profile-1:avatar:1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        origin.0, None,
        "profile-only avatar must not fabricate a content context"
    );
    assert_eq!(origin.1, "author_avatar");
    assert_eq!(origin.2, "author.avatar");
    let fabricated_work_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM linggan_material_content WHERE content_external_id='author-profile-1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(fabricated_work_count, 0);

    let targets = list_targets(&database, Some("creator"), 10).await.unwrap();
    assert_eq!(
        read_target_avatars(&database, &targets).await.unwrap()[&targets[0].target_ref],
        ObservationTargetAvatar::Pending,
    );
    let observation_ref: uuid::Uuid = sqlx::query_scalar(
        "SELECT observation_ref FROM linggan_media_observation \
         WHERE slot_key='xhs:author:author-profile-1:avatar:1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let asset = admit_media_blob(
        &database,
        observation_ref,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "image/png",
        1,
        "author-avatar/author-profile-1.png",
    )
    .await
    .expect("author avatar enters existing blob materialization lifecycle");
    assert_eq!(
        read_target_avatars(&database, &targets).await.unwrap()[&targets[0].target_ref],
        ObservationTargetAvatar::Local {
            local_asset_path: asset.local_asset_path,
        },
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn invalid_candidate_order_is_quarantined_without_creating_an_origin() {
    let database = proof_database("material_media_invalid").await;
    let mut record = media_record("note-media-invalid", "image", 1, "primary");
    record["observation"]["candidateUris"] = serde_json::json!([
        "https://media.example/different-first",
        "https://media.example/primary"
    ]);
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-media-invalid"}),
        record,
    )
    .await;
    let disposition: (String, String) =
        sqlx::query_as("SELECT disposition,reason FROM linggan_runtime_record_disposition")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        disposition,
        ("quarantined".into(), "media_origin_contract_invalid".into())
    );
    let origins: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_media_origin")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(origins, 0);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn database_identity_conflict_quarantines_only_the_bad_slot_and_duplicate_slots_never_insert()
{
    let database = proof_database("material_media_identity_conflicts").await;
    let seed_package = submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"seed-media"}),
        media_record("seed-media", "image", 1, "seed"),
    )
    .await;
    sqlx::query("INSERT INTO linggan_media_slot(slot_key,platform,content_external_id,role,ordinal,first_package_ref) VALUES('xhs:note-media-conflict:image:1','xhs','note-media-conflict','cover',1,$1)")
        .bind(seed_package).execute(database.pool()).await.unwrap();
    let target = serde_json::json!({"contentExternalId":"note-media-conflict"});
    submit_custom_package(
        &database,
        "xhs",
        &["media_slots"],
        target.clone(),
        "media_slots",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("media_slots",2)]}),
        vec![
            media_record("note-media-conflict", "image", 1, "conflict"),
            media_record("note-media-conflict", "image", 2, "healthy"),
        ],
    )
    .await;
    let dispositions: Vec<(i32,String)> = sqlx::query_as("SELECT record_ordinal,reason FROM linggan_runtime_record_disposition disposition JOIN linggan_runtime_capture_package package USING(package_ref) WHERE package.coverage #>> '{target,contentExternalId}'='note-media-conflict' ORDER BY record_ordinal")
        .fetch_all(database.pool()).await.unwrap();
    assert_eq!(dispositions[0], (0, "media_slot_identity_conflict".into()));
    assert_eq!(dispositions[1], (1, "media_origin_contract_valid".into()));
    let origins: Vec<String> = sqlx::query_scalar("SELECT slot_key FROM linggan_material_media_origin WHERE slot_key LIKE 'xhs:note-media-conflict:%'")
        .fetch_all(database.pool()).await.unwrap();
    assert_eq!(origins, vec!["xhs:note-media-conflict:image:2"]);

    let target = serde_json::json!({"contentExternalId":"note-media-duplicate"});
    let duplicate = media_record("note-media-duplicate", "image", 1, "duplicate");
    let mut duplicate_with_distinct_observation = duplicate.clone();
    duplicate_with_distinct_observation["observationRef"] = serde_json::json!(uuid::Uuid::new_v4());
    submit_custom_package(
        &database,
        "xhs",
        &["media_slots"],
        target.clone(),
        "media_slots",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("media_slots",2)]}),
        vec![duplicate, duplicate_with_distinct_observation],
    )
    .await;
    let duplicate_quarantines: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_runtime_record_disposition WHERE reason='media_slot_identity_duplicate'")
        .fetch_one(database.pool()).await.unwrap();
    assert_eq!(duplicate_quarantines, 2);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn observation_identity_conflicts_and_same_package_duplicates_quarantine_only_bad_records() {
    let database = proof_database("material_media_observation_conflicts").await;
    let historical_observation = uuid::Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-observation-conflict"}),
        media_record_with_observation(
            "note-observation-conflict",
            "image",
            1,
            "historical",
            historical_observation,
        ),
    )
    .await;
    let target = serde_json::json!({"contentExternalId":"note-observation-conflict"});
    submit_custom_package(
        &database,
        "xhs",
        &["media_slots"],
        target.clone(),
        "media_slots",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("media_slots",2)]}),
        vec![
            media_record_with_observation(
                "note-observation-conflict",
                "image",
                2,
                "conflict",
                historical_observation,
            ),
            media_record("note-observation-conflict", "image", 3, "healthy"),
        ],
    )
    .await;
    let reasons: Vec<String> = sqlx::query_scalar(
        "SELECT reason FROM linggan_runtime_record_disposition disposition \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE package.coverage #>> '{target,contentExternalId}'='note-observation-conflict' \
         ORDER BY package.accepted_at,record_ordinal",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert!(reasons.contains(&"media_observation_identity_conflict".to_owned()));
    assert_eq!(
        reasons.last().map(String::as_str),
        Some("media_origin_contract_valid")
    );

    let duplicate_observation = uuid::Uuid::new_v4();
    let target = serde_json::json!({"contentExternalId":"note-observation-duplicate"});
    submit_custom_package(
        &database,
        "xhs",
        &["media_slots"],
        target.clone(),
        "media_slots",
        "xhs",
        serde_json::json!({"target":target,"layers":[coverage_layer("media_slots",3)]}),
        vec![
            media_record_with_observation(
                "note-observation-duplicate",
                "image",
                1,
                "duplicate-a",
                duplicate_observation,
            ),
            media_record_with_observation(
                "note-observation-duplicate",
                "image",
                2,
                "duplicate-b",
                duplicate_observation,
            ),
            media_record("note-observation-duplicate", "image", 3, "healthy"),
        ],
    )
    .await;
    let duplicate_reasons: Vec<String> = sqlx::query_scalar(
        "SELECT reason FROM linggan_runtime_record_disposition disposition \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         WHERE package.coverage #>> '{target,contentExternalId}'='note-observation-duplicate' \
         ORDER BY record_ordinal",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        duplicate_reasons,
        vec![
            "media_observation_identity_duplicate",
            "media_observation_identity_duplicate",
            "media_origin_contract_valid",
        ]
    );
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn media_owner_seams_reject_unsafe_storage_key_components_without_writing_facts() {
    let database = proof_database("material_media_storage_keys").await;
    let observation_ref = uuid::Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-storage-key"}),
        media_record_with_observation(
            "note-storage-key",
            "image",
            1,
            "storage-key",
            observation_ref,
        ),
    )
    .await;
    for invalid in ["", "/absolute", "a/../b", "a/./b", "a//b"] {
        assert!(
            admit_media_blob(
                &database,
                observation_ref,
                "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
                "image/jpeg",
                12,
                invalid,
            )
            .await
            .is_err(),
            "unsafe storage key must fail: {invalid:?}"
        );
    }
    let admission = admit_media_blob(
        &database,
        observation_ref,
        "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
        "image/jpeg",
        12,
        "blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
    )
    .await
    .unwrap();
    for invalid in ["", "/absolute", "a/../b", "a/./b", "a//b"] {
        assert!(
            record_media_derivative_completion(
                &database,
                admission.processing_jobs[1],
                "ocr_text",
                "1320b046a60f7c39a3480dea50b655ca92ce61db269ea07e4037e7a6f0788e5a",
                12,
                Some(invalid),
            )
            .await
            .is_err(),
            "unsafe derivative storage key must fail: {invalid:?}"
        );
    }
    let derivatives: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_media_derivative")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(derivatives, 0);
}

#[tokio::test]
#[ignore = "requires the isolated PostgreSQL 16 proof harness"]
async fn media_owner_seams_preserve_unknown_mime_but_reject_unbounded_asset_sizes() {
    let database = proof_database("material_media_asset_contract").await;
    let observation_ref = uuid::Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"note-asset-contract"}),
        media_record_with_observation(
            "note-asset-contract",
            "image",
            1,
            "asset-contract",
            observation_ref,
        ),
    )
    .await;
    for (mime_type, byte_size) in [("image/jpeg", 0_i64), ("image/jpeg", 268_435_457_i64)] {
        assert!(
            admit_media_blob(
                &database,
                observation_ref,
                "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
                mime_type,
                byte_size,
                "blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
            )
            .await
            .is_err(),
            "unsafe asset contract must fail: mime={mime_type:?}, byte_size={byte_size}"
        );
    }
    admit_media_blob(
        &database,
        observation_ref,
        "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
        "application/x-new-platform-format",
        12,
        "blobs/8a/8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
    )
    .await
    .expect("unknown declared MIME is retained; delivery decides whether inline is safe");
    let blobs: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_media_blob")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(blobs, 1);
}

fn author_avatar_record(author_id: &str) -> serde_json::Value {
    serde_json::json!({
        "kind":"media_slot",
        "slotKey":format!("xhs:author:{author_id}:avatar:1"),
        "observationRef":uuid::Uuid::new_v4(),
        "slot":{"role":"avatar","ordinal":1},
        "observation":{
            "externalUri":"https://media.example/author-profile-1.png",
            "candidateUris":["https://media.example/author-profile-1.png"],
            "observedAt":"2026-08-28T10:00:00Z"
        },
        "sourceObject":{"platform":"xhs","type":"author","externalId":author_id}
    })
}

fn media_record(content_id: &str, role: &str, ordinal: i32, uri_suffix: &str) -> serde_json::Value {
    media_record_with_observation(content_id, role, ordinal, uri_suffix, uuid::Uuid::new_v4())
}

fn comment_image_record(content_id: &str, comment_id: &str, ordinal: i32) -> serde_json::Value {
    let uri = format!("https://media.example/{comment_id}-{ordinal}.webp");
    serde_json::json!({
        "kind":"media_slot",
        "slotKey":format!("xhs:comment:{comment_id}:comment_image:{ordinal}"),
        "observationRef":uuid::Uuid::new_v4(),
        "slot":{"role":"comment_image","ordinal":ordinal},
        "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-09-01T08:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"comment","externalId":comment_id},
        "contextContentExternalId":content_id
    })
}

fn live_photo_record_with_components(content_id: &str) -> serde_json::Value {
    let still = "https://media.example/live-still.webp";
    let motion = "https://media.example/live-motion.mp4";
    serde_json::json!({
        "kind":"media_slot",
        "slotKey":format!("xhs:{content_id}:live_photo:1"),
        "observationRef":uuid::Uuid::new_v4(),
        "slot":{"role":"live_photo","ordinal":1},
        "observation":{
            "externalUri":motion,
            "candidateUris":[motion,still],
            "components":{
                "still":{"candidateUris":[still]},
                "motion":{"candidateUris":[motion]}
            },
            "observedAt":"2026-08-28T10:00:00Z"
        },
        "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
    })
}

fn media_record_with_observation(
    content_id: &str,
    role: &str,
    ordinal: i32,
    uri_suffix: &str,
    observation_ref: uuid::Uuid,
) -> serde_json::Value {
    let uri = format!("https://media.example/{uri_suffix}");
    serde_json::json!({
        "kind":"media_slot","slotKey":format!("xhs:{content_id}:{role}:{ordinal}"),
        "observationRef":observation_ref,"slot":{"role":role,"ordinal":ordinal},
        "observation":{"externalUri":uri,"candidateUris":[uri],"observedAt":"2026-08-28T10:00:00Z"},
        "sourceObject":{"platform":"xhs","type":"content","externalId":content_id}
    })
}
