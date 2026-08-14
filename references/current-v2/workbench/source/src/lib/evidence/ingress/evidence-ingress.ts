/**
 * EvidenceIngress service per 07-evidence-ingress-release-b-contract.md §5–6.
 *
 * Single SERIALIZABLE transaction atomically writes:
 *   CapturePackage → RawSnapshot → RawRecord[] → CaptureArtifact[] → EvidenceIngressReceipt
 *
 * Does NOT advance ExecutionJob/QueueEntry/TaskAttempt/Runtime.
 * Does not emit any legacy snapshot event.
 * Does NOT create Normalization/ContractEvaluation/Projection/Media.
 */

import type { PrismaClient } from "@/lib/prisma-client";
import { Prisma } from "@/lib/prisma-client";
import { canonicalJsonString } from "./canonical-json";
import { sha256Hex } from "./base64";
import { validateSubmissionEnvelope, validatePackageIntegrity } from "./validator";
import type {
  EvidenceIngressAuthorityValidator,
} from "./authority-validator";
import {
  CollectionContractRegistry,
} from "../contracts/collection-contract-registry";
import type {
  CaptureSubmissionV2,
  EvidenceIngressResult,
  EvidenceIngressRejectReason,
  CaptureHeaderV2,
  EvidenceIngressAuthorityV2,
  RawRecordSubmissionV2,
  CaptureArtifactSubmissionV2,
} from "./types";

const CHECKSUM_ALGORITHM = "sha256";
const RETRY_DELAYS_MS = [10, 50];
const MAX_ATTEMPTS = 3;
const TX_TIMEOUT_MS = 30_000;

/**
 * Injectable EvidenceIngress core.
 *
 * Production wiring requires real CollectionContract definitions and authority
 * validators — B1-B-02 dark core only runs in tests and isolated DB proof.
 */
export class EvidenceIngress {
  constructor(
    private readonly db: PrismaClient,
    private readonly registry: CollectionContractRegistry,
    private readonly authorityValidator: EvidenceIngressAuthorityValidator,
  ) {}

  async submit(submission: CaptureSubmissionV2): Promise<EvidenceIngressResult> {
    // ── Phase 1: strict structure + protocol/platform + authority shape ──
    const envelope = validateSubmissionEnvelope(submission);
    if (!envelope.ok) {
      return reject(envelope.reason);
    }

    // Establish one request-owned snapshot before the first asynchronous trust
    // boundary. The validator and transaction must never observe later caller
    // mutations of authority, header, or package metadata.
    const authority = structuredClone(envelope.authority);
    const header = structuredClone(envelope.header);
    const capturePackage = structuredClone(envelope.capturePackage);

    // ── Phase 2: authority check (MUST run before base64 decode per §5.3) ──
    const authResult = await this.authorityValidator.validate(
      authority,
      header,
    );
    if (!authResult.valid) {
      return reject(authResult.reason);
    }

    // ── Phase 3: base64/length/checksum/canonical/header-consistency/target ──
    const pkg = validatePackageIntegrity(header, capturePackage);
    if (!pkg.ok) {
      return reject(pkg.reason);
    }

    // ── Phase 4: contract registry/hash/shape (after package integrity per §5.3) ──
    const contractResult = this.registry.lookup(
      header.contractId,
      header.contractVersion,
      header.contractHash,
    );
    if (contractResult.status === "not_registered") {
      return reject("contract_not_registered");
    }
    if (contractResult.status === "hash_mismatch") {
      return reject("contract_hash_mismatch");
    }
    const definition = contractResult.definition;

    // Shape validation
    const recordKinds = pkg.records.map((r) => r.recordKind);
    const slotIds = header.report.slots.map((s) => s.slotId);
    const shapeOk = this.registry.validateShape(definition, {
      recordKinds,
      slotIds,
      terminalState: header.report.terminal.state,
      recordsEmpty: pkg.records.length === 0,
    });
    if (!shapeOk) {
      return reject("contract_shape_mismatch");
    }

    // ── SERIALIZABLE transaction with retry (§6) ──
    return this.executeWithRetry({
      header,
      authority,
      packageBytes: pkg.packageBytes,
      records: pkg.records,
      artifacts: pkg.artifacts,
      restricted: capturePackage.restricted,
    });
  }

