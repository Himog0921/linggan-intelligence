/**
 * Execution Authority Provenance Audit — read-only tests (B1-B-05-R1).
 *
 * These tests read source files from the repository root and perform no
 * network, database, outbox, or API calls.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";
import { describe, it, expect } from "vitest";
import { EXECUTION_PROVENANCE } from "./execution-provenance-audit";

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../../..");
const readRepoFile = (relativePath: string) =>
  readFileSync(resolve(REPO_ROOT, relativePath), "utf8");
const modelSection = (schema: string, model: string, nextModel: string) => {
  const start = schema.indexOf(`model ${model} {`);
  const end = schema.indexOf(`model ${nextModel} {`, start + 1);
  expect(start, `${model} must exist`).toBeGreaterThanOrEqual(0);
  expect(end, `${nextModel} must follow ${model}`).toBeGreaterThan(start);
  return schema.slice(start, end);
};

const SCHEMA = readRepoFile("prisma/schema.prisma");
const OBSERVATION_SERVICE = readRepoFile(
  "src/lib/services/execution-task-observation-service.ts",
);
const SIGNATURE_SERVICE = readRepoFile(
  "src/lib/services/execution-station-signature-service.ts",
);
const QUEUE_CLAIM_SERVICE = readRepoFile(
  "src/lib/services/execution-queue-claim-service-v11.ts",
);

// ── Schema-level facts ──────────────────────────────────────────────────

describe("schema provenance", () => {
  it("ExecutionJob.workspaceId is required", () => {
    const section = modelSection(SCHEMA, "ExecutionJob", "ExecutionJobEvent");
    expect(section).toMatch(/workspaceId\s+String\b/);
    expect(section).not.toMatch(/workspaceId\s+String\?/);
  });

  it("TaskAttempt has a real FK to ExecutionJob", () => {
    const section = modelSection(SCHEMA, "TaskAttempt", "ExecutionPlannerSnapshot");
    expect(section).toMatch(/jobId\s+String\b/);
    expect(section).toContain(
      "task ExecutionJob @relation(fields: [jobId], references: [id], onDelete: Cascade)",
    );
  });

  it("QueueEntry, not Runtime, carries the V1.1 lease authority", () => {
    const queue = modelSection(SCHEMA, "ExecutionQueueEntry", "TaskAttempt");
    const runtime = modelSection(SCHEMA, "ExecutionTaskRuntime", "ExecutionTaskWriteback");
    expect(queue).toMatch(/leaseToken\s+String\?/);
    expect(queue).toMatch(/leaseEpoch\s+Int\b/);
    expect(queue).toContain("QueueEntry 成为执行权权威");
    expect(runtime).toMatch(/leaseToken\s+String\?/);
    expect(runtime).toMatch(/leaseEpoch\s+Int\b/);
  });

  it("ExecutionJob is the plan-version provenance source", () => {
    const rawSnapshot = modelSection(SCHEMA, "RawSnapshot", "RawRecord");
    expect(rawSnapshot).toMatch(/executionPlanVersion\s+String\?/);
    const job = modelSection(SCHEMA, "ExecutionJob", "ExecutionJobEvent");
    expect(job).toMatch(/executionPlanVersion\s+String\?\s+@default\(cuid\(\)\)/);
  });

  it("station workspace binding is nullable", () => {
    const section = modelSection(SCHEMA, "ExecutionStation", "ExecutionStationRequestNonce");
    expect(section).toMatch(/workspaceId\s+String\?/);
  });
});

// ── Source-level facts ──────────────────────────────────────────────────

describe("source provenance", () => {
  it("preserves the V1 grace path but adds a strict V2 verifier", () => {
    expect(SIGNATURE_SERVICE).toContain("if (!signingSecret || !signatureIsRequired)");
    expect(SIGNATURE_SERVICE).toContain("verifyExecutionStationIngressSession");
    expect(SIGNATURE_SERVICE).toContain("STATION_TOKEN_INVALID");
    expect(SIGNATURE_SERVICE).toContain("WORKSPACE_BINDING_INVALID");
  });

  it("startJob creates the lease on QueueEntry and copies it to Runtime/Attempt", () => {
    expect(QUEUE_CLAIM_SERVICE).toContain('UPDATE "ExecutionQueueEntry"');
    expect(QUEUE_CLAIM_SERVICE).toContain('"leaseToken" = ${leaseToken}');
    expect(QUEUE_CLAIM_SERVICE).toContain('"leaseEpoch" = ${nextLeaseEpoch}');
    expect(QUEUE_CLAIM_SERVICE).toContain("taskAttempt.create");
    expect(QUEUE_CLAIM_SERVICE).toContain("executionTaskRuntime.upsert");
  });

  it("progressLease validates and CASes QueueEntry leaseToken/leaseEpoch", () => {
    expect(QUEUE_CLAIM_SERVICE).toContain('qe."leaseToken"');
    expect(QUEUE_CLAIM_SERVICE).toContain('qe."leaseEpoch"');
    expect(QUEUE_CLAIM_SERVICE).toContain('"leaseToken" = ${input.leaseToken}');
    expect(QUEUE_CLAIM_SERVICE).toContain('"leaseEpoch" = ${input.leaseEpoch}');
  });

  it("observation lease checks are not treated as the QueueEntry authority", () => {
    expect(OBSERVATION_SERVICE).toContain("assertCurrentExecutionForCriticalWrite");
  });
});

// ── Audit verdicts ──────────────────────────────────────────────────────

describe("audit verdicts", () => {
  it("stationId is traceable as an identifier", () => {
    expect(EXECUTION_PROVENANCE.facts.stationId.verdict).toBe("TRACEABLE");
  });

  it("job workspace and attempt/job FK are traceable", () => {
    expect(EXECUTION_PROVENANCE.facts.workspaceId.verdict).toBe("TRACEABLE");
    expect(EXECUTION_PROVENANCE.facts.taskAttemptToJob.verdict).toBe("TRACEABLE");
  });

  it("sourcePrincipal is traceable from DEC-B1-023", () => {
    expect(EXECUTION_PROVENANCE.facts.sourcePrincipal.verdict).toBe("TRACEABLE");
  });

  it("active lease is traceable from QueueEntry", () => {
    expect(EXECUTION_PROVENANCE.facts.activeLease.verdict).toBe("TRACEABLE");
    expect(EXECUTION_PROVENANCE.facts.activeLease.sources).toEqual(
      expect.arrayContaining([
        expect.stringContaining("ExecutionQueueEntry"),
        expect.stringContaining("progressLease"),
      ]),
    );
  });

  it("plan version and signed-session workspace binding are traceable", () => {
    expect(EXECUTION_PROVENANCE.facts.executionPlanVersion.verdict).toBe("TRACEABLE");
    expect(EXECUTION_PROVENANCE.signedSessionBinding.verdict).toBe("TRACEABLE");
  });

  it("production-shaped dark authority can be constructed", () => {
    expect(EXECUTION_PROVENANCE.canConstructProductionAuthority).toBe(true);
    expect(EXECUTION_PROVENANCE.blockers).toEqual([]);
  });
});

// ── Isolation proof ────────────────────────────────────────────────────

describe("audit isolation", () => {
  it("audit module has no runtime dependency on db/network/outbox", () => {
    const source = readRepoFile("src/lib/evidence/adapters/execution-provenance-audit.ts");
    expect(source).not.toMatch(/from ["']@\/lib\/db["']/);
    expect(source).not.toMatch(/from ["']node:(http|https|net|child_process)["']/);
    expect(source).not.toMatch(/(?:from|require\() ["'](?:@\/lib\/db|@\/lib\/prisma|node:(?:http|https|net|child_process))["']/);
    expect(source).not.toMatch(/\b(?:fetch|axios)\s*\(/);
  });

  it("audit test only performs static source reads", () => {
    const source = readFileSync(fileURLToPath(import.meta.url), "utf8");
    expect(source).not.toMatch(/from ["']@\/lib\/(db|prisma)["']/);
    expect(source).not.toMatch(/from ["']node:(http|https|net|child_process)["']/);
  });
});
