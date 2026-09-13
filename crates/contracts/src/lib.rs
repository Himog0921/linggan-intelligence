//! Versioned contracts at Linggan's external boundaries.
//!
//! This crate currently contains the source contract for an XHS content-detail
//! package paired with a separate comments package. It deliberately does not
//! define Evidence, Comment persistence, HTTP payloads, workers, or research
//! results. Those are downstream concerns and must not be inferred from the
//! producer's capture shape.

pub mod xhs_comment_source_v0;

pub use xhs_comment_source_v0::{
    CapturePackageV0, CaptureRecordV0, PackageRole, SourceContractError, ValidatedCommentV0,
    ValidatedCommentsPackageV0, ValidatedContentDetailPackageV0, ValidatedXhsCommentCapturePairV0,
    XHS_COMMENT_SOURCE_CONTRACT_V0, validate_xhs_comment_capture_pair_v0,
};
