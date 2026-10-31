//! Corpus creator discovery JSON API. All reads remain free of provider/collection effects.
//!
//! 页面已迁到 React 应用（`apps/web`，WEB-REACT-001 阶段 D1），由 `web_app.rs` 提供；这里只剩读写接口。
use super::*;
use axum::routing::put;
use linggan_contracts::creator_discovery::CreatorScope;
use linggan_evidence::creator_discovery::{self as discovery, DiscoveryError};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/creators", get(list))
        .route(
            "/api/local/creators/{key}/preview",
            get(preview).post(start_preview),
        )
        .route(
            "/api/local/creator-account-research/preview",
            get(account_preview),
        )
        .route("/api/local/creator-account-research", post(account_start))
        .route(
            "/api/local/creator-account-research/{request}",
            get(account_read),
        )
        .route("/api/local/creator-research/preview", get(research_preview))
        .route("/api/local/creator-research", post(research_start))
        .route("/api/local/creator-research/{request}", get(research_read))
        .route(
            "/api/local/creators/{key}/observation-target",
            post(register),
        )
        .route(
            "/api/local/creators/{key}/observation-record",
            get(observation_record),
        )
        .route("/api/local/creator-discovery-policy", put(policy))
        .route(
            "/api/local/creator-discovery-overrides",
            put(override_field),
        )
        .route(
            "/api/local/creator-discovery-analysis/retry",
            post(|| async {
                crate::local_web::ai_settings::legacy_closed("legacy_creator_discovery")
            }),
        )
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AccountPreview {
    domain_ref: Uuid,
    platform: String,
    author_external_id: String,
}
async fn account_preview(
    State(state): State<LocalWebState>,
    Query(query): Query<AccountPreview>,
) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_account_research::preview(
            db,
            query.domain_ref,
            &query.platform,
            &query.author_external_id,
        )
        .await,
    )
}
async fn account_start(
    State(state): State<LocalWebState>,
    Json(body): Json<linggan_intelligence::ai_foundation::creator_account_research::Start>,
) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_account_research::start(db, &body).await,
    )
}
async fn account_read(State(state): State<LocalWebState>, Path(request): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_account_research::read(db, request).await,
    )
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResearchPreview {
    domain_ref: Uuid,
    work_ref: Uuid,
}
async fn research_preview(
    State(state): State<LocalWebState>,
    Query(query): Query<ResearchPreview>,
) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_research::preview(
            db,
            query.domain_ref,
            query.work_ref,
        )
        .await,
    )
}
async fn research_start(
    State(state): State<LocalWebState>,
    Json(body): Json<linggan_intelligence::ai_foundation::creator_research::Start>,
) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_research::start(db, &body).await,
    )
}
async fn research_read(State(state): State<LocalWebState>, Path(request): Path<Uuid>) -> Response {
    let Some(db) = state.database.database() else {
        return super::ai_settings::unavailable();
    };
    super::ai_settings::respond(
        linggan_intelligence::ai_foundation::creator_research::read(db, request).await,
    )
}
fn error(e: DiscoveryError) -> Response {
    let (status, code) = match e {
        DiscoveryError::Invalid("policy_revision_conflict") => {
            (axum::http::StatusCode::CONFLICT, "policy_revision_conflict")
        }
        DiscoveryError::Invalid("active_primary_domain_conflict") => (
            axum::http::StatusCode::CONFLICT,
            "active_primary_domain_conflict",
        ),
        DiscoveryError::Invalid(code) => (axum::http::StatusCode::BAD_REQUEST, code),
        DiscoveryError::NotFound => (
            axum::http::StatusCode::NOT_FOUND,
            "creator_resource_not_found",
        ),
        DiscoveryError::Database(_) => (
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "creator_read_unavailable",
        ),
    };
    local_read_json_error(status, code)
}
async fn list(State(state): State<LocalWebState>, Query(q): Query<CreatorScope>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let started = std::time::Instant::now();
    match discovery::load_list_timed(db, &q).await {
        Ok((data, timing)) => {
            let response_started = std::time::Instant::now();
            let mut response = Json(data).into_response();
            let phases = format!(
                "{}, response;dur={:.2}, total;dur={:.2}",
                timing.server_timing(),
                response_started.elapsed().as_secs_f64() * 1000.0,
                started.elapsed().as_secs_f64() * 1000.0,
            );
            if let Ok(value) = axum::http::HeaderValue::from_str(&phases) {
                response.headers_mut().insert("server-timing", value);
            }
            response
        }
        Err(e) => {
            #[cfg(test)]
            if std::env::var("LINGGAN_MODEL_SYNTHETIC_PREVIEW").as_deref()
                == Ok("SYNTHETIC-NOT-EVIDENCE")
            {
                eprintln!("Synthetic creator read failed: {e:?}");
            }
            error(e)
        }
    }
}

