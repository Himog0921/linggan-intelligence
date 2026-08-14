/**
 * Strict CaptureSubmissionV2 validator per 07-evidence-ingress-release-b-contract.md §5.3.
 *
 * Validation order is fixed:
 * 1. strict structure + basic value domain
 * 2. protocol/platform
 * 3. authority kind/structure
 * 4. base64/length/checksum/canonical JSON
 * 5. outer/inner header consistency
 * 6. target
 * 7. contract registry/hash/shape (handled by caller via registry)
 *
 * B1B02-R1-003: authority check MUST come before base64 decode, so that
 * an invalid authority returns its specific rejection even when the package
 * is also damaged. This is enforced by splitting validation into an envelope
 * phase (steps 1-3) and a package integrity phase (steps 4-6), with the
 * authority validator injected between them.
 *
 * All objects use strict shape: unknown keys → invalid_submission.
 */

import { assertJsonValue, canonicalJsonString } from "./canonical-json";
import { decodeStrictBase64, sha256Hex, isLowercaseHex64, parseIsoTimestamp } from "./base64";
import type {
  CaptureHeaderV2,
  CapturePackagePayloadV2,



  EvidenceIngressAuthorityV2,
  EvidenceIngressRejectReason,
  EvidencePlatformV2,
  IngressKindV2,
  RecordKindV2,
  ArtifactKindV2,
  CaptureTerminalStateV2,
  CaptureTerminalReasonV2,
  CaptureSlotStatusV2,
  RawRecordSubmissionV2,
  CaptureArtifactSubmissionV2,

} from "./types";

export type ValidationOutcome =
  | {
      ok: true;
      header: CaptureHeaderV2;
      packageBytes: Uint8Array;
      packagePayload: CapturePackagePayloadV2;
      records: RawRecordSubmissionV2[];
      artifacts: CaptureArtifactSubmissionV2[];
      authority: EvidenceIngressAuthorityV2;
      restricted: boolean;
    }
  | { ok: false; reason: EvidenceIngressRejectReason };

const PLATFORMS_V2: EvidencePlatformV2[] = ["xhs"];
const RECORD_KINDS_V2: RecordKindV2[] = ["note", "comment", "author", "metric"];
const ARTIFACT_KINDS_V2: ArtifactKindV2[] = [
  "platform_response",
  "page_snapshot",
  "dom_fragment",
  "media_inventory",
  "context",
];
const TERMINAL_STATES_V2: CaptureTerminalStateV2[] = ["completed", "blocked", "cancelled", "error"];
const TERMINAL_REASONS_V2: CaptureTerminalReasonV2[] = [
  "source_exhausted",
  "limit_reached",
  "target_missing",
  "login_required",
  "platform_blocked",
  "parser_failed",
  "network_failed",
  "user_cancelled",
];
const SLOT_STATUSES_V2: CaptureSlotStatusV2[] = ["observed", "absent", "unavailable", "not_applicable", "invalid"];
const INGRESS_KINDS_V2: IngressKindV2[] = ["execution", "manual_import", "recovery", "migration"];

// ── Strict shape helpers ───────────────────────────────────────────────────

function hasUnknownKeys(value: Record<string, unknown>, allowed: string[]): boolean {
  for (const key of Object.keys(value)) {
    if (!allowed.includes(key)) return true;
  }
  return false;
}

function isNonEmptyTrimmedString(value: unknown): value is string {
  return typeof value === "string" && value.trim() === value && value.length > 0;
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0;
}

