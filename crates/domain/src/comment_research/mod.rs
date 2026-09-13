//! Deterministic comment preparation for research.
//!
//! This module implements the narrow Cleaning Contract V1. It deliberately does
//! not decide whether a comment is valuable, assign semantic labels, or ingest a
//! comment from any source. Callers retain the raw comment unchanged and use the
//! returned text only as a derived research representation.

mod context_pack_v1;
mod execution_v1;

pub use context_pack_v1::{
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DIRECT_EVIDENCE_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_LIMIT,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_DISCUSSION_ITEM_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_TOTAL_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_BODY_MAX_CHARS,
    COMMENT_RESEARCH_CONTEXT_PACK_V1_WORK_TITLE_MAX_CHARS, CommentResearchContextPackInputV1,
    CommentResearchContextPackV1, ContextPackDiscussionExcerptV1, ContextPackOmissionV1,
    ContextPackPreviewSourceTextV1, ContextPackPreviewTextV1, ContextPackReadinessV1,
    ContextPackRelatedDiscussionInputV1, ContextPackSourceTextV1,
    build_comment_research_context_pack_v1,
};
pub use execution_v1::{
    COMMENT_ANALYSIS_OUTPUT_SCHEMA_V1, COMMENT_RESEARCH_EXECUTION_CONTRACT_V1,
    COMMENT_RESEARCH_FINGERPRINT_V1, COMMENT_RESEARCH_INPUT_SNAPSHOT_VERSION_V1,
    ContextSufficiencyV1, FrozenCommentResearchContextPackV1, ResearchFingerprintErrorV1,
    ResearchFingerprintInputV1, ResearchModelStrategyV1, freeze_comment_research_context_pack_v1,
    research_fingerprint_v1,
};

/// The outcome of deterministic preparation for comment research.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentResearchState {
    /// The comment has no researchable natural-language text after hard filters.
    Dropped,
    /// The derived text can be sent to later research steps without reply context.
    Analyzable,
    /// The text is retained, but a deterministic reply pattern needs work or thread context.
    ///
    /// This never means that the comment has low value.
    NeedsContext,
    /// The input contains data that cannot be safely interpreted by this contract.
    Anomaly,
}

/// A closed, auditable explanation for a preparation outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CleaningReasonCode {
    Blank,
    MentionOnly,
    EmojiOnly,
    MentionAndEmojiOnly,
    PunctuationOnly,
    NoEffectiveText,
    ControlCharacter,
    ContextDependentReply,
}

/// A derived research representation. `raw` is never mutated or retained here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanedComment {
    /// The text that later research stages may use. It is absent for dropped and anomalous input.
    pub research_text: Option<String>,
    pub state: CommentResearchState,
    /// One or more closed reason codes in stable order.
    pub reason_codes: Vec<CleaningReasonCode>,
}

impl CleanedComment {
    fn dropped(reason: CleaningReasonCode) -> Self {
        Self {
            research_text: None,
            state: CommentResearchState::Dropped,
            reason_codes: vec![reason],
        }
    }

    fn anomaly(reason: CleaningReasonCode) -> Self {
        Self {
            research_text: None,
            state: CommentResearchState::Anomaly,
            reason_codes: vec![reason],
        }
    }

    fn analyzable(research_text: String) -> Self {
        Self {
            research_text: Some(research_text),
            state: CommentResearchState::Analyzable,
            reason_codes: Vec::new(),
        }
    }

    fn needs_context(research_text: String) -> Self {
        Self {
            research_text: Some(research_text),
            state: CommentResearchState::NeedsContext,
            reason_codes: vec![CleaningReasonCode::ContextDependentReply],
        }
    }
}

/// Creates a deterministic, derived representation for later comment research.
///
/// The caller owns `raw`; this function never mutates it. It removes only
/// deterministic presentation noise: standalone `@mentions`, emoji components,
/// and whitespace around removed components. An unseparated long Chinese `@` run
/// is retained because plain text cannot prove where its username ends and its
/// semantic body begins. Natural-language text is retained, even when it is short.
/// A retained short reply can be marked [`CommentResearchState::NeedsContext`]
/// when it has a narrow, deterministic reference pattern.
pub fn clean_comment_for_research(raw: &str) -> CleanedComment {
    if raw.chars().any(is_disallowed_control_character) {
        return CleanedComment::anomaly(CleaningReasonCode::ControlCharacter);
    }

    let stripped = remove_standalone_mentions_and_emoji(raw);
    let research_text = normalize_whitespace(&stripped);

    if research_text.is_empty() {
        return CleanedComment::dropped(empty_reason(raw));
    }

    if research_text.chars().all(is_research_punctuation) {
        return CleanedComment::dropped(CleaningReasonCode::PunctuationOnly);
    }

    if has_no_effective_text(&research_text) {
        return CleanedComment::dropped(CleaningReasonCode::NoEffectiveText);
    }

    if requires_context(&research_text) {
        return CleanedComment::needs_context(research_text);
    }

    CleanedComment::analyzable(research_text)
}

