import { describe, expect, it, vi } from "vitest";

import { ContentProjectionReadService } from "./content-projection-read-service";

describe("ContentProjectionReadService", () => {
  it("reads one exact accepted revision in a repeatable-read transaction", async () => {
    const fixture = dbFixture();
    const result = await new ContentProjectionReadService(fixture.db as never).read({
      workspaceId: "workspace-1",
      platform: "xhs",
      platformContentId: "note-1",
    });

    expect(result).toEqual({
      status: "available",
      projection: expect.objectContaining({
        schemaVersion: "content-projection/v2",
        title: "V2 title",
        bodyText: "V2 body",
        originalUrl: "https://www.xiaohongshu.com/explore/note-1",
        lastObservedAt: "2026-08-12T01:00:00.000Z",
        cover: { status: "unavailable", reason: "no_observed_media" },
      }),
    });
    expect(fixture.transaction).toHaveBeenCalledWith(
      expect.any(Function),
      expect.objectContaining({ isolationLevel: "RepeatableRead" }),
    );
  });

  it("rejects an accepted evaluation that is no longer current", async () => {
    const fixture = dbFixture({ currentEvaluationId: "eval-new" });
    await expect(new ContentProjectionReadService(fixture.db as never).read({
      workspaceId: "workspace-1", platform: "xhs", platformContentId: "note-1",
    })).resolves.toEqual({ status: "unavailable", reason: "evaluation_not_current_accepted" });
  });

  it("accepts the one projected note when the same evaluation also contains comment inputs", async () => {
    const projection = projectionRow();
    projection.contractEvaluation.inputs.push(
      { canonicalObservationId: "canonical-comment-1" },
      { canonicalObservationId: "canonical-comment-2" },
    );
    const fixture = dbFixture({ projection });

    await expect(new ContentProjectionReadService(fixture.db as never).read({
      workspaceId: "workspace-1", platform: "xhs", platformContentId: "note-1",
    })).resolves.toMatchObject({ status: "available", projection: { title: "V2 title" } });
  });

  it("rejects an invalid observed URL instead of synthesizing or falling back", async () => {
    const fixture = dbFixture({ originalUrl: "javascript:alert(1)" });
    await expect(new ContentProjectionReadService(fixture.db as never).read({
      workspaceId: "workspace-1", platform: "xhs", platformContentId: "note-1",
    })).resolves.toEqual({ status: "unavailable", reason: "invalid_original_url" });
  });

  it("returns unavailable when V2 has no visible current and never asks for V1 fields", async () => {
    const fixture = dbFixture({ projection: null });
    await expect(new ContentProjectionReadService(fixture.db as never).read({
      workspaceId: "workspace-1", platform: "xhs", platformContentId: "note-1",
    })).resolves.toEqual({ status: "unavailable", reason: "no_visible_v2_projection" });
    expect(fixture.tx.contentMediaUsage.findMany).not.toHaveBeenCalled();
  });
});

function dbFixture(overrides: {
  projection?: ReturnType<typeof projectionRow> | null;
  currentEvaluationId?: string;
  originalUrl?: string | null;
} = {}) {
  const projection = overrides.projection === null
    ? null
    : overrides.projection ?? projectionRow(overrides);
  const tx = {
    contentCurrentProjection: { findFirst: vi.fn(async () => projection) },
    contentMediaUsage: { findMany: vi.fn(async () => []) },
  };
  const transaction = vi.fn(async (callback: (value: typeof tx) => unknown) => callback(tx));
  return { db: { $transaction: transaction }, tx, transaction };
}

function projectionRow(overrides: { currentEvaluationId?: string; originalUrl?: string | null } = {}) {
  return {
    contentAssetId: "asset-1",
    workspaceId: "workspace-1",
    currentObservationId: "content-observation-1",
    contractEvaluationId: "eval-1",
    projectionVersion: 3,
    visibilityState: "visible",
    contentAsset: { platform: "xhs", platformContentId: "note-1" },
    observation: {
      id: "content-observation-1",
      canonicalObservationId: "canonical-1",
      rawSnapshotId: "snapshot-1",
      title: "V2 title",
      bodyText: "V2 body",
      contentType: "note",
      publishedAt: null,
      originalUrl: overrides.originalUrl === undefined
        ? "https://www.xiaohongshu.com/explore/note-1"
        : overrides.originalUrl,
      observedAt: new Date("2026-08-12T01:00:00.000Z"),
    },
    contractEvaluation: {
      id: "eval-1",
      rawSnapshotId: "snapshot-1",
      contractId: "xhs.note-detail/v2",
      contractVersion: 2,
      decision: "accepted",
      currentPointers: [{ contractEvaluationId: overrides.currentEvaluationId ?? "eval-1" }],
      inputs: [{ canonicalObservationId: "canonical-1" }],
    },
  };
}
