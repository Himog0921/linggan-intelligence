import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { EvidenceIngress } from "../ingress/evidence-ingress";
import type { EvidenceIngressAuthorityValidator } from "../ingress/authority-validator";
import { buildSubmission, testRegistry } from "../ingress/test-fixtures";
import type { CaptureSubmissionBodyV2 } from "../ingress/types";
import { buildManualImportIngressBinding } from "./manual-import-adapter";

const RECEIVED_AT = new Date("2026-08-10T04:00:00.000Z");

function body(): CaptureSubmissionBodyV2 {
  return buildSubmission("manual_import").body;
}

const context = {
  workspaceId: "workspace-1",
  userId: "user-1",
  pluginAuthorization: {
    id: "plugin-authorization-1",
    workspaceId: "workspace-1",
  },
};

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(RECEIVED_AT);
});

afterEach(() => {
  vi.useRealTimers();
});

describe("manual_import authority adapter", () => {
  it("binds the verified user and plugin authorization using the server clock", async () => {
    const binding = buildManualImportIngressBinding(body(), context);

    expect(binding.submission.authority).toEqual({
      ingressKind: "manual_import",
      workspaceId: "workspace-1",
      receivedAt: RECEIVED_AT,
      sourcePrincipal: "user:user-1",
      importerIdentity: "plugin-authorization:plugin-authorization-1",
    });
    expect(
      await binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).toEqual({ valid: true });
  });

  it("rejects a plugin authorization from a different workspace", () => {
    expect(() =>
      buildManualImportIngressBinding(body(), {
        ...context,
        pluginAuthorization: {
          ...context.pluginAuthorization,
          workspaceId: "workspace-2",
        },
      }),
    ).toThrow(/same workspace/i);
  });

  it("rejects a plugin authorization without a workspace binding", () => {
    expect(() =>
      buildManualImportIngressBinding(body(), {
        ...context,
        pluginAuthorization: {
          ...context.pluginAuthorization,
          workspaceId: null,
        },
      }),
    ).toThrow(/workspace/i);
  });

  it("rejects authority and execution identity smuggled through the body", () => {
    const leakedBody = body();
    Object.assign(leakedBody as unknown as Record<string, unknown>, {
      receivedAt: "caller-controlled",
    });
    Object.assign(leakedBody.header as unknown as Record<string, unknown>, {
      jobId: "fabricated-job",
    });

    expect(() => buildManualImportIngressBinding(leakedBody, context)).toThrow(
      /server-authority|execution identity/i,
    );
  });

  it("rejects a non-manual header", () => {
    expect(() =>
      buildManualImportIngressBinding(buildSubmission("recovery").body, context),
    ).toThrow(/manual_import/);
  });

  it("fails closed if authority is mutated after binding", async () => {
    const binding = buildManualImportIngressBinding(body(), context);
    const mutated = {
      ...binding.submission.authority,
      workspaceId: "workspace-2",
    };

    expect(
      await binding.authorityValidator.validate(mutated, binding.submission.body.header),
    ).toEqual({
      valid: false,
      reason: "manual_import_authority_invalid",
    });
  });

  it.each([
    ["stationId", "station-1"],
    ["leaseToken", "lease-1"],
    ["recoveryAuthorizedBy", "user:recovery"],
    ["migrationAuthorization", "migration-1"],
    ["jobId", "job-1"],
    ["attemptId", "attempt-1"],
    ["leaseEpoch", 1],
    ["executionPlanVersion", "plan-1"],
    ["unexpected", "value"],
  ])("rejects the extra authority key %s", async (field, value) => {
    const binding = buildManualImportIngressBinding(body(), context);
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
      reason: "manual_import_authority_invalid",
    });
  });

  it.each([null, {}, { header: null }])(
    "rejects a malformed body without entering ingress: %j",
    (malformed) => {
      expect(() =>
        buildManualImportIngressBinding(
          malformed as unknown as CaptureSubmissionBodyV2,
          context,
        ),
      ).toThrow(/CaptureSubmissionBodyV2/);
    },
  );

  it("snapshots authority before the asynchronous validator boundary", async () => {
    const binding = buildManualImportIngressBinding(body(), context);

    let releaseValidation!: () => void;
    const validationReleased = new Promise<void>((resolve) => {
      releaseValidation = resolve;
    });
    let validationEntered!: () => void;
    const validatorEntered = new Promise<void>((resolve) => {
      validationEntered = resolve;
    });
    const delayedValidator: EvidenceIngressAuthorityValidator = {
      async validate(authority, header) {
        validationEntered();
        await validationReleased;
        return binding.authorityValidator.validate(authority, header);
      },
    };
    const db = { $transaction: vi.fn() };
    const ingress = new EvidenceIngress(db as never, testRegistry([]), delayedValidator);

    const resultPromise = ingress.submit(binding.submission);
    await validatorEntered;
    Object.assign(binding.submission.authority, {
      workspaceId: "workspace-attacker",
    });
    binding.submission.authority.receivedAt.setTime(0);
    Object.assign(binding.submission.body.header, { captureId: "capture-attacker" });
    binding.submission.body.capturePackage.checksumValue = "0".repeat(64);
    releaseValidation();

    await expect(resultPromise).resolves.toEqual({
      status: "rejected",
      reason: "contract_not_registered",
      retryable: false,
    });
    expect(db.$transaction).not.toHaveBeenCalled();
  });
});
