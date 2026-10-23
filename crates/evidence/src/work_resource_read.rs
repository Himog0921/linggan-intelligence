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
pub fn work_display_title_ctes() -> Result<String, WorkResourceReadError> {
    let native = crate::material_query_sql::work_resource_currents_sql();
    let binding = "$1::uuid[]";
    if native.matches(binding).count() != 1 || COVER_HEADLINE_SQL.matches(" = $1").count() != 1 {
        return Err(WorkResourceReadError::ProjectionUnavailable);
    }
    // Adapt only a fixed placeholder to a fixed relation, never a caller-supplied SQL fragment.
    let native = native.replacen(binding, "ARRAY(SELECT work_ref FROM cs_title_scope)", 1);
    let cover = COVER_HEADLINE_SQL.replacen(" = $1", " = current.public_ref", 1);
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
        let sql = work_display_title_ctes().unwrap();
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
    fn cover_heading_requires_exact_source_and_reading_and_real_first_page() {
        for required in ["origin.content_public_ref = $1", "origin.display_ordinal=1",
            "selected.reading_ref", "materialization.materialization_ref::text=reading.source_version",
            "WITHDRAWN_OR_RESTRICTED", "BYTES_CLEANED", "package.accepted_at <= $2",
            "retired.retired_job_ref", "block->>'kind'='heading'"] {
            assert!(COVER_HEADLINE_SQL.contains(required),"missing {required}");
        }
        for obsolete in ["linggan_media_ocr_layering_result", "BETWEEN 1 AND 3", "layer_version", "text_content"] {
            assert!(!COVER_HEADLINE_SQL.contains(obsolete));
        }
    }

}

/// The complete identity of one frozen block. A matching source UUID alone does
/// not authorize a different reading, block, field, version or observation time.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FrozenWorkSource {
    pub source_ref: Uuid,
    pub block_ref: String,
    pub field: String,
    pub source_version: String,
    pub as_of: String,
}

pub async fn read_frozen_work_text_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_ref: Uuid,
    source: &FrozenWorkSource,
) -> Result<Vec<String>, WorkResourceReadError> {
    Ok(read_frozen_work_texts_in_transaction(tx, work_ref, std::slice::from_ref(source))
        .await?.remove(source).unwrap_or_default())
}

/// Historical selection is read through the same dossier owner as current reads.
/// Current restrictions still apply; absent historical identities stay unavailable.
pub async fn read_frozen_work_texts_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work_ref: Uuid,
    sources: &[FrozenWorkSource],
) -> Result<std::collections::HashMap<FrozenWorkSource, Vec<String>>, WorkResourceReadError> {
    use std::collections::{BTreeSet, HashMap};
    let mut texts = HashMap::new();
    for cutoff in sources.iter().map(|s| s.as_of.as_str()).collect::<BTreeSet<_>>() {
        let canonical_cutoff = crate::research_dossier::cutoff_in(tx, Some(cutoff)).await?;
        let Some(dossier) = crate::research_dossier::read_in(tx, &[work_ref], &canonical_cutoff).await?.remove(&work_ref) else {
            continue;
        };
        for requested in sources.iter().filter(|source| source.as_of == cutoff) {
            if let Some(fragment) = dossier.fragments.iter().find(|fragment|
                fragment.fragment_id == requested.block_ref && fragment.source_ref == requested.source_ref
                && fragment.field == requested.field && fragment.source_version == requested.source_version) {
                texts.insert(requested.clone(), vec![fragment.text.clone()]);
            }
        }
    }
    Ok(texts)
}

/// Freeze canonical block identities and hashes, without another copy of source text.
pub async fn frozen_work_fragment_manifest_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    work: Uuid,
) -> Result<Vec<serde_json::Value>, WorkResourceReadError> {
    use serde_json::json;
    let as_of: String = sqlx::query_scalar("SELECT scope_001_now()::text")
        .fetch_one(&mut **tx).await?;
    let Some(dossier) = crate::research_dossier::read_in(tx, &[work], &as_of).await?.remove(&work) else {
        return Ok(Vec::new());
    };
    Ok(dossier.fragments.iter().map(|fragment| json!({
        "workRef":work,"fragmentId":fragment.fragment_id,"blockRef":fragment.fragment_id,
        "sourceRef":fragment.source_ref,"field":fragment.field,"sourceVersion":fragment.source_version,
        "readingRef":fragment.reading_ref,"unitRef":fragment.unit_ref,"asOf":as_of,
        "textHash":crate::creator_discovery::hash(&fragment.text),"start":fragment.start,"end":fragment.end
    })).collect())
}

/// Overview projection for a finite domain-qualified work set. Statistics callers must
/// iterate their complete scope; this function imposes only a per-batch bound.
pub async fn read_work_resource_overviews(
    database: &Database,
    domain: Uuid,
    public_refs: &[Uuid],
    as_of: &str,
) -> Result<Vec<WorkResource>, WorkResourceReadError> {
    crate::material_projection::read_work_overviews(database, domain, public_refs, as_of).await
}
