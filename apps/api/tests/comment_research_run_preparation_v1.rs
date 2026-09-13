use std::sync::Arc;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use linggan_api::comment_research_router_v0;
use linggan_contracts::CapturePackageV0;
use linggan_storage_postgres::CommentFactStore;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const FIXTURE: &str = include_str!("../../../fixtures/xhs/comment-evidence-set-v1.json");
const WORKSPACE: &str = "comment-research-run-preparation-v1-proof";

/// This proof invokes the real Axum POST route against one disposable
/// PostgreSQL database. It has no worker, queue, provider, model, vector, or
/// external-source dependency; the database assertion below uses a synthetic
/// validated `no_signal` only to prove selector reuse behavior.
#[tokio::test]
#[ignore = "requires scripts/prove-comment-research-run-preparation-v1.sh"]
async fn proves_confirmed_local_comment_research_run_preparation_v1() {
    let database_url = std::env::var("LINGGAN_COMMENT_RESEARCH_RUN_PREPARATION_TEST_DATABASE_URL")
        .expect("proof script must provide an isolated PostgreSQL URL");
    let mut store = CommentFactStore::connect(&database_url)
        .await
        .expect("isolated PostgreSQL connects");
    store
        .apply_comment_fact_storage_v0_migration()
        .await
        .expect("fact migration applies");
    store
        .apply_comment_context_storage_v0_migration()
        .await
        .expect("context migration applies");
    store
        .apply_comment_derivation_v1_migration()
        .await
        .expect("derivation migration applies");
    store
        .apply_comment_research_execution_foundation_v1_migration()
        .await
        .expect("execution foundation migration applies");
    store
        .apply_comment_research_run_preparation_v1_migration()
        .await
        .expect("preparation extension migration applies");
    let inspector = connect_inspector(&database_url).await;

    let first_a = admit_fixture_comment(
        &mut store,
        "source-a-first",
        "source-a",
        "source-a-comment-1",
        "孩子每天开始写作业都很困难，现在需要具体方法",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        "source-a-equivalent",
        "source-a",
        "source-a-comment-2",
        "孩子每天开始写作业都很困难，现在需要具体方法",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        "source-b-first",
        "source-b",
        "source-b-comment-1",
        "十岁孩子现在开始训练还来得及吗",
    )
    .await;
    admit_fixture_comment(
        &mut store,
        "source-c-needs-context",
        "source-c",
        "source-c-comment-1",
        "同问",
    )
    .await;

    let shared_store = Arc::new(store);
    let app = comment_research_router_v0(shared_store.clone());
    let page = get_text(&app, "/comment-research/voices").await;
    assert_eq!(page.0, StatusCode::OK);
    assert_confirmation_page_contract(&page.1);
    let live_preview = get_json(
        &app,
        "/api/v0/comment-research/plan-preview?workspace_id=comment-research-run-preparation-v1-proof&scope=available&limit=4",
    )
    .await;
    assert_eq!(live_preview.0, StatusCode::OK);

    let prepared = post_json(
        &app,
        "/api/v0/comment-research/runs",
        json!({
            "workspace_id": WORKSPACE,
            "scope": "available",
            "limit": 4,
        }),
    )
    .await;
    assert_eq!(prepared.0, StatusCode::OK, "{:#}", prepared.1);
    assert_eq!(prepared.1["run_state"], "prepared_for_execution");
    assert_eq!(prepared.1["scope_refreshed"], false);
    assert_eq!(prepared.1["frozen_item_total"], 3);
    assert_eq!(prepared.1["prepared_for_execution_total"], 2);
    assert_eq!(prepared.1["blocked_needs_context_total"], 1);
    assert_eq!(prepared.1["deduplicated_semantic_input_total"], 1);
    assert_eq!(
        prepared.1["source_distribution"].as_array().map(Vec::len),
        Some(3)
    );
    assert_eq!(
        prepared.1["execution_note"],
        "本次只冻结输入并创建待执行研究；当前未配置执行器，未调用模型。"
    );
    assert_no_forbidden_run_response_field(&prepared.1);

    let run_id = Uuid::parse_str(
        prepared.1["run_ref"]
            .as_str()
            .expect("run ref is public UUID"),
    )
    .expect("run ref parses");
    let frozen = inspector
        .query(
            "SELECT item.cleaned_research_text, item.frozen_context_pack_text, \
                    item.context_pack_integrity_sha256, item.research_fingerprint, \
                    item.context_sufficiency_state, item.execution_state, \
                    item.initial_failure_code, event.execution_state, event.failure_code \
               FROM comment_research_run_item_v1 AS item \
               JOIN comment_research_run_item_event_v1 AS event ON event.run_item_id = item.id \
              WHERE item.run_id = $1 \
              ORDER BY item.created_at ASC, item.id ASC",
            &[&run_id],
        )
        .await
        .expect("frozen run items query");
    assert_eq!(frozen.len(), 3);
    assert!(frozen.iter().any(|row| {
        row.get::<_, String>(4) == "insufficient_needs_context"
            && row.get::<_, String>(5) == "blocked"
            && row.get::<_, Option<String>>(6).as_deref() == Some("needs_context_insufficient")
            && row.get::<_, String>(7) == "blocked"
            && row.get::<_, Option<String>>(8).as_deref() == Some("needs_context_insufficient")
    }));
    assert!(frozen.iter().any(|row| {
        row.get::<_, String>(4) == "sufficient"
            && row.get::<_, String>(5) == "prepared"
            && row.get::<_, Option<String>>(6).is_none()
            && row.get::<_, String>(7) == "prepared"
            && row.get::<_, Option<String>>(8).is_none()
    }));
    assert!(frozen.iter().all(|row| {
        row.get::<_, String>(1)
            .starts_with("COMMENT_RESEARCH_INPUT_SNAPSHOT\n")
            && row.get::<_, String>(2).len() == 64
            && row.get::<_, String>(3).len() == 64
    }));
    let fingerprints = frozen
        .iter()
        .map(|row| row.get::<_, String>(3))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        fingerprints.len(),
        frozen.len(),
        "equivalent inputs freeze once per run"
    );
    let frozen_sources = inspector
        .query(
            "SELECT observation.note_id \
               FROM comment_research_run_item_v1 AS item \
               JOIN comment_observation_v0 AS observation ON observation.id = item.comment_observation_id \
              WHERE item.run_id = $1 \
              ORDER BY item.created_at ASC, item.id ASC",
            &[&run_id],
        )
        .await
        .expect("frozen source order query")
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(
        frozen_sources,
        vec!["source-a", "source-b", "source-c"],
        "creation recomputes source rotation before freezing"
    );

    // Browser-supplied source IDs are neither selection authority nor a
    // tolerated ignored field: strict JSON decoding rejects them before write.
    let counts_before_rejected_request = run_counts(&inspector).await;
    let rejected = post_json(
        &app,
        "/api/v0/comment-research/runs",
        json!({
            "workspace_id": WORKSPACE,
            "scope": "ready",
            "limit": 1,
            "evidence_ids": [first_a.comments_evidence.id.to_string()],
        }),
    )
    .await;
    assert_eq!(rejected.0, StatusCode::BAD_REQUEST);
    assert_eq!(rejected.1["error"]["code"], "invalid_request");
    assert_eq!(run_counts(&inspector).await, counts_before_rejected_request);

    // Existing User Voices and the live preview stay read-only and do not leak
    // stored input, fingerprint, executor, or provider state.
    let counts_before_reads = run_counts(&inspector).await;
    let voices = get_json(
        &app,
        "/api/v0/comment-research/voices?workspace_id=comment-research-run-preparation-v1-proof&limit=25&offset=0&filter=available",
    )
    .await;
    assert_eq!(voices.0, StatusCode::OK);
    assert!(
        voices.1["voices"]
            .as_array()
            .expect("voice array")
            .iter()
            .all(|voice| {
                voice.get("research_fingerprint").is_none()
                    && voice.get("frozen_context_pack_text").is_none()
                    && voice.get("model_strategy_id").is_none()
            })
    );
    let repeated_preview = get_json(
        &app,
        "/api/v0/comment-research/plan-preview?workspace_id=comment-research-run-preparation-v1-proof&scope=available&limit=4",
    )
    .await;
    assert_eq!(repeated_preview.0, StatusCode::OK);
    assert_eq!(run_counts(&inspector).await, counts_before_reads);

    // A body advance invalidates the old live preview summary. The POST still
    // selects itself inside its transaction, then reports the refresh instead
    // of accepting a stale browser candidate list.
    drop(app);
    let mut store = match Arc::try_unwrap(shared_store) {
        Ok(store) => store,
        Err(_) => panic!("router releases the store after drop"),
    };
    let advanced_a = admit_fixture_comment(
        &mut store,
        "source-a-first-advanced",
        "source-a",
        "source-a-comment-1",
        "孩子每天开始写作业都很困难，想知道可执行的训练步骤",
    )
    .await;
    let app = comment_research_router_v0(Arc::new(store));
    let refreshed = post_json(
        &app,
        "/api/v0/comment-research/runs",
        json!({
            "workspace_id": WORKSPACE,
            "scope": "available",
            "limit": 4,
            "preview": browser_preview_summary(&repeated_preview.1),
        }),
    )
    .await;
    assert_eq!(refreshed.0, StatusCode::OK);
    assert_eq!(refreshed.1["scope_refreshed"], true);
    assert!(
        refreshed.1["source_distribution"]
            .as_array()
            .expect("final source distribution")
            .iter()
            .any(|source| source["source_note_id"] == "source-a"),
        "final source distribution is returned without Evidence IDs"
    );
    let old_locator_in_current = inspector
        .query_opt(
            "SELECT 1 FROM comment_current_v0 AS current_projection \
               JOIN comment_observation_v0 AS observation \
                 ON observation.id = current_projection.current_observation_id \
              WHERE current_projection.workspace_id = $1 \
                AND observation.source_evidence_id = $2",
            &[&WORKSPACE, &first_a.comments_evidence.id],
        )
        .await
        .expect("old current locator query");
    assert!(old_locator_in_current.is_none());
    assert_ne!(
        advanced_a.comments_evidence.id,
        first_a.comments_evidence.id
    );

    // A valid conclusion excludes its exact current derivation/fingerprint;
    // incomplete historical RunItems are never used as a completion proxy.
    let reusable = inspector
        .query_one(
            "SELECT item.id, item.comment_observation_id, item.comment_derivation_id, item.research_fingerprint \
               FROM comment_research_run_item_v1 AS item \
               JOIN comment_observation_v0 AS observation ON observation.id = item.comment_observation_id \
              WHERE item.run_id = $1 AND item.execution_state = 'prepared' \
                AND observation.note_id = 'source-b' \
              ORDER BY item.created_at ASC, item.id ASC LIMIT 1",
            &[&run_id],
        )
        .await
        .expect("prepared item reads");
    inspector
        .execute(
            "INSERT INTO comment_analysis_v1 \
             (id, run_item_id, comment_observation_id, comment_derivation_id, research_fingerprint, conclusion_state, structured_output) \
             VALUES ($1, $2, $3, $4, $5, 'no_signal', '{\"outcome\":\"no_signal\"}'::jsonb)",
            &[
                &Uuid::new_v4(),
                &reusable.get::<_, Uuid>(0),
                &reusable.get::<_, Uuid>(1),
                &reusable.get::<_, Uuid>(2),
                &reusable.get::<_, String>(3),
            ],
        )
        .await
        .expect("validated no-signal seed inserts");
    let after_conclusion = post_json(
        &app,
        "/api/v0/comment-research/runs",
        json!({ "workspace_id": WORKSPACE, "scope": "available", "limit": 4 }),
    )
    .await;
    assert_eq!(after_conclusion.0, StatusCode::OK);
    assert_eq!(after_conclusion.1["excluded_existing_conclusion_total"], 1);
}

