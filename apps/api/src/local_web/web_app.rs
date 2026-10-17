//! WEB-REACT-001（DEC-0010）：由 React 应用（`apps/web`）提供的正式页面。
//!
//! Axum 只对 `REACT_PAGES` 登记的路径返回前端入口 HTML；接口、资源与尚未迁移的旧页面照旧，
//! 未登记的路径仍然 404——这里不设全局兜底，否则接口的 404 会变成一个看似正常的页面。
//!
//! 前端产物与本二进制属于同一个源码版本：本机运行链在同一个 checkout 里先构建前端，再构建并
//! 启动服务（`scripts/runtime/sync.sh`）。两边版本都已知却对不上时，页面拒绝提供，不让新接口
//! 配旧界面（或反过来）半新半旧地运行。
use super::comment_study::local_comment_study_guard;
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    middleware,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

/// 由 React 应用提供的正式页面。必须与 `apps/web/src/app/pages.ts` 逐项相同（前端测试对照）。
const REACT_PAGES: &[&str] = &[
    "/settings/models",
    "/ops/creation",
    "/corpus/creators",
    "/corpus/evidence",
    "/collection/targets",
    "/collection/tasks",
    "/collection/domains",
    "/collection/runtime",
    "/ops/overview",
    "/ops/publications",
    "/ops/accounts",
    "/ops/daily",
    "/ops/io",
    "/topics",
    "/topics/{canonical_key}",
];

/// 静态资源只允许这些类型：Vite 产物的脚本、样式、图片和字体。入口 HTML 与构建信息不经这里。
const ASSET_TYPES: &[(&str, &str)] = &[
    ("js", "text/javascript; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("webp", "image/webp"),
    ("woff2", "font/woff2"),
];

#[derive(Clone)]
pub(super) struct WebAppConfig {
    dist: Arc<PathBuf>,
    /// 本进程所属部署的版本（`sync.sh` 写进身份文件）。不是本机运行链启动时为 `unknown`。
    runtime_revision: Arc<str>,
}

impl WebAppConfig {
    fn from_environment() -> Self {
        let dist = match std::env::var_os("LINGGAN_WEB_DIST") {
            Some(path) if !path.is_empty() => PathBuf::from(path),
            _ => PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../web/dist")),
        };
        Self::new(dist, linggan_evidence::runtime_revision())
    }

    pub(super) fn new(dist: PathBuf, runtime_revision: &str) -> Self {
        Self {
            dist: Arc::new(dist),
            runtime_revision: Arc::from(runtime_revision),
        }
    }
}

pub(super) fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    routes_for(WebAppConfig::from_environment())
}

pub(super) fn routes_for<S: Clone + Send + Sync + 'static>(config: WebAppConfig) -> Router<S> {
    // 一级入口的默认子页由服务器决定，不放进前端路由：书签与旧链接不依赖脚本也能到达。
    let mut pages = Router::new().route(
        "/settings",
        get(|| async { Redirect::temporary("/settings/models") }),
    );
    for page in REACT_PAGES {
        pages = if *page == "/topics/{canonical_key}" {
            pages.route(page, get(topic_workspace_entry))
        } else {
            pages.route(page, get(entry))
        };
    }
    pages
        .layer(middleware::from_fn(local_comment_study_guard))
        .route("/ui-assets/{file}", get(asset))
        .with_state(config)
}

/// 前端产物现在能不能提供页面。
#[derive(Debug, PartialEq, Eq)]
enum WebAppState {
    Ready { html: String, revision: String },
    Missing,
    RevisionMismatch { web: String, runtime: String },
}

