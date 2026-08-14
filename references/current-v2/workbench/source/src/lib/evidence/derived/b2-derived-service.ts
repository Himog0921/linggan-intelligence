import { Prisma, type PrismaClient } from "@/lib/prisma-client";

import type { CollectionContractDefinitionV2 } from "../ingress/types";
import type { EvidenceTransactionFence } from "../transaction-fence";
import { B2SourceInvariantError } from "./b2-derived-errors";
import { verifyB2Source, type B2SourceSnapshot } from "./b2-source-verification";
import {
  evaluateXhsContract,
  normalizeXhsRecord,
  XHS_ADAPTER_VERSION,
  XHS_CANONICAL_SCHEMA_VERSION,
  XHS_EVALUATOR_VERSION,
  type XhsEvaluationMember,
  type XhsRawRecordFact,
} from "./xhs-derived-contract";

const MAX_TRANSACTION_ATTEMPTS = 3;
const RETRY_DELAYS_MS = [10, 50] as const;
const TRANSACTION_TIMEOUT_MS = 30_000;

export type EvidenceLifecycleStatus = "ACTIVE" | "ARCHIVED" | "REDACTED" | "PURGED";

export type EvidenceAnalysisAuditReceipt = {
  workspaceId: string;
  rawSnapshotId: string;
  accessAuditId: string;
  lifecycleStatus: EvidenceLifecycleStatus;
  integrityStatus: "verified" | "capture_identity_conflict" | string;
  packageBytes: Uint8Array;
  packageChecksumAlgorithm: string;
  packageChecksumValue: string;
  packageContentLength: number;
};

/**
 * Low-level source owned by the future controlled Evidence reader. Its method
 * must commit the access audit independently before it returns package bytes.
 */
export interface EvidenceAnalysisAuditSource {
  readCapturePackageAndAudit(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<EvidenceAnalysisAuditReceipt>;
}

const VERIFIED_EVIDENCE_ANALYSIS_TOKEN = Symbol("verified-evidence-analysis");
const verifiedEvidenceAnalysisState = new WeakMap<
  VerifiedEvidenceAnalysis,
  EvidenceAnalysisAuditReceipt
>();

/** Opaque runtime capability; same-shaped objects are not trusted. */
export class VerifiedEvidenceAnalysis {
  private readonly _verifiedEvidenceAnalysisBrand = true;

  constructor(token: typeof VERIFIED_EVIDENCE_ANALYSIS_TOKEN) {
    if (token !== VERIFIED_EVIDENCE_ANALYSIS_TOKEN) {
      throw new B2SourceInvariantError("Forged Evidence analysis capability.");
    }
  }
}

export interface EvidenceAnalysisReader {
  authorizeAnalysis(input: {
    workspaceId: string;
    rawSnapshotId: string;
  }): Promise<VerifiedEvidenceAnalysis>;
}

/**
 * Creates the only supported capability issuer. There is intentionally no
 * default source while BLK-001/BLK-015 are open.
 */
export function createEvidenceAnalysisReader(
  source: EvidenceAnalysisAuditSource,
): EvidenceAnalysisReader {
  return {
    async authorizeAnalysis(input) {
      const receipt = await source.readCapturePackageAndAudit({
        workspaceId: input.workspaceId,
        rawSnapshotId: input.rawSnapshotId,
      });
      const snapshotted = snapshotAuditReceipt(receipt);
      const capability = new VerifiedEvidenceAnalysis(VERIFIED_EVIDENCE_ANALYSIS_TOKEN);
      verifiedEvidenceAnalysisState.set(capability, snapshotted);
      return capability;
    },
  };
}

export type B2DerivedResult =
  | {
      status: "committed" | "replay";
      contractEvaluationId: string;
      decision: "accepted" | "rejected";
      completeness: "full" | "partial" | "not_applicable";
      revision: number;
    }
  | {
      status: "rejected";
      reason: "evidence_ineligible" | "concurrency_exhausted";
      retryable: boolean;
    };

export type B2DerivedCommand = {
  workspaceId: string;
  rawSnapshotId: string;
  retryMode: "replay" | "explicit_retry";
};

type B2Database = Pick<PrismaClient, "$transaction">;

export class B2DerivedService {
  constructor(
    private readonly db: B2Database,
    private readonly evidenceReader: EvidenceAnalysisReader,
  ) {}

