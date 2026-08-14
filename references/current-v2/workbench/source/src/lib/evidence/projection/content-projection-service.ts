/**
 * Content-only B3 Projection orchestration.
 *
 * This is the only public write seam. One SERIALIZABLE transaction proves the
 * exact current accepted evaluation, immutable ContentObservation, controlled
 * CurrentProjection, physical media ledger events, and active V2 usages.
 */
import { buildContentAssetKey, buildContentCode } from "@/lib/data-foundation/content-identity";
import { Prisma, type PrismaClient } from "@/lib/prisma-client";
import { parseCanonicalMediaProcessingPayload } from "@/lib/services/media-processing-queue-service";

import type { CanonicalMediaAdapter } from "../media/canonical-media-adapter";
import { writeCanonicalMediaSlotsInTransaction } from "../media/canonical-media-slot-writer";
import {
  readVerifiedMediaArtifacts,
  type VerifiedMediaArtifacts,
  type VerifiedMediaCandidate,
} from "../media/verified-artifact-reader";
import type { EvidenceTransactionFence } from "../transaction-fence";

const MAX_ATTEMPTS = 3;
const RETRY_MS = [10, 50] as const;

export type ProjectionInput = {
  workspaceId: string;
  rawSnapshotId: string;
  contractId: string;
  contractVersion: number;
  evaluationId: string;
  verifiedMedia?: VerifiedMediaArtifacts;
};

export type ProjectionResult =
  | { ok: true; observationId: string; version: number; replayed: boolean }
  | { ok: false; reason: string; detail?: string };

class ProjectionRejected extends Error {
  constructor(readonly result: Extract<ProjectionResult, { ok: false }>) {
    super(result.reason);
    this.name = "ProjectionRejected";
  }
}

type CurrentRow = {
  currentObservationId: string;
  contractEvaluationId: string;
  lastObservedAt: Date;
  projectionVersion: number;
  visibilityState: string;
};

type ControlledAdvanceRow = {
  projectionVersion: number;
  observationId: string;
  replayed: boolean;
};

export class ContentProjectionService {
  constructor(
    private readonly db: Pick<PrismaClient, "$transaction">,
    private readonly mediaAdapter: CanonicalMediaAdapter,
  ) {}

