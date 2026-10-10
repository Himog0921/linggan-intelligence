use super::*;

fn captured_tasks(tasks: &[TaskEvidence]) -> Value {
    json!(
        tasks
            .iter()
            .map(|task| json!({
                "taskRef":task.task,"workRef":task.work
            }))
            .collect::<Vec<_>>()
    )
}

#[test]
fn built_comparison_cannot_borrow_a_grant_from_an_earlier_source_task() {
    let (inputs, mut tasks, requested) = pair();
    let dependencies = captured_tasks(&tasks);
    let definitions = json!([Uuid::from_u128(101), Uuid::from_u128(201)]);
    let original = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert!(queue::captured_dependencies_match(
        &original,
        &dependencies,
        &definitions
    ));
    // A newer accepted task can reuse the same text, unit and topic identity.
    // Its authorization is still a different dependency from the captured one.
    tasks[1].task = Uuid::from_u128(3001);
    let replacement = build_comparison(&inputs, &tasks, &requested, &topics()).unwrap();
    assert!(!queue::captured_dependencies_match(
        &replacement,
        &dependencies,
        &definitions
    ));
    assert!(queue::captured_dependencies_match(
        &replacement,
        &captured_tasks(&tasks),
        &definitions
    ));
}

#[test]
fn captured_definitions_cover_assignments_and_unassigned_compared_boundaries() {
    let (mut inputs, mut tasks, requested) = pair();
    let dependencies = captured_tasks(&tasks);
    let definitions = json!([Uuid::from_u128(101), Uuid::from_u128(201)]);
    let mut catalog = topics();
    catalog[0]["definitionRef"] = json!(Uuid::from_u128(102));
    tasks[1].resolutions[0]["assignments"][0]["definitionRef"] = json!(Uuid::from_u128(102));
    for input in &mut inputs {
        input.topics = catalog.clone();
    }
    let revised = build_comparison(&inputs, &tasks, &requested, &catalog).unwrap();
    assert!(!queue::captured_dependencies_match(
        &revised,
        &dependencies,
        &definitions
    ));
    tasks[1].resolutions[0]["assignments"] = json!([assignment(100)]);
    tasks[1].resolutions[0]["comparedDefinitionRefs"] = json!([Uuid::from_u128(202)]);
    catalog = topics();
    catalog[1]["definitionRef"] = json!(Uuid::from_u128(202));
    for input in &mut inputs {
        input.topics = catalog.clone();
    }
    let compared = build_comparison(&inputs, &tasks, &requested, &catalog).unwrap();
    assert!(!queue::captured_dependencies_match(
        &compared,
        &dependencies,
        &definitions
    ));
    assert!(queue::captured_dependencies_match(
        &compared,
        &dependencies,
        &json!([Uuid::from_u128(101), Uuid::from_u128(202)])
    ));
}
