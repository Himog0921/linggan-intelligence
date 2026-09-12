//! COLLECTION-001 · Observation target identity and lifecycle.
//!
//! A target is the long-lived identity of something we intend to keep watching. Storing one
//! proves it was saved and nothing else: it is not an Acquisition Request, not an
//! Authorization, and not a claim that any capture will run (INV-36). The rules this module
//! encodes are in `docs/product/collection-monitoring-rules.md` §2.

use std::fmt;

/// Creator and keyword are two distinct kinds, never one polymorphic target. They differ in
/// what identifies them, in what "new content" means, and in whether deep archiving applies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetKind {
    Creator,
    Keyword,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creator => "creator",
            Self::Keyword => "keyword",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, CollectionContractError> {
        match raw {
            "creator" => Ok(Self::Creator),
            "keyword" => Ok(Self::Keyword),
            _ => Err(CollectionContractError::UnknownTargetKind),
        }
    }
}

/// The lifecycle from the product rules §2.1. Deep archiving must complete before monitoring
/// starts, so `Monitoring` is only reachable through `Archived` — see [`LifecycleState::may_move_to`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LifecycleState {
    /// Stored, decided about by nobody yet. Consumes no platform access.
    PendingDecision,
    /// Deep archiving has been requested. Not "running" — a request is not an execution.
    Archiving,
    /// Deep archiving finished to the standard in DECISION-02: every work has a link and a
    /// like count; other fields may be missing.
    Archived,
    Monitoring,
    Paused,
    /// The person decided not to watch this. Kept rather than deleted, because the decision
    /// itself is a fact worth having.
    Dismissed,
}

impl LifecycleState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PendingDecision => "pending_decision",
            Self::Archiving => "archiving",
            Self::Archived => "archived",
            Self::Monitoring => "monitoring",
            Self::Paused => "paused",
            Self::Dismissed => "dismissed",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, CollectionContractError> {
        match raw {
            "pending_decision" => Ok(Self::PendingDecision),
            "archiving" => Ok(Self::Archiving),
            "archived" => Ok(Self::Archived),
            "monitoring" => Ok(Self::Monitoring),
            "paused" => Ok(Self::Paused),
            "dismissed" => Ok(Self::Dismissed),
            _ => Err(CollectionContractError::UnknownLifecycleState),
        }
    }

    /// Creator-default legal moves. Creator monitoring is deliberately unreachable from
    /// `PendingDecision`; callers that know the target kind must use [`Self::may_move_to_for`].
    pub fn may_move_to(self, next: Self) -> bool {
        self.may_move_to_for(next, TargetKind::Creator)
    }

    /// Kind-aware legal moves. Keywords have no creator-profile deep baseline, so a valid rule
    /// may move them from pending directly into monitoring or paused. Creators retain the strict
    /// pending → archiving → archived → monitoring sequence.
    pub fn may_move_to_for(self, next: Self, kind: TargetKind) -> bool {
        matches!(
            (self, next),
            (Self::PendingDecision, Self::Archiving)
                | (Self::PendingDecision, Self::Dismissed)
                | (Self::Archiving, Self::Archived)
                | (Self::Archiving, Self::PendingDecision)
                | (Self::Archived, Self::Monitoring)
                | (Self::Archived, Self::Archiving)
                | (Self::Archived, Self::Dismissed)
                | (Self::Monitoring, Self::Paused)
                | (Self::Paused, Self::Monitoring)
                | (Self::Paused, Self::Dismissed)
                | (Self::Dismissed, Self::PendingDecision)
        ) || matches!(
            (kind, self, next),
            (
                TargetKind::Keyword,
                Self::PendingDecision,
                Self::Monitoring | Self::Paused
            )
        )
    }
}

/// Where a target came from. Both routes produce the same pending target — a push from the
/// browser is not a weaker claim than a manual add, it just happens while browsing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetSource {
    PluginPush,
    Manual,
}

impl TargetSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PluginPush => "plugin_push",
            Self::Manual => "manual",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, CollectionContractError> {
        match raw {
            "plugin_push" => Ok(Self::PluginPush),
            "manual" => Ok(Self::Manual),
            _ => Err(CollectionContractError::UnknownSource),
        }
    }
}

/// The single platform with a controlled verification base. Others stay closed even though
/// the column could hold them — a platform field is not an open platform (Issue #66).
pub const OPEN_PLATFORM: &str = "xhs";

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CollectionContractError {
    UnknownTargetKind,
    UnknownLifecycleState,
    UnknownSource,
    UnsupportedPlatform,
    EmptyIdentity,
    InvalidCreatorIdentity,
    IllegalTransition,
}

impl fmt::Display for CollectionContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnknownTargetKind => "target kind must be creator or keyword",
            Self::UnknownLifecycleState => "unknown lifecycle state",
            Self::UnknownSource => "target source must be plugin_push or manual",
            Self::UnsupportedPlatform => "only xhs has a controlled verification base",
            Self::EmptyIdentity => "a target must carry a non-empty normalised identity",
            Self::InvalidCreatorIdentity => {
                "creator identity must be a stable platform id, never a URL"
            }
            Self::IllegalTransition => "that lifecycle move is not allowed",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for CollectionContractError {}

