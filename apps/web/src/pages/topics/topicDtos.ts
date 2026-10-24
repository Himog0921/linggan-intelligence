import { isRecord } from "../../shared/api";
import * as d from "../../shared/decode";
import type * as T from "./types";

type Json = d.Json;
const nullable = <V>(
  row: Json,
  key: string,
  parse: (value: unknown) => V,
): V | null => (d.field(row, key) === null ? null : parse(d.field(row, key)));
const text = (value: unknown): string => {
  if (typeof value !== "string") throw new d.ShapeError("string");
  return value;
};
const numeric = (value: unknown): number => {
  if (typeof value !== "number" || !Number.isFinite(value))
    throw new d.ShapeError("number");
  return value;
};
const object = (value: unknown): Json => {
  if (!isRecord(value)) throw new d.ShapeError("object");
  return value;
};
const strings = (row: Json, key: string): string[] => {
  const value = d.field(row, key);
  if (!Array.isArray(value)) throw new d.ShapeError(key);
  return value.map(text);
};
const optionalStrings = (row: Json, key: string) =>
  key in row ? strings(row, key) : undefined;
const optionalText = (row: Json, key: string) =>
  key in row && d.field(row, key) !== null
    ? text(d.field(row, key))
    : undefined;
const optionalNumber = (row: Json, key: string) =>
  key in row ? numeric(d.field(row, key)) : undefined;
const optionalList = <V>(row: Json, key: string, parse: (row: Json) => V) =>
  key in row ? d.list(row, key, parse) : undefined;
const optionalObject = <V>(row: Json, key: string, parse: (row: Json) => V) =>
  key in row && d.field(row, key) !== null ? parse(d.obj(row, key)) : undefined;
