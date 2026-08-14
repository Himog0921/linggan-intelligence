/**
 * Dark V2 execution ingress adapter (DEC-B1-023 / B1-B-10).
 *
 * It binds a strict, server-verified station session to the current job,
 * attempt, queue lease, workspace, and immutable job plan version. It has no
 * route caller and never advances execution control state.
 */

import { createHash } from "node:crypto";

import {
  readVerifiedExecutionStationSession,
  type VerifiedExecutionStationSession,
  type VerifiedExecutionStationSessionClaims,
} from "@/lib/services/execution-station-signature-service";
import type {
  AuthorityValidationResult,
  EvidenceIngressAuthorityValidator,
} from "../ingress/authority-validator";
import type {
  CaptureHeaderV2,
  CaptureSubmissionBodyV2,
  CaptureSubmissionV2,
  EvidenceIngressAuthorityV2,
} from "../ingress/types";

type JobAuthorityRow = {
  id: string;
  workspaceId: string;
  status: string;
  executionPlanVersion: string | null;
};

type AttemptAuthorityRow = {
  id: string;
  jobId: string;
  stationId: string | null;
  leaseToken: string | null;
  leaseEpoch: number;
  endedAt: Date | null;
  result: string | null;
};

type QueueAuthorityRow = {
  jobId: string;
  workspaceId: string | null;
  status: string;
  reservedByStationId: string | null;
  leaseToken: string | null;
  leaseEpoch: number;
  leaseExpiresAt: Date | null;
  lastAttemptId: string | null;
};

type ExistingExecutionEvidenceRow = {
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
};

export type ExecutionAuthorityTransaction = {
  executionJob: {
    findUnique(args: {
      where: { id: string };
      select: {
        id: true;
        workspaceId: true;
        status: true;
        executionPlanVersion: true;
      };
    }): Promise<JobAuthorityRow | null>;
  };
  taskAttempt: {
    findUnique(args: {
      where: { id: string };
      select: {
        id: true;
        jobId: true;
        stationId: true;
        leaseToken: true;
        leaseEpoch: true;
        endedAt: true;
        result: true;
      };
    }): Promise<AttemptAuthorityRow | null>;
  };
  executionQueueEntry: {
    findUnique(args: {
      where: { jobId: string };
      select: {
        jobId: true;
        workspaceId: true;
        status: true;
        reservedByStationId: true;
        leaseToken: true;
        leaseEpoch: true;
        leaseExpiresAt: true;
        lastAttemptId: true;
      };
    }): Promise<QueueAuthorityRow | null>;
  };
  rawSnapshot: {
    findFirst(args: {
      where: {
        workspaceId: string;
        captureId: string;
        checksumAlgorithm: string;
        checksumValue: string;
        capturePackageId: { not: null };
      };
      select: {
        id: true;
        workspaceId: true;
        captureId: true;
        checksumAlgorithm: true;
        checksumValue: true;
        integrityStatus: true;
        jobId: true;
        attemptId: true;
        stationId: true;
        leaseEpoch: true;
        executionPlanVersion: true;
      };
    }): Promise<ExistingExecutionEvidenceRow | null>;
  };
};

export interface ExecutionAuthorityDatabase {
  $transaction<T>(
    callback: (tx: ExecutionAuthorityTransaction) => Promise<T>,
    options: { isolationLevel: "Serializable" },
  ): Promise<T>;
}

export type ExecutionIngressBinding = {
  submission: CaptureSubmissionV2;
  authorityValidator: EvidenceIngressAuthorityValidator;
};

type ExecutionHeader = Extract<CaptureHeaderV2, { ingressKind: "execution" }>;
type ExecutionAuthority = Extract<
  EvidenceIngressAuthorityV2,
  { ingressKind: "execution" }
>;

type BoundExecutionIdentity = {
  workspaceId: string;
  stationId: string;
  jobId: string;
  attemptId: string;
  leaseToken: string;
  leaseEpoch: number;
  executionPlanVersion: string;
  captureId: string;
  checksumAlgorithm: "sha256";
  checksumValue: string;
};

const AUTHORITY_FIELDS = [
  "workspaceId",
  "receivedAt",
  "sourcePrincipal",
  "stationId",
  "leaseToken",
  "importerIdentity",
  "recoveryAuthorizedBy",
  "migrationAuthorization",
] as const;

/**
 * Create a request-owned execution submission and a validator that re-reads
 * the same authority facts immediately before EvidenceIngress decodes bytes.
 */
