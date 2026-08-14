import { beforeEach, describe, expect, it, vi } from "vitest";

const { prismaMock } = vi.hoisted(() => ({
  prismaMock: {
    executionStation: { findUnique: vi.fn() },
    pluginAuthorization: { findUnique: vi.fn() },
    executionStationRequestNonce: {
      deleteMany: vi.fn(),
      createMany: vi.fn(),
    },
    $transaction: vi.fn(),
  },
}));

vi.mock("@/lib/db", () => ({ prisma: prismaMock }));

import {
  sha256Hex,
  signStationRequest,
  verifyExecutionStationIngressSession,
} from "@/lib/services/execution-station-signature-service";
import { encryptStationSigningSecret } from "@/lib/services/execution-station-signing-secret";
import type { CaptureSubmissionBodyV2 } from "../ingress/types";
import {
  buildExecutionIngressBinding,
  EXECUTION_AUTHORITY_DECISIONS,
  type ExecutionAuthorityDatabase,
  type ExecutionAuthorityTransaction,
} from "./execution-adapter";

const NOW = new Date("2026-08-10T04:00:00.000Z");
const SIGNING_SECRET = "signing-secret-1";
const STATION_TOKEN = "station-token-1";
const AUTHORIZATION_TOKEN = "authorization-token-1";

function makeBody(): CaptureSubmissionBodyV2 {
  return {
    header: {
      protocolVersion: "capture-submission/v2",
      captureId: "capture-1",
      platform: "xhs",
      target: {
        expectedTargetKey: "xhs:note/test",
        observedTargetKey: "xhs:note/test",
      },
      observedAt: "2026-08-10T03:59:00.000Z",
      collectorVersion: "2.0.92",
      contractId: "xhs.note-detail",
      contractVersion: 1,
      contractHash: "a".repeat(64),
      report: {
        startedAt: "2026-08-10T03:58:00.000Z",
        completedAt: "2026-08-10T03:59:00.000Z",
        terminal: {
          state: "completed",
          reason: "source_exhausted",
          retryable: false,
        },
        slots: [],
        counters: {
          requested: 1,
          discovered: 1,
          emitted: 1,
          deduplicated: 0,
          failed: 0,
        },
        diagnostics: {},
      },
      ingressKind: "execution",
      jobId: "job-1",
      attemptId: "attempt-1",
      leaseEpoch: 4,
      executionPlanVersion: "plan-version-1",
    },
    capturePackage: {
      encoding: "base64",
      packagePayload: "e30=",
      checksumAlgorithm: "sha256",
      checksumValue: "b".repeat(64),
      contentLength: 2,
      restricted: false,
    },
  };
}

type ContextState = {
  job: {
    id: string;
    workspaceId: string;
    status: string;
    executionPlanVersion: string | null;
  } | null;
  attempt: {
    id: string;
    jobId: string;
    stationId: string | null;
    leaseToken: string | null;
    leaseEpoch: number;
    endedAt: Date | null;
    result: string | null;
  } | null;
  queue: {
    jobId: string;
    workspaceId: string | null;
    status: string;
    reservedByStationId: string | null;
    leaseToken: string | null;
    leaseEpoch: number;
    leaseExpiresAt: Date | null;
    lastAttemptId: string | null;
  } | null;
  existingSnapshot: {
    id: string;
    workspaceId: string;
    captureId: string;
    checksumAlgorithm: string | null;
    checksumValue: string | null;
    integrityStatus: string | null;
    jobId: string | null;
    attemptId: string | null;
    stationId: string | null;
    leaseEpoch: number | null;
    executionPlanVersion: string | null;
  } | null;
};

function validState(): ContextState {
  return {
    job: {
      id: "job-1",
      workspaceId: "workspace-1",
      status: "in_progress",
      executionPlanVersion: "plan-version-1",
    },
    attempt: {
      id: "attempt-1",
      jobId: "job-1",
      stationId: "station-1",
      leaseToken: "lease-token-1",
      leaseEpoch: 4,
      endedAt: null,
      result: null,
    },
    queue: {
      jobId: "job-1",
      workspaceId: "workspace-1",
      status: "in_progress",
      reservedByStationId: "station-1",
      leaseToken: "lease-token-1",
      leaseEpoch: 4,
      leaseExpiresAt: new Date("2026-08-10T04:05:00.000Z"),
      lastAttemptId: "attempt-1",
    },
    existingSnapshot: null,
  };
}

