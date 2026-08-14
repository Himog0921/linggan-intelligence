import { NextRequest, NextResponse } from "next/server";

import { pluginDataWorkspaceErrorResponse, requirePluginDataWorkspaceContext } from "@/lib/plugin-data-workspace";
import { pluginAuthorizationErrorResponse } from "@/lib/plugin-authorization-route";
import { requirePluginAuthorizationRequest } from "@/lib/services/plugin-authorization-service";
import { submitManualImportEvidence } from "@/lib/evidence/ingress/evidence-ingress-orchestrator";
import { validateCaptureSubmissionBody } from "@/lib/evidence/ingress/validator";

export async function POST(request: NextRequest) {
  const rawBody: unknown = await request.json().catch(() => null);
  const submission = validateCaptureSubmissionBody(rawBody, "manual_import");
  if (!submission.ok) {
    return NextResponse.json(
      { error: "Invalid CaptureSubmissionV2.", code: submission.reason },
      { status: 400 },
    );
  }

  try {
    const authorization = await requirePluginAuthorizationRequest(request, { touch: true });
    const context = await requirePluginDataWorkspaceContext(request);

    const result = await submitManualImportEvidence({
      body: submission.body,
      context: {
        userId: context.userId,
        workspaceId: context.workspaceId,
        pluginAuthorization: { id: authorization.id, workspaceId: authorization.workspaceId },
      },
    });
    return NextResponse.json(result, {
      status: result.status === "rejected" ? 409 : 200,
    });
  } catch (error) {
    const authorizationResponse = pluginAuthorizationErrorResponse(error);
    if (authorizationResponse) return authorizationResponse;
    const workspaceResponse = pluginDataWorkspaceErrorResponse(error);
    if (workspaceResponse) return workspaceResponse;
    throw error;
  }
}