  async project(input: ProjectionInput, transactionFence?: EvidenceTransactionFence): Promise<ProjectionResult> {
    const command = snapshotInput(input);
    for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt += 1) {
      try {
        return await this.db.$transaction(
          async (tx) => {
            await transactionFence?.(tx);
            const result = await this.projectInTransaction(tx, command);
            await transactionFence?.(tx);
            return result;
          },
          {
            isolationLevel: Prisma.TransactionIsolationLevel.Serializable,
            timeout: 30_000,
          },
        );
      } catch (error) {
        if (error instanceof ProjectionRejected) return error.result;
        if (!isRetryable(error) || attempt === MAX_ATTEMPTS - 1) throw error;
        await sleep(RETRY_MS[attempt]);
      }
    }
    return fail("concurrency_exhausted");
  }

  private async projectInTransaction(
    tx: Prisma.TransactionClient,
    command: ReturnType<typeof snapshotInput>,
  ): Promise<ProjectionResult> {
    await lockExactCurrentEvaluation(tx, command);
    const evaluation = await tx.contractEvaluation.findFirst({
      where: {
        id: command.evaluationId,
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
      },
      select: { decision: true, completeness: true },
    });
    if (!evaluation || evaluation.decision !== "accepted" ||
        (evaluation.completeness !== "full" && evaluation.completeness !== "partial")) {
      throw reject("evaluation_not_accepted");
    }

    const inputs = await tx.contractEvaluationInput.findMany({
      where: {
        workspaceId: command.workspaceId,
        rawSnapshotId: command.rawSnapshotId,
        contractEvaluationId: command.evaluationId,
      },
      orderBy: { ordinal: "asc" },
      include: {
        canonicalObservation: {
          include: { normalizationRun: { select: { adapterVersion: true } } },
        },
      },
    });
    const noteInputs = inputs.filter(({ canonicalObservation }) =>
      canonicalObservation.observationKind === "note",
    );
    if (noteInputs.length !== 1) throw reject("note_input_cardinality_invalid");
    const canonical = noteInputs[0].canonicalObservation;
    const note = parseCanonicalNote(canonical.payload, canonical.subjectKey);

    await tx.$queryRaw`
      SELECT "lock_xhs_content_subject"(${command.workspaceId}, ${note.platformContentId})
    `;
    await lockExactCurrentEvaluation(tx, command);
    const currentAccepted = await tx.$queryRaw<Array<{
      canonicalObservationId: string;
      observedAt: Date;
    }>>`
      SELECT co."id" AS "canonicalObservationId", co."observedAt"
      FROM "ContractEvaluationCurrent" cec
      JOIN "ContractEvaluation" ce
        ON ce."workspaceId" = cec."workspaceId"
       AND ce."id" = cec."contractEvaluationId"
       AND ce."decision" = 'accepted'
       AND ce."completeness" IN ('full', 'partial')
      JOIN "ContractEvaluationInput" cei
        ON cei."workspaceId" = ce."workspaceId"
       AND cei."rawSnapshotId" = ce."rawSnapshotId"
       AND cei."contractEvaluationId" = ce."id"
      JOIN "CanonicalObservation" co
        ON co."workspaceId" = cei."workspaceId"
       AND co."rawSnapshotId" = cei."rawSnapshotId"
       AND co."id" = cei."canonicalObservationId"
      WHERE cec."workspaceId" = ${command.workspaceId}
        AND co."observationKind" = 'note'
        AND co."payload"->>'platform' = 'xhs'
        AND co."payload"->>'recordKind' = 'note'
        AND co."payload"#>>'{sourcePayload,noteId}' = ${note.platformContentId}
        AND co."payload"#>>'{sourcePayload,platformContentId}' = ${note.platformContentId}
    `;
    for (const accepted of currentAccepted) {
      const ordering = accepted.observedAt.getTime() - canonical.observedAt.getTime();
      if (ordering > 0) throw reject("stale_observation");
      if (ordering === 0 && accepted.canonicalObservationId !== canonical.id) {
        throw reject("ambiguous_observation_order");
      }
    }

    const contentAsset = await ensureContentAsset(tx, command.workspaceId, note);
    const currentRows = await tx.$queryRaw<CurrentRow[]>`
      SELECT "currentObservationId", "contractEvaluationId", "lastObservedAt",
             "projectionVersion", "visibilityState"
      FROM "ContentCurrentProjection"
      WHERE "workspaceId" = ${command.workspaceId}
        AND "contentAssetId" = ${contentAsset.id}
      FOR UPDATE
    `;
    const current = currentRows[0] ?? null;
    const existingObservation = await tx.contentObservation.findUnique({
      where: {
        workspaceId_contentAssetId_canonicalObservationId: {
          workspaceId: command.workspaceId,
          contentAssetId: contentAsset.id,
          canonicalObservationId: canonical.id,
        },
      },
      select: { id: true, observedAt: true },
    });

    if (current && existingObservation &&
        current.currentObservationId === existingObservation.id &&
        current.contractEvaluationId === command.evaluationId &&
        current.visibilityState === "visible") {
      await tx.$queryRaw`
        SELECT "assert_visible_content_media_completeness"(
          ${command.workspaceId}, ${contentAsset.id}
        )
      `;
      return {
        ok: true,
        observationId: existingObservation.id,
        version: current.projectionVersion,
        replayed: true,
      };
    }
    if (current && !existingObservation) {
      const ordering = canonical.observedAt.getTime() - current.lastObservedAt.getTime();
      if (ordering < 0) throw reject("stale_observation");
      if (ordering === 0) throw reject("ambiguous_observation_order");
    }

    const originalUrl = validateOriginalUrl(note.originalUrl);
    const observation = existingObservation ?? await tx.contentObservation.create({
      data: {
        workspaceId: command.workspaceId,
        contentAssetId: contentAsset.id,
        canonicalObservationId: canonical.id,
        rawSnapshotId: canonical.rawSnapshotId,
        rawRecordId: canonical.rawRecordId,
        observedAt: canonical.observedAt,
        contentType: note.contentType,
        title: note.title,
        bodyText: note.bodyText,
        publishedAt: null,
        authorId: null,
        originalUrl,
        fieldPresence: canonical.fieldPresence === null ? Prisma.DbNull : canonical.fieldPresence,
        qualityStatus: canonical.qualityStatus,
        adapterVersion: canonical.normalizationRun.adapterVersion,
      },
      select: { id: true, observedAt: true },
    });

    const advanced = await tx.$queryRaw<ControlledAdvanceRow[]>`
      SELECT * FROM "advance_content_current_projection"(
        ${command.workspaceId}, ${contentAsset.id}, ${observation.id}, ${command.evaluationId}
      )
    `;
    if (advanced.length !== 1) throw new Error("Controlled projection advance returned no row.");

    await this.activateObservedMedia(tx, command, canonical.id, contentAsset.id, canonical.observedAt);

    return {
      ok: true,
      observationId: observation.id,
      version: advanced[0].projectionVersion,
      replayed: advanced[0].replayed,
    };
  }

  private async activateObservedMedia(
    tx: Prisma.TransactionClient,
    command: ReturnType<typeof snapshotInput>,
    canonicalObservationId: string,
    contentAssetId: string,
    observedAt: Date,
  ): Promise<void> {
    if (command.verifiedMedia) {
      await writeCanonicalMediaSlotsInTransaction(tx, {
        canonicalObservationId,
        verified: command.verifiedMedia,
      });
    }
    const slots = await tx.canonicalMediaSlot.findMany({
      where: { workspaceId: command.workspaceId, canonicalObservationId },
      select: { slotId: true, status: true, kind: true, ordinal: true },
    });
    if (slots.some((slot) => slot.status !== "observed")) {
      throw reject("canonical_media_state_source_incomplete");
    }
    if (!command.verifiedMedia) {
      if (slots.length > 0) throw reject("verified_media_required");
      return; // No observed declaration means media is unknown, never absent/unavailable.
    }

    const verified = readVerifiedMediaArtifacts(command.verifiedMedia);
    if (verified.workspaceId !== command.workspaceId ||
        verified.rawSnapshotId !== command.rawSnapshotId) {
      throw reject("verified_media_evidence_mismatch");
    }
    const candidates = verified.candidates.filter((candidate) =>
      slots.some((slot) => slot.slotId === candidate.slotId),
    );
    validateCoverCandidates(candidates);

    const registration = await this.mediaAdapter.registerObservedMediaInTransaction(tx, {
      verified: command.verifiedMedia,
      canonicalObservationId,
    });
    if (!registration.ok) throw reject(registration.reason, registration.rejectedSlotIds.join(","));
    if (registration.eventIds.length !== candidates.length) {
      throw reject("canonical_media_event_cardinality_mismatch");
    }

    const events = await tx.outboxEvent.findMany({
      where: { id: { in: registration.eventIds } },
      select: { id: true, payload: true },
    });
    if (events.length !== candidates.length) throw reject("canonical_media_event_missing");
    const eventBySlot = new Map<string, {
      id: string;
      payload: NonNullable<ReturnType<typeof parseCanonicalMediaProcessingPayload>>;
    }>();
    for (const event of events) {
      const payload = parseCanonicalMediaProcessingPayload(event.payload);
      if (!payload || payload.workspaceId !== command.workspaceId ||
          payload.ledgerOrigin.canonicalObservationId !== canonicalObservationId ||
          eventBySlot.has(payload.ledgerOrigin.slotId)) {
        throw reject("canonical_media_event_runtime_invalid");
      }
      eventBySlot.set(payload.ledgerOrigin.slotId, { id: event.id, payload });
    }

    for (const candidate of candidates) {
      const event = eventBySlot.get(candidate.slotId);
      if (!event) throw reject("canonical_media_slot_event_missing", candidate.slotId);
      const usage = usageForCandidate(candidate, event.payload.role);
      await tx.contentMediaUsage.create({
        data: {
          workspaceId: command.workspaceId,
          contentAssetId,
          mediaItemId: event.payload.ledgerOrigin.mediaItemId,
          purpose: usage.purpose,
          ordinal: usage.ordinal,
          contractEvaluationId: command.evaluationId,
          canonicalObservationId,
          canonicalSlotId: candidate.slotId,
          mediaOriginId: event.payload.ledgerOrigin.originId,
          originGeneration: event.payload.ledgerOrigin.generation,
          mediaProcessingEventId: event.id,
          observedAt,
        },
      });
    }
  }
}

