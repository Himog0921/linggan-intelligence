#[path = "../../evidence/tests/support/material_fixture.rs"]
mod fixture;
#[path = "support/comment_research_fixture.rs"]
mod research_fixture;

use fixture::{proof_database, submit_package};
use linggan_evidence::{
    MaterialMediaDisposition, MediaProcessingClaimOutcome, OcrCompletionInput, OcrLayeringInput,
    OcrLineInput, admit_media_blob, claim_media_processing_work, complete_media_processing_ocr,
    ensure_media_processing_work, record_derivative_disposition,
};
use linggan_intelligence::comment_study_source::{
    StudySourceError, eligible_sources, preview_sources,
};
use linggan_intelligence::{
    comment_study_acceptance::accept_target_output,
    comment_study_batch::{
        MAX_TARGETS_PER_BATCH, PrepareStudyBatchRequest, next_run_needing_batch,
        prepare_study_batch,
    },
    comment_study_batch_acceptance::{
        BatchAcceptanceError, accept_study_batch_output, reject_study_batch_dispatch,
    },
    comment_study_batch_worker::{
        DEFAULT_BATCH_LEASE_SECONDS, claim_next_study_batch, recover_expired_study_batch_leases,
    },
    comment_study_candidate_recall::{
        advance_next_problem_pair, advance_next_problem_resolution, recall_problem_candidates,
    },
    comment_study_comparison_cache::{
        record_resolution_comparisons, serve_pending_resolutions_from_cache,
    },
    comment_study_embedding::{
        EmbeddingOutcome, ProbeOutcome, active_profile, embed_pending_signals,
        probe_and_register_embedding_profile,
    },
    comment_study_model_dispatch::reserve_study_batch_model_call,
    comment_study_model_runner::{StudyModelRunnerError, call_study_batch_model},
    comment_study_pair_worker::run_one_problem_pair,
    comment_study_problem_store::{
        PairSelection, accept_problem_pair, accept_problem_resolution, prepare_problem_pair,
        prepare_problem_resolution,
    },
    comment_study_read::{
        CommentStudyReadQuery, read_overview, read_problems, read_runs, read_signals, read_targets,
    },
    comment_study_recall::{RecallCompleteness, problem_representatives, recall_candidates},
    comment_study_resolution_worker::run_one_problem_resolution,
    comment_study_run::{PrepareStudyRunRequest, prepare_study_run},
    model_runner::prepare_next_batch_across_runs,
    model_runner::run_model_work_once,
    model_secrets::{ModelSecretStore, SyntheticModelSecrets},
    model_settings::ModelError,
    pi_adapter::PiAdapter,
};
use research_fixture::{comment_with_author, detail_with_author, reply_with_author};
use sqlx::Row;
use uuid::Uuid;

const RESET_SQL: &str = include_str!("../../../database/bootstrap/comment-study-reset.sql");
const STUDY_SCHEMA_SQL: &str = include_str!("../../../database/bootstrap/comment-study-001.sql");
const PROOF_DOMAIN_REF: Uuid = Uuid::from_u128(0x0000_0000_0000_4000_8000_0000_0000_0001);

fn domain_read_query(run_ref: Option<Uuid>) -> CommentStudyReadQuery {
    CommentStudyReadQuery {
        domain: Some(PROOF_DOMAIN_REF),
        run_ref,
        ..Default::default()
    }
}

struct UnavailableModelSecrets;

impl ModelSecretStore for UnavailableModelSecrets {
    fn put(&self, _: Uuid, _: Uuid, _: &str) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }

    fn get(&self, _: Uuid, _: Uuid) -> Result<String, ModelError> {
        Err(ModelError::SecretUnavailable)
    }

    fn delete(&self, _: Uuid, _: Uuid) -> Result<(), ModelError> {
        Err(ModelError::SecretUnavailable)
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn source_gate_selects_only_adhd_current_readable_and_unrestricted_comments() {
    let database = proof_database("comment_study_source_gate").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-source-note",
        "ADHD 家庭作业上下文",
        Some("creator-1"),
    )
    .await;
    let comment_ref = comment_with_author(
        &database,
        "study-source-note",
        "study-source-comment",
        "孩子每天写作业都要催，不催就不开始。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let domain_ref = PROOF_DOMAIN_REF;
    let as_of = "2099-01-01T00:00:00Z";

    let selected = eligible_sources(&database, domain_ref, as_of, 10)
        .await
        .unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].source_ref, comment_ref);
    assert_eq!(selected[0].clean_state, "direct");
    assert!(selected[0].research_text.contains("每天写作业"));
    assert_eq!(
        selected[0].context_manifest["contract"],
        "comment-study.context.v1"
    );
    assert!(
        selected[0].context_manifest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["kind"] == "native_title")
    );

    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'synthetic restriction proof' \
           FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(comment_ref)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        eligible_sources(&database, domain_ref, as_of, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        eligible_sources(&database, Uuid::new_v4(), as_of, 10).await,
        Err(StudySourceError::InvalidDomain)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn source_gate_excludes_content_author_voice_and_unknown_roles() {
    let database = proof_database("comment_study_source_author_role").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-author-role-note",
        "ADHD 家庭作业上下文",
        Some("creator-1"),
    )
    .await;
    let reader_comment = comment_with_author(
        &database,
        "study-author-role-note",
        "study-author-role-reader",
        "孩子每天写作业都要催，不催就不开始。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-author-role-note",
        "study-author-role-creator-root",
        "我是作者，这条是我的补充说明。",
        Some("creator-1"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    reply_with_author(
        &database,
        "study-author-role-note",
        "study-author-role-creator-reply",
        "study-author-role-reader",
        "谢谢你的留言，我补充一下。",
        Some("creator-1"),
        "2026-09-16T08:02:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-author-role-note",
        "study-author-role-unknown-commenter",
        "没有稳定作者身份，不能断言是用户声音。",
        None,
        "2026-09-16T08:03:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-author-role-note",
        "study-author-role-blank-commenter",
        "空白作者身份也不能断言为用户声音。",
        Some(" "),
        "2026-09-16T08:03:30Z",
    )
    .await;
    detail_with_author(
        &database,
        "study-author-role-unknown-work",
        "作者身份未知的 ADHD 笔记",
        None,
    )
    .await;
    comment_with_author(
        &database,
        "study-author-role-unknown-work",
        "study-author-role-unknown-work-comment",
        "作品作者未知，不能断言这条是用户声音。",
        Some("reader-2"),
        "2026-09-16T08:04:00Z",
    )
    .await;

    let selected = eligible_sources(&database, PROOF_DOMAIN_REF, "2099-01-01T00:00:00Z", 10)
        .await
        .unwrap();

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].source_ref, reader_comment);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn setup_preview_counts_the_same_source_gate_that_freezes_targets() {
    let database = proof_database("comment_study_source_preview").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-preview-note",
        "ADHD 研究对象资格",
        Some("creator-1"),
    )
    .await;
    let reader = comment_with_author(
        &database,
        "study-preview-note",
        "study-preview-reader",
        "孩子每天写作业都要催，不催就不开始。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-preview-note",
        "study-preview-creator",
        "我是作品作者，这是一条补充。",
        Some("creator-1"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-preview-note",
        "study-preview-unknown",
        "不知道作者身份的评论。",
        None,
        "2026-09-16T08:02:00Z",
    )
    .await;
    let restricted = comment_with_author(
        &database,
        "study-preview-note",
        "study-preview-restricted",
        "来源已经受限，不能进入研究。",
        Some("reader-2"),
        "2026-09-16T08:03:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-preview-note",
        "study-preview-emoji",
        "😭😭😭",
        Some("reader-3"),
        "2026-09-16T08:04:00Z",
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'synthetic restriction proof' \
           FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(restricted)
    .execute(database.pool())
    .await
    .unwrap();

    let domain_ref = PROOF_DOMAIN_REF;
    let as_of = "2099-01-01T00:00:00Z";
    let preview = preview_sources(&database, domain_ref, as_of).await.unwrap();
    let frozen = eligible_sources(&database, domain_ref, as_of, 3000)
        .await
        .unwrap();

    assert_eq!(preview.total_comment_count, 5);
    assert_eq!(preview.eligible_comment_count, frozen.len());
    assert_eq!(frozen.len(), 1);
    assert_eq!(frozen[0].source_ref, reader);
    assert_eq!(preview.excluded_counts.creator_voice, 1);
    assert_eq!(preview.excluded_counts.comment_author_unknown, 1);
    assert_eq!(preview.excluded_counts.source_restricted, 1);
    assert_eq!(preview.excluded_counts.text_not_researchable, 1);
    assert_eq!(preview.works.len(), 1);
    assert_eq!(preview.works[0].eligible_comment_count, 1);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn source_gate_excludes_withdrawn_ocr_but_keeps_the_comment_target() {
    let database = proof_database("comment_study_withdrawn_ocr").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-ocr-note",
        "ADHD 作品上下文",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "study-ocr-note",
        "study-ocr-comment",
        "孩子一写作业就拖延，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let observation_ref = Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"study-ocr-note"}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":"xhs:study-ocr-note:image:1",
            "observationRef":observation_ref,
            "slot":{"role":"image","ordinal":1},
            "observation":{
                "externalUri":"https://media.example/study-ocr-note.jpg",
                "candidateUris":["https://media.example/study-ocr-note.jpg"],
                "observedAt":"2026-09-16T08:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":"study-ocr-note"}
        }),
    )
    .await;
    let admission = admit_media_blob(
        &database,
        observation_ref,
        "8a126be6897fab75359a5d57f5889376aac0fadec42a4c4be9dcf1080cccdd62",
        "image/jpeg",
        12,
        "blobs/8a/study-ocr-note.jpg",
    )
    .await
    .unwrap();
    ensure_media_processing_work(&database).await.unwrap();
    let worker_ref = Uuid::new_v4();
    let claim = match claim_media_processing_work(&database, worker_ref, &["image_ocr".to_owned()])
        .await
        .unwrap()
    {
        MediaProcessingClaimOutcome::Claimed(claim) => claim,
        other => panic!("the freshly admitted OCR job is claimable: {other:?}"),
    };
    assert_eq!(claim.processor_kind, "image_ocr");
    assert!(admission.processing_jobs.contains(&claim.job_ref));
    let derivative_ref = complete_media_processing_ocr(
        &database,
        &claim,
        worker_ref,
        &OcrCompletionInput {
            engine_version: "paddle-test".to_owned(),
            image_width: 100,
            image_height: 100,
            raw_text: "原始 OCR 不能直接进入研究语境".to_owned(),
            raw_content_hash: "1320b046a60f7c39a3480dea50b655ca92ce61db269ea07e4037e7a6f0788e5a"
                .to_owned(),
            raw_storage_key: "derived/study-ocr-note.txt".to_owned(),
            layout_content_hash: "2320b046a60f7c39a3480dea50b655ca92ce61db269ea07e4037e7a6f0788e5a"
                .to_owned(),
            layout_byte_size: 12,
            layout_storage_key: "derived/study-ocr-note-layout.json".to_owned(),
            lines: vec![OcrLineInput {
                text: "图片中的作业计划".to_owned(),
                confidence: 0.99,
                bbox_norm: [0.0, 0.0, 1.0, 1.0],
            }],
            layering: OcrLayeringInput {
                state: "ACCEPTED".to_owned(),
                cover_headline: None,
                image_substantive_text: Some("图片中的作业计划".to_owned()),
                retained_ordinals: vec![0],
                headline_ordinals: vec![],
                excluded_lines: vec![],
            },
        },
    )
    .await
    .unwrap();
    let domain_ref = PROOF_DOMAIN_REF;
    let as_of = "2099-01-01T00:00:00Z";
    let before_withdrawal = eligible_sources(&database, domain_ref, as_of, 10)
        .await
        .unwrap();
    assert!(
        before_withdrawal[0].context_manifest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["kind"] == "image_substantive_text")
    );

    record_derivative_disposition(
        &database,
        derivative_ref,
        MaterialMediaDisposition::WithdrawnOrRestricted,
        "isolated-proof",
        "synthetic withdrawn OCR",
    )
    .await
    .unwrap();
    let after_withdrawal = eligible_sources(&database, domain_ref, as_of, 10)
        .await
        .unwrap();
    assert_eq!(after_withdrawal.len(), 1);
    assert!(
        !after_withdrawal[0].context_manifest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|source| source["kind"] == "image_substantive_text")
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn source_context_admits_only_the_latest_accepted_nonretired_ocr_semantic_text() {
    let database = proof_database("comment_study_ocr_context_qualification").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-ocr-qualification-note",
        "ADHD 作品上下文",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "study-ocr-qualification-note",
        "study-ocr-qualification-comment",
        "孩子一写作业就拖延，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let (accepted_derivative, _) = complete_study_context_ocr(
        &database,
        "study-ocr-qualification-note",
        1,
        "ACCEPTED",
        "可以进入研究语境的图片实质文本",
    )
    .await;
    let (superseded_derivative, _) = complete_study_context_ocr(
        &database,
        "study-ocr-qualification-note",
        2,
        "ACCEPTED",
        "旧的 accepted 文本不能越过最新 partial",
    )
    .await;
    let superseded_layout: Uuid = sqlx::query_scalar(
        "SELECT layout_ref FROM linggan_media_ocr_layout WHERE ocr_derivative_ref=$1",
    )
    .bind(superseded_derivative)
    .fetch_one(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_media_ocr_layering_result( \
           layering_ref,layout_ref,layer_version,state,decision_source,image_substantive_text, \
           retained_line_refs,excluded_lines,created_at \
         ) VALUES($1,$2,'rules-v2','PARTIAL','rules',$3,'[]'::jsonb,'[]'::jsonb, \
                  scope_001_now()+interval '1 second')",
    )
    .bind(Uuid::new_v4())
    .bind(superseded_layout)
    .bind("最新 partial 不能进入研究语境")
    .execute(database.pool())
    .await
    .unwrap();
    let (retired_derivative, retired_job) = complete_study_context_ocr(
        &database,
        "study-ocr-qualification-note",
        3,
        "ACCEPTED",
        "已退役 OCR 不能进入研究语境",
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_media_ocr_retirement(retired_job_ref,reason) \
         VALUES($1,'tesseract_replaced_by_paddleocr')",
    )
    .bind(retired_job)
    .execute(database.pool())
    .await
    .unwrap();

    let sources = eligible_sources(&database, PROOF_DOMAIN_REF, "2099-01-01T00:00:00Z", 10)
        .await
        .unwrap();
    let ocr_fragments: Vec<_> = sources[0].context_manifest["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|fragment| fragment["kind"] == "image_substantive_text")
        .collect();
    assert_eq!(ocr_fragments.len(), 1, "{ocr_fragments:?}");
    assert_eq!(
        ocr_fragments[0]["sourceRef"],
        serde_json::json!(accepted_derivative)
    );
    assert_eq!(ocr_fragments[0]["text"], "可以进入研究语境的图片实质文本");
    assert!(
        sources[0].context_manifest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|fragment| fragment["kind"] != "ocr_text"),
        "the raw OCR derivative must never enter the context manifest"
    );
    let manifest = sources[0].context_manifest.to_string();
    assert!(
        !manifest.contains("原始 OCR 1"),
        "the raw derivative itself is never a context fragment: {manifest}"
    );
    assert!(
        !manifest.contains("最新 partial 不能进入研究语境"),
        "the latest PARTIAL layer must suppress its older accepted layer: {manifest}"
    );
    assert!(
        !manifest.contains("已退役 OCR 不能进入研究语境"),
        "retired OCR must remain unavailable even if its layer was accepted: {manifest}"
    );
    assert_ne!(accepted_derivative, retired_derivative);
}

