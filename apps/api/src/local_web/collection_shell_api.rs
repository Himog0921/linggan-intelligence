//! The existing Collection header and navigation facts, without server-rendered markup.
use super::*;
use axum::http::StatusCode;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new().route("/api/collection/workspace/shell", get(read))
}

#[derive(Deserialize, Default)]
struct ShellQuery {
    domain: Option<String>,
}

async fn read(State(state): State<LocalWebState>, Query(query): Query<ShellQuery>) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let (domains, surface, as_of) = tokio::join!(
        read_observation_domains(database),
        read_collection_surface(database),
        sqlx::query_scalar::<_, String>("SELECT scope_001_now()::text").fetch_one(database.pool()),
    );
    let scheduler = match surface.surface_state.scheduler_state {
        collection::SchedulerState::Running => "running",
        collection::SchedulerState::Stale => "stale",
        collection::SchedulerState::Unreadable => "unknown",
    };
    let domains = domains.ok();
    let selected = domains.as_ref().and_then(|values| {
        linggan_evidence::observation_domain::resolve_collection_domain(
            values,
            query.domain.as_deref(),
        )
    });
    Json(json!({
        "asOf": as_of.ok(),
        "domainsReadable": domains.is_some(),
        "domains": domains.as_ref().map(|values| values.iter().map(|d| json!({
            "domainRef": d.domain_ref, "name": d.name, "status": d.status,
            "targetCount": d.target_count, "sampleCount": d.sample_count,
        })).collect::<Vec<_>>()),
        "selectedDomain": selected.map(|d| d.domain_ref),
        "totalTargets": surface.surface_state.total_targets,
        "monitoringTargets": surface.surface_state.monitoring_targets,
        "archivingTargets": surface.surface_state.archiving_targets,
        "scheduler": scheduler,
    }))
    .into_response()
}
