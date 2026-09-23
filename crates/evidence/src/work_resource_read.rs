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

pub async fn read_work_resource(
    database: &Database,
    public_ref: Uuid,
) -> Result<Option<WorkResource>, WorkResourceReadError> {
    crate::material_detail_read::read_material_detail(database, public_ref).await
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
        let native = crate::material_query_sql::work_resource_currents_sql()
            .replacen("$1::uuid[]", "ARRAY(SELECT work_ref FROM cs_title_scope)", 1);
        assert!(sql.contains(&native));
        assert!(sql.contains(&COVER_HEADLINE_SQL.replacen(" = $1", " = current.public_ref", 1)));
        assert!(!sql.contains("$1::uuid[]"));
        assert!(include_str!("material_projection.rs").contains("sqlx::query(crate::work_resource_read::COVER_HEADLINE_SQL)"));
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
        for required in ["origin.content_public_ref = $1", "display_ordinal BETWEEN 1 AND 3",
            "retired.retired_job_ref", "WITHDRAWN_OR_RESTRICTED", "= 'succeeded'",
            "package.accepted_at <= $2", "result.layering_ref DESC"] {
            assert!(COVER_HEADLINE_SQL.contains(required), "missing {required}");
        }
        assert!(!COVER_HEADLINE_SQL.contains("text_content"));
        assert!(!COVER_HEADLINE_SQL.contains("image_substantive_text"));
    }
}
