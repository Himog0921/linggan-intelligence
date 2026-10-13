//! Reuse existing domain actions and content; never return a second page shell into the modal.
use super::*;
pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/management/domains", get(list))
        .route("/api/collection/management/domains/{domain}", get(editor))
}
async fn list(State(state): State<LocalWebState>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "domain_read_unavailable",
        );
    };
    match read_observation_domains(db).await {
        Ok(domains)=>Json(json!({"items":domains.iter().map(|d|json!({"domainRef":d.domain_ref,"name":d.name,"description":d.description,"status":d.status,"targetCount":d.target_count})).collect::<Vec<_>>()})).into_response(),
        Err(_)=>local_read_json_error(axum::http::StatusCode::SERVICE_UNAVAILABLE,"domain_read_unavailable")
    }
}
async fn editor(State(state): State<LocalWebState>, Path(domain): Path<String>) -> Response {
    if domain != "new" && uuid::Uuid::parse_str(&domain).is_err() {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_domain");
    };
    let Html(page) =
        collection_domains_page(State(state), Query(CollectionParams::default())).await;
    let marker = if domain == "new" {
        "<form class=\"domain-create\"".to_owned()
    } else {
        format!("<article class=\"domain-card\" data-domain-ref=\"{domain}\"")
    };
    let closing = if domain == "new" {
        "</form>"
    } else {
        "</article>"
    };
    let fragment = page.find(&marker).and_then(|start| {
        page[start..]
            .find(closing)
            .map(|end| &page[start..start + end + closing.len()])
    });
    match fragment {
        Some(html) => Html(html.to_owned()).into_response(),
        None => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "domain_editor_unavailable",
        ),
    }
}
