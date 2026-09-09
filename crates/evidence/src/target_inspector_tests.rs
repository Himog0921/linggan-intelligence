use super::*;

fn facts() -> ArchiveFacts {
    ArchiveFacts {
        started: true,
        attempted: true,
        profiles: 1,
        works: 42,
        details: 42,
        retired_works: 0,
        quarantined: 0,
        blocked_details: 0,
        standard_directory_ready: true,
    }
}

#[test]
fn known_zero_is_distinct_from_unknown() {
    assert_ne!(
        TargetInspectorCount::Known(0),
        TargetInspectorCount::Unknown
    );
    let (_, coverage) =
        archive_projection("creator", &facts(), TargetInspectorExecutionState::Idle);
    assert_eq!(coverage.missing_details, TargetInspectorCount::Known(0));
}

#[test]
fn queued_work_is_not_running_and_needs_no_manual_action() {
    let execution = execution_projection(&LaneCounts {
        queued: 1,
        ..LaneCounts::default()
    });
    let (archive, coverage) = archive_projection("creator", &facts(), execution.state);
    assert_eq!(execution.state, TargetInspectorExecutionState::Queued);
    assert_eq!(execution.running_attempts, 0);
    assert_eq!(
        resolve_action(&archive, &coverage, &execution, false),
        TargetInspectorAction::NoActionQueued
    );
}

#[test]
fn a_claim_without_an_attempt_is_awaiting_producer_not_running() {
    let execution = execution_projection(&LaneCounts {
        awaiting: 1,
        ..LaneCounts::default()
    });
    assert_eq!(
        execution.state,
        TargetInspectorExecutionState::AwaitingProducer
    );
    assert_eq!(execution.running_attempts, 0);
}

#[test]
fn an_unrecovered_block_requires_human_action() {
    let mut blocked = facts();
    blocked.blocked_details = 1;
    let execution = execution_projection(&LaneCounts {
        blocked: 1,
        ..LaneCounts::default()
    });
    let (archive, coverage) = archive_projection("creator", &blocked, execution.state);
    assert_eq!(archive.state, TargetInspectorArchiveState::Blocked);
    assert_eq!(
        resolve_action(&archive, &coverage, &execution, true),
        TargetInspectorAction::HandleArchiveProblems
    );
}
