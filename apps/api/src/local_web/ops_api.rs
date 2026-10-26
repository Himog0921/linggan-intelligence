//! OPS-HTTP-001 read endpoints behind the 「运营」 pages: JSON lists, exports and historical
//! covers. All GET; the caller (`ops::routes`) applies the local read guard.
use super::*;
use axum::http::HeaderMap;
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, OpsPublicationFilter, OpsReadError, read_ops_accounts_scoped, read_ops_as_of,
    read_ops_businesses_scoped, read_ops_daily_scoped, read_ops_publication_export_scoped,
    read_ops_publications_scoped, read_ops_summary_scoped,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ops/businesses", get(businesses))
        .route("/api/local/ops/summary", get(summary))
        .route("/api/local/ops/publications", get(publications))
        .route("/api/local/ops/accounts", get(accounts))
        .route("/api/local/ops/members", get(members))
        .route("/api/local/ops/daily", get(daily))
        .route("/api/local/ops/export", get(export))
        .route("/ops/covers/{publication_ref}", get(cover))
}

#[derive(Deserialize)]
struct ListQuery {
    domain: Option<String>,
    operator: Option<String>,
    platform: Option<String>,
    account: Option<String>,
    from: Option<String>,
    to: Option<String>,
    views: Option<String>,
    page: Option<i64>,
}

#[derive(Deserialize)]
struct ExportQuery {
    domain: Option<String>,
    kind: Option<String>,
    format: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ActiveOpsMember {
    member_ref: Uuid,
    display_name: String,
    role: String,
}

pub(super) fn read_error(error: OpsReadError) -> Response {
    match error {
        OpsReadError::InvalidParameter(_) => {
            local_read_json_error(axum::http::StatusCode::BAD_REQUEST, "ops_parameter_invalid")
        }
        OpsReadError::BusinessNotFound => {
            local_read_json_error(axum::http::StatusCode::NOT_FOUND, "ops_business_not_found")
        }
        OpsReadError::Database(error) => {
            eprintln!(
                "ops read unavailable: database_kind={:?}",
                error.as_database_error().and_then(|value| value.code())
            );
            local_read_json_error(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "ops_read_unavailable",
            )
        }
    }
}

pub(super) fn not_connected() -> Response {
    local_read_json_error(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "read_model_not_connected",
    )
}

pub(super) fn json<T: serde::Serialize>(result: Result<T, OpsReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => read_error(error),
    }
}

/// The authenticated member's present visibility, reconstructed from current group events.
/// Request query parameters and historical source names never grant visibility.
async fn read_scope(
    state: &LocalWebState,
    headers: &HeaderMap,
) -> Result<Option<Vec<Uuid>>, Response> {
    let member = super::ops_auth::current_member(state, headers).await?;
    if member.role == "admin" {
        return Ok(None);
    }
    if member.role != "lead" {
        return Ok(Some(vec![member.member_ref]));
    }
    let Some(database) = state.database.database() else {
        return Err(not_connected());
    };
    let members: Vec<Uuid> = sqlx::query_scalar(
        "WITH latest AS (SELECT DISTINCT ON (member_ref) member_ref,group_ref,action \
                           FROM linggan_ops_group_membership_event \
                          ORDER BY member_ref,occurred_at DESC,event_ref DESC) \
         SELECT member.member_ref FROM latest lead_group \
         JOIN linggan_ops_group group_row ON group_row.group_ref=lead_group.group_ref AND group_row.state='active' \
         JOIN latest current_group ON current_group.group_ref=lead_group.group_ref AND current_group.action='joined' \
         JOIN linggan_ops_member member ON member.member_ref=current_group.member_ref AND member.state='active' \
         WHERE lead_group.member_ref=$1 AND lead_group.action='joined' ORDER BY member.member_ref",
    )
    .bind(member.member_ref)
    .fetch_all(database.pool())
    .await
    .map_err(|error| read_error(OpsReadError::Database(error)))?;
    Ok(Some(if members.is_empty() {
        vec![member.member_ref]
    } else {
        members
    }))
}

async fn businesses(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    json(read_ops_businesses_scoped(database, scope.as_deref()).await)
}

async fn summary(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<super::ops::BusinessQuery>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => json(read_ops_summary_scoped(database, business, scope.as_deref()).await),
        Err(error) => read_error(error),
    }
}

async fn publications(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    let business = match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => business,
        Err(error) => return read_error(error),
    };
    let pending_views = match query.views.as_deref() {
        None | Some("") => false,
        Some("pending") => true,
        Some(_) => return read_error(OpsReadError::InvalidParameter("views")),
    };
    let filter = OpsPublicationFilter {
        operator: query.operator,
        platform: query.platform,
        account: query.account,
        from: query.from,
        to: query.to,
        pending_views,
        page: query.page.unwrap_or(1),
    };
    json(read_ops_publications_scoped(database, business, filter, scope.as_deref()).await)
}

