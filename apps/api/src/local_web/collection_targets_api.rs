//! Collection workspace and object-inspection HTTP adapters.
//! All qualification, historical values and comparisons remain owned by evidence.
use super::{LocalWebState, collection, collection_target_presentation, local_read_json_error};
use axum::Router;

mod workspace;
mod inspection;

pub(super) fn routes() -> Router<LocalWebState> {
    workspace::routes().merge(inspection::routes())
}