export const citation = (row: Json): T.Citation => ({
  fragmentId: d.str(row, "fragmentId"),
  start: d.num(row, "start"),
  end: d.num(row, "end"),
});
const span = (row: Json): T.Span => ({
  start: d.num(row, "start"),
  end: d.num(row, "end"),
});
const fragment = (row: Json): T.Fragment => ({
  fragmentId: d.str(row, "fragmentId"),
  text: d.str(row, "text"),
  field: d.str(row, "field"),
  start: optionalNumber(row, "start"),
  end: optionalNumber(row, "end"),
  ...("workRef" in row ? { workRef: d.optStr(row, "workRef") } : {}),
  sourceKind: optionalText(row, "sourceKind"),
  sourceRef: optionalText(row, "sourceRef"),
});
const assignment = (row: Json): T.Assignment => ({
  topicRef: d.optStr(row, "topicRef"),
  definitionRef: d.optStr(row, "definitionRef"),
  label: optionalText(row, "label"),
  reason: optionalText(row, "reason"),
});
const relation = (row: Json): T.Relation => ({
  topicRef: optionalText(row, "topicRef"),
  displayName: optionalText(row, "displayName"),
  label: optionalText(row, "label"),
  relation: d.str(row, "relation"),
  reason: optionalText(row, "reason"),
});
const discussion = (row: Json): T.Discussion => ({
  unitId: optionalText(row, "unitId"),
  label: optionalText(row, "label"),
  statement: optionalText(row, "statement"),
  rationale: optionalText(row, "rationale"),
  status: optionalText(row, "status"),
  topicRef: d.optStr(row, "topicRef") ?? undefined,
  definitionRef: d.optStr(row, "definitionRef") ?? undefined,
  speakerRole: optionalText(row, "speakerRole"),
  evidenceRole: optionalText(row, "evidenceRole"),
  reason: optionalText(row, "reason"),
  assignments: optionalList(row, "assignments", assignment),
  evidence: d.list(row, "evidence", citation),
  relations: optionalList(row, "relations", relation),
  proposedTopic: optionalObject(row, "proposedTopic", (p) => ({
    domainReason: optionalText(p, "domainReason"),
    abstractionReason: optionalText(p, "abstractionReason"),
    parent: optionalObject(p, "parent", (p) => ({
      kind: d.str(p, "kind"),
      reason: d.str(p, "reason"),
    })),
  })),
  hierarchy: optionalObject(row, "hierarchy", (h) => ({
    parentTopicRef: optionalText(h, "parentTopicRef"),
    parentDefinitionRef: optionalText(h, "parentDefinitionRef"),
    reason: optionalText(h, "reason"),
  })),
});
const item = (row: Json): T.ResearchItem => ({
  label: optionalText(row, "label"),
  title: optionalText(row, "title"),
  text: optionalText(row, "text"),
  statement: optionalText(row, "statement"),
  answerTask: optionalText(row, "answerTask"),
  rationale: optionalText(row, "rationale"),
  need: optionalText(row, "need"),
  hypothesis: optionalText(row, "hypothesis"),
  verificationQuestion: optionalText(row, "verificationQuestion"),
  alternativeExplanation: optionalText(row, "alternativeExplanation"),
  status: optionalText(row, "status"),
  topicRef: optionalText(row, "topicRef"),
  workRef: optionalText(row, "workRef"),
  evidenceWorkRefs: optionalStrings(row, "evidenceWorkRefs"),
  evidence: optionalList(row, "evidence", citation),
  researchResultRef: d.optStr(row, "researchResultRef"),
  researchMethodVersion: optionalText(row, "researchMethodVersion"),
  researchAngleIndex: optionalNumber(row, "researchAngleIndex"),
  researchOpportunityIndex: optionalNumber(row, "researchOpportunityIndex"),
  limitations: optionalStrings(row, "limitations"),
  comparisonScope: optionalObject(row, "comparisonScope", (s) => ({
    selectedWorkRefs: strings(s, "selectedWorkRefs"),
    scopeWorkRefs: strings(s, "scopeWorkRefs"),
    boundary: d.str(s, "boundary"),
  })),
});
const output = (row: Json): T.ResearchOutput => ({
  discussions: d.list(row, "discussions", discussion),
  scenes: d.list(row, "scenes", item),
  responseMatches: d.list(row, "responseMatches", item),
  angles: d.list(row, "angles", item),
  productOpportunities: d.list(row, "productOpportunities", item),
  limitations: strings(row, "limitations"),
  journey: optionalObject(row, "journey", (j) => ({
    rationale: d.str(j, "rationale"),
    evidence: d.list(j, "evidence", citation),
    mainStage: d.str(j, "mainStage"),
    involvedStages: strings(j, "involvedStages"),
    path: d.str(j, "path"),
    overlays: strings(j, "overlays"),
  })),
});
const research = (row: Json): T.WorkResearch => ({
  readable: d.bool(row, "readable"),
  authorSourceState: d.str(row, "authorSourceState"),
  commentSourceState: d.str(row, "commentSourceState"),
  methodVersion: optionalText(row, "methodVersion"),
  resultRef: d.optStr(row, "resultRef"),
  resultRefs: optionalStrings(row, "resultRefs"),
  state: optionalText(row, "state"),
  fragments: d.list(row, "fragments", fragment),
  output: output(d.obj(row, "output")),
  core: optionalObject(row, "core", (c) => ({
    methodVersion: optionalText(c, "methodVersion"),
    units: optionalList(c, "units", discussion),
    coverage: optionalObject(c, "coverage", (c) => ({
      state: d.str(c, "state"),
      sourceChars: d.num(c, "sourceChars"),
      coveredChars: d.num(c, "coveredChars"),
      totalWindows: d.num(c, "totalWindows"),
      completedWindows: d.num(c, "completedWindows"),
    })),
  })),
});
const work = (row: Json): T.Work => ({
  workRef: d.str(row, "workRef"),
  platform: d.str(row, "platform"),
  contentExternalId: d.str(row, "contentExternalId"),
  title: nullable(row, "title", text),
  titleSource: d.str(row, "titleSource"),
  authorExternalId: nullable(row, "authorExternalId", text),
  creatorDisplayName: nullable(row, "creatorDisplayName", text),
  publishedAt: nullable(row, "publishedAt", text),
  publishedAtSourceText: nullable(row, "publishedAtSourceText", text),
  publishedAtPrecision: d.str(row, "publishedAtPrecision"),
  likes: nullable(row, "likes", numeric),
  collects: nullable(row, "collects", numeric),
  comments: nullable(row, "comments", numeric),
  shares: nullable(row, "shares", numeric),
  followerCount: nullable(row, "followerCount", numeric),
  own: d.bool(row, "own"),
  ownBreakout: d.bool(row, "ownBreakout"),
  readable: d.bool(row, "readable"),
  usageRoles: strings(row, "usageRoles"),
  topicRefs: strings(row, "topicRefs"),
  mainStage: nullable(row, "mainStage", text),
  involvedStages: strings(row, "involvedStages"),
  overlays: strings(row, "overlays"),
  path: d.str(row, "path"),
  annotation: nullable(row, "annotation", (value) => ({
    rationale: optionalText(object(value), "rationale"),
  })),
  research: nullable(row, "research", (value) => research(object(value))),
  media: {
    cover: optionalObject(d.obj(row, "media"), "cover", (c) => ({
      localAssetUrl: d.optStr(c, "localAssetUrl"),
      url: d.optStr(c, "url"),
    })),
  },
  preview:
    typeof d.field(row, "preview") === "string"
      ? text(d.field(row, "preview"))
      : { text: d.optStr(d.obj(row, "preview"), "text") },
  evidenceFragment: nullable(row, "evidenceFragment", (v) => {
    const f = object(v);
    return {
      text: d.str(f, "text"),
      sourceKind: optionalText(f, "sourceKind"),
      sourceRef: optionalText(f, "sourceRef"),
    };
  }),
});
const platformStats = (row: Json): T.PlatformStatistics => ({
  platform: d.str(row, "platform"),
  workCount: d.num(row, "workCount"),
  knownLikeCount: d.num(row, "knownLikeCount"),
  unknownLikeCount: d.num(row, "unknownLikeCount"),
  highPerformanceThreshold: nullable(row, "highPerformanceThreshold", numeric),
  highPerformanceCount: nullable(row, "highPerformanceCount", numeric),
  medianLikes: nullable(row, "medianLikes", numeric),
  p90Likes: nullable(row, "p90Likes", numeric),
  highPerformanceRuleVersion: nullable(
    row,
    "highPerformanceRuleVersion",
    numeric,
  ),
});
const stats = (row: Json): T.Statistics => ({
  workCount: d.num(row, "workCount"),
  authorCount: d.num(row, "authorCount"),
  unknownAuthorCount: d.num(row, "unknownAuthorCount"),
  platforms: d.list(row, "platforms", platformStats),
  ownBreakoutCount: d.num(row, "ownBreakoutCount"),
  ownPublishedCount: d.num(row, "ownPublishedCount"),
  ownPublicationUnknownCount: d.num(row, "ownPublicationUnknownCount"),
  ownHistoryPublishedCount: optionalNumber(row, "ownHistoryPublishedCount"),
  ownHistoryBreakoutCount: optionalNumber(row, "ownHistoryBreakoutCount"),
  recentLowFollowerHighLikeCount: optionalNumber(
    row,
    "recentLowFollowerHighLikeCount",
  ),
});
const journeyEntry = (row: Json): T.JourneyEntry => ({
  stage: d.str(row, "stage"),
  count: d.num(row, "count"),
  workRefs: strings(row, "workRefs"),
});
const journey = (row: Json): T.Journey => ({
  denominator: d.num(row, "denominator"),
  main: d.list(row, "main", journeyEntry),
  involved: d.list(row, "involved", journeyEntry),
  overlays: d.list(row, "overlays", journeyEntry),
  involvedMayOverlap: d.bool(row, "involvedMayOverlap"),
});
const topic = (row: Json): T.Topic => ({
  topicRef: d.str(row, "topicRef"),
  canonicalKey: d.str(row, "canonicalKey"),
  domainRef: nullable(row, "domainRef", text),
  definitionRef: d.str(row, "definitionRef"),
  definitionVersion: d.num(row, "definitionVersion"),
  displayName: d.str(row, "displayName"),
  definitionText: d.str(row, "definitionText"),
  lifecycleState: d.str(row, "lifecycleState"),
  parentTopicRef: nullable(row, "parentTopicRef", text),
  bindingVersion: nullable(row, "bindingVersion", numeric),
  directWorkRefs: strings(row, "directWorkRefs"),
  workRefs: strings(row, "workRefs"),
  statistics: stats(d.obj(row, "statistics")),
  journey: journey(d.obj(row, "journey")),
  aliases: optionalStrings(row, "aliases"),
  core: nullable(row, "core", (value) => {
    const c = object(value);
    return {
      qualityState: optionalText(c, "qualityState"),
      sourceState: d.str(c, "sourceState"),
      definitionSource: d.str(c, "definitionSource"),
      conceptRole: optionalText(c, "conceptRole"),
      inductionMethod: optionalText(c, "inductionMethod"),
      inclusionCriteria: strings(c, "inclusionCriteria"),
      exclusionCriteria: strings(c, "exclusionCriteria"),
      discussionCount: d.num(c, "discussionCount"),
      evidenceRoles: ((r) => ({
        support: d.num(r, "support"),
        challenge: d.num(r, "challenge"),
        context: d.num(r, "context"),
      }))(d.obj(c, "evidenceRoles")),
      relations: d.list(c, "relations", relation),
    };
  }),
});
const alternative = (row: Json): T.Alternative => ({
  alternativeRef: d.str(row, "alternativeRef"),
  title: nullable(row, "title", text),
  angle: nullable(row, "angle", text),
  rationale: nullable(row, "rationale", text),
  topicRef: nullable(row, "topicRef", text),
  definitionRef: nullable(row, "definitionRef", text),
  sourceState: optionalText(row, "sourceState"),
  kind: optionalText(row, "kind"),
  domainRef: optionalText(row, "domainRef"),
  methodVersion: optionalText(row, "methodVersion"),
  createdAt: optionalText(row, "createdAt"),
  evidenceWorkRefs: optionalStrings(row, "evidenceWorkRefs"),
  originalDefinition: optionalObject(row, "originalDefinition", (d) => ({
    displayName: text(d.displayName),
    definitionText: text(d.definitionText),
    version: numeric(d.version),
    definitionRef: text(d.definitionRef),
  })),
  fragments: optionalList(row, "fragments", (r) => ({
    ...fragment(r),
    workRef: d.str(r, "workRef"),
    start: d.num(r, "start"),
    end: d.num(r, "end"),
    sourceVersion: d.optStr(r, "sourceVersion"),
    citedRanges: d.list(r, "citedRanges", span),
  })),
});
export const decodeSnapshot = (value: unknown): T.Snapshot =>
  d.decode(value, (row) => ({
    domains: d.list(row, "domains", (r) => ({
      domainRef: d.str(r, "domainRef"),
      name: d.str(r, "name"),
      description: nullable(r, "description", text),
      status: d.str(r, "status"),
      researchGoal: nullable(r, "researchGoal", text),
    })),
    topics: d.list(row, "topics", topic),
    works: d.list(row, "works", work),
    statistics: stats(d.obj(row, "statistics")),
    journey: journey(d.obj(row, "journey")),
    sources: d.list(row, "sources", (r) => ({
      authorExternalId: nullable(r, "authorExternalId", text),
      platform: d.str(r, "platform"),
      creatorDisplayName: nullable(r, "creatorDisplayName", text),
      workCount: d.num(r, "workCount"),
      statistics: stats(d.obj(r, "statistics")),
      topicRefs: strings(r, "topicRefs"),
      workRefs: strings(r, "workRefs"),
    })),
    candidates: d.list(row, "candidates", item),
    changes: d.list(row, "changes", (r) => ({
      topicRef: d.str(r, "topicRef"),
      definitionRef: d.str(r, "definitionRef"),
      definitionVersion: d.num(r, "definitionVersion"),
      structureChanged: d.bool(r, "structureChanged"),
      notViewed: d.bool(r, "notViewed"),
      unviewedWorkRefs: strings(r, "unviewedWorkRefs"),
      lastViewedWorkRefs: nullable(r, "lastViewedWorkRefs", (v) => {
        if (!Array.isArray(v)) throw new d.ShapeError("lastViewedWorkRefs");
        return v.map(text);
      }),
      lastViewedAt: nullable(r, "lastViewedAt", text),
    })),
    alternatives: d.list(row, "alternatives", alternative),
    ownCreators: d.list(row, "ownCreators", (r) => ({
      platform: d.str(r, "platform"),
      authorExternalId: d.str(r, "authorExternalId"),
      active: d.bool(r, "active"),
    })),
    methodVersion: d.str(row, "methodVersion"),
    sourceBoundary: d.str(row, "sourceBoundary"),
    scope: ((r) => ({
      domainRef: nullable(r, "domainRef", text),
      topicRef: nullable(r, "topicRef", text),
      totalWorkCount: d.num(r, "totalWorkCount"),
      readableWorkCount: d.num(r, "readableWorkCount"),
      unclassifiedWorkCount: d.num(r, "unclassifiedWorkCount"),
      asOf: d.str(r, "asOf"),
      ownIdentityState: d.str(r, "ownIdentityState"),
      ownHistoryPublishedCount: d.num(r, "ownHistoryPublishedCount"),
      ownHistoryPublicationUnknownCount: d.num(
        r,
        "ownHistoryPublicationUnknownCount",
      ),
      ownHistoricalWorkRefs: strings(r, "ownHistoricalWorkRefs"),
      recentReferenceWorkRefs: strings(r, "recentReferenceWorkRefs"),
      loadedWorkCount: d.num(r, "loadedWorkCount"),
      canonicalWorkCount: d.num(r, "canonicalWorkCount"),
    }))(d.obj(row, "scope")),
  }));
