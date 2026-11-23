//! Session-bound shared draft projection and member-owned writing commands.
use super::{LocalWebState, content_workbench_api, local_read_json_error, ops_auth};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use linggan_intelligence::{content_workbench as workbench, creation_assets as assets};
use ops_auth::AuthenticatedMember;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;
use workbench::{CreateRequest, SaveRequest, WorkDocument, WorkbenchError};

const API: &str = "/api/local/ops/creation";

pub(super) fn public_member_path(method: &axum::http::Method, path: &str) -> bool {
    use axum::http::Method;
    let Some(path) = path.strip_prefix("/api/local/ops/creation/") else {
        return false;
    };
    let parts = path.split('/').collect::<Vec<_>>();
    match parts.as_slice() {
        ["domains"] => method == Method::GET,
        ["works"] => matches!(*method, Method::GET | Method::POST),
        ["works", work] => {
            Uuid::parse_str(work).is_ok() && matches!(*method, Method::GET | Method::POST)
        }
        ["accounts", account, "assets"] => {
            method == Method::GET && Uuid::parse_str(account).is_ok()
        }
        _ => false,
    }
}

pub(super) fn routes(state: &LocalWebState) -> Router<LocalWebState> {
    Router::new()
        .route(&format!("{API}/domains"), get(domains))
        .route(&format!("{API}/works"), get(list).post(create))
        .route(&format!("{API}/works/{{work}}"), get(read).post(save))
        .route(
            &format!("{API}/accounts/{{account}}/assets"),
            get(account_assets),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), member_guard))
        .layer(middleware::from_fn(
            super::comment_study::local_comment_study_guard,
        ))
}

async fn member_guard(
    State(state): State<LocalWebState>,
    mut request: Request,
    next: Next,
) -> Response {
    match ops_auth::current_member(&state, request.headers()).await {
        Ok(member) => request.extensions_mut().insert(member),
        Err(response) => return response,
    };
    next.run(request).await
}

pub(super) async fn admin_guard(
    State(state): State<LocalWebState>,
    mut request: Request,
    next: Next,
) -> Response {
    match ops_auth::current_member(&state, request.headers()).await {
        Ok(member) if member.role == "admin" => request.extensions_mut().insert(member),
        Ok(_) => return forbidden(),
        Err(response) => return response,
    };
    next.run(request).await
}

fn forbidden() -> Response {
    local_read_json_error(StatusCode::FORBIDDEN, "creation_member_forbidden")
}

fn unavailable() -> Response {
    local_read_json_error(StatusCode::SERVICE_UNAVAILABLE, "creation_not_connected")
}

pub(super) async fn transaction<'a>(
    pool: &'a PgPool,
    member: &AuthenticatedMember,
) -> Result<Transaction<'a, Postgres>, Response> {
    let mut tx = pool.begin().await.map_err(|_| unavailable())?;
    require_active(&mut tx, member).await?;
    Ok(tx)
}

pub(super) async fn require_active(
    tx: &mut Transaction<'_, Postgres>,
    member: &AuthenticatedMember,
) -> Result<(), Response> {
    match ops_auth::lock_active_member(tx, member).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(local_read_json_error(
            StatusCode::UNAUTHORIZED,
            "ops_auth_session_required",
        )),
        Err(_) => Err(unavailable()),
    }
}

type Api = Result<Json<Value>, Response>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Scope {
    domain_ref: Uuid,
    #[serde(default)]
    mine: bool,
}

async fn domains(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    let items: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('domainRef',domain_ref,'name',name) FROM observation_domain
         WHERE status='active' ORDER BY name,domain_ref",
    )
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(json!({"items":items})))
}

