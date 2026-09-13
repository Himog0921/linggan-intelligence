//! Immutable comment-observation decisions.
//!
//! The storage layer reads the current projection under a row lock, invokes
//! this pure decision, and either preserves that projection or appends a new
//! immutable observation. This crate does not use a database and does not infer
//! semantics from comment text.

use linggan_evidence::PreparedCommentRecordV0;

/// The minimal current fact required for a replay decision in source contract
/// V0. The V0 fixture proves comment text, but no other stable observation
/// fields, so only exact text hash changes can advance current here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentCommentFactV0 {
    pub text_sha256: String,
}

/// The only storage actions allowed for one accepted source comment record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommentObservationDecisionV0 {
    /// This identity has no current observation; append the first fact.
    CreateInitial,
    /// The current fact equals the source text; preserve the current pointer.
    ReplayUnchanged,
    /// The source text changed; append and advance current in the same transaction.
    AppendAndAdvance,
}

/// Decides replay behavior without inspecting semantic content.
pub fn decide_comment_observation_v0(
    current: Option<&CurrentCommentFactV0>,
    incoming: &PreparedCommentRecordV0,
) -> CommentObservationDecisionV0 {
    match current {
        None => CommentObservationDecisionV0::CreateInitial,
        Some(current) if current.text_sha256 == incoming.text_sha256 => {
            CommentObservationDecisionV0::ReplayUnchanged
        }
        Some(_) => CommentObservationDecisionV0::AppendAndAdvance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn incoming(hash: &str) -> PreparedCommentRecordV0 {
        PreparedCommentRecordV0 {
            source_record_index: 0,
            identity: linggan_evidence::CommentSourceIdentityV0 {
                workspace_id: "workspace".into(),
                platform: "xhs".into(),
                note_id: "note".into(),
                comment_id: "comment".into(),
            },
            text: "source text".into(),
            text_sha256: hash.into(),
        }
    }

    #[test]
    fn only_an_observed_body_change_advances_current_in_v0() {
        assert_eq!(
            decide_comment_observation_v0(None, &incoming("a")),
            CommentObservationDecisionV0::CreateInitial
        );
        assert_eq!(
            decide_comment_observation_v0(
                Some(&CurrentCommentFactV0 {
                    text_sha256: "a".into(),
                }),
                &incoming("a"),
            ),
            CommentObservationDecisionV0::ReplayUnchanged
        );
        assert_eq!(
            decide_comment_observation_v0(
                Some(&CurrentCommentFactV0 {
                    text_sha256: "a".into(),
                }),
                &incoming("b"),
            ),
            CommentObservationDecisionV0::AppendAndAdvance
        );
    }
}
