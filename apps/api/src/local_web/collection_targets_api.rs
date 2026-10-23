//! Collection workspace and object-inspection HTTP adapters.
//! Qualification, historical values and comparisons remain owned by evidence.
use super::{LocalWebState,collection,collection_target_presentation,local_read_json_error};
use axum::Router;
mod workspace;
mod inspection;
mod materials;
mod analysis;
pub(super) fn routes()->Router<LocalWebState>{
    workspace::routes().merge(inspection::routes()).merge(materials::routes()).merge(analysis::routes())
}
