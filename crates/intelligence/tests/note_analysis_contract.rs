use linggan_intelligence::note_analysis::*;
use serde_json::json;
use uuid::Uuid;
#[path = "support/note_analysis_response.rs"]
mod wire;
fn packet() -> Packet {
    Packet {
        domain_ref: Uuid::new_v4(),
        work_ref: Uuid::new_v4(),
        scope: Scope::NoteWithComments,
        as_of: "2026-11-04T00:00:00Z".into(),
        material_snapshot_identity: "v1".into(),
        coverage: json!({}),
        sources: vec![
            Source {
                citation_id: "body".into(),
                kind: "material".into(),
                source_ref: Uuid::new_v4(),
                source_version: "v1".into(),
                start: 7,
                text: "孩子🙂准备开始。".into(),
                speaker: "author".into(),
                parent_citation_id: None,
            },
            Source {
                citation_id: "c1".into(),
                kind: "comment".into(),
                source_ref: Uuid::new_v4(),
                source_version: "c1".into(),
                start: 0,
                text: "今晚试试。".into(),
                speaker: "other".into(),
                parent_citation_id: None,
            },
        ],
    }
}
#[test]
fn canonical_quotes_and_purpose_projections() {
    let p = packet();
    let o = accept(wire::response(&json!({"material":p})), &p).unwrap();
    assert_eq!(o.evidence[0].start, 7);
    assert_eq!(o.evidence[0].end, 7 + p.sources[0].text.chars().count());
    for purpose in ["topic", "comment"] {
        let c = context(&o, Uuid::new_v4(), "v1", purpose).unwrap();
        assert!(c.get("feedback").is_none());
        assert!(c.get("explanations").is_none());
        assert!(
            c["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .all(|e| e["kind"] == "material")
        );
    }
}
#[test]
fn forged_quotes_and_wrong_dependencies_fail() {
    let p = packet();
    let value = wire::response(&json!({"material":p}));
    let mut v = value.clone();
    v["evidence"][0]["quote"] = json!("没有这句话");
    assert!(accept(v, &p).is_err());
    let mut v = value.clone();
    v["tags"][0]["judgmentIds"] = json!(["unknown"]);
    assert!(accept(v, &p).is_err());
    let mut v = value;
    v["explanations"][0]["feedbackIds"] = json!(["f1"]);
    assert!(accept(v, &p).is_err());
}
#[test]
fn no_comments_and_no_invented_feedback() {
    let mut p = packet();
    p.scope = Scope::Note;
    p.sources.truncate(1);
    let value = wire::response(&json!({"material":p}));
    let o = accept(value.clone(), &p).unwrap();
    assert!(o.feedback.is_empty());
    let mut v = value;
    v["feedback"] =
        json!([{"id":"r","text":"用户反馈有效","kind":"observation","evidenceRefs":["e1"]}]);
    assert!(accept(v, &p).is_err());
}
#[test]
fn source_change_not_time_changes_fingerprint() {
    let p = packet();
    let mut q = p.clone();
    q.as_of = "later".into();
    q.coverage = json!({"platformCommentCount":99});
    assert_eq!(p.fingerprint(), q.fingerprint());
    q.sources[0].text.push('新');
    assert_ne!(p.fingerprint(), q.fingerprint());
}
#[test]
fn ambiguous_quote_is_rejected() {
    let mut p = packet();
    p.sources[0].text = "相同，相同".into();
    let mut v = wire::response(&json!({"material":p}));
    v["evidence"][0]["quote"] = json!("相同");
    assert!(accept(v, &p).is_err());
}

#[test]
fn platform_counts_stay_out_of_model_material() {
    let mut p = packet();
    p.coverage = json!({"platformCommentCount":999,"includedCommentCount":1});
    assert!(
        p.model_material()["coverage"]
            .get("platformCommentCount")
            .is_none()
    );
    assert_eq!(p.model_material()["coverage"]["includedCommentCount"], 1);
    assert_eq!(p.coverage["platformCommentCount"], 999);
}

#[test]
fn feedback_only_explanation_is_rejected_but_no_explanation_is_allowed() {
    let p = packet();
    let mut v = wire::response(&json!({"material":p}));
    v["explanations"][0]["featureIds"] = json!([]);
    assert!(accept(v.clone(), &p).is_err());
    // Pure feedback coverage belongs in limitations, not an invented feature link.
    v["explanations"] = json!([]);
    v["methodCandidates"] = json!([]);
    v["limitations"] = json!(["评论只有尝试意向，未见使用结果。"]);
    assert!(accept(v, &p).is_ok());
}

#[test]
fn quoted_source_typo_must_not_be_silently_corrected() {
    let mut p = packet();
    p.sources[0].text = "运动Cu进多巴胺。".into();
    let value = wire::response(&json!({"material":p}));
    assert!(accept(value.clone(), &p).is_ok());
    let mut corrected = value;
    corrected["evidence"][0]["quote"] = json!("运动Cu促多巴胺。");
    assert!(accept(corrected, &p).is_err());
}

#[test]
fn tags_use_base_judgments_while_methods_can_use_explanations() {
    let p = packet();
    let value = wire::response(&json!({"material":p}));
    for id in ["u1", "f1", "r1"] {
        let mut v = value.clone();
        v["tags"][0]["judgmentIds"] = json!([id]);
        assert!(accept(v, &p).is_ok(), "base judgment {id}");
    }
    let mut inferred = value.clone();
    inferred["understanding"]["discussion"]["kind"] = json!("interpretation");
    inferred["tags"][0]["judgmentIds"] = json!(["u1"]);
    assert!(accept(inferred, &p).is_ok());
    for refs in [json!(["x1"]), json!(["f1", "x1"]), json!(["m1"])] {
        let mut v = value.clone();
        v["tags"][0]["label"] = json!("可能帮助理解（假设）");
        v["tags"][0]["judgmentIds"] = refs;
        assert_eq!(accept(v, &p).unwrap_err(), "tag_basis");
    }
    // The existing method still cites x1: narrowing tags does not remove hypothesis research.
    let accepted = accept(value, &p).unwrap();
    assert_eq!(accepted.method_candidates[0].basis_ids, vec!["x1"]);
}
