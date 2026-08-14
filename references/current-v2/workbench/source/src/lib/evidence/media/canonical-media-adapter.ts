/**
 * Canonical Media → Media Domain dark adapter (B3-MEDIA-SRC-003).
 *
 * Accepts only the opaque verified-Artifact capability. It registers physical
 * Media identity and an auditable processing request, but never activates a
 * business media relation or Projection and has no runtime caller.
 */

import { Prisma, type PrismaClient } from "@/lib/prisma-client";
import {
  planSourceMediaIdentities,
  type SourceMediaIdentityItem,
} from "@/lib/services/media-library-identity";
import { observeMediaOriginInTransaction } from "@/lib/services/media-library-write-service";
import {
  enqueueCanonicalMediaProcessingJob,
  type CanonicalMediaCandidate,
} from "@/lib/services/media-processing-queue-service";

import {
  MediaArtifactInvariantError,
  readVerifiedMediaArtifacts,
  type VerifiedMediaArtifacts,
  type VerifiedMediaCandidate,
} from "./verified-artifact-reader";

const MAX_TRANSACTION_ATTEMPTS = 3;
const RETRY_DELAYS_MS = [10, 50] as const;

type CanonicalMediaDatabase = Pick<PrismaClient, "$transaction">;

export type CanonicalMediaRegistrationResult =
  | {
      ok: true;
      mediaItemIds: string[];
      originIds: string[];
      eventIds: string[];
      enqueued: number;
      replayed: number;
    }
  | { ok: false; reason: string; rejectedSlotIds: string[] };

class CanonicalMediaRegistrationRejected extends Error {
  constructor(readonly result: Extract<CanonicalMediaRegistrationResult, { ok: false }>) {
    super(result.reason);
    this.name = "CanonicalMediaRegistrationRejected";
  }
}

export class CanonicalMediaAdapter {
  constructor(private readonly db: CanonicalMediaDatabase) {}

  async registerObservedMedia(input: {
    verified: VerifiedMediaArtifacts;
    canonicalObservationId: string;
  }): Promise<CanonicalMediaRegistrationResult> {
    const command = Object.freeze({
      canonicalObservationId: required(input.canonicalObservationId, "canonicalObservationId"),
      evidence: readVerifiedMediaArtifacts(input.verified),
    });

    for (let attempt = 0; attempt < MAX_TRANSACTION_ATTEMPTS; attempt += 1) {
      try {
        return await this.db.$transaction(
          (tx) => registerInTransaction(tx, command),
          { isolationLevel: Prisma.TransactionIsolationLevel.Serializable },
        );
      } catch (error) {
        if (error instanceof CanonicalMediaRegistrationRejected) return error.result;
        if (!isRetryableTransactionConflict(error) || attempt === MAX_TRANSACTION_ATTEMPTS - 1) {
          throw error;
        }
        await sleep(RETRY_DELAYS_MS[attempt]);
      }
    }
    throw new MediaArtifactInvariantError("Canonical media registration retries exhausted.");
  }

  /** In-transaction variant for Phase 3: use within existing SERIALIZABLE tx. */
  async registerObservedMediaInTransaction(
    tx: Prisma.TransactionClient,
    input: { verified: VerifiedMediaArtifacts; canonicalObservationId: string },
  ): Promise<CanonicalMediaRegistrationResult> {
    const command = Object.freeze({
      canonicalObservationId: required(input.canonicalObservationId, "canonicalObservationId"),
      evidence: readVerifiedMediaArtifacts(input.verified),
    });
    try { return await registerInTransaction(tx, command); }
    catch (error) { if (error instanceof CanonicalMediaRegistrationRejected) return error.result; throw error; }
  }
}

