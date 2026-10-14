//! Material preview and explicit local re-layering; neither path launches research.
use super::*;
use axum::http::StatusCode;
use linggan_evidence::research_dossier as dossier;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub(super) struct Params {
    domain: Option<uuid::Uuid>,
    cursor: Option<uuid::Uuid>,
}

async fn scoped(
    database: &linggan_storage_postgres::Database,
    work: uuid::Uuid,
    domain: Option<uuid::Uuid>,
) -> Result<bool, sqlx::Error> {
    match domain {
        Some(domain)=>sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_domain_usage WHERE content_public_ref=$1 AND domain_ref=$2)").bind(work).bind(domain).fetch_one(database.pool()).await,
        None=>Ok(true),
    }
}

pub(super) async fn read(
    State(state): State<LocalWebState>,
    Path(work): Path<uuid::Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    match scoped(database, work, params.domain).await {
        Ok(true) => {}
        Ok(false) => return local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dossier_read_unavailable",
            );
        }
    }
    match dossier::read(database, work).await {
        Ok(Some(value)) => {
            let creator = dossier::bounded_fragments(&value.fragments, 4000);
            let comment = linggan_intelligence::comment_study_run::bounded_context_manifest(
                &json!({
                    "workRef":work,"dossierContract":value.contract,"dossierRevision":value.material_revision,
                    "cleanerVersion":linggan_intelligence::comment_study_run::context_cleaner_version(),
                    "sources":dossier::context_sources(&value)
                }),
                20000,
            );
            let total: usize = value.fragments.iter().map(|f| f.end).sum();
            let included: usize = creator.iter().map(|f| f.end).sum();
            Json(json!({"dossier":value,"relayerPlan":dossier::relayer_plan(&value),"markdown":dossier::markdown(&value),"inputs":{
                "creator":{"markdown":dossier::render_materials(&creator),"stage":"work_materials","characterBudget":4000,"includedCharacterCount":included,"omittedCharacterCount":total-included,"truncated":included<total,"fragments":creator},
                "comments":comment,
                "topics":{"markdown":dossier::render_materials(&value.fragments),"stage":"before_research_windowing","includedCharacterCount":total,"truncated":false,"fragments":value.fragments}
            }})).into_response()
        }
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "dossier_read_unavailable")
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Relayer {
    expected_revision: String,
    domain: Option<uuid::Uuid>,
}

pub(super) async fn relayer(
    State(state): State<LocalWebState>,
    Path(work): Path<uuid::Uuid>,
    Json(input): Json<Relayer>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    match scoped(database, work, input.domain).await {
        Ok(true) => {}
        Ok(false) => return local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dossier_read_unavailable",
            );
        }
    }
    match dossier::relayer_work(database, work, &input.expected_revision).await {
        Ok(result) => Json(result).into_response(),
        Err(linggan_evidence::ProducerRuntimeError::MaterialIdentityConflict) => {
            local_read_json_error(StatusCode::CONFLICT, "dossier_changed_or_scope_unavailable")
        }
        Err(_) => local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "dossier_relayer_failed"),
    }
}

pub(super) async fn script() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        concat!(
            include_str!("research_dossier.js"),
            "\n",
            include_str!("research_dossier_batch.js")
        ),
    )
        .into_response()
}

pub(super) async fn comments(
    State(state): State<LocalWebState>,
    Path(work): Path<uuid::Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(database) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let Some(domain) = params.domain else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "domain_required");
    };
    match scoped(database, work, Some(domain)).await {
        Ok(true) => {}
        Ok(false) => return local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dossier_read_unavailable",
            );
        }
    }
    match linggan_evidence::read_dossier_comments(database, work, params.cursor).await {
        Ok(page) => Json(page).into_response(),
        Err(_) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "material_comment_channel_unavailable",
        ),
    }
}

/// Same final input fitter as creator dispatch. Reading does not reserve tokens or enqueue work.
pub(super) async fn creator_preview(
    State(state): State<LocalWebState>,
    Path(work): Path<uuid::Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    let Some(domain) = params.domain else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "domain_required");
    };
    match prepare_creator_preview(db, domain, work).await {
        Ok(value) => Json(value).into_response(),
        Err(_) => local_read_json_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "creator_preview_unavailable",
        ),
    }
}

async fn prepare_creator_preview(
    db: &linggan_storage_postgres::Database,
    domain: uuid::Uuid,
    work: uuid::Uuid,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    use sqlx::Row;
    let platform:Option<String>=sqlx::query_scalar("SELECT content.platform FROM linggan_material_content content WHERE public_ref=$1 AND EXISTS(SELECT 1 FROM linggan_material_domain_usage usage WHERE usage.content_public_ref=$1 AND usage.domain_ref=$2)").bind(work).bind(domain).fetch_optional(db.pool()).await?;
    let Some(platform) = platform else {
        return Ok(json!({"available":false,"reason":"material_not_found"}));
    };
    let scope = serde_json::from_value(json!({"domain":domain,"platform":platform}))?;
    let data =
        linggan_evidence::creator_discovery::load_creator_material_for_work(db, &scope, work)
            .await?;
    let Some(work) = data.works.first() else {
        return Ok(json!({"available":false,"reason":"material_not_found"}));
    };
    let config=sqlx::query("SELECT config.config_ref,config.input_token_limit FROM linggan_creator_discovery_policy policy JOIN linggan_model_config config ON config.config_ref=policy.config_ref WHERE policy.domain_ref=$1 AND policy.platform=$2").bind(domain).bind(&platform).fetch_optional(db.pool()).await?;
    let Some(config) = config else {
        return Ok(
            json!({"available":false,"reason":"model_not_configured","markdown":dossier::render_materials(&work.fragments)}),
        );
    };
    let limit = config.get::<i32, _>("input_token_limit") as usize;
    match linggan_intelligence::creator_discovery_input::prepare(&data.domain, work, limit) {
        Ok((input, fragments)) => Ok(
            json!({"available":true,"stage":"prepared_request","configRef":config.get::<uuid::Uuid,_>("config_ref"),"inputTokenLimit":limit,
            "system":linggan_intelligence::creator_discovery_analysis::SYSTEM,"input":input,"fragments":fragments,
            "estimatedInputTokensUpperBound":input.to_string().len()+linggan_intelligence::creator_discovery_analysis::SYSTEM.len()+128,
            "asOf":data.as_of,"sourceFingerprint":work.fingerprint}),
        ),
        Err(_) => Ok(json!({"available":false,"reason":"configured_input_limit_too_small"})),
    }
}

pub(super) async fn plan(
    State(state): State<LocalWebState>,
    Path(work): Path<uuid::Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    match scoped(db, work, params.domain).await {
        Ok(true) => {}
        Ok(false) => return local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            return local_read_json_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "dossier_read_unavailable",
            );
        }
    }
    match dossier::plan(db, work).await {
        Ok(Some(value)) => Json(json!({"relayerPlan":value})).into_response(),
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(_) => {
            local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "dossier_read_unavailable")
        }
    }
}