async fn admit_fixture_comment(
    store: &mut CommentFactStore,
    suffix: &str,
    note_id: &str,
    comment_id: &str,
    text: &str,
) -> linggan_storage_postgres::CommentFactAdmissionOutcomeV0 {
    let fixture: Value = serde_json::from_str(FIXTURE).expect("fixture parses");
    let packages = fixture["packages"].as_object().expect("fixture packages");
    let mut detail: CapturePackageV0 =
        serde_json::from_value(packages["detailPackage"].clone()).expect("detail parses");
    let mut comments: CapturePackageV0 =
        serde_json::from_value(packages["commentsPackage"].clone()).expect("comments parse");
    detail.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["noteId"] = json!(note_id);
    comments.records[0].payload["commentId"] = json!(comment_id);
    comments.records[0].payload["text"] = json!(text);
    detail.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("run-detail-{suffix}")),
    );
    comments.extensions.insert(
        "packageRef".to_owned(),
        json!(format!("run-comments-{suffix}")),
    );
    store
        .admit_xhs_comment_capture_pair_v0(WORKSPACE, detail, comments)
        .await
        .expect("fixture comment admits")
}

fn browser_preview_summary(preview: &Value) -> Value {
    json!({
        "candidate_total": preview["candidates"].as_array().expect("preview candidates").len(),
        "sources": preview["source_distribution"].as_array().expect("preview sources").iter().map(|source| json!({
            "source_note_id": source["source_note_id"],
            "selected_total": source["selected_total"],
        })).collect::<Vec<_>>(),
        "candidates": preview["candidates"].as_array().expect("preview candidates").iter().map(|candidate| json!({
            "source_note_id": candidate["source_note_id"],
            "current_admitted_at": candidate["current_admitted_at"],
            "source_rotation_turn": candidate["source_rotation_turn"],
        })).collect::<Vec<_>>(),
    })
}