/// A target's normalised identity: what deduplication is done on.
///
/// For a creator this is the platform's own stable id, **never a URL** — the same creator is
/// reachable through several URL forms, and matching on URLs is how the legacy workbench ends
/// up with parallel duplicate monitors. For a keyword it is the term plus its ranking, because
/// the same word under two rankings is two different observation surfaces.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TargetIdentity {
    platform: String,
    kind: TargetKind,
    key: String,
}

impl TargetIdentity {
    pub fn creator(
        platform: &str,
        platform_creator_id: &str,
    ) -> Result<Self, CollectionContractError> {
        let platform_creator_id = platform_creator_id.trim();
        if platform_creator_id.is_empty() {
            return Err(CollectionContractError::EmptyIdentity);
        }
        if platform_creator_id.len() > 128
            || !platform_creator_id.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
        {
            return Err(CollectionContractError::InvalidCreatorIdentity);
        }
        Self::build(platform, TargetKind::Creator, platform_creator_id)
    }

    /// `ranking` is part of the identity on purpose: searching one term by "comprehensive" and
    /// by "latest" produces two different surfaces, and treating them as one target would let
    /// each overwrite the other's results.
    /// 一个关键词就是一个观察目标，**排序不进身份**。
    ///
    /// 此前身份是 `{词}::{排序}`，于是「考研自习」按点赞看和按综合看是两个目标，在列表上
    /// 占两行，删一个另一个还在。可这是同一个词的两条巡检口径——口径属于规则（`0076`），
    /// 不属于身份。把排序刻进身份还有一个说不通的地方：同一个词的两个目标各自建一次档，
    /// 各自攒一份历史，而它们面对的是同一批笔记。
    pub fn keyword(platform: &str, term: &str) -> Result<Self, CollectionContractError> {
        let term = term.trim().to_lowercase();
        if term.is_empty() {
            return Err(CollectionContractError::EmptyIdentity);
        }
        Self::build(platform, TargetKind::Keyword, &term)
    }

    fn build(platform: &str, kind: TargetKind, key: &str) -> Result<Self, CollectionContractError> {
        if platform != OPEN_PLATFORM {
            return Err(CollectionContractError::UnsupportedPlatform);
        }
        if key.trim().is_empty() {
            return Err(CollectionContractError::EmptyIdentity);
        }
        Ok(Self {
            platform: platform.to_owned(),
            kind,
            key: key.trim().to_owned(),
        })
    }

    pub fn platform(&self) -> &str {
        &self.platform
    }

    pub fn kind(&self) -> TargetKind {
        self.kind
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitoring_is_unreachable_without_archiving_first() {
        // The product rule this guards: a patrol without a baseline calls every work it sees
        // new, and surge judgement has no distribution to compare against.
        assert!(!LifecycleState::PendingDecision.may_move_to(LifecycleState::Monitoring));
        assert!(LifecycleState::PendingDecision.may_move_to(LifecycleState::Archiving));
        assert!(LifecycleState::Archiving.may_move_to(LifecycleState::Archived));
        assert!(LifecycleState::Archived.may_move_to(LifecycleState::Monitoring));
        assert!(
            LifecycleState::PendingDecision
                .may_move_to_for(LifecycleState::Monitoring, TargetKind::Keyword)
        );
        assert!(
            LifecycleState::PendingDecision
                .may_move_to_for(LifecycleState::Paused, TargetKind::Keyword)
        );
    }

    #[test]
    fn a_creator_is_identified_by_platform_id_never_by_url() {
        let from_id = TargetIdentity::creator("xhs", "5ebe6d210000000001000afe").unwrap();
        assert_eq!(from_id.key(), "5ebe6d210000000001000afe");
        // A URL is not an identity: the same creator has several URL forms, so accepting one
        // would let the same person enter twice under different keys.
        assert_eq!(
            TargetIdentity::creator("xhs", "https://www.xiaohongshu.com/user/profile/5ebe6d21")
                .unwrap_err(),
            CollectionContractError::InvalidCreatorIdentity
        );
    }

    /// **一个词就是一个目标，排序不再分裂它。**
    ///
    /// 排序是巡检口径，属于规则（`0076`）：同一个词按点赞看和按综合看，是它的两条规则，
    /// 不是两个观察对象。此前身份是 `{词}::{排序}`，同一个词在列表上占两行、各建一次档、
    /// 各攒一份历史，而它们面对的是同一批笔记。
    #[test]
    fn one_term_is_one_target_however_it_will_be_ranked() {
        let plain = TargetIdentity::keyword("xhs", "ADHD").unwrap();
        // 大小写与空白照旧归一，同一个词进不来两次。
        let padded = TargetIdentity::keyword("xhs", "  adhd  ").unwrap();
        assert_eq!(plain.key(), padded.key());
        assert_eq!(plain.key(), "adhd");
        let other = TargetIdentity::keyword("xhs", "a娃").unwrap();
        assert_ne!(plain.key(), other.key());
    }

    #[test]
    fn platforms_without_a_verification_base_stay_closed() {
        assert_eq!(
            TargetIdentity::creator("douyin", "abc").unwrap_err(),
            CollectionContractError::UnsupportedPlatform
        );
    }
}
