//! Loopback-only Topic Workspace page and JSON composition.

use super::{LocalWebState, shell};
use axum::{
    Json, Router,
    extract::{Path, State, rejection::JsonRejection},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use linggan_evidence::read_work_resource;
use linggan_intelligence::{
    TopicWorkspaceError, TopicWorkspaceImport, import_topic_workspace, read_topic_workspace,
    topic_workspace_schema_is_ready,
};
use serde_json::{Value, json};

const TOPIC_CSS: &str = include_str!("topic_workspace.css");
const TOPIC_JS: &str = include_str!("topic_workspace.js");

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/topics", get(entry))
        .route("/topics/{canonical_key}", get(page))
        .route("/api/local/topic-workspaces", post(import_json))
        .route(
            "/api/local/topic-workspaces/{canonical_key}",
            get(read_json),
        )
}

async fn entry() -> Redirect {
    Redirect::temporary("/topics/task-initiation-difficulty")
}

pub(super) async fn stylesheet() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        format!("{}\n{}\n{TOPIC_CSS}", super::LIDS_TOKENS, super::SHELL_CSS),
    )
        .into_response()
}

pub(super) async fn script() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/javascript; charset=utf-8"),
        )],
        TOPIC_JS,
    )
        .into_response()
}

async fn page(Path(canonical_key): Path<String>) -> Response {
    if !safe_key(&canonical_key) {
        return (StatusCode::BAD_REQUEST, "invalid Topic key").into_response();
    }
    Html(page_html(&canonical_key)).into_response()
}

async fn import_json(
    State(state): State<LocalWebState>,
    request: Result<Json<TopicWorkspaceImport>, JsonRejection>,
) -> Response {
    let Json(request) = match request {
        Ok(request) => request,
        Err(error) => {
            eprintln!("Topic workspace import request rejected: {error}");
            return json_error(
                StatusCode::BAD_REQUEST,
                TopicWorkspaceApiOutcome::Rejected,
                "invalid_topic_workspace_request",
            );
        }
    };
    let Some(database) = state.database.database() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_read_model_not_connected",
        );
    };
    match topic_workspace_schema_is_ready(database).await {
        Ok(true) => {}
        Ok(false) => {
            return json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                TopicWorkspaceApiOutcome::Unavailable,
                "topic_schema_not_ready",
            );
        }
        Err(error) => return unavailable(error),
    }
    match import_topic_workspace(database, &request).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => topic_error(error),
    }
}

async fn read_json(
    State(state): State<LocalWebState>,
    Path(canonical_key): Path<String>,
) -> Response {
    let Some(database) = state.database.database() else {
        return json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            TopicWorkspaceApiOutcome::Unavailable,
            "topic_read_model_not_connected",
        );
    };
    match read_topic_workspace(database, &canonical_key).await {
        Ok(Some(workspace)) => match compose_materials(database, workspace).await {
            Ok(payload) => Json(payload).into_response(),
            Err(error) => error,
        },
        Ok(None) => json_error(
            StatusCode::NOT_FOUND,
            TopicWorkspaceApiOutcome::NotFound,
            "topic_workspace_not_found",
        ),
        Err(error) => topic_error(error),
    }
}

async fn compose_materials(
    database: &linggan_storage_postgres::Database,
    workspace: linggan_intelligence::TopicWorkspace,
) -> Result<Value, Response> {
    let mut materials = Vec::with_capacity(workspace.members.len());
    for member in &workspace.members {
        let work = read_work_resource(database, member.work_public_ref)
            .await
            .map_err(|error| {
                eprintln!("Topic Work Resource read unavailable: {error}");
                json_error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    TopicWorkspaceApiOutcome::Unavailable,
                    "work_resource_read_unavailable",
                )
            })?
            .ok_or_else(|| {
                json_error(
                    StatusCode::CONFLICT,
                    TopicWorkspaceApiOutcome::Unavailable,
                    "topic_material_pack_reference_unavailable",
                )
            })?;
        materials.push(json!({
            "classification":member,
            "detailUrl":format!("/api/local/work-resources/{}", member.work_public_ref),
            "workResource":work
        }));
    }
    Ok(json!({"topic":workspace,"materials":materials}))
}