  async derive(input: B2DerivedCommand, transactionFence?: EvidenceTransactionFence): Promise<B2DerivedResult> {
    // Snapshot caller-owned input before the first await. Eligibility and the
    // transaction must be bound to the same immutable workspace/snapshot pair.
    const command: B2DerivedCommand = Object.freeze({
      workspaceId: input.workspaceId,
      rawSnapshotId: input.rawSnapshotId,
      retryMode: input.retryMode,
    });
    const capability = await this.evidenceReader.authorizeAnalysis({
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
    });
    const evidence = readVerifiedEvidenceAnalysis(capability);

    if (!isEligible(command, evidence)) {
      return evidenceIneligible();
    }

    for (let attempt = 0; attempt < MAX_TRANSACTION_ATTEMPTS; attempt += 1) {
      try {
        return await this.db.$transaction(
          async (tx) => {
            await transactionFence?.(tx);
            const result = await deriveInTransaction(tx, command, evidence);
            await transactionFence?.(tx);
            return result;
          },
          {
            isolationLevel: Prisma.TransactionIsolationLevel.Serializable,
            timeout: TRANSACTION_TIMEOUT_MS,
          },
        );
      } catch (error) {
        if (!isRetryableTransactionConflict(error)) throw error;
        if (attempt === MAX_TRANSACTION_ATTEMPTS - 1) {
          return {
            status: "rejected",
            reason: "concurrency_exhausted",
            retryable: true,
          };
        }
        await sleep(RETRY_DELAYS_MS[attempt]);
      }
    }

    return {
      status: "rejected",
      reason: "concurrency_exhausted",
      retryable: true,
    };
  }
}

async function deriveInTransaction(
  tx: Prisma.TransactionClient,
  command: B2DerivedCommand,
  evidence: EvidenceAnalysisAuditReceipt & {
    lifecycleStatus: "ACTIVE" | "ARCHIVED";
    integrityStatus: "verified";
  },
): Promise<B2DerivedResult> {
  // Package bytes are supplied only by the independently audited reader. The
  // writer transaction checks Evidence metadata/records but never reads the
  // CapturePackage registry.
  const snapshot = await tx.rawSnapshot.findFirst({
    where: {
      workspaceId: command.workspaceId,
      id: command.rawSnapshotId,
    },
    select: {
      id: true,
      workspaceId: true,
      captureId: true,
      platform: true,
      integrityStatus: true,
      checksumAlgorithm: true,
      checksumValue: true,
      contentLength: true,
      contractId: true,
      contractVersion: true,
      contractHash: true,
      records: {
        select: {
          id: true,
          workspaceId: true,
          rawSnapshotId: true,
          recordKind: true,
          platform: true,
          targetKey: true,
          externalRecordId: true,
          sequence: true,
          payloadHash: true,
          observedAt: true,
          idempotencyKey: true,
        },
      },
    },
  });

  if (!snapshot ||
      snapshot.integrityStatus !== "verified" ||
      snapshot.workspaceId !== evidence.workspaceId ||
      snapshot.id !== evidence.rawSnapshotId) {
    return evidenceIneligible();
  }
  const { capturePackage, contract, records } = verifyB2Source(
    snapshot as B2SourceSnapshot,
    evidence,
  );

  for (const record of records) {
    await persistNormalization(tx, record, command.retryMode);
  }

  const members = await selectEvaluationMembers(tx, records);
  const observationByRunId = await readCurrentObservations(tx, command, members);
  const inputMatches = members.every((member) => {
    if (member.memberKind !== "current" || member.runId === null) return true;
    const observation = observationByRunId.get(member.runId);
    return observation !== undefined &&
      observation.payloadHash === member.outputHash &&
      observation.rawRecordId === member.recordId;
  });
  if (!inputMatches) {
    throw new B2SourceInvariantError("Evaluation member/observation binding changed.");
  }
  const evaluation = evaluateXhsContract({
    snapshot: {
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
    },
    eligibility: {
      lifecycleStatus: evidence.lifecycleStatus,
      integrityStatus: "verified",
    },
    contract,
    terminal: capturePackage.header.report.terminal,
    slots: capturePackage.header.report.slots,
    members,
  });

  const existingEvaluation = await tx.contractEvaluation.findFirst({
    where: {
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
      contractId: contract.id,
      contractVersion: contract.version,
      evaluatorVersion: XHS_EVALUATOR_VERSION,
      canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
      evaluationInputHash: evaluation.evaluationInputHash,
    },
    select: {
      id: true,
      decision: true,
      completeness: true,
    },
  });
  if (existingEvaluation) {
    const current = await readEvaluationCurrent(tx, command, contract);
    if (!current) {
      throw new B2SourceInvariantError("A terminal evaluation exists without Evaluation Current.");
    }
    return {
      status: "replay",
      contractEvaluationId: existingEvaluation.id,
      decision: exactDecision(existingEvaluation.decision),
      completeness: exactCompleteness(existingEvaluation.completeness),
      revision: current.revision,
    };
  }

  const createdEvaluation = await tx.contractEvaluation.create({
    data: {
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
      contractId: contract.id,
      contractVersion: contract.version,
      evaluatorVersion: XHS_EVALUATOR_VERSION,
      canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
      evaluationInputHash: evaluation.evaluationInputHash,
      decision: evaluation.decision,
      completeness: evaluation.completeness,
      rejectionCode: evaluation.rejectionCode,
      rejectionReason: evaluation.rejectionReason,
    },
    select: { id: true },
  });

  const linkedMembers = members.filter(
    (member): member is XhsEvaluationMember & { runId: string; status: NonNullable<XhsEvaluationMember["status"]>; inputHash: string } =>
      member.runId !== null && member.status !== null && member.inputHash !== null,
  );
  if (linkedMembers.length > 0) {
    await tx.contractEvaluationNormalizationRun.createMany({
      data: linkedMembers.map((member) => ({
        evaluationId: createdEvaluation.id,
        normalizationRunId: member.runId,
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
        status: member.status,
        inputPayloadHash: member.inputHash,
        outputPayloadHash: member.outputHash,
      })),
    });
  }

  if (evaluation.decision === "accepted") {
    const inputs = evaluation.orderedMembers.map((member, ordinal) => {
      if (member.memberKind !== "current" || member.runId === null || member.outputHash === null) {
        throw new B2SourceInvariantError("Accepted evaluation contains a non-current member.");
      }
      const observation = observationByRunId.get(member.runId);
      if (!observation || observation.payloadHash !== member.outputHash) {
        throw new B2SourceInvariantError("Accepted evaluation observation binding changed.");
      }
      return {
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
        contractEvaluationId: createdEvaluation.id,
        canonicalObservationId: observation.id,
        ordinal,
        canonicalOutputHash: observation.payloadHash,
      };
    });
    if (inputs.length > 0) {
      await tx.contractEvaluationInput.createMany({ data: inputs });
    }
  }

  // B3 owns projection creation, while B2 owns accepted/rejected Current
  // decisions. The database coordinator locks each affected content subject
  // and revokes the prior visible Projection/usages before this CEC advances.
  const previousCEC = await readEvaluationCurrent(tx, command, contract);

  await tx.$queryRaw`
    SELECT "revoke_content_projection_for_evaluation"(
      ${command.workspaceId},
      ${previousCEC?.contractEvaluationId ?? null},
      ${createdEvaluation.id}
    )
  `;

  const revision = await advanceEvaluationCurrent(
    tx,
    command,
    contract,
    createdEvaluation.id,
  );

  return {
    status: "committed",
    contractEvaluationId: createdEvaluation.id,
    decision: evaluation.decision,
    completeness: evaluation.completeness,
    revision,
  };
}

export { B2SourceInvariantError } from "./b2-derived-errors";

async function persistNormalization(
  tx: Prisma.TransactionClient,
  record: XhsRawRecordFact,
  retryMode: B2DerivedCommand["retryMode"],
): Promise<void> {
  const normalized = normalizeXhsRecord(record);
  if (retryMode === "replay") {
    const existing = await tx.normalizationRun.findFirst({
      where: {
        rawRecordId: record.id,
        adapterId: normalized.adapterId,
        adapterVersion: normalized.adapterVersion,
        canonicalSchemaVersion: normalized.canonicalSchemaVersion,
        inputPayloadHash: normalized.inputPayloadHash,
      },
      orderBy: { attemptNumber: "desc" },
      select: { id: true },
    });
    if (existing) return;
  }

  const latest = await tx.normalizationRun.aggregate({
    where: {
      rawRecordId: record.id,
      adapterId: normalized.adapterId,
      adapterVersion: normalized.adapterVersion,
      canonicalSchemaVersion: normalized.canonicalSchemaVersion,
    },
    _max: { attemptNumber: true },
  });
  const attemptNumber = (latest._max.attemptNumber ?? 0) + 1;
  const run = await tx.normalizationRun.create({
    data: {
      workspaceId: record.workspaceId,
      rawSnapshotId: record.rawSnapshotId,
      rawRecordId: record.id,
      adapterId: normalized.adapterId,
      adapterVersion: normalized.adapterVersion,
      canonicalSchemaVersion: normalized.canonicalSchemaVersion,
      attemptNumber,
      status: normalized.status,
      inputPayloadHash: normalized.inputPayloadHash,
      outputPayloadHash: normalized.outputPayloadHash,
      ...(normalized.missingFields === null
        ? {}
        : { missingFields: normalized.missingFields as Prisma.InputJsonValue }),
      ...(normalized.parseErrors === null
        ? {}
        : { parseErrors: normalized.parseErrors as Prisma.InputJsonValue }),
    },
    select: { id: true },
  });

  if (normalized.status !== "normalized" || !normalized.observation) return;
  const observation = await tx.canonicalObservation.create({
    data: {
      workspaceId: record.workspaceId,
      normalizationRunId: run.id,
      rawSnapshotId: record.rawSnapshotId,
      rawRecordId: record.id,
      observationKind: normalized.observation.observationKind,
      subjectKey: normalized.observation.subjectKey,
      observedAt: normalized.observation.observedAt,
      schemaVersion: normalized.observation.schemaVersion,
      payload: normalized.observation.payload as Prisma.InputJsonValue,
      payloadHash: normalized.observation.payloadHash,
      qualityStatus: normalized.observation.qualityStatus,
      fieldPresence: normalized.observation.fieldPresence,
    },
    select: { id: true },
  });
  if (!observation.id) {
    throw new B2SourceInvariantError("CanonicalObservation insert returned no identity.");
  }
  await tx.normalizationRunCurrent.upsert({
    where: {
      workspaceId_rawRecordId: {
        workspaceId: record.workspaceId,
        rawRecordId: record.id,
      },
    },
    create: {
      workspaceId: record.workspaceId,
      rawRecordId: record.id,
      rawSnapshotId: record.rawSnapshotId,
      normalizationRunId: run.id,
      adapterId: normalized.adapterId,
      adapterVersion: normalized.adapterVersion,
      canonicalSchemaVersion: normalized.canonicalSchemaVersion,
    },
    update: {
      rawSnapshotId: record.rawSnapshotId,
      normalizationRunId: run.id,
      adapterId: normalized.adapterId,
      adapterVersion: normalized.adapterVersion,
      canonicalSchemaVersion: normalized.canonicalSchemaVersion,
    },
  });
}

async function selectEvaluationMembers(
  tx: Prisma.TransactionClient,
  records: XhsRawRecordFact[],
): Promise<XhsEvaluationMember[]> {
  const members: XhsEvaluationMember[] = [];
  for (const record of records) {
    const expected = normalizeXhsRecord(record);
    const current = await tx.normalizationRunCurrent.findUnique({
      where: {
        workspaceId_rawRecordId: {
          workspaceId: record.workspaceId,
          rawRecordId: record.id,
        },
      },
      select: {
        adapterId: true,
        adapterVersion: true,
        canonicalSchemaVersion: true,
        normalizationRun: {
          select: {
            id: true,
            rawRecordId: true,
            attemptNumber: true,
            status: true,
            adapterId: true,
            adapterVersion: true,
            canonicalSchemaVersion: true,
            inputPayloadHash: true,
            outputPayloadHash: true,
          },
        },
      },
    });
    if (current) {
      const run = current.normalizationRun;
      const exactVersion = current.adapterId === expected.adapterId &&
        current.adapterVersion === XHS_ADAPTER_VERSION &&
        current.canonicalSchemaVersion === XHS_CANONICAL_SCHEMA_VERSION &&
        run.adapterId === current.adapterId &&
        run.adapterVersion === current.adapterVersion &&
        run.canonicalSchemaVersion === current.canonicalSchemaVersion &&
        run.rawRecordId === record.id;
      members.push(memberFromRun(
        record,
        run,
        exactVersion ? "current" : "current_version_mismatch",
      ));
      continue;
    }

    const latest = await tx.normalizationRun.findFirst({
      where: {
        rawRecordId: record.id,
        adapterId: expected.adapterId,
        adapterVersion: XHS_ADAPTER_VERSION,
        canonicalSchemaVersion: XHS_CANONICAL_SCHEMA_VERSION,
      },
      orderBy: { attemptNumber: "desc" },
      select: {
        id: true,
        attemptNumber: true,
        status: true,
        adapterId: true,
        adapterVersion: true,
        canonicalSchemaVersion: true,
        inputPayloadHash: true,
        outputPayloadHash: true,
      },
    });
    if (!latest) {
      members.push(missingMember(record));
      continue;
    }
    const status = exactRunStatus(latest.status);
    const memberKind = status === "rejected"
      ? "latest_rejected_attempt"
      : status === "quarantined"
        ? "latest_quarantined_attempt"
        : "normalized_current_missing";
    members.push(memberFromRun(record, latest, memberKind));
  }
  return members.sort(compareMembersByRecord);
}

function memberFromRun(
  record: XhsRawRecordFact,
  run: {
    id: string;
    attemptNumber: number;
    status: string;
    adapterId: string;
    adapterVersion: string;
    canonicalSchemaVersion: string;
    inputPayloadHash: string;
    outputPayloadHash: string | null;
  },
  memberKind: XhsEvaluationMember["memberKind"],
): XhsEvaluationMember {
  return {
    recordId: record.id,
    memberKind,
    runId: run.id,
    attemptNumber: run.attemptNumber,
    status: exactRunStatus(run.status),
    adapterId: run.adapterId,
    adapterVersion: run.adapterVersion,
    canonicalSchemaVersion: run.canonicalSchemaVersion,
    inputHash: run.inputPayloadHash,
    outputHash: run.outputPayloadHash,
    sequence: record.sequence,
  };
}

function missingMember(record: XhsRawRecordFact): XhsEvaluationMember {
  return {
    recordId: record.id,
    memberKind: "missing",
    runId: null,
    attemptNumber: null,
    status: null,
    adapterId: null,
    adapterVersion: null,
    canonicalSchemaVersion: null,
    inputHash: null,
    outputHash: null,
    sequence: record.sequence,
  };
}

async function readCurrentObservations(
  tx: Prisma.TransactionClient,
  command: B2DerivedCommand,
  members: XhsEvaluationMember[],
): Promise<Map<string, { id: string; rawRecordId: string; payloadHash: string }>> {
  const runIds = members
    .filter((member) => member.memberKind === "current" && member.runId !== null)
    .map((member) => member.runId as string);
  if (runIds.length === 0) return new Map();
  const observations = await tx.canonicalObservation.findMany({
    where: {
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
      normalizationRunId: { in: runIds },
    },
    select: {
      id: true,
      normalizationRunId: true,
      rawRecordId: true,
      payloadHash: true,
    },
  });
  const byRunId = new Map<string, { id: string; rawRecordId: string; payloadHash: string }>();
  for (const observation of observations) {
    if (byRunId.has(observation.normalizationRunId)) {
      throw new B2SourceInvariantError("A NormalizationRun has multiple CanonicalObservations.");
    }
    byRunId.set(observation.normalizationRunId, {
      id: observation.id,
      rawRecordId: observation.rawRecordId,
      payloadHash: observation.payloadHash,
    });
  }
  return byRunId;
}

async function readEvaluationCurrent(
  tx: Prisma.TransactionClient,
  command: B2DerivedCommand,
  contract: CollectionContractDefinitionV2,
): Promise<{ contractEvaluationId: string; revision: number } | null> {
  return tx.contractEvaluationCurrent.findUnique({
    where: {
      workspaceId_rawSnapshotId_contractId_contractVersion: {
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
        contractId: contract.id,
        contractVersion: contract.version,
      },
    },
    select: { contractEvaluationId: true, revision: true },
  });
}

async function advanceEvaluationCurrent(
  tx: Prisma.TransactionClient,
  command: B2DerivedCommand,
  contract: CollectionContractDefinitionV2,
  evaluationId: string,
): Promise<number> {
  const current = await readEvaluationCurrent(tx, command, contract);
  if (!current) {
    await tx.contractEvaluationCurrent.create({
      data: {
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
        contractId: contract.id,
        contractVersion: contract.version,
        contractEvaluationId: evaluationId,
        revision: 1,
      },
    });
    return 1;
  }
  const revision = current.revision + 1;
  const updated = await tx.contractEvaluationCurrent.updateMany({
    where: {
      workspaceId: command.workspaceId,
      rawSnapshotId: command.rawSnapshotId,
      contractId: contract.id,
      contractVersion: contract.version,
      contractEvaluationId: current.contractEvaluationId,
      revision: current.revision,
    },
    data: { contractEvaluationId: evaluationId, revision },
  });
  if (updated.count !== 1) {
    throw { code: "40001", message: "ContractEvaluationCurrent CAS conflict." };
  }
  return revision;
}

function compareMembersByRecord(left: XhsEvaluationMember, right: XhsEvaluationMember): number {
  return compareSequenceThenId(left.sequence ?? null, left.recordId, right.sequence ?? null, right.recordId);
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

function exactRunStatus(value: string): NonNullable<XhsEvaluationMember["status"]> {
  if (value === "normalized" || value === "rejected" || value === "quarantined") return value;
  throw new B2SourceInvariantError(`Unexpected NormalizationRun status: ${value}.`);
}

function exactDecision(value: string): "accepted" | "rejected" {
  if (value === "accepted" || value === "rejected") return value;
  throw new B2SourceInvariantError(`Unexpected ContractEvaluation decision: ${value}.`);
}

function exactCompleteness(value: string): "full" | "partial" | "not_applicable" {
  if (value === "full" || value === "partial" || value === "not_applicable") return value;
  throw new B2SourceInvariantError(`Unexpected ContractEvaluation completeness: ${value}.`);
}

function isRetryableTransactionConflict(error: unknown): boolean {
  if (error instanceof Prisma.PrismaClientKnownRequestError) {
    return error.code === "P2034" ||
      (error.code === "P2002" && isExpectedDerivedUniqueConflict(error.meta?.target));
  }
  const code = structuredErrorCode(error);
  return code === "40001" || code === "40P01";
}

const EXPECTED_DERIVED_CONSTRAINTS = new Set([
  "NormalizationRun_record_adapter_attempt_key",
  "NormalizationRunCurrent_pkey",
  "NormalizationRunCurrent_workspace_run_key",
  "ContractEvaluation_identity_key",
  "ContractEvaluationCurrent_pkey",
  "ContractEvaluationNormalizationRun_evaluation_run_key",
  "ContractEvaluationInput_evaluation_observation_key",
  "ContractEvaluationInput_evaluation_ordinal_key",
]);

const EXPECTED_DERIVED_TARGETS = new Set([
  "adapterId,adapterVersion,attemptNumber,canonicalSchemaVersion,rawRecordId",
  "contractEvaluationId,canonicalObservationId",
  "contractEvaluationId,ordinal",
  "canonicalSchemaVersion,contractId,contractVersion,evaluationInputHash,evaluatorVersion,rawSnapshotId,workspaceId",
  "contractId,contractVersion,rawSnapshotId,workspaceId",
  "evaluationId,normalizationRunId",
  "normalizationRunId,workspaceId",
  "rawRecordId,workspaceId",
]);

function isExpectedDerivedUniqueConflict(target: unknown): boolean {
  if (typeof target === "string") return EXPECTED_DERIVED_CONSTRAINTS.has(target);
  if (!Array.isArray(target) || !target.every((field) => typeof field === "string")) return false;
  return EXPECTED_DERIVED_TARGETS.has([...target].sort().join(","));
}

function structuredErrorCode(error: unknown): string | null {
  if (error === null || typeof error !== "object") return null;
  const candidate = error as { code?: unknown; meta?: unknown; cause?: unknown };
  if (typeof candidate.code === "string" && /^[0-9A-Z]{5}$/.test(candidate.code)) {
    return candidate.code;
  }
  if (candidate.meta !== null && typeof candidate.meta === "object") {
    const metaCode = (candidate.meta as { code?: unknown }).code;
    if (typeof metaCode === "string" && /^[0-9A-Z]{5}$/.test(metaCode)) return metaCode;
  }
  if (candidate.cause !== undefined && candidate.cause !== error) {
    return structuredErrorCode(candidate.cause);
  }
  return null;
}

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

function isEligible(
  command: B2DerivedCommand,
  claim: EvidenceAnalysisAuditReceipt,
): claim is EvidenceAnalysisAuditReceipt & {
  lifecycleStatus: "ACTIVE" | "ARCHIVED";
  integrityStatus: "verified";
} {
  return claim.workspaceId === command.workspaceId &&
    claim.rawSnapshotId === command.rawSnapshotId &&
    claim.accessAuditId.trim().length > 0 &&
    (claim.lifecycleStatus === "ACTIVE" || claim.lifecycleStatus === "ARCHIVED") &&
    claim.integrityStatus === "verified";
}

function snapshotAuditReceipt(
  receipt: EvidenceAnalysisAuditReceipt,
): EvidenceAnalysisAuditReceipt {
  if (!(receipt.packageBytes instanceof Uint8Array) ||
      !receipt.workspaceId ||
      !receipt.rawSnapshotId ||
      !receipt.accessAuditId ||
      !Number.isSafeInteger(receipt.packageContentLength) ||
      receipt.packageContentLength < 0) {
    throw new B2SourceInvariantError("Controlled Evidence reader returned an invalid receipt.");
  }
  return Object.freeze({
    workspaceId: receipt.workspaceId,
    rawSnapshotId: receipt.rawSnapshotId,
    accessAuditId: receipt.accessAuditId,
    lifecycleStatus: receipt.lifecycleStatus,
    integrityStatus: receipt.integrityStatus,
    packageBytes: Uint8Array.from(receipt.packageBytes),
    packageChecksumAlgorithm: receipt.packageChecksumAlgorithm,
    packageChecksumValue: receipt.packageChecksumValue,
    packageContentLength: receipt.packageContentLength,
  });
}

function readVerifiedEvidenceAnalysis(
  capability: VerifiedEvidenceAnalysis,
): EvidenceAnalysisAuditReceipt {
  const state = verifiedEvidenceAnalysisState.get(capability);
  if (!state) {
    throw new B2SourceInvariantError("Evidence analysis capability is not authentic.");
  }
  return {
    ...state,
    packageBytes: Uint8Array.from(state.packageBytes),
  };
}

function evidenceIneligible(): Extract<B2DerivedResult, { status: "rejected" }> {
  return {
    status: "rejected",
    reason: "evidence_ineligible",
    retryable: false,
  };
}
