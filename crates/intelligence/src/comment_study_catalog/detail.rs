//! Stable-comment inspection. Current text, contextual parent and history remain distinct.
//! History collections return metadata, never unqualified prompt or Signal bodies.
use linggan_storage_postgres::Database;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

use super::{StudyCatalogError, clean, cursor, read};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentDetailQuery {
    pub domain: Uuid,
    pub work_ref: Uuid,
    pub comment_external_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CommentHistoryQuery {
    pub domain: Uuid,
    pub work_ref: Uuid,
    pub comment_external_id: String,
    pub cursor: Option<String>,
    pub limit: Option<i64>,
}

impl CommentDetailQuery {
    fn validate(&self) -> Result<(), StudyCatalogError> {
        if self.domain.to_string() != crate::comment_study_source::ADHD_DOMAIN_REF {
            return Err(StudyCatalogError::UnsupportedDomain);
        }
        if self.work_ref.is_nil()
            || self.comment_external_id.is_empty()
            || self.comment_external_id.len() > 512
            || self.comment_external_id.contains('\0')
        {
            return Err(StudyCatalogError::InvalidQuery);
        }
        Ok(())
    }

    fn key(&self) -> Value {
        json!({"workRef": self.work_ref, "commentExternalId": self.comment_external_id})
    }
}

impl CommentHistoryQuery {
    fn detail(&self) -> CommentDetailQuery {
        CommentDetailQuery {
            domain: self.domain,
            work_ref: self.work_ref,
            comment_external_id: self.comment_external_id.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum Collection {
    History,
    Versions,
}

impl Collection {
    fn resource(self) -> &'static str {
        match self {
            Self::History => "comment-history",
            Self::Versions => "comment-versions",
        }
    }

    fn sql(self) -> &'static str {
        match self {
            Self::History => include_str!("history.sql"),
            Self::Versions => include_str!("versions.sql"),
        }
    }
}

pub async fn read_comment_detail(
    database: &Database,
    query: &CommentDetailQuery,
) -> Result<Value, StudyCatalogError> {
    read_detail(database, query)
        .await
        .map_err(read::classify_read_error)
}

async fn read_detail(
    database: &Database,
    query: &CommentDetailQuery,
) -> Result<Value, StudyCatalogError> {
    query.validate()?;
    let mut tx = begin_read(database).await?;
    let as_of = read::resolve_as_of::<cursor::EntryPosition>(&mut tx, None).await?;
    let projection = read::read_one_projection(
        &mut tx,
        query.domain,
        query.work_ref,
        &query.comment_external_id,
        &as_of,
    )
    .await?;
    let rows = projection["rows"]
        .as_array()
        .ok_or(StudyCatalogError::ProjectionInvalid)?;
    let comment = match rows.first() {
        Some(row) => row
            .get("item")
            .filter(|value| value.is_object())
            .ok_or(StudyCatalogError::ProjectionInvalid)?
            .clone(),
        None => Value::Null,
    };
    let source = projection["currentSource"].clone();
    let parent = if !comment.is_null() {
        match source
            .pointer("/parentCommentKey/commentExternalId")
            .and_then(Value::as_str)
        {
            Some(id) => read_parent(&mut tx, query.work_ref, id, &as_of).await?,
            None => Value::Null,
        }
    } else {
        Value::Null
    };
    // These independently pageable metadata collections never load all historical bodies.
    let history = collection_page(&mut tx, query, Collection::History, &as_of, None, 50).await?;
    let versions = collection_page(&mut tx, query, Collection::Versions, &as_of, None, 50).await?;
    tx.commit().await?;
    Ok(json!({
        "contract": "comment-study.read.v2", "domainRef": query.domain, "asOf": as_of,
        "commentKey": query.key(), "workRef": query.work_ref, "source": source,
        "comment": comment, "parentContext": parent,
        "studyHistory": history, "materialVersions": versions,
        "indexCoverage": projection["indexCoverage"]
    }))
}

pub async fn read_comment_history(
    database: &Database,
    query: &CommentHistoryQuery,
) -> Result<Value, StudyCatalogError> {
    read_collection(database, query, Collection::History)
        .await
        .map_err(read::classify_read_error)
}

pub async fn read_comment_versions(
    database: &Database,
    query: &CommentHistoryQuery,
) -> Result<Value, StudyCatalogError> {
    read_collection(database, query, Collection::Versions)
        .await
        .map_err(read::classify_read_error)
}

async fn read_collection(
    database: &Database,
    query: &CommentHistoryQuery,
    collection: Collection,
) -> Result<Value, StudyCatalogError> {
    let detail = query.detail();
    detail.validate()?;
    let limit = query.limit.unwrap_or(50);
    if !(1..=100).contains(&limit) {
        return Err(StudyCatalogError::InvalidLimit);
    }
    let hash = collection_hash(&detail, collection)?;
    let previous = query
        .cursor
        .as_deref()
        .map(|value| cursor::decode_for::<cursor::EntryPosition>(collection.resource(), value, &hash))
        .transpose()?;
    let mut tx = begin_read(database).await?;
    let as_of = read::resolve_as_of(&mut tx, previous.as_ref()).await?;
    // A cursor grants no access. Check the domain and stable key again on every page.
    // Restrictions preserve safe history metadata, not historical source bodies.
    read::read_one_projection(
        &mut tx,
        detail.domain,
        detail.work_ref,
        &detail.comment_external_id,
        &as_of,
    )
    .await?;
    let result = collection_page(
        &mut tx,
        &detail,
        collection,
        &as_of,
        previous.as_ref().map(|value| &value.last),
        limit,
    )
    .await?;
    tx.commit().await?;
    Ok(result)
}

fn collection_hash(
    query: &CommentDetailQuery,
    collection: Collection,
) -> Result<String, StudyCatalogError> {
    cursor::scope_hash(&json!({
        "domain": query.domain, "commentKey": query.key(), "resource": collection.resource(),
        "sort": "created_desc_ref_desc.v1"
    }))
}

async fn collection_page(
    tx: &mut Transaction<'_, Postgres>,
    query: &CommentDetailQuery,
    collection: Collection,
    as_of: &str,
    last: Option<&cursor::EntryPosition>,
    limit: i64,
) -> Result<Value, StudyCatalogError> {
    let projection: Value = sqlx::query_scalar(collection.sql())
        .bind(query.domain)
        .bind(query.work_ref)
        .bind(&query.comment_external_id)
        .bind(as_of)
        .bind(last.map(|value| value.created_at.as_str()))
        .bind(last.map(|value| value.reference))
        .bind(limit + 1)
        .fetch_one(&mut **tx)
        .await?;
    let total = projection["totalCount"]
        .as_i64()
        .filter(|count| *count >= 0)
        .ok_or(StudyCatalogError::ProjectionInvalid)?;
    let mut rows = projection["rows"]
        .as_array()
        .ok_or(StudyCatalogError::ProjectionInvalid)?
        .clone();
    let has_more = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = if has_more {
        let row = rows.last().ok_or(StudyCatalogError::ProjectionInvalid)?;
        let last: cursor::EntryPosition = serde_json::from_value(row["position"].clone())
            .map_err(|_| StudyCatalogError::ProjectionInvalid)?;
        Some(cursor::encode_for(
            collection.resource(),
            &collection_hash(query, collection)?,
            as_of,
            last,
        )?)
    } else {
        None
    };
    let items: Result<Vec<Value>, StudyCatalogError> = rows
        .into_iter()
        .map(|row| {
            row.get("item")
                .filter(|value| value.is_object())
                .cloned()
                .ok_or(StudyCatalogError::ProjectionInvalid)
        })
        .collect();
    Ok(json!({
        "contract": "comment-study.read.v2", "domainRef": query.domain, "commentKey": query.key(),
        "items": items?, "totalCount": total,
        "page": {"limit": limit, "hasMore": has_more, "nextCursor": next_cursor, "asOf": as_of}
    }))
}

async fn begin_read(database: &Database) -> Result<Transaction<'_, Postgres>, StudyCatalogError> {
    let mut tx = database.pool().begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = '15s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL lock_timeout = '3s'")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn read_parent(
    tx: &mut Transaction<'_, Postgres>,
    work: Uuid,
    id: &str,
    as_of: &str,
) -> Result<Value, StudyCatalogError> {
    let row = sqlx::query(include_str!("parent.sql"))
        .bind(work)
        .bind(id)
        .bind(as_of)
        .fetch_one(&mut **tx)
        .await?;
    let source_ref: Option<Uuid> = row.try_get("source_ref")?;
    let state: String = row.try_get("source_state")?;
    let raw: Option<String> = row.try_get("raw_prefix")?;
    let cleaned = raw.as_deref().map(clean);
    let displayable = cleaned
        .as_ref()
        .is_some_and(|value| matches!(value.state.as_str(), "direct" | "context"));
    Ok(json!({
        "commentKey": {"workRef": work, "commentExternalId": id}, "sourceRef": source_ref,
        "sourceState": state,
        "commentText": if displayable { raw.as_deref() } else { None },
        "researchText": cleaned.as_ref().filter(|_| displayable).map(|value| value.text.as_str()),
        "cleanState": cleaned.as_ref().map(|value| value.state.as_str()),
        "cleanReasons": cleaned.as_ref().map(|value| &value.reasons),
        "contextOnly": true
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(id: &str) -> CommentDetailQuery {
        CommentDetailQuery {
            domain: Uuid::parse_str(crate::comment_study_source::ADHD_DOMAIN_REF).unwrap(),
            work_ref: Uuid::from_u128(1),
            comment_external_id: id.to_owned(),
        }
    }

    #[test]
    fn stable_key_is_literal_and_byte_bounded() {
        assert!(query("  评论%_  ").validate().is_ok());
        for id in [String::new(), "a\0b".into(), "中".repeat(171)] {
            assert!(query(&id).validate().is_err());
        }
        assert!(query(&"x".repeat(512)).validate().is_ok());
    }

    #[test]
    fn detail_rejects_cross_domain_and_unknown_fields() {
        let mut value = query("one");
        value.domain = Uuid::new_v4();
        assert!(matches!(value.validate(), Err(StudyCatalogError::UnsupportedDomain)));
        assert!(serde_json::from_value::<CommentDetailQuery>(json!({
            "domain": crate::comment_study_source::ADHD_DOMAIN_REF,
            "workRef": Uuid::from_u128(1), "commentExternalId": "one", "origin": "scheduled"
        })).is_err());
    }

    #[test]
    fn history_and_versions_have_separate_comment_bound_cursors() {
        let one = query("one");
        assert_ne!(
            collection_hash(&one, Collection::History).unwrap(),
            collection_hash(&one, Collection::Versions).unwrap()
        );
        assert_ne!(
            collection_hash(&one, Collection::History).unwrap(),
            collection_hash(&query("two"), Collection::History).unwrap()
        );
    }
}