fn is_disallowed_control_character(character: char) -> bool {
    character.is_control() && !matches!(character, '\n' | '\r' | '\t')
}

fn remove_standalone_mentions_and_emoji(raw: &str) -> String {
    let characters: Vec<char> = raw.chars().collect();
    let mut output = String::with_capacity(raw.len());
    let mut index = 0;

    while index < characters.len() {
        if starts_standalone_mention(&characters, index) {
            index = consume_mention(&characters, index);
            skip_separator_after_removed_component(&characters, &mut index);
            continue;
        }

        let character = characters[index];
        if is_emoji_component(character) || is_format_noise(character) {
            index += 1;
            continue;
        }

        output.push(character);
        index += 1;
    }

    output
}

fn starts_standalone_mention(characters: &[char], index: usize) -> bool {
    if characters[index] != '@' {
        return false;
    }

    let starts_at_a_token_boundary = index == 0 || characters[index - 1].is_whitespace();
    if !starts_at_a_token_boundary
        || !characters
            .get(index + 1)
            .is_some_and(|character| is_mention_name_character(*character))
    {
        return false;
    }

    let mention_end = consume_mention(characters, index);
    !is_ambiguous_unseparated_chinese_run(characters, index + 1, mention_end)
}

/// A plain-text Chinese run such as `@作者求具体方法` has no reliable username
/// boundary. Only a source-provided mention span can establish one. We preserve
/// these long runs rather than deleting possible semantic text. A short pure
/// mention like `@作者` remains a deterministic hard-filter candidate.
fn is_ambiguous_unseparated_chinese_run(
    characters: &[char],
    mention_start: usize,
    mention_end: usize,
) -> bool {
    let candidate = &characters[mention_start..mention_end];
    let lacks_textual_boundary = !characters
        .get(mention_end)
        .is_some_and(|character| character.is_whitespace() || is_mention_separator(*character));

    lacks_textual_boundary
        && candidate.len() > 2
        && candidate
            .iter()
            .all(|character| is_cjk_ideograph(*character))
}

fn is_cjk_ideograph(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF)
}

fn consume_mention(characters: &[char], mut index: usize) -> usize {
    debug_assert_eq!(characters[index], '@');
    index += 1;
    while characters
        .get(index)
        .is_some_and(|character| is_mention_name_character(*character))
    {
        index += 1;
    }
    index
}

fn is_mention_name_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.')
}

fn skip_separator_after_removed_component(characters: &[char], index: &mut usize) {
    while let Some(character) = characters.get(*index) {
        if character.is_whitespace()
            || is_emoji_component(*character)
            || is_mention_separator(*character)
        {
            *index += 1;
        } else {
            break;
        }
    }
}

fn is_mention_separator(character: char) -> bool {
    matches!(character, ',' | '，' | ':' | '：' | '、')
}

/// Formatting code points are not user language. They are removed only from the
/// derived research text; original input remains the caller's immutable fact.
fn is_format_noise(character: char) -> bool {
    matches!(
        character,
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{FE0E}' | '\u{FE0F}'
    )
}

fn is_emoji_component(character: char) -> bool {
    matches!(
        character as u32,
        0x1F000..=0x1FAFF | 0x2600..=0x27BF | 0x2300..=0x23FF | 0x2B00..=0x2BFF
    ) || matches!(character, '\u{200D}' | '\u{FE0E}' | '\u{FE0F}' | '\u{20E3}')
}

fn normalize_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn empty_reason(raw: &str) -> CleaningReasonCode {
    let without_whitespace = raw.chars().filter(|character| !character.is_whitespace());
    let mut has_mention = false;
    let mut has_emoji = false;
    let mut has_other = false;

    for character in without_whitespace {
        if is_format_noise(character) {
            continue;
        } else if is_emoji_component(character) {
            has_emoji = true;
        } else if character == '@' || is_mention_name_character(character) {
            has_mention = true;
        } else {
            has_other = true;
        }
    }

    match (has_mention, has_emoji, has_other) {
        (false, false, false) => CleaningReasonCode::Blank,
        (true, true, false) => CleaningReasonCode::MentionAndEmojiOnly,
        (true, false, false) => CleaningReasonCode::MentionOnly,
        (false, true, false) => CleaningReasonCode::EmojiOnly,
        _ => CleaningReasonCode::NoEffectiveText,
    }
}

fn has_no_effective_text(text: &str) -> bool {
    text.chars().all(|character| {
        character.is_whitespace()
            || is_research_punctuation(character)
            || is_emoji_component(character)
            || is_format_noise(character)
    })
}