#[derive(Deserialize)]
struct PreviewQuery {
    domain: Uuid,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreviewRequest {
    domain: Uuid,
    command_ref: Uuid,
}
async fn start_preview(
    State(state): State<LocalWebState>,
    Path(key): Path<String>,
    Json(body): Json<PreviewRequest>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Some((platform, author)) = linggan_contracts::creator_discovery::decode_creator_key(&key)
    else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_creator_key");
    };
    if platform != "xhs" {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "unsupported_creator_platform",
        );
    }
    let request = linggan_evidence::collection_run::CreateRun {
        idempotency_key: body.command_ref,
        domain_ref: body.domain,
        source: linggan_evidence::collection_run::RunSource::CreatorPreview {
            author_external_id: author,
        },
        ranking: None,
        work_limit: 30,
        comment_mode: linggan_evidence::collection_run::CommentMode::None,
        comment_target: None,
    };
    match linggan_evidence::collection_run::create_run(db, &request).await {
        Ok(mut value) => {
            value["domainRef"] = json!(body.domain);
            Json(value).into_response()
        }
        Err(linggan_evidence::collection_run::RunError::Invalid(code)) => {
            local_read_json_error(axum::http::StatusCode::UNPROCESSABLE_ENTITY, code)
        }
        Err(linggan_evidence::collection_run::RunError::Conflict) => local_read_json_error(
            axum::http::StatusCode::CONFLICT,
            "creator_preview_request_conflict",
        ),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "creator_preview_start_unavailable",
        ),
    }
}
async fn preview(
    State(state): State<LocalWebState>,
    Path(key): Path<String>,
    Query(params): Query<PreviewQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let Some((platform, author)) = linggan_contracts::creator_discovery::decode_creator_key(&key)
    else {
        return local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "invalid_creator_key");
    };
    if platform != "xhs" {
        return local_read_json_error(
            axum::http::StatusCode::BAD_REQUEST,
            "unsupported_creator_platform",
        );
    }
    let row = sqlx::query("SELECT run_ref,phase,control_intent,revision,work_limit,scope_frozen_at::text AS frozen_at,discovery_reason FROM collection_run WHERE domain_ref=$1 AND source_kind='creator_preview' AND search_query=$2 ORDER BY created_at DESC,run_ref DESC LIMIT 1")
        .bind(params.domain).bind(author).fetch_optional(db.pool()).await;
    let row = match row {
        Ok(Some(value)) => value,
        Ok(None) => return Json(json!({"state":"not_requested","runRef":null,"requestedLimit":30,"sampleCount":0,"works":[]})).into_response(),
        Err(_) => return local_read_json_error(axum::http::StatusCode::SERVICE_UNAVAILABLE, "creator_preview_unavailable"),
    };
    let run_ref: Uuid = row.get("run_ref");
    let refs:Result<Vec<(Uuid,i32)>,_>=sqlx::query_as("SELECT content_public_ref,ordinal FROM collection_run_work WHERE run_ref=$1 ORDER BY ordinal LIMIT 30")
        .bind(run_ref).fetch_all(db.pool()).await;
    let Ok(refs) = refs else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "creator_preview_unavailable",
        );
    };
    // Whether a card is already formal domain-research material comes from the server, never from the
    // page's current filter: it labels the card and seeds the default suggestion.
    let frozen: Vec<Uuid> = refs.iter().map(|(work_ref, _)| *work_ref).collect();
    let known: Result<Vec<Uuid>, _> = sqlx::query_scalar("SELECT DISTINCT content_public_ref FROM linggan_material_domain_usage WHERE domain_ref=$1 AND use_kind='domain_research' AND content_public_ref=ANY($2)")
        .bind(params.domain).bind(&frozen).fetch_all(db.pool()).await;
    let Ok(known) = known else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "creator_preview_unavailable",
        );
    };
    let mut works = Vec::new();
    for (work_ref, ordinal) in refs {
        // A card that cannot be read right now is counted in `frozenCount` but not listed. A read that
        // fails outright is not "no card": answer unavailable instead of a silently shorter sample.
        let work = match linggan_evidence::read_work_resource(db, work_ref).await {
            Ok(Some(work)) => work,
            Ok(None) => continue,
            Err(_) => {
                return local_read_json_error(
                    axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    "creator_preview_unavailable",
                );
            }
        };
        let source = serde_json::to_value(work).unwrap_or(json!({}));
        works.push(
            json!({"workRef":work_ref,"ordinal":ordinal,"inDomain":known.contains(&work_ref),
            "title":source["display"]["title"],"likes":source["display"]["engagement"]["likeCount"],
            "comments":source["display"]["engagement"]["commentCount"],
            "collects":source["display"]["engagement"]["collectCount"]}),
        );
    }
    let phase: String = row.get("phase");
    let control: String = row.get("control_intent");
    let snapshot = if control == "stopped" {
        "stopped"
    } else if phase == "ended" {
        "complete"
    } else if phase == "blocked" {
        "partial"
    } else {
        "running"
    };
    Json(
        json!({"state":snapshot,"runRef":run_ref,"revision":row.get::<i32,_>("revision"),
        "requestedLimit":row.get::<i32,_>("work_limit"),"sampleCount":works.len(),"frozenCount":frozen.len(),
        "scopeFrozenAt":row.get::<Option<String>,_>("frozen_at"),
        "stopReason":row.get::<Option<String>,_>("discovery_reason"),"works":works}),
    )
    .into_response()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Register {
    domain: Uuid,
    #[serde(default = "primary_role")]
    usage_role: String,
}
fn primary_role() -> String {
    "primary".into()
}
async fn register(
    State(state): State<LocalWebState>,
    Path(key): Path<String>,
    Json(body): Json<Register>,
) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    match linggan_evidence::register_discovered_creator(db, body.domain, &key, &body.usage_role)
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ObservationRecordQuery {
    domain: Uuid,
}
/// 创作者在本领域的观察记录：加入时间、用途、档案与巡查的真实时间线。
/// 没有关系时返回 `not_joined`；读不到库时返回不可用，绝不用空记录冒充「没观察过」。
async fn observation_record(
    State(state): State<LocalWebState>,
    Path(key): Path<String>,
    Query(params): Query<ObservationRecordQuery>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    match linggan_evidence::read_creator_observation_record(db, params.domain, &key).await {
        Ok(Some(record)) => Json(json!({
            "state": "joined",
            "targetRef": record.target_ref,
            "joinedAt": record.joined_at,
            "usageRole": record.usage_role,
            "firstStoredAt": record.first_stored_at,
            "lifecycleState": record.lifecycle_state,
            "lifecycleChangedAt": record.lifecycle_changed_at,
            "monitoringState": record.monitoring_state,
            "patrol": {
                "automaticEnabled": record.automatic_enabled,
                "lastDispatchedAt": record.last_dispatched_at,
                "lastSucceededAt": record.last_succeeded_at,
                "nextRunAt": record.next_run_at,
                "missedRuns": record.missed_runs,
                "latestHits": record.latest_hits,
                "latestNew": record.latest_new,
                "state": record.patrol_state,
            },
            "lastTransition": record.last_transition.map(|value| json!({
                "at": value.occurred_at,
                "from": value.from_state,
                "to": value.to_state,
                "reasonCode": value.reason_code,
                "reason": value.reason,
            })),
        }))
        .into_response(),
        Ok(None) => Json(json!({"state": "not_joined", "targetRef": null})).into_response(),
        Err(_) => local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "observation_record_unavailable",
        ),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy {
    domain: Uuid,
    like_threshold: Option<i64>,
    revision: i64,
}
async fn policy(State(state): State<LocalWebState>, Json(body): Json<Policy>) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    let result = async {
        if body.like_threshold.is_some_and(|n| n <= 0) {
            return Err(DiscoveryError::Invalid("invalid_like_threshold"));
        }
        let mut tx = db.pool().begin().await?;
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM observation_domain WHERE domain_ref=$1 FOR SHARE",
        )
        .bind(body.domain)
        .fetch_optional(&mut *tx)
        .await?;
        if status.as_deref() != Some("active") {
            return Err(DiscoveryError::Invalid("domain_paused_or_missing"));
        }
        sqlx::query("INSERT INTO linggan_creator_discovery_threshold(domain_ref,platform,revision) VALUES($1,'xhs',0) ON CONFLICT DO NOTHING")
            .bind(body.domain).execute(&mut *tx).await?;
        let existing = sqlx::query("SELECT revision FROM linggan_creator_discovery_threshold WHERE domain_ref=$1 AND platform='xhs' FOR UPDATE")
            .bind(body.domain).fetch_one(&mut *tx).await?;
        if existing.get::<i64, _>("revision") != body.revision {
            return Err(DiscoveryError::Invalid("policy_revision_conflict"));
        }
        let row = sqlx::query("UPDATE linggan_creator_discovery_threshold SET like_threshold=$2,revision=revision+1,updated_at=scope_001_now() WHERE domain_ref=$1 AND platform='xhs' AND revision=$3 RETURNING revision")
            .bind(body.domain).bind(body.like_threshold).bind(body.revision).fetch_one(&mut *tx).await?;
        tx.commit().await?;
        Ok::<_, DiscoveryError>(json!({"revision":row.get::<i64, _>("revision"),"saved":true}))
    }.await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Override {
    domain: Uuid,
    work_ref: Option<Uuid>,
    creator_key: Option<String>,
    field: String,
    value: Option<String>,
    reason: Option<String>,
    #[serde(default)]
    support_fragment_ids: Vec<String>,
}
async fn override_field(
    State(state): State<LocalWebState>,
    Json(body): Json<Override>,
) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    let result=async{
  if body.reason.as_ref().is_some_and(|v|v.chars().count()>300){return Err(DiscoveryError::Invalid("reason_too_long"));}
  let allowed=match body.field.as_str(){"relevance"=>vec!["related","unrelated","unknown"],"personal_experience"|"professional_output"|"explicit_promotion"|"institution_or_brand"=>vec!["yes","no","unknown"],"focus"=>vec!["vertical_tendency","multi_topic","unknown"],_=>return Err(DiscoveryError::Invalid("invalid_override_field"))};
  if body.value.as_deref().is_some_and(|v|!allowed.contains(&v)){return Err(DiscoveryError::Invalid("invalid_override_value"));}
  let q:CreatorScope=serde_json::from_value(json!({"domain":body.domain})).map_err(|_|DiscoveryError::Invalid("invalid_domain"))?;
  let mut data=discovery::load(db,&q).await?;let mut reference=q;reference.usage_role="reference".into();data.works.extend(discovery::load(db,&reference).await?.works);
  let positive=body.value.as_deref().is_some_and(|v|discovery::is_positive_override(&body.field,v));
  if body.support_fragment_ids.len()>40 {return Err(DiscoveryError::Invalid("too_many_support_fragments"));}
  let support=if body.field=="focus" || body.field=="institution_or_brand" {
      let key=body.creator_key.as_deref().ok_or(DiscoveryError::Invalid("creator_key_required"))?;
      let works=data.works.iter().filter(|w|w.creator_key.as_deref()==Some(key)).collect::<Vec<_>>();
      if works.is_empty(){return Err(DiscoveryError::NotFound);}
      discovery::author_support_fragments(&works,&body.field)
  }else{
      let w=data.works.iter().find(|w|Some(w.work_ref)==body.work_ref).ok_or(DiscoveryError::NotFound)?;
      if body.creator_key.as_deref().is_some_and(|key|w.creator_key.as_deref()!=Some(key)){return Err(DiscoveryError::Invalid("creator_work_mismatch"));}
      discovery::work_support_fragments(w)
  };
  let valid:std::collections::HashSet<_>=support.iter().filter_map(|f|f["fragmentId"].as_str()).collect();
  if body.support_fragment_ids.iter().any(|id|!valid.contains(id.as_str())) || (positive && body.support_fragment_ids.is_empty()) {
      return Err(DiscoveryError::Invalid("invalid_support_fragment"));
  }
  let annotation=json!({"value":{"value":body.value,"reason":body.reason.as_deref().map(str::trim).filter(|s|!s.is_empty()),"evidenceFragmentIds":body.support_fragment_ids},"supportFragmentIds":body.support_fragment_ids,"actor":"local_user","at":data.as_of});
  let mut tx=db.pool().begin().await?;
  let status:Option<String>=sqlx::query_scalar("SELECT status FROM observation_domain WHERE domain_ref=$1 FOR SHARE").bind(body.domain).fetch_optional(&mut *tx).await?;
  if status.as_deref()!=Some("active"){return Err(DiscoveryError::Invalid("domain_paused_or_missing"));}
  if body.field=="focus" || body.field=="institution_or_brand"{
   let key=body.creator_key.as_deref().ok_or(DiscoveryError::Invalid("creator_key_required"))?;let w=data.works.iter().find(|w|w.creator_key.as_deref()==Some(key)).ok_or(DiscoveryError::NotFound)?;
   sqlx::query("INSERT INTO linggan_creator_author_override(domain_ref,platform,author_external_id,manual_overrides) VALUES($1,$2,$3,CASE WHEN $4 THEN '{}'::jsonb ELSE jsonb_build_object($6::text,$5::jsonb) END) ON CONFLICT(domain_ref,platform,author_external_id) DO UPDATE SET manual_overrides=CASE WHEN $4 THEN linggan_creator_author_override.manual_overrides-$6::text ELSE linggan_creator_author_override.manual_overrides||jsonb_build_object($6::text,$5::jsonb) END,updated_at=scope_001_now()").bind(body.domain).bind(&w.platform).bind(&w.author_external_id).bind(body.value.is_none()).bind(annotation).bind(&body.field).execute(&mut *tx).await?;
  }else{
   let w=data.works.iter().find(|w|Some(w.work_ref)==body.work_ref).ok_or(DiscoveryError::NotFound)?;
   if body.creator_key.as_deref().is_some_and(|key|w.creator_key.as_deref()!=Some(key)){return Err(DiscoveryError::Invalid("creator_work_mismatch"));}
   sqlx::query("INSERT INTO linggan_creator_work_override(domain_ref,work_public_ref,manual_overrides) VALUES($1,$2,CASE WHEN $4 THEN '{}'::jsonb ELSE jsonb_build_object($3::text,$5::jsonb) END) ON CONFLICT(domain_ref,work_public_ref) DO UPDATE SET manual_overrides=CASE WHEN $4 THEN linggan_creator_work_override.manual_overrides-$3::text ELSE linggan_creator_work_override.manual_overrides||jsonb_build_object($3::text,$5::jsonb) END,updated_at=scope_001_now()").bind(body.domain).bind(w.work_ref).bind(&body.field).bind(body.value.is_none()).bind(annotation).execute(&mut *tx).await?;
  }
  tx.commit().await?;
  Ok::<_,DiscoveryError>(json!({"saved":true}))
 }.await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
