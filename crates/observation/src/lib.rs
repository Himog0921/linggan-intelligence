//! Turning one accepted capture record into a source identity, a content observation and a
//! recomputed current value. SCOPE-001 authorizes only the synthetic
//! `content-detail-processor-v1`; a second processor version fails closed.

mod processing;

pub use processing::{
    BusinessOutcome, ClaimedRecord, ProcessedRecord, ProcessingError, ProcessingFault,
    ProcessingOptions, ProcessingOutcome, claim_one_ready_record, process_one_ready_record,
    run_claimed_record,
};