fn is_research_punctuation(character: char) -> bool {
    character.is_ascii_punctuation()
        || matches!(
            character,
            '，' | '。'
                | '！'
                | '？'
                | '、'
                | '；'
                | '：'
                | '…'
                | '“'
                | '”'
                | '‘'
                | '’'
                | '（'
                | '）'
                | '【'
                | '】'
                | '《'
                | '》'
                | '〈'
                | '〉'
                | '「'
                | '」'
                | '『'
                | '』'
                | '—'
                | '－'
                | '～'
                | '·'
                | '｀'
        )
}

fn requires_context(text: &str) -> bool {
    let compact: String = text
        .chars()
        .filter(|character| !character.is_whitespace() && !is_research_punctuation(*character))
        .collect();

    matches!(
        compact.as_str(),
        "同问"
            | "我家也是"
            | "有效果吗"
            | "有用吗"
            | "这个可以吗"
            | "可以吗"
            | "真的吗"
            | "那呢"
            | "这个呢"
    ) || is_short_age_or_grade_reply(&compact)
}

fn is_short_age_or_grade_reply(compact: &str) -> bool {
    let Some(number_end) = compact.find(|character: char| !character.is_ascii_digit()) else {
        return false;
    };

    if number_end == 0 {
        return false;
    }

    matches!(&compact[number_end..], "岁" | "岁呢" | "年级" | "年级呢")
}

#[cfg(test)]
mod tests {
    use super::{
        CleanedComment, CleaningReasonCode, CommentResearchState, clean_comment_for_research,
    };

    fn assert_cleaned(
        raw: &str,
        state: CommentResearchState,
        research_text: Option<&str>,
        reasons: &[CleaningReasonCode],
    ) {
        let result = clean_comment_for_research(raw);
        assert_eq!(
            result,
            CleanedComment {
                state,
                research_text: research_text.map(ToOwned::to_owned),
                reason_codes: reasons.to_vec(),
            }
        );
    }

    #[test]
    fn preserves_semantic_text_while_removing_leading_mention_and_emoji() {
        let raw = "@作者 😂  我家孩子也这样，求具体方法！";
        assert_cleaned(
            raw,
            CommentResearchState::Analyzable,
            Some("我家孩子也这样，求具体方法！"),
            &[],
        );
        assert_eq!(raw, "@作者 😂  我家孩子也这样，求具体方法！");
    }

    #[test]
    fn drops_whitespace_for_ascii_and_chinese_spaces() {
        assert_cleaned(
            " \t\n\u{3000}\r ",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::Blank],
        );
    }

    #[test]
    fn drops_pure_emoji_and_pure_mention_variants() {
        assert_cleaned(
            "😂❤️\u{FE0F}",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::EmojiOnly],
        );
        assert_cleaned(
            "@小明",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::MentionOnly],
        );
        assert_cleaned(
            "@小明 😂❤️\u{FE0F}",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::MentionAndEmojiOnly],
        );
    }

    #[test]
    fn drops_meaningless_punctuation_without_treating_short_text_as_noise() {
        assert_cleaned(
            "！！！……---",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::PunctuationOnly],
        );
        assert_cleaned(
            "求分享",
            CommentResearchState::Analyzable,
            Some("求分享"),
            &[],
        );
    }

    #[test]
    fn retains_short_context_dependent_replies() {
        for raw in ["同问", "有效果吗？", "我家也是", "5年级呢", "4岁"] {
            assert_cleaned(
                raw,
                CommentResearchState::NeedsContext,
                Some(raw),
                &[CleaningReasonCode::ContextDependentReply],
            );
        }
    }

    #[test]
    fn preserves_ambiguous_unseparated_chinese_at_runs() {
        assert_cleaned(
            "@作者求具体方法",
            CommentResearchState::Analyzable,
            Some("@作者求具体方法"),
            &[],
        );
        assert_cleaned(
            "@作者求具体方法😂",
            CommentResearchState::Analyzable,
            Some("@作者求具体方法"),
            &[],
        );
    }

    #[test]
    fn does_not_strip_an_at_sign_inside_plain_text() {
        assert_cleaned(
            "请发到hello@example.com，谢谢",
            CommentResearchState::Analyzable,
            Some("请发到hello@example.com，谢谢"),
            &[],
        );
    }

    #[test]
    fn removes_zero_width_format_noise_from_derived_text() {
        assert_cleaned(
            "我\u{200B}家也是",
            CommentResearchState::NeedsContext,
            Some("我家也是"),
            &[CleaningReasonCode::ContextDependentReply],
        );
        assert_cleaned(
            "同\u{200B}问",
            CommentResearchState::NeedsContext,
            Some("同问"),
            &[CleaningReasonCode::ContextDependentReply],
        );
        assert_cleaned(
            "\u{200B}\u{200C}\u{200D}\u{FEFF}",
            CommentResearchState::Dropped,
            None,
            &[CleaningReasonCode::Blank],
        );
    }

    #[test]
    fn reports_disallowed_control_characters_as_anomalies() {
        assert_cleaned(
            "我家也是\u{0007}",
            CommentResearchState::Anomaly,
            None,
            &[CleaningReasonCode::ControlCharacter],
        );
    }
}
