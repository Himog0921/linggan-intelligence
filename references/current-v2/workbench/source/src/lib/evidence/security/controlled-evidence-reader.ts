/**
 * Controlled Evidence Reader — the only production CapturePackage reader.
 * PostgreSQL binds the audit identity to session_user and checks the active
 * workspace grant inside the SECURITY DEFINER function; callers cannot supply
 * or forge accessedBy.
 */
import { Prisma, type PrismaClient } from "@/lib/prisma-client";
import type {
  EvidenceAnalysisAuditReceipt,
  EvidenceAnalysisAuditSource,
  EvidenceLifecycleStatus,
} from "../derived/b2-derived-service";
import type {
  CaptureArtifactRegistryFact,
  EvidenceArtifactAuditReceipt,
  EvidenceArtifactAuditSource,
} from "../media/verified-artifact-reader";

type ControlledReaderDB = Pick<PrismaClient, "$queryRaw">;

type ControlledReadRow = {
  workspace_id: string;
  raw_snapshot_id: string;
  access_audit_id: string;
  lifecycle_status: string;
  integrity_status: string;
  package_bytes: Uint8Array;
  checksum_algorithm: string;
  checksum_value: string;
  content_length: number;
  restricted: boolean;
  capture_package_id: string;
  snapshot_metadata: unknown;
  artifact_descriptors: unknown;
};

export class ControlledEvidenceReader implements EvidenceAnalysisAuditSource, EvidenceArtifactAuditSource {
  constructor(private readonly db: ControlledReaderDB) {}

