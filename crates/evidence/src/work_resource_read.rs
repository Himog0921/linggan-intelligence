//! Shared read interface for displayable work resources across Intelligence surfaces.
//!
//! Page adapters consume this module. Typed material observations, Media V2 slots and
//! provenance remain internal implementation details, so another page cannot invent its own
//! title/creator/publication-time/cover inference path.

use linggan_contracts::EvidenceQuery;
use linggan_storage_postgres::Database;
use uuid::Uuid;

pub use crate::material_projection::MaterialReadError as WorkResourceReadError;
pub use crate::material_projection_types::{
    MaterialCollectionContext as WorkResourceCollectionContext,
    MaterialDisplay as WorkResourceDisplay, MaterialEngagement as WorkResourceEngagement,
    MaterialIdentity as WorkResourceIdentity, MaterialLaneSummary as WorkResourceLaneSummary,
    MaterialLibraryItem as WorkResource, MaterialLibraryProjection as WorkResourcePage,
    MaterialPreview as WorkResourcePreview, MaterialSummary as WorkResourceSummary,
};

pub(crate) const COVER_HEADLINE_SQL: &str = include_str!("work_resource_read/cover_headline.sql");

pub async fn read_work_resources(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<WorkResourcePage, WorkResourceReadError> {
    crate::material_projection::read_material_library(database, query).await
}

pub async fn validate_work_resource_query(
    database: &Database,
    query: &EvidenceQuery,
) -> Result<(), WorkResourceReadError> {
    crate::material_projection::validate_material_query(database, query).await
}

pub async fn read_work_resource(
    database: &Database,
    public_ref: Uuid,
) -> Result<Option<WorkResource>, WorkResourceReadError> {
    crate::material_detail_read::read_material_detail(database, public_ref).await
}

/// Hydrate a finite work source using the caller's transaction and canonical read owners.
/// This avoids nested pool acquisition when persisting a source-linked decision.
pub async fn read_work_resource_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    public_ref: Uuid,
) -> Result<Option<WorkResource>, WorkResourceReadError> {
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let Some(current) =
        crate::work_resource_current::read_work_resource_currents(tx, &[public_ref], &as_of)
            .await?
            .pop()
    else {
        return Ok(None);
    };
    let mut item = crate::material_projection::material_item(&current, None);
    crate::material_projection::enrich_discovery_material(
        tx,
        &mut item,
        &as_of,
        current.author_attribution_source.as_deref(),
    )
    .await?;
    crate::material_social_read::enrich(tx, &mut item, None, &as_of).await?;
    crate::material_projection::enrich_media_material(tx, &mut item, &as_of).await?;
    Ok(Some(item))
}

pub async fn work_resource_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    crate::material_projection::material_projection_schema_is_ready(database).await
}

/// Compile-time-only CTE composition for consumers that already own a work scope.
/// The caller supplies a `cs_title_scope(work_ref)` CTE and binds `$2` to its as-of time.
/// The output relation is `cs_display_titles(work_ref, display_title, display_title_source)`.
/// No caller text enters this SQL. Native selection still belongs to Work Resource Current;
/// OCR uses the exact selector used by the existing Evidence display fallback.
/// This is not a permission boundary: the caller must first qualify its domain/work scope.
pub fn work_display_title_ctes(ocr_schema_ready: bool) -> Result<String, WorkResourceReadError> {
    let native = crate::material_query_sql::work_resource_currents_sql();
    let binding = "$1::uuid[]";
    if native.matches(binding).count() != 1 || COVER_HEADLINE_SQL.matches(" = $1").count() != 1 {
        return Err(WorkResourceReadError::ProjectionUnavailable);
    }
    // Adapt only a fixed placeholder to a fixed relation, never a caller-supplied SQL fragment.
    let native = native.replacen(binding, "ARRAY(SELECT work_ref FROM cs_title_scope)", 1);
    let cover = if ocr_schema_ready {
        COVER_HEADLINE_SQL.replacen(" = $1", " = current.public_ref", 1)
    } else {
        "SELECT NULL::text AS cover_headline WHERE false".to_owned()
    };
    // Same Unicode White_Space set as str::trim; a non-ASCII blank is not a real title.
    let sql = format!(
        r#"cs_title_currents AS ({native}),
cs_display_titles AS (
    SELECT scope.work_ref,
           CASE WHEN usable.native_present THEN current.title ELSE cover.cover_headline END AS display_title,
           CASE WHEN usable.native_present THEN 'platform_title'
                WHEN cover.cover_headline IS NOT NULL THEN 'cover_ocr' ELSE 'unknown' END AS display_title_source
    FROM cs_title_scope scope
    LEFT JOIN cs_title_currents current ON current.public_ref = scope.work_ref
    CROSS JOIN LATERAL (
        SELECT NULLIF(btrim(current.title,
            U&'\0009\000A\000B\000C\000D\0020\0085\00A0\1680\2000\2001\2002\2003\2004\2005\2006\2007\2008\2009\200A\2028\2029\202F\205F\3000'), '') IS NOT NULL AS native_present
    ) usable
    LEFT JOIN LATERAL ({cover}) cover ON NOT usable.native_present
)"#,
    );
    Ok(sql)
}