  // ── Transaction execution with retry ─────────────────────────────────────

  private async executeWithRetry(
    validation: ValidatedSubmission,
  ): Promise<EvidenceIngressResult> {
    for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
      try {
        return await this.attemptTransaction(validation);
      } catch (err) {
        const retryable = isRetryableSerializationError(err);

        if (retryable) {
          if (attempt < MAX_ATTEMPTS - 1) {
            await sleep(RETRY_DELAYS_MS[attempt]);
            continue;
          }
          return reject("concurrency_exhausted");
        }

        if (isUniqueConstraintError(err)) {
          // P2002: re-read to determine replay/conflict (§6)
          const reread = await this.rereadAfterP2002(validation);
          if (reread !== null) {
            return reread;
          }
          // Not resolved — consume attempt
          if (attempt < MAX_ATTEMPTS - 1) {
            await sleep(RETRY_DELAYS_MS[attempt]);
            continue;
          }
          return reject("concurrency_exhausted");
        }

        // Infrastructure error — re-throw, do not disguise as rejection
        throw err;
      }
    }

    return reject("concurrency_exhausted");
  }

  private async attemptTransaction(
    validation: ValidatedSubmission,
  ): Promise<EvidenceIngressResult> {
    const { header, authority, packageBytes, records, artifacts, restricted } = validation;
    const workspaceId = authority.workspaceId;
    const checksumValue = sha256Hex(packageBytes);

    return this.db.$transaction(
      async (tx) => {
        // 1. Check for existing exact variant (replay)
        const existingVariant = await tx.rawSnapshot.findFirst({
          where: {
            workspaceId,
            captureId: header.captureId,
            checksumAlgorithm: CHECKSUM_ALGORITHM,
            checksumValue,
            capturePackageId: { not: null },
          },
          select: {
            id: true,
            integrityStatus: true,
            capturePackageId: true,
            checksumValue: true,
            ingressReceipt: { select: { id: true } },
          },
        });

        if (existingVariant) {
          // Exact verified replay must also have durable downstream work. A
          // success response without this row would strand Evidence forever.
          if (
            existingVariant.integrityStatus === "verified"
            && existingVariant.ingressReceipt
          ) {
            await tx.v2DurableWork.upsert({
              where: {
                workspaceId_rawSnapshotId: {
                  workspaceId,
                  rawSnapshotId: existingVariant.id,
                },
              },
              create: {
                workspaceId,
                rawSnapshotId: existingVariant.id,
                receiptId: existingVariant.ingressReceipt.id,
                status: "pending",
              },
              update: {},
            });
          }
          // Exact variant exists → replay
          return replayResult({
            id: existingVariant.id,
            integrityStatus: existingVariant.integrityStatus,
            checksumValue: existingVariant.checksumValue,
            ingressReceipt: existingVariant.ingressReceipt,
          });
        }

        // 2. Check for existing verified variant (conflict detection)
        const existingVerified = await tx.rawSnapshot.findFirst({
          where: {
            workspaceId,
            captureId: header.captureId,
            capturePackageId: { not: null },
            integrityStatus: "verified",
          },
          select: {
            id: true,
            checksumValue: true,
            ingressReceipt: { select: { id: true } },
          },
        });

        const isConflict = existingVerified !== null;
        const integrityStatus = isConflict
          ? "capture_identity_conflict" as const
          : "verified" as const;
        const integrityReason = isConflict
          ? "verified_variant_hash_mismatch"
          : null;

        // 3. Insert CapturePackage through Prisma so @default(cuid()) remains
        //    the sole ID contract from the schema/design freeze. The pg adapter
        //    accepts Uint8Array for the Bytes field.
        const capturePackage = await tx.capturePackage.create({
          data: {
            workspaceId,
            packagePayload: new Uint8Array(packageBytes),
            checksumAlgorithm: CHECKSUM_ALGORITHM,
            checksumValue,
            contentLength: packageBytes.length,
            restricted,
          },
          select: { id: true },
        });
        const capturePackageId = capturePackage.id;

        // 4. Insert RawSnapshot with all V2 fields
        const snapshotData = buildRawSnapshotData({
          header,
          authority,
          capturePackageId,
          checksumValue,
          contentLength: packageBytes.length,
          integrityStatus,
          integrityReason,
        });

        const rawSnapshot = await tx.rawSnapshot.create({
          data: snapshotData,
          // Runtime roles intentionally cannot read historical payloadClob or
          // storageKey.  Returning the Prisma default row would silently
          // require every column after a successful INSERT.
          select: { id: true },
        });

        // 5. Insert RawRecord[]
        if (records.length > 0) {
          const recordRows = records.map((r) => ({
            workspaceId,
            rawSnapshotId: rawSnapshot.id,
            jobId: null,
            recordType: null,
            recordKind: r.recordKind,
            platform: r.platform,
            targetKey: r.targetKey,
            externalRecordId: r.externalRecordId,
            sequence: r.sequence,
            payload: r.payload as Prisma.InputJsonValue,
            payloadHash: sha256Hex(Buffer.from(canonicalJsonString(r.payload), "utf8")),
            observedAt: new Date(r.observedAt),
            collectedAt: null,
            idempotencyKey: r.idempotencyKey,
            dedupeKey: null,
          }));
          await tx.rawRecord.createMany({ data: recordRows });
        }

        // 6. Insert CaptureArtifact[]
        if (artifacts.length > 0) {
          const artifactRows = artifacts.map((a) => ({
            workspaceId,
            rawSnapshotId: rawSnapshot.id,
            kind: a.kind,
            artifactChecksum: a.artifactChecksum,
            restricted: a.restricted,
          }));
          await tx.captureArtifact.createMany({ data: artifactRows });
        }

        // 7. Insert EvidenceIngressReceipt
        const receipt = await tx.evidenceIngressReceipt.create({
          data: {
            workspaceId,
            rawSnapshotId: rawSnapshot.id,
            captureId: header.captureId,
            ingressKind: header.ingressKind,
            collectorVersion: header.collectorVersion,
            receivedAt: authority.receivedAt,
          },
        });

        if (isConflict) {
          return {
            status: "conflict" as const,
            rawSnapshotId: rawSnapshot.id,
            receiptId: receipt.id,
            integrityStatus: "capture_identity_conflict" as const,
            checksumValue,
          };
        }

        // 8. V2DurableWork in same transaction — Evidence never committed without work.
        await tx.v2DurableWork.create({
          data: { workspaceId, rawSnapshotId: rawSnapshot.id, receiptId: receipt.id, status: "pending" },
        });

        return {
          status: "committed" as const,
          rawSnapshotId: rawSnapshot.id,
          receiptId: receipt.id,
          integrityStatus: "verified" as const,
          checksumValue,
        };
      },
      {
        isolationLevel: Prisma.TransactionIsolationLevel.Serializable,
        timeout: TX_TIMEOUT_MS,
      },
    );
  }

  // ── P2002 re-read (§6) ────────────────────────────────────────────────────

  private async rereadAfterP2002(
    validation: ValidatedSubmission,
  ): Promise<EvidenceIngressResult | null> {
    const { header, authority, packageBytes } = validation;
    const checksumValue = sha256Hex(packageBytes);

    // Read exact variant
    const exactVariant = await this.db.rawSnapshot.findFirst({
      where: {
        workspaceId: authority.workspaceId,
        captureId: header.captureId,
        checksumAlgorithm: CHECKSUM_ALGORITHM,
        checksumValue,
        capturePackageId: { not: null },
      },
      select: {
        id: true,
        integrityStatus: true,
        ingressReceipt: { select: { id: true } },
      },
    });

    if (exactVariant) {
      return replayResult({
        id: exactVariant.id,
        integrityStatus: exactVariant.integrityStatus,
        checksumValue,
        ingressReceipt: exactVariant.ingressReceipt,
      });
    }

    // Read verified variant (different hash means conflict)
    const verifiedVariant = await this.db.rawSnapshot.findFirst({
      where: {
        workspaceId: authority.workspaceId,
        captureId: header.captureId,
        capturePackageId: { not: null },
        integrityStatus: "verified",
      },
      select: { id: true },
    });

    if (verifiedVariant) {
      // Conflict — but we can't return conflict here because the conflict
      // snapshot hasn't been created yet. The retry loop will re-attempt
      // the transaction, which will detect the verified variant and insert
      // as conflict. Returning null consumes an attempt.
      return null;
    }

    // Neither visible — ambiguous, consume attempt
    return null;
  }
}

