//! Shared read interface for displayable work resources across Intelligence surfaces.
//!
//! Page adapters consume this module.  Typed material observations, Media V2 slots and
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

pub async fn work_resource_schema_is_ready(database: &Database) -> Result<bool, sqlx::Error> {
    crate::material_projection::material_projection_schema_is_ready(database).await
}
