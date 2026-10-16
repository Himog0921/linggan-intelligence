//! Narrow JSON read projections for the React collection management pages.
use super::*;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route(
            "/api/collection/management/domains",
            get(content_workbench_domains),
        )
        .route("/api/collection/workspace/domains", get(domains))
        .route("/api/collection/workspace/runtime", get(runtime))
}

#[derive(serde::Deserialize, Default)]
struct ManagementParams {
    domain: Option<String>,
    return_to: Option<String>,
}

/// Read-only selection data for the React content workbench.
///
/// The route name predates the collection React migration, but its sole remaining consumer is a
/// separate React page. Keep this projection intentionally small rather than coupling that page to
/// the management workspace's much larger domain read model.
async fn content_workbench_domains(State(state): State<LocalWebState>) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "domain_read_unavailable",
        );
    };
    match read_observation_domains(database).await {
        Ok(domains) => Json(content_workbench_domain_options(&domains)).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "domain_read_unavailable",
        ),
    }
}

fn content_workbench_domain_options(domains: &[ObservationDomain]) -> serde_json::Value {
    json!({
        "items": domains
            .iter()
            .map(|domain| json!({"domainRef": domain.domain_ref, "name": domain.name}))
            .collect::<Vec<_>>()
    })
}

async fn domains(
    State(state): State<LocalWebState>,
    Query(params): Query<ManagementParams>,
) -> Response {
    let as_of: Option<String> = match state.database.database() {
        Some(db) => sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(db.pool())
            .await
            .ok(),
        None => None,
    };
    let context = collection_management::ManagementContext::new(
        params.return_to.as_deref(),
        params.domain.as_deref(),
    );
    let Some(database) = state.database.database() else {
        return Json(json!({"asOf": as_of, "returnTo":context.return_to(), "readable":false}))
            .into_response();
    };
    let domains = match read_observation_domains(database).await {
        Ok(domains) => domains,
        Err(_) => {
            return Json(json!({"asOf": as_of, "returnTo":context.return_to(), "readable":false}))
                .into_response();
        }
    };
    let (targets, targets_readable): (Vec<(uuid::Uuid, String, String)>, bool) = match sqlx::query_as(
        "SELECT target.target_ref,COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key),target.target_kind \
         FROM collection_observation_target target \
         WHERE COALESCE(to_jsonb(target)->>'purpose_kind','long_term')='long_term' ORDER BY target.first_stored_at DESC,target.target_ref",
    )
    .fetch_all(database.pool())
    .await
    {
        Ok(values) => (values, true),
        Err(_) => (Vec::new(), false),
    };
    let (relations, relations_readable): (
        Vec<(uuid::Uuid, uuid::Uuid, String, String, String, String)>,
        bool,
    ) = match sqlx::query_as(
        "SELECT relation.domain_ref,relation.target_ref, \
                COALESCE(NULLIF(btrim(target.display_name),''),target.identity_key), \
                target.target_kind,relation.role,target.lifecycle_state \
         FROM observation_domain_target relation \
         JOIN collection_observation_target target USING(target_ref) \
         WHERE COALESCE(to_jsonb(target)->>'purpose_kind','long_term')='long_term' ORDER BY relation.domain_ref,target.first_stored_at DESC,target.target_ref",
    )
    .fetch_all(database.pool())
    .await
    {
        Ok(values) => (values, true),
        Err(_) => (Vec::new(), false),
    };
    let (lane_states, lanes_readable): (Vec<(uuid::Uuid, String, String, i64)>, bool) =
        match sqlx::query_as(
            "WITH domain_content AS ( \
                 SELECT DISTINCT domain_ref,content_public_ref \
                   FROM linggan_material_domain_usage \
             ), discovery_classified AS ( \
                 SELECT usage.domain_ref,usage.content_public_ref,'discovery'::text AS lane, \
                        'SEARCHABLE'::text AS state \
                   FROM linggan_material_domain_usage usage \
                   JOIN linggan_material_discovery_finding finding \
                     ON finding.content_public_ref=usage.content_public_ref \
                    AND finding.package_ref=usage.package_ref \
                   JOIN linggan_runtime_capture_package package \
                     ON package.package_ref=finding.package_ref \
                  GROUP BY usage.domain_ref,usage.content_public_ref \
             ), latest_lane AS ( \
                 SELECT DISTINCT ON (work_usage.domain_ref,lane.content_public_ref,lane.lane) \
                        work_usage.domain_ref,lane.content_public_ref,lane.lane,lane.retained, \
                        lane.producer_acquired,lane.failed,lane.known_unattempted, \
                        lane.unknown_count,lane.stopped_reason,package.coverage \
                   FROM linggan_material_lane_observation lane \
                   JOIN linggan_runtime_capture_package package USING(package_ref) \
                   JOIN linggan_runtime_task task ON task.task_id=package.task_id \
                   JOIN collection_work_order_lease_task lease_task \
                     ON lease_task.task_id=task.task_id \
                   JOIN collection_work_order_lease lease USING(lease_ref) \
                   JOIN collection_work_order_domain_usage work_usage USING(work_order_ref) \
                   JOIN domain_content dc \
                     ON dc.domain_ref=work_usage.domain_ref \
                    AND dc.content_public_ref=lane.content_public_ref \
                  ORDER BY work_usage.domain_ref,lane.content_public_ref,lane.lane, \
                           package.accepted_at DESC,lane.created_at DESC \
             ), classified AS ( \
                 SELECT domain_ref,content_public_ref,lane,CASE \
                     WHEN lane IN ('comments','replies') \
                          AND coverage #>> '{target,commentCollection,state}'='invalid_target' \
                          THEN 'FAILED' \
                     WHEN lane IN ('comments','replies') \
                          AND coverage #>> '{target,commentCollection,state}'='partial' \
                          THEN 'PARTIAL' \
                     WHEN lane IN ('comments','replies') \
                          AND coverage #>> '{target,commentCollection,state}'='complete' \
                          AND retained=0 THEN 'OBSERVED' \
                     WHEN lane IN ('comments','replies') \
                          AND coverage #>> '{target,commentCollection,state}'='complete' \
                          THEN 'SEARCHABLE' \
                     WHEN stopped_reason='risk_control' THEN 'RISK_CONTROL' \
                     WHEN retained IS NULL OR producer_acquired IS NULL THEN 'UNKNOWN' \
                     WHEN retained <> producer_acquired THEN 'PARTIAL' \
                     WHEN retained > 0 AND (COALESCE(failed,0)>0 OR \
                          COALESCE(known_unattempted,0)>0 OR COALESCE(unknown_count,0)>0) \
                          THEN 'PARTIAL' \
                     WHEN retained > 0 THEN 'SEARCHABLE' \
                     WHEN COALESCE(failed,0)>0 THEN 'FAILED' \
                     WHEN COALESCE(unknown_count,0)>0 THEN 'UNKNOWN' \
                     WHEN retained=0 THEN 'NOT_OBSERVED' \
                     ELSE 'UNKNOWN' END AS state \
                   FROM latest_lane \
                 UNION ALL \
                 SELECT domain_ref,content_public_ref,lane,state \
                   FROM discovery_classified \
             ) \
             SELECT domain_ref,lane,state,count(DISTINCT content_public_ref)::bigint \
               FROM classified GROUP BY domain_ref,lane,state",
        )
        .fetch_all(database.pool())
        .await
        {
            Ok(values) => (values, true),
            Err(_) => (Vec::new(), false),
        };
    let (media_states, media_readable): (Vec<(uuid::Uuid, String, String, i64)>, bool) =
        match sqlx::query_as(
            "WITH domain_content AS ( \
                 SELECT DISTINCT domain_ref,content_public_ref \
                   FROM linggan_material_domain_usage \
             ), current_origins AS ( \
                 SELECT DISTINCT ON (dc.domain_ref,dc.content_public_ref,origin.slot_key) \
                        dc.domain_ref,dc.content_public_ref,origin.observation_ref,origin.slot_key \
                   FROM domain_content dc \
                   LEFT JOIN linggan_material_media_origin origin \
                     ON origin.content_public_ref=dc.content_public_ref \
                  ORDER BY dc.domain_ref,dc.content_public_ref,origin.slot_key, \
                           origin.source_generation DESC NULLS LAST,origin.created_at DESC NULLS LAST \
             ), slot_facts AS ( \
                 SELECT dc.domain_ref,dc.content_public_ref,origin.slot_key, \
                        EXISTS (SELECT 1 FROM linggan_material_media_candidate candidate \
                                 WHERE candidate.observation_ref=origin.observation_ref) AS has_remote, \
                        EXISTS (SELECT 1 FROM linggan_media_download_attempt attempt \
                                 JOIN linggan_media_materialization materialization \
                                   USING(download_attempt_ref) \
                                 WHERE attempt.media_observation_ref=origin.observation_ref \
                                   AND NOT EXISTS ( \
                                       SELECT 1 FROM linggan_current_material_media_disposition disposition \
                                        WHERE disposition.slot_key=origin.slot_key \
                                           OR disposition.materialization_ref=materialization.materialization_ref \
                                           OR disposition.blob_sha256=materialization.blob_sha256)) AS localized, \
                        EXISTS (SELECT 1 FROM linggan_media_download_attempt attempt \
                                 WHERE attempt.media_observation_ref=origin.observation_ref \
                                   AND attempt.ended_at IS NOT NULL \
                                   AND attempt.terminal_reason IS NOT NULL \
                                   AND attempt.terminal_reason <> 'acquired') AS failed \
                   FROM current_origins origin \
                   JOIN domain_content dc USING(domain_ref,content_public_ref) \
             ), content_facts AS ( \
                 SELECT domain_ref,content_public_ref,count(slot_key)::bigint AS slots, \
                        count(slot_key) FILTER (WHERE localized)::bigint AS localized_slots, \
                        count(slot_key) FILTER (WHERE has_remote AND NOT localized)::bigint AS remote_slots, \
                        count(slot_key) FILTER (WHERE failed AND NOT localized)::bigint AS failed_slots \
                   FROM slot_facts GROUP BY domain_ref,content_public_ref \
             ), classified AS ( \
                 SELECT domain_ref,CASE \
                     WHEN slots=0 THEN 'UNKNOWN' \
                     WHEN localized_slots=slots THEN 'LOCALIZED' \
                     WHEN localized_slots>0 THEN 'PARTIAL' \
                     WHEN remote_slots>0 THEN 'REMOTE_ONLY' \
                     WHEN failed_slots>0 THEN 'FAILED' \
                     ELSE 'UNKNOWN' END AS state \
                   FROM content_facts \
             ) \
             SELECT domain_ref,'media_bytes'::text,state,count(*)::bigint \
               FROM classified GROUP BY domain_ref,state",
        )
        .fetch_all(database.pool())
        .await
        {
            Ok(values) => (values, true),
            Err(_) => (Vec::new(), false),
        };
    let (comment_counts, comments_readable): (Vec<(uuid::Uuid, i64)>, bool) = match sqlx::query_as(
        "SELECT usage.domain_ref,count(DISTINCT comment.material_ref)::bigint \
         FROM linggan_material_domain_usage usage \
         JOIN linggan_material_comment comment USING(content_public_ref) \
         GROUP BY usage.domain_ref",
    )
    .fetch_all(database.pool())
    .await
    {
        Ok(values) => (values, true),
        Err(_) => (Vec::new(), false),
    };
    let (last_observations, last_readable): (Vec<(uuid::Uuid, Option<String>)>, bool) =
        match sqlx::query_as(
            "SELECT usage.domain_ref,max(package.accepted_at)::text \
         FROM linggan_material_domain_usage usage \
         JOIN linggan_runtime_capture_package package USING(package_ref) \
         GROUP BY usage.domain_ref",
        )
        .fetch_all(database.pool())
        .await
        {
            Ok(values) => (values, true),
            Err(_) => (Vec::new(), false),
        };

    let items = domains.iter().map(|domain| {
        let lanes = ["discovery", "detail", "comments", "replies", "media_slots", "media_bytes", "ocr", "asr"].iter().map(|lane| {
            let readable = if *lane == "media_bytes" {media_readable} else {lanes_readable};
            let source = if *lane == "media_bytes" {&media_states} else {&lane_states};
            let observed: Vec<_> = source.iter().filter(|row|row.0==domain.domain_ref && row.1==*lane).map(|row|(row.2.as_str(),row.3)).collect();
            let summary = if !readable { collection_domain_lane_state_label("UNKNOWN").to_owned() }
                else if observed.is_empty() {collection_domain_lane_state_label(if domain.sample_count==Some(0){"NOT_OBSERVED"}else{"UNKNOWN"}).to_owned()}
                else {observed.iter().map(|(state,count)|if *count>0 {format!("{} {} 篇",collection_domain_lane_state_label(state),count)} else {collection_domain_lane_state_label(state).to_owned()}).collect::<Vec<_>>().join(" · ")};
            json!({"lane":lane,"label":collection_domain_lane_label(lane),"summary":summary,"readable":readable})
        }).collect::<Vec<_>>();
        json!({"domainRef":domain.domain_ref,"name":domain.name,"description":domain.description,"researchGoal":domain.research_goal,"status":domain.status,
            "targetCount":domain.target_count,"creatorTargetCount":if relations_readable{Some(relations.iter().filter(|r|r.0==domain.domain_ref && r.3=="creator").count())}else{None},"keywordTargetCount":if relations_readable{Some(relations.iter().filter(|r|r.0==domain.domain_ref && r.3=="keyword").count())}else{None},"primaryTargetCount":domain.primary_target_count,"referenceTargetCount":domain.reference_target_count,
            "sampleCount":domain.sample_count,"commentCount":if comments_readable{Some(comment_counts.iter().find(|row|row.0==domain.domain_ref).map_or(0,|row|row.1))}else{None},
            "lastReadable":last_readable,"lastObservation":last_observations.iter().find(|row|row.0==domain.domain_ref).and_then(|row|row.1.as_deref()),"lanes":lanes})
    }).collect::<Vec<_>>();
    Json(json!({"asOf":as_of,"returnTo":context.return_to(),"readable":true,"domains":items,
        "targetsReadable":targets_readable,"relationsReadable":relations_readable,
        "targets":targets.iter().map(|row|json!({"targetRef":row.0,"name":row.1,"kindLabel":collection_target_kind_label(&row.2)})).collect::<Vec<_>>(),
        "relations":relations.iter().map(|row|json!({"domainRef":row.0,"targetRef":row.1,"name":row.2,"kindLabel":collection_target_kind_label(&row.3),"role":row.4,"lifecycleLabel":collection_target_lifecycle_label(&row.5)})).collect::<Vec<_>>()
    })).into_response()
}

