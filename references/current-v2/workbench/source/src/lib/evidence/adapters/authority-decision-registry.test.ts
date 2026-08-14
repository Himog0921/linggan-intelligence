import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { describe, expect, it } from "vitest";

import { EXECUTION_PROVENANCE } from "./execution-provenance-audit";
import { MANUAL_IMPORT_PROVENANCE } from "./manual-import-provenance-audit";
import {
  NON_EXECUTION_PROVENANCE,
  NON_EXECUTION_PROVENANCE_AUDIT,
} from "./non-execution-provenance-audit";

const REPO_ROOT = path.resolve(__dirname, "../../../..");
const LEDGER = readFileSync(
  path.join(REPO_ROOT, "docs/architecture/v2/04-blocker-ledger.md"),
  "utf8",
);
const start = LEDGER.indexOf("## Authority Decision Registry (B1-B-07)");
const end = LEDGER.indexOf("\n### GOV-001", start);
if (start < 0 || end < 0) throw new Error("Authority Decision Registry section is missing");
const AUTHORITY_LEDGER = LEDGER.slice(start, end);

const CARD_SPECS = {
  "execution.sourcePrincipal": ["DR-B1-005-001", "`EvidenceIngressAuthorityV2.sourcePrincipal` (execution)", "CLOSED"],
  "execution.executionPlanVersion": ["DR-B1-005-002", "`CaptureHeaderV2.executionPlanVersion` (execution variant)", "CLOSED"],
  "execution.signedSessionBinding": ["DR-B1-005-003", "`ExecutionJob.workspaceId` 与签名工位会话的 workspace binding", "CLOSED"],
  "manual_import.sourcePrincipal": ["DR-B1-006-001", "`EvidenceIngressAuthorityV2.sourcePrincipal` (manual_import)", "CLOSED"],
  "manual_import.importerIdentity": ["DR-B1-006-002", "`EvidenceIngressAuthorityV2.importerIdentity` (manual_import)", "CLOSED"],
  "manual_import.workspaceId": ["DR-B1-006-003", "`PluginAuthorization.workspaceId` 与 session `workspaceId` 的等同性", "CLOSED"],
  "manual_import.receivedAt": ["DR-B1-006-004", "`EvidenceIngressAuthorityV2.receivedAt` (manual_import)", "CLOSED"],
  "manual_import.noExecutionFields": ["DR-B1-006-005", "`jobId`/`attemptId`/`stationId`/`leaseToken`/`leaseEpoch`/`executionPlanVersion` 在 manual_import 上为 null", "CLOSED"],
  "recovery.sourcePrincipal": ["DR-B1-006-R-001", "`EvidenceIngressAuthorityV2.sourcePrincipal` (recovery)", "CLOSED"],
  "recovery.recoveryAuthorizedBy": ["DR-B1-006-R-002", "`EvidenceIngressAuthorityV2.recoveryAuthorizedBy` (recovery)", "CLOSED"],
  "recovery.receivedAt": ["DR-B1-006-R-003", "`EvidenceIngressAuthorityV2.receivedAt` (recovery)", "CLOSED"],
  "recovery.noBodyAuthorityOverride": ["DR-B1-006-R-004", "recovery body 不得覆盖 authority 或携带 execution identity", "CLOSED"],
  "recovery.executionIdentitySeparation": ["DR-B1-006-R-005", "execution 专属字段在 recovery Evidence 上必须为 null", "CLOSED"],
  "migration.workspaceId": ["DR-B1-006-M-001", "`EvidenceIngressAuthorityV2.workspaceId` (migration)", "DECISION_REQUIRED"],
  "migration.sourcePrincipal": ["DR-B1-006-M-002", "`EvidenceIngressAuthorityV2.sourcePrincipal` (migration)", "DECISION_REQUIRED"],
  "migration.migrationAuthorization": ["DR-B1-006-M-003", "`EvidenceIngressAuthorityV2.migrationAuthorization` (migration)", "DECISION_REQUIRED"],
  "migration.receivedAt": ["DR-B1-006-M-004", "`EvidenceIngressAuthorityV2.receivedAt` (migration)", "DECISION_REQUIRED"],
  "migration.noBodyAuthorityOverride": ["DR-B1-006-M-005", "migration Evidence 不得接受 body-controlled authority 字段", "DECISION_REQUIRED"],
  "migration.executionIdentitySeparation": ["DR-B1-006-M-006", "migration Evidence 不得携带 `jobId`/`attemptId`/lease/task-attempt execution identity", "DECISION_REQUIRED"],
} as const;