const phases = (r: Json): T.Phases => ({
  extracting: d.num(r, "extracting"),
  extractQueued: d.num(r, "extractQueued"),
  resolving: d.num(r, "resolving"),
  resolveQueued: d.num(r, "resolveQueued"),
  comparing: d.num(r, "comparing"),
  compareQueued: d.num(r, "compareQueued"),
});
const counts = (r: Json): T.Counts => ({
  correctiveQueuedCount: d.num(r, "correctiveQueuedCount"),
  correctiveRunningCount: d.num(r, "correctiveRunningCount"),
  reviewQueuedCount: d.num(r, "reviewQueuedCount"),
  reviewRunningCount: d.num(r, "reviewRunningCount"),
  queuedCount: d.num(r, "queuedCount"),
  runningCount: d.num(r, "runningCount"),
  succeededCount: d.num(r, "succeededCount"),
  noSignalCount: d.num(r, "noSignalCount"),
  insufficientCount: d.num(r, "insufficientCount"),
  failedCount: d.num(r, "failedCount"),
  unknownDispatchCount: d.num(r, "unknownDispatchCount"),
  extractedWindowCount: d.num(r, "extractedWindowCount"),
  staleCount: d.num(r, "staleCount"),
  stoppedCount: d.num(r, "stoppedCount"),
  initialQueuedCount: d.num(r, "initialQueuedCount"),
  completedInitialWindowCount: d.num(r, "completedInitialWindowCount"),
  coveredWorkCount: d.num(r, "coveredWorkCount"),
  reassessmentQueuedCount: d.num(r, "reassessmentQueuedCount"),
  reassessmentCompletedCount: d.num(r, "reassessmentCompletedCount"),
  totalRunCount: optionalNumber(r, "totalRunCount"),
  totalTaskCount: d.num(r, "totalTaskCount"),
  phases: phases(d.obj(r, "phases")),
});
const diagnostic = (r: Json): T.Diagnostic => ({
  rule: optionalText(r, "rule"),
  schemaPath: optionalText(r, "schemaPath"),
  candidateCount: optionalNumber(r, "candidateCount"),
  unitCount: optionalNumber(r, "unitCount"),
  decisionCount: optionalNumber(r, "decisionCount"),
  semanticChecks: optionalList(r, "semanticChecks", (r) => ({
    checkId: d.str(r, "checkId"),
    verdict: d.str(r, "verdict"),
    rule: d.str(r, "rule"),
  })),
});
const round = (r: Json): T.CollectionRound => ({
  roundRef: d.str(r, "roundRef"),
  kind: d.str(r, "kind"),
  state: d.str(r, "state"),
  reason: nullable(r, "reason", text),
  detailSlotsReserved: d.num(r, "detailSlotsReserved"),
});
export const decodeProgress = (value: unknown): T.ResearchProgress =>
  d.decode(value, (r) => ({
    domainRef: d.str(r, "domainRef"),
    methodVersion: d.str(r, "methodVersion"),
    policy: nullable(r, "policy", (value) => {
      const p = object(value);
      return {
        status: d.str(p, "status"),
        modelConfigRef: d.str(p, "modelConfigRef"),
        dailyTokenLimit: d.num(p, "dailyTokenLimit"),
        runTokenLimit: d.num(p, "runTokenLimit"),
        automaticEnabled: d.bool(p, "automaticEnabled"),
        collectionEnabled: d.bool(p, "collectionEnabled"),
      };
    }),
    currentMethodSummary: counts(d.obj(r, "currentMethodSummary")),
    summary: counts(d.obj(r, "summary")),
    runs: d.list(r, "runs", (r) => ({
      ...counts(r),
      runRef: d.str(r, "runRef"),
      state: d.str(r, "state"),
      lastReason: nullable(r, "lastReason", text),
      trigger: d.str(r, "trigger"),
      methodVersion: d.str(r, "methodVersion"),
      createdAt: d.str(r, "createdAt"),
      taskIssues: d.list(r, "taskIssues", (r) => ({
        phase: d.str(r, "phase"),
        state: d.str(r, "state"),
        reason: nullable(r, "reason", text),
        taskRef: d.str(r, "taskRef"),
        validationDiagnostic: nullable(r, "validationDiagnostic", (v) =>
          diagnostic(object(v)),
        ),
      })),
    })),
    runListLimit: d.num(r, "runListLimit"),
    models: d.list(r, "models", (r) => ({
      configRef: d.str(r, "configRef"),
      modelId: d.str(r, "modelId"),
      inputTokenLimit: d.num(r, "inputTokenLimit"),
      outputTokenLimit: d.num(r, "outputTokenLimit"),
    })),
    usage: ((r) => ({
      chargedTokens: d.num(r, "chargedTokens"),
      reservedTokens: d.num(r, "reservedTokens"),
      timezone: d.str(r, "timezone"),
    }))(d.obj(r, "usage")),
    processingLimits: ((r) => ({
      automaticBatchWindows: d.num(r, "automaticBatchWindows"),
      materialSelection: (() => { const value = d.str(r, "materialSelection"); if (value !== 'whole_note') throw new d.ShapeError('materialSelection'); return 'whole_note' as const; })(),
      primaryCommentsPerWindow: d.num(r, "primaryCommentsPerWindow"),
    }))(d.obj(r, "processingLimits")),
    targets: d.list(r, "targets", (r) => ({
      targetRef: d.str(r, "targetRef"),
      displayName: d.str(r, "displayName"),
      kind: d.str(r, "kind"),
    })),
    collectionRounds: d.list(r, "collectionRounds", round),
  }));
