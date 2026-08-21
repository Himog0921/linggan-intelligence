//! Immutable source evidence. SCOPE-001 authorizes only synthetic Package/Record ingress after
//! the semantic code gate; real producer evidence remains out of scope.

mod ingress;
mod receipt;
mod work_order;

pub use ingress::{
    IngressError, IngressFault, IngressOptions, PreRoutingCode, ingest_capture_package,
    ingest_capture_package_with,
};
pub use receipt::{IngressOutcome, RejectionCode};
