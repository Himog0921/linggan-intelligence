//! Pure, bounded assembly for the Comment Research Context Pack V1 preview.
//!
//! A Context Pack is not a prompt and does not perform research. It is the
//! source-backed text boundary a later, separately authorized execution path
//! may use. The current deterministic research expression is the only direct
//! evidence of what the comment author said. Discussion and work text only
//! supply context for interpretation; they must never be recast as direct
//! comment evidence.

/// The maximum number of Unicode scalar values retained from the current
/// deterministic research expression. Truncation uses `chars`, never byte
/// slicing, so a UTF-8 scalar value cannot be split.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS: usize = 480;

/// At most this many already-captured related discussion excerpts are included.
/// This is a preview bound, not a claim that a complete discussion tree exists.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT: usize = 3;

/// The maximum Unicode scalar values retained from each related discussion
/// excerpt.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_MAX_CHARS: usize = 220;

/// The maximum Unicode scalar values retained from an observed work title.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_TITLE_MAX_CHARS: usize = 160;

/// The maximum Unicode scalar values retained from an observed work body.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_BODY_MAX_CHARS: usize = 500;

/// The strict total character budget for all textual fields in one preview.
/// It equals the sum of the explicit field caps, making the bound auditable
/// without allocating invisible remaining-budget heuristics.
pub const COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS: usize = 1_800;

/// Whether Cleaning Contract V1 says an expression can stand alone or should
/// be read with context. This remains a preparation fact, not an analysis
/// result or an execution state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPackReadinessV1 {
    Ready,
    NeedsContext,
}

/// Source-backed text state. `Blank` means the captured field existed but had
/// no content; `Unavailable` means this source-backed context set did not
/// provide the field. Neither state is silently converted into an empty string.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextPackSourceTextV1 {
    Observed(String),
    Blank,
    Unavailable,
}

/// One related reply whose root, parent, or reply-to pointer was observed to
/// refer to the current comment. These relationships are independent facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPackRelatedDiscussionInputV1 {
    pub text: String,
    pub root_comment: bool,
    pub parent_comment: bool,
    pub reply_to_comment: bool,
}

/// The only input consumed by pure Context Pack assembly. It has no database,
/// model, HTTP, prompt, provider, or scheduling dependency.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentResearchContextPackInputV1 {
    /// The deterministic Cleaning Contract V1 expression. This is the only
    /// text that may later support a claim about the current comment author.
    pub research_expression: String,
    pub readiness: ContextPackReadinessV1,
    /// `true` means a text-matching source-backed context set was admitted. It
    /// does not claim that the platform work or discussion tree is complete.
    pub has_source_backed_context: bool,
    pub related_discussion: Vec<ContextPackRelatedDiscussionInputV1>,
    pub work_title: ContextPackSourceTextV1,
    pub work_body: ContextPackSourceTextV1,
}

/// A retained preview text field. `truncated` is always explicit; consumers
/// must never mistake a retained prefix for the complete captured field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPackPreviewTextV1 {
    pub text: String,
    pub truncated: bool,
}

/// A title/body field with source availability preserved through preview
/// assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextPackPreviewSourceTextV1 {
    Observed(ContextPackPreviewTextV1),
    Blank,
    Unavailable,
}

/// A bounded related discussion excerpt. It remains discussion context and is
/// never moved into the direct-comment-evidence field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextPackDiscussionExcerptV1 {
    pub text: ContextPackPreviewTextV1,
    pub root_comment: bool,
    pub parent_comment: bool,
    pub reply_to_comment: bool,
}

/// A precise, user-displayable reason why part of a captured input is absent
/// from this preview. It is not a cleaning reason code and does not explain a
/// model result because no model has run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextPackOmissionV1 {
    DirectEvidenceTruncated,
    DiscussionExcerptTruncated,
    DiscussionItemLimitReached,
    WorkTitleTruncated,
    WorkBodyTruncated,
    SourceBackedContextUnavailable,
}

/// The concrete content assembly result for one current User Voice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommentResearchContextPackV1 {
    /// The only direct comment evidence in this pack.
    pub direct_comment_evidence: ContextPackPreviewTextV1,
    pub readiness: ContextPackReadinessV1,
    pub has_source_backed_context: bool,
    pub discussion_context: Vec<ContextPackDiscussionExcerptV1>,
    pub work_title: ContextPackPreviewSourceTextV1,
    pub work_body: ContextPackPreviewSourceTextV1,
    pub included_characters: usize,
    pub omissions: Vec<ContextPackOmissionV1>,
}

