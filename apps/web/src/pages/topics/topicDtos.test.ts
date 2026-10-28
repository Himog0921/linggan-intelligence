import { parseTopicProgress } from './topicResearchModel';
import { describe, expect, it } from "vitest";
import { ApiError } from "../../shared/api";
import {
  decodeAlternative,
  decodeCommand,
  decodeComments,
  decodeEvidence,
  decodeSnapshot,
} from "./topicDtos";
import { topicProgress, topicSnapshot, topicWork } from "./topicTestFixture";
describe("Topic JSON runtime boundary", () => {
  it("preserves unknown and zero separately and all canonical refs", () => {
    const value = decodeSnapshot(topicSnapshot());
    expect(value.works[0]?.likes).toBe(0);
    expect(value.works[1]?.likes).toBeNull();
    expect(value.statistics.platforms[0]?.highPerformanceThreshold).toBeNull();
    expect(value.topics[0]?.workRefs).toEqual(
      value.works.map((w) => w.workRef),
    );
  });
  it.each(["works", "scope", "statistics", "topics", "methodVersion"])(
    "rejects missing %s without inferred defaults",
    (key) => {
      const body: Record<string, unknown> = { ...topicSnapshot() };
      delete body[key];
      expect(() => decodeSnapshot(body)).toThrow(ApiError);
    },
  );
  it.each([null, {}, "2", false, NaN])(
    "rejects invalid work array/count %s",
    (value) => {
      const body = { ...topicSnapshot(), works: value };
      expect(() => decodeSnapshot(body)).toThrow(ApiError);
    },
  );
  it("requires explicit nullable numeric fields rather than silently writing zero", () => {
    const w: Record<string, unknown> = { ...topicWork() };
    delete w.likes;
    expect(() => decodeEvidence({ works: [w] })).toThrow(ApiError);
    expect(() =>
      decodeEvidence({ works: [{ ...topicWork(), likes: "0" }] }),
    ).toThrow(ApiError);
  });
  it("requires current workflow counts without historical defaults", () => {
    const body: Record<string, unknown> = { ...topicProgress() };
    delete body.summary;
    expect(() => parseTopicProgress(body)).toThrow(ApiError);
  });
  it("accepts optional parent context while keeping an exact empty comments set", () => {
    expect(
      decodeComments({
        items: [{ body: null, sourceRef: "synthetic" }],
        total: 1,
        nextCursor: null,
      }).items[0]?.parentText,
    ).toBeNull();
    expect(
      decodeComments({ items: [], total: 0, nextCursor: null }).total,
    ).toBe(0);
    expect(() =>
      decodeComments({ items: [], total: null, nextCursor: null }),
    ).toThrow(ApiError);
  });
  it("does not treat an empty 200 response as a persisted command", () => {
    expect(() => decodeCommand({})).toThrow(ApiError);
  });
});

it("decodes a real search-round shape without manufacturing collection kind or reason fields", async () => {
  const { decodeSearch } = await import("./topicDtos");
  const result = decodeSearch({
    domainRef: "domain",
    authorizations: [],
    rounds: [
      {
        roundRef: "round",
        domainRef: "domain",
        state: "partial",
        phase: "details",
        searches: [{ keyword: "合成词" }],
        candidateWorkRefs: ["work"],
        candidateCount: 1,
        detailSlotsReserved: 1,
        actualDetailsAcquired: 0,
        pendingDetailWorkRefs: ["work"],
        candidateLimit: 200,
      },
    ],
  });
  expect(result.rounds[0]?.state).toBe("partial");
  expect(result.rounds[0]?.actualDetailsAcquired).toBe(0);
});

const restrictedAlternative = {
  alternativeRef: "60000000-0000-4000-8000-000000000001",
  topicRef: null,
  definitionRef: null,
  sourceState: "source_unavailable",
  title: null,
  angle: null,
  rationale: null,
  fragments: [],
  originalDefinition: null,
  methodVersion: "topic-map.manual.v1",
};
it("accepts explicitly unavailable saved text without rejecting unrelated materials", () => {
  const decoded = decodeSnapshot({
    ...topicSnapshot(),
    alternatives: [restrictedAlternative],
  });
  expect(decoded.works).toHaveLength(2);
  expect(decoded.alternatives[0]).toMatchObject({
    sourceState: "source_unavailable",
    title: null,
    angle: null,
    rationale: null,
  });
  expect(decodeAlternative(restrictedAlternative)).toMatchObject({
    sourceState: "source_unavailable",
    title: null,
  });
});
it.each(["title", "angle", "rationale"])(
  "still rejects absent saved %s rather than inferring null",
  (key) => {
    const body: Record<string, unknown> = { ...restrictedAlternative };
    delete body[key];
    expect(() => decodeAlternative(body)).toThrow(ApiError);
  },
);
