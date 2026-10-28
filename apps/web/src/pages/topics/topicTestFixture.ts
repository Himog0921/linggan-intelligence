import type { TopicProgress } from './topicResearchModel';
/** Handwritten synthetic UI contract fixtures; no copied source material. */
import type {
  Counts,
  Journey,
  Snapshot,
  Statistics,
  Work,
} from "./types";
export const topicDomain = "10000000-0000-4000-8000-000000000001";
export const topicRef = "20000000-0000-4000-8000-000000000001";
export const definitionRef = "30000000-0000-4000-8000-000000000001";
export function topicWork(index = 0): Work {
  return {
    workRef: `40000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
    platform: "xhs",
    contentExternalId: `synthetic-${index}`,
    title: `合成材料 ${index}`,
    titleSource: "detail",
    authorExternalId: `synthetic-author-${index}`,
    creatorDisplayName: `合成作者 ${index}`,
    publishedAt: "2026-10-14T00:00:00Z",
    publishedAtSourceText: null,
    publishedAtPrecision: "second",
    likes: index === 0 ? 0 : null,
    collects: null,
    comments: null,
    shares: null,
    followerCount: null,
    own: false,
    ownBreakout: false,
    readable: true,
    usageRoles: ["primary"],
    topicRefs: [topicRef],
    mainStage: "pending",
    involvedStages: [],
    overlays: [],
    path: "family",
    annotation: null,
    research: null,
    media: {},
    preview: { text: "仅用于界面验证的合成文字" },
    evidenceFragment: null,
  };
}
export function topicStats(count = 2): Statistics {
  return {
    workCount: count,
    authorCount: count,
    unknownAuthorCount: 0,
    ownBreakoutCount: 0,
    ownPublishedCount: 0,
    ownPublicationUnknownCount: 0,
    ownHistoryPublishedCount: 0,
    ownHistoryBreakoutCount: 0,
    recentLowFollowerHighLikeCount: 0,
    platforms: [
      {
        platform: "xhs",
        workCount: count,
        knownLikeCount: count ? 1 : 0,
        unknownLikeCount: Math.max(0, count - 1),
        highPerformanceThreshold: null,
        highPerformanceCount: null,
        medianLikes: count ? 0 : null,
        p90Likes: count ? 0 : null,
        highPerformanceRuleVersion: null,
      },
    ],
  };
}
export function topicJourney(works: Work[]): Journey {
  return {
    denominator: works.length,
    main: [
      {
        stage: "pending",
        count: works.length,
        workRefs: works.map((w) => w.workRef),
      },
    ],
    involved: [],
    overlays: [],
    involvedMayOverlap: true,
  };
}
export function topicSnapshot(works = [topicWork(0), topicWork(1)]): Snapshot {
  const statistics = topicStats(works.length),
    journey = topicJourney(works);
  return {
    domains: [
      {
        domainRef: topicDomain,
        name: "合成主题领域",
        description: null,
        status: "active",
        researchGoal: null,
      },
    ],
    topics: [
      {
        topicRef,
        canonicalKey: "synthetic-topic",
        domainRef: topicDomain,
        definitionRef,
        definitionVersion: 1,
        displayName: "合成主题",
        definitionText: "明确合成材料边界",
        lifecycleState: "provisional",
        parentTopicRef: null,
        bindingVersion: 1,
        directWorkRefs: works.map((w) => w.workRef),
        workRefs: works.map((w) => w.workRef),
        statistics,
        journey,
        core: null,
      },
    ],
    works,
    statistics,
    journey,
    sources: works.map((w) => ({
      authorExternalId: w.authorExternalId,
      platform: w.platform,
      creatorDisplayName: w.creatorDisplayName,
      workCount: 1,
      statistics: topicStats(1),
      topicRefs: [topicRef],
      workRefs: [w.workRef],
    })),
    candidates: [],
    changes: [],
    alternatives: [],
    ownCreators: [],
    methodVersion: "synthetic-method.v1",
    sourceBoundary: "合成来源，不能外推",
    scope: {
      domainRef: topicDomain,
      topicRef: null,
      totalWorkCount: works.length,
      readableWorkCount: works.length,
      unclassifiedWorkCount: 0,
      asOf: "2026-10-16T00:00:00Z",
      ownIdentityState: "unknown",
      ownHistoryPublishedCount: 0,
      ownHistoryPublicationUnknownCount: 0,
      ownHistoricalWorkRefs: [],
      recentReferenceWorkRefs: works.map((w) => w.workRef),
      loadedWorkCount: works.length,
      canonicalWorkCount: works.length,
    },
  };
}
export function topicCounts(): Counts {
  return {
    correctiveQueuedCount: 0,
    correctiveRunningCount: 0,
    reviewQueuedCount: 0,
    reviewRunningCount: 0,
    queuedCount: 0,
    runningCount: 0,
    succeededCount: 0,
    noSignalCount: 0,
    insufficientCount: 0,
    failedCount: 0,
    unknownDispatchCount: 0,
    extractedWindowCount: 0,
    staleCount: 0,
    stoppedCount: 0,
    initialQueuedCount: 0,
    completedInitialWindowCount: 0,
    coveredWorkCount: 0,
    reassessmentQueuedCount: 0,
    reassessmentCompletedCount: 0,
    totalRunCount: 0,
    totalTaskCount: 0,
    phases: {
      extracting: 0,
      extractQueued: 0,
      resolving: 0,
      resolveQueued: 0,
      comparing: 0,
      compareQueued: 0,
    },
  };
}
export function topicProgress(): TopicProgress { return { summary: {pendingStageCount:0,succeededStageCount:0,failedStageCount:0},runs:[] }; }
export const topicContext = {
  defaultDomainRef: topicDomain,
  domains: [
    {
      domainRef: topicDomain,
      name: "合成主题领域",
      status: "active",
      targetCount: 0,
      sampleCount: 2,
    },
  ],
};
export function jsonResponse(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}