/// Builds a strict, deterministic and side-effect-free Context Pack preview.
///
/// All cuts use `chars()` and therefore never cut a UTF-8 scalar value in the
/// middle. The per-field caps sum to the total cap, and an invariant assertion
/// checks the actual assembled size. A later execution path must make its own
/// context-sufficiency decision, especially for `NeedsContext`; this preview
/// only reports the captured text presently available.
pub fn build_comment_research_context_pack_v1(
    input: CommentResearchContextPackInputV1,
) -> CommentResearchContextPackV1 {
    let mut omissions = Vec::new();
    let (direct_text, direct_truncated) = truncate_chars(
        &input.research_expression,
        COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS,
    );
    if direct_truncated {
        omissions.push(ContextPackOmissionV1::DirectEvidenceTruncated);
    }

    let mut discussion_context = Vec::new();
    if input.has_source_backed_context {
        let total_discussion_items = input.related_discussion.len();
        for item in input
            .related_discussion
            .into_iter()
            .take(COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT)
        {
            let (text, truncated) = truncate_chars(
                &item.text,
                COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_MAX_CHARS,
            );
            if truncated {
                omissions.push(ContextPackOmissionV1::DiscussionExcerptTruncated);
            }
            discussion_context.push(ContextPackDiscussionExcerptV1 {
                text: ContextPackPreviewTextV1 { text, truncated },
                root_comment: item.root_comment,
                parent_comment: item.parent_comment,
                reply_to_comment: item.reply_to_comment,
            });
        }
        if total_discussion_items > COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT {
            omissions.push(ContextPackOmissionV1::DiscussionItemLimitReached);
        }
    } else {
        omissions.push(ContextPackOmissionV1::SourceBackedContextUnavailable);
    }

    let work_title = preview_source_text(
        input.work_title,
        COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_TITLE_MAX_CHARS,
        ContextPackOmissionV1::WorkTitleTruncated,
        &mut omissions,
    );
    let work_body = preview_source_text(
        input.work_body,
        COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_BODY_MAX_CHARS,
        ContextPackOmissionV1::WorkBodyTruncated,
        &mut omissions,
    );

    let included_characters = direct_text.chars().count()
        + discussion_context
            .iter()
            .map(|entry| entry.text.text.chars().count())
            .sum::<usize>()
        + preview_source_text_character_count(&work_title)
        + preview_source_text_character_count(&work_body);
    assert!(
        included_characters <= COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS,
        "context pack field bounds must enforce the total character limit"
    );

    CommentResearchContextPackV1 {
        direct_comment_evidence: ContextPackPreviewTextV1 {
            text: direct_text,
            truncated: direct_truncated,
        },
        readiness: input.readiness,
        has_source_backed_context: input.has_source_backed_context,
        discussion_context,
        work_title,
        work_body,
        included_characters,
        omissions,
    }
}

fn preview_source_text(
    source_text: ContextPackSourceTextV1,
    max_chars: usize,
    truncation_omission: ContextPackOmissionV1,
    omissions: &mut Vec<ContextPackOmissionV1>,
) -> ContextPackPreviewSourceTextV1 {
    match source_text {
        ContextPackSourceTextV1::Observed(text) => {
            let (text, truncated) = truncate_chars(&text, max_chars);
            if truncated {
                omissions.push(truncation_omission);
            }
            ContextPackPreviewSourceTextV1::Observed(ContextPackPreviewTextV1 { text, truncated })
        }
        ContextPackSourceTextV1::Blank => ContextPackPreviewSourceTextV1::Blank,
        ContextPackSourceTextV1::Unavailable => ContextPackPreviewSourceTextV1::Unavailable,
    }
}