export const decodeSearch = (value: unknown): T.SearchProgress =>
  d.decode(value, (r) => ({
    domainRef: d.str(r, "domainRef"),
    authorizations: d.list(r, "authorizations", (r) => ({
      authorizationRef: d.str(r, "authorizationRef"),
      purpose: d.str(r, "purpose"),
      maxWorksPerTarget: d.num(r, "maxWorksPerTarget"),
      expiresAt: d.str(r, "expiresAt"),
    })),
    rounds: d.list(r, "rounds", (r) => ({
      roundRef: d.str(r, "roundRef"),
      domainRef: d.str(r, "domainRef"),
      state: d.str(r, "state"),
      detailSlotsReserved: d.num(r, "detailSlotsReserved"),
      phase: d.str(r, "phase"),
      searches: d.list(r, "searches", (r) => ({
        keyword: d.str(r, "keyword"),
      })),
      candidateCount: d.num(r, "candidateCount"),
      candidateLimit: d.num(r, "candidateLimit"),
      actualDetailsAcquired: d.num(r, "actualDetailsAcquired"),
      pendingDetailWorkRefs: strings(r, "pendingDetailWorkRefs"),
      candidateWorkRefs: strings(r, "candidateWorkRefs"),
    })),
  }));
export const decodeEvidence = (v: unknown): T.Evidence =>
  d.decode(v, (r) => ({ works: d.list(r, "works", work) }));
