//! Versioned contracts at Linggan's external boundaries.
//!
//! This crate currently contains the source contract for an XHS content-detail
//! package paired with a separate comments package. It deliberately does not
//! define Evidence, Comment persistence, HTTP payloads, workers, or research
//! results. Those are downstream concerns and must not be inferred from the
//! producer's capture shape.

pub mod xhs_comment_context_source_v0;
pub mod xhs_comment_source_v0;

pub use xhs_comment_source_v0::{
    CapturePackageV0, CaptureRecordV0, PackageRole, SourceContractError, ValidatedCommentV0,
    ValidatedCommentsPackageV0, ValidatedContentDetailPackageV0, ValidatedXhsCommentCapturePairV0,
    XHS_COMMENT_SOURCE_CONTRACT_V0, validate_xhs_comment_capture_pair_v0,
};

pub use xhs_comment_context_source_v0::{
    ContextPackageRoleV0, ContextSourceContractErrorV0, ContextTextAvailabilityV0,
    ValidatedContentDetailContextPackageV0, ValidatedContextCommentV0,
    ValidatedContextCommentsPackageV0, ValidatedRepliesPackageV0, ValidatedReplyV0,
    ValidatedWorkContextV0, ValidatedXhsCommentContextCaptureSetV0,
    XHS_COMMENT_CONTEXT_SOURCE_CONTRACT_V0, parse_and_validate_xhs_comment_context_capture_set_v0,
    validate_xhs_comment_context_capture_set_v0,
};
