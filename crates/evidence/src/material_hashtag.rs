//! One optional source field, independently projectable without replaying a detail or another lane.
use crate::creator_discovery::hash;
use serde_json::{Map, Value};
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub(crate) const VERSION: &str = "content-hashtags.v1";

pub(crate) struct Hashtags<'a> {
    pub state: &'static str,
    pub tags: Vec<&'a str>,
    pub source_hash: String,
}

pub(crate) fn parse(payload: &Map<String, Value>) -> Hashtags<'_> {
    let source = payload.get("hashtags");
    let source_hash = hash(&serde_json::json!([source.is_some(), source]).to_string());
    let Some(value) = source else {
        return Hashtags {
            state: "not_provided",
            tags: vec![],
            source_hash,
        };
    };
    let tags = value.as_array().and_then(|items| {
        items
            .iter()
            .map(|item| item.as_str().filter(|s| !s.trim().is_empty()))
            .collect::<Option<Vec<_>>>()
    });
    match tags {
        Some(tags) => Hashtags {
            state: "provided",
            tags,
            source_hash,
        },
        None => Hashtags {
            state: "invalid",
            tags: vec![],
            source_hash,
        },
    }
}

pub(crate) async fn project(
    tx: &mut Transaction<'_, Postgres>,
    material: Uuid,
    payload: &Map<String, Value>,
) -> Result<bool, sqlx::Error> {
    let parsed = parse(payload);
    let old = sqlx::query("SELECT source_hash FROM linggan_material_hashtag_projection WHERE material_ref=$1 AND projection_contract=$2")
        .bind(material).bind(VERSION).fetch_optional(&mut **tx).await?;
    if let Some(row) = old {
        if row.get::<String, _>("source_hash") != parsed.source_hash {
            return Err(sqlx::Error::Protocol("hashtag_source_changed".into()));
        }
        return Ok(false);
    }
    sqlx::query("INSERT INTO linggan_material_hashtag_projection(material_ref,projection_contract,state,tag_count,source_hash) VALUES($1,$2,$3,$4,$5)")
        .bind(material).bind(VERSION).bind(parsed.state).bind(i32::try_from(parsed.tags.len()).map_err(|_| sqlx::Error::Protocol("too_many_hashtags".into()))?)
        .bind(&parsed.source_hash).execute(&mut **tx).await?;
    for (ordinal, tag) in parsed.tags.iter().enumerate() {
        sqlx::query("INSERT INTO linggan_material_content_hashtag(material_ref,projection_contract,ordinal,tag_text) VALUES($1,$2,$3,$4)")
            .bind(material).bind(VERSION).bind(i32::try_from(ordinal).expect("count was validated"))
            .bind(tag).execute(&mut **tx).await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn absence_empty_and_malformed_are_different_facts() {
        for (payload, state, count) in [
            (json!({}), "not_provided", 0),
            (json!({"hashtags":[]}), "provided", 0),
            (json!({"hashtags":["#A","#B","#A"]}), "provided", 3),
            (json!({"hashtags":["#A",12]}), "invalid", 0),
            (json!({"hashtags":[""]}), "invalid", 0),
            (json!({"hashtags":null}), "invalid", 0),
        ] {
            let parsed = parse(payload.as_object().unwrap());
            assert_eq!(parsed.state, state);
            assert_eq!(parsed.tags.len(), count);
        }
    }
}