fn topic_error(error: TopicWorkspaceError) -> Response {
    match error {
        TopicWorkspaceError::InvalidRequest(_) => json_error(
            StatusCode::BAD_REQUEST,
            TopicWorkspaceApiOutcome::Rejected,
            "invalid_topic_workspace_request",
        ),
        TopicWorkspaceError::UnknownWorkResource(_) => json_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            TopicWorkspaceApiOutcome::Rejected,
            "unknown_work_resource",
        ),
        TopicWorkspaceError::IdempotencyConflict => json_error(
            StatusCode::CONFLICT,
            TopicWorkspaceApiOutcome::Conflict,
            "topic_idempotency_conflict",
        ),
        TopicWorkspaceError::VersionConflict { .. } => json_error(
            StatusCode::CONFLICT,
            TopicWorkspaceApiOutcome::Conflict,
            "topic_version_conflict",
        ),
        TopicWorkspaceError::CorruptProjection | TopicWorkspaceError::Database(_) => {
            unavailable(error)
        }
    }
}

fn unavailable(error: TopicWorkspaceError) -> Response {
    eprintln!("Topic workspace unavailable: {error}");
    json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        TopicWorkspaceApiOutcome::Unavailable,
        "topic_workspace_unavailable",
    )
}

#[derive(Clone, Copy)]
enum TopicWorkspaceApiOutcome {
    Unavailable,
    Rejected,
    Conflict,
    NotFound,
}

impl TopicWorkspaceApiOutcome {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Unavailable => "unavailable",
            Self::Rejected => "rejected",
            Self::Conflict => "conflict",
            Self::NotFound => "not_found",
        }
    }
}

