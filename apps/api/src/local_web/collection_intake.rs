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
            // A keyword without a ranking is not a weaker target — it is an ambiguous one.
            // Accepting it would silently pick a ranking on the person's behalf and later
            // merge two different surfaces into one identity.
            let ranking = intake
                .ranking
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or(IntakeRejection::KeywordNeedsRanking)?;
            TargetIdentity::keyword(intake.platform.trim(), intake.identity.trim(), ranking)
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

    #[test]
    fn a_keyword_without_a_ranking_is_rejected_rather_than_guessed() {
        // Picking a ranking on the person's behalf would later merge two different
        // observation surfaces into one identity.
        assert_eq!(
            parse_intake(&intake("keyword", "ADHD", None)).unwrap_err(),
            IntakeRejection::KeywordNeedsRanking
        );
        assert!(parse_intake(&intake("keyword", "ADHD", Some("comprehensive"))).is_ok());
    }

    #[test]
    fn creators_are_taken_by_platform_id_and_keywords_are_normalised() {
        let creator = parse_intake(&intake("creator", " 5ebe6d21 ", None)).unwrap();
        assert_eq!(creator.key(), "5ebe6d21");

        let keyword = parse_intake(&intake("keyword", " ADHD ", Some(" Comprehensive "))).unwrap();
        assert_eq!(keyword.key(), "adhd::comprehensive");
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
