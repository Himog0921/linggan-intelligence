import { describe, expect, it, vi } from "vitest";
import { Prisma } from "@/lib/prisma-client";

import { buildSubmission } from "./test-fixtures";
import type { CaptureSubmissionV2, EvidenceIngressResult } from "./types";
import { finalizeExecutionEvidenceControl } from "./execution-control-finalizer";

const NOW = new Date("2026-08-10T08:00:00.000Z");

function executionSubmission(
  terminalState: "completed" | "blocked" | "cancelled" | "error" = "completed",
): CaptureSubmissionV2 {
  const submission = buildSubmission("execution");
  submission.authority = {
    ingressKind: "execution",
    workspaceId: "workspace-1",
    receivedAt: new Date("2026-08-10T07:59:00.000Z"),
    sourcePrincipal: "execution-station:station-1",
    stationId: "station-1",
    leaseToken: "lease-1",
  };
  Object.assign(submission.body.header, {
    jobId: "job-1",
    attemptId: "attempt-1",
    leaseEpoch: 3,
    executionPlanVersion: "plan-1",
  });
  submission.body.header.report.terminal.state = terminalState;
  return submission;
}

const committed: EvidenceIngressResult = {
  status: "committed",
  rawSnapshotId: "snapshot-1",
  receiptId: "receipt-1",
  integrityStatus: "verified",
  checksumValue: "a".repeat(64),
};

function advancingDb() {
  const tx = {
    executionQueueEntry: {
      updateMany: vi.fn().mockResolvedValue({ count: 1 }),
      findUnique: vi.fn(),
    },
    executionJob: {
      updateMany: vi.fn().mockResolvedValue({ count: 1 }),
      findUnique: vi.fn(),
    },
    taskAttempt: {
      updateMany: vi.fn().mockResolvedValue({ count: 1 }),
      findUnique: vi.fn(),
    },
    executionTaskRuntime: {
      updateMany: vi.fn().mockResolvedValue({ count: 1 }),
      findUnique: vi.fn(),
    },
  };
  const db = {
    $transaction: vi.fn(async (callback) => callback(tx)),
    rawSnapshot: { deleteMany: vi.fn() },
    capturePackage: { deleteMany: vi.fn() },
    evidenceIngressReceipt: { deleteMany: vi.fn() },
  };
  return { db, tx };
}