function finalizedReplayState(): ContextState {
  const state = validState();
  state.job!.status = "raw_committed";
  state.attempt!.endedAt = NOW;
  state.attempt!.result = "success";
  state.queue!.status = "raw_committed";
  state.queue!.reservedByStationId = null;
  state.queue!.leaseToken = null;
  state.queue!.leaseExpiresAt = null;
  state.existingSnapshot = {
    id: "snapshot-1",
    workspaceId: "workspace-1",
    captureId: "capture-1",
    checksumAlgorithm: "sha256",
    checksumValue: "b".repeat(64),
    integrityStatus: "verified",
    jobId: "job-1",
    attemptId: "attempt-1",
    stationId: "station-1",
    leaseEpoch: 4,
    executionPlanVersion: "plan-version-1",
  };
  return state;
}

class FakeExecutionAuthorityDatabase implements ExecutionAuthorityDatabase {
  readonly transactionCount = vi.fn();

  constructor(readonly state: ContextState) {}

  async $transaction<T>(
    callback: (tx: ExecutionAuthorityTransaction) => Promise<T>,
  ): Promise<T> {
    this.transactionCount();
    const tx: ExecutionAuthorityTransaction = {
      executionJob: {
        findUnique: async () => this.state.job,
      },
      taskAttempt: {
        findUnique: async () => this.state.attempt,
      },
      executionQueueEntry: {
        findUnique: async () => this.state.queue,
      },
      rawSnapshot: {
        findFirst: async () => this.state.existingSnapshot,
      },
    };
    return callback(tx);
  }
}

function signedHeaders(bodyText: string): Headers {
  const timestamp = NOW.toISOString();
  const bodyHash = sha256Hex(bodyText);
  return new Headers({
    "x-cw-station-id": "station-1",
    "x-cw-plugin-version": "2.0.92",
    "x-cw-timestamp": timestamp,
    "x-cw-nonce": `nonce-${bodyHash.slice(0, 12)}`,
    "x-cw-body-sha256": bodyHash,
    "x-cw-signature": signStationRequest({
      method: "POST",
      path: "/api/v2/evidence/execution",
      timestamp,
      nonce: `nonce-${bodyHash.slice(0, 12)}`,
      bodyHash,
      signingSecret: SIGNING_SECRET,
    }),
  });
}

async function verifyBody(bodyText: string) {
  return verifyExecutionStationIngressSession({
    stationId: "station-1",
    stationToken: STATION_TOKEN,
    pluginAuthorizationId: "plugin-authorization-1",
    pluginAuthorizationToken: AUTHORIZATION_TOKEN,
    method: "POST",
    path: "/api/v2/evidence/execution",
    headers: signedHeaders(bodyText),
    bodyText,
    now: NOW,
  });
}

async function buildBinding(
  body: CaptureSubmissionBodyV2,
  db: ExecutionAuthorityDatabase,
  clock: () => Date = () => NOW,
) {
  const bodyText = JSON.stringify(body);
  const session = await verifyBody(bodyText);
  return buildExecutionIngressBinding(bodyText, session, db, clock);
}