// ── Types ──────────────────────────────────────────────────────────────────

type ValidatedSubmission = {
  header: CaptureHeaderV2;
  authority: EvidenceIngressAuthorityV2;
  packageBytes: Uint8Array;
  records: RawRecordSubmissionV2[];
  artifacts: CaptureArtifactSubmissionV2[];
  restricted: boolean;
};

// ── Helpers ────────────────────────────────────────────────────────────────

function reject(
  reason: EvidenceIngressRejectReason,
): Extract<EvidenceIngressResult, { status: "rejected" }> {
  return {
    status: "rejected",
    reason,
    retryable: reason === "concurrency_exhausted",
  };
}

function replayResult(snapshot: {
  id: string;
  integrityStatus: string | null;
  checksumValue: string | null;
  ingressReceipt: { id: string } | null;
}): EvidenceIngressResult {
  // Fail-closed: receipt, checksum, and integrityStatus must all be valid.
  if (!snapshot.ingressReceipt?.id) {
    throw new Error(
      `Implementation invariant: replay snapshot ${snapshot.id} has no EvidenceIngressReceipt.`
    );
  }
  if (!snapshot.checksumValue) {
    throw new Error(
      `Implementation invariant: replay snapshot ${snapshot.id} has no checksumValue.`
    );
  }
  const status = snapshot.integrityStatus;
  if (status !== "verified" && status !== "capture_identity_conflict") {
    throw new Error(
      `Implementation invariant: replay snapshot ${snapshot.id} has illegal integrityStatus ${JSON.stringify(status)}. ` +
      `Expected "verified" or "capture_identity_conflict".`
    );
  }
  return {
    status: "replay",
    rawSnapshotId: snapshot.id,
    receiptId: snapshot.ingressReceipt.id,
    integrityStatus: status,
    checksumValue: snapshot.checksumValue,
  };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

// ── Error classification (§6) ──────────────────────────────────────────────

function isUniqueConstraintError(err: unknown): boolean {
  if (err instanceof Prisma.PrismaClientKnownRequestError) {
    return err.code === "P2002";
  }
  return false;
}

export function isRetryableSerializationError(err: unknown): boolean {
  if (err instanceof Prisma.PrismaClientKnownRequestError) {
    if (err.code === "P2034") return true;
  }

  const sqlState = structuredErrorCode(err);
  return sqlState === "40001" || sqlState === "40P01";
}

function structuredErrorCode(error: unknown): string | null {
  if (error === null || typeof error !== "object") return null;

  const candidate = error as {
    code?: unknown;
    meta?: unknown;
    cause?: unknown;
  };
  if (typeof candidate.code === "string" && /^[0-9A-Z]{5}$/.test(candidate.code)) {
    return candidate.code;
  }
  if (candidate.meta !== null && typeof candidate.meta === "object") {
    const metaCode = (candidate.meta as { code?: unknown }).code;
    if (typeof metaCode === "string" && /^[0-9A-Z]{5}$/.test(metaCode)) {
      return metaCode;
    }
  }
  if (candidate.cause !== undefined && candidate.cause !== error) {
    return structuredErrorCode(candidate.cause);
  }
  return null;
}

// ── RawSnapshot data builder ───────────────────────────────────────────────

function buildRawSnapshotData(input: {
  header: CaptureHeaderV2;
  authority: EvidenceIngressAuthorityV2;
  capturePackageId: string;
  checksumValue: string;
  contentLength: number;
  integrityStatus: "verified" | "capture_identity_conflict";
  integrityReason: string | null;
}): Prisma.RawSnapshotUncheckedCreateInput {
  const { header, authority } = input;

  switch (header.ingressKind) {
    case "execution": {
      assertMatchingAuthorityKind(authority, "execution");
      return makeSnapshotRow({
        workspaceId: authority.workspaceId,
        jobId: header.jobId,
        attemptId: header.attemptId,
        stationId: authority.stationId,
        leaseEpoch: header.leaseEpoch,
        executionPlanVersion: header.executionPlanVersion,
        sourceSummary: null,
        importerIdentity: null,
        migrationAuthorization: null,
        recoveryCaptureId: null,
        recoveryAuthorizedBy: null,
        ...commonFields(input),
      });
    }
    case "manual_import": {
      assertMatchingAuthorityKind(authority, "manual_import");
      return makeSnapshotRow({
        workspaceId: authority.workspaceId,
        jobId: null,
        attemptId: null,
        stationId: null,
        leaseEpoch: null,
        executionPlanVersion: null,
        sourceSummary: header.sourceSummary,
        importerIdentity: authority.importerIdentity,
        migrationAuthorization: null,
        recoveryCaptureId: null,
        recoveryAuthorizedBy: null,
        ...commonFields(input),
      });
    }
    case "recovery": {
      assertMatchingAuthorityKind(authority, "recovery");
      return makeSnapshotRow({
        workspaceId: authority.workspaceId,
        jobId: null,
        attemptId: null,
        stationId: null,
        leaseEpoch: null,
        executionPlanVersion: null,
        sourceSummary: null,
        importerIdentity: null,
        migrationAuthorization: null,
        recoveryCaptureId: header.recoveryCaptureId,
        recoveryAuthorizedBy: authority.recoveryAuthorizedBy,
        ...commonFields(input),
      });
    }
    case "migration": {
      assertMatchingAuthorityKind(authority, "migration");
      return makeSnapshotRow({
        workspaceId: authority.workspaceId,
        jobId: null,
        attemptId: null,
        stationId: null,
        leaseEpoch: null,
        executionPlanVersion: null,
        sourceSummary: header.sourceSummary,
        importerIdentity: null,
        migrationAuthorization: authority.migrationAuthorization,
        recoveryCaptureId: null,
        recoveryAuthorizedBy: null,
        ...commonFields(input),
      });
    }
  }
}

function assertMatchingAuthorityKind<K extends EvidenceIngressAuthorityV2["ingressKind"]>(
  authority: EvidenceIngressAuthorityV2,
  expectedKind: K,
): asserts authority is Extract<EvidenceIngressAuthorityV2, { ingressKind: K }> {
  if (authority.ingressKind !== expectedKind) {
    throw new Error(
      `Implementation invariant: header ingressKind ${expectedKind} does not match authority ingressKind ${authority.ingressKind}.`,
    );
  }
}

function commonFields(input: {
  header: CaptureHeaderV2;
  authority: EvidenceIngressAuthorityV2;
  capturePackageId: string;
  checksumValue: string;
  contentLength: number;
  integrityStatus: "verified" | "capture_identity_conflict";
  integrityReason: string | null;
}) {
  const { header, authority } = input;
  return {
    captureId: header.captureId,
    platform: header.platform,
    targetKey: header.target.expectedTargetKey,
    expectedTargetKey: header.target.expectedTargetKey,
    observedTargetKey: header.target.observedTargetKey,
    observedAt: new Date(header.observedAt),
    receivedAt: authority.receivedAt,
    checksumAlgorithm: CHECKSUM_ALGORITHM,
    checksumValue: input.checksumValue,
    contentLength: input.contentLength,
    ingressKind: authority.ingressKind,
    capturePackageId: input.capturePackageId,
    protocolVersion: header.protocolVersion,
    collectorVersion: header.collectorVersion,
    sourcePrincipal: authority.sourcePrincipal,
    contractId: header.contractId,
    contractVersion: header.contractVersion,
    contractHash: header.contractHash,
    integrityStatus: input.integrityStatus,
    integrityReason: input.integrityReason,
    lifecycleStatus: "ACTIVE" as const,
  };
}

function makeSnapshotRow(fields: {
  workspaceId: string;
  jobId: string | null;
  attemptId: string | null;
  stationId: string | null;
  leaseEpoch: number | null;
  executionPlanVersion: string | null;
  sourceSummary: string | null;
  importerIdentity: string | null;
  migrationAuthorization: string | null;
  recoveryCaptureId: string | null;
  recoveryAuthorizedBy: string | null;
  captureId: string;
  platform: string;
  targetKey: string;
  expectedTargetKey: string;
  observedTargetKey: string | null;
  observedAt: Date;
  receivedAt: Date;
  checksumAlgorithm: string;
  checksumValue: string;
  contentLength: number;
  ingressKind: string;
  capturePackageId: string;
  protocolVersion: string;
  collectorVersion: string;
  sourcePrincipal: string;
  contractId: string;
  contractVersion: number;
  contractHash: string;
  integrityStatus: "verified" | "capture_identity_conflict";
  integrityReason: string | null;
  lifecycleStatus: "ACTIVE";
}): Prisma.RawSnapshotUncheckedCreateInput {
  return {
    workspaceId: fields.workspaceId,
    jobId: fields.jobId,
    attemptId: fields.attemptId,
    captureId: fields.captureId,
    platform: fields.platform,
    targetKey: fields.targetKey,
    expectedTargetKey: fields.expectedTargetKey,
    observedTargetKey: fields.observedTargetKey,
    observedAt: fields.observedAt,
    receivedAt: fields.receivedAt,
    clientObservedAt: null,
    clockSkewSeconds: null,
    observedAtSource: null,
    storageKey: null,
    checksumAlgorithm: fields.checksumAlgorithm,
    checksumValue: fields.checksumValue,
    contentLength: fields.contentLength,
    payloadClob: null,
    schemaVersion: null,
    pluginVersion: null,
    stationId: fields.stationId,
    qualityStatus: null,
    qualityReason: null,
    source: null,
    ingressKind: fields.ingressKind,
    capturePackageId: fields.capturePackageId,
    protocolVersion: fields.protocolVersion,
    collectorVersion: fields.collectorVersion,
    sourcePrincipal: fields.sourcePrincipal,
    sourceSummary: fields.sourceSummary,
    importerIdentity: fields.importerIdentity,
    migrationAuthorization: fields.migrationAuthorization,
    recoveryCaptureId: fields.recoveryCaptureId,
    recoveryAuthorizedBy: fields.recoveryAuthorizedBy,
    leaseEpoch: fields.leaseEpoch,
    executionPlanVersion: fields.executionPlanVersion,
    integrityStatus: fields.integrityStatus,
    integrityReason: fields.integrityReason,
    lifecycleStatus: fields.lifecycleStatus,
    contractId: fields.contractId,
    contractVersion: fields.contractVersion,
    contractHash: fields.contractHash,
  };
}
