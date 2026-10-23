//! A single full-note read contract; explicit commands are separate from read-only views.
use super::*;
use axum::http::{StatusCode, header};
use linggan_evidence::{material_reading, research_dossier as dossier};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Params {
    domain: Option<Uuid>,
    cursor: Option<Uuid>,
    as_of: Option<String>,
    format: Option<String>,
    kinds: Option<String>,
    unit_refs: Option<String>,
    block_refs: Option<String>,
    image_pages: Option<String>,
    sections: Option<String>,
}

async fn ensure_scope(
    db: &linggan_storage_postgres::Database,
    work: Uuid,
    domain: Option<Uuid>,
) -> Result<(), Response> {
    let exists:Result<bool,_>=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_content c WHERE c.public_ref=$1 AND ($2::uuid IS NULL OR EXISTS(SELECT 1 FROM linggan_material_domain_usage u WHERE u.content_public_ref=c.public_ref AND u.domain_ref=$2)))")
        .bind(work).bind(domain).fetch_one(db.pool()).await;
    match exists {
        Ok(true) => Ok(()),
        Ok(false) => Err(local_read_json_error(
            StatusCode::NOT_FOUND,
            "material_not_found",
        )),
        Err(_) => Err(unavailable()),
    }
}
fn unavailable() -> Response {
    local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "dossier_read_unavailable")
}
fn read_error(error: sqlx::Error) -> Response {
    match error {
        sqlx::Error::Protocol(reason) if reason == "future_cutoff" => {
            local_read_json_error(StatusCode::BAD_REQUEST, "future_cutoff")
        }
        sqlx::Error::Database(error)
            if matches!(error.code().as_deref(), Some("22007" | "22008")) =>
        {
            local_read_json_error(StatusCode::BAD_REQUEST, "invalid_cutoff")
        }
        _ => unavailable(),
    }
}
fn parts(value: Option<&str>) -> Result<Vec<String>, Response> {
    let Some(value) = value else {
        return Ok(vec![]);
    };
    let parts: Vec<_> = value.split(',').map(str::trim).collect();
    if parts.iter().any(|v| v.is_empty()) {
        return Err(local_read_json_error(
            StatusCode::BAD_REQUEST,
            "empty_selection_value",
        ));
    }
    Ok(parts.into_iter().map(str::to_owned).collect())
}
fn selection(params: &Params) -> Result<dossier::Selection, Response> {
    Ok(dossier::Selection {
        kinds: parts(params.kinds.as_deref())?,
        unit_refs: parts(params.unit_refs.as_deref())?,
        block_refs: parts(params.block_refs.as_deref())?,
        image_pages: parts(params.image_pages.as_deref())?
            .iter()
            .map(|p| {
                p.parse::<u64>().map_err(|_| {
                    local_read_json_error(StatusCode::BAD_REQUEST, "invalid_image_page")
                })
            })
            .collect::<Result<_, _>>()?,
    })
}

pub(super) async fn read(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "read_model_not_connected");
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    if !matches!(params.format.as_deref(), None | Some("json" | "markdown")) {
        return local_read_json_error(StatusCode::BAD_REQUEST, "invalid_format");
    }
    let selection = match selection(&params) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match dossier::read_at(db, work, params.as_of.as_deref()).await {
        Ok(Some(mut value)) => {
            let plan = dossier::relayer_plan(&value);
            if let Err(reason) = dossier::select(&mut value, &selection) {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"operation":"local_read","outcome":"unavailable","code":reason})),
                )
                    .into_response();
            }
            if params.format.as_deref() == Some("markdown") {
                (
                    [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                    dossier::markdown(&value),
                )
                    .into_response()
            } else {
                Json(json!({"dossier":value,"relayerPlan":plan})).into_response()
            }
        }
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(error) => read_error(error),
    }
}

pub(super) async fn candidates(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    match material_reading::read_candidates(db, work).await {
        Ok(candidates) => Json(json!({"candidates":candidates})).into_response(),
        Err(error) => reading_error(error),
    }
}

pub(super) async fn review(
    State(state): State<LocalWebState>,
    Path((work, unit)): Path<(Uuid, Uuid)>,
    Query(params): Query<Params>,
    Json(input): Json<material_reading::ReviewReadingRequest>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    let belongs:Result<bool,_>=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM linggan_material_unit WHERE unit_ref=$1 AND content_public_ref=$2)")
        .bind(unit).bind(work).fetch_one(db.pool()).await;
    match belongs {
        Ok(true) => {}
        Ok(false) => return local_read_json_error(StatusCode::NOT_FOUND, "reading_not_found"),
        Err(_) => return unavailable(),
    }
    match material_reading::review_reading(db, unit, &input, None).await {
        Ok(receipt) => Json(receipt).into_response(),
        Err(error) => reading_error(error),
    }
}
fn reading_error(error: material_reading::ReadingError) -> Response {
    use material_reading::ReadingError as E;
    let (status, reason) = match error {
        E::Invalid => (StatusCode::BAD_REQUEST, "invalid_reading_command"),
        E::NotFound => (StatusCode::NOT_FOUND, "reading_not_found"),
        E::Conflict => (StatusCode::CONFLICT, "reading_revision_conflict"),
        E::SourceChanged => (StatusCode::CONFLICT, "reading_source_changed"),
        E::Storage(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "reading_storage_unavailable",
        ),
    };
    local_read_json_error(status, reason)
}

