/**
 * V2 Durable Worker — lease-based B2→B3 processing with crash recovery.
 */
import { Prisma, type PrismaClient } from "@/lib/prisma-client";
import type { B2DerivedService, B2DerivedResult } from "../derived/b2-derived-service";
import type { VerifiedArtifactReader } from "../media/verified-artifact-reader";
import type { ContentProjectionService, ProjectionResult } from "../projection/content-projection-service";
import type { EvidenceTransactionFence } from "../transaction-fence";

const LEASE_DURATION_MS = 60_000;
type WorkFailure = { stage: "b2" | "b3"; reason: string; retryable: boolean };

class V2WorkLeaseLost extends Error {}

const DEAD_ATTEMPTS = 10;

type WorkerDB = Pick<PrismaClient, "v2DurableWork" | "contractEvaluation" | "$transaction">;

export interface V2WorkerDeps {
  db: WorkerDB;
  b2Service: B2DerivedService;
  artifactReader: Pick<VerifiedArtifactReader, "readAndVerify">;
  projectionService: ContentProjectionService;
  workerId: string;
  /** Test seam only; production always uses the fixed one-minute lease. */
  leaseDurationMs?: number;
}

export class V2DurableWorker {
  private readonly workerId: string;
  private readonly leaseDurationMs: number;
  constructor(private readonly deps: V2WorkerDeps) {
    this.workerId = deps.workerId;
    this.leaseDurationMs = deps.leaseDurationMs ?? LEASE_DURATION_MS;
    if (!Number.isInteger(this.leaseDurationMs) || this.leaseDurationMs <= 0) {
      throw new Error("leaseDurationMs must be a positive integer.");
    }
  }

  async tick(batchSize = 5): Promise<{ processed: number; errors: number; recovered: number }> {
    return this.tickSelected(batchSize);
  }

  async tickReceipt(input: {
    workspaceId: string;
    receiptId: string;
  }): Promise<{ processed: number; errors: number; recovered: number }> {
    if (!input.workspaceId.trim() || !input.receiptId.trim()) {
      throw new Error("workspaceId and receiptId are required for a targeted V2 worker tick.");
    }
    return this.tickSelected(1, input);
  }

  private async tickSelected(
    batchSize: number,
    selector?: { workspaceId: string; receiptId: string },
  ): Promise<{ processed: number; errors: number; recovered: number }> {
    const recovered = await this.recoverStaleWork(selector);
    let processed = 0, errors = 0;

    for (let i = 0; i < batchSize; i++) {
      const claimed = await this.claimWork(selector);
      if (!claimed) break;
      try {
        await this.processWork(claimed.id);
        processed++;
      } catch (err) {
        errors++;
        console.error(`[${this.workerId}] work ${claimed.id} failed:`, err);
      }
    }
    return { processed, errors, recovered };
  }

  private async recoverStaleWork(selector?: { workspaceId: string; receiptId: string }): Promise<number> {
    const now = new Date();
    const result = await this.deps.db.v2DurableWork.updateMany({
      where: {
        status: "processing",
        leaseExpiresAt: { lt: now },
        ...(selector ?? {}),
      },
      data: {
        status: "pending",
        attemptCount: { increment: 1 },
        lockedBy: null, lockedAt: null, leaseExpiresAt: null,
        lastError: `recovered: lease expired`,
      },
    });
    return result.count;
  }

  private async claimWork(selector?: { workspaceId: string; receiptId: string }): Promise<{ id: string } | null> {
    const now = new Date();
    const row = await this.deps.db.v2DurableWork.findFirst({
      where: {
        status: "pending",
        nextAttemptAt: { lte: now },
        ...(selector ?? {}),
      },
      orderBy: { nextAttemptAt: "asc" },
      select: { id: true },
    });
    if (!row) return null;

    const result = await this.deps.db.v2DurableWork.updateMany({
      where: { id: row.id, status: "pending", ...(selector ?? {}) },
      data: {
        status: "processing", lockedBy: this.workerId,
        lockedAt: now, leaseExpiresAt: new Date(now.getTime() + this.leaseDurationMs),
      },
    });
    if (result.count !== 1) return null; // claimed by another worker
    return { id: row.id };
  }