async fn accounts(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => json(
            read_ops_accounts_scoped(
                database,
                business,
                query.operator.as_deref(),
                scope.as_deref(),
            )
            .await,
        ),
        Err(error) => read_error(error),
    }
}

/// The whole Operations team can read the active internal roster.  This endpoint is only a
/// selector source; every transfer still rechecks the actor, target state and group boundary in
/// the command transaction.
async fn members(State(state): State<LocalWebState>, headers: HeaderMap) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    let members: Result<Vec<(Uuid, String, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT member_ref,display_name,role FROM linggan_ops_member \
         WHERE state='active' AND ($1::uuid[] IS NULL OR member_ref=ANY($1)) ORDER BY display_name,member_ref",
    )
    .bind(scope.as_deref())
    .fetch_all(database.pool())
    .await;
    match members {
        Ok(members) => Json(
            members
                .into_iter()
                .map(|(member_ref, display_name, role)| ActiveOpsMember {
                    member_ref,
                    display_name,
                    role,
                })
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => read_error(OpsReadError::Database(error)),
    }
}

async fn daily(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<ListQuery>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => json(
            read_ops_daily_scoped(
                database,
                business,
                query.operator.as_deref(),
                None,
                scope.as_deref(),
            )
            .await,
        ),
        Err(error) => read_error(error),
    }
}

/// 导出：当前业务下的全部记录，列与页面一致，附来源记录号与数据截至时间。
async fn export(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Query(query): Query<ExportQuery>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    let business = match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => business,
        Err(error) => return read_error(error),
    };
    let (Some(kind), Some(format)) = (query.kind.as_deref(), query.format.as_deref()) else {
        return read_error(OpsReadError::InvalidParameter("kind"));
    };
    if format != "csv" && format != "json" {
        return read_error(OpsReadError::InvalidParameter("format"));
    }
    let as_of = match read_ops_as_of(database).await {
        Ok(as_of) => as_of,
        Err(error) => return read_error(error),
    };
    let table = match kind {
        "accounts" => read_ops_accounts_scoped(database, business, None, scope.as_deref())
            .await
            .map(|page| super::ops_export::accounts(&page.items)),
        "publications" => read_ops_publication_export_scoped(database, business, scope.as_deref())
            .await
            .map(|items| super::ops_export::publications(&items)),
        "daily" => read_ops_daily_scoped(
            database,
            business,
            None,
            Some(i64::from(i32::MAX)),
            scope.as_deref(),
        )
        .await
        .map(|page| super::ops_export::daily(&page.items)),
        _ => return read_error(OpsReadError::InvalidParameter("kind")),
    };
    match table {
        Ok(table) => super::ops_export::response(kind, format, &table, as_of.as_ref()),
        Err(error) => read_error(error),
    }
}

/// Historical cover bytes, only for files registered to this publication.
async fn cover(
    State(state): State<LocalWebState>,
    headers: HeaderMap,
    Path(publication_ref): Path<String>,
) -> Response {
    let scope = match read_scope(&state, &headers).await {
        Ok(scope) => scope,
        Err(response) => return response,
    };
    let Ok(publication_ref) = uuid::Uuid::parse_str(&publication_ref) else {
        return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found");
    };
    if let Some(member_refs) = scope {
        let Some(database) = state.database.database() else {
            return not_connected();
        };
        let visible: Result<bool, sqlx::Error> = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM linggan_ops_publication p \
             LEFT JOIN linggan_ops_account account ON account.account_ref=p.account_ref \
             WHERE p.publication_ref=$1 AND ((p.source_system='linggan_native' AND (p.native_publisher_member_ref=ANY($2) OR account.current_responsible_member_ref=ANY($2))) \
               OR (p.source_system='content_workbench' AND (SELECT resolution.resolved_member_ref \
                    FROM linggan_ops_historical_owner_resolution resolution WHERE resolution.publication_ref=p.publication_ref \
                    ORDER BY resolution.decided_at DESC,resolution.resolution_ref DESC LIMIT 1)=ANY($2))))",
        ).bind(publication_ref).bind(&member_refs).fetch_one(database.pool()).await;
        match visible {
            Ok(true) => {}
            Ok(false) => {
                return local_read_json_error(
                    axum::http::StatusCode::NOT_FOUND,
                    "local_media_not_found",
                );
            }
            Err(error) => return read_error(OpsReadError::Database(error)),
        }
    }
    local_asset_delivery::deliver(
        state,
        local_asset_delivery::QualifiedAssetRequest::OpsCover { publication_ref },
    )
    .await
}
