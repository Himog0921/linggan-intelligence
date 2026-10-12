//! Synthetic calibration oracles exercise packet isolation and strict acceptance.
//! Their chosen verdicts do not establish a real reviewer's semantic accuracy.
use super::*;

#[test]
fn unquoted_duration_and_identity_are_absent_from_both_fact_review_packets() {
    for prefix in [
        "三个星期以前我开始陪孩子学习。",
        "我是一位成年家长，转述的是邻居孩子。",
    ] {
        let quote = "我只帮他拆开练习内容。";
        let suffix = "其他情况暂时没有写在这条引用里。";
        let f = source(&format!("{prefix}{quote}{suffix}"));
        let mut d = discussion(&f, "本人已经陪读三周并帮助拆解练习");
        d.evidence[0].start = f.start + prefix.chars().count();
        d.evidence[0].end = d.evidence[0].start + quote.chars().count();
        let extraction =
            extraction_review(&extraction(d.clone()), std::slice::from_ref(&f)).unwrap();
        let (mut units, resolution, candidates, _) = resolution_case();
        units[0].1 = d;
        let resolution = resolution_review(&units, &resolution, &candidates, &[f]).unwrap();
        for packet in [&extraction, &resolution] {
            assert_eq!(packet["evidence"][0]["text"], quote);
            assert!(packet["evidence"][0].get("context").is_none());
            assert!(packet["evidence"][0].get("before").is_none());
            assert!(packet["evidence"][0].get("after").is_none());
            assert!(!packet.to_string().contains(prefix));
            assert!(!packet.to_string().contains(suffix));
        }
        assert_eq!(extraction["outcomeContext"], json!([]));
        let failed = reject(&extraction, "semantic_discussion_unsupported", "fail");
        assert_eq!(
            validate_verification(&failed, &extraction["checks"]),
            Err("semantic_discussion_unsupported")
        );
    }
}

#[test]
fn one_method_can_keep_observed_experience_and_an_explicit_plan_without_claiming_effect() {
    let raw = "今天用讲解练习法更愿意开口，准备明天继续尝试；是否能长期坚持还不知道。";
    let f = source(raw);
    let mut d = discussion(&f, raw);
    d.label = "讲解练习法的体验与尝试安排".into();
    d.definition = "观察讲解练习法的使用体验并记录未来试用安排，不认定长期作用".into();
    d.inclusion_criteria = vec!["涉及同一讲解练习法的体验或安排".into()];
    d.exclusion_criteria = vec!["仅讨论另一种方法的独立障碍".into()];
    let packet = extraction_review(&extraction(d), &[f]).unwrap();
    assert_eq!(packet["evidence"][0]["text"], raw);
    assert_eq!(packet["proposal"]["discussions"][0]["statement"], raw);
    assert_eq!(
        packet["proposal"]["discussions"].as_array().unwrap().len(),
        1
    );
    assert!(validate_verification(&all_pass(&packet), &packet["checks"]).is_ok());
    // The same structure cannot bypass a failed support review of efficacy/persistence.
    let failed = reject(&packet, "semantic_discussion_unsupported", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_discussion_unsupported")
    );
    let uncertain = reject(&packet, "semantic_discussion_non_atomic", "uncertain");
    assert_eq!(
        validate_verification(&uncertain, &packet["checks"]),
        Err("semantic_review_uncertain")
    );
}

#[test]
fn outcome_window_is_only_visible_without_a_factual_proposal() {
    for outcome in ["analyzed", "no_signal", "insufficient"] {
        let f = source("未引用的时长与角色信息不能补充事实。");
        let mut output = extraction(discussion(&f, "一个合成提案"));
        output.outcome = outcome.into();
        let packet = extraction_review(&output, std::slice::from_ref(&f)).unwrap();
        assert_eq!(packet["outcomeContext"], json!([]));
        output.discussions.clear();
        output.scenes.push(super::super::super::Scene {
            label: "未经完整引用的场景".into(),
            evidence: vec![],
        });
        let packet = extraction_review(&output, std::slice::from_ref(&f)).unwrap();
        assert_eq!(packet["outcomeContext"], json!([]));
        output.scenes.clear();
        let packet = extraction_review(&output, std::slice::from_ref(&f)).unwrap();
        assert_eq!(
            !packet["outcomeContext"].as_array().unwrap().is_empty(),
            outcome != "analyzed"
        );
    }
}

#[test]
fn a_journey_quote_does_not_change_the_discussions_exact_citation_mapping() {
    let duration = "六周前开始陪读。";
    let action = "今天帮孩子拆练习内容。";
    let f = source(&format!("{duration}{action}"));
    let mut d = discussion(&f, "陪读六周并帮助孩子拆解练习");
    d.evidence[0].start = f.start + duration.chars().count();
    d.evidence[0].end = f.end;
    let mut output = extraction(d);
    output.journey.evidence.push(Citation {
        fragment_id: f.fragment_id.clone(),
        start: f.start,
        end: f.start + duration.chars().count(),
    });
    let packet = extraction_review(&output, &[f]).unwrap();
    let checks = packet["checks"].as_array().unwrap();
    assert_eq!(
        checks.iter().find(|c| c["checkId"] == "d0.s").unwrap()["subject"],
        "discussions.0"
    );
    assert_eq!(
        checks.iter().find(|c| c["checkId"] == "j0").unwrap()["subject"],
        "journey"
    );
    assert_eq!(
        packet["proposal"]["discussions"][0]["evidenceIds"],
        json!(["e0"])
    );
    assert_eq!(packet["proposal"]["journey"]["evidenceIds"], json!(["e1"]));
    let entries = packet["evidence"].as_array().unwrap();
    assert_eq!(
        entries.iter().find(|e| e["evidenceId"] == "e0").unwrap()["text"],
        action
    );
    assert_eq!(
        entries.iter().find(|e| e["evidenceId"] == "e1").unwrap()["text"],
        duration
    );
    assert_eq!(packet["outcomeContext"], json!([]));
    // The other item's exact quote remains visible in the same model request.
    // This is explicit mapping plus a reviewer constraint, not physical per-check isolation.
    assert!(packet.to_string().contains(duration));
    assert!(VERIFY_SYSTEM.contains("每项事实只允许其自身evidenceIds"));
    let failed = reject(&packet, "semantic_discussion_unsupported", "fail");
    assert_eq!(
        validate_verification(&failed, &packet["checks"]),
        Err("semantic_discussion_unsupported")
    );
}