export const decodeAlternative = (v: unknown): T.Alternative =>
  d.decode(v, alternative);
export const decodeResource = (v: unknown): T.Resource =>
  d.decode(v, (r) => ({
    item: ((r) => ({
      inspector: optionalObject(r, "inspector", (r) => ({
        detailCurrent: optionalObject(r, "detailCurrent", (r) => ({
          body: optionalObject(r, "body", (r) => ({
            value: d.optStr(r, "value"),
          })),
        })),
      })),
      evidenceFragment: optionalObject(r, "evidenceFragment", (r) => ({
        text: d.str(r, "text"),
      })),
    }))(d.obj(r, "item")),
  }));
export const decodeComments = (v: unknown): T.Comments =>
  d.decode(v, (r) => ({
    items: d.list(r, "items", (r) => ({
      body: nullable(r, "body", text),
      sourceRef: d.str(r, "sourceRef"),
      parentText: d.optStr(r, "parentText"),
    })),
    total: d.num(r, "total"),
    nextCursor: nullable(r, "nextCursor", text),
  }));
export const decodeCommand = (v: unknown): T.CommandReceipt =>
  d.decode(v, (r) => ({
    receiptRef: d.str(r, "receiptRef"),
    action: d.str(r, "action"),
    subjectRef: nullable(r, "subjectRef", text),
    revision: d.num(r, "revision"),
    persistedAt: d.str(r, "persistedAt"),
  }));