fn preview_source_text_character_count(value: &ContextPackPreviewSourceTextV1) -> usize {
    match value {
        ContextPackPreviewSourceTextV1::Observed(text) => text.text.chars().count(),
        ContextPackPreviewSourceTextV1::Blank | ContextPackPreviewSourceTextV1::Unavailable => 0,
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> (String, bool) {
    let mut characters = value.chars();
    let prefix: String = characters.by_ref().take(max_chars).collect();
    (prefix, characters.next().is_some())
}

#[cfg(test)]
mod tests {
    use super::{
        COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS,
        COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT,
        COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS, CommentResearchContextPackInputV1,
        ContextPackOmissionV1, ContextPackReadinessV1, ContextPackRelatedDiscussionInputV1,
        ContextPackSourceTextV1, build_comment_research_context_pack_v1,
    };

    #[test]
    fn separates_direct_evidence_from_source_backed_context_and_preserves_text_states() {
        let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
            research_expression: "我家也是这样".to_owned(),
            readiness: ContextPackReadinessV1::NeedsContext,
            has_source_backed_context: true,
            related_discussion: vec![ContextPackRelatedDiscussionInputV1 {
                text: "父评论在描述写作业拖延".to_owned(),
                root_comment: true,
                parent_comment: true,
                reply_to_comment: false,
            }],
            work_title: ContextPackSourceTextV1::Blank,
            work_body: ContextPackSourceTextV1::Unavailable,
        });

        assert_eq!(pack.direct_comment_evidence.text, "我家也是这样");
        assert_eq!(pack.discussion_context.len(), 1);
        assert_eq!(
            pack.discussion_context[0].text.text,
            "父评论在描述写作业拖延"
        );
        assert!(pack.discussion_context[0].root_comment);
        assert!(pack.discussion_context[0].parent_comment);
        assert!(!pack.discussion_context[0].reply_to_comment);
        assert_eq!(pack.readiness, ContextPackReadinessV1::NeedsContext);
        assert!(pack.has_source_backed_context);
        assert!(
            !pack
                .omissions
                .contains(&ContextPackOmissionV1::SourceBackedContextUnavailable)
        );
    }

    #[test]
    fn strict_budget_is_deterministic_and_never_splits_utf8_scalar_values() {
        let long_unicode =
            "中🚀".repeat(COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS + 10);
        let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
            research_expression: long_unicode,
            readiness: ContextPackReadinessV1::Ready,
            has_source_backed_context: true,
            related_discussion: (0..COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT + 2)
                .map(|index| ContextPackRelatedDiscussionInputV1 {
                    text: format!("讨论 {index} {}", "语🙂".repeat(200)),
                    root_comment: false,
                    parent_comment: true,
                    reply_to_comment: false,
                })
                .collect(),
            work_title: ContextPackSourceTextV1::Observed("题🚀".repeat(200)),
            work_body: ContextPackSourceTextV1::Observed("正文🙂".repeat(300)),
        });

        assert!(pack.direct_comment_evidence.truncated);
        assert!(
            pack.direct_comment_evidence
                .text
                .is_char_boundary(pack.direct_comment_evidence.text.len())
        );
        assert_eq!(
            pack.direct_comment_evidence.text.chars().count(),
            COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS
        );
        assert_eq!(
            pack.discussion_context.len(),
            COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT
        );
        assert!(
            pack.omissions
                .contains(&ContextPackOmissionV1::DiscussionItemLimitReached)
        );
        assert!(pack.included_characters <= COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS);
        assert!(
            pack.discussion_context
                .iter()
                .all(|entry| entry.text.text.is_char_boundary(entry.text.text.len()))
        );
    }

    #[test]
    fn missing_source_backed_context_is_explicit_without_inventing_text() {
        let pack = build_comment_research_context_pack_v1(CommentResearchContextPackInputV1 {
            research_expression: "同问".to_owned(),
            readiness: ContextPackReadinessV1::NeedsContext,
            has_source_backed_context: false,
            related_discussion: vec![ContextPackRelatedDiscussionInputV1 {
                text: "must not enter without matching context".to_owned(),
                root_comment: true,
                parent_comment: false,
                reply_to_comment: false,
            }],
            work_title: ContextPackSourceTextV1::Unavailable,
            work_body: ContextPackSourceTextV1::Unavailable,
        });

        assert!(pack.discussion_context.is_empty());
        assert!(
            pack.omissions
                .contains(&ContextPackOmissionV1::SourceBackedContextUnavailable)
        );
        assert_eq!(pack.direct_comment_evidence.text, "同问");
    }
}
