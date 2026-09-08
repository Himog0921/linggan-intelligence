//! Deterministic observation candidates and publication gating.
use serde_json::{Value, json};
pub(super) fn add_observations(value: &mut Value) {
    use crate::comment_intelligence_statistics::{Window, compare};
    let mut groups = std::collections::BTreeMap::<String, (Value, Value)>::new();
    for item in value["comparisonWindows"].as_array().into_iter().flatten() {
        let key = item["problemRef"].as_str().unwrap_or("").to_owned();
        let pair = groups.entry(key).or_default();
        if item["period"] == "previous" {
            pair.0 = item.clone()
        } else {
            pair.1 = item.clone()
        }
    }
    let mut observations = Vec::new();
    let mut comparisons = Vec::new();
    for (problem, (previous, current)) in groups {
        let p: Window = serde_json::from_value(previous.clone()).unwrap_or_default();
        let c: Window = serde_json::from_value(current.clone()).unwrap_or_default();
        let comparison = compare(&p, &c);
        let name = current["name"].as_str().unwrap_or("用户问题");
        let mut kinds = Vec::new();
        if comparison.rising {
            kinds.push(("share_rise", format!("{name}：可比样本中的占比上升")));
        }
        if comparison.comparable {
            let previous_works = previous["workRefs"].as_array().cloned().unwrap_or_default();
            let new_works = current["workRefs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|w| !previous_works.contains(w))
                .count();
            if c.works > p.works && new_works > 0 {
                kinds.push(("source_expansion", format!("{name}：在本库更多作品中出现")));
            }
        }
        // A zero baseline is not an infinite rise. Only a fully analysed, comparable history
        // and source first-observation dates can support the library-local novelty candidate.
        if comparison.reason == "前期尚无该问题样本，不计算增长率"
            && p.analyzed == p.eligible
            && c.analyzed == c.eligible
        {
            if current["firstProblemIsNew"] == true {
                kinds.push((
                    "first_observed",
                    format!("{name}：本库可用历史中首次观察到"),
                ));
            }
        }
        for (kind, title) in kinds {
            observations.push(json!({"observationRef":format!("{kind}:{problem}"),"type":kind,"title":title,"comments":c.comments,"works":c.works,"denominator":c.analyzed,"sourceRefs":current["sourceRefs"],"problemRef":problem,"reason":comparison.reason,"ruleVersion":comparison.rule_version,"comparison":comparison,"previousWindow":previous,"currentWindow":current,"representative":value["problems"].as_array().into_iter().flatten().find(|p|p["problemRef"]==problem).map(|p|&p["representative"])}));
        }
        comparisons.push(json!({"problemRef":problem,"comparison":comparison,"previousWindow":previous,"currentWindow":current}));
    }
    for g in value["groupObservations"].as_array().into_iter().flatten() {
        let kind = g["type"].as_str().unwrap_or("");
        let target = g["target"].as_str().unwrap_or("");
        let single = g["works"].as_i64() == Some(1);
        let title = if kind == "recurrence" {
            format!("{target}：跨作品反复表达")
        } else if single {
            format!("{target}：该作品内存在分歧")
        } else {
            format!("{target}：不同作品中出现相反立场")
        };
        observations.push(json!({"observationRef":format!("{kind}:{}",crate::comment_research::comment_source_hash(target)),"type":kind,"title":title,"comments":g["comments"],"works":g["works"],"distinctExpressions":g["distinctExpressions"],"sourceRefs":g["sourceRefs"],"representative":g["representative"],"reason":if kind=="recurrence"{"至少5条不同原声、3篇作品表达同一经验或判断"}else{"同一命题两种相反立场各至少2条原声；单篇必须有直接反驳回复"},"ruleVersion":"comment-group.v1"}));
    }
    observations.sort_by(|a, b| {
        let comparable = |v: &Value| v["comparison"]["comparable"] == true;
        comparable(b)
            .cmp(&comparable(a))
            .then_with(|| b["works"].as_i64().cmp(&a["works"].as_i64()))
            .then_with(|| {
                b["comparison"]["percentagePointChange"]
                    .as_f64()
                    .partial_cmp(&a["comparison"]["percentagePointChange"].as_f64())
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| b["comments"].as_i64().cmp(&a["comments"].as_i64()))
            .then_with(|| {
                a["observationRef"]
                    .as_str()
                    .cmp(&b["observationRef"].as_str())
            })
    });
    observations.truncate(3);
    for o in &mut observations {
        o["scope"] = value["scope"].clone();
        o["asOf"] = value["scope"]["asOf"].clone();
        o["limitation"] = json!(
            "仅描述当前库内可读样本；采集排序、覆盖和模型理解仍有局限，不代表整个社区或市场。"
        );
    }
    // Candidate computation is available for calibration; unapproved rules never publish cards.
    if value["rules"]["advancedReleaseEnabled"] != true {
        observations.clear();
    }
    value["observations"] = json!(observations);
    value["comparisons"] = json!(comparisons);
    if let Some(obj) = value.as_object_mut() {
        obj.remove("comparisonWindows");
        obj.remove("groupObservations");
    }
}
