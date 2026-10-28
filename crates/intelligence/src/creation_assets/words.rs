//! Rule vocabulary and limits (CREATION-ASSETS-001 spec appendix A). The word lists are a
//! starting point that has not been calibrated against real notes, which is why every list-based
//! rule only reminds and never blocks copying; revise [`RULESET_VERSION`] with any change.

pub const RULESET_VERSION: &str = "creation-checks.v2";

/// 疗效承诺
pub const CURE_PROMISES: &[&str] = &[
    "治好",
    "治愈",
    "根治",
    "痊愈",
    "保证能好",
    "保证能改",
    "彻底改掉",
    "不用吃药",
    "停药",
];
/// 讲效果的说法
pub const EFFECT_CLAIMS: &[&str] = &[
    "把行为掰过来",
    "掰过来",
    "基本不拖了",
    "改掉了",
    "变好了",
    "进步很大",
    "效果明显",
    "见效",
    "改善了",
];
/// 像官方或招揽口吻
pub const SOLICITING: &[&str] = &["我们机构", "我们的课", "报名", "咨询", "私信我", "扫码"];

pub const MAX_COVER_CHARS: usize = 10;
pub const MAX_TITLE_CHARS: usize = 20;
pub const MAX_SENTENCE_CHARS: usize = 70;
/// Findings per rule per request, so one pathological text cannot flood the panel.
pub const MAX_FINDINGS_PER_RULE: usize = 30;
