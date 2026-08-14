/**
 * Release-B dark execution control finalizer (DEC-B1-020 / 07 §8).
 *
 * EvidenceIngress owns Evidence only. After a verified commit/replay, this
 * independent SERIALIZABLE transaction releases the exact current lease and
 * advances the four V1 execution-control rows. It never updates or deletes
 * Evidence and has no route caller before the one-time cutover.
 */

import type { PrismaClient } from "@/lib/prisma-client";
import { Prisma } from "@/lib/prisma-client";

import type {
  CaptureSubmissionV2,
  CaptureTerminalStateV2,
  EvidenceIngressResult,
} from "./types";

type ExecutionControlDatabase = Pick<PrismaClient, "$transaction">;

export type EvidenceCommittedControlPending = {
  status: "evidence_committed_control_pending";
  rawSnapshotId: string;
  receiptId: string;
  checksumValue: string;
  retryable: true;
};

export type ExecutionControlFinalizationResult =
  | { status: "advanced"; terminalState: CaptureTerminalStateV2 }
  | { status: "already_advanced"; terminalState: CaptureTerminalStateV2 }
  | { status: "not_applicable" }
  | EvidenceCommittedControlPending;

type ExecutionControlIdentity = {
  workspaceId: string;
  jobId: string;
  attemptId: string;
  stationId: string;
  leaseToken: string;
  leaseEpoch: number;
  executionPlanVersion: string;
};

class ExecutionControlAdvanceConflict extends Error {}

const MAX_CONTROL_ATTEMPTS = 3;

export async function finalizeExecutionEvidenceControl(
  input: {
    submission: CaptureSubmissionV2;
    evidenceResult: EvidenceIngressResult;
  },
  db: ExecutionControlDatabase,
  clock: () => Date = () => new Date(),
): Promise<ExecutionControlFinalizationResult> {
  if (!shouldAdvanceControl(input.evidenceResult)) {
    return { status: "not_applicable" };
  }

  const evidenceResult = input.evidenceResult;
  const identity = executionControlIdentity(input.submission);
  const outcome = terminalControlOutcome(input.submission.body.header.report.terminal);
  const now = validServerTime(clock());

  for (let attempt = 1; attempt <= MAX_CONTROL_ATTEMPTS; attempt += 1) {
    try {
      const result = await db.$transaction(
        async (tx) => {
        const queueUpdate = await tx.executionQueueEntry.updateMany({
          where: {
            jobId: identity.jobId,
            workspaceId: identity.workspaceId,
            status: "in_progress",
            reservedByStationId: identity.stationId,
            leaseToken: identity.leaseToken,
            leaseEpoch: identity.leaseEpoch,
            lastAttemptId: identity.attemptId,
          },
          data: {
            status: outcome.queueStatus,
            reservedByStationId: null,
            reservedPlatformAccountId: null,
            reservedExecutionIdentityKey: null,
            reserveToken: null,
            reservedUntil: null,
            startBefore: null,
            leaseToken: null,
            leaseExpiresAt: null,
            ...(outcome.failureCode ? {
              lastErrorCode: outcome.failureCode,
              lastErrorMessage: outcome.errorMessage,
            } : {}),
            updatedAt: now,
          },
        });

        if (queueUpdate.count === 0) {
          return (await isAlreadyAdvanced(tx, identity, outcome))
            ? { status: "already_advanced" as const, terminalState: outcome.terminalState }
            : controlPending(evidenceResult);
        }

        const jobUpdate = await tx.executionJob.updateMany({
          where: {
            id: identity.jobId,
            workspaceId: identity.workspaceId,
            status: "in_progress",
            executionPlanVersion: identity.executionPlanVersion,
          },
          data: {
            status: outcome.jobStatus,
            ...(outcome.terminalState === "completed" ? {} : { completedAt: now }),
            updatedAt: now,
          },
        });
        requireSingleUpdate(jobUpdate.count, "ExecutionJob");

        const attemptUpdate = await tx.taskAttempt.updateMany({
          where: {
            id: identity.attemptId,
            jobId: identity.jobId,
            stationId: identity.stationId,
            leaseToken: identity.leaseToken,
            leaseEpoch: identity.leaseEpoch,
            endedAt: null,
          },
          data: {
            result: outcome.attemptResult,
            endedAt: now,
            ...(outcome.failureCode ? {
              failureCode: outcome.failureCode,
              errorMessage: outcome.errorMessage,
            } : {}),
          },
        });
        requireSingleUpdate(attemptUpdate.count, "TaskAttempt");

        const runtimeUpdate = await tx.executionTaskRuntime.updateMany({
          where: {
            jobId: identity.jobId,
            workspaceId: identity.workspaceId,
            status: "running",
            assignedStationId: identity.stationId,
            leaseToken: identity.leaseToken,
            leaseEpoch: identity.leaseEpoch,
            currentAttemptId: identity.attemptId,
          },
          data: {
            status: outcome.runtimeStatus,
            progress: 100,
            activeExecutor: null,
            leaseToken: null,
            leaseExpiresAt: null,
            executionPhase: outcome.executionPhase,
            ...(outcome.errorMessage ? { errorMessage: outcome.errorMessage } : {}),
            completedAt: now,
            updatedAt: now,
          },
        });
        requireSingleUpdate(runtimeUpdate.count, "ExecutionTaskRuntime");

        return { status: "advanced" as const, terminalState: outcome.terminalState };
        },
        {
          isolationLevel: Prisma.TransactionIsolationLevel.Serializable,
          timeout: 30_000,
        },
      );
      if (result.status === "evidence_committed_control_pending") {
        recordControlFailure(identity, "cas_miss");
      }
      return result;
    } catch (error) {
      if (isRetryableControlConflict(error)) {
        if (attempt < MAX_CONTROL_ATTEMPTS) continue;
        recordControlFailure(identity, "serialization_exhausted", error);
        return controlPending(evidenceResult);
      }
      if (error instanceof ExecutionControlAdvanceConflict) {
        recordControlFailure(identity, "cas_conflict");
        return controlPending(evidenceResult);
      }
      if (isControlDatabaseError(error)) {
        recordControlFailure(identity, "database_error", error);
        return controlPending(evidenceResult);
      }
      throw error;
    }
  }

  throw new Error("Execution control finalizer exhausted an unreachable retry loop.");
}

