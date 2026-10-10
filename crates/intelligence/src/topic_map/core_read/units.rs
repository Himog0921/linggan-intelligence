//! Definition qualification and membership statistics never trust a display name.
use super::*;
use sqlx::Row;

pub(super) async fn attach_rules(
    db: &Database,
    q: &TopicMapQuery,
    topics: &mut [TopicMapTopic],
    unavailable: &HashSet<Uuid>,
) -> Result<(), TopicMapError> {
    let rows = sqlx::query("SELECT definition_ref,inclusion_criteria,exclusion_criteria FROM linggan_topic_map_concept_rule WHERE ($1::uuid IS NULL OR domain_ref=$1)")
        .bind(q.domain_ref).fetch_all(db.pool()).await?;
    for topic in topics {
        let rule = rows
            .iter()
            .find(|r| r.get::<Uuid, _>("definition_ref") == topic.definition_ref);
        let criteria = rule.map(|r| {
            (
                r.get::<Vec<String>, _>("inclusion_criteria"),
                r.get::<Vec<String>, _>("exclusion_criteria"),
            )
        });
        apply_rule(topic, criteria, unavailable.contains(&topic.definition_ref));
    }
    Ok(())
}

pub(super) fn apply_rule(
    topic: &mut TopicMapTopic,
    criteria: Option<(Vec<String>, Vec<String>)>,
    unavailable: bool,
) {
    let origin = if criteria.is_some() {
        "machine_induced"
    } else if topic.lifecycle_state == "candidate" {
        "legacy"
    } else {
        "manual"
    };
    let (included, excluded) = if unavailable {
        topic.display_name = "来源受限主题".into();
        topic.definition_text = "该机器主题的定义来源当前不可用，等待重新确认。".into();
        topic.direct_work_refs.clear();
        (vec![], vec![])
    } else {
        criteria.unwrap_or_default()
    };
    topic.core = json!({"inclusionCriteria":included,"exclusionCriteria":excluded,
        "definitionSource":origin,"sourceState":if unavailable{"source_unavailable"}else{"available"},
        "discussionCount":0,"evidenceRoles":{"support":0,"challenge":0,"context":0},"relations":[]});
}

fn exact_topic<'a>(reference: &Value, topics: &'a [TopicMapTopic]) -> Option<&'a TopicMapTopic> {
    topics.iter().find(|t| {
        reference["topicRef"].as_str() == Some(&t.topic_ref.to_string())
            && reference["definitionRef"].as_str() == Some(&t.definition_ref.to_string())
            && t.lifecycle_state != "superseded"
            && t.core["sourceState"] != "source_unavailable"
    })
}

pub(super) fn definition_dependencies(unit: &Value) -> impl Iterator<Item = Uuid> + '_ {
    unit["comparedDefinitionRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(
            unit["assignments"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|a| &a["definitionRef"]),
        )
        .chain(
            unit["relations"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|r| &r["definitionRef"]),
        )
        .filter_map(|v| v.as_str().and_then(|s| s.parse().ok()))
}

pub(super) fn invalidate_annotation(work: &mut TopicMapWork, unavailable: &HashSet<Uuid>) {
    let affected = work
        .annotation
        .as_ref()
        .and_then(|a| a["definition_ref"].as_str())
        .and_then(|s| s.parse::<Uuid>().ok())
        .is_some_and(|id| unavailable.contains(&id));
    if !affected {
        return;
    }
    work.annotation = None;
    work.main_stage = "pending".into();
    work.involved_stages.clear();
    work.overlays.clear();
    work.path = "unknown".into();
}