function snapshotInput(input: ProjectionInput) {
  const workspaceId = required(input.workspaceId, "workspaceId");
  const rawSnapshotId = required(input.rawSnapshotId, "rawSnapshotId");
  const contractId = required(input.contractId, "contractId");
  const evaluationId = required(input.evaluationId, "evaluationId");
  if (!Number.isSafeInteger(input.contractVersion) || input.contractVersion < 1) {
    throw new TypeError("contractVersion must be a positive safe integer.");
  }
  return Object.freeze({
    workspaceId,
    rawSnapshotId,
    contractId,
    contractVersion: input.contractVersion,
    evaluationId,
    verifiedMedia: input.verifiedMedia,
  });
}

async function lockExactCurrentEvaluation(
  tx: Prisma.TransactionClient,
  command: ReturnType<typeof snapshotInput>,
): Promise<void> {
  const rows = await tx.$queryRaw<Array<{ contractEvaluationId: string }>>`
    SELECT "contractEvaluationId"
    FROM "ContractEvaluationCurrent"
    WHERE "workspaceId" = ${command.workspaceId}
      AND "rawSnapshotId" = ${command.rawSnapshotId}
      AND "contractId" = ${command.contractId}
      AND "contractVersion" = ${command.contractVersion}
    FOR UPDATE
  `;
  if (rows.length !== 1 || rows[0].contractEvaluationId !== command.evaluationId) {
    throw reject("cec_not_current");
  }
}

