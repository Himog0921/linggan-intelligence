import { readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import {
  NON_EXECUTION_PROVENANCE,
  NON_EXECUTION_PROVENANCE_AUDIT,
} from "./non-execution-provenance-audit";

const REPO_ROOT = path.resolve(__dirname, "../../../..");

function source(relativePath: string): string {
  return readFileSync(path.join(REPO_ROOT, relativePath), "utf8");
}

describe("non-execution authority readiness", () => {
  it("allows dark manual_import and recovery authority construction", () => {
    expect(NON_EXECUTION_PROVENANCE.manual_import.canConstructProductionAuthority).toBe(true);
    expect(NON_EXECUTION_PROVENANCE.recovery.canConstructProductionAuthority).toBe(true);
    expect(NON_EXECUTION_PROVENANCE.manual_import.blockers).toEqual([]);
    expect(NON_EXECUTION_PROVENANCE.recovery.blockers).toEqual([]);
  });

  it("keeps migration fully blocked without an authorized caller", () => {
    expect(NON_EXECUTION_PROVENANCE.migration.canConstructProductionAuthority).toBe(false);
    expect(NON_EXECUTION_PROVENANCE.migration.blockers).toEqual([
      "DR-B1-006-M-001",
      "DR-B1-006-M-002",
      "DR-B1-006-M-003",
      "DR-B1-006-M-004",
      "DR-B1-006-M-005",
      "DR-B1-006-M-006",
    ]);
  });

  it("keeps the aggregate gate closed while migration remains unresolved", () => {
    expect(NON_EXECUTION_PROVENANCE_AUDIT.canConstructProductionAuthority).toBe(false);
    expect(NON_EXECUTION_PROVENANCE_AUDIT.blockers).toEqual(
      NON_EXECUTION_PROVENANCE.migration.blockers,
    );
  });

  it("wires V2 EvidenceIngress into the hard-cut manual import and recovery routes", () => {
    // R1-05: The existing routes now include V2 EvidenceIngress dispatch.
    const manualRoute = source("src/app/api/execution-tasks/manual-import/route.ts");
    const recoveryRoute = source("src/app/api/execution-tasks/recovery-import/route.ts");
    expect(manualRoute).toContain("submitManualImportEvidence");
    expect(recoveryRoute).toContain("submitRecoveryEvidence");
  });

  it("keeps the adapters free of database, network, and control-plane calls", () => {
    const adapters = [
      source("src/lib/evidence/adapters/manual-import-adapter.ts"),
      source("src/lib/evidence/adapters/recovery-adapter.ts"),
      source("src/lib/evidence/adapters/request-bound-authority.ts"),
    ].join("\n");
    expect(adapters).not.toContain("@/lib/db");
    expect(adapters).not.toMatch(/\bfetch\b/);
    expect(adapters).not.toMatch(/executionJob|taskAttempt|executionTaskRuntime/);
  });
});
