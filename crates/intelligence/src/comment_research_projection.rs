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
    Ok(
        json!({"human":human.iter().map(|r|json!({"annotationRef":r.get::<Uuid,_>("annotation_ref"),"revision":r.get::<i32,_>("revision"),
        "facets":r.get::<Value,_>("facets"),"reason":r.get::<String,_>("reason"),"createdAt":r.get::<String,_>("created_at")})).collect::<Vec<_>>(),
        "analysis":analysis.iter().map(|r|json!({"workRef":r.get::<Uuid,_>("work_ref"),"ruleVersion":r.get::<String,_>("rule_version"),
            "modelVersion":r.get::<String,_>("model_version"),"state":r.get::<String,_>("state"),"failureCode":r.get::<Option<String>,_>("failure_code"),
            "result":r.get::<Option<Value>,_>("result"),"contextReadable":r.get::<bool,_>("context_readable"),"attempts":r.get::<i32,_>("attempts"),"updatedAt":r.get::<String,_>("updated_at")})).collect::<Vec<_>>() }),
    )
}

pub async fn read_comment_problem_groups(
    database: &Database,
    model_version: &str,
) -> Result<Value, CommentResearchError> {
    let rows=sqlx::query(
        "WITH latest_human AS (SELECT DISTINCT ON(source_ref) source_ref,facets FROM linggan_comment_annotation ORDER BY source_ref,revision DESC),
         human_problem AS (SELECT source_ref,facet->>'label' AS label,'human'::text AS origin FROM latest_human,
            LATERAL jsonb_array_elements(facets) facet WHERE facet->>'dimension'='problem'),
         model_problem AS (SELECT work.source_ref,facet->>'label' AS label,'model'::text AS origin
            FROM linggan_comment_analysis_work work,LATERAL jsonb_array_elements(work.result->'spans') span,
            LATERAL jsonb_array_elements(span->'facets') facet
            WHERE work.state='succeeded' AND work.rule_version=$1 AND work.model_version=$2 AND facet->>'dimension'='problem'
            AND ((work.result#>>'{contextRefs,parentSourceRef}') IS NULL OR EXISTS(SELECT 1 FROM linggan_comment_research_readable p WHERE p.material_ref::text=work.result#>>'{contextRefs,parentSourceRef}'))
            AND NOT EXISTS(SELECT 1 FROM latest_human h WHERE h.source_ref=work.source_ref)),
         candidates AS (SELECT * FROM human_problem UNION ALL SELECT * FROM model_problem)
         SELECT label,origin,count(DISTINCT source_ref) AS sample_count,array_agg(DISTINCT source_ref) AS source_refs
         FROM candidates JOIN linggan_material_comment_current current ON current.material_ref=source_ref
         JOIN linggan_comment_research_readable readable ON readable.material_ref=source_ref
         GROUP BY label,origin ORDER BY sample_count DESC,label,origin LIMIT 101")
        .bind(crate::comment_analysis::COMMENT_RULE_VERSION).bind(model_version).fetch_all(database.pool()).await?;
    let more = rows.len() > 100;
    Ok(
        json!({"items":rows.iter().take(100).map(|r|json!({"label":r.get::<String,_>("label"),"origin":r.get::<String,_>("origin"),
        "sampleCount":r.get::<i64,_>("sample_count"),"sourceRefs":r.get::<Vec<Uuid>,_>("source_refs").into_iter().take(20).collect::<Vec<_>>(),
        "sampleRole":"MATCHING_EXAMPLES_NOT_REPRESENTATIVE"})).collect::<Vec<_>>(),"truncated":more,
        "qualification":"CANDIDATE_LABEL_GROUPS_NOT_FORMAL_TOPICS","groupingRule":"EXACT_LABEL_EQUALITY","modelConnected":false}),
    )
}