async fn get_json(app: &axum::Router, uri: &str) -> (StatusCode, Value) {
    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(uri)
                    .body(Body::empty())
                    .expect("GET builds"),
            )
            .await
            .expect("GET responds"),
    )
    .await
}

async fn get_text(app: &axum::Router, uri: &str) -> (StatusCode, String) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("GET builds"),
        )
        .await
        .expect("GET responds");
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("page response reads");
    (
        status,
        String::from_utf8(body.to_vec()).expect("page is UTF-8"),
    )
}

fn assert_confirmation_page_contract(page: &str) {
    assert!(page.contains("准备本次研究"));
    assert!(page.contains("确认并冻结输入"));
    assert!(page.contains("待执行，尚未开始分析"));
    assert!(page.contains("不会调用模型、不生成结论、不扣费"));
    assert!(page.contains("/api/v0/comment-research/runs"));

    let start = page
        .find("async function submitRunPreparation()")
        .expect("page has run preparation submission");
    let end = page[start..]
        .find("function closePlanPreview()")
        .map(|offset| start + offset)
        .expect("submission ends before drawer close");
    let submission = &page[start..end];
    assert!(submission.contains("workspace_id: workspaceId"));
    assert!(submission.contains("scope: planPreviewScope.value"));
    assert!(submission.contains("limit: Number(planPreviewLimit.value)"));
    assert!(!submission.contains("evidence_id"));
    assert!(!submission.contains("evidence_ids"));
    assert!(!submission.contains("preview:"));
}

