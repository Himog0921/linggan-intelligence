//! Read-only, bounded object inspection. Merely opening a panel never queues work.
use super::{LocalWebState, local_read_json_error};
use axum::{Json, Router, extract::{Path, Query, State}, http::StatusCode,
    response::{IntoResponse, Response}, routing::get};
use linggan_evidence::target_inspection::{self as inspection, ComparisonMode, Error, Scope};
use serde::Serialize;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/collection/workspace/target/{target}/inspection/rounds", get(rounds))
        .route("/api/collection/workspace/target/{target}/inspection/rounds/{round}", get(round))
        .route("/api/collection/workspace/target/{target}/inspection/round-comparison", get(compare_rounds))
        .route("/api/collection/workspace/target/{target}/inspection/work-comparison", get(compare_works))
        .route("/api/collection/workspace/target/{target}/inspection/works/{work}/observations", get(trace))
        .route("/api/collection/workspace/target/{target}/inspection/works/{work}/observation-comparison", get(compare_observations))
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Params {
    domain: Option<String>,
    as_of: Option<String>,
    rule: Option<Uuid>,
    left: Option<Uuid>,
    right: Option<Uuid>,
    works: Option<String>,
    mode: Option<String>,
    offset: Option<usize>,
}
impl Params {
    fn scope(&self, target_ref: Uuid) -> Result<Scope, Error> {
        let domain_ref = match self.domain.as_deref() {
            None | Some("all") => None,
            Some(value) => Some(Uuid::parse_str(value).map_err(|_| Error::Invalid("invalid_domain"))?),
        };
        if self.as_of.as_ref().is_some_and(|v| v.is_empty() || v.len()>64) {
            return Err(Error::Invalid("invalid_knowledge_cutoff"));
        }
        Ok(Scope { target_ref, domain_ref, knowledge_cutoff: self.as_of.clone() })
    }
    fn pair(&self) -> Result<(Uuid, Uuid), Error> {
        Ok((self.left.ok_or(Error::Invalid("left_observation_required"))?,
            self.right.ok_or(Error::Invalid("right_observation_required"))?))
    }
    fn work_refs(&self) -> Result<Vec<Uuid>, Error> {
        let raw=self.works.as_deref().ok_or(Error::Invalid("works_required"))?;
        if raw.len()>110 { return Err(Error::Invalid("select_two_or_three_distinct_works")); }
        raw.split(',').map(|id|Uuid::parse_str(id).map_err(|_|Error::Invalid("invalid_work_reference"))).collect()
    }
}

fn respond<T: Serialize>(result: Result<T, Error>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(Error::Invalid(code)) => local_read_json_error(StatusCode::BAD_REQUEST,code),
        Err(Error::NotFound) => local_read_json_error(StatusCode::NOT_FOUND,"target_inspection_not_found"),
        Err(Error::Scope) => local_read_json_error(StatusCode::FORBIDDEN,"target_inspection_scope_unavailable"),
        Err(Error::Database(_) | Error::Material | Error::Acquisition(_)) => unavailable(),
    }
}
fn unavailable() -> Response {
    local_read_json_error(StatusCode::SERVICE_UNAVAILABLE,"target_inspection_unavailable")
}

async fn rounds(State(state):State<LocalWebState>,Path(target):Path<Uuid>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let scope=match params.scope(target){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::read_rounds(db,&scope,params.rule,params.offset.unwrap_or(0)).await)
}
async fn round(State(state):State<LocalWebState>,Path((target,round)):Path<(Uuid,Uuid)>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let scope=match params.scope(target){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::read_round_result(db,&scope,round).await)
}
async fn compare_rounds(State(state):State<LocalWebState>,Path(target):Path<Uuid>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let (scope,(left,right))=match params.scope(target).and_then(|s|params.pair().map(|p|(s,p))){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::compare_rounds(db,&scope,left,right).await)
}
async fn compare_works(State(state):State<LocalWebState>,Path(target):Path<Uuid>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let mode=match params.mode.as_deref(){None|Some("current")=>ComparisonMode::Current,
        Some("age_24h")=>ComparisonMode::Age24h,
        _=>return respond::<()>(Err(Error::Invalid("invalid_comparison_mode")))};
    let (scope,works)=match params.scope(target).and_then(|s|params.work_refs().map(|w|(s,w))){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::compare_works(db,&scope,&works,mode).await)
}
async fn trace(State(state):State<LocalWebState>,Path((target,work)):Path<(Uuid,Uuid)>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let scope=match params.scope(target){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::read_trace(db,&scope,work,params.offset.unwrap_or(0)).await)
}
async fn compare_observations(State(state):State<LocalWebState>,Path((target,work)):Path<(Uuid,Uuid)>,Query(params):Query<Params>) -> Response {
    let Some(db)=state.database.database() else{return unavailable()};
    let (scope,(left,right))=match params.scope(target).and_then(|s|params.pair().map(|p|(s,p))){Ok(v)=>v,Err(e)=>return respond::<()>(Err(e))};
    respond(inspection::compare_observations(db,&scope,work,left,right).await)
}
