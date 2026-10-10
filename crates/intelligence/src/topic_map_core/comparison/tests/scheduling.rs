use super::*;

fn dependencies(task: u128, work: u128, request: u128, definitions: &[u128]) -> Value {
    json!([{"taskRef":Uuid::from_u128(task),"workRef":Uuid::from_u128(work),
        "backfill":{"authorizationRequestRef":Uuid::from_u128(request),
            "forcedDefinitionRefs":definitions.iter().map(|id|Uuid::from_u128(*id)).collect::<Vec<_>>()}}])
}

fn grant(request: u128, works: &[u128], definitions: &[u128]) -> Value {
    json!({"backfillAuthorizations":[{"requestRef":Uuid::from_u128(request),
        "workRefs":works.iter().map(|id|Uuid::from_u128(*id)).collect::<Vec<_>>(),
        "definitionRefs":definitions.iter().map(|id|Uuid::from_u128(*id)).collect::<Vec<_>>()}]})
}

#[test]
fn comparison_checkpoint_can_refresh_in_active_authorized_work() {
    let previous = json!({"state":"unavailable","sourceTaskRefs":[Uuid::from_u128(10)]});
    let deps = dependencies(11, 1, 100, &[101]);
    let works = [Uuid::from_u128(1)];
    assert!(!scheduling::may_refresh(
        &json!({}),
        &json!({}),
        &deps,
        &works,
        false,
        false
    ));
    assert!(scheduling::current_on_demand(
        "on_demand",
        "completed",
        &json!({}),
        &json!({})
    ));
    assert!(!scheduling::current_on_demand(
        "historical",
        "completed",
        &json!({}),
        &json!({})
    ));
    assert!(!scheduling::current_on_demand(
        "incremental",
        "queued",
        &json!({}),
        &previous
    ));
    assert!(!scheduling::current_on_demand(
        "on_demand",
        "queued",
        &json!({"backfillReopened":true}),
        &previous
    ));
    assert!(scheduling::may_refresh(
        &json!({}),
        &previous,
        &deps,
        &works,
        true,
        false
    ));
    assert!(scheduling::may_refresh(
        &json!({}),
        &previous,
        &deps,
        &works,
        false,
        true
    ));
    assert!(!scheduling::may_refresh(
        &json!({}),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
}

#[test]
fn renewed_comparison_grant_must_cover_every_side_and_new_definition() {
    let previous = json!({"state":"current","sourceTaskRefs":[Uuid::from_u128(10)]});
    let deps = dependencies(11, 1, 100, &[101]);
    let works = [Uuid::from_u128(1), Uuid::from_u128(2)];
    let complete_scope = grant(100, &[1, 2], &[101]);
    let recall =
        scheduling::renewal_authorization(&complete_scope, &previous, &deps, &works).unwrap();
    assert_eq!(recall["backfill"]["comparisonScopeWorkRefs"], json!(works));
    assert_eq!(
        recall["backfill"]["forcedDefinitionRefs"],
        json!([Uuid::from_u128(101)])
    );
    assert_eq!(
        recall["backfill"]["authorizationRequestRef"],
        json!(Uuid::from_u128(100))
    );
    assert!(scheduling::may_refresh(
        &grant(100, &[1, 2], &[101]),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &grant(100, &[1], &[101]),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &grant(100, &[1, 2], &[102]),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &grant(200, &[1, 2], &[101]),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &grant(100, &[1, 2], &[101]),
        &previous,
        &dependencies(11, 3, 100, &[101]),
        &works,
        false,
        false
    ));
}

#[test]
fn completed_explicit_work_cannot_reuse_a_grant_for_future_unapproved_changes() {
    let previous = json!({"state":"current","sourceTaskRefs":[Uuid::from_u128(11)]});
    let works = [Uuid::from_u128(1)];
    let scope = grant(100, &[1], &[101]);
    assert!(!scheduling::may_refresh(
        &json!({"backfillReopened":true}),
        &json!({}),
        &dependencies(12, 1, 100, &[101]),
        &works,
        false,
        false
    ));
    // Merely changing the catalog while the accepted discussion task stays the
    // same does not turn an old Start into permanent automatic authorization.
    assert!(!scheduling::may_refresh(
        &scope,
        &previous,
        &dependencies(11, 1, 100, &[101]),
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &scope,
        &previous,
        &dependencies(12, 1, 100, &[102]),
        &works,
        false,
        false
    ));
    assert!(!scheduling::may_refresh(
        &scope,
        &previous,
        &dependencies(12, 1, 100, &[]),
        &works,
        false,
        false
    ));
    assert!(scheduling::may_refresh(
        &scope,
        &previous,
        &dependencies(12, 1, 100, &[101]),
        &works,
        false,
        false
    ));
}

#[test]
fn every_changed_source_task_requires_authorization_when_automatic_is_off() {
    let previous = json!({"state":"unavailable","sourceTaskRefs":[Uuid::from_u128(10)]});
    let works = [Uuid::from_u128(1), Uuid::from_u128(2)];
    let mut deps = dependencies(11, 1, 100, &[101]);
    deps.as_array_mut()
        .unwrap()
        .push(dependencies(12, 2, 200, &[101])[0].clone());
    assert!(!scheduling::may_refresh(
        &grant(100, &[1, 2], &[101]),
        &previous,
        &deps,
        &works,
        false,
        false
    ));
    assert!(scheduling::may_refresh(
        &json!({}),
        &previous,
        &deps,
        &works,
        true,
        false
    ));
}
