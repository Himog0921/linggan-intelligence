//! Turning one accepted capture record into a source identity, a content observation and a
//! recomputed current value. SCOPE-001 authorizes only the synthetic
//! `content-detail-processor-v1`; a second processor version fails closed.

mod content;
mod processing;
mod source_identity;

pub use processing::{
    BusinessOutcome, ProcessedRecord, ProcessingError, ProcessingOptions, ProcessingOutcome,
    process_one_ready_record,
};