function isPositiveInteger(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value > 0;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function checkStrictString(value: unknown, allowedValues?: string[]): value is string {
  if (typeof value !== "string") return false;
  if (allowedValues && !allowedValues.includes(value)) return false;
  return true;
}

// ── Main validation entry ──────────────────────────────────────────────────

/**
 * Phase 1: validate submission envelope (structure, protocol, authority shape).
 *
 * Does NOT decode the base64 package or verify checksums — those happen in
 * phase 2 (validateSubmission) or the caller's own package integrity step,
 * so that the authority validator can run before decoding.
 */
export type EnvelopeOutcome =
  | {
      ok: true;
      header: CaptureHeaderV2;
      authority: EvidenceIngressAuthorityV2;
      /** Raw body.capturePackage field (un-decoded). */
      capturePackage: {
        encoding: "base64";
        packagePayload: string;
        checksumAlgorithm: "sha256";
        checksumValue: string;
        contentLength: number;
        restricted: boolean;
      };
    }
  | { ok: false; reason: EvidenceIngressRejectReason };

export function validateSubmissionEnvelope(raw: unknown): EnvelopeOutcome {
  // ── Step 1: strict structure + basic value domain ──
  if (!isPlainObject(raw)) return reject("invalid_submission");
  if (hasUnknownKeys(raw, ["body", "authority"])) return reject("invalid_submission");

  const body = raw.body;
  const authority = raw.authority;
  if (!isPlainObject(body)) return reject("invalid_submission");
  if (!isPlainObject(authority)) return reject("invalid_submission");

  // Validate body structure
  if (hasUnknownKeys(body, ["header", "capturePackage"])) return reject("invalid_submission");
  const bodyHeader = body.header;
  const bodyPackage = body.capturePackage;
  if (!isPlainObject(bodyHeader)) return reject("invalid_submission");
  if (!isPlainObject(bodyPackage)) return reject("invalid_submission");

  // ── Validate header ──
  const headerResult = validateHeader(bodyHeader);
  if (!headerResult.ok) return headerResult;
  const header = headerResult.header;

  // ── Validate capturePackage metadata (not decode) ──
  if (hasUnknownKeys(bodyPackage, ["encoding", "packagePayload", "checksumAlgorithm", "checksumValue", "contentLength", "restricted"])) {
    return reject("invalid_submission");
  }
  if (bodyPackage.encoding !== "base64") return reject("invalid_submission");
  if (typeof bodyPackage.packagePayload !== "string") return reject("invalid_submission");
  if (bodyPackage.checksumAlgorithm !== "sha256") return reject("invalid_submission");
  if (typeof bodyPackage.checksumValue !== "string" || !isLowercaseHex64(bodyPackage.checksumValue)) {
    return reject("invalid_submission");
  }
  if (!isNonNegativeInteger(bodyPackage.contentLength)) return reject("invalid_submission");
  if (typeof bodyPackage.restricted !== "boolean") return reject("invalid_submission");

  // ── Step 3: authority kind/structure ──
  const authorityResult = validateAuthority(authority);
  if (!authorityResult.ok) return authorityResult;
  const validAuthority = authorityResult.authority;

  // Authority kind must match header kind
  if (validAuthority.ingressKind !== header.ingressKind) {
    return reject("invalid_submission");
  }

  return {
    ok: true,
    header,
    authority: validAuthority,
    capturePackage: {
      encoding: "base64",
      packagePayload: bodyPackage.packagePayload as string,
      checksumAlgorithm: "sha256",
      checksumValue: bodyPackage.checksumValue as string,
      contentLength: bodyPackage.contentLength as number,
      restricted: bodyPackage.restricted as boolean,
    },
  };
}

/**
 * Phase 2: decode the package payload and validate integrity.
 *
 * Call after authority check has passed.
 */
export type PackageIntegrityOutcome =
  | {
      ok: true;
      packageBytes: Uint8Array;
      packagePayload: CapturePackagePayloadV2;
      records: RawRecordSubmissionV2[];
      artifacts: CaptureArtifactSubmissionV2[];
    }
  | { ok: false; reason: EvidenceIngressRejectReason };

export function validatePackageIntegrity(
  header: CaptureHeaderV2,
  capturePackage: {
    encoding: "base64";
    packagePayload: string;
    checksumAlgorithm: "sha256";
    checksumValue: string;
    contentLength: number;
    restricted: boolean;
  },
): PackageIntegrityOutcome {
  // ── Step 4: base64/length/checksum/canonical JSON ──
  const packageBytes = decodeStrictBase64(capturePackage.packagePayload);
  if (!packageBytes) return reject("invalid_base64");

  // Length check
  if (packageBytes.length !== capturePackage.contentLength) return reject("package_length_mismatch");

  // Checksum check
  const computedChecksum = sha256Hex(packageBytes);
  if (computedChecksum !== capturePackage.checksumValue) return reject("package_checksum_mismatch");

  // Parse package payload
  let parsedPayload: unknown;
  try {
    parsedPayload = JSON.parse(Buffer.from(packageBytes).toString("utf8"));
    assertJsonValue(parsedPayload);
  } catch {
    return reject("invalid_submission");
  }

  // Validate package payload strict structure
  const payloadResult = validatePackagePayload(parsedPayload);
  if (!payloadResult.ok) return payloadResult;
  const packagePayload = payloadResult.payload;

  // Canonical check
  const regeneratedCanonical = canonicalJsonString(parsedPayload);
  const originalString = Buffer.from(packageBytes).toString("utf8");
  if (regeneratedCanonical !== originalString) return reject("package_not_canonical");

  // ── Step 5: outer/inner header consistency ──
  const outerCanonical = canonicalJsonString(header);
  const innerCanonical = canonicalJsonString(packagePayload.header);
  if (outerCanonical !== innerCanonical) return reject("header_package_mismatch");

  // Cross-field: counters.emitted must equal records.length (§3.3)
  if (header.report.counters.emitted !== packagePayload.records.length) {
    return reject("invalid_submission");
  }

  // Records empty rules (§3.3)
  const recordsEmpty = packagePayload.records.length === 0;
  if (header.report.terminal.state === "completed" && header.report.counters.emitted > 0 && recordsEmpty) {
    return reject("invalid_submission");
  }

  // Restricted propagation: if any artifact is restricted, package must be restricted (§3.3)
  for (const art of packagePayload.artifacts) {
    if (art.restricted && !capturePackage.restricted) {
      return reject("invalid_submission");
    }
  }

  // Validate each artifact payload (base64 + checksum + length)
  for (const art of packagePayload.artifacts) {
    const artBytes = decodeStrictBase64(art.artifactPayload);
    if (!artBytes) return reject("invalid_artifact_base64");

    if (artBytes.length !== art.contentLength) return reject("artifact_length_mismatch");

    const artChecksum = sha256Hex(artBytes);
    if (artChecksum !== art.artifactChecksum) return reject("artifact_checksum_mismatch");
  }

  // ── Step 6: target ──
  if (header.target.observedTargetKey !== null) {
    if (header.target.observedTargetKey.trim() !== header.target.expectedTargetKey) {
      return reject("target_identity_mismatch");
    }
  }

  return {
    ok: true,
    packageBytes,
    packagePayload,
    records: packagePayload.records,
    artifacts: packagePayload.artifacts,
  };
}

// ── Composite validator: envelope + package integrity (no authority validator) ──
//    Composes validateSubmissionEnvelope + validatePackageIntegrity for callers
//    that don't have an authority validator (e.g. unit tests of structure validation).

export function validateCaptureSubmission(raw: unknown): ValidationOutcome {
  const envelope = validateSubmissionEnvelope(raw);
  if (!envelope.ok) return envelope;

  const pkg = validatePackageIntegrity(envelope.header, envelope.capturePackage);
  if (!pkg.ok) return pkg;

  return {
    ok: true,
    header: envelope.header,
    packageBytes: pkg.packageBytes,
    packagePayload: pkg.packagePayload,
    records: pkg.records,
    artifacts: pkg.artifacts,
    authority: envelope.authority,
    restricted: envelope.capturePackage.restricted,
  };
}

export type CaptureSubmissionBodyValidation =
  | { ok: true; body: import("./types").CaptureSubmissionBodyV2 }
  | { ok: false; reason: EvidenceIngressRejectReason };

/**
 * Validate an untrusted route body before authentication or database access.
 *
 * Non-execution routes bind server authority only after authentication, so
 * their public boundary cannot use a TypeScript assertion to turn JSON into a
 * CaptureSubmissionBodyV2. This helper supplies a private synthetic authority
 * solely to reuse the strict envelope and package-integrity validators. The
 * synthetic values are never returned and can never reach persistence.
 */
export function validateCaptureSubmissionBody(
  raw: unknown,
  ingressKind: "manual_import" | "recovery",
): CaptureSubmissionBodyValidation {
  const syntheticAuthority = ingressKind === "manual_import"
    ? {
        ingressKind,
        workspaceId: "route-boundary",
        receivedAt: new Date(0),
        sourcePrincipal: "route-boundary",
        importerIdentity: "route-boundary",
      }
    : {
        ingressKind,
        workspaceId: "route-boundary",
        receivedAt: new Date(0),
        sourcePrincipal: "route-boundary",
        recoveryAuthorizedBy: "route-boundary",
      };
  const envelope = validateSubmissionEnvelope({
    body: raw,
    authority: syntheticAuthority,
  });
  if (!envelope.ok) return envelope;
  const integrity = validatePackageIntegrity(envelope.header, envelope.capturePackage);
  if (!integrity.ok) return integrity;
  return {
    ok: true,
    body: {
      header: envelope.header,
      capturePackage: envelope.capturePackage,
    },
  };
}

// ── Header validation ──────────────────────────────────────────────────────

type HeaderValidation =
  | { ok: true; header: CaptureHeaderV2 }
  | { ok: false; reason: EvidenceIngressRejectReason };

const HEADER_COMMON_KEYS = [
  "protocolVersion",
  "captureId",
  "platform",
  "target",
  "observedAt",
  "collectorVersion",
  "contractId",
  "contractVersion",
  "contractHash",
  "report",
];

function validateHeader(raw: unknown): HeaderValidation {
  if (!isPlainObject(raw)) return reject("invalid_submission");

  // Check common keys
  for (const key of HEADER_COMMON_KEYS) {
    if (!(key in raw)) return reject("invalid_submission");
  }

  // protocolVersion
  if (raw.protocolVersion !== "capture-submission/v2") return reject("invalid_submission");

  // platform — §2.3: only xhs is valid; douyin → unsupported_platform
  if (raw.platform === "douyin") return reject("unsupported_platform");
  if (!checkStrictString(raw.platform, PLATFORMS_V2)) return reject("invalid_submission");

  // captureId, collectorVersion, contractId — non-empty trimmed strings
  if (!isNonEmptyTrimmedString(raw.captureId)) return reject("invalid_submission");
  if (!isNonEmptyTrimmedString(raw.collectorVersion)) return reject("invalid_submission");
  if (!isNonEmptyTrimmedString(raw.contractId)) return reject("invalid_submission");

  // contractVersion — positive integer
  if (!isPositiveInteger(raw.contractVersion)) return reject("invalid_submission");

  // contractHash — lowercase 64 hex
  if (typeof raw.contractHash !== "string" || !isLowercaseHex64(raw.contractHash)) {
    return reject("invalid_submission");
  }

  // observedAt — valid ISO 8601 with timezone
  if (typeof raw.observedAt !== "string" || !parseIsoTimestamp(raw.observedAt)) {
    return reject("invalid_submission");
  }

  // target
  const target = raw.target;
  if (!isPlainObject(target)) return reject("invalid_submission");
  if (hasUnknownKeys(target, ["expectedTargetKey", "observedTargetKey"])) return reject("invalid_submission");
  if (!isNonEmptyTrimmedString(target.expectedTargetKey)) return reject("invalid_submission");
  // expectedTargetKey must start with "xhs:"
  if (!target.expectedTargetKey.startsWith("xhs:")) return reject("invalid_submission");
  if (target.observedTargetKey !== null && typeof target.observedTargetKey !== "string") {
    return reject("invalid_submission");
  }

  // report
  const reportResult = validateReport(raw.report);
  if (!reportResult.ok) return reportResult;

  // ingressKind + kind-specific fields
  const ingressKind = raw.ingressKind;
  if (!checkStrictString(ingressKind, INGRESS_KINDS_V2)) return reject("invalid_submission");

  if (ingressKind === "execution") {
    if (hasUnknownKeys(raw, [...HEADER_COMMON_KEYS, "ingressKind", "jobId", "attemptId", "leaseEpoch", "executionPlanVersion"])) {
      return reject("invalid_submission");
    }
    if (!isNonEmptyTrimmedString(raw.jobId)) return reject("invalid_submission");
    if (!isNonEmptyTrimmedString(raw.attemptId)) return reject("invalid_submission");
    if (!isNonNegativeInteger(raw.leaseEpoch)) return reject("invalid_submission");
    if (!isNonEmptyTrimmedString(raw.executionPlanVersion)) return reject("invalid_submission");
  } else if (ingressKind === "manual_import") {
    if (hasUnknownKeys(raw, [...HEADER_COMMON_KEYS, "ingressKind", "sourceSummary"])) {
      return reject("invalid_submission");
    }
    if (!isNonEmptyTrimmedString(raw.sourceSummary)) return reject("invalid_submission");
  } else if (ingressKind === "recovery") {
    if (hasUnknownKeys(raw, [...HEADER_COMMON_KEYS, "ingressKind", "recoveryCaptureId"])) {
      return reject("invalid_submission");
    }
    if (!isNonEmptyTrimmedString(raw.recoveryCaptureId)) return reject("invalid_submission");
  } else if (ingressKind === "migration") {
    if (hasUnknownKeys(raw, [...HEADER_COMMON_KEYS, "ingressKind", "sourceSummary"])) {
      return reject("invalid_submission");
    }
    if (!isNonEmptyTrimmedString(raw.sourceSummary)) return reject("invalid_submission");
  }

  // Build the typed header — all fields validated above
  const header = raw as unknown as CaptureHeaderV2;
  return { ok: true, header };
}

function validateReport(raw: unknown): { ok: true } | { ok: false; reason: EvidenceIngressRejectReason } {
  if (!isPlainObject(raw)) return reject("invalid_submission");
  if (hasUnknownKeys(raw, ["startedAt", "completedAt", "terminal", "slots", "counters", "diagnostics"])) {
    return reject("invalid_submission");
  }

  // Timestamps
  if (typeof raw.startedAt !== "string" || !parseIsoTimestamp(raw.startedAt)) return reject("invalid_submission");
  if (typeof raw.completedAt !== "string" || !parseIsoTimestamp(raw.completedAt)) return reject("invalid_submission");

  // terminal
  const terminal = raw.terminal;
  if (!isPlainObject(terminal)) return reject("invalid_submission");
  if (hasUnknownKeys(terminal, ["state", "reason", "retryable"])) return reject("invalid_submission");
  if (!checkStrictString(terminal.state, TERMINAL_STATES_V2)) return reject("invalid_submission");
  if (!checkStrictString(terminal.reason, TERMINAL_REASONS_V2)) return reject("invalid_submission");
  if (typeof terminal.retryable !== "boolean") return reject("invalid_submission");

  // counters
  const counters = raw.counters;
  if (!isPlainObject(counters)) return reject("invalid_submission");
  if (hasUnknownKeys(counters, ["requested", "discovered", "emitted", "deduplicated", "failed"])) {
    return reject("invalid_submission");
  }
  for (const key of ["requested", "discovered", "emitted", "deduplicated", "failed"]) {
    if (!isNonNegativeInteger(counters[key])) return reject("invalid_submission");
  }

  // slots — array of objects, slotId unique
  if (!Array.isArray(raw.slots)) return reject("invalid_submission");
  const slotIds = new Set<string>();
  for (const slot of raw.slots) {
    if (!isPlainObject(slot)) return reject("invalid_submission");
    if (hasUnknownKeys(slot, ["slotId", "status", "reason"])) return reject("invalid_submission");
    if (!isNonEmptyTrimmedString(slot.slotId)) return reject("invalid_submission");
    if (slotIds.has(slot.slotId)) return reject("invalid_submission");
    slotIds.add(slot.slotId);
    if (!checkStrictString(slot.status, SLOT_STATUSES_V2)) return reject("invalid_submission");
    if (slot.reason !== null && !isNonEmptyTrimmedString(slot.reason)) return reject("invalid_submission");
  }

  // diagnostics — must be valid JSON object
  if (!isPlainObject(raw.diagnostics)) return reject("invalid_submission");
  try {
    assertJsonValue(raw.diagnostics);
  } catch {
    return reject("invalid_submission");
  }

  return { ok: true };
}

// ── Authority validation ───────────────────────────────────────────────────

type AuthorityValidation =
  | { ok: true; authority: EvidenceIngressAuthorityV2 }
  | { ok: false; reason: EvidenceIngressRejectReason };

const AUTHORITY_COMMON_KEYS = ["ingressKind", "workspaceId", "receivedAt", "sourcePrincipal"];

function validateAuthority(raw: unknown): AuthorityValidation {
  if (!isPlainObject(raw)) return reject("invalid_submission");

  const ingressKind = raw.ingressKind;
  if (!checkStrictString(ingressKind, INGRESS_KINDS_V2)) return reject("invalid_submission");

  if (ingressKind === "execution") {
    if (hasUnknownKeys(raw, [...AUTHORITY_COMMON_KEYS, "stationId", "leaseToken"])) {
      return reject("invalid_submission");
    }
  } else if (ingressKind === "manual_import") {
    if (hasUnknownKeys(raw, [...AUTHORITY_COMMON_KEYS, "importerIdentity"])) {
      return reject("invalid_submission");
    }
  } else if (ingressKind === "recovery") {
    if (hasUnknownKeys(raw, [...AUTHORITY_COMMON_KEYS, "recoveryAuthorizedBy"])) {
      return reject("invalid_submission");
    }
  } else if (ingressKind === "migration") {
    if (hasUnknownKeys(raw, [...AUTHORITY_COMMON_KEYS, "migrationAuthorization"])) {
      return reject("invalid_submission");
    }
  }

  if (!isNonEmptyTrimmedString(raw.workspaceId)) return reject("invalid_submission");
  if (!isNonEmptyTrimmedString(raw.sourcePrincipal)) return reject("invalid_submission");

  // receivedAt must be a Date
  if (!(raw.receivedAt instanceof Date) || isNaN(raw.receivedAt.getTime())) {
    return reject("invalid_submission");
  }

  if (ingressKind === "execution") {
    if (!isNonEmptyTrimmedString(raw.stationId)) return reject("invalid_submission");
    if (!isNonEmptyTrimmedString(raw.leaseToken)) return reject("invalid_submission");
  } else if (ingressKind === "manual_import") {
    if (!isNonEmptyTrimmedString(raw.importerIdentity)) return reject("invalid_submission");
  } else if (ingressKind === "recovery") {
    if (!isNonEmptyTrimmedString(raw.recoveryAuthorizedBy)) return reject("invalid_submission");
  } else if (ingressKind === "migration") {
    if (!isNonEmptyTrimmedString(raw.migrationAuthorization)) return reject("invalid_submission");
  }

  const authority = raw as unknown as EvidenceIngressAuthorityV2;
  return { ok: true, authority };
}

// ── Package payload validation ─────────────────────────────────────────────

type PayloadValidation =
  | { ok: true; payload: CapturePackagePayloadV2 }
  | { ok: false; reason: EvidenceIngressRejectReason };

function validatePackagePayload(raw: unknown): PayloadValidation {
  if (!isPlainObject(raw)) return reject("invalid_submission");
  if (hasUnknownKeys(raw, ["schemaVersion", "header", "records", "artifacts"])) {
    return reject("invalid_submission");
  }
  if (raw.schemaVersion !== "capture-package/v2") return reject("invalid_submission");

  // header — re-validate same as outer header
  const headerResult = validateHeader(raw.header);
  if (!headerResult.ok) return headerResult;

  // records — strict array
  if (!Array.isArray(raw.records)) return reject("invalid_submission");
  const sequences = new Set<number>();
  const idempotencyKeys = new Set<string>();
  for (const rec of raw.records) {
    const recResult = validateRecord(rec);
    if (!recResult.ok) return recResult;

    // sequence uniqueness
    if (sequences.has(rec.sequence)) return reject("invalid_submission");
    sequences.add(rec.sequence);

    // idempotencyKey uniqueness
    if (idempotencyKeys.has(rec.idempotencyKey)) return reject("invalid_submission");
    idempotencyKeys.add(rec.idempotencyKey);
  }

  // artifacts — strict array
  if (!Array.isArray(raw.artifacts)) return reject("invalid_submission");
  for (const art of raw.artifacts) {
    const artResult = validateArtifact(art);
    if (!artResult.ok) return artResult;
  }

  return { ok: true, payload: raw as CapturePackagePayloadV2 };
}

function validateRecord(raw: unknown): { ok: true } | { ok: false; reason: EvidenceIngressRejectReason } {
  if (!isPlainObject(raw)) return reject("invalid_submission");
  if (hasUnknownKeys(raw, ["idempotencyKey", "recordKind", "platform", "targetKey", "externalRecordId", "sequence", "payload", "observedAt"])) {
    return reject("invalid_submission");
  }
  // Client must NOT submit payloadHash (§3.2)
  if ("payloadHash" in raw) return reject("invalid_submission");

  if (!isNonEmptyTrimmedString(raw.idempotencyKey)) return reject("invalid_submission");
  if (!checkStrictString(raw.recordKind, RECORD_KINDS_V2)) return reject("invalid_submission");
  if (!checkStrictString(raw.platform, PLATFORMS_V2)) return reject("invalid_submission");
  if (raw.targetKey !== null) {
    if (!isNonEmptyTrimmedString(raw.targetKey)) return reject("invalid_submission");
    if (!raw.targetKey.startsWith("xhs:")) return reject("invalid_submission");
  }
  if (raw.externalRecordId !== null) {
    if (!isNonEmptyTrimmedString(raw.externalRecordId)) return reject("invalid_submission");
  }
  if (!isNonNegativeInteger(raw.sequence)) return reject("invalid_submission");
  if (typeof raw.observedAt !== "string" || !parseIsoTimestamp(raw.observedAt)) return reject("invalid_submission");

  // payload — must be valid JSON
  try {
    assertJsonValue(raw.payload);
  } catch {
    return reject("invalid_submission");
  }

  return { ok: true };
}

function validateArtifact(raw: unknown): { ok: true } | { ok: false; reason: EvidenceIngressRejectReason } {
  if (!isPlainObject(raw)) return reject("invalid_submission");
  if (hasUnknownKeys(raw, ["kind", "encoding", "artifactPayload", "artifactChecksum", "contentLength", "restricted"])) {
    return reject("invalid_submission");
  }
  if (!checkStrictString(raw.kind, ARTIFACT_KINDS_V2)) return reject("invalid_submission");
  if (raw.encoding !== "base64") return reject("invalid_submission");
  if (typeof raw.artifactPayload !== "string") return reject("invalid_submission");
  if (typeof raw.artifactChecksum !== "string" || !isLowercaseHex64(raw.artifactChecksum)) {
    return reject("invalid_submission");
  }
  if (!isNonNegativeInteger(raw.contentLength)) return reject("invalid_submission");
  if (typeof raw.restricted !== "boolean") return reject("invalid_submission");

  return { ok: true };
}

// ── Helpers ────────────────────────────────────────────────────────────────

function reject(reason: EvidenceIngressRejectReason): { ok: false; reason: EvidenceIngressRejectReason } {
  return { ok: false, reason };
}
