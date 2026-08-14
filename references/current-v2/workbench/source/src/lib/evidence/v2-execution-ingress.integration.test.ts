// @vitest-environment node

/**
 * Real execution-authority proof for the V2 ingress composition root.
 *
 * The administrator seeds only the authenticated station and live execution
 * lease preconditions. The production composition root must use three distinct
 * login roles for HMAC/nonce verification, immutable Evidence commit, and the
 * independent control finalizer.
 */
import { NextRequest } from "next/server";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

import { POST as postExecutionEvidence } from "@/app/api/v2/evidence/execution/route";
import { createIndependentPrismaClient } from "@/lib/db";
import { PrismaClient } from "@/lib/prisma-client";
import {
  encryptStationSigningSecret,
} from "@/lib/services/execution-station-signing-secret";
import {
  sha256Hex as sha256Text,
  signStationRequest,
} from "@/lib/services/execution-station-signature-service";

import { computeContractHash } from "./contracts/collection-contract-registry";
import { XHS_NOTE_DETAIL } from "./contracts/xhs-collection-contracts";
import { sha256Hex } from "./ingress/base64";
import { canonicalJsonString } from "./ingress/canonical-json";
import type {
  CaptureHeaderV2,
  CapturePackagePayloadV2,
  CaptureSubmissionBodyV2,
} from "./ingress/types";

const enabled = process.env.V2_EXECUTION_INGRESS_INTEGRATION_DB === "1";
const describeIntegration = enabled ? describe : describe.skip;
const PATH = "/api/v2/evidence/execution";