async fn list(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
    Query(scope): Query<Scope>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    let drafts = workbench::list_works_in(
        &mut tx,
        Some(scope.domain_ref),
        true,
        scope.mine.then_some(member.member_ref),
    )
    .await
    .map_err(content_workbench_api::error)?;
    let owners: Vec<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT work_ref, created_by_member_ref FROM linggan_creation_work WHERE work_ref=ANY($1)",
    )
    .bind(drafts.iter().map(|draft| draft.work_id).collect::<Vec<_>>())
    .fetch_all(&mut *tx)
    .await
    .map_err(|_| unavailable())?;
    let owners: std::collections::HashMap<_, _> = owners.into_iter().collect();
    let ideas = workbench::list_idea_boxes_in(&mut tx, scope.domain_ref, true)
        .await
        .map_err(content_workbench_api::error)?;
    let members = super::ops_api::member_scope(&mut *tx, &member)
        .await
        .map_err(|_| unavailable())?;
    let accounts: Vec<Value> = sqlx::query_scalar(
        "SELECT jsonb_build_object('ref',account_ref,'name',account_name,'platform',platform,'canPublish',($2 OR $3=current_responsible_member_ref) IS TRUE)
         FROM linggan_ops_account WHERE domain_ref=$1 AND ledger_state IN ('in_ledger','native_active') AND platform='xhs'
         AND ($4::uuid[] IS NULL OR current_responsible_member_ref=ANY($4))
         AND (source_system<>'linggan_native' OR native_record_state='active')
         ORDER BY account_name,account_ref",
    )
    .bind(scope.domain_ref).bind(member.role == "admin").bind(member.member_ref).bind(members.as_deref())
    .fetch_all(&mut *tx).await.map_err(|_| unavailable())?;
    let works = drafts.iter().map(|draft| {
        let owner = owners.get(&draft.work_id).copied().flatten();
        json!({
            "workId": draft.work_id, "title": draft.title, "savedAt": draft.saved_at,
            "ownership": ownership(owner, &member), "publishedRevisionId": draft.published_revision_id.filter(|_| owner == Some(member.member_ref)),
        })
    }).collect::<Vec<_>>();
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(
        json!({"works": works, "ideas": ideas, "accounts": accounts}),
    ))
}

fn ownership(owner: Option<Uuid>, member: &AuthenticatedMember) -> &'static str {
    match owner {
        None => "unknown",
        Some(owner) if owner == member.member_ref => "mine",
        Some(_) => "team",
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DraftProjection {
    title: String,
    paragraphs: Vec<workbench::Paragraph>,
    tags: Vec<String>,
    account_ref: Option<Uuid>,
    has_restricted_text: bool,
}

/// The team draft projection excludes attachments, use provenance, source excerpts and
/// account assets. Structured restricted prose is withheld for other account owners.
fn project(mut document: WorkDocument, can_read_assets: bool) -> DraftProjection {
    let mut restricted = false;
    for paragraph in &mut document.paragraphs {
        if !can_read_assets
            && document
                .experience_slots
                .iter()
                .any(|slot| slot.paragraph_id == paragraph.id)
        {
            paragraph.text.clear();
            restricted = true;
        }
    }
    restricted |= workbench::strip_member_comment_quotes(&mut document);
    DraftProjection {
        title: document.title,
        paragraphs: document.paragraphs,
        tags: document.tags,
        account_ref: document.account_ref.filter(|_| can_read_assets),
        has_restricted_text: restricted,
    }
}

fn plain(document: &WorkDocument) -> bool {
    let mut manual = document.clone();
    manual.title.clear();
    manual.paragraphs.clear();
    manual.tags.clear();
    manual.account_ref = None;
    manual == WorkDocument::default()
}

async fn owner_for(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
) -> Result<Option<Uuid>, Response> {
    sqlx::query_scalar::<_, Option<Uuid>>(
        "SELECT created_by_member_ref FROM linggan_creation_work WHERE work_ref=$1",
    )
    .bind(work)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|_| unavailable())?
    .ok_or_else(|| content_workbench_api::error(WorkbenchError::NotFound))
}

async fn account_access(
    tx: &mut Transaction<'_, Postgres>,
    member: &AuthenticatedMember,
    account: Uuid,
) -> Result<bool, Response> {
    let members = super::ops_api::member_scope(&mut **tx, member)
        .await
        .map_err(|_| unavailable())?;
    sqlx::query_scalar::<_, bool>(
        "SELECT ($2::uuid[] IS NULL OR COALESCE(current_responsible_member_ref=ANY($2),false))
         FROM linggan_ops_account WHERE account_ref=$1 FOR SHARE",
    )
    .bind(account)
    .bind(members.as_deref())
    .fetch_optional(&mut **tx)
    .await
    .map(|row| row.unwrap_or(false))
    .map_err(|_| unavailable())
}