#[cfg(test)]
mod title_tests {
    use super::*;

    #[test]
    fn catalog_titles_reuse_current_and_the_single_cover_selector() {
        let sql = work_display_title_ctes(true).unwrap();
        let native = crate::material_query_sql::work_resource_currents_sql().replacen(
            "$1::uuid[]",
            "ARRAY(SELECT work_ref FROM cs_title_scope)",
            1,
        );
        assert!(sql.contains(&native));
        assert!(sql.contains(&COVER_HEADLINE_SQL.replacen(" = $1", " = current.public_ref", 1)));
        assert!(!sql.contains("$1::uuid[]"));
        assert!(
            include_str!("material_projection.rs")
                .contains("crate::work_resource_read::COVER_HEADLINE_SQL.replacen")
        );
    }

    #[test]
    fn absent_ocr_schema_does_not_reference_uninstalled_relations() {
        let sql = work_display_title_ctes(false).unwrap();
        assert!(!sql.contains("linggan_media_ocr_layering_result"));
        assert!(!sql.contains("linggan_media_ocr_retirement"));
        assert!(sql.contains("NULL::text AS cover_headline WHERE false"));
    }

    #[test]
    fn cover_headline_requires_live_owned_qualified_media_not_raw_ocr() {
        for required in [
            "origin.content_public_ref = $1",
            "display_ordinal BETWEEN 1 AND 3",
            "retired.retired_job_ref",
            "WITHDRAWN_OR_RESTRICTED",
            "= 'succeeded'",
            "package.accepted_at <= $2",
            "result.layering_ref DESC",
        ] {
            assert!(COVER_HEADLINE_SQL.contains(required), "missing {required}");
        }
        assert!(!COVER_HEADLINE_SQL.contains("text_content"));
        assert!(!COVER_HEADLINE_SQL.contains("image_substantive_text"));
    }

    #[test]
    fn cover_headline_lookup_starts_from_the_selected_works_media_origins() {
        assert!(COVER_HEADLINE_SQL.contains("WITH current_origin AS MATERIALIZED"));
        assert!(COVER_HEADLINE_SQL.contains("FROM current_origin origin"));
        assert!(
            COVER_HEADLINE_SQL.contains(
                "JOIN linggan_media_processing_job job ON job.slot_key = origin.slot_key"
            )
        );
        assert!(!COVER_HEADLINE_SQL.contains("JOIN LATERAL"));
    }
}

