//! OPS-HTTP-001 read endpoints behind the 「运营」 pages: JSON lists, exports and historical
//! covers. All GET; the caller (`ops::routes`) applies the local read guard.
use super::*;
use linggan_intelligence::ops_ledger::read::{
    OpsBusiness, OpsPublicationFilter, OpsReadError, read_ops_accounts, read_ops_as_of,
    read_ops_businesses, read_ops_daily, read_ops_publication_export, read_ops_publications,
    read_ops_summary,
};
use serde::Deserialize;

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/api/local/ops/businesses", get(businesses))
        .route("/api/local/ops/summary", get(summary))
        .route("/api/local/ops/publications", get(publications))
        .route("/api/local/ops/accounts", get(accounts))
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

fn read_error(error: OpsReadError) -> Response {
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

fn not_connected() -> Response {
    local_read_json_error(
        axum::http::StatusCode::SERVICE_UNAVAILABLE,
        "read_model_not_connected",
    )
}

fn json<T: serde::Serialize>(result: Result<T, OpsReadError>) -> Response {
    match result {
        Ok(value) => Json(value).into_response(),
        Err(error) => read_error(error),
    }
}

async fn businesses(State(state): State<LocalWebState>) -> Response {
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    json(read_ops_businesses(database).await)
}

async fn summary(
    State(state): State<LocalWebState>,
    Query(query): Query<super::ops::BusinessQuery>,
) -> Response {
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => json(read_ops_summary(database, business).await),
        Err(error) => read_error(error),
    }
}

async fn publications(
    State(state): State<LocalWebState>,
    Query(query): Query<ListQuery>,
) -> Response {
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
    json(read_ops_publications(database, business, filter).await)
}

async fn accounts(State(state): State<LocalWebState>, Query(query): Query<ListQuery>) -> Response {
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => {
            json(read_ops_accounts(database, business, query.operator.as_deref()).await)
        }
        Err(error) => read_error(error),
    }
}

async fn daily(State(state): State<LocalWebState>, Query(query): Query<ListQuery>) -> Response {
    let Some(database) = state.database.database() else {
        return not_connected();
    };
    match OpsBusiness::parse(query.domain.as_deref()) {
        Ok(business) => {
            json(read_ops_daily(database, business, query.operator.as_deref(), None).await)
        }
        Err(error) => read_error(error),
    }
}

/// 导出：当前业务下的全部记录，列与页面一致，附来源记录号与数据截至时间。
async fn export(State(state): State<LocalWebState>, Query(query): Query<ExportQuery>) -> Response {
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
        "accounts" => read_ops_accounts(database, business, None)
            .await
            .map(|page| super::ops_export::accounts(&page.items)),
        "publications" => read_ops_publication_export(database, business)
            .await
            .map(|items| super::ops_export::publications(&items)),
        "daily" => read_ops_daily(database, business, None, Some(i64::from(i32::MAX)))
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
    Path(publication_ref): Path<String>,
) -> Response {
    let Ok(publication_ref) = uuid::Uuid::parse_str(&publication_ref) else {
        return local_read_json_error(axum::http::StatusCode::NOT_FOUND, "local_media_not_found");
    };
    local_asset_delivery::deliver(
        state,
        local_asset_delivery::QualifiedAssetRequest::OpsCover { publication_ref },
    )
    .await
}
