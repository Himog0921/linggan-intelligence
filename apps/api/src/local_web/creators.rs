//! Corpus creator discovery composition. All reads remain free of provider/collection effects.
use super::*;
use axum::routing::put;
use linggan_contracts::creator_discovery::CreatorScope;
use linggan_evidence::creator_discovery::{self as discovery, DiscoveryError};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/corpus/creators", get(page))
        .route("/assets/creators.css", get(css))
        .route("/assets/creators.js", get(js))
        .route("/api/local/creators", get(list))
        .route(
            "/api/local/creators/{key}/observation-target",
            post(register),
        )
        .route("/api/local/creator-discovery-policy", put(policy))
        .route(
            "/api/local/creator-discovery-overrides",
            put(override_field),
        )
        .route("/api/local/creator-discovery-analysis/retry", post(retry))
        .layer(axum::middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}
fn error(e: DiscoveryError) -> Response {
    let (status, code) = match e {
        DiscoveryError::Invalid("policy_revision_conflict") => {
            (axum::http::StatusCode::CONFLICT, "policy_revision_conflict")
        }
        DiscoveryError::Invalid("active_primary_domain_conflict") => {
            (axum::http::StatusCode::CONFLICT, "active_primary_domain_conflict")
        }
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
#[derive(Deserialize)]
struct PageQuery {
    domain: Option<Uuid>,
}
async fn page(State(state): State<LocalWebState>, Query(q): Query<PageQuery>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    let domains = match linggan_evidence::observation_domain::read_observation_domains(db).await {
        Ok(v) => v,
        Err(_) => return error(DiscoveryError::Invalid("domain_read_unavailable")),
    };
    let selected = q
        .domain
        .and_then(|id| domains.iter().find(|d| d.domain_ref == id));
    let picker = super::corpus_domain_picker(&domains, selected, "/corpus/creators", None);
    let header = shell::global_header(
        shell::PrimarySurface::Corpus,
        "库内作品的作者",
        &format!("语料 / {picker} / <b>创作者</b>"),
        "<span id=\"creator-header-readout\" class=\"creator-header-readout\" aria-live=\"polite\">读取中…</span>",
        None,
    );
    let nav = shell::corpus_side_nav(
        shell::CorpusPage::Creators,
        q.domain.map(|x| x.to_string()).as_deref(),
        "作者来自当前领域已有作品",
    );
    let domain_value = selected.map(|d| d.domain_ref.to_string()).unwrap_or_default();
    Html(page_markup(&header, &nav, &domain_value)).into_response()
}
fn page_markup(header: &str, nav: &str, domain_value: &str) -> String {
    include_str!("creators.html")
        .replace("{{HEADER}}", header)
        .replace("{{SIDE_NAV}}", nav)
        .replace("{{DOMAIN_VALUE}}", domain_value)
}

#[cfg(test)]
mod page_tests {
    use super::page_markup;

    #[test]
    fn creator_page_keeps_one_server_selected_domain_and_one_complete_pager() {
        let domain = uuid::Uuid::new_v4().to_string();
        let html = page_markup(
            "<header id=\"creator-header-readout\"></header>",
            "<nav></nav>",
            &domain,
        );
        assert!(html.contains(&format!("name=\"domain\" value=\"{domain}\"")));
        assert!(page_markup("", "", "").contains("name=\"domain\" value=\"\""));
        assert!(!html.contains("<select name=\"domain\""));
        assert_eq!(html.matches("data-page-nav").count(), 1);
        for control in [
            "data-page-prev",
            "data-page-next",
            "data-page-jump",
            "id=\"page-size\"",
        ] {
            assert!(html.contains(control), "missing pager control: {control}");
        }
        assert!(html.contains("id=\"creator-results\""));
    }
}
async fn css() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        format!(
            "{}\n{}\n{}",
            super::LIDS_TOKENS,
            super::SHELL_CSS,
            include_str!("creators.css")
        ),
    )
}
async fn js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("creators.js"),
    )
}
async fn list(State(state): State<LocalWebState>, Query(q): Query<CreatorScope>) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "read_model_not_connected",
        );
    };
    match discovery::load(db, &q).await {
        Ok(data) => Json(discovery::aggregate(&data, &q)).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Register {
    domain: Uuid,
    #[serde(default="primary_role")]
    usage_role: String,
}
fn primary_role()->String{"primary".into()}
async fn register(
    State(state): State<LocalWebState>,
    Path(key): Path<String>,
    Json(body): Json<Register>,
) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    match linggan_evidence::register_discovered_creator(db, body.domain, &key, &body.usage_role).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Policy {
    domain: Uuid,
    like_threshold: Option<i64>,
    revision: i64,
    analysis_enabled: Option<bool>,
    config_ref: Option<Uuid>,
    daily_token_limit: Option<i64>,
}
async fn policy(State(state): State<LocalWebState>, Json(body): Json<Policy>) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    let result = async {
        if body.like_threshold.is_some_and(|n| n <= 0) {
            return Err(DiscoveryError::Invalid("invalid_like_threshold"));
        }
        if body.daily_token_limit.is_some_and(|n| !(1024..=10_000_000).contains(&n)) {
            return Err(DiscoveryError::Invalid("invalid_daily_token_limit"));
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
        sqlx::query("INSERT INTO linggan_creator_discovery_policy(domain_ref,platform,revision) VALUES($1,'xhs',0) ON CONFLICT DO NOTHING")
            .bind(body.domain).execute(&mut *tx).await?;
        let existing = sqlx::query("SELECT revision,analysis_enabled,config_ref,daily_token_limit FROM linggan_creator_discovery_policy WHERE domain_ref=$1 AND platform='xhs' FOR UPDATE")
            .bind(body.domain).fetch_one(&mut *tx).await?;
        if existing.get::<i64, _>("revision") != body.revision {
            return Err(DiscoveryError::Invalid("policy_revision_conflict"));
        }
        let was_enabled: bool = existing.get("analysis_enabled");
        let enabled = body.analysis_enabled.unwrap_or(was_enabled);
        let config = body.config_ref.or(existing.get("config_ref"));
        let limit = body.daily_token_limit.or(existing.get("daily_token_limit"));
        if enabled {
            if config.is_none() {
                return Err(DiscoveryError::Invalid("analysis_config_required"));
            }
            if limit.is_none() {
                return Err(DiscoveryError::Invalid("daily_token_limit_required"));
            }
        }
        if let Some(config) = config {
            let known: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_model_config WHERE config_ref=$1)")
                .bind(config).fetch_one(&mut *tx).await?;
            if !known {
                return Err(DiscoveryError::Invalid("analysis_config_not_found"));
            }
        }
        let row = sqlx::query("UPDATE linggan_creator_discovery_policy SET like_threshold=$2,revision=revision+1,analysis_enabled=$4,config_ref=$5,daily_token_limit=$6,updated_at=scope_001_now() WHERE domain_ref=$1 AND platform='xhs' AND revision=$3 RETURNING revision")
            .bind(body.domain).bind(body.like_threshold).bind(body.revision)
            .bind(enabled).bind(config).bind(limit).fetch_one(&mut *tx).await?;
        if enabled && !was_enabled {
            sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='queued',attempt_count=0,next_attempt_at=scope_001_now() WHERE domain_ref=$1 AND job_state='paused'")
                .bind(body.domain).execute(&mut *tx).await?;
            sqlx::query("UPDATE linggan_creator_discovery_author_analysis SET job_state='queued',attempt_count=0,next_attempt_at=scope_001_now() WHERE domain_ref=$1 AND job_state='paused'")
                .bind(body.domain).execute(&mut *tx).await?;
        }
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
   sqlx::query("INSERT INTO linggan_creator_discovery_author_analysis(domain_ref,platform,author_external_id,requested_fingerprint,manual_overrides,job_state) VALUES($1,$2,$3,'manual',CASE WHEN $4 THEN '{}'::jsonb ELSE jsonb_build_object($6::text,$5::jsonb) END,'idle') ON CONFLICT(domain_ref,platform,author_external_id) DO UPDATE SET manual_overrides=CASE WHEN $4 THEN linggan_creator_discovery_author_analysis.manual_overrides-$6::text ELSE linggan_creator_discovery_author_analysis.manual_overrides||jsonb_build_object($6::text,$5::jsonb) END,updated_at=scope_001_now()").bind(body.domain).bind(&w.platform).bind(&w.author_external_id).bind(body.value.is_none()).bind(annotation).bind(&body.field).execute(&mut *tx).await?;
  }else{
   let w=data.works.iter().find(|w|Some(w.work_ref)==body.work_ref).ok_or(DiscoveryError::NotFound)?;
   if body.creator_key.as_deref().is_some_and(|key|w.creator_key.as_deref()!=Some(key)){return Err(DiscoveryError::Invalid("creator_work_mismatch"));}
   sqlx::query("INSERT INTO linggan_creator_discovery_work_analysis(domain_ref,work_public_ref,requested_fingerprint,manual_overrides,job_state) VALUES($1,$2,$3,CASE WHEN $5 THEN '{}'::jsonb ELSE jsonb_build_object($4::text,$6::jsonb) END,'queued') ON CONFLICT(domain_ref,work_public_ref) DO UPDATE SET manual_overrides=CASE WHEN $5 THEN linggan_creator_discovery_work_analysis.manual_overrides-$4::text ELSE linggan_creator_discovery_work_analysis.manual_overrides||jsonb_build_object($4::text,$6::jsonb) END,updated_at=scope_001_now()").bind(body.domain).bind(w.work_ref).bind(&w.fingerprint).bind(&body.field).bind(body.value.is_none()).bind(annotation).execute(&mut *tx).await?;
  }
  tx.commit().await?;
  Ok::<_,DiscoveryError>(json!({"saved":true}))
 }.await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Retry {
    domain: Uuid,
    work_ref: Option<Uuid>,
    creator_key: Option<String>,
}
async fn retry(State(state): State<LocalWebState>, Json(body): Json<Retry>) -> Response {
    let Some(db) = state.database.database() else {
        return error(DiscoveryError::Invalid("read_model_not_connected"));
    };
    if body.work_ref.is_some() == body.creator_key.is_some() {
        return error(DiscoveryError::Invalid("one_retry_scope_required"));
    }
    let result=async {
        let mut tx=db.pool().begin().await?;
        let status:Option<String>=sqlx::query_scalar("SELECT status FROM observation_domain WHERE domain_ref=$1 FOR SHARE").bind(body.domain).fetch_optional(&mut *tx).await?;
        if status.as_deref()!=Some("active"){return Err(DiscoveryError::Invalid("domain_paused_or_missing"));}
        let analysis_enabled:bool=sqlx::query_scalar("SELECT COALESCE((SELECT analysis_enabled FROM linggan_creator_discovery_policy WHERE domain_ref=$1 AND platform='xhs'),false)").bind(body.domain).fetch_one(&mut *tx).await?;
        if !analysis_enabled {return Err(DiscoveryError::Invalid("analysis_disabled"));}
        let affected=if let Some(work)=body.work_ref {
            sqlx::query("UPDATE linggan_creator_discovery_work_analysis SET job_state='queued',attempt_count=0,next_attempt_at=scope_001_now(),last_error_code=NULL WHERE domain_ref=$1 AND work_public_ref=$2 AND job_state IN('failed','paused')")
                .bind(body.domain).bind(work).execute(&mut *tx).await?.rows_affected()
        }else{
            let (platform,author)=body.creator_key.as_deref().and_then(linggan_contracts::creator_discovery::decode_creator_key).ok_or(DiscoveryError::Invalid("invalid_creator_key"))?;
            sqlx::query("UPDATE linggan_creator_discovery_author_analysis SET job_state='queued',attempt_count=0,next_attempt_at=scope_001_now(),last_error_code=NULL WHERE domain_ref=$1 AND platform=$2 AND author_external_id=$3 AND job_state IN('failed','paused')")
                .bind(body.domain).bind(platform).bind(author).execute(&mut *tx).await?.rows_affected()
        };
        tx.commit().await?;
        Ok::<_,DiscoveryError>(json!({"queued":affected==1}))
    }.await;
    match result {
        Ok(v) => Json(v).into_response(),
        Err(e) => error(e),
    }
}
