import { computeContractHash } from "../contracts/collection-contract-registry";
import { XHS_COLLECTION_CONTRACTS } from "../contracts/xhs-collection-contracts";
import { sha256Hex } from "../ingress/base64";
import {
  assertJsonValue,
  canonicalJsonString,
  type JsonValue,
} from "../ingress/canonical-json";
import type {
  CapturePackagePayloadV2,
  CaptureSlotV2,
  CaptureTerminalV2,
  CollectionContractDefinitionV2,
} from "../ingress/types";
import { B2SourceInvariantError } from "./b2-derived-errors";
import type { XhsRawRecordFact } from "./xhs-derived-contract";

type AuditedPackageEvidence = {
  packageBytes: Uint8Array;
  packageChecksumAlgorithm: string;
  packageChecksumValue: string;
  packageContentLength: number;
};

export type B2SourceSnapshot = {
  id: string;
  workspaceId: string;
  captureId: string;
  platform: string;
  integrityStatus: string | null;
  checksumAlgorithm: string | null;
  checksumValue: string | null;
  contentLength: number | null;
  contractId: string | null;
  contractVersion: number | null;
  contractHash: string | null;
  records: RawRecordRow[];
};

type RawRecordRow = {
  id: string;
  workspaceId: string;
  rawSnapshotId: string;
  recordKind: string | null;
  platform: string;
  targetKey: string | null;
  externalRecordId: string | null;
  sequence: number | null;
  payloadHash: string | null;
  observedAt: Date;
  idempotencyKey: string;
};

export function verifyB2Source(
  snapshot: B2SourceSnapshot,
  evidence: AuditedPackageEvidence,
): {
  capturePackage: CapturePackagePayloadV2;
  contract: CollectionContractDefinitionV2;
  records: XhsRawRecordFact[];
} {
  const capturePackage = readAndVerifyCapturePackage(snapshot, evidence);
  const contract = resolveContract(snapshot.contractId, snapshot.contractVersion);
  assertHeaderBinding(snapshot, capturePackage, contract);
  const records = verifyAndSnapshotRecords(
    snapshot.workspaceId,
    snapshot.id,
    snapshot.records,
    capturePackage.records,
  );
  return { capturePackage, contract, records };
}

function readAndVerifyCapturePackage(
  snapshot: B2SourceSnapshot,
  evidence: AuditedPackageEvidence,
): CapturePackagePayloadV2 {
  const bytes = new Uint8Array(evidence.packageBytes);
  const checksum = sha256Hex(bytes);
  if (snapshot.checksumAlgorithm !== "sha256" ||
      evidence.packageChecksumAlgorithm !== "sha256" ||
      snapshot.checksumValue !== checksum ||
      evidence.packageChecksumValue !== checksum ||
      snapshot.contentLength !== bytes.length ||
      evidence.packageContentLength !== bytes.length) {
    throw new B2SourceInvariantError("CapturePackage checksum/length binding changed.");
  }

  const text = Buffer.from(bytes).toString("utf8");
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
    assertJsonValue(parsed);
  } catch {
    throw new B2SourceInvariantError("CapturePackage is not valid canonical JSON.");
  }
  if (canonicalJsonString(parsed) !== text || !isCapturePackagePayload(parsed)) {
    throw new B2SourceInvariantError("CapturePackage shape or canonical bytes changed.");
  }
  return structuredClone(parsed);
}

function resolveContract(
  contractId: string | null,
  contractVersion: number | null,
): CollectionContractDefinitionV2 {
  const contract = XHS_COLLECTION_CONTRACTS.find(
    (candidate) => candidate.id === contractId && candidate.version === contractVersion,
  );
  if (!contract) {
    throw new B2SourceInvariantError(
      `RawSnapshot contract is not registered for B2: ${String(contractId)}@${String(contractVersion)}.`,
    );
  }
  return contract;
}

function assertHeaderBinding(
  snapshot: B2SourceSnapshot,
  capturePackage: CapturePackagePayloadV2,
  contract: CollectionContractDefinitionV2,
): void {
  const header = capturePackage.header;
  if (header.captureId !== snapshot.captureId ||
      header.platform !== snapshot.platform ||
      header.platform !== "xhs" ||
      header.contractId !== contract.id ||
      header.contractVersion !== contract.version ||
      header.contractHash !== computeContractHash(contract) ||
      snapshot.contractHash !== header.contractHash) {
    throw new B2SourceInvariantError("CapturePackage header no longer matches RawSnapshot.");
  }
}

