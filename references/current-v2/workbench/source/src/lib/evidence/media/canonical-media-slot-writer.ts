/** Dark CanonicalMediaSlot writer. No runtime caller is wired in this slice. */

import { Prisma, type PrismaClient } from "@/lib/prisma-client";

import {
  MediaArtifactInvariantError,
  readVerifiedMediaArtifacts,
  type VerifiedMediaArtifacts,
} from "./verified-artifact-reader";

const MAX_TRANSACTION_ATTEMPTS = 3;
const RETRY_DELAYS_MS = [10, 50] as const;

export type WriteSlotInput = {
  canonicalObservationId: string;
  verified: VerifiedMediaArtifacts;
};

export type WriteSlotResult = {
  inserted: number;
  replayed: number;
  slotIds: string[];
};

type SlotDatabase = Pick<PrismaClient, "$transaction">;

export class CanonicalMediaSlotWriter {
  constructor(private readonly db: SlotDatabase) {}

  async writeSlots(input: WriteSlotInput): Promise<WriteSlotResult> {
    const command = Object.freeze({
      canonicalObservationId: input.canonicalObservationId,
      evidence: readVerifiedMediaArtifacts(input.verified),
    });
    if (command.evidence.candidates.length === 0) {
      return { inserted: 0, replayed: 0, slotIds: [] };
    }

    for (let attempt = 0; attempt < MAX_TRANSACTION_ATTEMPTS; attempt += 1) {
      try {
        return await this.db.$transaction(
          (tx) => writeInTransaction(tx, command),
          { isolationLevel: Prisma.TransactionIsolationLevel.Serializable },
        );
      } catch (error) {
        if (!isRetryableTransactionConflict(error) || attempt === MAX_TRANSACTION_ATTEMPTS - 1) {
          throw error;
        }
        await sleep(RETRY_DELAYS_MS[attempt]);
      }
    }
    throw new MediaArtifactInvariantError("Canonical media transaction retries exhausted.");
  }
}

/** Reuses the exact writer core inside an existing SERIALIZABLE orchestration transaction. */
export async function writeCanonicalMediaSlotsInTransaction(
  tx: Prisma.TransactionClient,
  input: WriteSlotInput,
): Promise<WriteSlotResult> {
  return writeInTransaction(tx, Object.freeze({
    canonicalObservationId: input.canonicalObservationId,
    evidence: readVerifiedMediaArtifacts(input.verified),
  }));
}

async function writeInTransaction(
  tx: Prisma.TransactionClient,
  command: {
    canonicalObservationId: string;
    evidence: ReturnType<typeof readVerifiedMediaArtifacts>;
  },
): Promise<WriteSlotResult> {
  const observation = await tx.canonicalObservation.findFirst({
    where: {
      id: command.canonicalObservationId,
      workspaceId: command.evidence.workspaceId,
      rawSnapshotId: command.evidence.rawSnapshotId,
    },
    select: {
      id: true,
      observationKind: true,
      subjectKey: true,
    },
  });
  if (!observation) {
    throw new MediaArtifactInvariantError("CanonicalObservation is not bound to the verified Evidence.");
  }

  if (observation.observationKind !== "note") {
    throw new MediaArtifactInvariantError("CanonicalObservation kind cannot consume note media.");
  }
  const subjectCandidates = command.evidence.candidates.filter((candidate) =>
    observation.subjectKey === `xhs:note:${encodeURIComponent(candidate.noteId)}`,
  );
  if (subjectCandidates.length === 0) return { inserted: 0, replayed: 0, slotIds: [] };

  let inserted = 0;
  let replayed = 0;
  const slotIds: string[] = [];
  for (const candidate of subjectCandidates) {
    const created = await tx.canonicalMediaSlot.createMany({
      data: [{
        workspaceId: command.evidence.workspaceId,
        canonicalObservationId: observation.id,
        slotId: candidate.slotId,
        status: "observed",
        kind: candidate.kind,
        ordinal: candidate.ordinal,
      }],
      skipDuplicates: true,
    });
    const row = await tx.canonicalMediaSlot.findUniqueOrThrow({
      where: {
        workspaceId_canonicalObservationId_slotId: {
          workspaceId: command.evidence.workspaceId,
          canonicalObservationId: observation.id,
          slotId: candidate.slotId,
        },
      },
      select: { id: true, status: true, kind: true, ordinal: true },
    });
    if (row.status !== "observed" || row.kind !== candidate.kind || row.ordinal !== candidate.ordinal) {
      throw new MediaArtifactInvariantError("CanonicalMediaSlot replay fields changed.");
    }
    inserted += created.count;
    replayed += created.count === 0 ? 1 : 0;
    slotIds.push(row.id);
  }
  return { inserted, replayed, slotIds };
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
