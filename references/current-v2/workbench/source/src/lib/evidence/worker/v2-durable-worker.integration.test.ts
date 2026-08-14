import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { createIndependentPrismaClient } from "@/lib/db";
import { Prisma, PrismaClient } from "@/lib/prisma-client";
import { V2DurableWorker } from "./v2-durable-worker";

const enabled = process.env.V2_WORKER_INTEGRATION_DB === "1";
const describeIntegration = enabled ? describe : describe.skip;

describeIntegration("V2DurableWorker real database fencing", () => {
  const adminDsn = requiredEnv("V2_WORKER_INTEGRATION_ADMIN_DSN");
  const canonicalDsn = requiredEnv("V2_CANONICAL_WRITER_DSN");
  const admin = client(adminDsn);
  const workerA = client(canonicalDsn);
  const workerB = client(canonicalDsn);
  const prefix = `worker-proof-${Date.now()}`;

  beforeAll(async () => {
    const rows = await admin.$queryRawUnsafe<Array<{ name: string }>>("SELECT current_database() AS name");
    if (!rows[0]?.name.startsWith("content_workbench_v2_rc_r2_")) {
      throw new Error(`Refusing worker proof database ${rows[0]?.name ?? "unknown"}`);
    }
  });

  afterAll(async () => {
    await Promise.all([admin.$disconnect(), workerA.$disconnect(), workerB.$disconnect()]);
  });

  it("allows exactly one of two independent clients to enter B2", async () => {
    const work = await seedWork(admin, `${prefix}-race`);
    const derive = vi.fn(async () => ({
      status: "rejected" as const,
      reason: "proof_non_retryable",
      retryable: false,
    }));
    const projection = { project: vi.fn() };
    const workers = [
      new V2DurableWorker({ db: workerA, b2Service: { derive } as never, artifactReader: artifactReader(), projectionService: projection as never, workerId: `${prefix}-a` }),
      new V2DurableWorker({ db: workerB, b2Service: { derive } as never, artifactReader: artifactReader(), projectionService: projection as never, workerId: `${prefix}-b` }),
    ];

    await Promise.all(workers.map((worker) => worker.tickReceipt({
      workspaceId: "ws-r2",
      receiptId: `${work.id}-receipt`,
    })));

    expect(derive).toHaveBeenCalledTimes(1);
    expect(projection.project).not.toHaveBeenCalled();
    await expect(admin.v2DurableWork.findUniqueOrThrow({ where: { id: work.id } })).resolves.toMatchObject({
      status: "dead",
      attemptCount: 1,
      lockedBy: null,
    });
  });

  it("prevents an expired old worker from writing after another owner takes the lease", async () => {
    const work = await seedWork(admin, `${prefix}-stale`);
    let release!: () => void;
    const gate = new Promise<void>((resolve) => { release = resolve; });
    const entered = vi.fn();
    const derive = vi.fn(async () => {
      entered();
      await gate;
      return { status: "rejected" as const, reason: "late_old_worker", retryable: false };
    });
    const oldWorker = new V2DurableWorker({
      db: workerA,
      b2Service: { derive } as never,
      artifactReader: artifactReader(),
      projectionService: { project: vi.fn() } as never,
      workerId: `${prefix}-old`,
    });

    const tick = oldWorker.tickReceipt({ workspaceId: "ws-r2", receiptId: `${work.id}-receipt` });
    await vi.waitFor(() => expect(entered).toHaveBeenCalledOnce());
    await admin.v2DurableWork.update({
      where: { id: work.id },
      data: { lockedBy: `${prefix}-new`, leaseExpiresAt: new Date(Date.now() + 60_000) },
    });
    release();
    await expect(tick).resolves.toMatchObject({ processed: 0, errors: 1 });
    await expect(admin.v2DurableWork.findUniqueOrThrow({ where: { id: work.id } })).resolves.toMatchObject({
      status: "processing",
      lockedBy: `${prefix}-new`,
      lastError: null,
    });
  });

  it("claims only the explicitly bound canary receipt and leaves unrelated pending work untouched", async () => {
    const target = await seedWork(admin, `${prefix}-target`);
    const unrelated = await seedWork(admin, `${prefix}-unrelated`);
    const derive = vi.fn(async () => ({
      status: "rejected" as const,
      reason: "proof_non_retryable",
      retryable: false,
    }));
    const worker = new V2DurableWorker({
      db: workerA,
      b2Service: { derive } as never,
      artifactReader: artifactReader(),
      projectionService: { project: vi.fn() } as never,
      workerId: `${prefix}-canary`,
    });

    await worker.tickReceipt({ workspaceId: "ws-r2", receiptId: `${target.id}-receipt` });

    expect(derive).toHaveBeenCalledOnce();
    expect(derive).toHaveBeenCalledWith(expect.objectContaining({
      rawSnapshotId: `${target.id}-snapshot`,
    }), expect.any(Function));
    await expect(admin.v2DurableWork.findUniqueOrThrow({ where: { id: target.id } }))
      .resolves.toMatchObject({ status: "dead" });
    await expect(admin.v2DurableWork.findUniqueOrThrow({ where: { id: unrelated.id } }))
      .resolves.toMatchObject({ status: "pending", lockedBy: null, attemptCount: 0 });
  });

  it("rolls back B2 domain writes when the DB lease expires before transaction commit", async () => {
    const work = await seedWork(admin, `${prefix}-b2-fence`);
    const domainId = `${prefix}-b2-domain`;
    let entered!: () => void;
    const began = new Promise<void>((resolve) => { entered = resolve; });
    const b2Service = {
      derive: async (_input: unknown, fence: (tx: Prisma.TransactionClient) => Promise<void>) => workerA.$transaction(async (tx) => {
        await fence(tx);
        await tx.contentAsset.create({ data: domainAsset(domainId) });
        entered();
        await delay(180);
        await fence(tx);
        return { status: "rejected" as const, reason: "evidence_ineligible" as const, retryable: false };
      }),
    };
    const worker = new V2DurableWorker({
      db: workerA,
      b2Service: b2Service as never,
      artifactReader: artifactReader(),
      projectionService: { project: vi.fn() } as never,
      workerId: `${prefix}-b2-old`,
      leaseDurationMs: 80,
    });

    const tick = worker.tickReceipt({ workspaceId: "ws-r2", receiptId: `${work.id}-receipt` });
    await began;
    const takeover = workerB.v2DurableWork.updateMany({
      where: { id: work.id, status: "processing" },
      data: { lockedBy: `${prefix}-b2-new`, leaseExpiresAt: new Date(Date.now() + 60_000) },
    });
    await expect(tick).resolves.toMatchObject({ processed: 0, errors: 1 });
    await expect(takeover).resolves.toMatchObject({ count: 1 });
    expect(await admin.contentAsset.count({ where: { id: domainId } })).toBe(0);
  });

  it("rolls back B3 domain writes when the DB lease expires before transaction commit", async () => {
    const work = await seedWork(admin, `${prefix}-b3-fence`);
    const evaluationId = await seedAcceptedEvaluation(admin, work.id);
    const domainId = `${prefix}-b3-domain`;
    let entered!: () => void;
    const began = new Promise<void>((resolve) => { entered = resolve; });
    const projectionService = {
      project: async (_input: unknown, fence: (tx: Prisma.TransactionClient) => Promise<void>) => workerA.$transaction(async (tx) => {
        await fence(tx);
        await tx.contentAsset.create({ data: domainAsset(domainId) });
        entered();
        await delay(180);
        await fence(tx);
        return { ok: true as const, observationId: "never-committed", version: 1, replayed: false };
      }),
    };
    const worker = new V2DurableWorker({
      db: workerA,
      b2Service: { derive: vi.fn(async () => ({
        status: "committed", decision: "accepted", completeness: "full", revision: 1,
        contractEvaluationId: evaluationId,
      })) } as never,
      artifactReader: artifactReader(),
      projectionService: projectionService as never,
      workerId: `${prefix}-b3-old`,
      leaseDurationMs: 80,
    });

    const tick = worker.tickReceipt({ workspaceId: "ws-r2", receiptId: `${work.id}-receipt` });
    await began;
    const takeover = workerB.v2DurableWork.updateMany({
      where: { id: work.id, status: "processing" },
      data: { lockedBy: `${prefix}-b3-new`, leaseExpiresAt: new Date(Date.now() + 60_000) },
    });
    await expect(tick).resolves.toMatchObject({ processed: 0, errors: 1 });
    await expect(takeover).resolves.toMatchObject({ count: 1 });
    expect(await admin.contentAsset.count({ where: { id: domainId } })).toBe(0);
  });
});