/// Resolve an immutable source version only after the caller qualifies the work's current use.
/// Derivative text still passes the shared disposition/retirement reader before it is returned.
pub async fn read_frozen_work_text_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_ref: Uuid,
    source_ref: Uuid,
    field: &str,
) -> Result<Vec<String>, WorkResourceReadError> {
    match field {
        "title" | "body" => {
            let row: Option<(Option<String>,Option<String>)> = sqlx::query_as("SELECT detail.title,detail.body_text FROM linggan_material_content_detail detail JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=detail.package_ref AND disposition.record_ordinal=detail.record_ordinal WHERE detail.material_ref=$1 AND detail.content_public_ref=$2 AND disposition.disposition<>'quarantined'").bind(source_ref).bind(work_ref).fetch_optional(&mut **tx).await?;
            if let Some((title, body)) = row {
                return Ok(if field == "title" { title } else { body }
                    .into_iter()
                    .collect());
            }
            if field == "title" {
                return Ok(sqlx::query_scalar("SELECT finding.title FROM linggan_material_discovery_finding finding JOIN linggan_runtime_record_disposition disposition ON disposition.package_ref=finding.package_ref AND disposition.record_ordinal=finding.record_ordinal WHERE finding.material_ref=$1 AND finding.content_public_ref=$2 AND disposition.disposition<>'quarantined' AND finding.title IS NOT NULL").bind(source_ref).bind(work_ref).fetch_all(&mut **tx).await?);
            }
            Ok(Vec::new())
        }
        "ocr" | "transcript" => {
            let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
                .fetch_one(&mut **tx)
                .await?;
            let mut resources =
                crate::material_media_read::read_derivatives_batch(tx, &[work_ref], &as_of).await?;
            let mut texts = Vec::new();
            if let Some((derivatives, _, _, _, _)) = resources.remove(&work_ref) {
                for d in derivatives {
                    if d["jobRef"].as_str() != Some(&source_ref.to_string())
                        || matches!(
                            d["dispositionState"].as_str(),
                            Some("WITHDRAWN_OR_RESTRICTED" | "OCR_RETIRED")
                        )
                    {
                        continue;
                    }
                    if field == "transcript" && d["kind"] == "asr_text" {
                        if let Some(text) = d["displayText"].as_str() {
                            texts.push(text.into());
                        }
                    }
                    if field == "ocr" {
                        if d.pointer("/ocrLayering/cleanCorpusEligible")
                            .and_then(serde_json::Value::as_bool)
                            != Some(true)
                        {
                            continue;
                        }
                        if let Some(text) = d
                            .pointer("/ocrLayering/imageSubstantiveText")
                            .and_then(serde_json::Value::as_str)
                        {
                            texts.push(text.into());
                        }
                        // The exact hash retained by the caller chooses an accepted old layout;
                        // current restriction above remains authoritative for the job and bytes.
                        let old:Vec<String>=sqlx::query_scalar("SELECT layer.image_substantive_text FROM linggan_media_ocr_layout layout JOIN linggan_media_ocr_layering_result layer USING(layout_ref) JOIN linggan_media_derivative derivative ON derivative.derivative_ref=layout.ocr_derivative_ref WHERE derivative.job_ref=$1 AND layout.content_public_ref=$2 AND layer.state='ACCEPTED' AND layer.image_substantive_text IS NOT NULL").bind(source_ref).bind(work_ref).fetch_all(&mut **tx).await?;
                        texts.extend(old);
                    }
                }
            }
            Ok(texts)
        }
        _ => Ok(Vec::new()),
    }
}

/// Freeze exact canonical source identities and text hashes without persisting source text.
pub async fn frozen_work_fragment_manifest_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work: Uuid,
) -> Result<Vec<serde_json::Value>, WorkResourceReadError> {
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx)
        .await?;
    let Some(c) = crate::work_resource_current::read_work_resource_currents(tx, &[work], &as_of)
        .await?
        .pop()
    else {
        return Ok(Vec::new());
    };
    let mut fragments = Vec::new();
    let mut add = |field: &str, text: &str, source: Uuid, version: &str| {
        let text: String = text.chars().take(4000).collect();
        let hash = crate::creator_discovery::hash(&text);
        fragments.push(serde_json::json!({"workRef":work,"fragmentId":format!("{work}.{field}.{}",&hash[..12]),"sourceRef":source,"field":field,"sourceVersion":version,"textHash":hash,"start":0,"end":text.chars().count()}));
    };
    for (field, text, source) in [
        ("title", c.title.as_deref(), &c.title_source),
        ("body", c.body_text.as_deref(), &c.body_source),
    ] {
        if let (Some(text), Some(reference)) =
            (text.filter(|s| !s.trim().is_empty()), source.material_ref)
        {
            add(
                field,
                text,
                reference,
                source
                    .recorded_at
                    .as_deref()
                    .unwrap_or("immutable-material"),
            );
        }
    }
    let mut derivatives =
        crate::material_media_read::read_derivatives_batch(tx, &[work], &as_of).await?;
    if let Some((items, _, _, _, _)) = derivatives.remove(&work) {
        for d in items {
            if matches!(
                d["dispositionState"].as_str(),
                Some("WITHDRAWN_OR_RESTRICTED" | "OCR_RETIRED")
            ) {
                continue;
            }
            let clean = d
                .pointer("/ocrLayering/imageSubstantiveText")
                .and_then(serde_json::Value::as_str);
            let text = clean.or_else(|| {
                if d["kind"] == "asr_text" {
                    d["displayText"].as_str()
                } else {
                    None
                }
            });
            if let (Some(text), Some(source)) =
                (text, d["jobRef"].as_str().and_then(|s| s.parse().ok()))
            {
                add(
                    if clean.is_some() { "ocr" } else { "transcript" },
                    text,
                    source,
                    d["processorVersion"].as_str().unwrap_or(""),
                );
            }
        }
    }
    Ok(fragments)
}