describe("buildExecutionIngressBinding", () => {
  beforeEach(() => {
    vi.resetAllMocks();
    prismaMock.executionStation.findUnique.mockResolvedValue({
      id: "station-1",
      workspaceId: "workspace-1",
      pluginAuthorizationId: "plugin-authorization-1",
      stationTokenHash: sha256Hex(STATION_TOKEN),
      signingSecretEncrypted: encryptStationSigningSecret(SIGNING_SECRET),
      signingSecretVersion: 2,
    });
    prismaMock.pluginAuthorization.findUnique.mockResolvedValue({
      id: "plugin-authorization-1",
      workspaceId: "workspace-1",
      authorizationTokenHash: sha256Hex(AUTHORIZATION_TOKEN),
      status: "active",
      expiresAt: null,
    });
    prismaMock.executionStationRequestNonce.deleteMany.mockResolvedValue({ count: 0 });
    prismaMock.executionStationRequestNonce.createMany.mockResolvedValue({ count: 1 });
    prismaMock.$transaction.mockImplementation(
      async (callback: (tx: typeof prismaMock) => Promise<unknown>) =>
        callback(prismaMock),
    );
  });

  it("binds the signed station session to the current server execution context", async () => {
    const db = new FakeExecutionAuthorityDatabase(validState());
    const binding = await buildBinding(makeBody(), db);

    expect(binding.submission.authority).toEqual({
      ingressKind: "execution",
      workspaceId: "workspace-1",
      receivedAt: NOW,
      sourcePrincipal: "execution-station:station-1",
      stationId: "station-1",
      leaseToken: "lease-token-1",
    });
    await expect(
      binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).resolves.toEqual({ valid: true });
    expect(db.transactionCount).toHaveBeenCalledTimes(2);
  });

  it("allows only an exact verified package replay after control already advanced", async () => {
    const db = new FakeExecutionAuthorityDatabase(finalizedReplayState());
    const binding = await buildBinding(makeBody(), db);

    expect(binding.submission.authority).toMatchObject({
      stationId: "station-1",
      leaseToken: "lease-token-1",
    });
    await expect(
      binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).resolves.toEqual({ valid: true });
  });

  it("allows an exact verified package to retry pending control after lease expiry", async () => {
    const state = validState();
    state.queue!.leaseExpiresAt = new Date(NOW.getTime() - 1);
    state.existingSnapshot = finalizedReplayState().existingSnapshot;
    const db = new FakeExecutionAuthorityDatabase(state);
    const binding = await buildBinding(makeBody(), db, () => NOW);

    await expect(
      binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).resolves.toEqual({ valid: true });
  });

  it.each([
    ["different capture", (state: ContextState) => { state.existingSnapshot!.captureId = "capture-other"; }],
    ["different package hash", (state: ContextState) => { state.existingSnapshot!.checksumValue = "c".repeat(64); }],
    ["conflict Evidence", (state: ContextState) => { state.existingSnapshot!.integrityStatus = "capture_identity_conflict"; }],
  ])("rejects %s after control already advanced", async (_label, mutate) => {
    const state = finalizedReplayState();
    mutate(state);
    await expect(
      buildBinding(makeBody(), new FakeExecutionAuthorityDatabase(state)),
    ).rejects.toThrow(/verified current execution context/);
  });

  it.each([
    ["job workspace", (state: ContextState) => { state.job!.workspaceId = "workspace-other"; }],
    ["queue workspace", (state: ContextState) => { state.queue!.workspaceId = null; }],
    ["job plan version", (state: ContextState) => { state.job!.executionPlanVersion = null; }],
    ["attempt station", (state: ContextState) => { state.attempt!.stationId = "station-other"; }],
    ["attempt lease", (state: ContextState) => { state.attempt!.leaseToken = "lease-old"; }],
    ["queue lease epoch", (state: ContextState) => { state.queue!.leaseEpoch = 3; }],
    ["queue current attempt", (state: ContextState) => { state.queue!.lastAttemptId = "attempt-old"; }],
    ["expired lease", (state: ContextState) => { state.queue!.leaseExpiresAt = new Date(NOW.getTime() - 1); }],
  ])("fails closed when %s is not bound", async (_label, mutate) => {
    const state = validState();
    mutate(state);
    await expect(
      buildBinding(
        makeBody(),
        new FakeExecutionAuthorityDatabase(state),
        () => NOW,
      ),
    ).rejects.toThrow(/verified current execution context/);
  });

  it("rejects a header plan version that differs from the immutable job version", async () => {
    const body = makeBody();
    if (body.header.ingressKind !== "execution") throw new Error("fixture invariant");
    body.header.executionPlanVersion = "plan-version-other";

    await expect(
      buildBinding(
        body,
        new FakeExecutionAuthorityDatabase(validState()),
        () => NOW,
      ),
    ).rejects.toThrow(/verified current execution context/);
  });

  it("re-reads authority and rejects a lease that becomes stale after binding", async () => {
    const state = validState();
    const db = new FakeExecutionAuthorityDatabase(state);
    const binding = await buildBinding(makeBody(), db);
    state.queue!.leaseToken = "new-lease-token";

    await expect(
      binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).resolves.toEqual({
      valid: false,
      reason: "execution_authority_invalid",
    });
  });

  it("rejects caller-supplied authority fields", async () => {
    const body = makeBody();
    Object.assign(body, { workspaceId: "workspace-hijacked" });

    await expect(
      buildBinding(
        body,
        new FakeExecutionAuthorityDatabase(validState()),
        () => NOW,
      ),
    ).rejects.toThrow(/server-authority fields/);
  });

  it("rejects body B when the genuine session was issued for signed body A", async () => {
    const bodyAText = JSON.stringify(makeBody());
    const bodyB = makeBody();
    if (bodyB.header.ingressKind !== "execution") throw new Error("fixture invariant");
    bodyB.header.captureId = "capture-b";
    const session = await verifyBody(bodyAText);

    await expect(
      buildExecutionIngressBinding(
        JSON.stringify(bodyB),
        session,
        new FakeExecutionAuthorityDatabase(validState()),
        () => NOW,
      ),
    ).rejects.toThrow(/does not bind this request body/);
  });

  it("rejects an ordinary same-shaped frozen session before any DB read", async () => {
    const db = new FakeExecutionAuthorityDatabase(validState());
    const forged = Object.freeze({
      kind: "verified_execution_station_session",
      stationId: "station-1",
      workspaceId: "workspace-1",
      pluginAuthorizationId: "plugin-authorization-1",
      signingSecretVersion: 2,
      requestMethod: "POST",
      requestPath: "/api/v2/evidence/execution",
      requestBodySha256: sha256Hex(JSON.stringify(makeBody())),
      verifiedAt: NOW,
    });

    await expect(
      buildExecutionIngressBinding(
        JSON.stringify(makeBody()),
        forged as never,
        db,
        () => NOW,
      ),
    ).rejects.toThrow(/genuine verified execution station session/);
    expect(db.transactionCount).not.toHaveBeenCalled();
  });

  it("propagates infrastructure and clock failures instead of disguising them", async () => {
    const dbError = new Error("database unavailable");
    const throwingDb: ExecutionAuthorityDatabase = {
      $transaction: async () => {
        throw dbError;
      },
    };
    const bodyText = JSON.stringify(makeBody());
    const session = await verifyBody(bodyText);
    await expect(
      buildExecutionIngressBinding(bodyText, session, throwingDb, () => NOW),
    ).rejects.toBe(dbError);

    const db = new FakeExecutionAuthorityDatabase(validState());
    const binding = await buildBinding(makeBody(), db, () => NOW);
    const clockError = new Error("clock unavailable");
    const invalidClockBinding = await buildBinding(
      makeBody(),
      db,
      (() => {
        let calls = 0;
        return () => {
          calls += 1;
          if (calls === 1) return NOW;
          throw clockError;
        };
      })(),
    );
    await expect(
      invalidClockBinding.authorityValidator.validate(
        invalidClockBinding.submission.authority,
        invalidClockBinding.submission.body.header,
      ),
    ).rejects.toBe(clockError);
    await expect(
      binding.authorityValidator.validate(
        binding.submission.authority,
        binding.submission.body.header,
      ),
    ).resolves.toEqual({ valid: true });
  });
});

describe("closed execution authority decisions", () => {
  it("contains exactly the three DR-B1-005 cards resolved by DEC-B1-023", () => {
    expect(Object.values(EXECUTION_AUTHORITY_DECISIONS)).toEqual([
      expect.objectContaining({ id: "DR-B1-005-001", status: "CLOSED" }),
      expect.objectContaining({ id: "DR-B1-005-002", status: "CLOSED" }),
      expect.objectContaining({ id: "DR-B1-005-003", status: "CLOSED" }),
    ]);
  });
});
