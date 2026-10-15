//! Read-only card projection; semantic extraction and identity remain in intelligence.
//! The time baseline bounds additions, not a historical source-permission snapshot.
use super::LocalWebState;
use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CardsQuery {
    domain: Option<Uuid>,
    q: Option<String>,
    view: Option<String>,
    order: Option<String>,
    limit: Option<i64>,
    cursor: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CardsCursor {
    version: u8,
    domain: Uuid,
    q: String,
    view: String,
    order: String,
    baseline: i64,
    scope_revision: String,
    key: i64,
    problem: Uuid,
}

struct ValidatedQuery {
    domain: Uuid,
    q: String,
    view: String,
    order: String,
    limit: i64,
    cursor: Option<CardsCursor>,
}

impl CardsQuery {
    fn validated(&self) -> Result<ValidatedQuery, ()> {
        let domain = self.domain.ok_or(())?;
        let q = self.q.as_deref().unwrap_or("").trim().to_owned();
        let view = self.view.as_deref().unwrap_or("all").to_owned();
        let order = self.order.as_deref().unwrap_or("recent").to_owned();
        let limit = self.limit.unwrap_or(12);
        if q.chars().count() > 300
            || !matches!(view.as_str(), "all" | "changes")
            || !matches!(order.as_str(), "recent" | "created" | "support")
            || !(1..=50).contains(&limit)
        {
            return Err(());
        }
        let cursor = self
            .cursor
            .as_deref()
            .map(|value| {
                if value.len() > 8192 {
                    return Err(());
                }
                let cursor: CardsCursor = serde_json::from_str(value).map_err(|_| ())?;
                if cursor.version != 1
                    || cursor.domain != domain
                    || cursor.q != q
                    || cursor.view != view
                    || cursor.order != order
                    || cursor.baseline <= 0
                    || cursor.key < 0
                    || cursor.scope_revision.len() != 32
                    || !cursor.scope_revision.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(());
                }
                Ok(cursor)
            })
            .transpose()?;
        Ok(ValidatedQuery {
            domain,
            q,
            view,
            order,
            limit,
            cursor,
        })
    }
}

pub(super) fn routes() -> Router<LocalWebState> {
    Router::new()
        .route("/assets/comment-study-results.css", get(styles))
        .route("/assets/comment-study-results.js", get(script))
        .route("/api/local/comment-study/problem-cards", get(cards))
}

async fn styles() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("comment_study_results.css"),
    )
}

async fn script() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("comment_study_results.js"),
    )
}