describeIntegration("V2 execution ingress real authority and role proof", () => {
  const admin = client(requiredEnv("V2_EXECUTION_ADMIN_DSN"));
  const suffix = `${Date.now()}`;
  const workspaceId = `ws-v2-execution-${suffix}`;
  const authorizationId = `authorization-v2-execution-${suffix}`;
  const authorizationToken = `authorization-token-${suffix}`;
  const stationId = `station-v2-execution-${suffix}`;
  const stationToken = `station-token-${suffix}`;
  const signingSecret = `station-signing-secret-${suffix}`;
  const jobId = `job-v2-execution-${suffix}`;
  const attemptId = `attempt-v2-execution-${suffix}`;
  const captureId = `capture-v2-execution-${suffix}`;
  const leaseToken = `lease-v2-execution-${suffix}`;
  const executionPlanVersion = `plan-v2-execution-${suffix}`;
  const noteId = `note-v2-execution-${suffix}`;
  let database = "";

  beforeAll(async () => {
    const rows = await admin.$queryRaw<Array<{ name: string }>>`
      SELECT current_database() AS name
    `;
    database = rows[0]?.name ?? "";
    if (!database.startsWith("content_workbench_v2_rc_e2e_")) {
      throw new Error(`Refusing non-E2E database ${database || "unknown"}`);
    }

    await admin.pluginAuthorization.create({
      data: {
        id: authorizationId,
        workspaceId,
        deviceId: `device-${suffix}`,
        authorizationTokenHash: sha256Text(authorizationToken),
        status: "active",
      },
    });
    await admin.executionStation.create({
      data: {
        id: stationId,
        stationKey: `station-key-${suffix}`,
        workspaceId,
        pluginAuthorizationId: authorizationId,
        displayName: "V2 execution proof",
        capabilities: "[]",
        stationTokenHash: sha256Text(stationToken),
        signingSecretEncrypted: encryptStationSigningSecret(signingSecret),
        signingSecretVersion: 1,
      },
    });
    await admin.executionJob.create({
      data: {
        id: jobId,
        workspaceId,
        platform: "xhs",
        targetType: "note",
        targetKey: `xhs:note/${noteId}`,
        jobType: "collect_detail",
        collectionProfile: "note_full",
        requiredFields: {},
        payload: {},
        executionPlanVersion,
        lane: "manual_hot",
        status: "in_progress",
      },
    });
    const leaseExpiresAt = new Date(Date.now() + 5 * 60_000);
    await admin.executionQueueEntry.create({
      data: {
        jobId,
        workspaceId,
        platform: "xhs",
        source: "manual",
        lane: "manual_hot",
        status: "in_progress",
        reservedByStationId: stationId,
        leaseToken,
        leaseEpoch: 1,
        leaseExpiresAt,
        lastAttemptId: attemptId,
      },
    });
    await admin.taskAttempt.create({
      data: {
        id: attemptId,
        captureId,
        jobId,
        stationId,
        leaseToken,
        leaseEpoch: 1,
      },
    });
    await admin.executionTaskRuntime.create({
      data: {
        jobId,
        workspaceId,
        status: "running",
        activeExecutor: stationId,
        assignedStationId: stationId,
        leaseToken,
        leaseEpoch: 1,
        leaseExpiresAt,
        currentAttemptId: attemptId,
        startedAt: new Date(),
      },
    });
  });

  afterAll(async () => {
    await admin.$disconnect();
    if (database) console.log(`[v2-execution] Preserving proof database ${database}.`);
  });

  it("commits once, replays with a fresh nonce, and rejects reuse of the old nonce", async () => {
    const body = buildSubmission({
      suffix,
      noteId,
      captureId,
      jobId,
      attemptId,
      executionPlanVersion,
    });
    const bodyText = JSON.stringify(body);
    const now = new Date();
    const nonce = `nonce-v2-execution-${suffix}`;
    const headers = signedHeaders({
      stationId,
      signingSecret,
      nonce,
      now,
      bodyText,
    });
    const result = await post(headers, bodyText);
    expect(result).toMatchObject({
      ok: true,
      status: "committed",
      integrityStatus: "verified",
    });
    if (result.status !== "committed" || typeof result.rawSnapshotId !== "string") {
      throw new Error(`Expected committed execution Evidence, received ${result.status}`);
    }

    const [snapshot, work, job, queue, attempt, runtime, nonces] = await Promise.all([
      admin.rawSnapshot.findUniqueOrThrow({ where: { id: result.rawSnapshotId } }),
      admin.v2DurableWork.findFirstOrThrow({
        where: { workspaceId, rawSnapshotId: result.rawSnapshotId },
      }),
      admin.executionJob.findUniqueOrThrow({ where: { id: jobId } }),
      admin.executionQueueEntry.findUniqueOrThrow({ where: { jobId } }),
      admin.taskAttempt.findUniqueOrThrow({ where: { id: attemptId } }),
      admin.executionTaskRuntime.findUniqueOrThrow({ where: { jobId } }),
      admin.executionStationRequestNonce.count({ where: { stationId, nonce } }),
    ]);
    expect(snapshot).toMatchObject({
      workspaceId,
      captureId,
      jobId,
      attemptId,
      stationId,
      leaseEpoch: 1,
      executionPlanVersion,
      integrityStatus: "verified",
    });
    expect(work).toMatchObject({ status: "pending", attemptCount: 0 });
    expect(job.status).toBe("raw_committed");
    expect(queue).toMatchObject({
      status: "raw_committed",
      reservedByStationId: null,
      leaseToken: null,
      leaseExpiresAt: null,
    });
    expect(attempt.result).toBe("success");
    expect(attempt.endedAt).not.toBeNull();
    expect(runtime).toMatchObject({
      status: "completed",
      progress: 100,
      activeExecutor: null,
      leaseToken: null,
      leaseExpiresAt: null,
    });
    expect(nonces).toBe(1);

    const freshReplay = await post(
      signedHeaders({
        stationId,
        signingSecret,
        nonce: `${nonce}-fresh-replay`,
        now,
        bodyText,
      }),
      bodyText,
    );
    expect(freshReplay).toMatchObject({
      ok: true,
      status: "replay",
      rawSnapshotId: result.rawSnapshotId,
      integrityStatus: "verified",
    });

    const controlAfterReplay = await Promise.all([
      admin.executionJob.findUniqueOrThrow({ where: { id: jobId } }),
      admin.executionQueueEntry.findUniqueOrThrow({ where: { jobId } }),
      admin.taskAttempt.findUniqueOrThrow({ where: { id: attemptId } }),
      admin.executionTaskRuntime.findUniqueOrThrow({ where: { jobId } }),
    ]);
    expect(controlAfterReplay[0].status).toBe("raw_committed");
    expect(controlAfterReplay[1]).toMatchObject({
      status: "raw_committed",
      reservedByStationId: null,
      leaseToken: null,
    });
    expect(controlAfterReplay[2].result).toBe("success");
    expect(controlAfterReplay[3]).toMatchObject({
      status: "completed",
      progress: 100,
      leaseToken: null,
    });

    await expect(post(headers, bodyText)).resolves.toMatchObject({
      ok: false,
      reason: "SIGNATURE_NONCE_REPLAYED",
      httpStatus: 401,
    });

    await expect(post(
      signedHeaders({
        stationId,
        signingSecret,
        nonce: `${nonce}-version-mismatch`,
        now,
        bodyText,
        pluginVersion: "2.0.93",
      }),
      bodyText,
    )).resolves.toMatchObject({
      ok: false,
      reason: "PLUGIN_VERSION_MISMATCH",
      httpStatus: 401,
    });

    const outdatedBody = buildSubmission({
      suffix,
      noteId,
      captureId,
      jobId,
      attemptId,
      executionPlanVersion,
      collectorVersion: "2.0.91",
    });
    const outdatedBodyText = JSON.stringify(outdatedBody);
    await expect(post(
      signedHeaders({
        stationId,
        signingSecret,
        nonce: `${nonce}-outdated-version`,
        now,
        bodyText: outdatedBodyText,
        pluginVersion: "2.0.91",
      }),
      outdatedBodyText,
    )).resolves.toMatchObject({
      ok: false,
      reason: "PLUGIN_VERSION_OUTDATED",
      httpStatus: 401,
    });

    expect(await admin.rawSnapshot.count({ where: { workspaceId, captureId } })).toBe(1);
    expect(await admin.v2DurableWork.count({ where: { workspaceId } })).toBe(1);
    expect(await admin.executionStationRequestNonce.count({ where: { stationId } })).toBe(2);

    console.log(`[v2-execution] database=${database}`);
    console.log(`[v2-execution] workspace=${workspaceId}`);
    console.log("[v2-execution] evidence=1 durableWork=1 nonce=2 control=advanced freshReplay=replay oldNonce=SIGNATURE_NONCE_REPLAYED mismatch=PLUGIN_VERSION_MISMATCH outdated=PLUGIN_VERSION_OUTDATED");
  }, 60_000);

  async function post(headers: Headers, bodyText: string): Promise<Record<string, unknown>> {
    headers.set("authorization", `Bearer ${authorizationToken}`);
    headers.set("x-cw-station-token", stationToken);
    headers.set("x-cw-plugin-authorization-id", authorizationId);
    const response = await postExecutionEvidence(new NextRequest(`http://localhost${PATH}`, {
      method: "POST",
      headers,
      body: bodyText,
    }));
    return {
      ...(await response.json() as Record<string, unknown>),
      httpStatus: response.status,
    };
  }
});