async fn runtime(
    State(state): State<LocalWebState>,
    Query(params): Query<ManagementParams>,
) -> Response {
    let as_of: Option<String> = match state.database.database() {
        Some(db) => sqlx::query_scalar("SELECT scope_001_now()::text")
            .fetch_one(db.pool())
            .await
            .ok(),
        None => None,
    };
    let context = collection_management::ManagementContext::new(
        params.return_to.as_deref(),
        params.domain.as_deref(),
    );
    let Some(database) = state.database.database() else {
        return Json(json!({"asOf":as_of,"returnTo":context.return_to(),"readable":false}))
            .into_response();
    };
    let (capacity, roster, control) = tokio::join!(
        read_runtime_capacity(database),
        read_station_overview(database),
        collection::collection_control_surface_view::read_collection_control_surface(database, 100)
    );
    let capacity = capacity.ok();
    let roster = roster.ok();
    let mut capabilities = station_view::CapabilityMatrix::new();
    if let Some((stations, _)) = &roster {
        for station in stations {
            if let Ok(rows) = read_station_capabilities(database, station.station_ref).await {
                capabilities.insert(station.station_ref, rows);
            }
        }
    }
    let control = match control {
        Ok(collection::collection_control_surface_view::CollectionControlSurfaceRead::Ready(
            value,
        )) => Some(value),
        _ => None,
    };
    let recovery_lanes = if control.is_none() && !linggan_evidence::collection_governance_enabled()
    {
        Some(collection::collection_control_surface_view::recovery_runtime_lanes())
    } else {
        None
    };
    let control = runtime_control_for_page(
        control.as_ref(),
        recovery_lanes.as_deref(),
        state.account_digest_key.is_some(),
    );
    let projection = station_view::runtime_projection(
        capacity.as_ref(),
        roster.as_ref().map(|(a, b)| (a.as_slice(), b.as_slice())),
        &capabilities,
        control.as_ref(),
        station_view::recent_installation_cutoff(),
    );
    Json(json!({"asOf":as_of,"returnTo":context.return_to(),"readable":true,"projection":projection})).into_response()
}

