/**
 * POST /api/execution-tasks/recovery-import — plugin_local_recovery 显式恢复导入。
 *
 * 输入：插件「导出恢复数据」产出的 plugin-local-recovery/v1 恢复包。
 * 行为：只写 RawSnapshot（source=plugin_local_recovery，可审计、按 captureId 去重），
 * 绝不更新 TaskAttempt / ExecutionTaskRuntime / ExecutionQueueEntry（报告 §9.3 红线：
 * 旧数据不得伪造为新租约成功；原任务以新 lease 复跑或人工核验）。
 * 权限：owner / admin。建议先带 jobId 单目标演练，核对无误后再全量导入。
 */
import { NextRequest, NextResponse } from "next/server";
import {
  requireRequestWorkspaceRole,
  WorkspaceAuthError,
  WorkspacePermissionError,
} from "@/lib/request-workspace";
import { submitRecoveryEvidence } from "@/lib/evidence/ingress/evidence-ingress-orchestrator";
import { validateCaptureSubmissionBody } from "@/lib/evidence/ingress/validator";

export async function POST(request: NextRequest) {
  const rawBody: unknown = await request.json().catch(() => null);
  const submission = validateCaptureSubmissionBody(rawBody, "recovery");
  if (!submission.ok) {
    return NextResponse.json(
      { error: "Invalid CaptureSubmissionV2.", code: submission.reason },
      { status: 400 },
    );
  }

  try {
    const context = await requireRequestWorkspaceRole(
      request,
      ["owner", "admin"],
      "只有工作区所有者或管理员可以执行恢复导入。"
    );

    const result = await submitRecoveryEvidence({
      body: submission.body,
      context: {
        userId: context.userId ?? "workbench",
        workspaceId: context.workspaceId,
        role: context.role ?? "admin",
      },
    });
    return NextResponse.json(result, {
      status: result.status === "rejected" ? 409 : 200,
    });
  } catch (error) {
    if (error instanceof WorkspaceAuthError) {
      return NextResponse.json({ error: error.message }, { status: 401 });
    }
    if (error instanceof WorkspacePermissionError) {
      return NextResponse.json({ error: error.message }, { status: 403 });
    }
    throw error;
  }
}