  async readCapturePackageAndAudit(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<EvidenceAnalysisAuditReceipt> {
    const workspaceId = input.workspaceId.trim();
    const rawSnapshotId = input.rawSnapshotId.trim();
    if (!workspaceId || !rawSnapshotId) {
      throw new EvidenceReaderError("INVALID_IDENTITY", "workspaceId and rawSnapshotId are required.");
    }
    const row = await this.readControlled(workspaceId, rawSnapshotId, "b2_normalization");
    return this.analysisReceipt(row, workspaceId, rawSnapshotId);
  }

  async readCaptureArtifactsAndAudit(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<EvidenceArtifactAuditReceipt> {
    const workspaceId = input.workspaceId.trim();
    const rawSnapshotId = input.rawSnapshotId.trim();
    if (!workspaceId || !rawSnapshotId) {
      throw new EvidenceReaderError("INVALID_IDENTITY", "workspaceId and rawSnapshotId are required.");
    }
    const row = await this.readControlled(workspaceId, rawSnapshotId, "artifact_verification");
    const analysis = this.analysisReceipt(row, workspaceId, rawSnapshotId);
    return {
      ...analysis,
      capturePackageId: requiredString(row.capture_package_id, "capturePackageId"),
      snapshot: parseSnapshot(row.snapshot_metadata),
      artifacts: parseArtifacts(row.artifact_descriptors),
    };
  }

  private async readControlled(
    workspaceId: string,
    rawSnapshotId: string,
    accessReason: "b2_normalization" | "artifact_verification",
  ): Promise<ControlledReadRow> {
    const rows = await this.db.$queryRaw<ControlledReadRow[]>(Prisma.sql`
      SELECT *
      FROM evidence_private.read_capture_package_and_audit(
        ${workspaceId},
        ${rawSnapshotId},
        ${accessReason},
        NULL
      )
    `);
    const row = rows[0];
    if (!row || row.workspace_id !== workspaceId || row.raw_snapshot_id !== rawSnapshotId) {
      throw new EvidenceReaderError("CONTROLLED_READ_MISMATCH", "Controlled reader returned a mismatched fact.");
    }
    if (!row.access_audit_id || !(row.package_bytes instanceof Uint8Array)) {
      throw new EvidenceReaderError("CONTROLLED_READ_INCOMPLETE", "Controlled reader returned incomplete evidence.");
    }
    return row;
  }

  private analysisReceipt(
    row: ControlledReadRow,
    workspaceId: string,
    rawSnapshotId: string,
  ): EvidenceAnalysisAuditReceipt {
    const lifecycleStatuses: EvidenceLifecycleStatus[] = [
      "ACTIVE",
      "ARCHIVED",
      "REDACTED",
      "PURGED",
    ];
    const lifecycleStatus = lifecycleStatuses.find(
      (candidate) => candidate === row.lifecycle_status,
    );
    if (!lifecycleStatus) {
      throw new EvidenceReaderError("CONTROLLED_READ_INVALID_STATUS", "Controlled reader returned an invalid lifecycle status.");
    }
    return {
      workspaceId,
      rawSnapshotId,
      accessAuditId: row.access_audit_id,
      lifecycleStatus,
      integrityStatus: row.integrity_status,
      packageBytes: new Uint8Array(row.package_bytes),
      packageChecksumAlgorithm: row.checksum_algorithm,
      packageChecksumValue: row.checksum_value,
      packageContentLength: row.content_length,
    };
  }
}

function parseSnapshot(value: unknown): EvidenceArtifactAuditReceipt["snapshot"] {
  const row = requiredObject(value, "snapshotMetadata");
  const records = requiredArray(row.records, "snapshotMetadata.records").map((entry, index) => {
    const record = requiredObject(entry, `snapshotMetadata.records[${index}]`);
    const observedAtText = requiredString(record.observedAt, "record.observedAt");
    // PostgreSQL returns `timestamp without time zone` values inside jsonb
    // without an offset. Evidence timestamps are stored as UTC by contract;
    // parsing the bare value as local time shifts the fact on non-UTC hosts.
    const observedAt = new Date(
      /(?:Z|[+-]\d{2}:?\d{2})$/i.test(observedAtText)
        ? observedAtText
        : `${observedAtText}Z`,
    );
    if (!Number.isFinite(observedAt.getTime())) {
      throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", "record.observedAt is invalid.");
    }
    return {
      id: requiredString(record.id, "record.id"),
      workspaceId: requiredString(record.workspaceId, "record.workspaceId"),
      rawSnapshotId: requiredString(record.rawSnapshotId, "record.rawSnapshotId"),
      recordKind: nullableString(record.recordKind, "record.recordKind"),
      platform: requiredString(record.platform, "record.platform"),
      targetKey: nullableString(record.targetKey, "record.targetKey"),
      externalRecordId: nullableString(record.externalRecordId, "record.externalRecordId"),
      sequence: nullableInteger(record.sequence, "record.sequence"),
      payloadHash: nullableString(record.payloadHash, "record.payloadHash"),
      observedAt,
      idempotencyKey: requiredString(record.idempotencyKey, "record.idempotencyKey"),
    };
  });
  return {
    id: requiredString(row.id, "snapshot.id"),
    workspaceId: requiredString(row.workspaceId, "snapshot.workspaceId"),
    captureId: requiredString(row.captureId, "snapshot.captureId"),
    platform: requiredString(row.platform, "snapshot.platform"),
    integrityStatus: nullableString(row.integrityStatus, "snapshot.integrityStatus"),
    checksumAlgorithm: nullableString(row.checksumAlgorithm, "snapshot.checksumAlgorithm"),
    checksumValue: nullableString(row.checksumValue, "snapshot.checksumValue"),
    contentLength: nullableInteger(row.contentLength, "snapshot.contentLength"),
    contractId: nullableString(row.contractId, "snapshot.contractId"),
    contractVersion: nullableInteger(row.contractVersion, "snapshot.contractVersion"),
    contractHash: nullableString(row.contractHash, "snapshot.contractHash"),
    capturePackageId: nullableString(row.capturePackageId, "snapshot.capturePackageId"),
    records,
  };
}

function parseArtifacts(value: unknown): CaptureArtifactRegistryFact[] {
  return requiredArray(value, "artifactDescriptors").map((entry, index) => {
    const artifact = requiredObject(entry, `artifactDescriptors[${index}]`);
    if (typeof artifact.restricted !== "boolean") {
      throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", "artifact.restricted is invalid.");
    }
    return {
      id: requiredString(artifact.id, "artifact.id"),
      workspaceId: requiredString(artifact.workspaceId, "artifact.workspaceId"),
      rawSnapshotId: requiredString(artifact.rawSnapshotId, "artifact.rawSnapshotId"),
      kind: requiredString(artifact.kind, "artifact.kind"),
      artifactChecksum: requiredString(artifact.artifactChecksum, "artifact.artifactChecksum"),
      restricted: artifact.restricted,
    };
  });
}

function requiredObject(value: unknown, label: string): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", `${label} is invalid.`);
  }
  return value as Record<string, unknown>;
}

function requiredArray(value: unknown, label: string): unknown[] {
  if (!Array.isArray(value)) {
    throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", `${label} is invalid.`);
  }
  return value;
}

function requiredString(value: unknown, label: string): string {
  if (typeof value !== "string" || !value) {
    throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", `${label} is invalid.`);
  }
  return value;
}

function nullableString(value: unknown, label: string): string | null {
  if (value === null) return null;
  if (typeof value !== "string") {
    throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", `${label} is invalid.`);
  }
  return value;
}

function nullableInteger(value: unknown, label: string): number | null {
  if (value === null) return null;
  if (typeof value !== "number" || !Number.isSafeInteger(value)) {
    throw new EvidenceReaderError("CONTROLLED_READ_INVALID_METADATA", `${label} is invalid.`);
  }
  return value;
}

export class EvidenceReaderError extends Error {
  constructor(
    public readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "EvidenceReaderError";
  }
}