fn read_state(config: &WebAppConfig) -> WebAppState {
    let Ok(html) = std::fs::read_to_string(config.dist.join("index.html")) else {
        return WebAppState::Missing;
    };
    let revision = std::fs::read_to_string(config.dist.join("build-info.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .and_then(|info| info["revision"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned());
    if !same_revision(&revision, &config.runtime_revision) {
        return WebAppState::RevisionMismatch {
            web: revision,
            runtime: config.runtime_revision.to_string(),
        };
    }
    WebAppState::Ready { html, revision }
}

/// 只有两边都已知才比较；短提交号允许与更长的写法互为前缀。不知道就不判定为不一致。
fn same_revision(web: &str, runtime: &str) -> bool {
    let known = |value: &str| !value.is_empty() && value != "unknown";
    if !known(web) || !known(runtime) {
        return true;
    }
    let (web, runtime) = (web.to_ascii_lowercase(), runtime.to_ascii_lowercase());
    web.starts_with(&runtime) || runtime.starts_with(&web)
}

async fn entry(State(config): State<WebAppConfig>) -> Response {
    match read_state(&config) {
        WebAppState::Ready { html, .. } => Html(html).into_response(),
        WebAppState::Missing => unavailable(
            "web_dist_missing",
            "新版网页的前端文件没有就绪。请重新运行本机同步构建（scripts/runtime/sync.sh）。",
        ),
        WebAppState::RevisionMismatch { .. } => unavailable(
            "web_revision_mismatch",
            "新版网页的前端文件与后台服务不是同一个版本，已停止提供页面。请重新运行本机同步构建。",
        ),
    }
}

// Keep the previous Workspace key boundary before handing the route to React.
async fn topic_workspace_entry(
    State(config): State<WebAppConfig>,
    Path(canonical_key): Path<String>,
) -> Response {
    if !safe_topic_key(&canonical_key) {
        return (StatusCode::BAD_REQUEST, "invalid Topic key").into_response();
    }
    entry(State(config)).await
}

fn safe_topic_key(value: &str) -> bool {
    (2..=96).contains(&value.len())
        && value.starts_with(|character: char| character.is_ascii_lowercase())
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || character == '-'
                || character == '_'
        })
}

/// 基础设施故障的纯文本说明，不是业务页面。
fn unavailable(code: &'static str, message: &'static str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        format!("{message}\n{code}\n"),
    )
        .into_response()
}

async fn asset(State(config): State<WebAppConfig>, Path(file): Path<String>) -> Response {
    let Some(content_type) = asset_content_type(&file) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Ok(bytes) = std::fs::read(config.dist.join(&file)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    // 文件名带内容指纹，内容变了名字就变，可以长期缓存；旧名字在新构建里自然消失。
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

/// 只接受单段文件名：字母、数字、短横线、下划线，用点分段且必须有扩展名。
/// 不可能出现路径分隔符或 `..`，也就读不到产物目录以外的文件。
fn asset_content_type(file: &str) -> Option<&'static str> {
    let well_formed = file.len() <= 128
        && file.contains('.')
        && file.split('.').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        });
    if !well_formed {
        return None;
    }
    let extension = file.rsplit('.').next()?;
    ASSET_TYPES
        .iter()
        .find(|(known, _)| *known == extension)
        .map(|(_, content_type)| *content_type)
}

/// `/health` 里的前端产物状态：给人和部署脚本核对「界面和后台是不是同一个版本」。
pub(super) fn health() -> Value {
    health_for(&WebAppConfig::from_environment())
}

fn health_for(config: &WebAppConfig) -> Value {
    match read_state(config) {
        WebAppState::Ready { revision, .. } => json!({
            "state": "ready",
            "revision": revision,
            "runtimeRevision": config.runtime_revision.as_ref(),
            "pages": REACT_PAGES,
        }),
        WebAppState::Missing => json!({
            "state": "missing",
            "revision": null,
            "runtimeRevision": config.runtime_revision.as_ref(),
            "pages": REACT_PAGES,
        }),
        WebAppState::RevisionMismatch { web, runtime } => json!({
            "state": "revision_mismatch",
            "revision": web,
            "runtimeRevision": runtime,
            "pages": REACT_PAGES,
        }),
    }
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    pub(in crate::local_web) fn state_of(config: &WebAppConfig) -> &'static str {
        match read_state(config) {
            WebAppState::Ready { .. } => "ready",
            WebAppState::Missing => "missing",
            WebAppState::RevisionMismatch { .. } => "revision_mismatch",
        }
    }

    pub(in crate::local_web) fn health_of(config: &WebAppConfig) -> Value {
        health_for(config)
    }

    pub(in crate::local_web) fn content_type_of(file: &str) -> Option<&'static str> {
        asset_content_type(file)
    }

    pub(in crate::local_web) fn revisions_match(web: &str, runtime: &str) -> bool {
        same_revision(web, runtime)
    }
}
