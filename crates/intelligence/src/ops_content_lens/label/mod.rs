//! Blind title-only labeling, independent primary/audit runs and versioned agreement.
mod advance;
mod contract;
mod run;

pub(crate) use advance::advance;
pub(crate) use contract::{Title, accepted_labels, payload, response_schema};
pub use run::{Plan, RunReceipt, plan, start};

use crate::ai_foundation::TaskKey;
const TASK: TaskKey = TaskKey::OpsTitleLabel;
pub(crate) const CONTRACT: &str = "ops-title-label.v1";
pub(super) const DIMENSIONS: [&str; 4] = ["moment", "offer", "situation", "audience"];