async fn cards(
    State(state): State<LocalWebState>,
    query: Result<Query<CardsQuery>, QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return invalid();
    };
    let Ok(ValidatedQuery {
        domain,
        q,
        view,
        order,
        limit,
        cursor,
    }) = query.validated()
    else {
        return invalid();
    };
    let Some(database) = super::database(&state) else {
        return unavailable();
    };
    let result: Result<Value, sqlx::Error> = async {
        let mut tx = database.pool().begin().await?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *tx)
            .await?;
        sqlx::query("SET LOCAL statement_timeout='8s'")
            .execute(&mut *tx)
            .await?;
        let now: i64 = sqlx::query_scalar(
            "SELECT floor(extract(epoch FROM statement_timestamp())*1000000)::bigint",
        )
        .fetch_one(&mut *tx)
        .await?;
        let baseline = cursor.as_ref().map_or(now, |value| value.baseline);
        if baseline > now {
            return Ok(json!({"invalidCursor":true}));
        }
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM observation_domain WHERE domain_ref=$1)",
        )
        .bind(domain)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Ok(json!({"missingDomain":true}));
        }
        let search = q
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let mut body: Value = sqlx::query_scalar(include_str!("comment_study_problem_cards.sql"))
            .bind(domain)
            .bind(baseline)
            .bind(search)
            .bind(&view)
            .bind(&order)
            .bind(cursor.as_ref().map(|value| value.key))
            .bind(cursor.as_ref().map(|value| value.problem))
            .bind(limit + 1)
            .fetch_one(&mut *tx)
            .await?;
        let scope_revision = body["scopeRevision"]
            .as_str()
            .ok_or(sqlx::Error::RowNotFound)?
            .to_owned();
        // Current eligibility can change inside an addition baseline. A changed ordering
        // invalidates the cursor instead of silently duplicating or skipping problems.
        if cursor
            .as_ref()
            .is_some_and(|value| value.scope_revision != scope_revision)
        {
            return Ok(json!({"changedScope":true}));
        }
        let items = body["problems"]
            .as_array_mut()
            .ok_or(sqlx::Error::RowNotFound)?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next = if more {
            let last = items.last().ok_or(sqlx::Error::RowNotFound)?;
            let problem = last["problemRef"]
                .as_str()
                .and_then(|v| Uuid::parse_str(v).ok())
                .ok_or(sqlx::Error::RowNotFound)?;
            let key = last["sortKey"].as_i64().ok_or(sqlx::Error::RowNotFound)?;
            Some(
                serde_json::to_string(&CardsCursor {
                    version: 1,
                    domain,
                    q,
                    view,
                    order,
                    baseline,
                    scope_revision,
                    key,
                    problem,
                })
                .map_err(|_| sqlx::Error::RowNotFound)?,
            )
        } else {
            None
        };
        for item in items {
            if let Some(object) = item.as_object_mut() {
                object.remove("sortKey");
            }
        }
        body["contract"] = json!("comment-study.problem-cards.v1");
        body["domainRef"] = json!(domain);
        body["page"] = json!({
            "limit":limit,"hasMore":more,"nextCursor":next,
            "baselineMicros":baseline,"recentDays":28
        });
        tx.commit().await?;
        Ok(body)
    }
    .await;
    match result {
        Ok(body) if body["invalidCursor"] == true => invalid(),
        Ok(body) if body["changedScope"] == true => super::error(
            StatusCode::CONFLICT,
            "problem_cards_changed",
            "研究结果或来源资格已变化，请从第一页重新读取。",
            false,
        ),
        Ok(body) if body["missingDomain"] == true => super::error(
            StatusCode::NOT_FOUND,
            "domain_not_found",
            "当前领域不可读取。",
            false,
        ),
        Ok(body) => Json(body).into_response(),
        Err(_) => unavailable(),
    }
}

fn invalid() -> Response {
    super::error(
        StatusCode::BAD_REQUEST,
        "invalid_problem_cards_query",
        "筛选或翻页位置已失效，请从第一页重新读取。",
        false,
    )
}

fn unavailable() -> Response {
    super::error(
        StatusCode::SERVICE_UNAVAILABLE,
        "problem_cards_unavailable",
        "问题列表暂时无法读取，已有研究数据未被删除。",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cards_query_requires_domain_and_known_scope() {
        assert!(CardsQuery::default().validated().is_err());
        let mut query = CardsQuery {
            domain: Some(Uuid::nil()),
            ..Default::default()
        };
        assert!(query.validated().is_ok());
        query.view = Some("trending".into());
        assert!(query.validated().is_err());
        query.view = None;
        query.limit = Some(51);
        assert!(query.validated().is_err());
    }

    #[test]
    fn cursor_is_scoped_and_does_not_authorize_other_domains() {
        let cursor = CardsCursor {
            version: 1,
            domain: Uuid::nil(),
            q: "".into(),
            view: "all".into(),
            order: "recent".into(),
            baseline: 1,
            scope_revision: "a".repeat(32),
            key: 0,
            problem: Uuid::nil(),
        };
        let mut query = CardsQuery {
            domain: Some(Uuid::nil()),
            cursor: Some(serde_json::to_string(&cursor).unwrap()),
            ..Default::default()
        };
        assert!(query.validated().is_ok());
        query.q = Some("changed".into());
        assert!(query.validated().is_err());
        query.q = None;
        query.domain = Some(Uuid::new_v4());
        assert!(query.validated().is_err());
    }
}
