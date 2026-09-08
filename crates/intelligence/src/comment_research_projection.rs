//! Bounded derived research reads; source restrictions are checked before returning annotations.
use crate::comment_research::CommentResearchError;
use linggan_evidence::comment_research_read::read_comment_research_source;
use linggan_storage_postgres::Database;
use serde_json::{Value, json};
use sqlx::Row;
use uuid::Uuid;

pub async fn read_comment_research_annotations(
    database: &Database,
    source_ref: Uuid,
) -> Result<Value, CommentResearchError> {
    read_comment_research_source(database, source_ref).await?;
    let human=sqlx::query("SELECT annotation_ref,revision,facets,reason,created_at::text AS created_at FROM linggan_comment_annotation
        WHERE source_ref=$1 ORDER BY revision DESC")
        .bind(source_ref).fetch_all(database.pool()).await?;
    let analysis=sqlx::query("SELECT work_ref,rule_version,model_version,state,failure_code,
        CASE WHEN (result#>>'{contextRefs,parentSourceRef}') IS NULL OR EXISTS(SELECT 1 FROM linggan_comment_research_readable p WHERE p.material_ref::text=result#>>'{contextRefs,parentSourceRef}') THEN result ELSE NULL END AS result,
        ((result#>>'{contextRefs,parentSourceRef}') IS NULL OR EXISTS(SELECT 1 FROM linggan_comment_research_readable p WHERE p.material_ref::text=result#>>'{contextRefs,parentSourceRef}')) AS context_readable,
        attempts,updated_at::text AS updated_at
        FROM linggan_comment_analysis_work WHERE source_ref=$1 ORDER BY created_at DESC,work_ref")
        .bind(source_ref).fetch_all(database.pool()).await?;
    let mut response = json!({"human":human.iter().map(|r|json!({"annotationRef":r.get::<Uuid,_>("annotation_ref"),"revision":r.get::<i32,_>("revision"),
        "facets":r.get::<Value,_>("facets"),"reason":r.get::<String,_>("reason"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>(),
        "analysis":analysis.iter().map(|r|json!({"workRef":r.get::<Uuid,_>("work_ref"),"ruleVersion":r.get::<String,_>("rule_version"),
            "modelVersion":r.get::<String,_>("model_version"),"state":r.get::<String,_>("state"),"failureCode":r.get::<Option<String>,_>("failure_code"),
            "result":r.get::<Option<Value>,_>("result"),"contextReadable":r.get::<bool,_>("context_readable"),"attempts":r.get::<i32,_>("attempts"),"updatedAt":r.get::<String,_>("updated_at")})).collect::<Vec<_>>() });
    if let Some(items) = response["analysis"].as_array_mut() {
        for item in items {
            if !item["result"].is_null()
                && !crate::comment_daily_read::context_readable(database, &item["result"])
                    .await
                    .map_err(|_| CommentResearchError::SourceUnavailable)?
            {
                item["result"] = Value::Null;
                item["contextReadable"] = json!(false);
            }
        }
    }
    Ok(response)
}

pub async fn read_comment_problem_groups(
    database: &Database,
    model_version: &str,
) -> Result<Value, CommentResearchError> {
    let rows=sqlx::query("SELECT current.material_ref,h.annotation_ref,h.facets,m.result FROM linggan_material_comment_current current JOIN linggan_comment_research_readable readable USING(material_ref) LEFT JOIN LATERAL(SELECT annotation_ref,facets FROM linggan_comment_annotation WHERE source_ref=current.material_ref ORDER BY revision DESC LIMIT 1) h ON true LEFT JOIN LATERAL(SELECT result FROM linggan_comment_analysis_work WHERE source_ref=current.material_ref AND rule_version IN ($1,'comment-research.v2','comment-research.v3') AND (model_version=$2 OR starts_with(model_version,$2||':')) ORDER BY created_at DESC,work_ref DESC LIMIT 1) m ON true WHERE h.annotation_ref IS NOT NULL OR m.result IS NOT NULL ORDER BY current.material_ref LIMIT 10001")
        .bind(crate::comment_analysis::COMMENT_RULE_VERSION).bind(model_version).fetch_all(database.pool()).await?;
    let mut groups: std::collections::BTreeMap<(String, &str), std::collections::BTreeSet<Uuid>> =
        std::collections::BTreeMap::new();
    for row in rows.iter().take(10000) {
        let source: Uuid = row.get("material_ref");
        let human: Option<Uuid> = row.get("annotation_ref");
        let model: Option<Value> = row.get("result");
        let (facets, origin) = if human.is_some() {
            (
                row.get::<Option<Value>, _>("facets")
                    .and_then(|v| v.as_array().cloned())
                    .unwrap_or_default(),
                "human",
            )
        } else if let Some(result) = model {
            let parent = result
                .pointer("/contextRefs/parentSourceRef")
                .and_then(Value::as_str)
                .and_then(|s| Uuid::parse_str(s).ok());
            if let Some(parent) = parent {
                if read_comment_research_source(database, parent)
                    .await
                    .is_err()
                {
                    continue;
                }
            }
            if !crate::comment_daily_read::context_readable(database, &result)
                .await
                .map_err(|_| CommentResearchError::SourceUnavailable)?
            {
                continue;
            }
            (
                result["spans"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .flat_map(|s| s["facets"].as_array().into_iter().flatten().cloned())
                    .collect(),
                "model",
            )
        } else {
            continue;
        };
        for facet in facets {
            if facet["dimension"] == "problem" {
                if let Some(label) = facet["label"].as_str() {
                    groups
                        .entry((label.into(), origin))
                        .or_default()
                        .insert(source);
                }
            }
        }
    }
    let mut sorted: Vec<_> = groups.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    let truncated = rows.len() > 10000 || sorted.len() > 100;
    Ok(
        json!({"items":sorted.into_iter().take(100).map(|((label,origin),refs)|json!({"label":label,"origin":origin,"sampleCount":refs.len(),"sourceRefs":refs.into_iter().take(20).collect::<Vec<_>>(),"sampleRole":"MATCHING_EXAMPLES_NOT_REPRESENTATIVE"})).collect::<Vec<_>>(),"truncated":truncated,"qualification":"CANDIDATE_LABEL_GROUPS_NOT_FORMAL_TOPICS","groupingRule":"EXACT_LABEL_EQUALITY","modelConnected":false,"analyzedSourceLimit":10000}),
    )
}
