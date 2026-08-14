// @vitest-environment node

import { describe, expect, it, vi } from "vitest";

import { V2DurableWorker } from "./v2-durable-worker";

const WORKSPACE_ID = "ws-worker-media";
const SNAPSHOT_ID = "snapshot-worker-media";
const EVALUATION_ID = "evaluation-worker-media";

describe("V2DurableWorker audited media boundary", () => {
  it("passes the exact audited capability to Projection before reporting complete", async () => {
    const verified = Object.freeze({ opaque: "test-capability" });
    const artifactReader = { readAndVerify: vi.fn(async () => ({ ok: true as const, verified })) };
    const projectionService = {
      project: vi.fn(async (input: Record<string, unknown>) => {
        expect(input.verifiedMedia).toBe(verified);
        return { ok: true as const, observationId: "observation-1", version: 1, replayed: false };
      }),
    };
    const db = workerDb("worker-media-proof");
    const worker = new V2DurableWorker({
      db: db.value,
      b2Service: acceptedB2(),
      artifactReader: artifactReader as never,
      projectionService: projectionService as never,
      workerId: "worker-media-proof",
    });

    await expect(worker.tick(1)).resolves.toEqual({ processed: 1, errors: 0, recovered: 0 });
    expect(artifactReader.readAndVerify).toHaveBeenCalledWith({
      workspaceId: WORKSPACE_ID,
      rawSnapshotId: SNAPSHOT_ID,
    });
    expect(projectionService.project).toHaveBeenCalledOnce();
    expect(db.updates).toContainEqual(expect.objectContaining({ status: "b3_completed" }));
  });

  it("makes invalid audited Artifact evidence non-retryable and never calls Projection", async () => {
    const artifactReader = {
      readAndVerify: vi.fn(async () => ({
        ok: false as const,
        reason: "media_inventory_shape_invalid",
        detail: "artifact.candidates[0]",
      })),
    };
    const projectionService = { project: vi.fn() };
    const db = workerDb("worker-invalid-media-proof");
    const worker = new V2DurableWorker({
      db: db.value,
      b2Service: acceptedB2(),
      artifactReader: artifactReader as never,
      projectionService: projectionService as never,
      workerId: "worker-invalid-media-proof",
    });

    await expect(worker.tick(1)).resolves.toEqual({ processed: 1, errors: 0, recovered: 0 });
    expect(projectionService.project).not.toHaveBeenCalled();
    expect(db.updates).toContainEqual(expect.objectContaining({
      status: "dead",
      deadReason: "non_retryable",
      lastError: "b3: media_inventory_shape_invalid:artifact.candidates[0]",
    }));
  });

  it("does not retry an unsupported live_photo media fact as if it could become valid", async () => {
    const artifactReader = {
      readAndVerify: vi.fn(async () => ({ ok: true as const, verified: Object.freeze({}) })),
    };
    const projectionService = {
      project: vi.fn(async () => ({
        ok: false as const,
        reason: "canonical_media_kind_source_incomplete",
      })),
    };
    const db = workerDb("worker-live-photo-proof");
    const worker = new V2DurableWorker({
      db: db.value,
      b2Service: acceptedB2(),
      artifactReader: artifactReader as never,
      projectionService: projectionService as never,
      workerId: "worker-live-photo-proof",
    });

    await expect(worker.tick(1)).resolves.toEqual({ processed: 1, errors: 0, recovered: 0 });
    expect(db.updates).toContainEqual(expect.objectContaining({
      status: "dead",
      deadReason: "non_retryable",
      lastError: "b3: canonical_media_kind_source_incomplete",
    }));
  });
});

function acceptedB2() {
  return {
    derive: vi.fn(async () => ({
      status: "committed" as const,
      decision: "accepted" as const,
      completeness: "full" as const,
      revision: 1,
      contractEvaluationId: EVALUATION_ID,
    })),
  } as never;
}

function workerDb(workerId: string) {
  const updates: Array<Record<string, unknown>> = [];
  let findWorkCalls = 0;
  const value = {
    v2DurableWork: {
      updateMany: vi.fn(async ({ data }: { data: Record<string, unknown> }) => {
        if (data && Object.keys(data).length > 0) updates.push(data);
        return { count: data.status === "pending" && data.attemptCount ? 0 : 1 };
      }),
      findFirst: vi.fn(async () => ({ id: "work-media-1" })),
      findUniqueOrThrow: vi.fn(async () => {
        findWorkCalls++;
        return findWorkCalls === 1
          ? { workspaceId: WORKSPACE_ID, rawSnapshotId: SNAPSHOT_ID, attemptCount: 0, lockedBy: workerId, leaseExpiresAt: new Date(Date.now() + 60_000) }
          : { attemptCount: 0 };
      }),
    },
    contractEvaluation: {
      findUniqueOrThrow: vi.fn(async () => ({ contractId: "xhs.note-detail", contractVersion: 1 })),
    },
    $transaction: vi.fn(),
  };
  return { value: value as never, updates };
}
