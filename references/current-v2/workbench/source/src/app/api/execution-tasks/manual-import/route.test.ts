import { beforeEach, describe, expect, it, vi } from "vitest";

import { buildSubmission } from "@/lib/evidence/ingress/test-fixtures";

const mocks = vi.hoisted(() => ({
  requirePluginAuthorizationRequest: vi.fn(),
  requirePluginDataWorkspaceContext: vi.fn(),
  pluginAuthorizationErrorResponse: vi.fn(),
  pluginDataWorkspaceErrorResponse: vi.fn(),
  submitManualImportEvidence: vi.fn(),
}));

vi.mock("@/lib/services/plugin-authorization-service", () => ({
  requirePluginAuthorizationRequest: mocks.requirePluginAuthorizationRequest,
}));
vi.mock("@/lib/plugin-authorization-route", () => ({
  pluginAuthorizationErrorResponse: mocks.pluginAuthorizationErrorResponse,
}));
vi.mock("@/lib/plugin-data-workspace", () => ({
  requirePluginDataWorkspaceContext: mocks.requirePluginDataWorkspaceContext,
  pluginDataWorkspaceErrorResponse: mocks.pluginDataWorkspaceErrorResponse,
}));
vi.mock("@/lib/evidence/ingress/evidence-ingress-orchestrator", () => ({
  submitManualImportEvidence: mocks.submitManualImportEvidence,
}));

import { POST } from "@/app/api/execution-tasks/manual-import/route";

describe("POST /api/execution-tasks/manual-import", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.requirePluginAuthorizationRequest.mockResolvedValue({
      id: "auth-1",
      workspaceId: "workspace-1",
    });
    mocks.requirePluginDataWorkspaceContext.mockResolvedValue({
      workspaceId: "workspace-1",
      userId: "user-1",
    });
    mocks.pluginAuthorizationErrorResponse.mockReturnValue(null);
    mocks.pluginDataWorkspaceErrorResponse.mockReturnValue(null);
    mocks.submitManualImportEvidence.mockResolvedValue({
      status: "committed",
      captureId: "capture-1",
      receiptId: "receipt-1",
    });
  });

  it("submits a strictly validated V2 body using authenticated authority", async () => {
    const body = buildSubmission("manual_import").body;
    const request = new Request("http://localhost:3000/api/execution-tasks/manual-import", {
      method: "POST",
      headers: { "content-type": "application/json", authorization: "Bearer plugin-token" },
      body: JSON.stringify(body),
    });

    const response = await POST(request as never);

    expect(response.status).toBe(200);
    expect(mocks.submitManualImportEvidence).toHaveBeenCalledWith({
      body,
      context: {
        workspaceId: "workspace-1",
        userId: "user-1",
        pluginAuthorization: { id: "auth-1", workspaceId: "workspace-1" },
      },
    });
  });

  it.each([null, [], {}, { records: [] }, { result: { records: [] } }])(
    "rejects legacy or malformed JSON before auth and persistence: %j",
    async (body) => {
      const request = new Request("http://localhost:3000/api/execution-tasks/manual-import", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });

      const response = await POST(request as never);

      expect(response.status).toBe(400);
      expect(mocks.requirePluginAuthorizationRequest).not.toHaveBeenCalled();
      expect(mocks.requirePluginDataWorkspaceContext).not.toHaveBeenCalled();
      expect(mocks.submitManualImportEvidence).not.toHaveBeenCalled();
    },
  );

  it("returns HTTP 409 for a retained capture identity conflict", async () => {
    const body = buildSubmission("manual_import").body;
    mocks.submitManualImportEvidence.mockResolvedValueOnce({
      status: "rejected",
      reason: "capture_identity_conflict",
      integrityStatus: "capture_identity_conflict",
      conflictRawSnapshotId: "snapshot-conflict",
    });

    const response = await POST(new Request(
      "http://localhost:3000/api/execution-tasks/manual-import",
      { method: "POST", body: JSON.stringify(body) },
    ) as never);

    expect(response.status).toBe(409);
    await expect(response.json()).resolves.toMatchObject({
      status: "rejected",
      integrityStatus: "capture_identity_conflict",
    });
  });
});
