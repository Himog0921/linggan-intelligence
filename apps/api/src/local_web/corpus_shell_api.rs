//! 语料二级页面共享的只读外壳接口（WEB-REACT-001 阶段 D1）。
//!
//! 页头面包屑里的领域选择器，以及一级导航「采集」项的状态词，此前由各旧页面在服务器端拼进
//! HTML（`corpus_domain_picker`、证据库页里的 `count_targets`）。页面迁到 React 之后，这两件事实
//! 由本接口给出：用哪个领域、采集有没有观察目标都以服务器为准，前端不重算。
//!
//! 只读，不触发采集、模型或任何写入；与其他本机接口使用同一道来源校验和 no-store。
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    middleware,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_evidence::{
    TargetCounts, count_targets,
    observation_domain::{ObservationDomain, read_observation_domains, resolve_current_domain},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/corpus/shell", get(read))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

#[derive(Deserialize)]
struct ShellQuery {
    /// 地址里的领域参数。无效或不存在的值不报错，只是没有选中任何领域（与旧页面一致）。
    #[serde(default)]
    domain: Option<String>,
}

async fn read(State(state): State<LocalWebState>, Query(query): Query<ShellQuery>) -> Response {
    let Some(database) = state.database.database() else {
        return super::local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let domains = match read_observation_domains(database).await {
        Ok(domains) => domains,
        Err(_) => {
            return super::local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "domain_read_unavailable",
            );
        }
    };
    // 采集状态词读不到时是「状态未知」，不是「无观察目标」：读不到不能推断为空。
    let collection_state = collection_state_word(count_targets(database, None).await);
    Json(shell_json(
        &domains,
        query.domain.as_deref(),
        collection_state,
    ))
    .into_response()
}

/// 与证据库旧页面逐字一致：有观察目标是「观察中」，读到零个是「无观察目标」，读不到是「状态未知」。
fn collection_state_word<E>(counts: Result<TargetCounts, E>) -> &'static str {
    match counts {
        Ok(counts) if counts.total > 0 => "观察中",
        Ok(_) => "无观察目标",
        Err(_) => "状态未知",
    }
}

/// 领域选择器用到的字段：与旧 `render_domain_picker` 读取的完全相同（名称、暂停与否、目标数、作品数）。
fn shell_json(
    domains: &[ObservationDomain],
    requested_domain: Option<&str>,
    collection_state: &str,
) -> Value {
    let selected = resolve_current_domain(domains, requested_domain);
    json!({
        "domains": domains.iter().map(|domain| json!({
            "domainRef": domain.domain_ref,
            "name": domain.name,
            "status": domain.status,
            "targetCount": domain.target_count,
            "sampleCount": domain.sample_count,
        })).collect::<Vec<_>>(),
        "selectedDomain": selected.map(|domain| domain.domain_ref),
        "collectionState": collection_state,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Method, Request, header},
    };
    use tower::ServiceExt;
    use uuid::Uuid;

    fn domain(name: &str, status: &str, samples: Option<i64>) -> ObservationDomain {
        ObservationDomain {
            domain_ref: Uuid::new_v4(),
            name: name.to_owned(),
            description: None,
            research_goal: None,
            status: status.to_owned(),
            sample_count: samples,
            target_count: 3,
            primary_target_count: 2,
            reference_target_count: 1,
        }
    }

    #[test]
    fn collection_state_never_calls_an_unreadable_count_empty() {
        let observed = TargetCounts {
            total: 1,
            ..TargetCounts::default()
        };
        assert_eq!(collection_state_word::<()>(Ok(observed)), "观察中");
        assert_eq!(
            collection_state_word::<()>(Ok(TargetCounts::default())),
            "无观察目标"
        );
        assert_eq!(
            collection_state_word(Err::<TargetCounts, _>(())),
            "状态未知"
        );
    }

    #[test]
    fn shell_json_carries_exactly_what_the_picker_reads_and_resolves_the_selection() {
        let first = domain("领域一", "active", Some(12));
        let second = domain("领域二", "paused", None);
        let domains = vec![first.clone(), second.clone()];
        let shell = shell_json(&domains, Some(&second.domain_ref.to_string()), "观察中");
        assert_eq!(shell["selectedDomain"], json!(second.domain_ref));
        assert_eq!(shell["collectionState"], "观察中");
        assert_eq!(
            shell["domains"][0],
            json!({
                "domainRef": first.domain_ref, "name": "领域一", "status": "active",
                "targetCount": 3, "sampleCount": 12
            })
        );
        // 作品数未知是 null，不是 0。
        assert_eq!(shell["domains"][1]["sampleCount"], Value::Null);
        assert_eq!(shell["domains"][1]["status"], "paused");
    }

    #[test]
    fn an_unknown_or_malformed_domain_selects_nothing_instead_of_falling_back() {
        let domains = vec![domain("领域一", "active", Some(1))];
        for requested in [
            None,
            Some(""),
            Some("not-a-uuid"),
            Some("00000000-0000-4000-8000-000000000999"),
        ] {
            let shell = shell_json(&domains, requested, "状态未知");
            assert_eq!(shell["selectedDomain"], Value::Null, "{requested:?}");
        }
    }

    async fn call(method: Method, uri: &str, host: &str, extra: &[(&str, &str)]) -> Response {
        let mut request = Request::builder()
            .method(method)
            .uri(uri)
            .header(header::HOST, host);
        for (name, value) in extra {
            request = request.header(*name, *value);
        }
        super::super::app()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn body_json(response: Response) -> Value {
        serde_json::from_slice(&to_bytes(response.into_body(), 1 << 20).await.unwrap()).unwrap()
    }

    #[tokio::test]
    async fn without_a_database_the_read_fails_loudly_with_the_local_error_shape() {
        let response = call(
            Method::GET,
            "/api/local/corpus/shell",
            "127.0.0.1:3000",
            &[],
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        assert_eq!(
            body_json(response).await,
            json!({"operation": "local_read", "outcome": "unavailable", "code": "read_model_not_connected"})
        );
    }

    #[tokio::test]
    async fn only_the_local_origin_may_read_it() {
        for (host, extra) in [
            ("evil.example:3000", vec![]),
            ("127.0.0.1:3000", vec![("sec-fetch-site", "cross-site")]),
            (
                "127.0.0.1:3000",
                vec![("origin", "http://evil.example:3000")],
            ),
        ] {
            let response = call(Method::GET, "/api/local/corpus/shell", host, &extra).await;
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{host} {extra:?}");
            assert_eq!(
                body_json(response).await,
                json!({"error": "local_comment_study_origin_rejected"})
            );
        }
        let allowed = call(
            Method::GET,
            "/api/local/corpus/shell?domain=whatever",
            "localhost:3000",
            &[("sec-fetch-site", "same-origin")],
        )
        .await;
        // 来源通过后才会走到数据库这一步（测试里没有数据库）。
        assert_eq!(allowed.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn the_read_model_has_no_write_verbs() {
        for method in [Method::POST, Method::PUT, Method::DELETE] {
            let response = call(method, "/api/local/corpus/shell", "127.0.0.1:3000", &[]).await;
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
        }
    }
}