export const decodeResearchReceipt = (v: unknown): T.ResearchReceipt =>
  d.decode(v, (r) => ({
    requestRef: d.str(r, "requestRef"),
    domainRef: d.optStr(r, "domainRef"),
    state: optionalText(r, "state"),
    runRef: d.optStr(r, "runRef"),
    roundRef: optionalText(r, "roundRef"),
    phase: optionalText(r, "phase"),
    workRefs: optionalStrings(r, "workRefs"),
  }));
export const decodePreview = (v: unknown): T.StructurePreview =>
  d.decode(v, (r) => ({
    previewHash: d.str(r, "previewHash"),
    impact: ((r) => ({ unassignedWorkRefs: strings(r, "unassignedWorkRefs") }))(
      d.obj(r, "impact"),
    ),
  }));
export const decodeStructure = (v: unknown): T.StructureReceipt =>
  d.decode(v, (r) => ({
    requestRef: d.str(r, "requestRef"),
    domainRef: d.str(r, "domainRef"),
    kind: d.str(r, "kind"),
    state: d.str(r, "state"),
    previewHash: d.str(r, "previewHash"),
    oldReferencesRetained: d.bool(r, "oldReferencesRetained"),
    claimsInherited: d.bool(r, "claimsInherited"),
    destinations: d.list(r, "destinations", (r) => ({
      topicRef: d.str(r, "topicRef"),
    })),
  }));