pub(super) async fn version(
    State(state): State<LocalWebState>,
    Path((work, reading)): Path<(Uuid, Uuid)>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    match dossier::read_version(db, work, reading).await {
        Ok(Some(value)) => Json(json!({"workRef":work,"material":value})).into_response(),
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "reading_not_found"),
        Err(error) => read_error(error),
    }
}

pub(super) async fn metadata(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    let sections = match parts(params.sections.as_deref()) {
        Ok(v)
            if !v.is_empty()
                && v.iter()
                    .all(|s| matches!(s.as_str(), "author" | "interaction")) =>
        {
            v
        }
        _ => return local_read_json_error(StatusCode::BAD_REQUEST, "metadata_sections_required"),
    };
    match dossier::read_at(db, work, params.as_of.as_deref()).await {
        Ok(Some(value)) => {
            let mut selected = serde_json::Map::new();
            for section in sections {
                selected.insert(section.clone(), value.metadata[&section].clone());
            }
            Json(json!({"workRef":work,"asOf":value.as_of,"metadata":selected})).into_response()
        }
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(error) => read_error(error),
    }
}

pub(super) async fn comments(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let Some(domain) = params.domain else {
        return local_read_json_error(StatusCode::BAD_REQUEST, "domain_required");
    };
    if let Err(response) = ensure_scope(db, work, Some(domain)).await {
        return response;
    }
    match linggan_evidence::read_dossier_comments(db, work, params.cursor).await {
        Ok(value) => Json(value).into_response(),
        Err(_) => unavailable(),
    }
}

pub(super) async fn plan(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Query(params): Query<Params>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, params.domain).await {
        return response;
    }
    match dossier::plan(db, work).await {
        Ok(Some(value)) => Json(json!({"relayerPlan":value})).into_response(),
        Ok(None) => local_read_json_error(StatusCode::NOT_FOUND, "material_not_found"),
        Err(error) => read_error(error),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Relayer {
    expected_revision: String,
    domain: Option<Uuid>,
}
pub(super) async fn relayer(
    State(state): State<LocalWebState>,
    Path(work): Path<Uuid>,
    Json(input): Json<Relayer>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    if let Err(response) = ensure_scope(db, work, input.domain).await {
        return response;
    }
    match dossier::relayer_work(db, work, &input.expected_revision).await {
        Ok(value) => Json(value).into_response(),
        Err(linggan_evidence::ProducerRuntimeError::MaterialIdentityConflict) => {
            local_read_json_error(StatusCode::CONFLICT, "dossier_changed_or_scope_unavailable")
        }
        Err(_) => local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "dossier_relayer_failed"),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Batch {
    domain: Uuid,
    work_refs: Vec<Uuid>,
    as_of: Option<String>,
    #[serde(default)]
    selection: dossier::Selection,
}
pub(super) async fn batch(
    State(state): State<LocalWebState>,
    Json(input): Json<Batch>,
) -> Response {
    let Some(db) = state.database.database() else {
        return unavailable();
    };
    let unique: std::collections::BTreeSet<_> = input.work_refs.iter().collect();
    if unique.is_empty() || unique.len() != input.work_refs.len() {
        return local_read_json_error(
            StatusCode::BAD_REQUEST,
            "unique_nonempty_work_refs_required",
        );
    }
    match batch_read(db, &input).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => read_error(error),
    }
}
async fn batch_read(
    db: &linggan_storage_postgres::Database,
    input: &Batch,
) -> Result<Value, sqlx::Error> {
    let mut tx = db.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let cutoff = dossier::cutoff_in(&mut tx, input.as_of.as_deref()).await?;
    let mut results = vec![];
    for work in &input.work_refs {
        if !dossier::creation_work_in_domain(&mut tx, *work, input.domain).await? {
            results.push(json!({"workRef":work,"state":"not_found"}));
            continue;
        }
        // A failed work query cannot leave the surrounding batch transaction aborted.
        sqlx::query("SAVEPOINT dossier_work")
            .execute(&mut *tx)
            .await?;
        match dossier::read_in(&mut tx, &[*work], &cutoff).await {
            Ok(mut values) => match values.remove(work) {
                Some(mut value) => match dossier::select(&mut value, &input.selection) {
                    Ok(()) => {
                        results.push(json!({"workRef":work,"state":"available","dossier":value}))
                    }
                    Err(reason) => results
                        .push(json!({"workRef":work,"state":"invalid_selection","reason":reason})),
                },
                None => results.push(json!({"workRef":work,"state":"not_found"})),
            },
            Err(_) => {
                sqlx::query("ROLLBACK TO SAVEPOINT dossier_work")
                    .execute(&mut *tx)
                    .await?;
                results.push(json!({"workRef":work,"state":"unavailable"}));
            }
        }
        sqlx::query("RELEASE SAVEPOINT dossier_work")
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(json!({"contract":dossier::VERSION,"asOf":cutoff,"results":results}))
}