function verifyAndSnapshotRecords(
  workspaceId: string,
  rawSnapshotId: string,
  rows: RawRecordRow[],
  packageRecords: CapturePackagePayloadV2["records"],
): XhsRawRecordFact[] {
  if (rows.length !== packageRecords.length) {
    throw new B2SourceInvariantError("RawRecord count no longer matches CapturePackage.");
  }
  const packageByIdempotencyKey = new Map(
    packageRecords.map((record) => [record.idempotencyKey, record]),
  );
  if (packageByIdempotencyKey.size !== packageRecords.length) {
    throw new B2SourceInvariantError("CapturePackage contains duplicate record identity.");
  }

  return rows.map((row) => {
    const packaged = packageByIdempotencyKey.get(row.idempotencyKey);
    const packagedPayloadHash = packaged === undefined
      ? null
      : sha256Hex(Buffer.from(canonicalJsonString(packaged.payload), "utf8"));
    if (!packaged ||
        row.workspaceId !== workspaceId ||
        row.rawSnapshotId !== rawSnapshotId ||
        row.recordKind !== packaged.recordKind ||
        row.platform !== packaged.platform ||
        row.targetKey !== packaged.targetKey ||
        row.externalRecordId !== packaged.externalRecordId ||
        row.sequence !== packaged.sequence ||
        row.observedAt.toISOString() !== new Date(packaged.observedAt).toISOString() ||
        row.payloadHash === null ||
        row.payloadHash !== packagedPayloadHash) {
      throw new B2SourceInvariantError(`RawRecord ${row.id} no longer matches CapturePackage.`);
    }
    return {
      id: row.id,
      workspaceId: row.workspaceId,
      rawSnapshotId: row.rawSnapshotId,
      recordKind: row.recordKind,
      platform: row.platform,
      sequence: row.sequence,
      payload: structuredClone(packaged.payload) as JsonValue,
      payloadHash: row.payloadHash,
      observedAt: new Date(row.observedAt.getTime()),
    };
  }).sort(compareRawRecords);
}

function isCapturePackagePayload(value: JsonValue): value is CapturePackagePayloadV2 {
  if (!isJsonObject(value) || value.schemaVersion !== "capture-package/v2") return false;
  if (!isJsonObject(value.header) || !Array.isArray(value.records) || !Array.isArray(value.artifacts)) {
    return false;
  }
  const header = value.header;
  if (typeof header.captureId !== "string" ||
      header.platform !== "xhs" ||
      typeof header.contractId !== "string" ||
      !Number.isInteger(header.contractVersion) ||
      !isJsonObject(header.report)) {
    return false;
  }
  const report = header.report;
  if (!isCaptureTerminal(report.terminal) ||
      !Array.isArray(report.slots) ||
      !report.slots.every(isCaptureSlot)) {
    return false;
  }
  return value.records.every(isPackageRecord);
}

function isCaptureTerminal(value: JsonValue | undefined): value is CaptureTerminalV2 {
  return isJsonObject(value) &&
    (value.state === "completed" || value.state === "blocked" || value.state === "cancelled" || value.state === "error") &&
    (value.reason === "source_exhausted" || value.reason === "limit_reached" || value.reason === "target_missing" ||
      value.reason === "login_required" || value.reason === "platform_blocked" || value.reason === "parser_failed" ||
      value.reason === "network_failed" || value.reason === "user_cancelled") &&
    typeof value.retryable === "boolean";
}

function isCaptureSlot(value: JsonValue): value is CaptureSlotV2 {
  return isJsonObject(value) &&
    typeof value.slotId === "string" &&
    (value.status === "observed" || value.status === "absent" || value.status === "unavailable" ||
      value.status === "not_applicable" || value.status === "invalid") &&
    (value.reason === null || typeof value.reason === "string");
}

function isPackageRecord(value: JsonValue): value is CapturePackagePayloadV2["records"][number] {
  return isJsonObject(value) &&
    typeof value.idempotencyKey === "string" &&
    (value.recordKind === "note" || value.recordKind === "comment" || value.recordKind === "author" || value.recordKind === "metric") &&
    value.platform === "xhs" &&
    (value.targetKey === null || typeof value.targetKey === "string") &&
    (value.externalRecordId === null || typeof value.externalRecordId === "string") &&
    Number.isInteger(value.sequence) &&
    value.payload !== undefined &&
    typeof value.observedAt === "string" &&
    Number.isFinite(new Date(value.observedAt).getTime());
}

function isJsonObject(value: JsonValue | undefined): value is { [key: string]: JsonValue } {
  return value !== null && value !== undefined && typeof value === "object" && !Array.isArray(value);
}

function compareRawRecords(left: XhsRawRecordFact, right: XhsRawRecordFact): number {
  return compareSequenceThenId(left.sequence, left.id, right.sequence, right.id);
}

function compareSequenceThenId(
  leftSequence: number | null,
  leftId: string,
  rightSequence: number | null,
  rightId: string,
): number {
  if (leftSequence !== null) {
    if (rightSequence === null) return -1;
    if (leftSequence !== rightSequence) return leftSequence - rightSequence;
  } else if (rightSequence !== null) {
    return 1;
  }
  return leftId.localeCompare(rightId);
}