export async function buildExecutionIngressBinding(
  signedBodyText: string,
  verifiedSession: VerifiedExecutionStationSession,
  db: ExecutionAuthorityDatabase,
  clock: () => Date = () => new Date(),
): Promise<ExecutionIngressBinding> {
  const sessionSnapshot = readVerifiedExecutionStationSession(verifiedSession);
  assertSignedRequestBinding(signedBodyText, sessionSnapshot);
  const bodySnapshot = parseSignedBody(signedBodyText);
  const receivedAt = validServerTime(clock());

  const header = assertExecutionBodyBoundary(bodySnapshot);
  const identity = await loadAndVerifyIdentity(
    db,
    header,
    bodySnapshot.capturePackage,
    sessionSnapshot,
    receivedAt,
  );

  const authority: ExecutionAuthority = {
    ingressKind: "execution",
    workspaceId: identity.workspaceId,
    receivedAt,
    sourcePrincipal: `execution-station:${identity.stationId}`,
    stationId: identity.stationId,
    leaseToken: identity.leaseToken,
  };

  return {
    submission: {
      body: bodySnapshot,
      authority: cloneAuthority(authority),
    },
    authorityValidator: new CurrentExecutionAuthorityValidator(
      db,
      identity,
      authority,
      clock,
    ),
  };
}

class CurrentExecutionAuthorityValidator
  implements EvidenceIngressAuthorityValidator
{
  private readonly identity: BoundExecutionIdentity;
  private readonly authority: ExecutionAuthority;

  constructor(
    private readonly db: ExecutionAuthorityDatabase,
    identity: BoundExecutionIdentity,
    authority: ExecutionAuthority,
    private readonly clock: () => Date,
  ) {
    this.identity = structuredClone(identity);
    this.authority = cloneAuthority(authority);
  }

  async validate(
    authority: EvidenceIngressAuthorityV2,
    header: CaptureHeaderV2,
  ): Promise<AuthorityValidationResult> {
    if (
      header.ingressKind !== "execution" ||
      !authorityEquals(authority, this.authority) ||
      !headerEqualsIdentity(header, this.identity)
    ) {
      return invalidAuthority();
    }

    const current = await loadIdentity(this.db, this.identity);
    const now = validServerTime(this.clock());
    return contextMatches(current, this.identity, now)
      ? { valid: true }
      : invalidAuthority();
  }
}

async function loadAndVerifyIdentity(
  db: ExecutionAuthorityDatabase,
  header: ExecutionHeader,
  capturePackage: CaptureSubmissionBodyV2["capturePackage"],
  session: VerifiedExecutionStationSessionClaims,
  now: Date,
): Promise<BoundExecutionIdentity> {
  const identity: BoundExecutionIdentity = {
    workspaceId: session.workspaceId,
    stationId: session.stationId,
    jobId: header.jobId,
    attemptId: header.attemptId,
    leaseToken: "",
    leaseEpoch: header.leaseEpoch,
    executionPlanVersion: header.executionPlanVersion,
    captureId: header.captureId,
    checksumAlgorithm: capturePackage.checksumAlgorithm,
    checksumValue: capturePackage.checksumValue,
  };
  const current = await loadIdentity(db, identity);
  identity.leaseToken = current.queue?.leaseToken ?? current.attempt?.leaseToken ?? "";

  if (!contextMatches(current, identity, now)) {
    throw new Error(
      "Execution submission is not bound to a verified current execution context.",
    );
  }
  return identity;
}

async function loadIdentity(
  db: ExecutionAuthorityDatabase,
  identity: BoundExecutionIdentity,
): Promise<{
  job: JobAuthorityRow | null;
  attempt: AttemptAuthorityRow | null;
  queue: QueueAuthorityRow | null;
  existingEvidence: ExistingExecutionEvidenceRow | null;
}> {
  return db.$transaction(
    async (tx) => {
      const job = await tx.executionJob.findUnique({
        where: { id: identity.jobId },
        select: {
          id: true,
          workspaceId: true,
          status: true,
          executionPlanVersion: true,
        },
      });
      const attempt = await tx.taskAttempt.findUnique({
        where: { id: identity.attemptId },
        select: {
          id: true,
          jobId: true,
          stationId: true,
          leaseToken: true,
          leaseEpoch: true,
          endedAt: true,
          result: true,
        },
      });
      const queue = await tx.executionQueueEntry.findUnique({
        where: { jobId: identity.jobId },
        select: {
          jobId: true,
          workspaceId: true,
          status: true,
          reservedByStationId: true,
          leaseToken: true,
          leaseEpoch: true,
          leaseExpiresAt: true,
          lastAttemptId: true,
        },
      });
      const existingEvidence = await tx.rawSnapshot.findFirst({
        where: {
          workspaceId: identity.workspaceId,
          captureId: identity.captureId,
          checksumAlgorithm: identity.checksumAlgorithm,
          checksumValue: identity.checksumValue,
          capturePackageId: { not: null },
        },
        select: {
          id: true,
          workspaceId: true,
          captureId: true,
          checksumAlgorithm: true,
          checksumValue: true,
          integrityStatus: true,
          jobId: true,
          attemptId: true,
          stationId: true,
          leaseEpoch: true,
          executionPlanVersion: true,
        },
      });
      return { job, attempt, queue, existingEvidence };
    },
    { isolationLevel: "Serializable" },
  );
}