async fn post_json(app: &axum::Router, uri: &str, value: Value) -> (StatusCode, Value) {
    response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        serde_json::to_vec(&value).expect("JSON body serializes"),
                    ))
                    .expect("POST builds"),
            )
            .await
            .expect("POST responds"),
    )
    .await
}

async fn response_json(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response reads");
    (
        status,
        serde_json::from_slice(&body).expect("JSON response"),
    )
}

async fn connect_inspector(database_url: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .expect("inspector connects");
    tokio::spawn(async move {
        connection.await.expect("inspector stays connected");
    });
    client
}

async fn run_counts(inspector: &Client) -> Vec<i64> {
    let tables = [
        "comment_research_run_v1",
        "comment_research_run_item_v1",
        "comment_research_run_item_event_v1",
        "comment_analysis_v1",
    ];
    let mut counts = Vec::with_capacity(tables.len());
    for table in tables {
        counts.push(
            inspector
                .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                .await
                .expect("count query")
                .get(0),
        );
    }
    counts
}

fn assert_no_forbidden_run_response_field(value: &Value) {
    const FORBIDDEN: &[&str] = &[
        "author",
        "comment_id",
        "derivation",
        "derivation_id",
        "evidence_id",
        "fingerprint",
        "model",
        "provider",
        "token",
        "cost",
        "raw_text",
        "original_text",
        "source_text",
        "frozen_context",
        "context_pack",
        "prompt",
        "queue",
        "worker",
    ];
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                assert!(
                    !FORBIDDEN.contains(&key.as_str()),
                    "public response must not expose {key}"
                );
                assert_no_forbidden_run_response_field(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                assert_no_forbidden_run_response_field(nested);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}