/// Transport-only command adapter. Every decision and write remains with the existing owner.
pub(super) async fn command(
    state: &LocalWebState,
    action: &str,
    fields: &serde_json::Value,
) -> Result<Option<String>, String> {
    let text = |name: &str| -> Result<&str, String> {
        fields
            .get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("invalid_{name}"))
    };
    let optional_text = |name: &str| -> Result<Option<&str>, String> {
        match fields.get(name) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::String(value)) => Ok(Some(value)),
            _ => Err(format!("invalid_{name}")),
        }
    };
    let reference = |name: &str| -> Result<uuid::Uuid, String> {
        uuid::Uuid::parse_str(text(name)?).map_err(|_| format!("invalid_{name}"))
    };
    let return_to = optional_text("return_to")?;
    let database = state.database.database().ok_or_else(|| {
        if action.starts_with("runtime-") {
            "read_model_not_connected"
        } else {
            "database_unavailable"
        }
        .to_owned()
    })?;
    let mut location = collection_management::redirect_path("/collection/runtime", None, return_to);
    match action {
        "domain-save" => {
            let name = text("name")?;
            let description = optional_text("description")?;
            let goal = optional_text("research_goal")?;
            let domain_ref = match optional_text("domain_ref")? {
                Some(value) => Some(
                    uuid::Uuid::parse_str(value).map_err(|_| "invalid_domain_ref".to_owned())?,
                ),
                None => None,
            };
            let result = if let Some(domain_ref) = domain_ref {
                linggan_evidence::observation_domain::update_observation_domain(
                    database,
                    domain_ref,
                    name,
                    description,
                    goal,
                    optional_text("status")?.unwrap_or("active"),
                )
                .await
                .map(|_| domain_ref)
            } else {
                linggan_evidence::observation_domain::create_observation_domain_with_details(
                    database,
                    name,
                    description,
                    goal,
                )
                .await
                .map(|domain| domain.domain_ref)
            };
            let domain_ref = result.map_err(domain_save_error)?;
            location = collection_management::redirect_path(
                &format!("/collection/domains?domain={domain_ref}"),
                None,
                return_to,
            );
        }
        "domain-target" => {
            let domain_ref = reference("domain_ref")?;
            let target_ref = reference("target_ref")?;
            let role = optional_text("role")?.unwrap_or("primary");
            match text("action")? {
                "assign" => linggan_evidence::assign_target_domain_with_role(
                    database, target_ref, domain_ref, role,
                )
                .await
                .map(|_| ()),
                "role" => {
                    linggan_evidence::set_target_domain_role(database, target_ref, domain_ref, role)
                        .await
                }
                "remove" => {
                    linggan_evidence::remove_target_domain(database, target_ref, domain_ref).await
                }
                _ => return Err("action_invalid".to_owned()),
            }
            .map_err(target_relation_error)?;
            location = collection_management::redirect_path(
                &format!("/collection/domains?domain={domain_ref}"),
                None,
                return_to,
            );
        }
        "runtime-register" => {
            register_station(database, text("display_name")?.trim(), 200)
                .await
                .map_err(|error| station_command_error(error, "station_rejected"))?;
        }
        "runtime-rename" => {
            rename_station(database, reference("station_ref")?, text("display_name")?)
                .await
                .map_err(|error| station_command_error(error, "station_name_rejected"))?;
        }
        "runtime-claim-window" => {
            let hours = match fields.get("valid_for_hours") {
                Some(serde_json::Value::Number(value)) => {
                    value.as_i64().and_then(|value| i32::try_from(value).ok())
                }
                Some(serde_json::Value::String(value)) => value.parse::<i32>().ok(),
                _ => None,
            }
            .ok_or_else(|| "invalid_valid_for_hours".to_owned())?;
            open_claim_window(database, reference("station_ref")?, hours)
                .await
                .map_err(|error| station_command_error(error, "claim_window_rejected"))?;
        }
        "runtime-close-window" => {
            close_claim_window(database, reference("station_ref")?)
                .await
                .map_err(|error| station_command_error(error, "close_window_rejected"))?;
        }
        "runtime-retire" => {
            retire_station(database, reference("station_ref")?, "在执行工位页停用")
                .await
                .map_err(|error| station_command_error(error, "retire_rejected"))?;
        }
        "runtime-claims" => {
            claim_installation(
                database,
                reference("installation_ref")?,
                reference("station_ref")?,
            )
            .await
            .map_err(|error| station_command_error(error, "claim_rejected"))?;
        }
        "runtime-accepting" => {
            let accepting = match fields.get("accepting") {
                Some(serde_json::Value::Bool(value)) => Some(*value),
                Some(serde_json::Value::String(value)) => value.parse::<bool>().ok(),
                _ => None,
            }
            .ok_or_else(|| "invalid_accepting".to_owned())?;
            set_station_accepting(database, reference("station_ref")?, accepting, "person")
                .await
                .map_err(|error| control_command_error(error, "station_acceptance_rejected"))?;
        }
        "runtime-account-bindings" => {
            bind_observation_account(
                database,
                reference("account_ref")?,
                reference("installation_ref")?,
                "person",
            )
            .await
            .map_err(|error| control_command_error(error, "account_binding_rejected"))?;
        }
        _ => return Err("unknown_action".to_owned()),
    }
    Ok(Some(location))
}