function contextMatches(
  current: {
    job: JobAuthorityRow | null;
    attempt: AttemptAuthorityRow | null;
    queue: QueueAuthorityRow | null;
    existingEvidence: ExistingExecutionEvidenceRow | null;
  },
  expected: BoundExecutionIdentity,
  now: Date,
): boolean {
  const { job, attempt, queue, existingEvidence } = current;
  const sharedIdentityMatches = Boolean(
    job &&
      attempt &&
      queue &&
      job.id === expected.jobId &&
      job.workspaceId === expected.workspaceId &&
      job.executionPlanVersion === expected.executionPlanVersion &&
      attempt.id === expected.attemptId &&
      attempt.jobId === expected.jobId &&
      attempt.stationId === expected.stationId &&
      attempt.leaseToken === expected.leaseToken &&
      attempt.leaseEpoch === expected.leaseEpoch &&
      queue.jobId === expected.jobId &&
      queue.workspaceId === expected.workspaceId &&
      queue.leaseEpoch === expected.leaseEpoch &&
      queue.lastAttemptId === expected.attemptId,
  );
  if (!sharedIdentityMatches || !job || !attempt || !queue) return false;

  const liveContextMatches =
    job.status === "in_progress" &&
    attempt.endedAt === null &&
    queue.status === "in_progress" &&
    queue.reservedByStationId === expected.stationId &&
    queue.leaseToken === expected.leaseToken &&
      queue.leaseExpiresAt !== null &&
      queue.leaseExpiresAt.getTime() > now.getTime();
  if (liveContextMatches) return true;

  // Evidence commits before execution control advances. If that independent
  // control transaction fails, an exact verified package must remain able to
  // re-enter after the wall-clock lease deadline so it can retry the CAS. The
  // stored lease/attempt identity must still be unchanged; a re-lease or a
  // different package therefore remains fail-closed.
  const controlPendingReplayMatches =
    job.status === "in_progress" &&
    attempt.endedAt === null &&
    attempt.result === null &&
    queue.status === "in_progress" &&
    queue.reservedByStationId === expected.stationId &&
    queue.leaseToken === expected.leaseToken &&
    exactVerifiedEvidenceMatches(existingEvidence, expected);
  if (controlPendingReplayMatches) return true;

  return Boolean(
    job.status === "raw_committed" &&
      attempt.result === "success" &&
      attempt.endedAt !== null &&
      queue.status === "raw_committed" &&
      queue.reservedByStationId === null &&
      queue.leaseToken === null &&
      queue.leaseExpiresAt === null &&
      exactVerifiedEvidenceMatches(existingEvidence, expected),
  );
}

function exactVerifiedEvidenceMatches(
  evidence: ExistingExecutionEvidenceRow | null,
  expected: BoundExecutionIdentity,
): boolean {
  return Boolean(
    evidence &&
      evidence.workspaceId === expected.workspaceId &&
      evidence.captureId === expected.captureId &&
      evidence.checksumAlgorithm === expected.checksumAlgorithm &&
      evidence.checksumValue === expected.checksumValue &&
      evidence.integrityStatus === "verified" &&
      evidence.jobId === expected.jobId &&
      evidence.attemptId === expected.attemptId &&
      evidence.stationId === expected.stationId &&
      evidence.leaseEpoch === expected.leaseEpoch &&
      evidence.executionPlanVersion === expected.executionPlanVersion,
  );
}

function assertExecutionBodyBoundary(
  body: CaptureSubmissionBodyV2,
): ExecutionHeader {
  if (!isRecord(body) || !isRecord(body.header) || !isRecord(body.capturePackage)) {
    throw new Error(
      "CaptureSubmissionBodyV2 must contain object header and capturePackage fields for execution ingress.",
    );
  }
  if (body.header.ingressKind !== "execution") {
    throw new Error(
      "CaptureSubmissionBodyV2.header.ingressKind must be execution for execution ingress.",
    );
  }

  const containers: readonly object[] = [body, body.header];
  const leaks = AUTHORITY_FIELDS.filter((field) =>
    containers.some((container) => Object.hasOwn(container, field)),
  );
  if (leaks.length > 0) {
    throw new Error(
      `CaptureSubmissionBodyV2 crosses the execution trust boundary with server-authority fields: ${leaks.join(", ")}.`,
    );
  }

  requiredIdentifier(body.header.jobId, "header.jobId");
  requiredIdentifier(body.header.attemptId, "header.attemptId");
  requiredIdentifier(
    body.header.executionPlanVersion,
    "header.executionPlanVersion",
  );
  if (!Number.isInteger(body.header.leaseEpoch) || body.header.leaseEpoch < 0) {
    throw new Error("header.leaseEpoch must be a non-negative integer.");
  }
  return body.header;
}