pub(super) fn current_unit(
    unit: &Value,
    result: Option<Uuid>,
    topics: &[TopicMapTopic],
    unavailable: &HashSet<Uuid>,
) -> Value {
    let mut unit = unit.clone();
    unit["researchResultRef"] = json!(result);
    let withdrawn = definition_dependencies(&unit).any(|d| unavailable.contains(&d));
    let references_current = ["assignments", "relations"].iter().all(|key| {
        unit[*key]
            .as_array()
            .into_iter()
            .flatten()
            .all(|r| exact_topic(r, topics).is_some())
    });
    let compared_current = unit["comparedDefinitionRefs"]
        .as_array()
        .into_iter()
        .flatten()
        .all(|d| {
            topics.iter().any(|t| {
                d.as_str() == Some(&t.definition_ref.to_string())
                    && t.lifecycle_state != "superseded"
            })
        });
    if withdrawn || !references_current || !compared_current {
        unit["status"] = json!("uncertain");
        unit["reason"] = json!(if withdrawn {
            "比较所依赖的主题定义来源当前不可用，保留原讨论，归属等待重新判断。"
        } else {
            "主题定义已更新，保留原讨论，归属等待重新判断。"
        });
        unit["assignments"] = json!([]);
        unit["relations"] = json!([]);
        // Recall diagnostics may contain the former candidate's label or explanation.
        unit.as_object_mut().unwrap().remove("recall");
    }
    unit
}

pub(super) fn attach_membership(unit: &Value, work: Uuid, topics: &mut [TopicMapTopic]) {
    if !["matched", "new"].contains(&unit["status"].as_str().unwrap_or("")) {
        return;
    }
    for assignment in unit["assignments"].as_array().into_iter().flatten() {
        let reference = exact_topic(assignment, topics).map(|t| t.topic_ref);
        if let Some(topic) = topics.iter_mut().find(|t| Some(t.topic_ref) == reference)
            && !topic.direct_work_refs.contains(&work)
        {
            topic.direct_work_refs.push(work);
        }
    }
}

fn work_units(work: &TopicMapWork) -> impl Iterator<Item = &Value> {
    work.research
        .as_ref()
        .and_then(|r| r.pointer("/core/units"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

pub(super) fn summarize_scope(works: &[TopicMapWork], topics: &mut [TopicMapTopic]) {
    for topic in topics.iter_mut() {
        topic.core["discussionCount"] = json!(0);
        topic.core["evidenceRoles"] = json!({"support":0,"challenge":0,"context":0});
        topic.core["relations"] = json!([]);
    }
    let mut counted = HashSet::new();
    for work in works {
        for unit in work_units(work) {
            let Some(id) = unit["unitId"].as_str() else {
                continue;
            };
            if !["new", "matched"].contains(&unit["status"].as_str().unwrap_or("")) {
                continue;
            }
            for assignment in unit["assignments"].as_array().into_iter().flatten() {
                let reference = exact_topic(assignment, topics).map(|t| t.topic_ref);
                let Some(topic) = topics.iter_mut().find(|t| Some(t.topic_ref) == reference) else {
                    continue;
                };
                if counted.insert((topic.topic_ref, work.work_ref, id.to_owned())) {
                    topic.core["discussionCount"] =
                        json!(topic.core["discussionCount"].as_u64().unwrap_or(0) + 1);
                    let role = unit["evidenceRole"]
                        .as_str()
                        .filter(|r| ["support", "challenge", "context"].contains(r))
                        .unwrap_or("context");
                    topic.core["evidenceRoles"][role] =
                        json!(topic.core["evidenceRoles"][role].as_u64().unwrap_or(0) + 1);
                }
            }
        }
    }
    attach_relations(works, topics);
}

fn attach_relations(works: &[TopicMapWork], topics: &mut [TopicMapTopic]) {
    for work in works {
        for unit in work_units(work).filter(|u| u["status"] == "new") {
            let source = unit["assignments"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|a| exact_topic(a, topics))
                .map(|t| t.topic_ref);
            for relation in unit["relations"].as_array().into_iter().flatten() {
                let Some(target) = exact_topic(relation, topics) else {
                    continue;
                };
                if Some(target.topic_ref) == source {
                    continue;
                }
                let value = json!({"topicRef":target.topic_ref,"displayName":target.display_name,
                    "relation":relation["relation"],"reason":relation["reason"]});
                if let Some(topic) = topics.iter_mut().find(|t| Some(t.topic_ref) == source)
                    && let Some(relations) = topic.core["relations"].as_array_mut()
                    && !relations.contains(&value)
                {
                    relations.push(value);
                }
            }
        }
    }
}
