//! WEB-REACT-001 E: existing operations APIs and default entry. No page HTML or legacy assets.
use super::*;
use linggan_intelligence::ops_ledger::read::OpsBusiness;
use serde::Deserialize;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/ops", get(entry))
        .merge(super::ops_api::routes())
        .merge(super::ops_overview_api::routes())
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

#[derive(Deserialize)]
pub(super) struct BusinessQuery {
    pub(super) domain: Option<String>,
}

/// WEB-REACT-001 C: 运营默认进入正式创作工作区；台账五页由同一 React 应用提供。
async fn entry(Query(query): Query<BusinessQuery>) -> Response {
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(OpsBusiness::All) => Redirect::to("/ops/creation").into_response(),
        Ok(OpsBusiness::Domain(domain)) => {
            Redirect::to(&format!("/ops/creation?domain={domain}")).into_response()
        }
        Err(error) => super::ops_api::read_error(error),
    }
}
