/**
 * Execution Authority Provenance Audit (DEC-B1-023 / B1-B-10).
 *
 * Every fact below is now traceable through the dark strict-session verifier
 * and execution adapter. Existing V1 callers remain on their current path.
 */

const stationId = {
  verdict: "TRACEABLE" as const,
  sources: [
    "src/lib/services/execution-station-signature-service.ts (strict V2 station-token + HMAC verification)",
    "src/lib/evidence/adapters/execution-adapter.ts (session station bound to attempt and queue)",
  ],
};

const workspaceId = {
  verdict: "TRACEABLE" as const,
  sources: [
    "prisma/schema.prisma (ExecutionJob.workspaceId — NOT NULL)",
    "src/lib/services/execution-station-signature-service.ts (non-null station/authorization workspace equality)",
    "src/lib/evidence/adapters/execution-adapter.ts (job/queue/session workspace equality)",
  ],
};

const sourcePrincipal = {
  verdict: "TRACEABLE" as const,
  sources: [
    "docs/architecture/v2/01-decisions.md (DEC-B1-023)",
    "src/lib/evidence/adapters/execution-adapter.ts (execution-station:<stationId>)",
  ],
};

const taskAttemptToJob = {
  verdict: "TRACEABLE" as const,
  sources: [
    "prisma/schema.prisma (TaskAttempt.jobId → ExecutionJob.id FK)",
    "src/lib/evidence/adapters/execution-adapter.ts (current attempt/job equality)",
  ],
};

const activeLease = {
  verdict: "TRACEABLE" as const,
  sources: [
    "prisma/schema.prisma (ExecutionQueueEntry lease fields)",
    "src/lib/services/execution-queue-claim-service-v11.ts (startJob/progressLease authority)",
    "src/lib/evidence/adapters/execution-adapter.ts (SERIALIZABLE current lease re-read)",
  ],
};

const executionPlanVersion = {
  verdict: "TRACEABLE" as const,
  sources: [
    "prisma/schema.prisma (ExecutionJob.executionPlanVersion)",
    "prisma/migrations/20260810143000_add_execution_job_plan_version/migration.sql",
    "src/lib/evidence/adapters/execution-adapter.ts (job/header plan equality; null rejected)",
  ],
};

export const EXECUTION_PROVENANCE = {
  facts: {
    stationId,
    workspaceId,
    sourcePrincipal,
    taskAttemptToJob,
    activeLease,
    executionPlanVersion,
  },
  signedSessionBinding: {
    verdict: "TRACEABLE" as const,
    sources: [
      "src/lib/services/execution-station-signature-service.ts (station token, mandatory HMAC, plugin authorization and non-null workspace binding)",
      "src/lib/evidence/adapters/execution-adapter.ts (session/job/queue workspace equality)",
    ],
  },
  canConstructProductionAuthority: true as const,
  blockers: [] as const,
} as const;
