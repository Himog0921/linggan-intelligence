/**
 * Test fixtures for EvidenceIngress unit tests.
 * Builds valid CaptureSubmissionV2 objects for each ingress kind.
 */

import { canonicalJsonString, type JsonValue } from "./canonical-json";
import { sha256Hex } from "./base64";
import type {
  CaptureSubmissionV2,
  CaptureHeaderV2,
  CapturePackagePayloadV2,
  CapturePackageSubmissionV2,
  CaptureSubmissionBodyV2,
  EvidenceIngressAuthorityV2,
  CollectionContractDefinitionV2,
} from "./types";

import { CollectionContractRegistry } from "../contracts/collection-contract-registry";
import { type EvidenceIngressAuthorityValidator, type AuthorityValidationResult } from "./authority-validator";
import type { EvidenceIngressRejectReason } from "./types";

export const TEST_CONTRACT: CollectionContractDefinitionV2 = {
  id: "xhs-note-detail-v1",
  version: 1,
  platforms: ["xhs"],
  recordKinds: ["note", "comment", "author", "metric"],
  slots: [
    { slotId: "note_content", requirement: "required" },
    { slotId: "cover_image", requirement: "optional" },
  ],
  terminalPolicy: {
    allowedStates: ["completed", "blocked", "cancelled", "error"],
    allowEmptyRecords: true,
  },
  mediaPolicy: "metadata_only",
};

export function testRegistry(
  defs: CollectionContractDefinitionV2[] = [TEST_CONTRACT],
): CollectionContractRegistry {
  return new CollectionContractRegistry(defs);
}

function computeContractHash(def: CollectionContractDefinitionV2): string {
  return sha256Hex(Buffer.from(canonicalJsonString(def), "utf8"));
}

export const TEST_CONTRACT_HASH = computeContractHash(TEST_CONTRACT);

type HeaderKind = CaptureHeaderV2["ingressKind"];

function commonHeaderFields(kind: HeaderKind) {
  return {
    protocolVersion: "capture-submission/v2" as const,
    captureId: "cap-001",
    platform: "xhs" as const,
    target: { expectedTargetKey: "xhs:note/abc123", observedTargetKey: "xhs:note/abc123" },
    observedAt: "2026-08-05T12:00:00Z",
    collectorVersion: "1.0.0",
    contractId: TEST_CONTRACT.id,
    contractVersion: TEST_CONTRACT.version,
    contractHash: TEST_CONTRACT_HASH,
    report: {
      startedAt: "2026-08-05T11:55:00Z",
      completedAt: "2026-08-05T12:00:00Z",
      terminal: { state: "completed" as const, reason: "limit_reached" as const, retryable: false },
      slots: [
        { slotId: "note_content", status: "observed" as const, reason: null as string | null },
        { slotId: "cover_image", status: "absent" as const, reason: "no_cover" as string | null },
      ],
      counters: { requested: 1, discovered: 1, emitted: 1, deduplicated: 0, failed: 0 },
      diagnostics: {} as Record<string, unknown>,
    },
    ingressKind: kind,
  };
}

export function buildBaseHeader(kind: HeaderKind = "manual_import"): CaptureHeaderV2 {
  const common = commonHeaderFields(kind);
  switch (kind) {
    case "execution":
      return {
        ...common,
        ingressKind: "execution" as const,
        jobId: "job-001",
        attemptId: "att-001",
        leaseEpoch: 1,
        executionPlanVersion: "plan-v1",
      } as CaptureHeaderV2;
    case "manual_import":
      return {
        ...common,
        ingressKind: "manual_import" as const,
        sourceSummary: "manual import test",
      } as CaptureHeaderV2;
    case "recovery":
      return {
        ...common,
        ingressKind: "recovery" as const,
        recoveryCaptureId: "rec-cap-001",
      } as CaptureHeaderV2;
    case "migration":
      return {
        ...common,
        ingressKind: "migration" as const,
        sourceSummary: "migration source",
      } as CaptureHeaderV2;
  }
}

export function buildBaseHeaderManualImport(): Extract<CaptureHeaderV2, { ingressKind: "manual_import" }> {
  return buildBaseHeader("manual_import") as Extract<CaptureHeaderV2, { ingressKind: "manual_import" }>;
}

export function buildPackagePayload(
  header: CaptureHeaderV2,
  records: CapturePackagePayloadV2["records"] = [],
  artifacts: CapturePackagePayloadV2["artifacts"] = [],
): { payload: CapturePackagePayloadV2; bytes: Uint8Array } {
  const payload: CapturePackagePayloadV2 = {
    schemaVersion: "capture-package/v2",
    header,
    records,
    artifacts,
  };
  const jsonStr = canonicalJsonString(payload);
  const bytes = Buffer.from(jsonStr, "utf8");
  return { payload, bytes };
}