function assertSignedRequestBinding(
  signedBodyText: string,
  session: VerifiedExecutionStationSessionClaims,
): void {
  requiredIdentifier(session.stationId, "verifiedSession.stationId");
  requiredIdentifier(session.workspaceId, "verifiedSession.workspaceId");
  requiredIdentifier(session.pluginAuthorizationId, "verifiedSession.pluginAuthorizationId");
  if (
    !Number.isInteger(session.signingSecretVersion) ||
    session.signingSecretVersion < 1 ||
    !(session.verifiedAt instanceof Date) ||
    !Number.isFinite(session.verifiedAt.getTime())
  ) {
    throw new Error("Verified execution station session metadata is invalid.");
  }
  if (
    session.requestMethod !== "POST" ||
    !session.requestPath.startsWith("/") ||
    sha256Text(signedBodyText) !== session.requestBodySha256
  ) {
    throw new Error("Signed execution station session does not bind this request body.");
  }
}

function parseSignedBody(signedBodyText: string): CaptureSubmissionBodyV2 {
  let value: unknown;
  try {
    value = JSON.parse(signedBodyText);
  } catch {
    throw new Error("Signed execution request body must be valid JSON.");
  }
  if (!isRecord(value) || !isRecord(value.header) || !isRecord(value.capturePackage)) {
    throw new Error(
      "Signed execution request must contain object header and capturePackage fields.",
    );
  }
  return value as CaptureSubmissionBodyV2;
}

function sha256Text(value: string): string {
  return createHash("sha256").update(value).digest("hex");
}

function headerEqualsIdentity(
  header: ExecutionHeader,
  identity: BoundExecutionIdentity,
): boolean {
  return (
    header.jobId === identity.jobId &&
    header.attemptId === identity.attemptId &&
    header.leaseEpoch === identity.leaseEpoch &&
    header.executionPlanVersion === identity.executionPlanVersion
  );
}

function authorityEquals(
  actual: EvidenceIngressAuthorityV2,
  expected: ExecutionAuthority,
): boolean {
  if (!isRecord(actual) || actual.ingressKind !== "execution") return false;
  const keys = [
    "ingressKind",
    "workspaceId",
    "receivedAt",
    "sourcePrincipal",
    "stationId",
    "leaseToken",
  ];
  if (
    Reflect.ownKeys(actual).length !== keys.length ||
    !keys.every((key) => Object.hasOwn(actual, key))
  ) {
    return false;
  }
  return (
    actual.workspaceId === expected.workspaceId &&
    actual.sourcePrincipal === expected.sourcePrincipal &&
    actual.stationId === expected.stationId &&
    actual.leaseToken === expected.leaseToken &&
    actual.receivedAt instanceof Date &&
    actual.receivedAt.getTime() === expected.receivedAt.getTime()
  );
}

function cloneAuthority(authority: ExecutionAuthority): ExecutionAuthority {
  return {
    ...authority,
    receivedAt: new Date(authority.receivedAt.getTime()),
  };
}

function requiredIdentifier(value: unknown, field: string): string {
  if (typeof value !== "string" || !value.trim()) {
    throw new Error(`${field} must be a non-empty verified identifier.`);
  }
  return value.trim();
}

function validServerTime(value: Date): Date {
  if (!(value instanceof Date) || !Number.isFinite(value.getTime())) {
    throw new Error("The server clock must return a valid Date.");
  }
  return new Date(value.getTime());
}

function invalidAuthority(): AuthorityValidationResult {
  return { valid: false, reason: "execution_authority_invalid" };
}

function isRecord(value: unknown): value is Record<PropertyKey, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export const EXECUTION_AUTHORITY_DECISIONS = {
  SOURCE_PRINCIPAL: {
    id: "DR-B1-005-001",
    status: "CLOSED",
    decision: "DEC-B1-023",
  },
  EXECUTION_PLAN_VERSION: {
    id: "DR-B1-005-002",
    status: "CLOSED",
    decision: "DEC-B1-023",
  },
  SIGNED_STATION_WORKSPACE: {
    id: "DR-B1-005-003",
    status: "CLOSED",
    decision: "DEC-B1-023",
  },
} as const;