describe("execution evidence control finalizer", () => {
  it("advances QueueEntry, Job, Attempt, and Runtime after verified Evidence commit", async () => {
    const { db, tx } = advancingDb();

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({ status: "advanced", terminalState: "completed" });

    expect(tx.executionQueueEntry.updateMany).toHaveBeenCalledWith({
      where: {
        jobId: "job-1",
        workspaceId: "workspace-1",
        status: "in_progress",
        reservedByStationId: "station-1",
        leaseToken: "lease-1",
        leaseEpoch: 3,
        lastAttemptId: "attempt-1",
      },
      data: expect.objectContaining({
        status: "raw_committed",
        reservedByStationId: null,
        leaseToken: null,
        leaseExpiresAt: null,
        updatedAt: NOW,
      }),
    });
    expect(tx.executionJob.updateMany).toHaveBeenCalledWith({
      where: {
        id: "job-1",
        workspaceId: "workspace-1",
        status: "in_progress",
        executionPlanVersion: "plan-1",
      },
      data: { status: "raw_committed", updatedAt: NOW },
    });
    expect(tx.taskAttempt.updateMany).toHaveBeenCalledWith({
      where: {
        id: "attempt-1",
        jobId: "job-1",
        stationId: "station-1",
        leaseToken: "lease-1",
        leaseEpoch: 3,
        endedAt: null,
      },
      data: { result: "success", endedAt: NOW },
    });
    expect(tx.executionTaskRuntime.updateMany).toHaveBeenCalledWith({
      where: {
        jobId: "job-1",
        workspaceId: "workspace-1",
        status: "running",
        assignedStationId: "station-1",
        leaseToken: "lease-1",
        leaseEpoch: 3,
        currentAttemptId: "attempt-1",
      },
      data: expect.objectContaining({
        status: "completed",
        progress: 100,
        activeExecutor: null,
        leaseToken: null,
        leaseExpiresAt: null,
        executionPhase: "completed",
        completedAt: NOW,
        updatedAt: NOW,
      }),
    });
  });

  it.each([
    ["blocked", "failed", "failed", "failed", "failed"],
    ["error", "failed", "failed", "failed", "failed"],
    ["cancelled", "cancelled", "cancelled", "stopped", "stopped"],
  ] as const)(
    "keeps %s Evidence but advances control to a non-success terminal state",
    async (terminalState, queueStatus, jobStatus, attemptResult, runtimeStatus) => {
      const { db, tx } = advancingDb();

      await expect(
        finalizeExecutionEvidenceControl(
          { submission: executionSubmission(terminalState), evidenceResult: committed },
          db as never,
          () => NOW,
        ),
      ).resolves.toEqual({ status: "advanced", terminalState });

      expect(tx.executionQueueEntry.updateMany).toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({ status: queueStatus }),
      }));
      expect(tx.executionJob.updateMany).toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({ status: jobStatus, completedAt: NOW }),
      }));
      expect(tx.taskAttempt.updateMany).toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({ result: attemptResult, endedAt: NOW }),
      }));
      expect(tx.executionTaskRuntime.updateMany).toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({
          status: runtimeStatus,
          progress: 100,
          executionPhase: null,
          completedAt: NOW,
        }),
      }));
      expect(tx.taskAttempt.updateMany).not.toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({ result: "success" }),
      }));
      expect(tx.executionTaskRuntime.updateMany).not.toHaveBeenCalledWith(expect.objectContaining({
        data: expect.objectContaining({ status: "completed" }),
      }));
    },
  );

  it("does not enter the control transaction for conflict or rejection", async () => {
    const { db } = advancingDb();
    const conflict: EvidenceIngressResult = {
      status: "conflict",
      rawSnapshotId: "snapshot-conflict",
      receiptId: "receipt-conflict",
      integrityStatus: "capture_identity_conflict",
      checksumValue: "b".repeat(64),
    };
    const rejected: EvidenceIngressResult = {
      status: "rejected",
      reason: "contract_shape_mismatch",
      retryable: false,
    };

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: conflict },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({ status: "not_applicable" });
    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: rejected },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({ status: "not_applicable" });
    expect(db.$transaction).not.toHaveBeenCalled();
  });

  it("treats an already advanced verified replay as idempotent success", async () => {
    const { db, tx } = advancingDb();
    tx.executionQueueEntry.updateMany.mockResolvedValueOnce({ count: 0 });
    tx.executionQueueEntry.findUnique.mockResolvedValueOnce({
      status: "raw_committed",
      workspaceId: "workspace-1",
      reservedByStationId: null,
      leaseToken: null,
      leaseEpoch: 3,
      lastAttemptId: "attempt-1",
    });
    tx.executionJob.findUnique.mockResolvedValueOnce({
      status: "raw_committed",
      workspaceId: "workspace-1",
      executionPlanVersion: "plan-1",
    });
    tx.taskAttempt.findUnique.mockResolvedValueOnce({
      jobId: "job-1",
      stationId: "station-1",
      leaseEpoch: 3,
      result: "success",
      endedAt: NOW,
    });
    tx.executionTaskRuntime.findUnique.mockResolvedValueOnce({
      workspaceId: "workspace-1",
      status: "completed",
      progress: 100,
      assignedStationId: "station-1",
      leaseToken: null,
      leaseEpoch: 3,
      currentAttemptId: "attempt-1",
      executionPhase: "completed",
      completedAt: NOW,
    });
    const replay: EvidenceIngressResult = {
      ...committed,
      status: "replay",
    };

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: replay },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({ status: "already_advanced", terminalState: "completed" });
    expect(tx.executionJob.updateMany).not.toHaveBeenCalled();
    expect(tx.taskAttempt.updateMany).not.toHaveBeenCalled();
    expect(tx.executionTaskRuntime.updateMany).not.toHaveBeenCalled();
  });

  it("returns retryable control_pending without deleting Evidence when CAS misses", async () => {
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { db, tx } = advancingDb();
    tx.executionQueueEntry.updateMany.mockResolvedValueOnce({ count: 0 });
    tx.executionQueueEntry.findUnique.mockResolvedValueOnce({
      status: "in_progress",
      workspaceId: "workspace-1",
      reservedByStationId: "station-other",
      leaseToken: "lease-other",
      leaseEpoch: 4,
      lastAttemptId: "attempt-other",
    });
    tx.executionJob.findUnique.mockResolvedValueOnce(null);
    tx.taskAttempt.findUnique.mockResolvedValueOnce(null);
    tx.executionTaskRuntime.findUnique.mockResolvedValueOnce(null);

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({
      status: "evidence_committed_control_pending",
      rawSnapshotId: "snapshot-1",
      receiptId: "receipt-1",
      checksumValue: "a".repeat(64),
      retryable: true,
    });
    expect(db.rawSnapshot.deleteMany).not.toHaveBeenCalled();
    expect(db.capturePackage.deleteMany).not.toHaveBeenCalled();
    expect(db.evidenceIngressReceipt.deleteMany).not.toHaveBeenCalled();
    expect(errorLog).toHaveBeenCalledWith(
      "[EvidenceIngress] execution control remains pending",
      expect.objectContaining({ category: "cas_miss", jobId: "job-1" }),
    );
    errorLog.mockRestore();
  });

  it("returns control_pending when the independent control transaction fails", async () => {
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { db } = advancingDb();
    db.$transaction.mockRejectedValueOnce(
      new Prisma.PrismaClientUnknownRequestError("database unavailable", {
        clientVersion: "test",
      }),
    );

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual(expect.objectContaining({
      status: "evidence_committed_control_pending",
      retryable: true,
    }));
    expect(errorLog).toHaveBeenCalledWith(
      "[EvidenceIngress] execution control remains pending",
      expect.objectContaining({ category: "database_error" }),
    );
    errorLog.mockRestore();
  });

  it("classifies Prisma driver-adapter failures as explicit control DB failures", async () => {
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { db } = advancingDb();
    const driverError = Object.assign(new Error("postgres trigger failure"), {
      name: "DriverAdapterError",
    });
    db.$transaction.mockRejectedValueOnce(driverError);

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toMatchObject({
      status: "evidence_committed_control_pending",
      retryable: true,
    });
    expect(errorLog).toHaveBeenCalledWith(
      "[EvidenceIngress] execution control remains pending",
      expect.objectContaining({ category: "database_error", errorCode: "DriverAdapterError" }),
    );
    errorLog.mockRestore();
  });

  it("propagates post-commit clock and unknown programming failures", async () => {
    const { db } = advancingDb();

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => new Date(Number.NaN),
      ),
    ).rejects.toThrow(/server clock/);
    expect(db.$transaction).not.toHaveBeenCalled();

    const programmingError = new Error("programming invariant failed");
    db.$transaction.mockRejectedValueOnce(programmingError);
    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).rejects.toBe(programmingError);
  });

  it("retries only listed serialization failures up to three attempts", async () => {
    const { db } = advancingDb();
    db.$transaction
      .mockRejectedValueOnce({ code: "40001" })
      .mockRejectedValueOnce({ meta: { code: "40P01" } });

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toEqual({ status: "advanced", terminalState: "completed" });
    expect(db.$transaction).toHaveBeenCalledTimes(3);
  });

  it.each([
    ["SQLSTATE code", { code: "40001" }],
    ["Prisma meta code", { meta: { code: "40P01" } }],
    ["nested cause", { cause: { code: "40001" } }],
  ])("returns retryable pending after three listed %s failures", async (_label, failure) => {
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const { db } = advancingDb();
    db.$transaction.mockRejectedValue(failure);

    await expect(
      finalizeExecutionEvidenceControl(
        { submission: executionSubmission(), evidenceResult: committed },
        db as never,
        () => NOW,
      ),
    ).resolves.toMatchObject({
      status: "evidence_committed_control_pending",
      retryable: true,
    });
    expect(db.$transaction).toHaveBeenCalledTimes(3);
    expect(errorLog).toHaveBeenCalledWith(
      "[EvidenceIngress] execution control remains pending",
      expect.objectContaining({
        category: "serialization_exhausted",
        errorCode: expect.stringMatching(/^(40001|40P01)$/),
      }),
    );
    errorLog.mockRestore();
  });
});