function client(dsn: string): PrismaClient {
  return createIndependentPrismaClient(dsn);
}

function requiredEnv(name: string): string {
  if (!enabled) return "postgresql://disabled";
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required when V2_EXECUTION_INGRESS_INTEGRATION_DB=1`);
  return value;
}

function signedHeaders(input: {
  stationId: string;
  signingSecret: string;
  nonce: string;
  now: Date;
  bodyText: string;
  pluginVersion?: string;
}): Headers {
  const timestamp = input.now.toISOString();
  const bodyHash = sha256Text(input.bodyText);
  return new Headers({
    "x-cw-plugin-version": input.pluginVersion ?? "2.0.92",
    "x-cw-station-id": input.stationId,
    "x-cw-timestamp": timestamp,
    "x-cw-nonce": input.nonce,
    "x-cw-body-sha256": bodyHash,
    "x-cw-signature": signStationRequest({
      method: "POST",
      path: PATH,
      timestamp,
      nonce: input.nonce,
      bodyHash,
      signingSecret: input.signingSecret,
    }),
  });
}

function buildSubmission(input: {
  suffix: string;
  noteId: string;
  captureId: string;
  jobId: string;
  attemptId: string;
  executionPlanVersion: string;
  collectorVersion?: string;
}): CaptureSubmissionBodyV2 {
  const observedAt = "2026-08-12T08:10:00.000Z";
  const targetKey = `xhs:note/${input.noteId}`;
  const header: CaptureHeaderV2 = {
    protocolVersion: "capture-submission/v2",
    ingressKind: "execution",
    captureId: input.captureId,
    platform: "xhs",
    target: {
      expectedTargetKey: targetKey,
      observedTargetKey: targetKey,
    },
    observedAt,
    collectorVersion: input.collectorVersion ?? "2.0.92",
    contractId: XHS_NOTE_DETAIL.id,
    contractVersion: XHS_NOTE_DETAIL.version,
    contractHash: computeContractHash(XHS_NOTE_DETAIL),
    jobId: input.jobId,
    attemptId: input.attemptId,
    leaseEpoch: 1,
    executionPlanVersion: input.executionPlanVersion,
    report: {
      startedAt: "2026-08-12T08:09:00.000Z",
      completedAt: observedAt,
      terminal: {
        state: "completed",
        reason: "source_exhausted",
        retryable: false,
      },
      slots: [
        { slotId: "note", status: "observed", reason: null },
        { slotId: "comments", status: "not_applicable", reason: "not_requested" },
      ],
      counters: {
        requested: 1,
        discovered: 1,
        emitted: 1,
        deduplicated: 0,
        failed: 0,
      },
      diagnostics: {},
    },
  };
  const record: CapturePackagePayloadV2["records"][number] = {
    idempotencyKey: `record-v2-execution-${input.suffix}`,
    recordKind: "note",
    platform: "xhs",
    targetKey,
    externalRecordId: input.noteId,
    sequence: 0,
    payload: {
      noteId: input.noteId,
      platformContentId: input.noteId,
      type: "normal",
      title: "V2 execution proof",
      content: "Authenticated station Evidence.",
      url: `https://www.xiaohongshu.com/explore/${input.noteId}`,
    },
    observedAt,
  };
  const capturePackage: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records: [record],
    artifacts: [],
  };
  const packageBytes = Buffer.from(canonicalJsonString(capturePackage), "utf8");
  return {
    header,
    capturePackage: {
      encoding: "base64",
      packagePayload: packageBytes.toString("base64"),
      checksumAlgorithm: "sha256",
      checksumValue: sha256Hex(packageBytes),
      contentLength: packageBytes.length,
      restricted: false,
    },
  };
}
