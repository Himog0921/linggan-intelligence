/**
 * V2 Content Projection Read Service.
 *
 * Every database fact is read inside one repeatable-read transaction. The
 * service never consults ContentAsset display columns and never manufactures
 * timestamps, URLs, media relations, or fallback values.
 */
import { Prisma, type PrismaClient } from "@/lib/prisma-client";
import { getSafeHttpUrl } from "@/lib/safe-url";
import {
  deliveryUrlForSelectedReplica,
  evaluateMediaItemDelivery,
  mediaItemDeliverySelect,
} from "@/lib/services/media-library-read-model";
import { verifyLocalMediaBlob } from "@/lib/services/local-media-storage";

export interface ContentProjectionReadInput {
  workspaceId: string;
  platform: "xhs";
  platformContentId: string;
}

export interface V2MediaUsageRef {
  purpose: string;
  canonicalSlotId: string;
  canonicalObservationId: string;
  mediaItemId: string;
  mediaOriginId: string;
  originGeneration: number;
  ordinal: number | null;
  deliveryState: "ready" | "processing" | "failed" | "unavailable";
  deliveryUrl: string | null;
}

export interface ContentProjectionDTO {
  schemaVersion: "content-projection/v2";
  contentAssetId: string;
  platform: "xhs";
  platformContentId: string;
  workspaceId: string;
  projectionVersion: number;
  visibilityState: "visible";
  title: string | null;
  bodyText: string | null;
  contentType: string | null;
  publishedAt: string | null;
  originalUrl: string | null;
  cover: { status: "available"; url: string; mediaItemId: string } | { status: "unavailable"; reason: string };
  mediaUsages: V2MediaUsageRef[];
  lastObservedAt: string;
  source: "v2";
}

export interface ContentProjectionReadResult {
  status: "available" | "unavailable";
  projection?: ContentProjectionDTO;
  reason?: string;
}

const projectionInclude = {
  contentAsset: { select: { platform: true, platformContentId: true } },
  observation: {
    select: {
      id: true,
      canonicalObservationId: true,
      rawSnapshotId: true,
      title: true,
      bodyText: true,
      contentType: true,
      publishedAt: true,
      originalUrl: true,
      observedAt: true,
    },
  },
  contractEvaluation: {
    select: {
      id: true,
      rawSnapshotId: true,
      contractId: true,
      contractVersion: true,
      decision: true,
      currentPointers: {
        select: { contractEvaluationId: true },
      },
      inputs: {
        select: { canonicalObservationId: true },
      },
    },
  },
} satisfies Prisma.ContentCurrentProjectionInclude;

const usageInclude = {
  processingEvent: { select: { eventType: true, workspaceId: true, payload: true } },
  mediaItem: { select: mediaItemDeliverySelect },
} satisfies Prisma.ContentMediaUsageInclude;

type ProjectionRow = Prisma.ContentCurrentProjectionGetPayload<{ include: typeof projectionInclude }>;
type UsageRow = Prisma.ContentMediaUsageGetPayload<{ include: typeof usageInclude }>;

export class ContentProjectionReadService {
  constructor(private readonly db: PrismaClient) {}

  async read(rawInput: ContentProjectionReadInput): Promise<ContentProjectionReadResult> {
    const input = validateInput(rawInput);
    if (!input) return { status: "unavailable", reason: "invalid_content_identity" };

    return this.db.$transaction(
      async (tx) => {
        const projection = await tx.contentCurrentProjection.findFirst({
          where: {
            workspaceId: input.workspaceId,
            visibilityState: "visible",
            contentAsset: {
              platform: input.platform,
              platformContentId: input.platformContentId,
            },
          },
          include: projectionInclude,
        });
        if (!projection) return unavailable("no_visible_v2_projection");
        if (!isCurrentAcceptedProjection(projection)) {
          return unavailable("evaluation_not_current_accepted");
        }

        const usages = await tx.contentMediaUsage.findMany({
          where: {
            workspaceId: input.workspaceId,
            contentAssetId: projection.contentAssetId,
            contractEvaluationId: projection.contractEvaluationId,
            canonicalObservationId: projection.observation.canonicalObservationId,
            validTo: null,
          },
          include: usageInclude,
          orderBy: [{ purpose: "asc" }, { ordinal: "asc" }, { id: "asc" }],
        });
        const media = await readMedia(usages, projection);
        if (!media.ok) return unavailable(media.reason);

        const originalUrl = projection.observation.originalUrl === null
          ? null
          : getSafeHttpUrl(projection.observation.originalUrl);
        if (projection.observation.originalUrl !== null && !originalUrl) {
          return unavailable("invalid_original_url");
        }

        const coverCandidate = media.items.find((item) =>
          item.purpose === "source_cover" && item.deliveryState === "ready" && item.deliveryUrl,
        ) ?? media.items.find((item) =>
          item.purpose === "source_image" && item.deliveryState === "ready" && item.deliveryUrl,
        );

        return {
          status: "available" as const,
          projection: {
            schemaVersion: "content-projection/v2" as const,
            contentAssetId: projection.contentAssetId,
            platform: "xhs" as const,
            platformContentId: projection.contentAsset.platformContentId,
            workspaceId: projection.workspaceId,
            projectionVersion: projection.projectionVersion,
            visibilityState: "visible" as const,
            title: projection.observation.title,
            bodyText: projection.observation.bodyText,
            contentType: projection.observation.contentType,
            publishedAt: projection.observation.publishedAt?.toISOString() ?? null,
            originalUrl,
            cover: coverCandidate?.deliveryUrl
              ? { status: "available" as const, url: coverCandidate.deliveryUrl, mediaItemId: coverCandidate.mediaItemId }
              : { status: "unavailable" as const, reason: media.items.length === 0 ? "no_observed_media" : "no_deliverable_cover" },
            mediaUsages: media.items,
            lastObservedAt: projection.observation.observedAt.toISOString(),
            source: "v2" as const,
          },
        };
      },
      { isolationLevel: Prisma.TransactionIsolationLevel.RepeatableRead },
    );
  }
}

