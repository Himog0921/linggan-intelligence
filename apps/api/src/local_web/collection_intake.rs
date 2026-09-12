//! COLLECTION-001 · accepting an observation target from the browser.
//!
//! This route stores identity. It does **not** need an Acquisition Authorization, and that is
//! not an oversight: storing a target consumes no platform access. The person was already
//! looking at that page; recording who they were looking at reaches nothing new.
//!
//! What *does* need authorisation is deep archiving — the moment we go back to the platform
//! and read 200 works. That route does not exist yet, and must not be added here (INV-36:
//! no earlier stage may be reported as a later one succeeding).

use linggan_contracts::{CollectionContractError, TargetIdentity, TargetKind, TargetSource};
use serde::Deserialize;
use serde_json::Value;

/// What the browser sends. Deliberately narrow: identity plus whatever public facts the
/// producer already had on screen. No cookies, no tokens, no raw payload.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetIntake {
    pub platform: String,
    pub target_kind: String,
    /// For a creator: the platform's own stable id. For a keyword: the term.
    pub identity: String,
    /// Keyword only. Two rankings of one term are two observation surfaces, so ranking is
    /// part of the identity rather than a display detail.
    #[serde(default)]
    pub ranking: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
    /// Public facts already visible on the page: avatar, bio, follower count…
    ///
    /// The capability registry records that `userPageData` may be absent, so follower counts
    /// can be genuinely unknown. An absent field must stay absent all the way through —
    /// never substituted with 0, and never back-filled from an older capture.
    #[serde(default)]
    pub identity_facts: Option<Value>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum IntakeRejection {
    UnsupportedPlatform,
    UnknownTargetKind,
    EmptyIdentity,
    CreatorIdentityMustBePlatformId,
    KeywordNeedsRanking,
}

impl IntakeRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::UnknownTargetKind => "unknown_target_kind",
            Self::EmptyIdentity => "empty_identity",
            Self::CreatorIdentityMustBePlatformId => "creator_identity_must_be_platform_id",
            Self::KeywordNeedsRanking => "keyword_needs_ranking",
        }
    }
}

impl From<CollectionContractError> for IntakeRejection {
    fn from(error: CollectionContractError) -> Self {
        match error {
            CollectionContractError::UnsupportedPlatform => Self::UnsupportedPlatform,
            CollectionContractError::UnknownTargetKind => Self::UnknownTargetKind,
            CollectionContractError::InvalidCreatorIdentity => {
                Self::CreatorIdentityMustBePlatformId
            }
            _ => Self::EmptyIdentity,
        }
    }
}

/// Turn what the browser sent into a normalised identity, or say exactly why not.
pub fn parse_intake(intake: &TargetIntake) -> Result<TargetIdentity, IntakeRejection> {
    let kind = TargetKind::parse(intake.target_kind.trim())
        .map_err(|_| IntakeRejection::UnknownTargetKind)?;
    if intake.identity.trim().is_empty() {
        return Err(IntakeRejection::EmptyIdentity);
    }
    match kind {
        TargetKind::Creator => {
            TargetIdentity::creator(intake.platform.trim(), intake.identity.trim())
                .map_err(IntakeRejection::from)
        }
        TargetKind::Keyword => {
            // 排序**不再进身份**（`0076`）：一个词就是一个观察目标，按哪些榜去看是它的
            // 巡检规则，可以有好几条。此前这里要求必须带排序，理由是「不带排序的关键词
            // 是个含糊的目标」——那在一个词只能有一条口径的年代成立；现在口径有自己的
            // 归处，再把它塞进身份，只会让同一个词分裂成几个各自建档的目标。
            TargetIdentity::keyword(intake.platform.trim(), intake.identity.trim())
                .map_err(IntakeRejection::from)
        }
    }
}

/// Every target that arrives through this route came from the browser.
pub const INTAKE_SOURCE: TargetSource = TargetSource::PluginPush;

#[cfg(test)]
mod tests {
    use super::*;

    fn intake(kind: &str, identity: &str, ranking: Option<&str>) -> TargetIntake {
        TargetIntake {
            platform: "xhs".to_owned(),
            target_kind: kind.to_owned(),
            identity: identity.to_owned(),
            ranking: ranking.map(str::to_owned),
            display_name: None,
            identity_facts: None,
        }
    }

    /// **不带排序的关键词是完整的，带排序的也不会因此分裂。**
    ///
    /// 排序是巡检口径，属于规则（`0076`）。此前这里要求必须带排序，理由是「替人猜一个
    /// 排序会把两个不同的观察面并成一个身份」——那在一个词只能有一条口径的年代成立。
    /// 现在反过来了：把排序塞进身份，同一个词会分裂成几个各自建档、各攒一份历史的目标，
    /// 而它们面对的是同一批笔记。
    #[test]
    fn a_keyword_is_taken_by_its_term_whatever_ranking_accompanies_it() {
        let without = parse_intake(&intake("keyword", "ADHD", None)).expect("term alone is enough");
        let with = parse_intake(&intake("keyword", "ADHD", Some("comprehensive")))
            .expect("an accompanying ranking is accepted but does not enter the identity");
        assert_eq!(without.key(), with.key());
        assert_eq!(without.key(), "adhd");
    }

    #[test]
    fn creators_are_taken_by_platform_id_and_keywords_are_normalised() {
        let creator = parse_intake(&intake("creator", " 5ebe6d21 ", None)).unwrap();
        assert_eq!(creator.key(), "5ebe6d21");

        let keyword = parse_intake(&intake("keyword", " ADHD ", Some(" Comprehensive "))).unwrap();
        assert_eq!(keyword.key(), "adhd");
    }

    #[test]
    fn unsupported_platforms_and_kinds_are_named_not_lumped_together() {
        let mut douyin = intake("creator", "abc", None);
        douyin.platform = "douyin".to_owned();
        assert_eq!(
            parse_intake(&douyin).unwrap_err(),
            IntakeRejection::UnsupportedPlatform
        );
        assert_eq!(
            parse_intake(&intake("topic", "abc", None)).unwrap_err(),
            IntakeRejection::UnknownTargetKind
        );
        assert_eq!(
            parse_intake(&intake("creator", "   ", None)).unwrap_err(),
            IntakeRejection::EmptyIdentity
        );
    }

    #[test]
    fn creator_url_is_rejected_before_storage() {
        assert_eq!(
            parse_intake(&intake(
                "creator",
                "https://www.xiaohongshu.com/user/profile/5ebe6d21",
                None
            ))
            .unwrap_err(),
            IntakeRejection::CreatorIdentityMustBePlatformId
        );
    }
}
