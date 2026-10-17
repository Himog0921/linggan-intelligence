import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  applyQualifiedReader,
  bodyCitations,
  citationText,
  discussionUnits,
  highlighted,
  locatedCitations,
  sharesDiscussion,
  unreadResearchWork,
} from "./topicModel";
import { topicWork } from "./topicTestFixture";
import type { Reader, Work, WorkResearch } from "./types";
function research(unitId = "unit", topic = "topic-a"): WorkResearch {
  return {
    readable: true,
    authorSourceState: "available",
    commentSourceState: "available",
    fragments: [
      {
        fragmentId: "fragment",
        field: "body",
        text: "甲🧭乙丙",
        start: 10,
        workRef: topicWork().workRef,
      },
    ],
    output: {
      discussions: [],
      scenes: [],
      angles: [],
      productOpportunities: [],
      responseMatches: [],
      limitations: [],
    },
    core: {
      units: [
        {
          unitId,
          label: "同名讨论",
          status: "matched",
          assignments: [{ topicRef: topic, definitionRef: "definition" }],
          evidence: [{ fragmentId: "fragment", start: 11, end: 13 }],
        },
      ],
    },
  };
}
describe("Topic source and canonical identity algorithms", () => {
  it("matches discussion by canonical ref and never by same label", () => {
    const a = { ...topicWork(), research: research("a", "topic-a") },
      b = { ...topicWork(1), research: research("b", "topic-b") };
    expect(sharesDiscussion(a, b)).toBe(false);
    b.research = research("b", "topic-a");
    expect(sharesDiscussion(a, b)).toBe(true);
  });
  it("deduplicates unit and assignment identity while preserving explicit no-signal core", () => {
    const r = research();
    if (!r.core?.units) throw Error("fixture");
    const u = r.core.units[0];
    if (!u) throw Error("fixture");
    r.core.units.push(u);
    u.assignments?.push({
      topicRef: "topic-a",
      definitionRef: "different-label",
    });
    expect(discussionUnits({ ...topicWork(), research: r })).toHaveLength(1);
    expect(
      discussionUnits({ ...topicWork(), research: r })[0]?.assignments,
    ).toHaveLength(1);
    r.core.units = [];
    r.output.discussions = [{ topicRef: "stale", evidence: [] }];
    expect(discussionUnits({ ...topicWork(), research: r })).toEqual([]);
  });
  it("uses Unicode codepoint offsets, rejects invalid spans and deduplicates citations", () => {
    const w = { ...topicWork(), research: research() };
    const citations = [
      { fragmentId: "fragment", start: 11, end: 13 },
      { fragmentId: "fragment", start: 11, end: 13 },
      { fragmentId: "fragment", start: 9, end: 12 },
      { fragmentId: "fragment", start: 12, end: 99 },
    ];
    expect(citationText(w, citations)).toBe("🧭乙");
    expect(locatedCitations(w, citations)).toHaveLength(1);
    expect(
      renderToStaticMarkup(
        highlighted("甲🧭乙丙", [
          { start: 1, end: 3 },
          { start: 2, end: 4 },
        ]),
      ),
    ).toBe("甲<mark>🧭乙丙</mark>");
  });
  it("never highlights another work or text-mismatched source as this body", () => {
    const w = { ...topicWork(), research: research() };
    const f = w.research.fragments[0];
    if (!f) throw Error("fixture");
    f.fragmentId = `${w.workRef}.body.fragment`;
    f.start = 0;
    const unit = w.research.core?.units?.[0];
    if (!unit) throw Error("fixture");
    unit.evidence = [{ fragmentId: f.fragmentId, start: 1, end: 3 }];
    expect(bodyCitations(w, "甲🧭乙丙")).toEqual([{ start: 1, end: 3 }]);
    f.workRef = topicWork(1).workRef;
    expect(bodyCitations(w, "甲🧭乙丙")).toEqual([]);
    f.workRef = w.workRef;
    expect(bodyCitations(w, "不同正文")).toEqual([]);
  });
  it.each(["null", "missing", "rejected"])(
    "removes stale research while safe body and comments remain independently readable: %s",
    (mode) => {
      const stale = {
        ...topicWork(),
        research: research(),
        annotation: { rationale: "STALE" },
      };
      const reader: Reader = {
        work: unreadResearchWork(stale),
        domainRef: "domain",
        loading: true,
      };
      const current: Work = { ...stale, research: null };
      const evidence: PromiseSettledResult<{ works: Work[] }> =
        mode === "rejected"
          ? { status: "rejected", reason: new Error("not qualified") }
          : {
              status: "fulfilled",
              value: { works: mode === "missing" ? [] : [current] },
            };
      applyQualifiedReader(reader, [
        {
          status: "fulfilled",
          value: {
            item: {
              inspector: { detailCurrent: { body: { value: "safe source" } } },
            },
          },
        },
        {
          status: "fulfilled",
          value: {
            items: [
              { body: "safe comment", sourceRef: "comment", parentText: null },
            ],
            total: 1,
            nextCursor: null,
          },
        },
        evidence,
      ]);
      expect(reader.work.research).toBeNull();
      expect(reader.work.annotation).toBeNull();
      expect(reader.resource?.item.inspector?.detailCurrent?.body?.value).toBe(
        "safe source",
      );
      expect(reader.commentResource?.items[0]?.body).toBe("safe comment");
    },
  );
  it("preserves independently qualified comment-only research after author body failure", () => {
    const r = {
      ...research(),
      authorSourceState: "source_unavailable",
      commentSourceState: "available",
    };
    const current = { ...topicWork(), readable: false, research: r };
    const reader: Reader = {
      work: unreadResearchWork(current),
      domainRef: "domain",
      loading: true,
    };
    applyQualifiedReader(reader, [
      { status: "rejected", reason: new Error("body unavailable") },
      { status: "fulfilled", value: { items: [], total: 0, nextCursor: null } },
      { status: "fulfilled", value: { works: [current] } },
    ]);
    expect(reader.work.research).toEqual(r);
    expect(reader.resource).toBeNull();
  });
});