function validateInput(input: ContentProjectionReadInput): ContentProjectionReadInput | null {
  if (!input || input.platform !== "xhs") return null;
  if (!nonEmpty(input.workspaceId) || !nonEmpty(input.platformContentId)) return null;
  return {
    workspaceId: input.workspaceId.trim(),
    platform: "xhs",
    platformContentId: input.platformContentId.trim(),
  };
}

function isCurrentAcceptedProjection(projection: ProjectionRow): boolean {
  const evaluation = projection.contractEvaluation;
  return evaluation.decision === "accepted"
    && evaluation.rawSnapshotId === projection.observation.rawSnapshotId
    && evaluation.currentPointers.length === 1
    && evaluation.currentPointers[0]?.contractEvaluationId === evaluation.id
    && evaluation.inputs.filter(
      (input) => input.canonicalObservationId === projection.observation.canonicalObservationId,
    ).length === 1;
}

async function readMedia(
  rows: UsageRow[],
  projection: ProjectionRow,
): Promise<{ ok: true; items: V2MediaUsageRef[] } | { ok: false; reason: string }> {
  const items: V2MediaUsageRef[] = [];
  for (const row of rows) {
    if (!hasCompleteProvenance(row, projection)) {
      return { ok: false, reason: "incomplete_media_provenance" };
    }
    const delivery = await evaluateMediaItemDelivery(
      row.mediaItem,
      (pathname, expectedByteSize) => verifyLocalMediaBlob({ pathname, expectedByteSize }),
    );
    items.push({
      purpose: row.purpose,
      canonicalSlotId: row.canonicalSlotId,
      canonicalObservationId: row.canonicalObservationId,
      mediaItemId: row.mediaItemId,
      mediaOriginId: row.mediaOriginId,
      originGeneration: row.originGeneration,
      ordinal: row.ordinal,
      deliveryState: delivery.deliveryState,
      deliveryUrl: deliveryUrlForSelectedReplica(delivery.selected, delivery.available),
    });
  }
  return { ok: true, items };
}

function hasCompleteProvenance(row: UsageRow, projection: ProjectionRow): row is UsageRow & {
  contractEvaluationId: string;
  canonicalObservationId: string;
  canonicalSlotId: string;
  mediaOriginId: string;
  originGeneration: number;
  mediaProcessingEventId: string;
} {
  if (!row.contractEvaluationId || !row.canonicalObservationId || !row.canonicalSlotId
    || !row.mediaOriginId || row.originGeneration === null || !row.mediaProcessingEventId) {
    return false;
  }
  if (row.contractEvaluationId !== projection.contractEvaluationId
    || row.canonicalObservationId !== projection.observation.canonicalObservationId) {
    return false;
  }
  const event = row.processingEvent;
  if (!event || event.eventType !== "media.processing_requested" || event.workspaceId !== row.workspaceId) {
    return false;
  }
  const payload = record(event.payload);
  const origin = record(payload?.ledgerOrigin);
  return origin?.canonicalObservationId === row.canonicalObservationId
    && origin?.slotId === row.canonicalSlotId
    && origin?.originId === row.mediaOriginId
    && origin?.mediaItemId === row.mediaItemId
    && origin?.generation === row.originGeneration;
}

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null;
}

function nonEmpty(value: unknown): value is string {
  return typeof value === "string" && value.trim().length > 0;
}

function unavailable(reason: string): ContentProjectionReadResult {
  return { status: "unavailable", reason };
}
