import { describe, expect, it } from "vitest";
import { ApiError } from "../../shared/api";
import { workspace } from "./workspaceApi";
import { workspaceFixture } from "./workspaceTestFixture";

type Fixture = ReturnType<typeof workspaceFixture>;
const invalidCases: [string, (value: Fixture) => unknown][] = [
  [
    "missing title",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        workResource: {
          ...item.workResource,
          display: {
            creatorDisplayName: null,
            publishedAt: null,
            publishedAtSourceText: null,
          },
        },
      })),
    }),
  ],
  [
    "wrong title type",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        workResource: {
          ...item.workResource,
          display: { ...item.workResource.display, title: 0 },
        },
      })),
    }),
  ],
  [
    "fractional version",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        definition: { ...value.topic.definition, version: 1.2 },
      },
    }),
  ],
  [
    "negative version",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        definition: { ...value.topic.definition, version: -1 },
      },
    }),
  ],
  [
    "unknown run kind",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        classificationRun: {
          ...value.topic.classificationRun,
          runKind: "unrecognized",
        },
      },
    }),
  ],
  [
    "unqualified run",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        classificationRun: {
          ...value.topic.classificationRun,
          runState: "running",
        },
      },
    }),
  ],
  [
    "partial pack",
    (value) => ({ ...value, materials: value.materials.slice(1) }),
  ],
  [
    "duplicate frozen member",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        members: [value.topic.members[0], value.topic.members[0]],
      },
    }),
  ],
  [
    "wrong resource identity",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        workResource: {
          ...item.workResource,
          identity: {
            ...item.workResource.identity,
            publicRef: "30000000-0000-4000-8000-000000000001",
          },
        },
      })),
    }),
  ],
  [
    "external detail link",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        detailUrl: "javascript:alert(1)",
      })),
    }),
  ],
  [
    "wrong role",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        classification: { ...item.classification, role: "automatic" },
      })),
    }),
  ],
  [
    "rationale outside frozen member",
    (value) => ({
      ...value,
      materials: value.materials.map((item) => ({
        ...item,
        classification: { ...item.classification, rationale: "other" },
      })),
    }),
  ],
  [
    "missing source boundary",
    (value) => ({
      ...value,
      topic: {
        ...value.topic,
        materialPack: {
          materialPackRef: value.topic.materialPack.materialPackRef,
        },
      },
    }),
  ],
];
describe("Topic workspace composed read contract", () => {
  it("preserves nullable source fields and exact classification identities", () => {
    const decoded = workspace(workspaceFixture(), "workspace-proof");
    expect(decoded.topic.definition.version).toBe(2);
    expect(decoded.materials[0]).toMatchObject({
      title: null,
      creator: null,
      publishedAt: null,
      publishedAtSourceText: null,
      role: "support",
      ordinal: 1,
    });
    expect(decoded.materials[1]?.publishedAtSourceText).toBe(
      "平台原始相对时间",
    );
  });
  it("reads existing completed machine-proposed candidate workspaces", () => {
    const value = workspaceFixture("candidate-10000000000040008000000000000001");
    value.topic.definition.lifecycleState = "candidate";
    value.topic.classificationRun.runKind = "machine_proposed";
    const decoded = workspace(value, value.topic.canonicalKey);
    expect(decoded.materials).toHaveLength(3);
    expect(decoded.topic.definition.displayName).toBe(value.topic.definition.displayName);
  });
  it("accepts zero members and additional unused fields", () => {
    const value = workspaceFixture();
    expect(
      workspace({
        ...value,
        topic: { ...value.topic, members: [], extra: "ignored" },
        materials: [],
      }).materials,
    ).toEqual([]);
  });
  it.each(invalidCases)(
    "rejects %s instead of rendering a partial/stale pack",
    (_name, change) => {
      expect(() =>
        workspace(change(workspaceFixture()), "workspace-proof"),
      ).toThrow(ApiError);
    },
  );
  it("rejects a valid body for a different canonical key", () => {
    expect(() =>
      workspace(workspaceFixture("other-workspace"), "workspace-proof"),
    ).toThrow(ApiError);
  });
});
