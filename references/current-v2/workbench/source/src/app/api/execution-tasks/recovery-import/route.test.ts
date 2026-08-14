import { beforeEach, describe, expect, it, vi } from "vitest";

import { buildSubmission } from "@/lib/evidence/ingress/test-fixtures";

const mocks = vi.hoisted(() => ({
  requireRequestWorkspaceRole: vi.fn(),
  submitRecoveryEvidence: vi.fn(),
}));

vi.mock("@/lib/request-workspace", () => ({
  requireRequestWorkspaceRole: mocks.requireRequestWorkspaceRole,
  WorkspaceAuthError: class WorkspaceAuthError extends Error {},
  WorkspacePermissionError: class WorkspacePermissionError extends Error {},
}));
vi.mock("@/lib/evidence/ingress/evidence-ingress-orchestrator", () => ({
  submitRecoveryEvidence: mocks.submitRecoveryEvidence,
}));

import { POST } from "@/app/api/execution-tasks/recovery-import/route";

describe("POST /api/execution-tasks/recovery-import", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.requireRequestWorkspaceRole.mockResolvedValue({
      workspaceId: "workspace-1",
      userId: "user-1",
      role: "admin",
    });
    mocks.submitRecoveryEvidence.mockResolvedValue({
      status: "committed",
      captureId: "capture-1",
      receiptId: "receipt-1",
    });
  });

  it("submits only a strictly validated recovery V2 body", async () => {
    const body = buildSubmission("recovery").body;
    const request = new Request("http://localhost:3000/api/execution-tasks/recovery-import", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });

    const response = await POST(request as never);

    expect(response.status).toBe(200);
    expect(mocks.submitRecoveryEvidence).toHaveBeenCalledWith({
      body,
      context: { workspaceId: "workspace-1", userId: "user-1", role: "admin" },
    });
  });

  it.each([null, [], {}, { export: { schemaVersion: "plugin-local-recovery/v1" } }])(
    "rejects legacy or malformed JSON before auth and persistence: %j",
    async (body) => {
      const request = new Request("http://localhost:3000/api/execution-tasks/recovery-import", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });

      const response = await POST(request as never);

      expect(response.status).toBe(400);
      expect(mocks.requireRequestWorkspaceRole).not.toHaveBeenCalled();
      expect(mocks.submitRecoveryEvidence).not.toHaveBeenCalled();
    },
  );

  it("returns HTTP 409 for a retained capture identity conflict", async () => {
    const body = buildSubmission("recovery").body;
    mocks.submitRecoveryEvidence.mockResolvedValueOnce({
      status: "rejected",
      reason: "capture_identity_conflict",
      integrityStatus: "capture_identity_conflict",
      conflictRawSnapshotId: "snapshot-conflict",
    });

    const response = await POST(new Request(
      "http://localhost:3000/api/execution-tasks/recovery-import",
      { method: "POST", body: JSON.stringify(body) },
    ) as never);

    expect(response.status).toBe(409);
    await expect(response.json()).resolves.toMatchObject({
      status: "rejected",
      integrityStatus: "capture_identity_conflict",
    });
  });
});