async function registerInTransaction(
  tx: Prisma.TransactionClient,
  command: {
    canonicalObservationId: string;
    evidence: ReturnType<typeof readVerifiedMediaArtifacts>;
  },
): Promise<CanonicalMediaRegistrationResult> {
  const observation = await tx.canonicalObservation.findFirst({
    where: {
      id: command.canonicalObservationId,
      workspaceId: command.evidence.workspaceId,
      rawSnapshotId: command.evidence.rawSnapshotId,
    },
    select: {
      id: true,
      workspaceId: true,
      rawSnapshotId: true,
      observationKind: true,
      subjectKey: true,
      observedAt: true,
    },
  });
  if (!observation) return rejected("canonical_observation_not_bound", []);
  if (observation.observationKind !== "note") {
    return rejected("canonical_observation_kind_unsupported", []);
  }

  const subjectCandidates = command.evidence.candidates.filter((candidate) =>
    observation.subjectKey === `xhs:note:${encodeURIComponent(candidate.noteId)}`,
  );
  const slots = await tx.canonicalMediaSlot.findMany({
    where: {
      workspaceId: observation.workspaceId,
      canonicalObservationId: observation.id,
    },
    select: { slotId: true, status: true, kind: true, ordinal: true },
  });
  const binding = bindObservedSlots(slots, subjectCandidates);
  if (!binding.ok) return binding;
  if (binding.candidates.length === 0) return emptySuccess();

  const unsupported = binding.candidates.filter((candidate) =>
    candidate.kind === "live_photo" || candidate.purpose === "live_photo",
  );
  if (unsupported.length > 0) {
    return rejected(
      "canonical_media_kind_source_incomplete",
      unsupported.map((candidate) => candidate.slotId),
    );
  }

  const mappedItems = binding.candidates.map((candidate) => ({
    candidate,
    item: identityItem(candidate),
  }));
  if (mappedItems.some((mapped) => mapped.item === null)) {
    return rejected(
      "canonical_media_slot_mapping_invalid",
      mappedItems.filter((mapped) => mapped.item === null).map((mapped) => mapped.candidate.slotId),
    );
  }
  const plan = planSourceMediaIdentities({
    platform: "xhs",
    platformContentId: binding.platformContentId,
    items: mappedItems.map((mapped) => mapped.item as SourceMediaIdentityItem),
  });
  if (plan.rejected.length > 0 || plan.identities.length === 0) {
    return rejected(
      "canonical_media_identity_rejected",
      binding.candidates.map((candidate) => candidate.slotId),
    );
  }

  const identityBindings = plan.identities.map((identity) => ({
    identity,
    candidates: binding.candidates.filter((candidate) =>
      candidate.observedAddress === identity.sourceUrl &&
      identity.usages.some((usage) => usageMatchesCandidate(usage, candidate)),
    ),
  }));
  const bindingCounts = new Map(binding.candidates.map((candidate) => [candidate.slotId, 0]));
  for (const identityBinding of identityBindings) {
    for (const candidate of identityBinding.candidates) {
      bindingCounts.set(candidate.slotId, (bindingCounts.get(candidate.slotId) ?? 0) + 1);
    }
  }
  const ambiguous = [...bindingCounts.entries()]
    .filter(([, count]) => count !== 1)
    .map(([slotId]) => slotId);
  if (ambiguous.length > 0) return rejected("canonical_media_identity_binding_ambiguous", ambiguous);

  const mediaItemIds = new Set<string>();
  const originIds = new Set<string>();
  const eventIds = new Set<string>();
  let enqueued = 0;
  let replayed = 0;
  for (const identityBinding of identityBindings) {
    const origin = await observeMediaOriginInTransaction({
      workspaceId: observation.workspaceId,
      provider: "xhs",
      stableLocator: identityBinding.identity.stableLocator,
      kind: identityBinding.identity.kind,
      sourceUrl: identityBinding.identity.sourceUrl,
      observedAt: observation.observedAt,
      db: tx,
    });
    if (!origin.accepted) {
      throw new CanonicalMediaRegistrationRejected(
        rejected(
          "canonical_media_source_superseded",
          identityBinding.candidates.map((candidate) => candidate.slotId),
        ),
      );
    }
    mediaItemIds.add(origin.mediaItemId);
    originIds.add(origin.id);

    for (const candidate of identityBinding.candidates) {
      const queueCandidate: CanonicalMediaCandidate = {
        workspaceId: observation.workspaceId,
        source: { type: "raw_evidence", id: observation.rawSnapshotId },
        platform: "xhs",
        platformContentId: candidate.platformContentId,
        role: processingRole(candidate),
        kind: candidate.kind as "image" | "video",
        sourceUrl: candidate.observedAddress,
        contentAssetId: null,
        ledgerOrigin: {
          originId: origin.id,
          mediaItemId: origin.mediaItemId,
          generation: origin.generation,
          canonicalObservationId: observation.id,
          slotId: candidate.slotId,
        },
      };
      const queued = await enqueueCanonicalMediaProcessingJob(queueCandidate, {
        db: tx,
        now: observation.observedAt,
      });
      if (!queued.eventId) {
        throw new CanonicalMediaRegistrationRejected(
          rejected("canonical_media_enqueue_rejected", [candidate.slotId]),
        );
      }
      eventIds.add(queued.eventId);
      if (queued.created) enqueued += 1;
      else replayed += 1;
    }
  }

  return {
    ok: true,
    mediaItemIds: [...mediaItemIds],
    originIds: [...originIds],
    eventIds: [...eventIds],
    enqueued,
    replayed,
  };
}