fn domain_save_error(
    error: linggan_evidence::observation_domain::ObservationDomainError,
) -> String {
    use linggan_evidence::observation_domain::ObservationDomainError;
    match error {
        ObservationDomainError::DuplicateName => "name_taken",
        ObservationDomainError::EmptyName => "name_required",
        ObservationDomainError::Database(_) | ObservationDomainError::SchemaUnavailable => {
            "domain_storage_unavailable"
        }
        ObservationDomainError::UnknownDomain | ObservationDomainError::InvalidStatus => {
            "save_rejected"
        }
    }
    .to_owned()
}

fn target_relation_error(error: linggan_evidence::CollectionTargetError) -> String {
    use linggan_evidence::CollectionTargetError;
    match error {
        CollectionTargetError::Database(_) | CollectionTargetError::SchemaUnavailable => {
            "relation_storage_unavailable"
        }
        CollectionTargetError::Contract(_)
        | CollectionTargetError::IllegalTransition { .. }
        | CollectionTargetError::UnknownTarget
        | CollectionTargetError::UnknownDomain
        | CollectionTargetError::DomainAlreadyAssigned => "relation_rejected",
    }
    .to_owned()
}

// PostgreSQL integrity errors are explicit constraint rejections. In particular, the
// station owner reports a duplicate display name as SQLSTATE 23505, not a typed variant.
// Transport/pool errors provide no such acknowledgement and must retain the request identity.
fn known_constraint_rejection(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(error) if error.code().is_some_and(|code|code.starts_with("23")))
}

