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

#[test]
fn running_execution_does_not_hide_archive_problems() {
    let mut blocked = facts();
    blocked.quarantined = 2;
    let execution = execution_projection(&LaneCounts {
        running: 1,
        ..LaneCounts::default()
    });
    let (archive, coverage) = archive_projection("creator", &blocked, execution.state);
    assert_eq!(execution.state, TargetInspectorExecutionState::Running);
    assert_eq!(
        resolve_action(&archive, &coverage, &execution, true),
        TargetInspectorAction::HandleArchiveProblems,
        "运行中与隔离材料是并列事实，不能用前者吞掉后者的处理入口"
    );
}

/// **详情缺口归自己的出口管，不能被并进「档案有问题」。**
///
/// 把 `missing_details` 也列进 `resolve_action` 开头那条早返回，`ContinueArchive` 就再也
/// 走不到：抽屉里的「补采缺口」主按钮消失，只剩一句「存在已知阻塞」，而同一个目标在列表
/// 行上仍然写着「补采缺口」——一个缺口两个说法，能一键补采的入口在抽屉里没了。
///
/// 这不是要求把缺口藏起来。覆盖读数与动作无关，照旧渲染 `已取得 / 总数`；要求的是别把
/// 「系统自己会接着做」的活，说成「需要人来处理的已知阻塞」。
///
/// 断言取 `ContinueArchive` 而不是「不等于 HandleArchiveProblems」：后者在
/// `NoActionHealthy` 上也会放行——缺口还在，动作却变成「当前无需处理」。
#[test]
fn a_detail_gap_keeps_its_own_action() {
    let mut pending = facts();
    pending.details = 40; // works 42，欠 2 篇详情
    let execution = execution_projection(&LaneCounts::default());
    let (archive, coverage) = archive_projection("creator", &pending, execution.state);
    assert_eq!(coverage.missing_details, TargetInspectorCount::Known(2));
    assert_eq!(
        resolve_action(&archive, &coverage, &execution, true),
        TargetInspectorAction::ContinueArchive,
        "详情缺口要走 ContinueArchive；并进「已知阻塞」会让补采缺口的主按钮不可达"
    );
}
