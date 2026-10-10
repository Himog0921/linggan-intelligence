//! Merge qualified windows without turning a partial task into a saved result.
use super::*;

const ARRAYS: [&str; 6] = [
    "discussions",
    "scenes",
    "responseMatches",
    "angles",
    "productOpportunities",
    "limitations",
];

#[derive(Default)]
struct Projection {
    fragments: Vec<Value>,
    fragment_ids: HashSet<String>,
    units: Vec<Value>,
    unit_ids: HashSet<String>,
    arrays: HashMap<&'static str, Vec<Value>>,
    coverage: work_summary::Coverage,
    journey: work_summary::Journey,
    author: bool,
    comments: bool,
}

impl Projection {
    fn add(
        &mut self,
        part: &Window,
        work: Uuid,
        topics: &mut [TopicMapTopic],
        unavailable: &HashSet<Uuid>,
    ) {
        self.coverage.add(part);
        self.journey.add(part);
        for fragment in &part.input.fragments {
            let owner = fragment_owner(part, fragment);
            if owner == Some(work) {
                self.author |=
                    ["title", "body", "ocr", "transcript"].contains(&fragment.field.as_str());
                self.comments |=
                    ["studied_comment", "unresearched_comment"].contains(&fragment.field.as_str());
            }
            if self.fragment_ids.insert(fragment.fragment_id.clone()) {
                let mut value = json!(fragment);
                value["workRef"] = json!(owner);
                self.fragments.push(value);
            }
        }
        self.add_arrays(part);
        // Comparison analyses retain their cross-work scope; they create no primary-work units.
        if part.is_comparison() {
            return;
        }
        for unit in part.core["units"].as_array().into_iter().flatten() {
            let Some(id) = unit["unitId"].as_str() else {
                continue;
            };
            if !self.unit_ids.insert(id.to_owned()) {
                continue;
            }
            let mut unit = units::current_unit(unit, part.result, topics, unavailable);
            unit["researchTaskRef"] = json!(part.task);
            units::attach_membership(&unit, work, topics);
            self.units.push(unit);
        }
    }

    fn add_arrays(&mut self, part: &Window) {
        let comparison = part
            .is_comparison()
            .then(|| ComparisonScope::from_manifest(&part.manifest, part.input.work.work_ref))
            .flatten();
        let evidence_works = if part.is_comparison() {
            comparison
                .as_ref()
                .map(|scope| scope.selected_work_refs.clone())
                .unwrap_or_default()
        } else {
            vec![part.input.work.work_ref]
        };
        for key in ARRAYS {
            for (index, original) in part.output[key]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let mut value = original.clone();
                if value.is_object()
                    && let Some(scope) = &comparison
                {
                    value["comparisonScope"] = scope.value(part.result);
                }
                if ["angles", "productOpportunities"].contains(&key) {
                    let Some(result) = part.result else {
                        continue;
                    };
                    value["researchResultRef"] = json!(result);
                    value["researchMethodVersion"] = json!(part.method);
                    value["evidenceWorkRefs"] = json!(evidence_works);
                    let field = if key == "angles" {
                        "researchAngleIndex"
                    } else {
                        "researchOpportunityIndex"
                    };
                    value[field] = json!(index);
                }
                let values = self.arrays.entry(key).or_default();
                if !values.contains(&value) {
                    values.push(value);
                }
            }
        }
    }
}

fn fragment_owner(part: &Window, fragment: &Fragment) -> Option<Uuid> {
    if part.is_comparison() {
        return part.input.coverage["fragmentWorkRefs"][&fragment.fragment_id]
            .as_str()?
            .parse()
            .ok();
    }
    Some(part.input.work.work_ref)
}

pub(super) fn project_work(
    work: &mut TopicMapWork,
    mut parts: Vec<Window>,
    topics: &mut [TopicMapTopic],
    unavailable: &HashSet<Uuid>,
) -> Vec<Value> {
    parts.sort_by(|a, b| {
        b.time
            .cmp(&a.time)
            .then(b.result.cmp(&a.result))
            .then(b.task.cmp(&a.task))
    });
    // One current accepted revision owns each source window. Changing the model
    // or its wording cannot turn an older analysis into additional discussions.
    // History and saved results keep their immutable individual references.
    let mut source_windows = HashSet::new();
    parts.retain(|part| {
        part.is_comparison()
            || source_windows.insert(work_summary::semantic_window_key(&part.input))
    });
    let Some(latest) = parts.first() else {
        return vec![];
    };
    let mut projection = Projection::default();
    for part in &parts {
        projection.add(part, work.work_ref, topics, unavailable);
    }
    let mut output = latest.output.clone();
    for key in ARRAYS {
        output[key] = json!(projection.arrays.remove(key).unwrap_or_default());
    }
    output["journey"] = parts
        .iter()
        .find_map(work_summary::author_journey)
        .cloned()
        .unwrap_or_else(partial::pending_journey);
    let candidates = candidates(work.work_ref, &projection.units, &parts);
    let completed: Vec<_> = parts.iter().filter_map(|p| p.result).collect();
    let partial_tasks: Vec<_> = parts
        .iter()
        .filter(|p| p.is_partial())
        .map(|p| p.task)
        .collect();
    work.research = Some(json!({"resultRef":completed.first(),"resultRefs":completed,
        "partialTaskRefs":partial_tasks,"state":if partial_tasks.is_empty(){"available"}else{"partial"},
        "readable":true,"authorSourceState":if work.readable || projection.author{"available"}else{"source_unavailable"},
        "commentSourceState":if projection.comments{"available"}else{"source_unavailable"},
        "methodVersion":latest.method,"createdAt":latest.time,"output":output,"fragments":projection.fragments,
        "core":{"methodVersion":crate::topic_map_core::METHOD_VERSION,"coverage":projection.coverage.value(),"units":projection.units}}));
    projection.journey.apply(work);
    candidates
}

fn candidates(work: Uuid, units: &[Value], parts: &[Window]) -> Vec<Value> {
    units.iter().filter(|u| ["new", "uncertain"].contains(&u["status"].as_str().unwrap_or("")))
        .map(|unit| {
            let part = parts.iter().find(|p| unit["researchTaskRef"] == json!(p.task));
            json!({"label":unit["label"],"workRef":work,"evidence":unit["evidence"],
                "resultRef":unit["researchResultRef"],"taskRef":unit["researchTaskRef"],
                "methodVersion":part.map(|p|p.method.as_str()),"state":unit["status"],"formalRelease":false})
        }).collect()
}