function parseCanonicalNote(payload: Prisma.JsonValue, subjectKey: string) {
  if (!isRecord(payload) || payload.platform !== "xhs" || payload.recordKind !== "note" ||
      !isRecord(payload.sourcePayload)) {
    throw reject("canonical_note_shape_invalid");
  }
  const source = payload.sourcePayload;
  const noteId = requiredField(source.noteId, "noteId");
  const platformContentId = requiredField(source.platformContentId, "platformContentId");
  if (noteId !== platformContentId ||
      subjectKey !== `xhs:note:${encodeURIComponent(noteId)}`) {
    throw reject("note_identity_mismatch");
  }
  if (source.type !== "normal" && source.type !== "video") {
    throw reject("content_type_invalid");
  }
  return {
    noteId,
    platformContentId,
    contentType: source.type,
    title: optionalString(source, "title"),
    bodyText: optionalString(source, "content"),
    originalUrl: optionalString(source, "url"),
  };
}

async function ensureContentAsset(
  tx: Prisma.TransactionClient,
  workspaceId: string,
  note: ReturnType<typeof parseCanonicalNote>,
) {
  const existing = await tx.contentAsset.findUnique({
    where: {
      workspaceId_platform_platformContentId: {
        workspaceId,
        platform: "xhs",
        platformContentId: note.platformContentId,
      },
    },
    select: { id: true },
  });
  if (existing) return existing;
  const assetKey = buildContentAssetKey({ workspaceId, platform: "xhs", platformContentId: note.platformContentId });
  const contentCode = buildContentCode({ platform: "xhs", platformContentId: note.platformContentId });
  if (!assetKey || !contentCode) throw new Error("Validated content identity could not produce server keys.");
  return tx.contentAsset.create({
    data: {
      workspaceId,
      platform: "xhs",
      platformContentId: note.platformContentId,
      assetKey,
      contentCode,
      contentType: note.contentType,
      contentKind: note.contentType === "video" ? "video" : "image_text",
    },
    select: { id: true },
  });
}

function validateOriginalUrl(value: string | null): string | null {
  if (value === null) return null;
  let parsed: URL;
  try { parsed = new URL(value); }
  catch { throw reject("invalid_original_url"); }
  const host = parsed.hostname.toLowerCase();
  const allowed = host === "xiaohongshu.com" || host.endsWith(".xiaohongshu.com") ||
    host === "xhslink.com" || host.endsWith(".xhslink.com");
  if (parsed.protocol !== "https:" || !allowed || parsed.username || parsed.password) {
    throw reject("invalid_original_url");
  }
  return value;
}

function validateCoverCandidates(candidates: readonly VerifiedMediaCandidate[]): void {
  const covers = candidates.filter((candidate) => candidate.purpose === "cover");
  const explicit = covers.filter((candidate) => candidate.coverProvenance === "platform_explicit");
  const fallback = covers.filter((candidate) => candidate.coverProvenance === "first_observed_image");
  if (explicit.length > 1 || fallback.length > 1 ||
      (explicit.length === 1 && fallback.length > 0) ||
      covers.length !== explicit.length + fallback.length) {
    throw reject("canonical_cover_ambiguous");
  }
}

function usageForCandidate(candidate: VerifiedMediaCandidate, role: string) {
  if (candidate.purpose === "cover" && role === "cover") {
    return { purpose: "source_cover", ordinal: null } as const;
  }
  if (candidate.purpose === "body" && role === `image:${candidate.ordinal}`) {
    return { purpose: "source_image", ordinal: candidate.ordinal } as const;
  }
  if (candidate.purpose === "video" && role === "video") {
    return { purpose: "source_video", ordinal: null } as const;
  }
  throw reject("canonical_media_business_slot_mismatch", candidate.slotId);
}

function optionalString(record: Record<string, unknown>, key: string): string | null {
  if (!(key in record) || record[key] === null) return null;
  if (typeof record[key] !== "string") throw reject(`canonical_${key}_invalid`);
  return record[key] as string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function required(value: string, name: string): string {
  const normalized = value.trim();
  if (!normalized) throw new TypeError(`${name} is required.`);
  return normalized;
}

function requiredField(value: unknown, name: string): string {
  if (typeof value !== "string" || value.trim().length === 0) throw reject(`${name}_missing`);
  return value;
}

function reject(reason: string, detail?: string): ProjectionRejected {
  return new ProjectionRejected(fail(reason, detail));
}

function fail(reason: string, detail?: string): Extract<ProjectionResult, { ok: false }> {
  return detail === undefined ? { ok: false, reason } : { ok: false, reason, detail };
}

function isRetryable(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const code = (error as { code?: unknown }).code;
  return code === "P2002" || code === "P2034" || code === "40001" || code === "40P01" ||
    error.message.includes("40001") || error.message.includes("40P01");
}

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
