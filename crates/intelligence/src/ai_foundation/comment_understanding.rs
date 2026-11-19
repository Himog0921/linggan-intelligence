//! Comment Understanding V2: one model call reads the comments of one work and places their
//! problem and need signals in the domain's problem catalog.
//!
//! The model proposes meaning only. Which evidence is accepted, which catalog entries exist, who
//! counts as a distinct author and whether a Problem is formal are decided by this crate.
mod accept;
mod consolidation;
mod contract;

pub use accept::{
    AcceptedSignal, AcceptedTarget, AcceptedUnit, Assignment, NewProblem, Outcome, TargetFrame,
    TargetOutcome, UnitRejection, accept_unit,
};
pub use consolidation::{
    CONSOLIDATION_CONTRACT, ConsolidationDecision, ConsolidationRejected, Verdict,
    accept_consolidation, consolidation_schema,
};
pub use contract::{
    CONTRACT, CatalogEntry, MAX_SIGNALS_PER_TARGET, RESULT_CONTRACT, TargetInput, catalog_alias,
    payload, response_schema, target_alias,
};
