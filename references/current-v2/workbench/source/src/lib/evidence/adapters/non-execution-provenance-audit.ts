/**
 * Non-execution authority provenance audit (B1-B-06-R2).
 *
 * Records what the repository proves for manual_import, recovery, and
 * migration. DEC-B1-022 resolves the first two dark adapter boundaries;
 * migration deliberately remains unresolved and has no runtime caller.
 */
import { MANUAL_IMPORT_PROVENANCE } from "./manual-import-provenance-audit";

type Verdict = "TRACEABLE" | "DECISION_REQUIRED";

type AuditFact = {
  verdict: Verdict;
  sources: readonly string[];
  blocker?: string;
  question?: string;
  note?: string;
};

type AuditKind = "manual_import" | "recovery" | "migration";

type NonExecutionAudit = {
  ingressKind: AuditKind;
  facts: Readonly<Record<string, AuditFact>>;
  sources: readonly string[];
  blockers: readonly string[];
  canConstructProductionAuthority: boolean;
  createsExecutionJobOrTaskAttempt: boolean;
  executionIdentityObservation: string;
};

const recoveryFacts = {
  workspaceId: {
    verdict: "TRACEABLE" as const,
    sources: [
      "src/app/api/execution-tasks/recovery-import/route.ts:25-29 — requireRequestWorkspaceRole(owner/admin)",
      "src/lib/request-workspace.ts:144-201 — authenticated session, active workspace membership, and returned context.workspaceId",
      "src/lib/request-workspace.ts:204-213 — role gate is applied to the returned workspace context",
    ],
    note: "The current V1 route proves the owner/admin session source; the dark V2 recovery adapter now binds that verified workspace without accepting a body value.",
  },
  sourcePrincipal: {
    verdict: "TRACEABLE" as const,
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:250 — recovery sourcePrincipal comes from the owner/admin session",
      "src/lib/request-workspace.ts:190-213 — role-gated session context exposes stable userId",
      "src/lib/evidence/adapters/recovery-adapter.ts — canonical sourcePrincipal is user:<userId>",
      "docs/architecture/v2/01-decisions.md DEC-B1-022 — identity format is confirmed",
    ],
  },
  recoveryAuthorizedBy: {
    verdict: "TRACEABLE" as const,
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:250 — recoveryAuthorizedBy comes from an owner/admin session",
      "src/lib/evidence/adapters/recovery-adapter.ts — owner/admin is required and recoveryAuthorizedBy is user:<userId>",
      "src/lib/evidence/adapters/recovery-adapter.test.ts — editor and missing user identity are rejected",
    ],
  },
  receivedAt: {
    verdict: "TRACEABLE" as const,
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:252 — receivedAt uses the server clock",
      "src/lib/evidence/adapters/recovery-adapter.ts — clock is invoked inside the adapter",
      "src/lib/evidence/adapters/recovery-adapter.test.ts — fixed server clock proves export/body cannot provide receivedAt",
    ],
  },
  noBodyAuthorityOverride: {
    verdict: "TRACEABLE" as const,
    sources: [
      "src/lib/evidence/adapters/request-bound-authority.ts — rejects body/header authority fields and exact-binds the generated authority",
      "src/lib/evidence/adapters/recovery-adapter.test.ts — caller authority leakage and post-binding mutation are rejected",
    ],
  },
  executionIdentitySeparation: {
    verdict: "TRACEABLE" as const,
    sources: [
      "src/lib/evidence/ingress/types.ts:84-105 — recovery header excludes execution identity",
      "src/lib/evidence/adapters/request-bound-authority.ts — rejects job/attempt/station/lease/plan fields at runtime",
      "src/lib/evidence/adapters/recovery-adapter.test.ts — fabricated execution identity is rejected",
      "src/lib/evidence/ingress/evidence-ingress.integration.test.ts — recovery persistence writes execution columns as null",
    ],
    note: "The dark V2 adapter does not call the current V1 recovery service; route hard cutover remains a later task.",
  },
} satisfies Record<string, AuditFact>;

