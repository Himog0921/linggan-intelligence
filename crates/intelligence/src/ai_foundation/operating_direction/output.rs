use super::*;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Candidate {
    existing_direction_ref: Option<Uuid>,
    label: String,
    definition: String,
    topic_refs: Vec<Uuid>,
    evidence_work_refs: Vec<Uuid>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    candidates: Vec<Candidate>,
}

pub(crate) fn schema() -> Value {
    let ids = json!({"type":"array","items":{"type":"string"}});
    json!({"type":"object","additionalProperties":false,"required":["candidates"],"properties":{"candidates":{"type":"array","items":{
        "type":"object","additionalProperties":false,"required":["existingDirectionRef","label","definition","topicRefs","evidenceWorkRefs"],
        "properties":{"existingDirectionRef":{"type":["string","null"]},"label":{"type":"string"},"definition":{"type":"string"},"topicRefs":ids,"evidenceWorkRefs":ids}
    }}}})
}

pub(crate) fn accepted(text: &str, p: &Packet) -> Option<Vec<Value>> {
    let output: Output = serde_json::from_str(text).ok()?;
    let mut assigned = BTreeSet::new();
    let mut reused = BTreeSet::new();
    let mut result = Vec::new();
    for row in output.candidates {
        let works: BTreeSet<_> = row.evidence_work_refs.iter().copied().collect();
        if works.is_empty()
            || works.len() != row.evidence_work_refs.len()
            || row.topic_refs.is_empty()
            || row.label.trim().is_empty()
            || row.definition.trim().is_empty()
            || row
                .existing_direction_ref
                .is_some_and(|id| !p.existing.contains_key(&id) || !reused.insert(id))
        {
            return None;
        }
        let mut relevant = BTreeSet::new();
        let mut versions = BTreeMap::new();
        for id in &row.topic_refs {
            let t = p.topics.get(id)?;
            if !assigned.insert(*id) || works.is_disjoint(&t.work_refs) {
                return None;
            }
            relevant.extend(t.work_refs.iter().copied());
            versions.insert(*id, t.definition_ref);
        }
        if !works.is_subset(&relevant) {
            return None;
        }
        result.push(json!({"directionRef":row.existing_direction_ref.unwrap_or_else(Uuid::new_v4),"existingDirectionRef":row.existing_direction_ref,
            "label":row.label,"definition":row.definition,"topicRefs":row.topic_refs,"topicDefinitionRefs":versions,"evidenceWorkRefs":works}));
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_proposal_must_cite_each_mapped_topic_and_a_known_direction() {
        let [a, b, w1, w2, d] = std::array::from_fn(|_| Uuid::new_v4());
        let topic = |w| Topic {
            definition_ref: Uuid::new_v4(),
            label: "合成主题".into(),
            work_refs: [w].into(),
        };
        let packet = Packet {
            task: TaskKey::DirectionProposal,
            domain_ref: Uuid::new_v4(),
            topics: [(a, topic(w1)), (b, topic(w2))].into(),
            evidence: BTreeMap::new(),
            existing: [(d, json!({}))].into(),
        };
        let mut c = json!({"existingDirectionRef":d,"label":"合并方向","definition":"一个家庭任务","topicRefs":[a,b],"evidenceWorkRefs":[w1,w2]});
        let evaluate = |c: &Value| accepted(&json!({"candidates":[c]}).to_string(), &packet);
        assert_eq!(evaluate(&c).unwrap()[0]["directionRef"], json!(d));
        c["evidenceWorkRefs"] = json!([w1]);
        assert!(evaluate(&c).is_none());
        c["evidenceWorkRefs"] = json!([w1, w2]);
        c["existingDirectionRef"] = json!(Uuid::new_v4());
        assert!(evaluate(&c).is_none());
        c["existingDirectionRef"] = Value::Null;
        let same = json!({"candidates":[c,c]});
        assert!(accepted(&same.to_string(), &packet).is_none());
        assert_eq!(accepted(r#"{"candidates":[]}"#, &packet), Some(vec![]));
    }
}
