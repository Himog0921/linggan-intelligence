//! Versioned, deterministic comparison gates; observation text cannot invent statistics.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Window {
    pub eligible: i64,
    pub analyzed: i64,
    pub comments: i64,
    pub works: i64,
    pub observed_days: i64,
    pub model_versions: i64,
    pub rule_versions: i64,
    pub late_material: i64,
    pub model_signature: Option<String>,
    pub rule_signature: Option<String>,
    pub capture_signature: Option<String>,
    pub unknown_capture: i64,
    pub classification_changes: i64,
}
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub comparable: bool,
    pub reason: &'static str,
    pub previous_share: Option<f64>,
    pub current_share: Option<f64>,
    pub percentage_point_change: Option<f64>,
    pub rising: bool,
    pub rule_version: &'static str,
}
pub fn compare(previous: &Window, current: &Window) -> Comparison {
    let share = |w: &Window| {
        if w.analyzed > 0 {
            Some(w.comments as f64 / w.analyzed as f64)
        } else {
            None
        }
    };
    let previous_share = share(previous);
    let current_share = share(current);
    let coverage = |w: &Window| {
        if w.eligible > 0 {
            w.analyzed as f64 / w.eligible as f64
        } else {
            0.0
        }
    };
    let reason = if previous.eligible < 30 || current.eligible < 30 {
        "样本不足：每个窗口至少需要30条有效评论"
    } else if coverage(previous) < 0.8 || coverage(current) < 0.8 {
        "分析覆盖不足80%"
    } else if (coverage(previous) - coverage(current)).abs() > 0.100000001 {
        "分析覆盖差超过10个百分点"
    } else if previous.classification_changes > 0 || current.classification_changes > 0 {
        "近期有人工归类调整，变化不归因于新讨论"
    } else if previous.unknown_capture > 0
        || current.unknown_capture > 0
        || previous.capture_signature.is_none()
        || previous.capture_signature != current.capture_signature
    {
        "采集范围或规则口径未知或发生变化"
    } else if previous.observed_days < 5 || current.observed_days < 5 {
        "每个窗口至少需要5个有正常观察的完整日"
    } else if current.comments < 5 || current.works < 3 {
        "当前问题至少需要5条评论、3篇作品"
    } else if previous.model_versions != 1
        || current.model_versions != 1
        || previous.rule_versions != 1
        || current.rule_versions != 1
        || previous.model_signature != current.model_signature
        || previous.rule_signature != current.rule_signature
    {
        "模型或规则版本不可比"
    } else if previous.late_material > 0 || current.late_material > 0 {
        "窗口含延迟入库材料，不作讨论变化判断"
    } else if previous.comments == 0 {
        "前期尚无该问题样本，不计算增长率"
    } else {
        "可比样本中占比变化"
    };
    let comparable = reason == "可比样本中占比变化";
    let difference = if comparable {
        current_share
            .zip(previous_share)
            .map(|(c, p)| (c - p) * 100.0)
    } else {
        None
    };
    Comparison {
        comparable,
        reason,
        previous_share,
        current_share,
        percentage_point_change: difference,
        rising: difference.is_some_and(|d| d >= 5.0 - 1e-9),
        rule_version: "comment-change.v1",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn w(n: i64, total: i64) -> Window {
        Window {
            eligible: total,
            analyzed: total,
            comments: n,
            works: 4,
            observed_days: 7,
            model_versions: 1,
            rule_versions: 1,
            late_material: 0,
            model_signature: Some("model.v1".into()),
            rule_signature: Some("comment-research.v3".into()),
            capture_signature: Some("target.rule".into()),
            unknown_capture: 0,
            classification_changes: 0,
        }
    }
    #[test]
    fn volume_is_not_share_growth() {
        let c = compare(&w(10, 100), &w(40, 400));
        assert!(c.comparable);
        assert!(!c.rising);
        assert_eq!(c.percentage_point_change, Some(0.0));
    }
    #[test]
    fn zero_baseline_is_not_infinity() {
        let c = compare(&w(0, 100), &w(10, 100));
        assert!(!c.comparable);
        assert_eq!(c.percentage_point_change, None);
    }
    #[test]
    fn import_and_partial_coverage_do_not_create_trends() {
        let mut cur = w(25, 100);
        cur.late_material = 1;
        assert!(!compare(&w(10, 100), &cur).comparable);
        cur.late_material = 0;
        cur.analyzed = 70;
        assert!(!compare(&w(10, 100), &cur).comparable);
    }
    #[test]
    fn comparable_rise_is_five_percentage_points() {
        assert!(compare(&w(10, 100), &w(15, 100)).rising);
        let mut cur = w(20, 100);
        cur.observed_days = 4;
        assert!(!compare(&w(10, 100), &cur).rising);
    }
}
