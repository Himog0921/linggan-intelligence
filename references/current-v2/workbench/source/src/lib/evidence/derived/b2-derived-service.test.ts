// @vitest-environment node

import { describe, expect, it, vi } from "vitest";

import { Prisma } from "@/lib/prisma-client";

import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_LIST_SCAN } from "../contracts/xhs-collection-contracts";
import { sha256Hex } from "../ingress/base64";
import { canonicalJsonBytes } from "../ingress/canonical-json";
import type { CapturePackagePayloadV2 } from "../ingress/types";
import {
  B2DerivedService,
  B2SourceInvariantError,
  createEvidenceAnalysisReader,
  type EvidenceAnalysisAuditReceipt,
} from "./b2-derived-service";
import { evaluateXhsContract } from "./xhs-derived-contract";

function receipt(
  overrides: Partial<EvidenceAnalysisAuditReceipt> = {},
): EvidenceAnalysisAuditReceipt {
  return {
    workspaceId: "workspace-1",
    rawSnapshotId: "snapshot-1",
    accessAuditId: "audit-1",
    lifecycleStatus: "ACTIVE",
    integrityStatus: "verified",
    packageBytes: new Uint8Array(),
    packageChecksumAlgorithm: "sha256",
    packageChecksumValue: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    packageContentLength: 0,
    ...overrides,
  };
}

function readerFor(value: EvidenceAnalysisAuditReceipt) {
  return createEvidenceAnalysisReader({
    readCapturePackageAndAudit: vi.fn().mockResolvedValue(value),
  });
}