const AUDIT_FACTS = {
  "execution.sourcePrincipal": EXECUTION_PROVENANCE.facts.sourcePrincipal,
  "execution.executionPlanVersion": EXECUTION_PROVENANCE.facts.executionPlanVersion,
  "execution.signedSessionBinding": EXECUTION_PROVENANCE.signedSessionBinding,
  "manual_import.sourcePrincipal": MANUAL_IMPORT_PROVENANCE.facts.sourcePrincipal,
  "manual_import.importerIdentity": MANUAL_IMPORT_PROVENANCE.facts.importerIdentity,
  "manual_import.workspaceId": MANUAL_IMPORT_PROVENANCE.facts.workspaceId,
  "manual_import.receivedAt": MANUAL_IMPORT_PROVENANCE.facts.receivedAt,
  "manual_import.noExecutionFields": MANUAL_IMPORT_PROVENANCE.facts.noExecutionFields,
  "recovery.sourcePrincipal": NON_EXECUTION_PROVENANCE.recovery.facts.sourcePrincipal,
  "recovery.recoveryAuthorizedBy": NON_EXECUTION_PROVENANCE.recovery.facts.recoveryAuthorizedBy,
  "recovery.receivedAt": NON_EXECUTION_PROVENANCE.recovery.facts.receivedAt,
  "recovery.noBodyAuthorityOverride": NON_EXECUTION_PROVENANCE.recovery.facts.noBodyAuthorityOverride,
  "recovery.executionIdentitySeparation": NON_EXECUTION_PROVENANCE.recovery.facts.executionIdentitySeparation,
  "migration.workspaceId": NON_EXECUTION_PROVENANCE.migration.facts.workspaceId,
  "migration.sourcePrincipal": NON_EXECUTION_PROVENANCE.migration.facts.sourcePrincipal,
  "migration.migrationAuthorization": NON_EXECUTION_PROVENANCE.migration.facts.migrationAuthorization,
  "migration.receivedAt": NON_EXECUTION_PROVENANCE.migration.facts.receivedAt,
  "migration.noBodyAuthorityOverride": NON_EXECUTION_PROVENANCE.migration.facts.noBodyAuthorityOverride,
  "migration.executionIdentitySeparation": NON_EXECUTION_PROVENANCE.migration.facts.executionIdentitySeparation,
} as const;

type FactKey = keyof typeof CARD_SPECS;

function card(id: string): string {
  const cardStart = AUTHORITY_LEDGER.indexOf(`| ID | ${id} |`);
  if (cardStart < 0) throw new Error(`Missing authority card ${id}`);
  const next = AUTHORITY_LEDGER.indexOf("\n#### ", cardStart);
  const section = AUTHORITY_LEDGER.indexOf("\n---", cardStart);
  const candidates = [next, section].filter((value) => value > cardStart);
  return AUTHORITY_LEDGER.slice(
    cardStart,
    candidates.length > 0 ? Math.min(...candidates) : AUTHORITY_LEDGER.length,
  );
}

function row(cardText: string, label: string): string {
  const line = cardText.split("\n").find((value) => value.startsWith(`| ${label} |`));
  if (!line) throw new Error(`Missing ${label} row`);
  return line.slice(`| ${label} |`.length, -1).trim();
}

describe("authority decision registry closed world", () => {
  it("contains exactly the 19 audited authority cards", () => {
    const ids = Array.from(
      AUTHORITY_LEDGER.matchAll(/^\| ID \| (DR-B1-(?:005-\d{3}|006-(?:[RM]-)?\d{3})) \|$/gm),
      (match) => match[1],
    );
    const expected = Object.values(CARD_SPECS).map(([id]) => id);
    expect(ids).toHaveLength(19);
    expect([...ids].sort()).toEqual([...expected].sort());
  });

  it("matches every audit fact to its ledger field and current state", () => {
    expect(Object.keys(AUDIT_FACTS).sort()).toEqual(Object.keys(CARD_SPECS).sort());
    for (const key of Object.keys(CARD_SPECS) as FactKey[]) {
      const [id, field, status] = CARD_SPECS[key];
      const fact = AUDIT_FACTS[key];
      const cardText = card(id);
      expect(row(cardText, "字段"), key).toBe(field);
      expect(row(cardText, "状态"), key).toBe(`**${status}**`);
      expect(fact.verdict, key).toBe(status === "CLOSED" ? "TRACEABLE" : "DECISION_REQUIRED");
      if (status === "DECISION_REQUIRED") {
        expect("blocker" in fact ? fact.blocker : null, key).toBe(id);
      }
    }
  });

  it("closes execution, manual_import and recovery but keeps migration blocked", () => {
    expect(MANUAL_IMPORT_PROVENANCE.canConstructProductionAuthority).toBe(true);
    expect(NON_EXECUTION_PROVENANCE.recovery.canConstructProductionAuthority).toBe(true);
    expect(EXECUTION_PROVENANCE.canConstructProductionAuthority).toBe(true);
    expect(NON_EXECUTION_PROVENANCE.migration.canConstructProductionAuthority).toBe(false);
    expect(NON_EXECUTION_PROVENANCE_AUDIT.canConstructProductionAuthority).toBe(false);
  });

  it("has exactly six unresolved authority cards", () => {
    const unresolved = Object.values(CARD_SPECS).filter(([, , status]) =>
      status === "DECISION_REQUIRED"
    );
    expect(unresolved).toHaveLength(6);
  });
});

describe("authority ledger source integrity", () => {
  it("points only to existing repository files and valid line ranges", () => {
    const refs = Array.from(
      AUTHORITY_LEDGER.matchAll(/((?:src|prisma|docs)\/[A-Za-z0-9_./\[\]-]+\.(?:ts|tsx|prisma|md)):(\d+)(?:-(\d+))?/g),
    );
    expect(refs.length).toBeGreaterThan(0);
    for (const [, relativePath, startText, endText] of refs) {
      const fullPath = path.join(REPO_ROOT, relativePath);
      expect(existsSync(fullPath), relativePath).toBe(true);
      const lineCount = readFileSync(fullPath, "utf8").split("\n").length;
      expect(Number(startText)).toBeGreaterThan(0);
      expect(Number(endText ?? startText)).toBeLessThanOrEqual(lineCount);
    }
  });

  it("does not use ambiguous bare TypeScript filenames", () => {
    const files = Array.from(
      AUTHORITY_LEDGER.matchAll(/([A-Za-z0-9_./\[\]-]+\.(?:ts|tsx)):\d+(?:-\d+)?/g),
      (match) => match[1],
    );
    expect(files.length).toBeGreaterThan(0);
    for (const file of files) expect(file).toContain("/");
  });
});