  private async processWork(workId: string): Promise<void> {
    const work = await this.deps.db.v2DurableWork.findUniqueOrThrow({
      where: { id: workId },
      select: { workspaceId: true, rawSnapshotId: true, attemptCount: true, lockedBy: true, leaseExpiresAt: true },
    });
    this.assertOwned(workId, work.lockedBy, work.leaseExpiresAt);

    const fence = this.transactionFence(workId);
    const b2Result: B2DerivedResult = await this.deps.b2Service.derive({
      workspaceId: work.workspaceId,
      rawSnapshotId: work.rawSnapshotId,
      retryMode: work.attemptCount > 0 ? "explicit_retry" : "replay",
    }, fence);

    if (b2Result.status === "rejected") {
      await this.handleFailure(workId, { stage: "b2", reason: b2Result.reason ?? b2Result.status, retryable: (b2Result as { retryable?: boolean }).retryable !== false });
      return;
    }

    await this.ownedUpdate(workId, {
      b2Result: JSON.parse(JSON.stringify(b2Result)) as Prisma.JsonObject,
    });

    if ((b2Result.status === "committed" || b2Result.status === "replay") && b2Result.decision === "accepted") {
      let mediaResult: Awaited<ReturnType<VerifiedArtifactReader["readAndVerify"]>>;
      try {
        mediaResult = await this.deps.artifactReader.readAndVerify({
          workspaceId: work.workspaceId,
          rawSnapshotId: work.rawSnapshotId,
        });
      } catch (error) {
        await this.handleFailure(workId, {
          stage: "b3",
          reason: `verified_media_read_failed:${error instanceof Error ? error.message : String(error)}`,
          retryable: true,
        });
        return;
      }
      if (!mediaResult.ok) {
        await this.handleFailure(workId, {
          stage: "b3",
          reason: `${mediaResult.reason}:${mediaResult.detail}`,
          retryable: false,
        });
        return;
      }
      const evaluation = await this.deps.db.contractEvaluation.findUniqueOrThrow({
        where: { id: b2Result.contractEvaluationId },
        select: { contractId: true, contractVersion: true },
      });
      await this.renewOwnedLease(workId);
      const b3Result: ProjectionResult = await this.deps.projectionService.project({
        workspaceId: work.workspaceId, rawSnapshotId: work.rawSnapshotId,
        contractId: evaluation.contractId, contractVersion: evaluation.contractVersion,
        evaluationId: b2Result.contractEvaluationId,
        verifiedMedia: mediaResult.verified,
      }, fence);
      if (b3Result.ok) {
        await this.ownedUpdate(workId, {
          status: "b3_completed",
          b3Result: b3Result as unknown as Prisma.JsonObject,
          lockedBy: null,
          lockedAt: null,
          leaseExpiresAt: null,
        });
      } else {
        await this.handleFailure(workId, {
          stage: "b3",
          reason: b3Result.reason,
          retryable: b3Result.reason !== "canonical_media_kind_source_incomplete",
        });
      }
    } else {
      await this.ownedUpdate(workId, {
        status: "b3_completed",
        lockedBy: null,
        lockedAt: null,
        leaseExpiresAt: null,
      });
    }
  }

  private transactionFence(workId: string): EvidenceTransactionFence {
    return async (tx) => {
      const rows = await tx.$queryRaw<Array<{
        lockedBy: string | null;
        leaseValid: boolean;
      }>>`
        SELECT "lockedBy", "leaseExpiresAt" > clock_timestamp() AS "leaseValid"
        FROM "V2DurableWork"
        WHERE id = ${workId} AND status = 'processing'
        FOR UPDATE
      `;
      const row = rows[0];
      if (!row || row.lockedBy !== this.workerId || row.leaseValid !== true) {
        throw new V2WorkLeaseLost(`Lease transaction fence rejected work ${workId}`);
      }
    };
  }

  private assertOwned(workId: string, lockedBy: string | null, leaseExpiresAt: Date | null): void {
    if (lockedBy !== this.workerId || !leaseExpiresAt || leaseExpiresAt <= new Date()) {
      throw new V2WorkLeaseLost(`Lease lost for work ${workId}`);
    }
  }

  private async ownedUpdate(workId: string, data: Prisma.V2DurableWorkUpdateManyMutationInput): Promise<void> {
    const result = await this.deps.db.v2DurableWork.updateMany({
      where: {
        id: workId,
        status: "processing",
        lockedBy: this.workerId,
        leaseExpiresAt: { gt: new Date() },
      },
      data,
    });
    if (result.count !== 1) throw new V2WorkLeaseLost(`Lease lost for work ${workId}`);
  }

  private async renewOwnedLease(workId: string): Promise<void> {
    await this.ownedUpdate(workId, {
      leaseExpiresAt: new Date(Date.now() + this.leaseDurationMs),
    });
  }

  private async handleFailure(workId: string, failure: WorkFailure): Promise<void> {
    const work = await this.deps.db.v2DurableWork.findUniqueOrThrow({ where: { id: workId }, select: { attemptCount: true } });
    const next = work.attemptCount + 1;
    const isDead = !failure.retryable || next >= DEAD_ATTEMPTS;

    if (isDead) {
      await this.ownedUpdate(workId, {
        status: "dead", attemptCount: next,
        lastError: `${failure.stage}: ${failure.reason}`,
        deadReason: failure.retryable ? "max_attempts" : "non_retryable",
        lockedBy: null, lockedAt: null, leaseExpiresAt: null,
      });
    } else {
      const backoff = Math.min(1000 * Math.pow(2, next - 1), 300_000);
      await this.ownedUpdate(workId, {
        status: "pending", attemptCount: next,
        nextAttemptAt: new Date(Date.now() + backoff),
        lockedBy: null, lockedAt: null, leaseExpiresAt: null,
      });
    }
  }
}
