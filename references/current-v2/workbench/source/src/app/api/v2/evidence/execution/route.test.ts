import { beforeEach, describe, expect, it, vi } from "vitest";
import { NextRequest } from "next/server";
import { createHash } from "node:crypto";

const mocks = vi.hoisted(() => ({
  submitExecutionEvidence: vi.fn(),
  genericDbAccess: vi.fn(),
  genericAuthorizationAccess: vi.fn(),
}));

vi.mock("@/lib/evidence/ingress/evidence-ingress-orchestrator", () => ({
  submitExecutionEvidence: mocks.submitExecutionEvidence,
}));
vi.mock("@/lib/db", () => ({
  prisma: new Proxy({}, { get: () => mocks.genericDbAccess }),
}));
vi.mock("@/lib/services/plugin-authorization-service", () => ({
  requirePluginAuthorizationRequest: mocks.genericAuthorizationAccess,
}));

import { POST } from "./route";

describe("POST /api/v2/evidence/execution", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const key of [
      "V2_CUTOVER_CANARY_TOKEN_SHA256",
      "V2_CUTOVER_CANARY_STATION_ID",
      "V2_CUTOVER_CANARY_WORKSPACE_ID",
      "V2_CUTOVER_CANARY_MATERIAL_ID",
      "V2_CUTOVER_CANARY_REQUEST_RECEIPT",
    ]) delete process.env[key];
    mocks.submitExecutionEvidence.mockResolvedValue({
      status: "committed",
      integrityStatus: "verified",
      rawSnapshotId: "snapshot-1",
      receiptId: "receipt-1",
      checksumValue: "a".repeat(64),
    });
  });

  it("passes the exact raw body and signed headers to the strict execution seam", async () => {
    const bodyText = '{"body":{"header":"sentinel"}}';
    const request = requestWithHeaders(bodyText);

    const response = await POST(request as never);

    expect(response.status).toBe(200);
    await expect(response.json()).resolves.toMatchObject({ ok: true, status: "committed" });
    expect(mocks.submitExecutionEvidence).toHaveBeenCalledWith(expect.objectContaining({
      stationId: "station-1",
      stationToken: "station-token",
      pluginAuthorizationId: "auth-1",
      pluginAuthorizationToken: "plugin-token",
      method: "POST",
      path: "/api/v2/evidence/execution",
      bodyText,
    }));
    const forwarded = mocks.submitExecutionEvidence.mock.calls[0]?.[0];
    expect(forwarded.headers.get("x-cw-plugin-version")).toBe("2.0.92");
    expect(mocks.genericDbAccess).not.toHaveBeenCalled();
    expect(mocks.genericAuthorizationAccess).not.toHaveBeenCalled();
  });

  it("does not report success while execution control is pending", async () => {
    mocks.submitExecutionEvidence.mockResolvedValueOnce({
      status: "evidence_committed_control_pending",
      rawSnapshotId: "snapshot-1",
      receiptId: "receipt-1",
      checksumValue: "a".repeat(64),
      retryable: true,
    });
    const response = await POST(requestWithHeaders("{}") as never);
    expect(response.status).toBe(503);
    await expect(response.json()).resolves.toMatchObject({ ok: false, retryable: true });
  });

  it("rejects stale execution context as non-retryable so it cannot block newer outbox entries", async () => {
    mocks.submitExecutionEvidence.mockRejectedValueOnce(
      new Error("Execution submission is not bound to a verified current execution context."),
    );

    const response = await POST(requestWithHeaders("{}") as never);

    expect(response.status).toBe(409);
    await expect(response.json()).resolves.toMatchObject({
      ok: false,
      reason: "execution_context_not_current",
      retryable: false,
    });
  });

  it.each(["blocked", "error", "cancelled"])(
    "acknowledges %s terminal Evidence transport without claiming execution success",
    async (terminalState) => {
      mocks.submitExecutionEvidence.mockResolvedValueOnce({
        status: "evidence_committed_terminal",
        terminalState,
        rawSnapshotId: "snapshot-1",
        receiptId: "receipt-1",
        checksumValue: "a".repeat(64),
        retryable: false,
      });
      const response = await POST(requestWithHeaders("{}") as never);
      expect(response.status).toBe(200);
      await expect(response.json()).resolves.toMatchObject({
        ok: true,
        executionSucceeded: false,
        status: "evidence_committed_terminal",
        terminalState,
        retryable: false,
      });
    },
  );

  it("rejects missing trust-boundary headers before the ingress seam", async () => {
    const response = await POST(new NextRequest("http://localhost/api/v2/evidence/execution", {
      method: "POST",
      body: "{}",
    }) as never);
    expect(response.status).toBe(400);
    expect(mocks.submitExecutionEvidence).not.toHaveBeenCalled();
  });

  it("binds a maintenance canary receipt to the capture identity before ingress", async () => {
    const token = configureCutoverCanary();
    const response = await POST(requestWithHeaders(
      JSON.stringify({ header: { captureId: "capture-attacker" } }),
      cutoverHeaders(token),
    ) as never);

    expect(response.status).toBe(400);
    await expect(response.json()).resolves.toMatchObject({ ok: false, reason: "invalid_cutover_canary_binding" });
    expect(mocks.submitExecutionEvidence).not.toHaveBeenCalled();
  });

  it("rejects replay as a second successful use of the frozen canary receipt", async () => {
    const token = configureCutoverCanary();
    mocks.submitExecutionEvidence.mockResolvedValueOnce({
      status: "replay",
      integrityStatus: "verified",
      rawSnapshotId: "snapshot-1",
      receiptId: "receipt-1",
      checksumValue: "a".repeat(64),
    });
    const response = await POST(requestWithHeaders(
      JSON.stringify({ header: { captureId: "capture-canary" } }),
      cutoverHeaders(token),
    ) as never);

    expect(response.status).toBe(409);
    await expect(response.json()).resolves.toMatchObject({
      ok: false,
      reason: "cutover_canary_already_consumed",
    });
  });
});

function requestWithHeaders(body: string, extraHeaders: Record<string, string> = {}): NextRequest {
  return new NextRequest("http://localhost/api/v2/evidence/execution", {
    method: "POST",
    headers: {
      authorization: "Bearer plugin-token",
      "x-cw-station-id": "station-1",
      "x-cw-station-token": "station-token",
      "x-cw-plugin-authorization-id": "auth-1",
      "x-cw-plugin-version": "2.0.92",
      "x-cw-timestamp": "2026-08-12T00:00:00.000Z",
      "x-cw-nonce": "nonce-1",
      "x-cw-body-sha256": "a".repeat(64),
      "x-cw-signature": "b".repeat(64),
      ...extraHeaders,
    },
    body,
  });
}

function configureCutoverCanary(): string {
  const token = "canary-token-1234567890-1234567890-1234567890";
  process.env.V2_CUTOVER_CANARY_TOKEN_SHA256 = createHash("sha256").update(token).digest("hex");
  process.env.V2_CUTOVER_CANARY_STATION_ID = "station-1";
  process.env.V2_CUTOVER_CANARY_WORKSPACE_ID = "workspace-canary";
  process.env.V2_CUTOVER_CANARY_MATERIAL_ID = "material-canary";
  process.env.V2_CUTOVER_CANARY_REQUEST_RECEIPT = "capture-canary";
  return token;
}

function cutoverHeaders(token: string): Record<string, string> {
  return {
    "x-v2-cutover-token": token,
    "x-v2-cutover-workspace-id": "workspace-canary",
    "x-v2-cutover-request-receipt": "capture-canary",
  };
}