function shouldAdvanceControl(
  result: EvidenceIngressResult,
): result is Extract<EvidenceIngressResult, { status: "committed" | "replay" }> {
  return result.status === "committed" ||
    (result.status === "replay" && result.integrityStatus === "verified");
}

function executionControlIdentity(
  submission: CaptureSubmissionV2,
): ExecutionControlIdentity {
  const { header } = submission.body;
  const { authority } = submission;
  if (header.ingressKind !== "execution" || authority.ingressKind !== "execution") {
    throw new Error("Execution control finalization requires an execution submission.");
  }
  return {
    workspaceId: authority.workspaceId,
    jobId: header.jobId,
    attemptId: header.attemptId,
    stationId: authority.stationId,
    leaseToken: authority.leaseToken,
    leaseEpoch: header.leaseEpoch,
    executionPlanVersion: header.executionPlanVersion,
  };
}

async function isAlreadyAdvanced(
  tx: Prisma.TransactionClient,
  identity: ExecutionControlIdentity,
  outcome: TerminalControlOutcome,
): Promise<boolean> {
  const [queue, job, attempt, runtime] = await Promise.all([
    tx.executionQueueEntry.findUnique({
      where: { jobId: identity.jobId },
      select: {
        status: true,
        workspaceId: true,
        reservedByStationId: true,
        leaseToken: true,
        leaseEpoch: true,
        lastAttemptId: true,
      },
    }),
    tx.executionJob.findUnique({
      where: { id: identity.jobId },
      select: {
        status: true,
        workspaceId: true,
        executionPlanVersion: true,
      },
    }),
    tx.taskAttempt.findUnique({
      where: { id: identity.attemptId },
      select: {
        jobId: true,
        stationId: true,
        leaseEpoch: true,
        result: true,
        endedAt: true,
      },
    }),
    tx.executionTaskRuntime.findUnique({
      where: { jobId: identity.jobId },
      select: {
        workspaceId: true,
        status: true,
        progress: true,
        assignedStationId: true,
        leaseToken: true,
        leaseEpoch: true,
        currentAttemptId: true,
        executionPhase: true,
        completedAt: true,
      },
    }),
  ]);

  return Boolean(
    queue &&
      queue.status === outcome.queueStatus &&
      queue.workspaceId === identity.workspaceId &&
      queue.reservedByStationId === null &&
      queue.leaseToken === null &&
      queue.leaseEpoch === identity.leaseEpoch &&
      queue.lastAttemptId === identity.attemptId &&
      job &&
      job.status === outcome.jobStatus &&
      job.workspaceId === identity.workspaceId &&
      job.executionPlanVersion === identity.executionPlanVersion &&
      attempt &&
      attempt.jobId === identity.jobId &&
      attempt.stationId === identity.stationId &&
      attempt.leaseEpoch === identity.leaseEpoch &&
      attempt.result === outcome.attemptResult &&
      attempt.endedAt !== null &&
      runtime &&
      runtime.workspaceId === identity.workspaceId &&
      runtime.status === outcome.runtimeStatus &&
      runtime.progress === 100 &&
      runtime.assignedStationId === identity.stationId &&
      runtime.leaseToken === null &&
      runtime.leaseEpoch === identity.leaseEpoch &&
      runtime.currentAttemptId === identity.attemptId &&
      runtime.executionPhase === outcome.executionPhase &&
      runtime.completedAt !== null,
  );
}

