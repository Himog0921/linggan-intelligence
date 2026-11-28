use super::*;
use linggan_intelligence::ai_foundation::operating_direction as ai;
pub(super) async fn read_directions(
    State(state): State<LocalWebState>,
    Extension(model): Extension<Arc<TopicMapReadModel>>,
    Query(query): Query<ResearchQuery>,
) -> Response {
    let db = match database(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };
    let scope = TopicMapQuery {
        domain_ref: Some(query.domain_ref),
        ..Default::default()
    };
    let snap = match model.read(db, &scope).await {
        Ok(s) => s,
        Err(e) => return map_error(e),
    };
    match linggan_intelligence::topic_map::directions::read(db, query.domain_ref, &snap).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => map_error(e),
    }
}
pub(super) async fn preview_directions(
    State(state): State<LocalWebState>,
    Extension(model): Extension<Arc<TopicMapReadModel>>,
    Json(command): Json<linggan_intelligence::topic_map::directions::Save>,
) -> Response {
    let db = match database(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };
    let scope = TopicMapQuery {
        domain_ref: Some(command.domain_ref),
        ..Default::default()
    };
    let snap = match model.read(db, &scope).await {
        Ok(s) => s,
        Err(e) => return map_error(e),
    };
    match linggan_intelligence::topic_map::directions::preview(db, command, &snap).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => map_error(e),
    }
}
pub(super) async fn save_directions(
    State(state): State<LocalWebState>,
    Extension(model): Extension<Arc<TopicMapReadModel>>,
    Json(command): Json<linggan_intelligence::topic_map::directions::Save>,
) -> Response {
    let db = match database(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };
    let scope = TopicMapQuery {
        domain_ref: Some(command.domain_ref),
        ..Default::default()
    };
    let snap = match model.read(db, &scope).await {
        Ok(s) => s,
        Err(e) => return map_error(e),
    };
    match linggan_intelligence::topic_map::directions::save(db, command, &snap).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => map_error(e),
    }
}
pub(super) async fn list_runs(
    State(state): State<LocalWebState>,
    Query(query): Query<ResearchQuery>,
) -> Response {
    let db = match database(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };
    super::super::ai_settings::respond(ai::latest(db, query.domain_ref).await)
}
pub(super) async fn generate(
    State(state): State<LocalWebState>,
    Extension(model): Extension<Arc<TopicMapReadModel>>,
    Json(command): Json<ai::Generate>,
) -> Response {
    let db = match database(&state) {
        Ok(db) => db,
        Err(e) => return e,
    };
    let scope = TopicMapQuery {
        domain_ref: Some(command.domain_ref),
        ..Default::default()
    };
    let snap = match model.read(db, &scope).await {
        Ok(s) => s,
        Err(e) => return map_error(e),
    };
    super::super::ai_settings::respond(ai::generate(db, command, &snap).await)
}