async fn read(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
    Path(work): Path<Uuid>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    let owner = owner_for(&mut tx, work).await?;
    let (domain, mut revision) = workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(content_workbench_api::error)?;
    let account_allowed = match revision.document.account_ref {
        Some(account) => account_access(&mut tx, &member, account).await?,
        None => false,
    };
    let editable = owner == Some(member.member_ref) && plain(&revision.document);
    workbench::redact_revision_in(&mut tx, domain, &mut revision)
        .await
        .map_err(content_workbench_api::error)?;
    let draft = project(revision.document, account_allowed);
    let mut publications = Vec::new();
    if owner == Some(member.member_ref) {
        let links = assets::publication_links_in(&mut tx, work)
            .await
            .map_err(|_| unavailable())?;
        if !links.is_empty() {
            let link_refs = links
                .iter()
                .map(|link| link.publication_ref)
                .collect::<Vec<_>>();
            let visible = super::ops_api::member_scope(&mut *tx, &member)
                .await
                .map_err(|_| unavailable())?;
            let accessible: std::collections::HashSet<Uuid> = sqlx::query_scalar(
                "SELECT p.publication_ref FROM linggan_ops_publication p
                 LEFT JOIN linggan_ops_account a ON a.account_ref=p.account_ref
                 WHERE p.publication_ref=ANY($1) AND linggan_ops_publication_visible(p,a,$2,$3)",
            )
            .bind(link_refs)
            .bind(visible.as_deref())
            .bind(member.member_ref)
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| unavailable())?
            .into_iter()
            .collect();
            publications.extend(
                links
                    .into_iter()
                    .filter(|link| accessible.contains(&link.publication_ref)),
            );
        }
    }
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(json!({
        "workId": work, "domainRef": domain, "revisionId": revision.id, "draft": draft,
        "ownership": ownership(owner, &member), "editable": editable, "publications": publications,
    })))
}

async fn check_input(
    tx: &mut Transaction<'_, Postgres>,
    member: &AuthenticatedMember,
    domain: Uuid,
    document: &WorkDocument,
) -> Result<(), Response> {
    if !plain(document) {
        return Err(content_workbench_api::error(WorkbenchError::Invalid(
            "member_document",
        )));
    }
    if let Some(account) = document.account_ref {
        let can_use = sqlx::query_scalar::<_, bool>(
            "SELECT ($3 OR current_responsible_member_ref IS NOT DISTINCT FROM $4)
             FROM linggan_ops_account WHERE account_ref=$1 AND domain_ref=$2 AND platform='xhs'
             AND ledger_state IN ('in_ledger','native_active') AND (source_system<>'linggan_native' OR native_record_state='active') FOR SHARE",
        )
        .bind(account).bind(domain).bind(member.role == "admin").bind(member.member_ref)
        .fetch_optional(&mut **tx).await.map_err(|_| unavailable())?.unwrap_or(false);
        if !can_use {
            return Err(forbidden());
        }
    }
    Ok(())
}

async fn create(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
    Json(body): Json<CreateRequest>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    check_input(&mut tx, &member, body.domain_ref, &body.document).await?;
    let receipt = workbench::create_work_as_in(&mut tx, &body, Some(member.member_ref))
        .await
        .map_err(content_workbench_api::error)?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(json!(receipt)))
}

async fn save(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
    Path(work): Path<Uuid>,
    Json(body): Json<SaveRequest>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    if owner_for(&mut tx, work).await? != Some(member.member_ref) {
        return Err(forbidden());
    }
    // Match the workbench revision advisory lock, so a concurrent admin change cannot be discarded
    // between inspecting the current document shape and applying a member's plain-text change.
    workbench::lock_save_in(&mut tx, work, body.request_id)
        .await
        .map_err(content_workbench_api::error)?;
    let (domain, current) = workbench::read_revision_in(&mut tx, work, None)
        .await
        .map_err(content_workbench_api::error)?;
    if !plain(&current.document) {
        return Err(forbidden());
    }
    check_input(&mut tx, &member, domain, &body.document).await?;
    let receipt = workbench::save_as_in(&mut tx, work, &body, Some(member.member_ref))
        .await
        .map_err(content_workbench_api::error)?;
    tx.commit().await.map_err(|_| unavailable())?;
    if receipt.outcome == "stale" {
        return Err(local_read_json_error(
            StatusCode::CONFLICT,
            "creation_stale",
        ));
    }
    Ok(Json(json!(receipt)))
}

async fn account_assets(
    State(state): State<LocalWebState>,
    Extension(member): Extension<AuthenticatedMember>,
    Path(account): Path<Uuid>,
) -> Api {
    let db = state.database.database().ok_or_else(unavailable)?;
    let mut tx = transaction(db.pool(), &member).await?;
    if !account_access(&mut tx, &member, account).await? {
        return Err(forbidden());
    }
    let identity = assets::read_identity_in(&mut tx, account)
        .await
        .map_err(|_| unavailable())?;
    let persona_facts = assets::list_persona_facts_in(&mut tx, account)
        .await
        .map_err(|_| unavailable())?;
    let experiences = assets::list_experiences_in(&mut tx, account)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(Json(
        json!({"identity": identity, "personaFacts": persona_facts, "experiences": experiences}),
    ))
}