const migrationFacts = {
  workspaceId: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-001",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:251, 415-417 — migration is an internal controlled capability and has no current ingress",
      "src/lib/evidence/ingress/types.ts:186-192 — migration workspaceId exists only in the V2 authority type",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — migration authority appears only as a test fixture, not runtime provenance",
    ],
    question: "Which authorized internal migration caller supplies and binds workspaceId? No current migration ingress/caller was found.",
  },
  sourcePrincipal: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-002",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:251 — migration authority comes from a future authorized internal caller",
      "src/lib/evidence/ingress/types.ts:186-192 — type contract only; no runtime migration caller",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — fixture value is not an authority source",
    ],
    question: "Which internal caller and identity format are authoritative for migration sourcePrincipal?",
  },
  migrationAuthorization: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-003",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:251 — migration fields come from a future authorized internal caller",
      "src/lib/evidence/ingress/types.ts:186-192 — migrationAuthorization is a required type field only",
      "src/lib/evidence/ingress/validator.ts:526-527 — validator checks non-empty shape, not authority provenance",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — fixture value is not an authority source",
    ],
    question: "What authorization reference is issued to the internal migration caller, and where is it verified before V2 construction?",
  },
  receivedAt: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-004",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:251-252 — migration authority is future-internal and receivedAt is server-clock only",
      "src/lib/evidence/ingress/types.ts:173-178 — Date field exists only in the type contract",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — fixture does not prove a runtime clock injection",
    ],
    question: "Which internal migration adapter injects server-clock receivedAt, and where is that runtime behavior proved?",
  },
  noBodyAuthorityOverride: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-005",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:415-417 — no migration API, CLI, or scheduled task exists in the first release",
      "src/lib/evidence/ingress/types.ts:156-192 — body and authority are separate type-level inputs",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — fixture separation is not a runtime caller",
    ],
    note: "There is no current migration ingress from which to prove that a caller cannot override authority fields. The absence of an ingress is recorded as a blocker, not treated as proof of safety.",
    question: "What is the sole authorized internal migration call boundary, and how does it prevent body-controlled authority fields?",
  },
  executionIdentitySeparation: {
    verdict: "DECISION_REQUIRED" as const,
    blocker: "DR-B1-006-M-006",
    sources: [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md:244, 415-417 — non-execution migration must not carry execution identity and has no current ingress",
      "src/lib/evidence/ingress/types.ts:84-105 — migration header variant has no jobId/attemptId/lease fields",
      "src/lib/evidence/ingress/test-fixtures.ts:191-195 — fixture absence/presence cannot prove runtime separation",
    ],
    question: "Which internal migration caller proves that jobId, attemptId, lease, and task-attempt relationships never enter migration evidence?",
  },
} satisfies Record<string, AuditFact>;

function collectBlockers(facts: Readonly<Record<string, AuditFact>>): string[] {
  return Object.values(facts)
    .filter((fact) => fact.verdict === "DECISION_REQUIRED")
    .map((fact) => fact.blocker)
    .filter((blocker): blocker is string => Boolean(blocker));
}

function makeAudit(
  ingressKind: AuditKind,
  facts: Readonly<Record<string, AuditFact>>,
  sources: readonly string[],
  createsExecutionJobOrTaskAttempt: boolean,
  executionIdentityObservation: string,
): NonExecutionAudit {
  const blockers = collectBlockers(facts);
  return {
    ingressKind,
    facts,
    sources,
    blockers,
    canConstructProductionAuthority: blockers.length === 0,
    createsExecutionJobOrTaskAttempt,
    executionIdentityObservation,
  };
}

export const NON_EXECUTION_PROVENANCE = {
  manual_import: {
    ...makeAudit(
      "manual_import",
      MANUAL_IMPORT_PROVENANCE.facts,
      [
        "src/lib/evidence/adapters/manual-import-provenance-audit.ts — DEC-B1-022 provenance proof",
        "src/lib/evidence/adapters/manual-import-adapter.ts — dark authority adapter",
        "src/app/api/execution-tasks/manual-import/route.ts — current V1 route",
      ],
      false,
      "The dark V2 adapter creates no ExecutionJob or TaskAttempt. The unchanged V1 route is outside this adapter and remains active until hard cutover.",
    ),
    blockers: MANUAL_IMPORT_PROVENANCE.blockers,
  },
  recovery: makeAudit(
    "recovery",
    recoveryFacts,
    [
      "src/app/api/execution-tasks/recovery-import/route.ts",
      "src/lib/request-workspace.ts",
      "src/lib/services/plugin-local-recovery-service.ts",
      "src/lib/evidence/adapters/recovery-adapter.ts",
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md §3.4, §7",
    ],
    false,
    "The dark V2 recovery adapter creates and carries no execution identity. The unchanged V1 route remains outside this adapter until hard cutover.",
  ),
  migration: makeAudit(
    "migration",
    migrationFacts,
    [
      "docs/architecture/v2/07-evidence-ingress-release-b-contract.md §7",
      "src/lib/evidence/ingress/types.ts — type contract only",
      "src/lib/evidence/ingress/test-fixtures.ts — fixture only and explicitly excluded as runtime proof",
    ],
    false,
    "No current migration EvidenceIngress ingress/caller was found; therefore no runtime execution-identity behavior can be proven.",
  ),
} as const;

export const NON_EXECUTION_PROVENANCE_AUDIT = {
  ...NON_EXECUTION_PROVENANCE,
  canConstructProductionAuthority: false as const,
  blockers: Array.from(
    new Set([
      ...NON_EXECUTION_PROVENANCE.manual_import.blockers,
      ...NON_EXECUTION_PROVENANCE.recovery.blockers,
      ...NON_EXECUTION_PROVENANCE.migration.blockers,
    ]),
  ),
} as const;
