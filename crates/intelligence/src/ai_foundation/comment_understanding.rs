//! Comment Understanding V2: one model call reads the comments of one work and places their
//! problem and need signals in the domain's problem catalog.
//!
//! The model proposes meaning only. Which evidence is accepted, which catalog entries exist, who
//! counts as a distinct author and whether a Problem is formal are decided by this crate.
mod accept;
pub mod catalog;
mod consolidation;
pub(crate) mod consolidation_queue;
mod context;
mod contract;
#[cfg(test)]
mod execution_tests;
mod plan;
pub mod publish;
pub(super) mod read;
pub(crate) mod redirect;
mod run;
mod start;
mod target;
pub mod unit;

pub use accept::{
    AcceptedSignal, AcceptedTarget, AcceptedUnit, Assignment, NewProblem, Outcome, TargetFrame,
    TargetOutcome, UnitRejection, accept_unit,
};
pub use consolidation::{
    CONSOLIDATION_CONTRACT, ConsolidationDecision, ConsolidationRejected, Verdict,
    accept_consolidation, consolidation_schema,
};
pub use context::work_context;
pub use contract::{
    CONTRACT, CatalogEntry, MAX_SIGNALS_PER_TARGET, RESULT_CONTRACT, TargetInput, catalog_alias,
    payload, response_schema, target_alias,
};
pub use plan::{
    OUTPUT_TOKENS_PER_TARGET, PlannedTarget, PlannedUnit, UNIT_MAX_OUTPUT_ESTIMATE,
    UNIT_MAX_TARGETS, WorkGroup, capacity, plan_hash, split,
};
pub use read::{read_latest, read_problem, read_problems};
pub use run::{Cancel, cancel, read_run, read_runs};
pub use start::{MAX_SELECTIONS, Receipt, Selection, Start, preview, start};
pub use target::{
    CurrentSource, ParentContext, Target, current_source_in, read_in as read_target_in,
};

pub use consolidation_queue::{StartConsolidation, start_consolidation};
pub use redirect::{RevokeRedirect, revoke_redirect};
