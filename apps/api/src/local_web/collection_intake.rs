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
}

impl IntakeRejection {
    pub fn code(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::UnknownTargetKind => "unknown_target_kind",
            Self::EmptyIdentity => "empty_identity",
            Self::CreatorIdentityMustBePlatformId => "creator_identity_must_be_platform_id",
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
            // 一个词就是一个观察目标；排序是独立巡检规则的口径。目标 intake 不保留一个
            // 既不进入 identity、也不建立 rule 的无效字段。
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

    fn intake(kind: &str, identity: &str) -> TargetIntake {
        TargetIntake {
            platform: "xhs".to_owned(),
            target_kind: kind.to_owned(),
            identity: identity.to_owned(),
            display_name: None,
            identity_facts: None,
        }
    }

    /// 关键词 identity 只有 term；排序由另一个 monitor-rule 合同承担。
    #[test]
    fn a_keyword_is_taken_by_its_term() {
        let keyword = parse_intake(&intake("keyword", "ADHD")).expect("term alone is enough");
        assert_eq!(keyword.key(), "adhd");
    }

    #[test]
    fn a_legacy_ranking_payload_is_ignored_not_stored_as_a_target_fact() {
        let intake: TargetIntake = serde_json::from_value(serde_json::json!({
            "platform":"xhs", "targetKind":"keyword", "identity":"ADHD",
            "ranking":"most_liked"
        }))
        .unwrap();
        assert_eq!(parse_intake(&intake).unwrap().key(), "adhd");
    }

    #[test]
    fn creators_are_taken_by_platform_id_and_keywords_are_normalised() {
        let creator = parse_intake(&intake("creator", " 5ebe6d21 ")).unwrap();
        assert_eq!(creator.key(), "5ebe6d21");

        let keyword = parse_intake(&intake("keyword", " ADHD ")).unwrap();
        assert_eq!(keyword.key(), "adhd");
    }

    #[test]
    fn unsupported_platforms_and_kinds_are_named_not_lumped_together() {
        let mut douyin = intake("creator", "abc");
        douyin.platform = "douyin".to_owned();
        assert_eq!(
            parse_intake(&douyin).unwrap_err(),
            IntakeRejection::UnsupportedPlatform
        );
        assert_eq!(
            parse_intake(&intake("topic", "abc")).unwrap_err(),
            IntakeRejection::UnknownTargetKind
        );
        assert_eq!(
            parse_intake(&intake("creator", "   ")).unwrap_err(),
            IntakeRejection::EmptyIdentity
        );
    }

    #[test]
    fn creator_url_is_rejected_before_storage() {
        assert_eq!(
            parse_intake(&intake(
                "creator",
                "https://www.xiaohongshu.com/user/profile/5ebe6d21",
            ))
            .unwrap_err(),
            IntakeRejection::CreatorIdentityMustBePlatformId
        );
    }
}
