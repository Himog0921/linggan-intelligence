import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { buildSubmission } from "../ingress/test-fixtures";
import type { CaptureSubmissionBodyV2 } from "../ingress/types";
import { buildRecoveryIngressBinding } from "./recovery-adapter";

const RECEIVED_AT = new Date("2026-08-10T04:00:00.000Z");

function body(): CaptureSubmissionBodyV2 {
  return buildSubmission("recovery").body;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(RECEIVED_AT);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("recovery authority adapter", () => {
  it.each(["owner", "admin"] as const)(
    "binds a verified %s session using the server clock",
    async (role) => {
      const binding = buildRecoveryIngressBinding(
        body(),
        { workspaceId: "workspace-1", userId: "user-1", role },
      );

      expect(binding.submission.authority).toEqual({
        ingressKind: "recovery",
        workspaceId: "workspace-1",
        receivedAt: RECEIVED_AT,
        sourcePrincipal: "user:user-1",
        recoveryAuthorizedBy: "user:user-1",
      });
      expect(
        await binding.authorityValidator.validate(
          binding.submission.authority,
          binding.submission.body.header,
        ),
      ).toEqual({ valid: true });
    },
  );

  it("rejects a session without owner/admin authority", () => {
    expect(() =>
      buildRecoveryIngressBinding(body(), {
        workspaceId: "workspace-1",
        userId: "user-1",
        role: "editor",
      }),
    ).toThrow(/owner.*admin/i);
  });

  it("rejects a missing authenticated user identity", () => {
    expect(() =>
      buildRecoveryIngressBinding(body(), {
        workspaceId: "workspace-1",
        userId: "  ",
        role: "owner",
      }),
    ).toThrow(/userId/);
  });

  it("rejects authority and execution identity smuggled through the body", () => {
    const leakedBody = body();
    Object.assign(leakedBody as unknown as Record<string, unknown>, {
      recoveryAuthorizedBy: "caller-controlled",
    });
    Object.assign(leakedBody.header as unknown as Record<string, unknown>, {
      attemptId: "fabricated-attempt",
    });

    expect(() =>
      buildRecoveryIngressBinding(leakedBody, {
        workspaceId: "workspace-1",
        userId: "user-1",
        role: "admin",
      }),
    ).toThrow(/server-authority|execution identity/i);
  });

  it("rejects a non-recovery header", () => {
    expect(() =>
      buildRecoveryIngressBinding(buildSubmission("manual_import").body, {
        workspaceId: "workspace-1",
        userId: "user-1",
        role: "owner",
      }),
    ).toThrow(/recovery/);
  });

  it("fails closed if authority is mutated after binding", async () => {
    const binding = buildRecoveryIngressBinding(
      body(),
      { workspaceId: "workspace-1", userId: "user-1", role: "owner" },
    );
    const mutated = {
      ...binding.submission.authority,
      recoveryAuthorizedBy: "user:someone-else",
    };

    expect(
      await binding.authorityValidator.validate(mutated, binding.submission.body.header),
    ).toEqual({
      valid: false,
      reason: "recovery_authority_invalid",
    });
  });

  it.each([
    ["stationId", "station-1"],
    ["leaseToken", "lease-1"],
    ["importerIdentity", "plugin-authorization:1"],
    ["migrationAuthorization", "migration-1"],
    ["jobId", "job-1"],
    ["attemptId", "attempt-1"],
    ["leaseEpoch", 1],
    ["executionPlanVersion", "plan-1"],
    ["unexpected", "value"],
  ])("rejects the extra authority key %s", async (field, value) => {
    const binding = buildRecoveryIngressBinding(
      body(),
      { workspaceId: "workspace-1", userId: "user-1", role: "owner" },
    );
    const authority = {
      ...binding.submission.authority,
      [field]: value,
    };

    expect(
      await binding.authorityValidator.validate(
        authority,
        binding.submission.body.header,
      ),
    ).toEqual({
      valid: false,
      reason: "recovery_authority_invalid",
    });
  });
});