function bindObservedSlots(
  slots: Array<{ slotId: string; status: string; kind: string; ordinal: number }>,
  candidates: readonly VerifiedMediaCandidate[],
):
  | { ok: true; candidates: VerifiedMediaCandidate[]; platformContentId: string }
  | Extract<CanonicalMediaRegistrationResult, { ok: false }> {
  const candidateBySlot = new Map(candidates.map((candidate) => [candidate.slotId, candidate]));
  const rejectedSlots: string[] = [];
  for (const slot of slots) {
    const candidate = candidateBySlot.get(slot.slotId);
    if (slot.status !== "observed" || !candidate ||
        candidate.kind !== slot.kind || candidate.ordinal !== slot.ordinal) {
      rejectedSlots.push(slot.slotId);
    }
  }
  for (const candidate of candidates) {
    if (!slots.some((slot) => slot.slotId === candidate.slotId)) rejectedSlots.push(candidate.slotId);
  }
  if (rejectedSlots.length > 0 || slots.length !== candidates.length) {
    return rejected("canonical_media_slot_binding_mismatch", [...new Set(rejectedSlots)]);
  }
  const platformContentIds = new Set(candidates.map((candidate) => candidate.platformContentId));
  if (platformContentIds.size > 1) {
    return rejected("canonical_media_subject_identity_mismatch", candidates.map((candidate) => candidate.slotId));
  }
  return {
    ok: true,
    candidates: [...candidates],
    platformContentId: [...platformContentIds][0] ?? "",
  };
}

function identityItem(candidate: VerifiedMediaCandidate): SourceMediaIdentityItem | null {
  if (candidate.purpose === "cover" && candidate.kind === "image") {
    return { purpose: "source_cover", kind: "image", sourceUrl: candidate.observedAddress };
  }
  if (candidate.purpose === "body" && candidate.kind === "image") {
    return {
      purpose: "source_image",
      kind: "image",
      ordinal: candidate.ordinal,
      sourceUrl: candidate.observedAddress,
    };
  }
  if (candidate.purpose === "video" && candidate.kind === "video") {
    return { purpose: "source_video", kind: "video", sourceUrl: candidate.observedAddress };
  }
  return null;
}

function usageMatchesCandidate(
  usage: { purpose: SourceMediaIdentityItem["purpose"]; ordinal: number | null },
  candidate: VerifiedMediaCandidate,
): boolean {
  if (candidate.purpose === "cover") return usage.purpose === "source_cover" && usage.ordinal === null;
  if (candidate.purpose === "body") {
    return usage.purpose === "source_image" && usage.ordinal === candidate.ordinal;
  }
  return candidate.purpose === "video" && usage.purpose === "source_video" && usage.ordinal === null;
}

function processingRole(candidate: VerifiedMediaCandidate): "cover" | "video" | `image:${number}` {
  if (candidate.purpose === "cover") return "cover";
  if (candidate.purpose === "video") return "video";
  return `image:${candidate.ordinal}`;
}

function required(value: string, name: string): string {
  const normalized = value.trim();
  if (!normalized) throw new MediaArtifactInvariantError(`${name} is required.`);
  return normalized;
}

function rejected(
  reason: string,
  rejectedSlotIds: string[],
): Extract<CanonicalMediaRegistrationResult, { ok: false }> {
  return { ok: false, reason, rejectedSlotIds: [...new Set(rejectedSlotIds)].sort() };
}

function emptySuccess(): Extract<CanonicalMediaRegistrationResult, { ok: true }> {
  return {
    ok: true,
    mediaItemIds: [],
    originIds: [],
    eventIds: [],
    enqueued: 0,
    replayed: 0,
  };
}

function isRetryableTransactionConflict(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  const code = (error as { code?: unknown }).code;
  return code === "P2034" || code === "40001" || code === "40P01" ||
    error.message.includes("40001") || error.message.includes("40P01");
}

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}
