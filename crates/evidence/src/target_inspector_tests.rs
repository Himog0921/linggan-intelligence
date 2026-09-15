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

/// **受阻详情与隔离材料同一条出口，执行状态吞不掉它。**
///
/// `archive_projection` 只在执行空闲时才走到 `Blocked` 那一支：执行一进入运行或排队，
/// archive 状态先被压成 `Running`/`Queued`，`facts.blocked_details > 0` 就再也标记不出来。
/// 而 `resolve_action` 的开头那条早返回读的正是 `archive.state == Blocked`——它不成立，
/// 紧接着 execution 那一支就返回 `NoActionRunning`，抽屉主操作显示「当前无需处理」。
///
/// 同屏矛盾：同一次投影的「异常」栏由 `coverage.blocked_details` 渲染，写着「N 条待处理」；
/// 通往人工判断失效作品的「查看待取得作品」入口随之消失。一行说有事、一行说没事。
///
/// 隔离材料走同一个守卫，所以它没被吞掉——这条差异就是缺陷本身，不是设计。断言取
/// `HandleArchiveProblems` 而不是「不等于 `NoActionRunning`」：后者在别的动作上也放行，
/// 而这里要钉的是**人的处理入口存在**。
#[test]
fn running_execution_does_not_hide_blocked_details() {
    let mut blocked = facts();
    blocked.details = 40; // works 42，其中 1 篇的详情任务已经卡住
    blocked.blocked_details = 1;
    let execution = execution_projection(&LaneCounts {
        running: 1,
        ..LaneCounts::default()
    });
    let (archive, coverage) = archive_projection("creator", &blocked, execution.state);
    assert_eq!(execution.state, TargetInspectorExecutionState::Running);
    assert_eq!(coverage.blocked_details, TargetInspectorCount::Known(1));
    assert_eq!(
        resolve_action(&archive, &coverage, &execution, true),
        TargetInspectorAction::HandleArchiveProblems,
        "运行中与受阻详情是并列事实，不能用前者吞掉后者的处理入口"
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