function artifactReader() {
  return {
    readAndVerify: vi.fn(async () => ({ ok: true as const, verified: {} as never })),
  };
}

function domainAsset(id: string) {
  return {
    id,
    workspaceId: "ws-r2",
    assetKey: `asset:${id}`,
    platform: "xhs",
    platformContentId: id,
    contentType: "note",
  };
}

async function delay(milliseconds: number) {
  await new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function client(dsn: string): PrismaClient {
  return createIndependentPrismaClient(dsn);
}

function requiredEnv(name: string): string {
  if (!enabled) return "postgresql://disabled";
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required when V2_WORKER_INTEGRATION_DB=1`);
  return value;
}

async function seedWork(db: PrismaClient, id: string) {
  const snapshotId = `${id}-snapshot`;
  const receiptId = `${id}-receipt`;
  const packageId = `${id}-package`;
  await db.$executeRaw(Prisma.sql`
    INSERT INTO "CapturePackage"(
      "id","workspaceId","packagePayload","checksumAlgorithm","checksumValue","contentLength"
    ) VALUES (
      ${packageId},'ws-r2',decode('7b7d','hex'),'sha256',${"a".repeat(64)},2
    )
  `);
  await db.$executeRaw(Prisma.sql`
    INSERT INTO "RawSnapshot"(
      "id","workspaceId","captureId","platform","targetKey","observedAt",
      "capturePackageId","integrityStatus","lifecycleStatus"
    ) VALUES (
      ${snapshotId},'ws-r2',${`${id}-capture`},'xhs',${`${id}-target`},CURRENT_TIMESTAMP,
      ${packageId},'verified','ACTIVE'
    )
  `);
  await db.$executeRaw(Prisma.sql`
    INSERT INTO "EvidenceIngressReceipt"(
      "id","workspaceId","rawSnapshotId","captureId","ingressKind","collectorVersion"
    ) VALUES (
      ${receiptId},'ws-r2',${snapshotId},${`${id}-capture`},'manual_import','r2-proof'
    )
  `);
  await db.$executeRaw(Prisma.sql`
    INSERT INTO "V2DurableWork"(
      "id","workspaceId","rawSnapshotId","receiptId","status"
    ) VALUES (${id},'ws-r2',${snapshotId},${receiptId},'pending')
  `);
  return { id };
}

async function seedAcceptedEvaluation(db: PrismaClient, workId: string): Promise<string> {
  const id = `${workId}-evaluation`;
  await db.contractEvaluation.create({
    data: {
      id,
      workspaceId: "ws-r2",
      rawSnapshotId: `${workId}-snapshot`,
      contractId: "contract-proof",
      contractVersion: 1,
      evaluatorVersion: "proof",
      canonicalSchemaVersion: "proof",
      evaluationInputHash: "b".repeat(64),
      decision: "accepted",
      completeness: "full",
    },
  });
  return id;
}