describe("B2DerivedService Evidence boundary", () => {
  it.each([
    { lifecycleStatus: "REDACTED" as const, integrityStatus: "verified" as const },
    { lifecycleStatus: "PURGED" as const, integrityStatus: "verified" as const },
    { lifecycleStatus: "ACTIVE" as const, integrityStatus: "capture_identity_conflict" as const },
  ])("rejects $lifecycleStatus/$integrityStatus before opening a transaction", async (eligibility) => {
    const db = { $transaction: vi.fn() };
    const service = new B2DerivedService(db as never, readerFor(receipt(eligibility)));

    await expect(service.derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).resolves.toEqual({
      status: "rejected",
      reason: "evidence_ineligible",
      retryable: false,
    });
    expect(db.$transaction).not.toHaveBeenCalled();
  });

  it("rejects receipts that are not bound to the requested snapshot", async () => {
    const db = { $transaction: vi.fn() };
    const service = new B2DerivedService(db as never, readerFor(receipt({
      workspaceId: "other-workspace",
    })));

    await expect(service.derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).resolves.toMatchObject({ status: "rejected", reason: "evidence_ineligible" });
    expect(db.$transaction).not.toHaveBeenCalled();
  });

  it("rejects a same-shaped forged capability", async () => {
    const forgedReader = {
      authorizeAnalysis: vi.fn().mockResolvedValue({ _verifiedEvidenceAnalysisBrand: true }),
    };
    const service = new B2DerivedService({ $transaction: vi.fn() } as never, forgedReader as never);

    await expect(service.derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).rejects.toBeInstanceOf(B2SourceInvariantError);
  });

  it("snapshots caller command and audited receipt before later mutation", async () => {
    const packageValue: CapturePackagePayloadV2 = {
      schemaVersion: "capture-package/v2",
      header: {
        protocolVersion: "capture-submission/v2",
        ingressKind: "manual_import",
        captureId: "capture-1",
        platform: "xhs",
        target: { expectedTargetKey: "xhs:test", observedTargetKey: "xhs:test" },
        observedAt: "2026-08-05T11:58:00.000Z",
        collectorVersion: "test-1.0.0",
        contractId: XHS_LIST_SCAN.id,
        contractVersion: XHS_LIST_SCAN.version,
        contractHash: computeContractHash(XHS_LIST_SCAN),
        sourceSummary: "test",
        report: {
          startedAt: "2026-08-05T11:58:00.000Z",
          completedAt: "2026-08-05T11:58:00.000Z",
          terminal: { state: "completed", reason: "limit_reached", retryable: false },
          slots: [{ slotId: "note_list", status: "observed", reason: null }],
          counters: { requested: 0, discovered: 0, emitted: 0, deduplicated: 0, failed: 0 },
          diagnostics: {},
        },
      },
      records: [],
      artifacts: [],
    };
    const mutableBytes = canonicalJsonBytes(packageValue);
    const packageHash = sha256Hex(mutableBytes);
    const mutableReceipt = receipt({
      packageBytes: mutableBytes,
      packageChecksumValue: packageHash,
      packageContentLength: mutableBytes.length,
    });
    let transactionEntered!: () => void;
    const entered = new Promise<void>((resolve) => { transactionEntered = resolve; });
    let releaseTransaction!: () => void;
    const released = new Promise<void>((resolve) => { releaseTransaction = resolve; });
    const findSnapshot = vi.fn().mockResolvedValue({
      id: "snapshot-1",
      workspaceId: "workspace-1",
      captureId: "capture-1",
      platform: "xhs",
      integrityStatus: "verified",
      checksumAlgorithm: "sha256",
      checksumValue: packageHash,
      contentLength: mutableBytes.length,
      contractId: XHS_LIST_SCAN.id,
      contractVersion: XHS_LIST_SCAN.version,
      contractHash: computeContractHash(XHS_LIST_SCAN),
      records: [],
    });
    const reachedEvaluation = new Error("reached evaluation with immutable receipt");
    const findEvaluation = vi.fn().mockRejectedValue(reachedEvaluation);
    const db = {
      $transaction: vi.fn(async (callback: (tx: unknown) => Promise<unknown>) => {
        transactionEntered();
        await released;
        return callback({
          rawSnapshot: { findFirst: findSnapshot },
          contractEvaluation: { findFirst: findEvaluation },
        });
      }),
    };
    const mutableInput = {
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay" as const,
    };
    const resultPromise = new B2DerivedService(db as never, readerFor(mutableReceipt))
      .derive(mutableInput);
    await entered;
    Object.assign(mutableInput, {
      workspaceId: "workspace-mutated",
      rawSnapshotId: "snapshot-mutated",
      retryMode: "explicit_retry",
    });
    Object.assign(mutableReceipt, {
      workspaceId: "receipt-mutated",
      lifecycleStatus: "REDACTED",
    });
    mutableBytes.fill(0);
    releaseTransaction();

    await expect(resultPromise).rejects.toBe(reachedEvaluation);
    expect(findSnapshot).toHaveBeenCalledWith(expect.objectContaining({
      where: { workspaceId: "workspace-1", id: "snapshot-1" },
    }));
    const expectedEvaluationHash = evaluateXhsContract({
      snapshot: { workspaceId: "workspace-1", rawSnapshotId: "snapshot-1" },
      eligibility: { lifecycleStatus: "ACTIVE", integrityStatus: "verified" },
      contract: XHS_LIST_SCAN,
      terminal: packageValue.header.report.terminal,
      slots: packageValue.header.report.slots,
      members: [],
    }).evaluationInputHash;
    expect(findEvaluation).toHaveBeenCalledWith(expect.objectContaining({
      where: expect.objectContaining({ evaluationInputHash: expectedEvaluationHash }),
    }));
  });

  it("returns a retryable result only after three serialization conflicts", async () => {
    const db = { $transaction: vi.fn().mockRejectedValue({ code: "40001" }) };
    const service = new B2DerivedService(db as never, readerFor(receipt()));

    await expect(service.derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).resolves.toEqual({
      status: "rejected",
      reason: "concurrency_exhausted",
      retryable: true,
    });
    expect(db.$transaction).toHaveBeenCalledTimes(3);
  });

  it("retries only allow-listed Derived P2002 constraints", async () => {
    const expected = new Prisma.PrismaClientKnownRequestError("expected conflict", {
      code: "P2002",
      clientVersion: "test",
      meta: { target: "ContractEvaluation_identity_key" },
    });
    const unexpected = new Prisma.PrismaClientKnownRequestError("unexpected conflict", {
      code: "P2002",
      clientVersion: "test",
      meta: { target: "User_email_key" },
    });
    const retryingDb = { $transaction: vi.fn().mockRejectedValue(expected) };
    const propagatingDb = { $transaction: vi.fn().mockRejectedValue(unexpected) };

    await expect(new B2DerivedService(retryingDb as never, readerFor(receipt())).derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).resolves.toMatchObject({ reason: "concurrency_exhausted" });
    await expect(new B2DerivedService(propagatingDb as never, readerFor(receipt())).derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).rejects.toBe(unexpected);
  });

  it("propagates infrastructure failures without disguising them", async () => {
    const infrastructureFailure = new Error("database unavailable");
    const db = { $transaction: vi.fn().mockRejectedValue(infrastructureFailure) };
    const service = new B2DerivedService(db as never, readerFor(receipt({
      lifecycleStatus: "ARCHIVED",
    })));

    await expect(service.derive({
      workspaceId: "workspace-1",
      rawSnapshotId: "snapshot-1",
      retryMode: "replay",
    })).rejects.toBe(infrastructureFailure);
  });
});