async fn complete_study_context_ocr(
    database: &linggan_storage_postgres::Database,
    content_external_id: &str,
    ordinal: u8,
    layering_state: &str,
    image_substantive_text: &str,
) -> (Uuid, Uuid) {
    let observation_ref = Uuid::new_v4();
    let slot_key = format!("xhs:{content_external_id}:image:{ordinal}");
    submit_package(
        database,
        "media_slots",
        serde_json::json!({"contentExternalId":content_external_id}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":observation_ref,
            "slot":{"role":"image","ordinal":ordinal},
            "observation":{
                "externalUri":format!("https://media.example/{content_external_id}-{ordinal}.jpg"),
                "candidateUris":[format!("https://media.example/{content_external_id}-{ordinal}.jpg")],
                "observedAt":"2026-09-16T08:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":content_external_id}
        }),
    )
    .await;
    let blob_hash = format!("{ordinal:x}").repeat(64);
    admit_media_blob(
        database,
        observation_ref,
        &blob_hash,
        "image/jpeg",
        12,
        &format!("blobs/study-ocr-{ordinal}.jpg"),
    )
    .await
    .unwrap();
    ensure_media_processing_work(database).await.unwrap();
    let worker_ref = Uuid::new_v4();
    let claim = match claim_media_processing_work(database, worker_ref, &["image_ocr".to_owned()])
        .await
        .unwrap()
    {
        MediaProcessingClaimOutcome::Claimed(claim) => claim,
        other => panic!("the synthetic OCR job is claimable: {other:?}"),
    };
    let derivative_ref = complete_media_processing_ocr(
        database,
        &claim,
        worker_ref,
        &OcrCompletionInput {
            engine_version: "paddle-test".to_owned(),
            image_width: 100,
            image_height: 100,
            raw_text: format!("原始 OCR {ordinal}"),
            raw_content_hash: format!("{:x}", ordinal + 3).repeat(64),
            raw_storage_key: format!("derived/study-ocr-{ordinal}.txt"),
            layout_content_hash: format!("{:x}", ordinal + 6).repeat(64),
            layout_byte_size: 12,
            layout_storage_key: format!("derived/study-ocr-{ordinal}-layout.json"),
            lines: vec![OcrLineInput {
                text: image_substantive_text.to_owned(),
                confidence: 0.99,
                bbox_norm: [0.0, 0.0, 1.0, 1.0],
            }],
            layering: OcrLayeringInput {
                state: layering_state.to_owned(),
                cover_headline: None,
                image_substantive_text: Some(image_substantive_text.to_owned()),
                retained_ordinals: vec![0],
                headline_ordinals: vec![],
                excluded_lines: vec![],
            },
        },
    )
    .await
    .unwrap();
    (derivative_ref, claim.job_ref)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_reobserved_media_slot_contributes_one_context_fragment_per_derived_text() {
    let database = proof_database("comment_study_reobserved_slot").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-reobserved-note",
        "ADHD 作品上下文",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "study-reobserved-note",
        "study-reobserved-comment",
        "孩子一写作业就拖延，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let slot_key = "xhs:study-reobserved-note:image:1";
    let first_observation_ref = Uuid::new_v4();
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"study-reobserved-note"}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":first_observation_ref,
            "slot":{"role":"image","ordinal":1},
            "observation":{
                "externalUri":"https://media.example/study-reobserved-note.jpg",
                "candidateUris":["https://media.example/study-reobserved-note.jpg"],
                "observedAt":"2026-09-16T08:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":"study-reobserved-note"}
        }),
    )
    .await;
    admit_media_blob(
        &database,
        first_observation_ref,
        "6a45d0f1e3c7b2984f5a0d6c8e1b3a7952d4c6e8f0a2b4c6d8e0f2a4b6c8d0e2",
        "image/jpeg",
        12,
        "blobs/6a/study-reobserved-note.jpg",
    )
    .await
    .unwrap();
    ensure_media_processing_work(&database).await.unwrap();
    let worker_ref = Uuid::new_v4();
    let claim = match claim_media_processing_work(&database, worker_ref, &["image_ocr".to_owned()])
        .await
        .unwrap()
    {
        MediaProcessingClaimOutcome::Claimed(claim) => claim,
        other => panic!("the freshly admitted OCR job is claimable: {other:?}"),
    };
    complete_media_processing_ocr(
        &database,
        &claim,
        worker_ref,
        &OcrCompletionInput {
            engine_version: "paddle-test".to_owned(),
            image_width: 100,
            image_height: 100,
            raw_text: "原始 OCR 不作为评论研究语境".to_owned(),
            raw_content_hash: "2c8e0a4f6b1d3e5a7c9f0b2d4e6a8c0f1b3d5e7a9c1f3b5d7e9a1c3f5b7d9e1a"
                .to_owned(),
            raw_storage_key: "derived/study-reobserved-note.txt".to_owned(),
            layout_content_hash: "3c8e0a4f6b1d3e5a7c9f0b2d4e6a8c0f1b3d5e7a9c1f3b5d7e9a1c3f5b7d9e1a"
                .to_owned(),
            layout_byte_size: 12,
            layout_storage_key: "derived/study-reobserved-note-layout.json".to_owned(),
            lines: vec![OcrLineInput {
                text: "图片中的作业计划".to_owned(),
                confidence: 0.99,
                bbox_norm: [0.0, 0.0, 1.0, 1.0],
            }],
            layering: OcrLayeringInput {
                state: "ACCEPTED".to_owned(),
                cover_headline: None,
                image_substantive_text: Some("图片中的作业计划".to_owned()),
                retained_ordinals: vec![0],
                headline_ordinals: vec![],
                excluded_lines: vec![],
            },
        },
    )
    .await
    .unwrap();

    // The same slot is captured a second time, which is ordinary re-observation rather than new
    // material: the OCR text behind it is still one derived document.
    submit_package(
        &database,
        "media_slots",
        serde_json::json!({"contentExternalId":"study-reobserved-note"}),
        serde_json::json!({
            "kind":"media_slot",
            "slotKey":slot_key,
            "observationRef":Uuid::new_v4(),
            "slot":{"role":"image","ordinal":1},
            "observation":{
                "externalUri":"https://media.example/study-reobserved-note.jpg",
                "candidateUris":["https://media.example/study-reobserved-note.jpg"],
                "observedAt":"2026-09-17T08:00:00Z"
            },
            "sourceObject":{"platform":"xhs","type":"content","externalId":"study-reobserved-note"}
        }),
    )
    .await;
    let generations: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_material_media_origin WHERE slot_key=$1")
            .bind(slot_key)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        generations, 2,
        "the fixture has to produce a genuinely re-observed slot for this proof to mean anything"
    );

    let domain_ref = PROOF_DOMAIN_REF;
    let sources = eligible_sources(&database, domain_ref, "2099-01-01T00:00:00Z", 10)
        .await
        .unwrap();
    assert_eq!(sources.len(), 1);
    let ocr_fragments: Vec<_> = sources[0].context_manifest["sources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|source| source["kind"] == "image_substantive_text")
        .collect();
    assert_eq!(
        ocr_fragments.len(),
        1,
        "a slot observed twice still holds one OCR text, so the context must not carry it twice: {ocr_fragments:?}"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn semantic_acceptance_is_atomic_and_keeps_deferred_signals_visible() {
    let database = proof_database("comment_study_semantic_acceptance").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-semantic-note",
        "ADHD 家庭作业上下文",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-semantic-note",
        "study-semantic-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    let target_ref =
        seed_running_target(&database, policy_ref, content_public_ref, source_ref).await;
    let accepted = accept_target_output(
        &database,
        target_ref,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        None,
        semantic_output("每天写作业都要催,不催就不开始"),
    )
    .await
    .unwrap();
    assert_eq!(accepted.state, "accepted");
    assert_eq!(accepted.signal_count, 1);
    let signal = sqlx::query(
        "SELECT evidence,evidence_start,evidence_end,eligibility_state \
         FROM linggan_comment_study_signal WHERE target_ref=$1",
    )
    .bind(target_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        signal.get::<String, _>("evidence"),
        "每天写作业都要催，不催就不开始"
    );
    assert_eq!(signal.get::<String, _>("eligibility_state"), "eligible");
    let stored_evidence: String = signal.get("evidence");
    let evidence_start: i32 = signal.get("evidence_start");
    let evidence_end: i32 = signal.get("evidence_end");
    let source_body: String =
        sqlx::query_scalar("SELECT body_text FROM linggan_material_comment WHERE material_ref=$1")
            .bind(source_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        source_body
            .chars()
            .skip(evidence_start as usize)
            .take((evidence_end - evidence_start) as usize)
            .collect::<String>(),
        stored_evidence,
        "accepted evidence must remain a literal span of the immutable comment source"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "succeeded"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn semantic_contract_rejection_leaves_no_partial_signal() {
    let database = proof_database("comment_study_semantic_rejection").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-reject-note",
        "ADHD 上下文",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-reject-note",
        "study-reject-comment",
        "孩子拖延，孩子拖延，要努力。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let target_ref = seed_running_target(
        &database,
        seed_study_policy(&database).await,
        content_public_ref,
        source_ref,
    )
    .await;
    let rejected = accept_target_output(
        &database,
        target_ref,
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        None,
        serde_json::json!({"contract":"comment-study.semantic.v1","signals":[
            {"kind":"emotion","proposition":"焦虑。","evidence":"孩子拖延","problemFrame":null},
            {"kind":"belief","proposition":"需要努力。","evidence":"努力","problemFrame":null}
        ]}),
    )
    .await
    .unwrap();
    assert_eq!(rejected.state, "rejected");
    assert_eq!(rejected.signal_count, 0);
    assert_eq!(
        rejected.rejection_code.as_deref(),
        Some("evidence_not_contiguous")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_signal WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn accepted_empty_semantic_output_is_no_signal_not_a_successful_signal_set() {
    let database = proof_database("comment_study_no_signal").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-no-signal-note",
        "ADHD 上下文",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-no-signal-note",
        "study-no-signal-comment",
        "这个视频拍得真好。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let target_ref = seed_running_target(
        &database,
        seed_study_policy(&database).await,
        content_public_ref,
        source_ref,
    )
    .await;
    let receipt = accept_target_output(
        &database,
        target_ref,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        None,
        serde_json::json!({"contract":"comment-study.semantic.v1","signals":[]}),
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "accepted");
    assert_eq!(receipt.signal_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "no_signal"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn clean_read_projection_reports_new_lifecycle_states_without_old_result_fallback() {
    let database = proof_database("comment_study_read_projection").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-read-note",
        "ADHD 作业讨论",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-read-note",
        "study-read-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    let target_ref =
        seed_running_target(&database, policy_ref, content_public_ref, source_ref).await;
    accept_target_output(
        &database,
        target_ref,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        None,
        semantic_output("每天写作业都要催,不催就不开始"),
    )
    .await
    .unwrap();
    let query = domain_read_query(None);
    let overview = read_overview(&database, &query).await.unwrap();
    assert_eq!(overview["contract"], "comment-study.read.v1");
    assert_eq!(overview["cleanLayerState"], "configured");
    assert_eq!(
        overview["latestRun"]["targetStates"]["succeeded"], 1,
        "overview={overview}"
    );
    let runs = read_runs(&database, &query).await.unwrap();
    let run_ref = runs["runs"][0]["runRef"].as_str().unwrap().parse().unwrap();
    let run_query = domain_read_query(Some(run_ref));
    let targets = read_targets(&database, &run_query).await.unwrap();
    assert_eq!(targets["targets"][0]["state"], "succeeded");
    assert_eq!(
        targets["targets"][0]["commentText"],
        "孩子每天写作业都要催，不催就不开始，我很着急。"
    );
    assert_eq!(targets["targets"][0]["sourceState"], "known");
    let signals = read_signals(&database, &run_query).await.unwrap();
    assert_eq!(signals["signals"][0]["eligibilityState"], "eligible");
    assert!(signals["signals"][0]["resolutionRef"].is_null());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn overview_counts_a_final_json_schema_rejection_as_a_semantic_contract_failure() {
    let database = proof_database("comment_study_read_semantic_summary").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-semantic-summary-note",
        "ADHD 语义摘要",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-semantic-summary-note",
        "study-semantic-summary-comment",
        "孩子每天写作业都要催。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let target_ref = seed_running_target(
        &database,
        seed_study_policy(&database).await,
        content_public_ref,
        source_ref,
    )
    .await;
    sqlx::query("UPDATE linggan_comment_study_target SET state='failed' WHERE target_ref=$1")
        .bind(target_ref)
        .execute(database.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_semantic_attempt( \
           attempt_ref,target_ref,attempt_ordinal,request_hash,state,rejection_code,finished_at \
         ) VALUES($1,$2,1,$3,'rejected','semantic_json_schema',scope_001_now())",
    )
    .bind(Uuid::new_v4())
    .bind(target_ref)
    .bind("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
    .execute(database.pool())
    .await
    .unwrap();

    let overview = read_overview(&database, &domain_read_query(None))
        .await
        .unwrap();
    assert_eq!(
        overview["latestRun"]["semanticSummary"]["finalSemanticContractFailureCount"], 1,
        "overview={overview}"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn read_targets_hides_comment_text_once_the_source_becomes_restricted_after_freeze() {
    let database = proof_database("comment_study_read_targets_restricted").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-read-restricted-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-read-restricted-note",
        "study-read-restricted-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    let target_ref =
        seed_running_target(&database, policy_ref, content_public_ref, source_ref).await;
    accept_target_output(
        &database,
        target_ref,
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        None,
        semantic_output("每天写作业都要催,不催就不开始"),
    )
    .await
    .unwrap();
    let query = domain_read_query(None);
    let runs = read_runs(&database, &query).await.unwrap();
    let run_ref = runs["runs"][0]["runRef"].as_str().unwrap().parse().unwrap();
    let run_query = domain_read_query(Some(run_ref));
    let before = read_targets(&database, &run_query).await.unwrap();
    assert_eq!(
        before["targets"][0]["commentText"],
        "孩子每天写作业都要催，不催就不开始，我很着急。"
    );
    assert_eq!(before["targets"][0]["sourceState"], "known");

    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'restricted after read-target proof' \
           FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let after = read_targets(&database, &run_query).await.unwrap();
    assert!(
        after["targets"][0]["commentText"].is_null(),
        "after={after}"
    );
    assert_eq!(after["targets"][0]["sourceState"], "restricted");
    assert_eq!(
        after["targets"][0]["state"], "succeeded",
        "restriction must not silently change the frozen target lifecycle state"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn read_targets_reports_unknown_source_state_without_panicking_when_body_text_is_absent() {
    // The source gate (comment_study_source.rs) only ever selects KNOWN-body comments, and
    // linggan_material_comment rows are append-only (a direct UPDATE is rejected by
    // linggan_plugin_runtime_forbid_mutation(), confirmed while writing this test), so a target's
    // source_ref cannot legitimately regress to an UNKNOWN body after freeze through any real code
    // path today. `seed_running_target` bypasses the eligibility gate entirely (it is a raw INSERT),
    // so this constructs the otherwise-unreachable combination directly: an UNKNOWN-body comment
    // wired as a target's source_ref from the start. This proves the defensive
    // `sourceState:"unknown"` branch in read_targets returns a null commentText instead of
    // panicking on a missing column value; it does not claim this combination can arise in
    // production.
    let database = proof_database("comment_study_read_targets_unknown_body").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-read-unknown-body-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let package = fixture::submit_package_at(
        &database,
        "comments",
        serde_json::json!({"contentExternalId":"study-read-unknown-body-note"}),
        serde_json::json!({
            "kind":"comment",
            "sourceObject":{"platform":"xhs","type":"content","externalId":"study-read-unknown-body-note"},
            "payload":{
                "commentId":"study-read-unknown-body-comment",
                "noteId":"study-read-unknown-body-note",
                "authorId":"reader-1"
            },
        }),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let (source_ref, content_public_ref): (Uuid, Uuid) = sqlx::query_as(
        "SELECT material_ref,content_public_ref FROM linggan_material_comment WHERE package_ref=$1",
    )
    .bind(package)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT body_state FROM linggan_material_comment WHERE material_ref=$1"
        )
        .bind(source_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "UNKNOWN",
        "fixture must actually omit the comment body for this proof to mean anything"
    );
    let policy_ref = seed_study_policy(&database).await;
    seed_running_target(&database, policy_ref, content_public_ref, source_ref).await;

    let query = domain_read_query(None);
    let runs = read_runs(&database, &query).await.unwrap();
    let run_ref = runs["runs"][0]["runRef"].as_str().unwrap().parse().unwrap();
    let run_query = domain_read_query(Some(run_ref));
    let targets = read_targets(&database, &run_query).await.unwrap();
    assert!(
        targets["targets"][0]["commentText"].is_null(),
        "targets={targets}"
    );
    assert_eq!(targets["targets"][0]["sourceState"], "unknown");
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn read_signals_hides_evidence_and_proposition_once_the_source_becomes_restricted_after_freeze()
 {
    // read_targets already refused to keep returning a comment's text once its source became
    // restricted after freezing. A Signal's `evidence` is a guaranteed literal substring of that
    // same comment text (see comment_study_semantic.rs), and `proposition` is a derived summary of
    // it, so read_signals must apply the identical restriction check on the same lineage, not just
    // read_targets. Before this fix, the "待归并" (pending merge) tab kept quoting the original
    // wording verbatim after its source had been restricted.
    let database = proof_database("comment_study_read_signals_restricted").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-read-signal-restricted-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-read-signal-restricted-note",
        "study-read-signal-restricted-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let content_public_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    let target_ref =
        seed_running_target(&database, policy_ref, content_public_ref, source_ref).await;
    accept_target_output(
        &database,
        target_ref,
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        None,
        semantic_output("每天写作业都要催,不催就不开始"),
    )
    .await
    .unwrap();
    let query = domain_read_query(None);
    let runs = read_runs(&database, &query).await.unwrap();
    let run_ref = runs["runs"][0]["runRef"].as_str().unwrap().parse().unwrap();
    let run_query = domain_read_query(Some(run_ref));
    let before = read_signals(&database, &run_query).await.unwrap();
    assert_eq!(before["signals"][0]["sourceState"], "known");
    assert_eq!(
        before["signals"][0]["evidence"], "每天写作业都要催，不催就不开始",
        "before={before}"
    );
    assert_eq!(
        before["signals"][0]["proposition"],
        "孩子在家庭作业中存在自主启动困难。"
    );

    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'restricted after read-signal proof' \
           FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .execute(database.pool())
    .await
    .unwrap();

    let after = read_signals(&database, &run_query).await.unwrap();
    assert_eq!(after["signals"][0]["sourceState"], "restricted");
    assert!(after["signals"][0]["evidence"].is_null(), "after={after}");
    assert!(
        after["signals"][0]["proposition"].is_null(),
        "after={after}"
    );
    assert_eq!(
        after["signals"][0]["eligibilityState"], "eligible",
        "restriction must not silently change the signal's own eligibility fact"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn user_selected_work_run_freezes_only_selected_comments() {
    let database = proof_database("comment_study_run_selected_work").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(&database, "study-run-one", "ADHD 笔记一", Some("creator-1")).await;
    detail_with_author(&database, "study-run-two", "ADHD 笔记二", Some("creator-2")).await;
    let selected_source = comment_with_author(
        &database,
        "study-run-one",
        "study-run-one-comment",
        "孩子作业总要催。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let selected_newer_source = comment_with_author(
        &database,
        "study-run-one",
        "study-run-one-newer-comment",
        "同一作品第二条合格评论。",
        Some("reader-3"),
        "2026-09-16T08:02:00Z",
    )
    .await;
    comment_with_author(
        &database,
        "study-run-two",
        "study-run-two-comment",
        "另一篇作品的评论。",
        Some("reader-2"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let selected_work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(selected_source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    sqlx::query("UPDATE linggan_comment_study_policy SET comment_budget=1 WHERE policy_ref=$1")
        .bind(policy_ref)
        .execute(database.pool())
        .await
        .unwrap();
    let preview = preview_sources(&database, PROOF_DOMAIN_REF, "2099-01-01T00:00:00Z")
        .await
        .unwrap();
    assert_eq!(
        preview
            .works
            .iter()
            .find(|work| work.work_ref == selected_work)
            .map(|work| work.eligible_comment_count),
        Some(2),
        "setup must report all selected-work candidates before the run applies its policy budget"
    );
    let prepared = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![selected_work],
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared.selected_work_count, 1);
    assert_eq!(prepared.covered_work_count, 1);
    assert_eq!(prepared.comment_budget, 1);
    assert_eq!(prepared.target_count, 1);
    assert_eq!(prepared.needs_context_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT source_ref FROM linggan_comment_study_target WHERE run_ref=$1",
        )
        .bind(prepared.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        selected_newer_source,
        "the same selected-work gate is applied before its policy budget truncates the frozen set"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn next_run_needing_batch_finds_a_freshly_created_runs_queued_targets_and_can_batch_them() {
    // Before this fix, nothing in production code ever called prepare_study_batch: a StudyRun's
    // targets were frozen as `queued` by prepare_study_run (the real HTTP-driven "创建研究运行"
    // path exercised here) and then sat there forever, because the model worker's
    // claim_next_study_batch loop only ever claims batches that already exist — it never creates
    // one. This walks the exact real path a user triggers end to end: prepare_study_run →
    // next_run_needing_batch (the new "which run should be packaged next" query) →
    // prepare_study_batch, and asserts the run actually leaves `queued` behind.
    let database = proof_database("comment_study_next_run_needing_batch").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-dispatch-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-dispatch-note",
        "study-dispatch-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_study_policy(&database).await;

    assert_eq!(
        next_run_needing_batch(&database, &[]).await.unwrap(),
        None,
        "no run exists yet; there is nothing to package"
    );

    let prepared = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    assert_eq!(prepared.target_count, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1"
        )
        .bind(prepared.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued",
        "prepare_study_run only freezes targets; it must not itself start processing them"
    );

    assert_eq!(
        next_run_needing_batch(&database, &[]).await.unwrap(),
        Some(prepared.run_ref),
        "the freshly created run has a queued target and must be found"
    );

    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: prepared.run_ref,
            maximum_targets: MAX_TARGETS_PER_BATCH,
        },
    )
    .await
    .unwrap();
    assert_eq!(batch.target_refs.len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1"
        )
        .bind(prepared.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "running",
        "packaging into a batch must move the target off queued"
    );

    assert_eq!(
        next_run_needing_batch(&database, &[]).await.unwrap(),
        None,
        "every queued target for this run has now been packaged; nothing left to find"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_run_whose_target_never_fits_its_model_budget_does_not_starve_a_later_run() {
    // A single comment can be longer than its own run's configured model input budget.
    // fit_targets_to_model_budget then keeps every candidate out of the batch, so
    // prepare_study_batch reports NoQueuedTargets even though a queued target still exists.
    // next_run_needing_batch always picks the *oldest* run with a queued target, so without the
    // exclusion list this older, permanently-unbatchable run would be picked again on every single
    // tick forever, and a perfectly fine run created after it would never get a turn. This proves
    // prepare_next_batch_across_runs — the actual function the worker calls every tick — walks
    // past the stuck run within one call and lets the later run through.
    let database = proof_database("comment_study_batch_dispatch_starvation").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-dispatch-stuck-note",
        "ADHD 笔记（评论超长）",
        Some("creator-1"),
    )
    .await;
    detail_with_author(
        &database,
        "study-dispatch-fine-note",
        "ADHD 笔记（正常）",
        Some("creator-2"),
    )
    .await;
    let oversized_text = "孩子每天写作业都要催不催就不开始".repeat(400);
    let stuck_source = comment_with_author(
        &database,
        "study-dispatch-stuck-note",
        "study-dispatch-stuck-comment",
        &oversized_text,
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let fine_source = comment_with_author(
        &database,
        "study-dispatch-fine-note",
        "study-dispatch-fine-comment",
        "孩子写作业总是拖拉。",
        Some("reader-2"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    let stuck_work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(stuck_source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let fine_work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(fine_source)
    .fetch_one(database.pool())
    .await
    .unwrap();

    let policy_ref = seed_study_policy(&database).await;
    // The lowest limit the schema allows (linggan_model_config_input_token_limit_check requires
    // 1024..=32768): the oversized single comment's own manifest cannot fit even at this floor,
    // while the ordinary-length comment in the second run fits comfortably. linggan_model_config
    // rows are immutable (see linggan_model_config_immutable), so the limit must be set at insert
    // time.
    seed_study_model_config_with_input_limit(&database, policy_ref, 1024).await;

    let stuck_run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![stuck_work],
        },
    )
    .await
    .unwrap();
    // created_at has second precision here; make sure the stuck run really is the older one.
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let fine_run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![fine_work],
        },
    )
    .await
    .unwrap();
    assert_eq!(stuck_run.target_count, 1);
    assert_eq!(fine_run.target_count, 1);

    assert!(
        prepare_next_batch_across_runs(&database).await.unwrap(),
        "the fine run's target must still get packaged in this same tick"
    );

    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1"
        )
        .bind(fine_run.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "running",
        "the later, batchable run must not be starved by the earlier stuck one"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE run_ref=$1"
        )
        .bind(stuck_run.run_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued",
        "the stuck run's target is honestly left queued, not silently dropped or faked as failed"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn batch_freezes_only_one_work_context_and_marks_only_its_targets_running() {
    let database = proof_database("comment_study_same_work_batch").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-batch-one",
        "第一篇 ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    detail_with_author(
        &database,
        "study-batch-two",
        "第二篇 ADHD 笔记",
        Some("creator-2"),
    )
    .await;
    let first_source = comment_with_author(
        &database,
        "study-batch-one",
        "study-batch-one-comment-1",
        "第一篇的第一条评论。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let _second_source = comment_with_author(
        &database,
        "study-batch-one",
        "study-batch-one-comment-2",
        "第一篇的第二条评论。",
        Some("reader-2"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    let other_source = comment_with_author(
        &database,
        "study-batch-two",
        "study-batch-two-comment-1",
        "第二篇的评论。",
        Some("reader-3"),
        "2026-09-16T08:02:00Z",
    )
    .await;
    let first_work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(first_source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let second_work: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(other_source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_study_policy(&database).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![first_work, second_work],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert!(!batch.target_refs.is_empty());
    let batch_work_refs: Vec<Uuid> = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_comment_study_target WHERE target_ref=ANY($1)",
    )
    .bind(&batch.target_refs)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert!(
        batch_work_refs
            .iter()
            .all(|work_ref| *work_ref == batch.content_public_ref)
    );
    assert_eq!(
        batch.input_manifest["workRef"],
        batch.content_public_ref.to_string()
    );
    let other_work = if batch.content_public_ref == first_work {
        second_work
    } else {
        first_work
    };
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_target \
             WHERE run_ref=$1 AND content_public_ref=$2 AND state='queued'",
        )
        .bind(run.run_ref)
        .bind(other_work)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        if other_work == first_work { 2 } else { 1 },
        "another work remains entirely queued for a later batch"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_malformed_sibling_result_does_not_undo_the_signals_of_a_valid_target() {
    let database = proof_database("comment_study_batch_target_isolation").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-isolation-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    for (comment_id, author_id) in [
        ("study-isolation-comment-1", "reader-1"),
        ("study-isolation-comment-2", "reader-2"),
    ] {
        comment_with_author(
            &database,
            "study-isolation-note",
            comment_id,
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            Some(author_id),
            "2026-09-16T08:00:00Z",
        )
        .await;
    }
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment \
         WHERE comment_external_id='study-isolation-comment-1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    seed_study_model_config(&database, policy_ref).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(batch.target_refs.len(), 2);
    let claim = claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    let reserved = reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
        .await
        .unwrap();
    // The second result carries a signal `kind` in the outcome slot, exactly as the provider sent
    // on 2026-09-17. Deserializing the response as one strict document used to reject the whole
    // batch, so the first target's perfectly valid Signals were discarded with it.
    let receipt = accept_study_batch_output(
        &database,
        batch.batch_ref,
        claim.lease_token,
        serde_json::json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":batch.batch_ref,
            "contentPublicRef":batch.content_public_ref,
            "results":[
                {
                    "targetRef":batch.target_refs[0],
                    "outcome":"signals",
                    "reason":null,
                    "signals":[{
                        "kind":"problem",
                        "proposition":"孩子在家庭作业中存在自主启动困难。",
                        "evidence":"每天写作业都要催,不催就不开始",
                        "problemFrame":{
                            "actor":{"value":"评论者","basis":"我很着急"},
                            "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
                            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                            "context":{"value":"家庭作业","basis":"写作业"}
                        }
                    }]
                },
                {"targetRef":batch.target_refs[1],"outcome":"experience","reason":null,"signals":[]}
            ]
        }),
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "completed_with_failures");
    assert_eq!(receipt.accepted_target_count, 1);
    assert_eq!(receipt.retried_target_count, 1);
    assert_eq!(receipt.failed_target_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_signal WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT state,rejection_code FROM linggan_comment_study_semantic_attempt \
             WHERE target_ref=$1",
        )
        .bind(batch.target_refs[1])
        .fetch_all(database.pool())
        .await
        .unwrap(),
        vec![(
            "rejected".to_owned(),
            Some("semantic_json_schema".to_owned())
        )]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(batch.target_refs[1])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued"
    );
    // The valid target must not be asked for again: a repair pass covers only the failed sibling.
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_model_invocation WHERE invocation_ref=$1",
        )
        .bind(reserved.invocation_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "succeeded"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_run_closes_as_completed_once_every_target_resolves() {
    let database = proof_database("comment_study_run_completed").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-run-closure-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    for (comment_id, author_id) in [
        ("study-run-closure-comment-1", "reader-1"),
        ("study-run-closure-comment-2", "reader-2"),
    ] {
        comment_with_author(
            &database,
            "study-run-closure-note",
            comment_id,
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            Some(author_id),
            "2026-09-16T08:00:00Z",
        )
        .await;
    }
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment \
         WHERE comment_external_id='study-run-closure-comment-1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    seed_study_model_config(&database, policy_ref).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    assert_eq!(batch.target_refs.len(), 2);
    // Still open while its targets are only frozen, not resolved.
    assert_eq!(
        run_state(&database, run.run_ref).await,
        ("running".to_owned(), false)
    );
    let claim = claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
        .await
        .unwrap();
    // One target yields Signals and the other honestly yields none. `no_signal` is a resolved
    // outcome, so neither of them makes this run a partial failure.
    let receipt = accept_study_batch_output(
        &database,
        batch.batch_ref,
        claim.lease_token,
        serde_json::json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":batch.batch_ref,
            "contentPublicRef":batch.content_public_ref,
            "results":[
                {
                    "targetRef":batch.target_refs[0],
                    "outcome":"signals",
                    "reason":null,
                    "signals":[{
                        "kind":"problem",
                        "proposition":"孩子在家庭作业中存在自主启动困难。",
                        "evidence":"每天写作业都要催,不催就不开始",
                        "problemFrame":{
                            "actor":{"value":"评论者","basis":"我很着急"},
                            "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
                            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                            "context":{"value":"家庭作业","basis":"写作业"}
                        }
                    }]
                },
                {"targetRef":batch.target_refs[1],"outcome":"no_signal",
                 "reason":"未表达研究合同中的信号","signals":[]}
            ]
        }),
    )
    .await
    .unwrap();
    assert_eq!(receipt.accepted_target_count, 2);
    assert_eq!(
        run_state(&database, run.run_ref).await,
        ("completed".to_owned(), true)
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn batch_admission_keeps_valid_target_when_a_sibling_is_missing() {
    let database = proof_database("comment_study_batch_partial_acceptance").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-batch-accept-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    for (comment_id, author_id) in [
        ("study-batch-accept-comment-1", "reader-1"),
        ("study-batch-accept-comment-2", "reader-2"),
    ] {
        comment_with_author(
            &database,
            "study-batch-accept-note",
            comment_id,
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            Some(author_id),
            "2026-09-16T08:00:00Z",
        )
        .await;
    }
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment \
         WHERE comment_external_id='study-batch-accept-comment-1'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    seed_study_model_config(&database, policy_ref).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 12,
        },
    )
    .await
    .unwrap();
    assert_eq!(batch.target_refs.len(), 2);
    let claim = claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    let reserved = reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
        .await
        .unwrap();
    assert_eq!(
        reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
            .await
            .unwrap()
            .invocation_ref,
        reserved.invocation_ref,
        "repeating a live reservation reuses the same ledger receipt"
    );
    let receipt = accept_study_batch_output(
        &database,
        batch.batch_ref,
        claim.lease_token,
        serde_json::json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":batch.batch_ref,
            "contentPublicRef":batch.content_public_ref,
            "results":[{
                "targetRef":batch.target_refs[0],
                "outcome":"signals",
                "reason":null,
                "signals":[{
                    "kind":"problem",
                    "proposition":"孩子在家庭作业中存在自主启动困难。",
                    "evidence":"每天写作业都要催,不催就不开始",
                    "problemFrame":{
                        "actor":{"value":"评论者","basis":"我很着急"},
                        "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
                        "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
                        "context":{"value":"家庭作业","basis":"写作业"}
                    }
                }]
            }]
        }),
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "completed_with_failures");
    assert_eq!(receipt.accepted_target_count, 1);
    assert_eq!(receipt.retried_target_count, 1);
    assert_eq!(receipt.failed_target_count, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_model_invocation WHERE invocation_ref=$1",
        )
        .bind(reserved.invocation_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT model_invocation_ref FROM linggan_comment_study_semantic_attempt \
             WHERE batch_ref=$1 AND target_ref=$2",
        )
        .bind(batch.batch_ref)
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        Some(reserved.invocation_ref)
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "succeeded"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(batch.target_refs[1])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_signal WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1",
        )
        .bind(batch.batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "completed_with_failures"
    );
    // The retriable sibling is still queued, so closing the run here would declare a research
    // round finished while one of its comments has never been studied.
    assert_eq!(
        run_state(&database, run.run_ref).await,
        ("running".to_owned(), false)
    );
}

/// Drives one real batch through `call_study_batch_model` against a checked-in local process, so
/// the mapping from an actual provider response shape to a settled attempt is proven rather than
/// argued. `PiAdapter::configured_with_test_command` exists for exactly this and keeps the test on
/// the production adapter boundary.
async fn dispatch_one_batch_through_the_test_adapter(
    database: &linggan_storage_postgres::Database,
    note: &str,
    comment_id: &str,
    body: &str,
) -> (Uuid, Uuid, StudyModelRunnerError) {
    detail_with_author(database, note, "ADHD 笔记", Some("creator-1")).await;
    let source_ref = comment_with_author(
        database,
        note,
        comment_id,
        body,
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(database).await;
    seed_study_model_config(database, policy_ref).await;
    let run = prepare_study_run(
        database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    let adapter = PiAdapter::configured_with_test_command(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_settlement_adapter.sh"),
    );
    let error = call_study_batch_model(
        database,
        &SyntheticModelSecrets,
        &adapter,
        batch.batch_ref,
        claim.lease_token,
    )
    .await
    .expect_err("the synthetic child never returns a contract response");
    (batch.batch_ref, batch.target_refs[0], error)
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_transport_failure_from_the_provider_settles_the_batch_it_dispatched() {
    let database = proof_database("comment_study_dispatch_transport").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (batch_ref, target_ref, error) = dispatch_one_batch_through_the_test_adapter(
        &database,
        "study-transport-note",
        "study-transport-comment",
        "孩子每天写作业都要催，不催就不开始。SETTLEMENT_TRANSPORT_LIMIT",
    )
    .await;
    assert!(matches!(error, StudyModelRunnerError::ProviderFailure));
    // The provider's own code has to survive into the attempt, or a run of transport limits reads
    // the same as a run of crashes.
    assert_eq!(
        sqlx::query_as::<_, (i32, String, Option<String>, Option<String>, Option<String>)>(
            "SELECT attempt_ordinal,state,rejection_code,output_manifest->>'stage', \
                    output_manifest->>'providerFailureCode' \
             FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_all(database.pool())
        .await
        .unwrap(),
        vec![(
            1,
            "rejected".to_owned(),
            Some("provider_failure".to_owned()),
            Some("provider_dispatch".to_owned()),
            Some("response_too_large".to_owned())
        )]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1",
        )
        .bind(batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT state,failure_code FROM linggan_model_invocation \
             WHERE invocation_ref=(SELECT model_invocation_ref FROM linggan_comment_study_batch \
                                   WHERE batch_ref=$1)",
        )
        .bind(batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        ("failed".to_owned(), Some("response_too_large".to_owned()))
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_unparsable_response_banks_its_usage_before_it_settles() {
    let database = proof_database("comment_study_dispatch_unparsable").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (batch_ref, target_ref, error) = dispatch_one_batch_through_the_test_adapter(
        &database,
        "study-unparsable-note",
        "study-unparsable-comment",
        "孩子每天写作业都要催，不催就不开始。SETTLEMENT_UNPARSEABLE",
    )
    .await;
    assert!(matches!(error, StudyModelRunnerError::OutputNotJson));
    // The call was billed even though its text was useless, so the tokens must be on the ledger
    // and the invocation must not be left `running`.
    //
    // `callStarted` is the discriminating assertion: only `checkpoint_invocation_usage` writes that
    // key, and it has to run *before* the text is judged. Asserting the token counts alone proves
    // nothing about that order, because the failure finisher records usage from the same response
    // on its own — a mutation restoring the old order keeps the counts green and only loses this
    // key. Without the checkpoint, a crash between the provider returning and the finisher running
    // loses the usage entirely.
    assert_eq!(
        sqlx::query_as::<
            _,
            (
                Option<i64>,
                Option<i64>,
                String,
                Option<String>,
                Option<String>
            ),
        >(
            "SELECT input_tokens,output_tokens,state,failure_code,result->>'callStarted' \
             FROM linggan_model_invocation \
             WHERE invocation_ref=(SELECT model_invocation_ref FROM linggan_comment_study_batch \
                                   WHERE batch_ref=$1)",
        )
        .bind(batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        (
            Some(111),
            Some(222),
            "failed".to_owned(),
            Some("model_invalid_output".to_owned()),
            Some("true".to_owned())
        )
    );
    assert_eq!(
        sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
            "SELECT state,rejection_code,output_manifest->>'providerFailureCode' \
             FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
        )
        .bind(target_ref)
        .fetch_all(database.pool())
        .await
        .unwrap(),
        vec![(
            "rejected".to_owned(),
            Some("provider_failure".to_owned()),
            Some("model_invalid_output".to_owned())
        )]
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn repeated_dispatch_failures_exhaust_a_target_instead_of_re_leasing_it_forever() {
    let database = proof_database("comment_study_dispatch_bound").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-dispatch-bound-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-dispatch-bound-note",
        "study-dispatch-bound-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    seed_study_model_config(&database, policy_ref).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();

    // Every dispatch answers the way the oversized note in #305 did: the provider call comes back
    // as a transport failure rather than a contract response, so nothing reaches semantic
    // acceptance and the old code recorded no attempt at all.
    let mut target_states = Vec::new();
    for _ in 0..3 {
        let batch = prepare_study_batch(
            &database,
            PrepareStudyBatchRequest {
                run_ref: run.run_ref,
                maximum_targets: 1,
            },
        )
        .await
        .unwrap();
        let claim = claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
            .await
            .unwrap()
            .expect("a prepared batch is claimable");
        reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
            .await
            .unwrap();
        reject_study_batch_dispatch(
            &database,
            batch.batch_ref,
            claim.lease_token,
            Some("response_too_large"),
        )
        .await
        .unwrap();
        target_states.push(
            sqlx::query_scalar::<_, String>(
                "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
            )
            .bind(batch.target_refs[0])
            .fetch_one(database.pool())
            .await
            .unwrap(),
        );
    }
    assert_eq!(target_states, vec!["queued", "queued", "failed"]);

    let attempts: Vec<(i32, String, Option<String>)> = sqlx::query_as(
        "SELECT attempt_ordinal,state,rejection_code \
         FROM linggan_comment_study_semantic_attempt ORDER BY attempt_ordinal",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert_eq!(
        attempts,
        vec![
            (
                1,
                "rejected".to_owned(),
                Some("provider_failure".to_owned())
            ),
            (
                2,
                "rejected".to_owned(),
                Some("provider_failure".to_owned())
            ),
            (
                3,
                "rejected".to_owned(),
                Some("provider_failure".to_owned())
            ),
        ]
    );
    // The coarse rejection code is all the schema allows; the layer that actually failed has to
    // stay recoverable from the attempt itself, otherwise a run of transport limits is
    // indistinguishable from a run of crashes.
    let recorded_failure_code: Option<String> = sqlx::query_scalar(
        "SELECT output_manifest->>'providerFailureCode' \
         FROM linggan_comment_study_semantic_attempt ORDER BY attempt_ordinal LIMIT 1",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(recorded_failure_code.as_deref(), Some("response_too_large"));

    // The bound has to be real: a fourth dispatch must be impossible, not merely slower.
    assert!(matches!(
        prepare_study_batch(
            &database,
            PrepareStudyBatchRequest {
                run_ref: run.run_ref,
                maximum_targets: 1,
            },
        )
        .await,
        Err(_)
    ));
    assert_eq!(
        claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
            .await
            .unwrap()
            .map(|claim| claim.batch_ref),
        None
    );
    // The run has to close with its last target. Leaving it `running` for good is what made a
    // finished run indistinguishable from one still waiting on a model.
    assert_eq!(
        run_state(&database, run.run_ref).await,
        ("completed_with_failures".to_owned(), true)
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn expired_batch_lease_rejects_late_output_and_returns_target_to_queue() {
    let database = proof_database("comment_study_batch_lease_recovery").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-batch-lease-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-batch-lease-note",
        "study-batch-lease-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    seed_study_model_config(&database, policy_ref).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    assert_eq!(claim.batch_ref, batch.batch_ref);
    let reserved = reserve_study_batch_model_call(&database, batch.batch_ref, claim.lease_token)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_batch \
         SET lease_expires_at=scope_001_now()-interval '1 second' WHERE batch_ref=$1",
    )
    .bind(batch.batch_ref)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        recover_expired_study_batch_leases(&database).await.unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_target WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "queued"
    );
    // Returning to `queued` is only safe because the recovery now spends an attempt: a batch that
    // always outlives its lease would otherwise be re-dispatched on real, billed calls forever.
    assert_eq!(
        sqlx::query_as::<_, (i32, String, Option<String>, Option<String>)>(
            "SELECT attempt_ordinal,state,rejection_code,output_manifest->>'stage' \
             FROM linggan_comment_study_semantic_attempt WHERE target_ref=$1",
        )
        .bind(batch.target_refs[0])
        .fetch_all(database.pool())
        .await
        .unwrap(),
        vec![(
            1,
            "rejected".to_owned(),
            Some("provider_failure".to_owned()),
            Some("lease_expired".to_owned())
        )]
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1",
        )
        .bind(batch.batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "failed"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_model_invocation WHERE invocation_ref=$1",
        )
        .bind(reserved.invocation_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "failed"
    );
    assert!(matches!(
        accept_study_batch_output(
            &database,
            batch.batch_ref,
            claim.lease_token,
            serde_json::json!({})
        )
        .await,
        Err(BatchAcceptanceError::BatchUnavailable)
    ));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn source_restriction_after_freeze_cancels_batch_without_leasing_it() {
    let database = proof_database("comment_study_batch_source_recheck").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-batch-recheck-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let source_ref = comment_with_author(
        &database,
        "study-batch-recheck-note",
        "study-batch-recheck-comment",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_study_policy(&database).await;
    let run = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        &database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 1,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_material_comment_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'restricted after batch freeze' \
           FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(source_ref)
    .execute(database.pool())
    .await
    .unwrap();

    assert!(
        claim_next_study_batch(&database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT state FROM linggan_comment_study_batch WHERE batch_ref=$1",
        )
        .bind(batch.batch_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "cancelled"
    );
    let target = sqlx::query(
        "SELECT state,dependency_state,exclusion_reason \
         FROM linggan_comment_study_target WHERE target_ref=$1",
    )
    .bind(batch.target_refs[0])
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(target.get::<String, _>("state"), "excluded");
    assert_eq!(target.get::<String, _>("dependency_state"), "input_invalid");
    assert_eq!(
        target
            .get::<Option<String>, _>("exclusion_reason")
            .as_deref(),
        Some("source_unavailable_after_freeze")
    );
    // Nothing of this run was ever studiable, which is a different outcome from a run that tried
    // and lost targets.
    assert_eq!(
        run_state(&database, run.run_ref).await,
        ("cancelled".to_owned(), true)
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn reply_context_is_frozen_as_context_but_not_evidence() {
    let database = proof_database("comment_study_run_parent_context").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-parent-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let parent_ref = comment_with_author(
        &database,
        "study-parent-note",
        "study-parent-comment",
        "孩子每天写作业都要催。",
        Some("creator-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let reply_ref = reply_with_author(
        &database,
        "study-parent-note",
        "study-parent-reply",
        "study-parent-comment",
        "我也是",
        Some("reader-2"),
        "2026-09-16T08:00:01Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(parent_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    seed_study_policy(&database).await;
    let prepared = prepare_study_run(
        &database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let target_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM linggan_comment_study_target WHERE run_ref=$1")
            .bind(prepared.run_ref)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        target_count, 1,
        "the creator parent is context, not a target"
    );
    let reply_target = sqlx::query(
        "SELECT dependency_state,state,parent_source_ref,input_manifest \
         FROM linggan_comment_study_target WHERE run_ref=$1 AND source_ref=$2",
    )
    .bind(prepared.run_ref)
    .bind(reply_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        reply_target.get::<String, _>("dependency_state"),
        "parent_available"
    );
    assert_eq!(reply_target.get::<String, _>("state"), "queued");
    assert_eq!(reply_target.get::<Uuid, _>("parent_source_ref"), parent_ref);
    assert_eq!(
        reply_target.get::<serde_json::Value, _>("input_manifest")["parentContext"]["sourceRef"],
        parent_ref.to_string()
    );
}

/// Builds one work with two differently authored comments, runs them through the production batch
/// path so their Signals carry canonical text, and returns both Signal refs.
/// Fixes the order a Signal was recorded in. Both Signals of a work are written in one
/// transaction, so `created_at` ties and every rule that falls back to it decides by a random
/// UUID — which would make an ordering assertion pass or fail by chance.
async fn record_order(
    database: &linggan_storage_postgres::Database,
    signal_ref: Uuid,
    recorded_at: &str,
) {
    sqlx::query(
        "UPDATE linggan_comment_study_signal SET created_at=$2::timestamptz WHERE signal_ref=$1",
    )
    .bind(signal_ref)
    .bind(recorded_at)
    .execute(database.pool())
    .await
    .unwrap();
}

async fn two_eligible_signals(
    database: &linggan_storage_postgres::Database,
    note: &str,
) -> (Uuid, Uuid) {
    two_eligible_signals_from(
        database,
        note,
        ["reader-1", "reader-2"],
        ["需要外部催促", "迟迟无法开始"],
    )
    .await
}

/// Comment evidence is append-only, so which account a Signal belongs to has to be decided here,
/// when the comment is captured, rather than corrected afterwards.
///
/// The barriers are a parameter because the canonical text is what the embedding cache is keyed
/// by: two notes given the same barrier produce the same Signal text on purpose, and therefore
/// share one vector. A caller that needs its Signals to sit at different points in the space has
/// to say different things.
async fn two_eligible_signals_from(
    database: &linggan_storage_postgres::Database,
    note: &str,
    authors: [&str; 2],
    barriers: [&str; 2],
) -> (Uuid, Uuid) {
    detail_with_author(database, note, "ADHD 笔记", Some("creator-1")).await;
    for (index, author) in authors.iter().enumerate() {
        comment_with_author(
            database,
            note,
            &format!("{note}-comment-{index}"),
            "孩子每天写作业都要催，不催就不开始，我很着急。",
            Some(author),
            "2026-09-16T08:00:00Z",
        )
        .await;
    }
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE comment_external_id=$1",
    )
    .bind(format!("{note}-comment-0"))
    .fetch_one(database.pool())
    .await
    .unwrap();
    // A Domain has one active policy. Reuse it when a second work in this test shares the Domain.
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT policy_ref FROM linggan_comment_study_active_policy WHERE domain_ref=$1",
    )
    .bind(PROOF_DOMAIN_REF)
    .fetch_optional(database.pool())
    .await
    .unwrap();
    let policy_ref = match existing {
        Some(policy_ref) => policy_ref,
        None => {
            let policy_ref = seed_study_policy(database).await;
            seed_study_model_config(database, policy_ref).await;
            policy_ref
        }
    };
    let _ = policy_ref;
    let run = prepare_study_run(
        database,
        PrepareStudyRunRequest {
            domain_ref: PROOF_DOMAIN_REF,
            content_public_refs: vec![work_ref],
        },
    )
    .await
    .unwrap();
    let batch = prepare_study_batch(
        database,
        PrepareStudyBatchRequest {
            run_ref: run.run_ref,
            maximum_targets: 2,
        },
    )
    .await
    .unwrap();
    let claim = claim_next_study_batch(database, Uuid::new_v4(), DEFAULT_BATCH_LEASE_SECONDS)
        .await
        .unwrap()
        .expect("the prepared batch is claimable");
    reserve_study_batch_model_call(database, batch.batch_ref, claim.lease_token)
        .await
        .unwrap();
    let signal = |target: Uuid, barrier: &str| {
        serde_json::json!({
            "targetRef":target,"outcome":"signals","reason":null,
            "signals":[{
                "kind":"problem",
                "proposition":"孩子在家庭作业中存在自主启动困难。",
                "evidence":"每天写作业都要催,不催就不开始",
                "problemFrame":{
                    "actor":{"value":"孩子","basis":"孩子"},
                    "goalOrExpectedState":{"value":"自主开始作业","basis":"不催就不开始"},
                    "barrierOrUnmetNeed":{"value":barrier,"basis":"都要催"},
                    "context":{"value":"家庭作业","basis":"写作业"}
                }
            }]
        })
    };
    accept_study_batch_output(
        database,
        batch.batch_ref,
        claim.lease_token,
        serde_json::json!({
            "contract":"comment-study.note-batch.v1",
            "batchRef":batch.batch_ref,
            "contentPublicRef":batch.content_public_ref,
            "results":[
                signal(batch.target_refs[0], barriers[0]),
                signal(batch.target_refs[1], barriers[1])
            ]
        }),
    )
    .await
    .unwrap();
    // Keyed by target, never by insertion order: both Signals are written in one transaction, so
    // `created_at` ties and the ordering falls through to a random UUID. A caller that relied on
    // that would pass or fail depending on which UUID sorted first.
    let signal_for = |target: Uuid| async move {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT signal_ref FROM linggan_comment_study_signal WHERE target_ref=$1",
        )
        .bind(target)
        .fetch_one(database.pool())
        .await
        .unwrap()
    };
    (
        signal_for(batch.target_refs[0]).await,
        signal_for(batch.target_refs[1]).await,
    )
}

async fn resolution_state_for(
    database: &linggan_storage_postgres::Database,
    signal_ref: Uuid,
) -> Option<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT state,decision_manifest->>'retrievalIncompleteReason' \
         FROM linggan_comment_study_resolution WHERE signal_ref=$1",
    )
    .bind(signal_ref)
    .fetch_optional(database.pool())
    .await
    .unwrap()
}

/// Attaches an already-encoded Signal to a Problem as a confirmed member.
async fn seed_membership(
    database: &linggan_storage_postgres::Database,
    signal_ref: Uuid,
    problem_ref: Uuid,
) {
    let resolution_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,resolved_problem_ref,resolved_at \
         ) VALUES($1,$2,$3,'assigned','{}'::jsonb,$4,scope_001_now())",
    )
    .bind(resolution_ref)
    .bind(signal_ref)
    .bind(PROOF_DOMAIN_REF)
    .bind(problem_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_membership( \
           membership_ref,signal_ref,problem_ref,resolution_ref) VALUES($1,$2,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(signal_ref)
    .bind(problem_ref)
    .bind(resolution_ref)
    .execute(database.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn representatives_are_chosen_for_reach_rather_than_for_being_nearest() {
    let database = proof_database("comment_study_representatives").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    // Four members covering the four roles the rules distinguish: the lead, a same-account
    // same-note twin that adds no reach, a member on another account and another note, and the
    // member furthest from the lead. Roles are assigned explicitly — the helper alone cannot
    // produce a same-account twin.
    let (lead, twin) = two_eligible_signals_from(
        &database,
        "study-rep-note-a",
        ["reader-1", "reader-1"],
        ["需要外部催促", "自己不愿动笔"],
    )
    .await;
    let (other_account, far) = two_eligible_signals_from(
        &database,
        "study-rep-note-b",
        ["reader-2", "reader-3"],
        ["拖到很晚才开始", "写一半就走神"],
    )
    .await;
    record_order(&database, lead, "2026-09-16T08:00:00Z").await;
    record_order(&database, twin, "2026-09-16T08:01:00Z").await;
    record_order(&database, other_account, "2026-09-16T08:02:00Z").await;
    record_order(&database, far, "2026-09-16T08:03:00Z").await;
    let profile = seed_embedding_profile(&database).await;
    let hash = |signal| signal_canonical_hash(&database, signal);
    seed_vector(&database, profile, &hash(lead).await, 0.0).await;
    seed_vector(&database, profile, &hash(twin).await, 0.02).await;
    seed_vector(&database, profile, &hash(other_account).await, 0.05).await;
    seed_vector(&database, profile, &hash(far).await, 1.2).await;
    let problem = seed_existing_problem(
        &database,
        "作业启动困难",
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        &[lead, twin],
    )
    .await;
    seed_vector(
        &database,
        profile,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        0.4,
    )
    .await;
    for member in [lead, twin, other_account, far] {
        seed_membership(&database, member, problem).await;
    }

    let chosen =
        problem_representatives(&database, profile, &hash(lead).await, PROOF_DOMAIN_REF, 3)
            .await
            .unwrap();
    assert_eq!(chosen.len(), 3);
    assert_eq!(chosen[0], lead, "a seed leads, whatever the distances say");
    assert_eq!(
        chosen[1], other_account,
        "second place goes to the earliest member on another account and another note"
    );
    assert_ne!(
        chosen[1], twin,
        "second place goes to a member that adds reach; the same-account same-note twin adds none"
    );
    assert_eq!(
        chosen[2], far,
        "third place spans the Problem: the furthest confirmed member, not the next nearest"
    );
    assert!(
        !chosen.contains(&twin),
        "with three richer candidates available the twin never earns a slot: {chosen:?}"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_comparison_already_paid_for_is_not_bought_again() {
    let database = proof_database("comment_study_comparison_cache").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-cache-note").await;
    let profile = seed_embedding_profile(&database).await;
    let first_hash = signal_canonical_hash(&database, first).await;
    seed_vector(&database, profile, &first_hash, 0.0).await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, second).await,
        0.1,
    )
    .await;
    let problem = seed_existing_problem(
        &database,
        "作业启动困难",
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        &[first, second],
    )
    .await;
    // The seeded revision's own core hash, so path B can reach the Problem at all.
    seed_vector(
        &database,
        profile,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        0.3,
    )
    .await;

    // A first comparison is recorded the way a real model call would leave it behind.
    let prepared = prepare_problem_resolution(&database, first, vec![problem])
        .await
        .unwrap();
    let verdict = serde_json::json!({
        "contract":"comment-study.problem-resolution.v1",
        "candidates":[{"problemRef":problem,"dimensions":{
            "actor":"different","goalOrExpectedState":"different",
            "barrierOrUnmetNeed":"different","context":"different"
        }}]
    });
    record_resolution_comparisons(&database, first, &verdict, None)
        .await
        .unwrap();
    accept_problem_resolution(&database, prepared.resolution_ref, verdict)
        .await
        .unwrap();

    // The *same* Signal text against the *same* Problem core under the same policy: a second
    // pending resolution must be answerable without reserving an invocation at all.
    let second_run = prepare_problem_resolution(&database, second, vec![problem])
        .await
        .unwrap();
    assert_eq!(second_run.state, "pending");
    let served = serve_pending_resolutions_from_cache(&database)
        .await
        .unwrap();

    let (state, invocation): (String, Option<Uuid>) = sqlx::query_as(
        "SELECT state,model_invocation_ref FROM linggan_comment_study_resolution \
         WHERE resolution_ref=$1",
    )
    .bind(second_run.resolution_ref)
    .fetch_one(database.pool())
    .await
    .unwrap();
    if signal_canonical_hash(&database, second).await == first_hash {
        // Identical canonical sentences share a cache key, which is the whole point.
        assert_eq!(served, 1);
        assert_eq!(state, "deferred_novel");
        assert_eq!(
            invocation, None,
            "a cached verdict must never reserve a model invocation"
        );
    } else {
        // Different sentences are a different question; the cache must not answer it.
        assert_eq!(served, 0);
        assert_eq!(state, "pending");
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_signal_is_never_called_novel_while_the_catalogue_cannot_be_searched() {
    let database = proof_database("comment_study_resolution_incomplete").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-resolution-gap-note").await;

    // No qualified profile at all: there is no catalogue to search, so nothing may be declared new.
    assert!(advance_next_problem_resolution(&database).await.unwrap());
    // Which of the two got advanced is not fixed, so it is recorded now rather than inferred
    // later: once both carry a resolution, "the other one" is no longer derivable.
    let settled_first = if resolution_state_for(&database, first).await.is_some() {
        first
    } else {
        second
    };
    assert_eq!(
        resolution_state_for(&database, settled_first).await,
        Some((
            "retrieval_incomplete".to_owned(),
            Some("no_qualified_profile".to_owned())
        ))
    );

    // With a profile and vectors, but an active Problem whose core was never encoded, the
    // catalogue is still only partly searchable — and still not evidence of novelty.
    let profile = seed_embedding_profile(&database).await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, first).await,
        0.0,
    )
    .await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, second).await,
        0.1,
    )
    .await;
    seed_existing_problem(
        &database,
        "另一类困难",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        &[first, second],
    )
    .await;
    assert!(advance_next_problem_resolution(&database).await.unwrap());
    assert_eq!(
        resolution_state_for(&database, settled_first).await,
        Some((
            "retrieval_incomplete".to_owned(),
            Some("problem_core_vectors_incomplete".to_owned())
        ))
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_incomplete_recall_retries_after_a_profile_becomes_qualified() {
    let database = proof_database("comment_study_resolution_resume").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-resolution-resume-note").await;

    assert!(advance_next_problem_resolution(&database).await.unwrap());
    let settled_first = if resolution_state_for(&database, first).await.is_some() {
        first
    } else {
        second
    };
    assert_eq!(
        resolution_state_for(&database, settled_first).await,
        Some((
            "retrieval_incomplete".to_owned(),
            Some("no_qualified_profile".to_owned())
        ))
    );

    let profile = seed_embedding_profile(&database).await;
    for signal in [first, second] {
        seed_vector(
            &database,
            profile,
            &signal_canonical_hash(&database, signal).await,
            0.0,
        )
        .await;
    }

    assert!(advance_next_problem_resolution(&database).await.unwrap());
    assert_eq!(
        resolution_state_for(&database, settled_first).await,
        Some(("deferred_novel".to_owned(), None)),
        "a complete, empty catalogue is now evidence of novelty"
    );
    let prior_reason: Option<String> = sqlx::query_scalar(
        "SELECT candidate_manifest->'priorRetrievalIncomplete'->>'retrievalIncompleteReason' \
         FROM linggan_comment_study_resolution WHERE signal_ref=$1",
    )
    .bind(settled_first)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(prior_reason.as_deref(), Some("no_qualified_profile"));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_searchable_but_empty_catalogue_is_the_one_case_that_means_novel() {
    let database = proof_database("comment_study_resolution_novel").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-resolution-novel-note").await;
    let profile = seed_embedding_profile(&database).await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, first).await,
        0.0,
    )
    .await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, second).await,
        0.1,
    )
    .await;
    // Nothing in the catalogue and nothing unsearchable: an empty candidate set here really does
    // mean "no existing Problem covers this", which is what deferred_novel asserts.
    assert!(advance_next_problem_resolution(&database).await.unwrap());
    let resolved = resolution_state_for(&database, first)
        .await
        .or(resolution_state_for(&database, second).await)
        .expect("the advanced Signal received a resolution");
    assert_eq!(resolved, ("deferred_novel".to_owned(), None));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_accepted_eligible_signal_carries_the_canonical_text_recall_searches_by() {
    let database = proof_database("comment_study_canonical_written").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, _second) = two_eligible_signals(&database, "study-canonical-note").await;
    let row = sqlx::query(
        "SELECT eligibility_state,canonical_text,canonical_hash \
         FROM linggan_comment_study_signal WHERE signal_ref=$1",
    )
    .bind(first)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(row.get::<String, _>("eligibility_state"), "eligible");
    let text: String = row.get("canonical_text");
    // The five slots are the shape recall depends on; a Signal stored without them would be
    // invisible to the pool while still looking like a healthy eligible Signal.
    assert_eq!(text.lines().count(), 5, "{text}");
    assert!(text.contains("障碍：需要外部催促"), "{text}");
    assert_eq!(row.get::<String, _>("canonical_hash").len(), 64);
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn the_unmerged_pool_is_what_lets_a_second_eligible_signal_find_the_first() {
    let database = proof_database("comment_study_recall_pool").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-recall-pool-note").await;
    let profile = seed_embedding_profile(&database).await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, first).await,
        0.0,
    )
    .await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, second).await,
        0.1,
    )
    .await;

    let recalled = recall_candidates(&database, profile, second).await.unwrap();
    assert_eq!(recalled.completeness, RecallCompleteness::Complete);
    assert_eq!(
        recalled.pool_signal_refs,
        vec![first],
        "with no Problem yet, the only thing to compare against is the other unmerged Signal"
    );
    assert!(recalled.problem_refs.is_empty());
    assert!(recalled.identity_problem_refs.is_empty());
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn the_unmerged_pool_keeps_an_independent_signal_with_the_same_canonical_text() {
    let database = proof_database("comment_study_recall_identical_pool").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals_from(
        &database,
        "study-recall-identical-pool-note",
        ["reader-1", "reader-2"],
        ["需要外部催促", "需要外部催促"],
    )
    .await;
    let profile = seed_embedding_profile(&database).await;
    let canonical_hash = signal_canonical_hash(&database, first).await;
    assert_eq!(
        canonical_hash,
        signal_canonical_hash(&database, second).await
    );
    seed_vector(&database, profile, &canonical_hash, 0.0).await;

    let recalled = recall_candidates(&database, profile, second).await.unwrap();
    assert_eq!(recalled.completeness, RecallCompleteness::Complete);
    assert_eq!(
        recalled.pool_signal_refs,
        vec![first],
        "a same-text Signal from a different source and author is the independent second reading \
         the pairing boundary must decide, not an item recall may discard"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn an_active_problem_without_an_encoded_core_makes_recall_report_itself_incomplete() {
    let database = proof_database("comment_study_recall_incomplete").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-recall-gap-note").await;
    let profile = seed_embedding_profile(&database).await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, first).await,
        0.0,
    )
    .await;
    seed_vector(
        &database,
        profile,
        &signal_canonical_hash(&database, second).await,
        0.1,
    )
    .await;
    // An active Problem whose core was never encoded is invisible to the vector path. Reporting
    // "no candidates" here would read exactly like "this is new" and create a duplicate.
    seed_existing_problem(
        &database,
        "另一类困难",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        &[first, second],
    )
    .await;

    let recalled = recall_candidates(&database, profile, second).await.unwrap();
    assert_eq!(
        recalled.completeness,
        RecallCompleteness::Incomplete {
            reason: "problem_core_vectors_incomplete"
        }
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn lexical_problem_candidate_recall_reads_the_active_problem_revision() {
    let database = proof_database("comment_study_revision_candidate_recall").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (seeker, supporter) =
        two_eligible_signals(&database, "study-revision-candidate-note").await;
    let problem = seed_existing_problem(
        &database,
        "孩子写作业时需要反复催促才开始",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        &[seeker, supporter],
    )
    .await;

    let recalled = recall_problem_candidates(&database, seeker).await.unwrap();
    assert_eq!(recalled.candidates.len(), 1);
    let candidate = &recalled.candidates[0];
    assert_eq!(candidate.problem_ref, problem);
    assert_eq!(candidate.definition, "孩子写作业时需要反复催促才开始");
    assert_eq!(candidate.stable_identity["actor"], "test");
    assert_eq!(
        candidate.include_criteria,
        serde_json::json!(["test inclusion"])
    );
    assert_eq!(candidate.exclude_criteria, serde_json::json!([]));
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn a_problem_reached_through_both_its_core_and_a_member_appears_once() {
    let database = proof_database("comment_study_recall_fold").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-recall-fold-note").await;
    let profile = seed_embedding_profile(&database).await;
    let first_hash = signal_canonical_hash(&database, first).await;
    let second_hash = signal_canonical_hash(&database, second).await;
    seed_vector(&database, profile, &first_hash, 0.05).await;
    seed_vector(&database, profile, &second_hash, 0.0).await;
    // Two seeds, because a Problem that one person's single reading produced is exactly what the
    // schema refuses: the second independent account is the point, not a formality.
    let problem = seed_existing_problem(
        &database,
        "作业启动困难",
        "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        &[first, second],
    )
    .await;
    // The seeded core hash, plus a member Signal that is also encoded: both paths reach the same
    // Problem, and folding has to leave exactly one entry rather than ranking it twice.
    seed_vector(
        &database,
        profile,
        "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
        0.2,
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_comment_study_resolution( \
           resolution_ref,signal_ref,domain_ref,state,candidate_manifest,resolved_problem_ref,resolved_at \
         ) VALUES($1,$2,$3,'assigned','{}'::jsonb,$4,scope_001_now())",
    )
    .bind(Uuid::new_v4())
    .bind(first)
    .bind(PROOF_DOMAIN_REF)
    .bind(problem)
    .execute(database.pool())
    .await
    .unwrap();
    let resolution: Uuid = sqlx::query_scalar(
        "SELECT resolution_ref FROM linggan_comment_study_resolution WHERE signal_ref=$1",
    )
    .bind(first)
    .fetch_one(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_membership( \
           membership_ref,signal_ref,problem_ref,resolution_ref) VALUES($1,$2,$3,$4)",
    )
    .bind(Uuid::new_v4())
    .bind(first)
    .bind(problem)
    .bind(resolution)
    .execute(database.pool())
    .await
    .unwrap();

    let recalled = recall_candidates(&database, profile, second).await.unwrap();
    assert_eq!(recalled.completeness, RecallCompleteness::Complete);
    assert_eq!(recalled.problem_refs, vec![problem]);
    assert!(
        !recalled.pool_signal_refs.contains(&first),
        "a Signal that already supports a Problem has left the unmerged pool"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn provider_workers_read_problem_revisions_and_release_pre_dispatch_claims() {
    let database = proof_database("comment_study_provider_worker_claims").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "study-provider-worker-note").await;
    let problem = seed_existing_problem(
        &database,
        "需要外部催促才能开始家庭作业",
        "1111111111111111111111111111111111111111111111111111111111111111",
        &[first, second],
    )
    .await;
    let resolution = prepare_problem_resolution(&database, first, vec![problem])
        .await
        .unwrap();
    let adapter = PiAdapter::configured();
    let error = run_one_problem_resolution(&database, &UnavailableModelSecrets, &adapter)
        .await
        .expect_err("the synthetic missing secret prevents provider I/O");
    assert!(matches!(
        error,
        linggan_intelligence::comment_study_resolution_worker::ResolutionWorkerError::Model(
            ModelError::SecretUnavailable
        )
    ));
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT model_invocation_ref FROM linggan_comment_study_resolution WHERE resolution_ref=$1",
        )
        .bind(resolution.resolution_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        None,
        "a failed request build must release the Resolution for a later configured worker"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_model_invocation \
             WHERE state='failed' AND result->>'failureCode'='model_secret_unavailable'",
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1,
        "the failed request build remains auditable even though the Resolution was released"
    );

    accept_problem_resolution(
        &database,
        resolution.resolution_ref,
        serde_json::json!({
            "contract":"comment-study.problem-resolution.v1",
            "candidates":[{"problemRef":problem,"dimensions":{
                "actor":"different","goalOrExpectedState":"different",
                "barrierOrUnmetNeed":"different","context":"different"
            }}]
        }),
    )
    .await
    .unwrap();
    let novel = prepare_problem_resolution(&database, second, Vec::new())
        .await
        .unwrap();
    accept_problem_resolution(
        &database,
        novel.resolution_ref,
        serde_json::json!({
            "contract":"comment-study.problem-resolution.v1",
            "candidates":[]
        }),
    )
    .await
    .unwrap();
    let pair = prepare_problem_pair(&database, first, second, test_pair_selection())
        .await
        .unwrap();
    let error = run_one_problem_pair(&database, &UnavailableModelSecrets, &adapter)
        .await
        .expect_err("the synthetic missing secret prevents provider I/O");
    assert!(matches!(
        error,
        linggan_intelligence::comment_study_pair_worker::PairWorkerError::Model(
            ModelError::SecretUnavailable
        )
    ));
    assert_eq!(
        sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT model_invocation_ref FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
        )
        .bind(pair.pair_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        None,
        "a failed request build must release the Pair for a later configured worker"
    );
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn no_existing_match_stays_deferred_until_two_independent_signals_create_one_problem() {
    let database = proof_database("comment_study_problem_pair").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    detail_with_author(
        &database,
        "study-problem-note",
        "ADHD 笔记",
        Some("creator-1"),
    )
    .await;
    let first_source = comment_with_author(
        &database,
        "study-problem-note",
        "study-problem-comment-1",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    let second_source = comment_with_author(
        &database,
        "study-problem-note",
        "study-problem-comment-2",
        "孩子每天写作业都要催，不催就不开始，我很着急。",
        Some("reader-2"),
        "2026-09-16T08:01:00Z",
    )
    .await;
    let work_ref: Uuid = sqlx::query_scalar(
        "SELECT content_public_ref FROM linggan_material_comment WHERE material_ref=$1",
    )
    .bind(first_source)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let policy_ref = seed_study_policy(&database).await;
    let first_target = seed_running_target(&database, policy_ref, work_ref, first_source).await;
    let second_target = seed_running_target(&database, policy_ref, work_ref, second_source).await;
    for target_ref in [first_target, second_target] {
        accept_target_output(
            &database,
            target_ref,
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            None,
            semantic_output("每天写作业都要催,不催就不开始"),
        )
        .await
        .unwrap();
    }
    let first_signal: Uuid = sqlx::query_scalar(
        "SELECT signal_ref FROM linggan_comment_study_signal WHERE target_ref=$1",
    )
    .bind(first_target)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let second_signal: Uuid = sqlx::query_scalar(
        "SELECT signal_ref FROM linggan_comment_study_signal WHERE target_ref=$1",
    )
    .bind(second_target)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let existing_problem = seed_existing_problem(
        &database,
        "另一类困难",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        &[first_signal, second_signal],
    )
    .await;
    let different = serde_json::json!({
        "contract":"comment-study.problem-resolution.v1",
        "candidates":[{"problemRef":existing_problem,"dimensions":{
            "actor":"different","goalOrExpectedState":"different",
            "barrierOrUnmetNeed":"different","context":"different"
        }}]
    });
    let first_resolution =
        prepare_problem_resolution(&database, first_signal, vec![existing_problem])
            .await
            .unwrap();
    let second_resolution =
        prepare_problem_resolution(&database, second_signal, vec![existing_problem])
            .await
            .unwrap();
    assert_eq!(
        accept_problem_resolution(
            &database,
            first_resolution.resolution_ref,
            different.clone()
        )
        .await
        .unwrap()
        .state,
        "deferred_novel"
    );
    assert_eq!(
        accept_problem_resolution(&database, second_resolution.resolution_ref, different)
            .await
            .unwrap()
            .state,
        "deferred_novel"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_comment_study_problem")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        1,
        "a no-match Signal has not created a Problem"
    );
    let pair = prepare_problem_pair(
        &database,
        first_signal,
        second_signal,
        test_pair_selection(),
    )
    .await
    .unwrap();
    let accepted = accept_problem_pair(
        &database,
        pair.pair_ref,
        serde_json::json!({
            "contract":"comment-study.problem-pair.v1",
            "firstSignalRef":pair.first_signal_ref,
            "secondSignalRef":pair.second_signal_ref,
            "dimensions":{
                "actor":"same","goalOrExpectedState":"same",
                "barrierOrUnmetNeed":"same","context":"same"
            },
            "proposedProblem":{
                "title":"作业自主启动困难",
                "definition":"孩子在家庭作业中存在自主启动困难",
                "stableIdentity":{"actor":"孩子","barrier":"需要外部催促"},
                "includeCriteria":["需要持续外部催促才能开始家庭作业"],
                "excludeCriteria":["仅一次忘记作业"]
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(accepted.state, "approved");
    assert!(accepted.problem_ref.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_problem_membership WHERE problem_ref=$1",
        )
        .bind(accepted.problem_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        2
    );
    let problems = read_problems(&database, &domain_read_query(None))
        .await
        .unwrap();
    let created = problems["problems"]
        .as_array()
        .unwrap()
        .iter()
        .find(|problem| problem["problemRef"] == accepted.problem_ref.unwrap().to_string())
        .expect("the accepted Problem remains readable through its current revision");
    assert_eq!(created["stableIdentity"]["actor"], "孩子");
    assert_eq!(
        created["includeCriteria"],
        serde_json::json!(["需要持续外部催促才能开始家庭作业"])
    );
    assert_eq!(
        created["excludeCriteria"],
        serde_json::json!(["仅一次忘记作业"])
    );
}

fn semantic_output(evidence: &str) -> serde_json::Value {
    serde_json::json!({"contract":"comment-study.semantic.v1","signals":[{
        "kind":"problem","proposition":"孩子在家庭作业中存在自主启动困难。",
        "evidence":evidence,
        "problemFrame":{
            "actor":{"value":"评论者","basis":"我很着急"},
            "goalOrExpectedState":{"value":"孩子自主开始作业","basis":"不催就不开始"},
            "barrierOrUnmetNeed":{"value":"需要外部催促","basis":"都要催"},
            "context":{"value":"家庭作业","basis":"写作业"}
        }
    }]})
}

/// A run's own state and whether it carries a finish time. The schema ties the two together, so a
/// run reported as closed while `finished_at` stays null would be a lie the CHECK cannot catch on
/// a row nobody updates.
async fn run_state(database: &linggan_storage_postgres::Database, run_ref: Uuid) -> (String, bool) {
    sqlx::query_as::<_, (String, bool)>(
        "SELECT state,finished_at IS NOT NULL FROM linggan_comment_study_run WHERE run_ref=$1",
    )
    .bind(run_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

/// Seeds a Problem the way creation does: identity row plus its immutable first revision. A
/// Problem without a revision has no core for recall to encode, so tests must not create half of
/// one.
/// Registers a qualified profile the way a machine probe would, without running a model.
async fn seed_embedding_profile(database: &linggan_storage_postgres::Database) -> Uuid {
    linggan_intelligence::comment_study_embedding::register_embedding_profile(
        database,
        "test-revision",
        serde_json::json!({"backend":"test","dtype":"test"}),
        Some(serde_json::json!({"probe":"synthetic","dimension":512})),
    )
    .await
    .unwrap()
}

fn test_pair_selection() -> PairSelection {
    PairSelection {
        profile_ref: Uuid::new_v4(),
        recall_rank: 1,
        admissible_rank: 1,
    }
}

/// A unit vector `angle` radians away from the reference direction, so a test can state "these two
/// are near" or "these two are far" without depending on a model.
fn unit_vector(angle: f64) -> String {
    let mut values = vec![0.0_f64; 512];
    values[0] = angle.cos();
    values[1] = angle.sin();
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    )
}

async fn seed_vector(
    database: &linggan_storage_postgres::Database,
    profile_ref: Uuid,
    canonical_hash: &str,
    angle: f64,
) {
    sqlx::query(
        "INSERT INTO linggan_comment_study_embedding_cache(profile_ref,canonical_hash,embedding) \
         VALUES($1,$2,$3::text::public.vector) ON CONFLICT DO NOTHING",
    )
    .bind(profile_ref)
    .bind(canonical_hash)
    .bind(unit_vector(angle))
    .execute(database.pool())
    .await
    .unwrap();
}

/// Reads the canonical hash a Signal actually carries, so a test never guesses at the template.
async fn signal_canonical_hash(
    database: &linggan_storage_postgres::Database,
    signal_ref: Uuid,
) -> String {
    sqlx::query_scalar(
        "SELECT canonical_hash FROM linggan_comment_study_signal WHERE signal_ref=$1",
    )
    .bind(signal_ref)
    .fetch_one(database.pool())
    .await
    .unwrap()
}

async fn seed_existing_problem(
    database: &linggan_storage_postgres::Database,
    definition: &str,
    definition_hash: &str,
    seed_signal_refs: &[Uuid],
) -> Uuid {
    let problem_ref = Uuid::new_v4();
    let revision_ref = Uuid::new_v4();
    let domain_ref = PROOF_DOMAIN_REF;
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem(problem_ref,domain_ref,state) \
         VALUES($1,$2,'active')",
    )
    .bind(problem_ref)
    .bind(domain_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_problem_revision( \
           revision_ref,problem_ref,domain_ref,identity_version,title,definition,core_frame, \
           inclusions,exclusions,seed_signal_refs,canonical_text,canonical_hash,definition_hash,reason \
         ) VALUES($1,$2,$3,1,$4,$5,'{\"actor\":\"test\"}'::jsonb,'[\"test inclusion\"]'::jsonb,'[]'::jsonb,$6,$7,$8,$9,'test_seed')",
    )
    .bind(revision_ref)
    .bind(problem_ref)
    .bind(domain_ref)
    .bind(definition)
    .bind(definition)
    .bind(seed_signal_refs)
    .bind(format!("表达：{definition}\n主体：未明确\n目标：未明确\n障碍：未明确\n场景：未明确"))
    .bind("dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd")
    .bind(definition_hash)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE linggan_comment_study_problem SET current_revision_ref=$2 WHERE problem_ref=$1",
    )
    .bind(problem_ref)
    .bind(revision_ref)
    .execute(database.pool())
    .await
    .unwrap();
    problem_ref
}

async fn seed_study_policy(database: &linggan_storage_postgres::Database) -> Uuid {
    let policy_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_policy( \
           policy_ref,domain_ref,contract,comment_budget,context_character_budget \
         ) VALUES($1,$2,'comment-study.v1',100,12000)",
    )
    .bind(policy_ref)
    .bind(PROOF_DOMAIN_REF)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_active_policy(domain_ref,policy_ref) VALUES($1,$2)",
    )
    .bind(PROOF_DOMAIN_REF)
    .bind(policy_ref)
    .execute(database.pool())
    .await
    .unwrap();
    policy_ref
}

async fn seed_study_model_config(
    database: &linggan_storage_postgres::Database,
    policy_ref: Uuid,
) -> Uuid {
    seed_study_model_config_with_input_limit(database, policy_ref, 16000).await
}

async fn seed_study_model_config_with_input_limit(
    database: &linggan_storage_postgres::Database,
    policy_ref: Uuid,
    input_token_limit: i32,
) -> Uuid {
    let connection_ref = Uuid::new_v4();
    let version_ref = Uuid::new_v4();
    let model_ref = Uuid::new_v4();
    let config_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_model_connection(connection_ref,enabled,revision) VALUES($1,true,1)",
    )
    .bind(connection_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_connection_version( \
           version_ref,connection_ref,revision,name,api,base_url,local_endpoint,secret_ref \
         ) VALUES($1,$2,1,'Synthetic comment-study','openai-completions', \
                   'http://127.0.0.1:18080',true,$3)",
    )
    .bind(version_ref)
    .bind(connection_ref)
    .bind(Uuid::new_v4())
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_entry(model_ref,connection_version_ref,model_id,origin) \
         VALUES($1,$2,'synthetic-comment-study','manual')",
    )
    .bind(model_ref)
    .bind(version_ref)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_model_config( \
           config_ref,model_ref,input_token_limit,output_token_limit,timeout_seconds,max_attempts \
         ) VALUES($1,$2,$3,2000,30,1)",
    )
    .bind(config_ref)
    .bind(model_ref)
    .bind(input_token_limit)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE linggan_comment_study_policy SET model_config_ref=$2 WHERE policy_ref=$1")
        .bind(policy_ref)
        .bind(config_ref)
        .execute(database.pool())
        .await
        .unwrap();
    config_ref
}

async fn seed_running_target(
    database: &linggan_storage_postgres::Database,
    policy_ref: Uuid,
    content_public_ref: Uuid,
    source_ref: Uuid,
) -> Uuid {
    let run_ref = Uuid::new_v4();
    let hash = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    sqlx::query(
        "INSERT INTO linggan_comment_study_run( \
           run_ref,policy_ref,as_of,state,selection_manifest,selection_hash \
         ) VALUES($1,$2,scope_001_now(),'running','{}',$3)",
    )
    .bind(run_ref)
    .bind(policy_ref)
    .bind(hash)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_work( \
           run_ref,content_public_ref,domain_ref,observation_role,selection_reason,context_state,context_manifest,context_hash \
         ) VALUES($1,$2,$3,'primary','user_selected','ready',jsonb_build_object('workRef',$2::text),$4)",
    )
    .bind(run_ref)
    .bind(content_public_ref)
    .bind(PROOF_DOMAIN_REF)
    .bind(hash)
    .execute(database.pool())
    .await
    .unwrap();
    let target_ref = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO linggan_comment_study_target( \
           target_ref,run_ref,content_public_ref,source_ref,research_text,research_sha256, \
           dependency_state,state,input_manifest,input_hash \
         ) VALUES($1,$2,$3,$4,'synthetic research text',$5,'self_contained','running','{}',$5)",
    )
    .bind(target_ref)
    .bind(run_ref)
    .bind(content_public_ref)
    .bind(source_ref)
    .bind(hash)
    .execute(database.pool())
    .await
    .unwrap();
    target_ref
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn reset_replaces_only_comment_research_derivations_and_preserves_raw_evidence() {
    let database = proof_database("comment_study_rebuild_reset").await;
    detail_with_author(
        &database,
        "study-reset-note",
        "SYNTHETIC study reset note",
        Some("creator-1"),
    )
    .await;
    comment_with_author(
        &database,
        "study-reset-note",
        "study-reset-comment",
        "我想知道怎样让孩子愿意写作业",
        Some("reader-1"),
        "2026-09-16T08:00:00Z",
    )
    .await;
    sqlx::query(
        "INSERT INTO linggan_comment_research_restriction( \
           content_public_ref,comment_external_id,reason \
         ) SELECT content_public_ref,comment_external_id,'preserve qualification fact' \
           FROM linggan_material_comment WHERE comment_external_id='study-reset-comment'",
    )
    .execute(database.pool())
    .await
    .unwrap();
    let content_before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_content")
        .fetch_one(database.pool())
        .await
        .unwrap();
    let comment_before: i64 = sqlx::query_scalar("SELECT count(*) FROM linggan_material_comment")
        .fetch_one(database.pool())
        .await
        .unwrap();

    let mut transaction = database.pool().begin().await.unwrap();
    sqlx::raw_sql(RESET_SQL)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO linggan_comment_study_reset_receipt( \
           receipt_ref,reset_scope,old_relation_counts,preserved_relation_counts,requested_by \
         ) SELECT gen_random_uuid(),'local_comment_study_derived_only',old_relation_counts, \
           jsonb_build_object('content',(SELECT count(*) FROM linggan_material_content), \
                              'comment',(SELECT count(*) FROM linggan_material_comment)), \
           'isolated-proof' FROM comment_study_reset_counts",
    )
    .execute(&mut *transaction)
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let old_relation: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_research_derivation')::text")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert!(old_relation.is_none());
    let old_restriction: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_research_restriction')::text")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert!(old_restriction.is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_material_comment_restriction",)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        1
    );
    let new_relation: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('linggan_comment_study_target')::text")
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(
        new_relation.as_deref(),
        Some("linggan_comment_study_target")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_material_content")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        content_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM linggan_material_comment")
            .fetch_one(database.pool())
            .await
            .unwrap(),
        comment_before
    );
    let receipt = sqlx::query(
        "SELECT reset_scope,old_relation_counts,preserved_relation_counts \
         FROM linggan_comment_study_reset_receipt",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        receipt.get::<String, _>("reset_scope"),
        "local_comment_study_derived_only"
    );
    assert_eq!(
        receipt.get::<serde_json::Value, _>("preserved_relation_counts")["content"],
        content_before
    );
    assert_eq!(
        receipt.get::<serde_json::Value, _>("preserved_relation_counts")["comment"],
        comment_before
    );
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL proof database"]
async fn a_pair_partner_is_the_nearest_admissible_signal_not_the_earliest_one() {
    let database = proof_database("comment_study_pair_partner").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    // The seeker plus three pool candidates, arranged so that arrival order, raw nearness and the
    // rule each pick a different partner: `far` arrives first among the candidates, `twin` is the
    // nearest of all but shares the seeker's account, and only `near` is both admissible and close.
    let (seeker, twin) = two_eligible_signals_from(
        &database,
        "pair-note-a",
        ["reader-1", "reader-1"],
        ["需要外部催促", "自己不愿动笔"],
    )
    .await;
    let (far, near) = two_eligible_signals_from(
        &database,
        "pair-note-b",
        ["reader-2", "reader-3"],
        ["拖到很晚才开始", "写一半就走神"],
    )
    .await;
    record_order(&database, seeker, "2026-09-16T08:00:00Z").await;
    record_order(&database, far, "2026-09-16T08:01:00Z").await;
    record_order(&database, near, "2026-09-16T08:02:00Z").await;
    record_order(&database, twin, "2026-09-16T08:03:00Z").await;
    let profile = seed_embedding_profile(&database).await;
    let hash = |signal| signal_canonical_hash(&database, signal);
    seed_vector(&database, profile, &hash(seeker).await, 0.0).await;
    seed_vector(&database, profile, &hash(twin).await, 0.01).await;
    seed_vector(&database, profile, &hash(near).await, 0.1).await;
    seed_vector(&database, profile, &hash(far).await, 1.3).await;
    for _ in 0..4 {
        assert!(advance_next_problem_resolution(&database).await.unwrap());
    }
    for signal in [seeker, twin, near, far] {
        assert_eq!(
            resolution_state_for(&database, signal).await,
            Some(("deferred_novel".to_owned(), None)),
            "an empty searchable catalogue leaves every Signal novel"
        );
    }

    assert!(advance_next_problem_pair(&database).await.unwrap());
    let paired: Vec<Uuid> = sqlx::query_scalar(
        "SELECT unnest(ARRAY[first_signal_ref,second_signal_ref]) \
         FROM linggan_comment_study_problem_pair",
    )
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert!(
        paired.contains(&seeker) && paired.contains(&near),
        "the partner is the nearest admissible Signal; arrival order would have chosen the far one \
         and raw nearness the same-account one: {paired:?}"
    );
    assert!(
        !paired.contains(&twin),
        "a second reading from the same account is not independent support, however near it sits"
    );
    assert!(
        !paired.contains(&far),
        "arriving early is not a reason to spend a model call on a distant Signal"
    );
    let selection: serde_json::Value = sqlx::query_scalar(
        "SELECT pair_manifest->'selection' FROM linggan_comment_study_problem_pair",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(
        selection["contract"],
        "comment-study.problem-pair-selection.v1"
    );
    assert_eq!(selection["method"], "nearest_admissible");
    assert_eq!(selection["embeddingProfileRef"], profile.to_string());
    assert_eq!(selection["admissibleRank"], 1);
    assert!(
        selection["recallRank"]
            .as_i64()
            .is_some_and(|rank| rank >= 1),
        "the persisted rank is the actual ordered recall position, not a similarity threshold"
    );
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL proof database"]
async fn one_primary_comparison_does_not_expand_after_a_valid_non_create_outcome() {
    let database = proof_database("comment_study_pair_retry").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    // Three accounts, each with a second same-account reading that can never be admitted. The
    // seeker's nearest admissible partner is `second`; `third` sits further out.
    let (first, first_echo) = two_eligible_signals_from(
        &database,
        "retry-note-a",
        ["reader-1", "reader-1"],
        ["需要外部催促", "自己不愿动笔"],
    )
    .await;
    let (second, second_echo) = two_eligible_signals_from(
        &database,
        "retry-note-b",
        ["reader-2", "reader-2"],
        ["拖到很晚才开始", "写一半就走神"],
    )
    .await;
    let (third, third_echo) = two_eligible_signals_from(
        &database,
        "retry-note-c",
        ["reader-3", "reader-3"],
        ["坐下来也发呆", "要陪着才肯写"],
    )
    .await;
    let profile = seed_embedding_profile(&database).await;
    let hash = |signal| signal_canonical_hash(&database, signal);
    for (index, signal) in [first, second, third, first_echo, second_echo, third_echo]
        .into_iter()
        .enumerate()
    {
        record_order(&database, signal, &format!("2026-09-16T08:0{index}:00Z")).await;
    }
    seed_vector(&database, profile, &hash(first).await, 0.0).await;
    seed_vector(&database, profile, &hash(second).await, 0.05).await;
    seed_vector(&database, profile, &hash(third).await, 0.2).await;
    seed_vector(&database, profile, &hash(first_echo).await, 2.0).await;
    seed_vector(&database, profile, &hash(second_echo).await, 2.1).await;
    seed_vector(&database, profile, &hash(third_echo).await, 2.2).await;
    for _ in 0..6 {
        assert!(advance_next_problem_resolution(&database).await.unwrap());
    }

    assert!(advance_next_problem_pair(&database).await.unwrap());
    let pair_ref: Uuid = sqlx::query_scalar(
        "SELECT pair_ref FROM linggan_comment_study_problem_pair WHERE state='pending'",
    )
    .fetch_one(database.pool())
    .await
    .unwrap();
    // The model reports a conflicting dimension: these two are not the same Problem.
    let receipt = accept_problem_pair(
        &database,
        pair_ref,
        serde_json::json!({
            "contract":"comment-study.problem-pair.v1",
            "firstSignalRef":first.min(second),
            "secondSignalRef":first.max(second),
            "dimensions":{
                "actor":"same","goalOrExpectedState":"same",
                "barrierOrUnmetNeed":"different","context":"same"
            }
        }),
    )
    .await
    .unwrap();
    assert_eq!(receipt.state, "rejected");
    assert_eq!(receipt.decision_reason, "not_same_problem");
    assert_eq!(
        receipt.problem_ref, None,
        "a conflicting dimension creates nothing"
    );

    // The two Signals remain deferred novel because no Problem was created, but their one automatic
    // primary comparison is consumed. The worker must not silently turn "not the same" into a
    // near-exhaustive search of the remaining pool.
    assert_eq!(
        resolution_state_for(&database, first).await,
        Some(("deferred_novel".to_owned(), None))
    );
    assert_eq!(
        resolution_state_for(&database, second).await,
        Some(("deferred_novel".to_owned(), None))
    );
    assert!(
        advance_next_problem_pair(&database).await.unwrap(),
        "unpaired Signals may still receive their own primary comparison"
    );
    assert!(advance_next_problem_pair(&database).await.unwrap());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT max(comparison_count) FROM ( \
               SELECT signal_ref,count(*) AS comparison_count FROM ( \
                 SELECT first_signal_ref AS signal_ref FROM linggan_comment_study_problem_pair \
                 UNION ALL \
                 SELECT second_signal_ref AS signal_ref FROM linggan_comment_study_problem_pair \
               ) compared GROUP BY signal_ref \
             ) degrees",
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1,
        "every Signal may receive one primary comparison, never a second automatic partner"
    );
    assert!(
        !advance_next_problem_pair(&database).await.unwrap(),
        "once every admissible Signal has its primary comparison, the automatic worker stops"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT pair_manifest->'decision'->>'code' \
             FROM linggan_comment_study_problem_pair WHERE pair_ref=$1",
        )
        .bind(pair_ref)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "not_same_problem"
    );
    let run_ref: Uuid = sqlx::query_scalar(
        "SELECT target.run_ref FROM linggan_comment_study_signal signal \
         JOIN linggan_comment_study_target target USING(target_ref) \
         WHERE signal.signal_ref=$1",
    )
    .bind(first)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let signals = read_signals(&database, &domain_read_query(Some(run_ref)))
        .await
        .unwrap();
    let projected = signals["signals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|signal| signal["signalRef"] == first.to_string())
        .unwrap();
    assert_eq!(
        projected["pairOutcomes"][0]["decisionReason"],
        "not_same_problem"
    );
    assert_eq!(
        projected["pairOutcomes"][0]["selection"]["admissibleRank"],
        1
    );
    assert!(
        projected["pairOutcomes"][0]["selection"]
            .get("embeddingProfileRef")
            .is_none(),
        "the read projection exposes only the selection facts a reviewer can interpret"
    );
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL proof database"]
async fn a_tick_encodes_a_waiting_signal_before_it_tries_to_recall_against_it() {
    let database = proof_database("comment_study_tick_encodes").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "tick-encode-note").await;
    seed_embedding_profile(&database).await;
    let vectors = |database: &linggan_storage_postgres::Database| {
        let pool = database.pool().clone();
        async move {
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM linggan_comment_study_embedding_cache",
            )
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    assert_eq!(vectors(&database).await, 0, "nothing is encoded yet");

    let adapter = PiAdapter::configured_with_test_embedding(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_embedding_runtime.sh"),
    );
    assert!(
        run_model_work_once(&database, &SyntheticModelSecrets, &adapter)
            .await
            .unwrap(),
        "the tick has encoding to do"
    );

    assert!(
        vectors(&database).await > 0,
        "the waiting Signal was encoded through the real adapter boundary"
    );
    // The order is what the outcome turns on, so the assertion is about the outcome rather than
    // about which branch ran. Recall reached before encoding answers `retrieval_incomplete`, and
    // that answer is final: the Signal stays stuck behind a condition the same tick could have
    // cleared for free, with no provider call involved.
    // Stops as soon as both Signals have been judged. Carrying on would reach the pair worker,
    // which calls the provider — a different boundary, stubbed by a different script, and not
    // what this proof is about.
    for _ in 0..20 {
        if resolution_state_for(&database, first).await.is_some()
            && resolution_state_for(&database, second).await.is_some()
        {
            break;
        }
        assert!(
            run_model_work_once(&database, &SyntheticModelSecrets, &adapter)
                .await
                .unwrap(),
            "the tick still has local work to do"
        );
    }
    for signal in [first, second] {
        assert_eq!(
            resolution_state_for(&database, signal).await,
            Some(("deferred_novel".to_owned(), None)),
            "an encoded Signal against an empty catalogue is novel, not unsearchable"
        );
    }
}

#[tokio::test]
#[ignore = "isolated PostgreSQL proof"]
async fn embedding_work_covers_an_active_problem_fixed_core() {
    let database = proof_database("comment_study_encode_problem_core").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let (first, second) = two_eligible_signals(&database, "encode-problem-core-note").await;
    let profile = seed_embedding_profile(&database).await;
    let problem = seed_existing_problem(
        &database,
        "固定问题核心",
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        &[first, second],
    )
    .await;
    let core_hash: String = sqlx::query_scalar(
        "SELECT revision.canonical_hash FROM linggan_comment_study_problem problem \
         JOIN linggan_comment_study_problem_revision revision \
           ON revision.revision_ref=problem.current_revision_ref \
         WHERE problem.problem_ref=$1",
    )
    .bind(problem)
    .fetch_one(database.pool())
    .await
    .unwrap();
    let adapter = PiAdapter::configured_with_test_embedding(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/support/comment_study_embedding_runtime.sh"),
    );
    assert!(matches!(
        embed_pending_signals(&database, &adapter).await.unwrap(),
        EmbeddingOutcome::Encoded { .. }
    ));
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM linggan_comment_study_embedding_cache \
             WHERE profile_ref=$1 AND canonical_hash=$2)",
        )
        .bind(profile)
        .bind(core_hash)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        "an active Problem core without an embedding would permanently make recall incomplete"
    );
}

fn embedding_runtime(script: &str) -> PiAdapter {
    PiAdapter::configured_with_test_embedding(
        std::path::PathBuf::from("/bin/sh"),
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(script),
    )
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL proof database"]
async fn a_runtime_that_encodes_everything_onto_one_point_is_refused_qualification() {
    let database = proof_database("comment_study_probe_collapsed").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let outcome = probe_and_register_embedding_profile(
        &database,
        &embedding_runtime("tests/support/comment_study_embedding_runtime_collapsed.sh"),
    )
    .await
    .unwrap();

    // Protocol-valid, deterministic, correctly shaped, unit norm — and worthless. Every structural
    // check passes; only asking whether different texts land in different places catches it.
    match outcome {
        ProbeOutcome::Refused { failing_check, .. } => {
            assert_eq!(failing_check, "different_texts_encoded_identically");
        }
        other => panic!("a collapsed runtime must not be qualified: {other:?}"),
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM linggan_comment_study_embedding_profile"
        )
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0,
        "a refused probe registers nothing: an unusable space must not become the one every \
         Problem is compared in"
    );
    assert_eq!(
        active_profile(&database).await.unwrap(),
        None,
        "and recall still has no catalogue to search"
    );
}

#[tokio::test]
#[ignore = "requires the local PostgreSQL proof database"]
async fn a_probed_runtime_becomes_the_qualified_profile_with_its_own_evidence() {
    let database = proof_database("comment_study_probe_qualified").await;
    sqlx::raw_sql(STUDY_SCHEMA_SQL)
        .execute(database.pool())
        .await
        .unwrap();
    let adapter = embedding_runtime("tests/support/comment_study_embedding_runtime.sh");
    let outcome = probe_and_register_embedding_profile(&database, &adapter)
        .await
        .unwrap();
    let ProbeOutcome::Qualified {
        profile_ref,
        evidence,
    } = outcome
    else {
        panic!("a runtime that encodes consistently and distinctly qualifies: {outcome:?}")
    };
    assert_eq!(active_profile(&database).await.unwrap(), Some(profile_ref));
    // The record has to say what it ran on and what it cost, and has to name what it could not
    // measure rather than leave the gap to be read as a zero.
    for field in ["backend", "coldStartMs", "peakRssBytes", "repeatCosine"] {
        assert!(
            evidence.get(field).is_some_and(|value| !value.is_null()),
            "the qualification evidence carries {field}: {evidence}"
        );
    }
    assert_eq!(
        evidence
            .get("notCaptured")
            .and_then(|value| value.as_array()),
        Some(&vec![
            serde_json::json!("systemMemoryPressure"),
            serde_json::json!("swapActivity")
        ]),
        "what the runtime cannot measure is named, not omitted"
    );

    // Probing the same runtime again is the same profile, not a second one. A *fresh* adapter,
    // because the handshake is read once per process: reprobing without a restart would compare
    // a cached line with itself and could not tell a measurement in the identity from a fact.
    // Measurements change across restarts; if they reached the identity, every probe would orphan
    // the vectors already encoded under the profile before it.
    let restarted = embedding_runtime("tests/support/comment_study_embedding_runtime.sh");
    let repeated = probe_and_register_embedding_profile(&database, &restarted)
        .await
        .unwrap();
    let ProbeOutcome::Qualified {
        profile_ref: repeated_ref,
        ..
    } = repeated
    else {
        panic!("the second probe also qualifies")
    };
    assert_eq!(repeated_ref, profile_ref, "one runtime, one profile");
}