export function buildPackageSubmission(
  header: CaptureHeaderV2,
  records: CapturePackagePayloadV2["records"] = [],
  artifacts: CapturePackagePayloadV2["artifacts"] = [],
): CapturePackageSubmissionV2 {
  const { bytes } = buildPackagePayload(header, records, artifacts);
  return {
    encoding: "base64",
    packagePayload: Buffer.from(bytes).toString("base64"),
    checksumAlgorithm: "sha256",
    checksumValue: sha256Hex(bytes),
    contentLength: bytes.length,
    restricted: false,
  };
}

export function buildBody(
  header: CaptureHeaderV2,
  records: CapturePackagePayloadV2["records"] = [],
  artifacts: CapturePackagePayloadV2["artifacts"] = [],
): CaptureSubmissionBodyV2 {
  return {
    header,
    capturePackage: buildPackageSubmission(header, records, artifacts),
  };
}

export function buildAuthority(
  kind: EvidenceIngressAuthorityV2["ingressKind"] = "manual_import",
  overrides: Record<string, unknown> = {},
): EvidenceIngressAuthorityV2 {
  const base: Record<string, unknown> = {
    workspaceId: "ws-test-001",
    receivedAt: new Date("2026-08-05T12:00:01Z"),
    sourcePrincipal: "user-test@example.com",
  };

  switch (kind) {
    case "execution":
      return {
        ingressKind: "execution",
        ...base,
        stationId: "station-001",
        leaseToken: "lease-token-abc",
        ...overrides,
      } as EvidenceIngressAuthorityV2;
    case "manual_import":
      return {
        ingressKind: "manual_import",
        ...base,
        importerIdentity: "importer-001",
        ...overrides,
      } as EvidenceIngressAuthorityV2;
    case "recovery":
      return {
        ingressKind: "recovery",
        ...base,
        recoveryAuthorizedBy: "admin-001",
        ...overrides,
      } as EvidenceIngressAuthorityV2;
    case "migration":
      return {
        ingressKind: "migration",
        ...base,
        migrationAuthorization: "migration-auth-001",
        ...overrides,
      } as EvidenceIngressAuthorityV2;
  }
}

export function buildSubmission(
  kind: EvidenceIngressAuthorityV2["ingressKind"] = "manual_import",
  options: {
    headerOverrides?: Partial<CaptureHeaderV2>;
    records?: CapturePackagePayloadV2["records"];
    artifacts?: CapturePackagePayloadV2["artifacts"];
    authorityOverrides?: Record<string, unknown>;
  } = {},
): CaptureSubmissionV2 {
  let header = buildBaseHeader(kind);

  // Apply overrides on top of the kind-correct base header
  if (options.headerOverrides) {
    header = { ...header, ...options.headerOverrides } as CaptureHeaderV2;
  }

  // Sync counters.emitted with actual records length
  const emitted = options.records?.length ?? 0;
  if (header.report.counters.emitted !== emitted) {
    header = {
      ...header,
      report: {
        ...header.report,
        counters: { ...header.report.counters, emitted },
      },
    } as CaptureHeaderV2;
  }

  return {
    body: buildBody(header, options.records, options.artifacts),
    authority: buildAuthority(kind, options.authorityOverrides),
  };
}

export function buildTestRecord(
  overrides: Partial<{
    idempotencyKey: string;
    recordKind: "note" | "comment" | "author" | "metric";
    sequence: number;
    payload: JsonValue;
    targetKey: string | null;
    externalRecordId: string | null;
    observedAt: string;
  }> = {},
): CapturePackagePayloadV2["records"][0] {
  return {
    idempotencyKey: overrides.idempotencyKey ?? "rec-key-001",
    recordKind: overrides.recordKind ?? "note",
    platform: "xhs",
    targetKey: overrides.targetKey ?? "xhs:note/abc123",
    externalRecordId: overrides.externalRecordId ?? "ext-001",
    sequence: overrides.sequence ?? 0,
    payload: overrides.payload ?? { title: "test note", body: "content" },
    observedAt: overrides.observedAt ?? "2026-08-05T11:58:00Z",
  };
}

// ── Test-only authority validators (§3.4, moved here per B1B02-R1-004) ────

/**
 * Test validator that always approves. For unit tests only — never use in
 * production paths.
 */
export class AlwaysValidAuthorityValidator implements EvidenceIngressAuthorityValidator {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  validate(_authority: EvidenceIngressAuthorityV2, _header: CaptureHeaderV2): AuthorityValidationResult {
    return { valid: true };
  }
}

/**
 * Test validator that always rejects with a given reason. For unit tests only.
 */
export class AlwaysInvalidAuthorityValidator implements EvidenceIngressAuthorityValidator {
  constructor(private reason: EvidenceIngressRejectReason) {}

  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  validate(_authority: EvidenceIngressAuthorityV2, _header: CaptureHeaderV2): AuthorityValidationResult {
    return { valid: false, reason: this.reason };
  }
}