type TerminalControlOutcome = {
  terminalState: CaptureTerminalStateV2;
  queueStatus: "raw_committed" | "failed" | "cancelled";
  jobStatus: "raw_committed" | "failed" | "cancelled";
  attemptResult: "success" | "failed" | "stopped";
  runtimeStatus: "completed" | "failed" | "stopped";
  executionPhase: "completed" | null;
  failureCode: string | null;
  errorMessage: string | null;
};

function terminalControlOutcome(
  terminal: CaptureSubmissionV2["body"]["header"]["report"]["terminal"],
): TerminalControlOutcome {
  if (terminal.state === "completed") {
    return {
      terminalState: "completed",
      queueStatus: "raw_committed",
      jobStatus: "raw_committed",
      attemptResult: "success",
      runtimeStatus: "completed",
      executionPhase: "completed",
      failureCode: null,
      errorMessage: null,
    };
  }
  const errorMessage = `capture terminal ${terminal.state}: ${terminal.reason}`;
  if (terminal.state === "cancelled") {
    return {
      terminalState: terminal.state,
      queueStatus: "cancelled",
      jobStatus: "cancelled",
      attemptResult: "stopped",
      runtimeStatus: "stopped",
      executionPhase: null,
      failureCode: `capture_${terminal.reason}`,
      errorMessage,
    };
  }
  return {
    terminalState: terminal.state,
    queueStatus: "failed",
    jobStatus: "failed",
    attemptResult: "failed",
    runtimeStatus: "failed",
    executionPhase: null,
    failureCode: `capture_${terminal.reason}`,
    errorMessage,
  };
}

function requireSingleUpdate(count: number, model: string): void {
  if (count !== 1) {
    throw new ExecutionControlAdvanceConflict(
      `${model} no longer matches the verified execution control context.`,
    );
  }
}

function validServerTime(value: Date): Date {
  if (!(value instanceof Date) || !Number.isFinite(value.getTime())) {
    throw new Error("The server clock must return a valid Date.");
  }
  return new Date(value.getTime());
}

function isRetryableControlConflict(error: unknown): boolean {
  if (error instanceof Prisma.PrismaClientKnownRequestError && error.code === "P2034") {
    return true;
  }
  const code = structuredErrorCode(error);
  return code === "40001" || code === "40P01";
}

function structuredErrorCode(error: unknown): string | null {
  if (error === null || typeof error !== "object") return null;
  const candidate = error as { code?: unknown; meta?: unknown; cause?: unknown };
  if (typeof candidate.code === "string" && /^[0-9A-Z]{5}$/.test(candidate.code)) {
    return candidate.code;
  }
  if (candidate.meta !== null && typeof candidate.meta === "object") {
    const metaCode = (candidate.meta as { code?: unknown }).code;
    if (typeof metaCode === "string" && /^[0-9A-Z]{5}$/.test(metaCode)) return metaCode;
  }
  return structuredErrorCode(candidate.cause);
}

function isControlDatabaseError(error: unknown): boolean {
  return error instanceof Prisma.PrismaClientKnownRequestError ||
    error instanceof Prisma.PrismaClientUnknownRequestError ||
    error instanceof Prisma.PrismaClientInitializationError ||
    error instanceof Prisma.PrismaClientRustPanicError ||
    (error instanceof Error && error.name === "DriverAdapterError");
}

function recordControlFailure(
  identity: ExecutionControlIdentity,
  category:
    | "cas_miss"
    | "cas_conflict"
    | "database_error"
    | "serialization_exhausted",
  error?: unknown,
): void {
  const errorCode = structuredErrorCode(error) ??
    (error instanceof Prisma.PrismaClientKnownRequestError
      ? error.code
      : error instanceof Error
        ? error.name
        : null);
  console.error("[EvidenceIngress] execution control remains pending", {
    category,
    errorCode,
    jobId: identity.jobId,
    attemptId: identity.attemptId,
    leaseEpoch: identity.leaseEpoch,
  });
}

function controlPending(
  result: Extract<EvidenceIngressResult, { status: "committed" | "replay" }>,
): EvidenceCommittedControlPending {
  return {
    status: "evidence_committed_control_pending",
    rawSnapshotId: result.rawSnapshotId,
    receiptId: result.receiptId,
    checksumValue: result.checksumValue,
    retryable: true,
  };
}