fn station_command_error(error: linggan_evidence::StationError, rejection: &str) -> String {
    use linggan_evidence::StationError;
    match error {
        StationError::Database(error) if known_constraint_rejection(&error) => rejection.to_owned(),
        StationError::Database(_) | StationError::SchemaUnavailable => {
            "station_storage_unavailable".to_owned()
        }
        StationError::Control(error) => control_command_error(error, rejection),
        _ => rejection.to_owned(),
    }
}

fn control_command_error(
    error: linggan_evidence::CollectionControlError,
    rejection: &str,
) -> String {
    use linggan_evidence::CollectionControlError;
    match error {
        CollectionControlError::Database(error) if known_constraint_rejection(&error) => {
            rejection.to_owned()
        }
        CollectionControlError::Database(_) | CollectionControlError::SchemaUnavailable => {
            "control_storage_unavailable".to_owned()
        }
        _ => rejection.to_owned(),
    }
}

#[cfg(test)]
#[path = "collection_management_api_tests.rs"]
mod tests;

#[cfg(test)]
mod command_error_tests {
    use super::*;
    #[test]
    fn domain_and_relation_storage_failures_do_not_claim_a_known_rejection() {
        use linggan_evidence::observation_domain::ObservationDomainError;
        assert_eq!(
            domain_save_error(ObservationDomainError::Database(sqlx::Error::PoolClosed)),
            "domain_storage_unavailable"
        );
        assert_eq!(
            domain_save_error(ObservationDomainError::SchemaUnavailable),
            "domain_storage_unavailable"
        );
        assert_eq!(
            domain_save_error(ObservationDomainError::DuplicateName),
            "name_taken"
        );
        assert_eq!(
            domain_save_error(ObservationDomainError::InvalidStatus),
            "save_rejected"
        );
        assert_eq!(
            target_relation_error(linggan_evidence::CollectionTargetError::Database(
                sqlx::Error::PoolClosed
            )),
            "relation_storage_unavailable"
        );
        assert_eq!(
            target_relation_error(linggan_evidence::CollectionTargetError::SchemaUnavailable),
            "relation_storage_unavailable"
        );
        assert_eq!(
            target_relation_error(linggan_evidence::CollectionTargetError::UnknownTarget),
            "relation_rejected"
        );
    }
    #[test]
    fn typed_owner_rejections_allow_correcting_the_draft_without_an_unknown_lock() {
        use linggan_evidence::CollectionTargetError;
        use linggan_evidence::observation_domain::ObservationDomainError;
        for error in [
            ObservationDomainError::UnknownDomain,
            ObservationDomainError::InvalidStatus,
        ] {
            assert_eq!(domain_save_error(error), "save_rejected");
        }
        for error in [
            CollectionTargetError::UnknownTarget,
            CollectionTargetError::UnknownDomain,
            CollectionTargetError::DomainAlreadyAssigned,
            CollectionTargetError::IllegalTransition {
                from: "dismissed".to_owned(),
                to: "monitoring".to_owned(),
            },
        ] {
            assert_eq!(target_relation_error(error), "relation_rejected");
        }
    }
    #[test]
    fn station_and_nested_control_transport_failures_keep_an_unconfirmed_identity() {
        use linggan_evidence::{CollectionControlError, StationError};
        assert_eq!(
            station_command_error(
                StationError::Database(sqlx::Error::PoolClosed),
                "station_rejected"
            ),
            "station_storage_unavailable"
        );
        assert_eq!(
            station_command_error(StationError::SchemaUnavailable, "claim_window_rejected"),
            "station_storage_unavailable"
        );
        assert_eq!(
            station_command_error(
                StationError::Control(CollectionControlError::Database(sqlx::Error::PoolClosed)),
                "claim_rejected"
            ),
            "control_storage_unavailable"
        );
        assert_eq!(
            control_command_error(
                CollectionControlError::SchemaUnavailable,
                "account_binding_rejected"
            ),
            "control_storage_unavailable"
        );
        assert_eq!(
            station_command_error(StationError::InvalidDisplayName, "station_name_rejected"),
            "station_name_rejected"
        );
        assert_eq!(
            control_command_error(
                CollectionControlError::UnknownAccount,
                "account_binding_rejected"
            ),
            "account_binding_rejected"
        );
    }
}
