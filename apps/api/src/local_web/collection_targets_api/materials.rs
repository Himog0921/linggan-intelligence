use super::{LocalWebState, local_read_json_error};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use linggan_evidence::target_inspection::{self as inspection, Error, Scope};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/collection/workspace/target/{target}/inspection/works/{work}/dossier",
            get(dossier),
        )
        .route(
            "/api/collection/workspace/target/{target}/inspection/works/{work}/related",
            get(related),
        )
        .route(
            "/api/collection/workspace/target/{target}/inspection/detail-gaps",
            get(detail_gaps),
        )
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    domain: Option<String>,
    as_of: Option<String>,
}
impl Params {
    fn scope(self, target_ref: Uuid) -> Result<Scope, Error> {
        let domain_ref = match self.domain.as_deref() {
            None | Some("all") => None,
            Some(v) => Some(Uuid::parse_str(v).map_err(|_| Error::Invalid("invalid_domain"))?),
        };
        if self
            .as_of
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 64)
        {
            return Err(Error::Invalid("invalid_knowledge_cutoff"));
        }
        Ok(Scope {
            target_ref,
            domain_ref,
            knowledge_cutoff: self.as_of,
        })
    }
}
fn respond<T: Serialize>(result: Result<T, Error>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(Error::Invalid(code)) => local_read_json_error(StatusCode::BAD_REQUEST, code),
        Err(Error::NotFound) => {
            local_read_json_error(StatusCode::NOT_FOUND, "target_inspection_not_found")
        }
        Err(Error::Scope) => {
            local_read_json_error(StatusCode::FORBIDDEN, "target_inspection_scope_unavailable")
        }
        Err(Error::Database(_) | Error::Material | Error::Acquisition(_)) => unavailable(),
    }
}
fn unavailable() -> Response {
    local_read_json_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "target_inspection_unavailable",
    )
}
async fn dossier(
    State(state): State<LocalWebState>,
    Path((target, work)): Path<(Uuid, Uuid)>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let scope = match params.scope(target) {
        Ok(v) => v,
        Err(e) => return respond::<()>(Err(e)),
    };
    respond(inspection::read_dossier(db, &scope, work).await)
}
async fn related(
    State(state): State<LocalWebState>,
    Path((target, work)): Path<(Uuid, Uuid)>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let scope = match params.scope(target) {
        Ok(v) => v,
        Err(e) => return respond::<()>(Err(e)),
    };
    respond(inspection::related_works(db, &scope, work).await)
}

async fn detail_gaps(
    State(state): State<LocalWebState>,
    Path(target): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let scope = match params.scope(target) {
        Ok(v) => v,
        Err(e) => return respond::<()>(Err(e)),
    };
    respond(inspection::preview_detail_gaps(db, &scope).await)
}