fn json_error(
    status: StatusCode,
    outcome: TopicWorkspaceApiOutcome,
    code: &'static str,
) -> Response {
    (
        status,
        Json(json!({
            "operation":"topic_workspace",
            "outcome":outcome.as_str(),
            "code":code
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn error_responses_preserve_their_real_outcome() {
        let unknown_work = uuid::Uuid::nil();
        let cases = [
            (
                topic_error(TopicWorkspaceError::InvalidRequest("invalid")),
                StatusCode::BAD_REQUEST,
                "rejected",
                "invalid_topic_workspace_request",
            ),
            (
                topic_error(TopicWorkspaceError::UnknownWorkResource(unknown_work)),
                StatusCode::UNPROCESSABLE_ENTITY,
                "rejected",
                "unknown_work_resource",
            ),
            (
                topic_error(TopicWorkspaceError::IdempotencyConflict),
                StatusCode::CONFLICT,
                "conflict",
                "topic_idempotency_conflict",
            ),
            (
                topic_error(TopicWorkspaceError::VersionConflict {
                    expected: Some(1),
                    actual: Some(2),
                }),
                StatusCode::CONFLICT,
                "conflict",
                "topic_version_conflict",
            ),
            (
                json_error(
                    StatusCode::NOT_FOUND,
                    TopicWorkspaceApiOutcome::NotFound,
                    "topic_workspace_not_found",
                ),
                StatusCode::NOT_FOUND,
                "not_found",
                "topic_workspace_not_found",
            ),
            (
                topic_error(TopicWorkspaceError::CorruptProjection),
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "topic_workspace_unavailable",
            ),
        ];

        for (response, status, outcome, code) in cases {
            assert_eq!(response.status(), status);
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("error body reads");
            let body: Value = serde_json::from_slice(&body).expect("error body is JSON");
            assert_eq!(body.pointer("/operation"), Some(&json!("topic_workspace")));
            assert_eq!(body.pointer("/outcome"), Some(&json!(outcome)));
            assert_eq!(body.pointer("/code"), Some(&json!(code)));
        }
    }
}

fn safe_key(value: &str) -> bool {
    (2..=96).contains(&value.len())
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

fn page_html(canonical_key: &str) -> String {
    let header = shell::global_header(
        shell::PrimarySurface::Topic,
        "本机暂定研究 <span class=\"v7-tech-key\">LOCAL PROVISIONAL RESEARCH</span>",
        "主题图谱 <span class=\"v7-slash\">/</span> <b>领域探索</b> <span class=\"v7-slash\">/</span> <span class=\"v7-context-current\">Topic 工作区</span>",
        "<span class=\"v7-kpi\"><em>生命周期</em><b>暂定</b></span><span class=\"v7-kpi\"><em>裁定方式</em><b>人工</b></span><i class=\"v7-vr\" aria-hidden=\"true\"></i><span>来源与覆盖不得外推 <span class=\"v7-tech-key\">NO GENERALIZATION</span></span>",
        None,
    );
    format!(
        r##"<!doctype html>
<html lang="zh-CN" data-theme="linggan-intelligence">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width,initial-scale=1">
  <title>Topic 工作区 · Linggan Intelligence</title>
  <link rel="stylesheet" href="/assets/topic-workspace.css">
  <script defer src="/assets/topic-workspace.js"></script>
</head>
<body>
<div class="v7-app topic-app" data-topic-workspace data-topic-key="{canonical_key}">
  {header}
  <div class="v7-shell">
    <aside class="v7-side topic-rail" aria-label="Topic 研究导航">
      <div class="v7-nav-label" data-readout="TOPIC">主题研究</div>
      <a class="v7-side-nav" href="#definition" aria-current="page"><i>01</i><span>定义与版本</span></a>
      <a class="v7-side-nav" href="#materials"><i>02</i><span>材料裁定</span></a>
      <a class="v7-side-nav" href="#boundary"><i>03</i><span>来源边界</span></a>
      <div class="v7-side-foot"><span class="v7-side-dot"></span>暂定主题<br><span class="v7-tech-key">PROVISIONAL TOPIC</span><br>不构成正式 Topic、趋势或市场事实。</div>
    </aside>
    <main class="topic-workspace" aria-labelledby="topic-title">
      <section class="topic-definition" id="definition">
        <div>
          <p class="topic-eyebrow">领域探索 / TOPIC WORKSPACE</p>
          <div class="topic-title-row"><span class="topic-state">暂定主题 <small>PROVISIONAL TOPIC</small></span><span id="topic-version">VERSION UNKNOWN</span></div>
          <h1 id="topic-title">当前未读取 Topic</h1>
          <p id="topic-definition-text">页面只声明研究结构；本机读模型返回前，不显示合成定义或材料。</p>
        </div>
        <dl class="topic-facts">
          <div><dt>裁定方式</dt><dd id="topic-run-kind">人工裁定 <small>HUMAN ADJUDICATED</small></dd></div>
          <div><dt>材料包</dt><dd id="topic-pack-ref">尚未读取</dd></div>
          <div><dt>读取状态</dt><dd id="topic-read-state">当前未读取 Topic</dd></div>
        </dl>
      </section>
      <section class="topic-research" id="materials" aria-label="Topic 材料研究面">
        <aside class="topic-roles">
          <p class="topic-panel-label">分类视角 <small>CLASSIFICATION</small></p>
          <button type="button" data-role-filter="all" aria-pressed="true">全部裁定 <b id="topic-count-all">—</b></button>
          <button type="button" data-role-filter="support" aria-pressed="false">支持材料 <b id="topic-count-support">—</b></button>
          <button type="button" data-role-filter="challenge" aria-pressed="false">挑战材料 <b id="topic-count-challenge">—</b></button>
          <button type="button" data-role-filter="boundary" aria-pressed="false">边界材料 <b id="topic-count-boundary">—</b></button>
          <div class="topic-adjudication"><strong>人工裁定说明</strong><p id="topic-adjudication-note">尚未读取。</p></div>
        </aside>
        <section class="topic-materials" aria-labelledby="topic-materials-title">
          <header><div><p class="topic-panel-label">冻结材料包 <small>FROZEN MATERIAL PACK</small></p><h2 id="topic-materials-title">精确 Work Resource 引用</h2></div><span id="topic-material-count">—</span></header>
          <div class="topic-feedback" id="topic-feedback" role="status" aria-live="polite">正在读取本机 Topic 工作区…</div>
          <div class="topic-material-list" id="topic-material-list" role="listbox" aria-label="Topic 材料"></div>
        </section>
        <aside class="topic-inspector" aria-labelledby="topic-inspector-title">
          <p class="topic-panel-label">当前材料 <small>CURRENT MATERIAL</small></p>
          <h2 id="topic-inspector-title">请选择一条材料</h2>
          <p id="topic-inspector-rationale">右侧只显示当前 Work Resource 读模型与人工裁定理由。</p>
          <dl id="topic-inspector-facts"></dl>
          <a id="topic-inspector-link" hidden>打开 Work Resource JSON</a>
        </aside>
      </section>
      <section class="topic-boundary" id="boundary">
        <div><p class="topic-panel-label">来源边界 <small>SOURCE BOUNDARY</small></p><p id="topic-source-boundary">尚未读取；不能判断覆盖、代表性或新鲜度。</p></div>
        <div><strong>这里没有宣称</strong><p>没有正式 Topic 发布、趋势结论、市场规模、Claim 或自动分类成功。</p></div>
      </section>
    </main>
  </div>
</div>
</body>
</html>"##
    )
}
