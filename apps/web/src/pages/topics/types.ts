/** Browser projections of the existing Rust Topic Map DTOs. Domain decisions remain in Rust. */
export interface Citation {
  fragmentId: string;
  start: number;
  end: number;
  sourceRef?: string;
  field?: string;
}
export interface Fragment {
  fragmentId: string;
  text: string;
  field: string;
  start?: number;
  end?: number;
  workRef?: string | null;
  sourceKind?: string;
  sourceType?: string;
  sourceRef?: string;
}
export interface Assignment {
  topicRef?: string | null;
  definitionRef?: string | null;
  label?: string;
  reason?: string;
}
export interface Relation {
  topicRef?: string;
  displayName?: string;
  label?: string;
  relation: string;
  reason?: string;
}
export interface Discussion {
  unitId?: string;
  unitKey?: string;
  label?: string;
  statement?: string;
  rationale?: string;
  status?: string;
  topicRef?: string;
  definitionRef?: string;
  speakerRole?: string;
  evidenceRole?: string;
  assignments?: Assignment[];
  evidence?: Citation[];
  relations?: Relation[];
  reason?: string;
  proposedTopic?: {
    domainReason?: string;
    abstractionReason?: string;
    parent?: { kind: string; reason: string };
  };
  hierarchy?: {
    parentTopicRef?: string;
    parentDefinitionRef?: string;
    reason?: string;
  };
}
export interface DiscussionUnit extends Discussion {
  unitKey: string;
  assignments: Assignment[];
  evidence: Citation[];
  legacy: boolean;
}
export interface ResearchCoverage {
  state: string;
  sourceChars: number;
  coveredChars: number;
  totalWindows: number;
  completedWindows: number;
}
export interface ResearchItem {
  label?: string;
  title?: string;
  text?: string;
  statement?: string;
  answerTask?: string;
  rationale?: string;
  need?: string;
  hypothesis?: string;
  verificationQuestion?: string;
  alternativeExplanation?: string;
  topicRef?: string;
  workRef?: string;
  evidenceWorkRefs?: string[];
  evidence?: Citation[];
  researchResultRef?: string | null;
  researchMethodVersion?: string;
  researchAngleIndex?: number;
  researchOpportunityIndex?: number;
  applicability?: string;
  format?: string;
  audience?: string;
  whyItWorks?: string;
  boundary?: string;
  reusableElements?: string[];
  contentElement?: string;
  response?: string;
  interpretation?: string;
  condition?: string;
  risk?: string;
  status?: string;
  limitations?: string[];
  comparisonScope?: {
    selectedWorkRefs: string[];
    scopeWorkRefs: string[];
    boundary: string;
  };
}
export interface ResearchOutput {
  discussions?: Discussion[];
  scenes?: ResearchItem[];
  responseMatches?: ResearchItem[];
  angles?: ResearchItem[];
  productOpportunities?: ResearchItem[];
  limitations?: string[];
  journey?: {
    rationale?: string;
    evidence?: Citation[];
    mainStage?: string;
    involvedStages?: string[];
    path?: string;
    overlays?: string[];
  };
}
export interface WorkResearch {
  readable: boolean;
  authorSourceState: string;
  commentSourceState: string;
  methodVersion?: string;
  resultRef?: string | null;
  resultRefs?: string[];
  state?: string;
  fragments: Fragment[];
  output: ResearchOutput;
  core?: {
    methodVersion?: string;
    coverage?: ResearchCoverage;
    units?: Discussion[];
  };
}
export interface Work {
  workRef: string;
  platform: string;
  contentExternalId: string;
  title: string | null;
  titleSource: string;
  authorExternalId: string | null;
  creatorDisplayName: string | null;
  publishedAt: string | null;
  publishedAtSourceText: string | null;
  publishedAtPrecision: string;
  likes: number | null;
  collects: number | null;
  comments: number | null;
  shares: number | null;
  followerCount: number | null;
  own: boolean;
  ownBreakout: boolean;
  readable: boolean;
  usageRoles: string[];
  topicRefs: string[];
  mainStage: string | null;
  involvedStages: string[];
  overlays: string[];
  path: string;
  annotation: { rationale?: string } | null;
  research: WorkResearch | null;
  media: {
    cover?: { localAssetUrl?: string | null; url?: string | null };
    [key: string]: unknown;
  };
  preview: { text?: string | null } | string;
  evidenceFragment: {
    text: string;
    sourceKind?: string;
    sourceRef?: string;
  } | null;
  sourceManifest?: Record<string, unknown>;
}
export interface PlatformStatistics {
  platform: string;
  workCount: number;
  knownLikeCount: number;
  unknownLikeCount: number;
  highPerformanceThreshold: number | null;
  highPerformanceCount: number | null;
  medianLikes: number | null;
  p90Likes: number | null;
  highPerformanceRuleVersion?: number | null;
}
export interface Statistics {
  workCount: number;
  authorCount: number;
  unknownAuthorCount: number;
  ownBreakoutCount?: number;
  platforms: PlatformStatistics[];
  ownHistoryPublishedCount?: number;
  ownHistoryBreakoutCount?: number;
  recentLowFollowerHighLikeCount?: number;
  ownPublishedCount?: number;
  ownPublicationUnknownCount?: number;
}
export interface JourneyEntry {
  stage: string;
  label?: string;
  count: number;
  workRefs: string[];
}
export interface Journey {
  denominator: number;
  main: JourneyEntry[];
  involved: JourneyEntry[];
  overlays?: JourneyEntry[];
  involvedMayOverlap?: boolean;
}
export interface Topic {
  topicRef: string;
  canonicalKey: string;
  domainRef: string | null;
  definitionRef: string;
  definitionVersion: number;
  displayName: string;
  definitionText: string;
  lifecycleState: string;
  parentTopicRef: string | null;
  bindingVersion: number | null;
  directWorkRefs: string[];
  workRefs: string[];
  statistics: Statistics;
  journey: Journey;
  aliases?: string[];
  core: {
    qualityState?: string;
    sourceState?: string;
    definitionSource?: string;
    conceptRole?: string;
    inductionMethod?: string;
    inclusionCriteria?: string[];
    exclusionCriteria?: string[];
    discussionCount?: number;
    evidenceRoles?: { support?: number; challenge?: number; context?: number };
    relations?: Relation[];
  } | null;
}
export interface Domain {
  domainRef: string;
  id?: string;
  name: string;
  displayName?: string;
  description?: string | null;
  status?: string;
  researchGoal?: string | null;
}
export interface Source {
  authorExternalId: string | null;
  platform: string;
  creatorDisplayName: string | null;
  workCount: number;
  statistics: Statistics;
  topicRefs: string[];
  highPerformanceCount?: number;
  recentLowFollowerHighLikeCount?: number;
  followerCount?: number | null;
  workRefs?: string[];
  own?: boolean;
  role?: string;
}
export interface Change {
  topicRef: string;
  structureChanged?: boolean;
  notViewed?: boolean;
  unviewedWorkRefs?: string[];
  lastViewedWorkRefs?: string[] | null;
  definitionVersion?: number;
  lastViewedAt?: string | null;
  definitionRef?: string;
  [key: string]: unknown;
}
export interface Alternative {
  alternativeRef: string;
  title: string | null;
  angle: string | null;
  rationale: string | null;
  topicRef: string | null;
  definitionRef: string | null;
  sourceState?: string;
  definitionVersion?: number;
  createdAt?: string;
  methodVersion?: string;
  evidenceWorkRefs?: string[];
  kind?: string;
  domainRef?: string;
  fragments?: FrozenFragment[];
  originalDefinition?: {
    displayName: string;
    definitionText: string;
    version: number;
    definitionRef: string;
  } | null;
}
export type ReadableAlternative = Alternative & {
  title: string;
  angle: string;
  rationale: string;
  sourceState: "available" | "version_changed_available";
};
export interface Snapshot {
  domains: Domain[];
  topics: Topic[];
  works: Work[];
  statistics: Statistics;
  journey: Journey;
  sources: Source[];
  candidates: ResearchItem[];
  changes: Change[];
  alternatives: Alternative[];
  ownCreators: {
    platform: string;
    authorExternalId: string;
    active: boolean;
  }[];
  methodVersion: string;
  sourceBoundary: string;
  scope: {
    domainRef: string | null;
    topicRef: string | null;
    totalWorkCount: number;
    readableWorkCount: number;
    unclassifiedWorkCount: number;
    asOf: string;
    ownIdentityState: string;
    ownHistoryPublishedCount?: number;
    ownHistoryPublicationUnknownCount?: number;
    ownHistoricalWorkRefs?: string[];
    recentReferenceWorkRefs: string[];
    readLimit?: number;
    loadedWorkCount?: number;
    canonicalWorkCount?: number;
  };
}
export interface DiscussionRow {
  work: Work;
  discussion: DiscussionUnit;
  assignment: Assignment | null;
}
export interface DiscussionGroup {
  key: string;
  topicRef: string;
  label: string;
  canonical: boolean;
  rows: DiscussionRow[];
  works: Work[];
  evidenceCount: number;
}
export interface LocatedCitation extends Citation {
  fragment: Fragment;
  localStart: number;
  localEnd: number;
}
export interface Reader {
  work: Work;
  domainRef: string;
  loading: boolean;
  resource?: Resource | null;
  resourceError?: string;
  commentResource?: Comments | null;
  commentError?: string;
  qualificationError?: string;
}
export interface ViewState {
  domainRef: string;
  topicRef: string;
  platform: string;
  windowDays: string;
  referenceWindowDays: string;
  view: string;
  tab: string;
  path: string;
  overlay: string;
  stage: string;
  metric: string;
  mapMode: string;
  materialMode: string;
  summaryChart: string;
  plotPlatform: string;
  changesOnly: boolean;
  query: string;
  collapsed: string[];
  compare: string[];
  navigationOpen: boolean;
  overviewWindowDays: string;
  journeyWindowDays: string;
  sceneQuery: string;
  showAllScenes: boolean;
  legacyExpanded: boolean;
  layoutVersion: number;
}

export interface Span {
  start: number;
  end: number;
}
export interface Resource {
  item: {
    inspector?: {
      detailCurrent?: { body?: { value?: string | null } | null } | null;
    } | null;
    evidenceFragment?: { text: string } | null;
  };
}
export interface Comment {
  body: string | null;
  sourceRef: string;
  parentText: string | null;
}
export interface Comments {
  items: Comment[];
  total: number;
  nextCursor: string | null;
}
export interface FrozenFragment extends Fragment {
  citedRanges: Span[];
  sourceVersion?: string | null;
  workRef: string;
  start: number;
  end: number;
}
export interface Phases {
  extracting: number;
  extractQueued: number;
  resolving: number;
  resolveQueued: number;
  comparing: number;
  compareQueued: number;
}
export interface Counts {
  queuedCount: number;
  runningCount: number;
  succeededCount: number;
  noSignalCount: number;
  insufficientCount: number;
  failedCount: number;
  unknownDispatchCount: number;
  extractedWindowCount: number;
  staleCount: number;
  stoppedCount: number;
  initialQueuedCount: number;
  completedInitialWindowCount: number;
  coveredWorkCount: number;
  correctiveQueuedCount: number;
  correctiveRunningCount: number;
  reviewQueuedCount: number;
  reviewRunningCount: number;
  reassessmentQueuedCount: number;
  reassessmentCompletedCount: number;
  totalRunCount?: number;
  totalTaskCount: number;
  phases: Phases;
}
export interface Diagnostic {
  rule?: string;
  schemaPath?: string;
  candidateCount?: number;
  unitCount?: number;
  decisionCount?: number;
  semanticChecks?: { checkId: string; verdict: string; rule: string }[];
}
export interface CollectionRound {
  roundRef: string;
  kind: string;
  state: string;
  reason: string | null;
  detailSlotsReserved: number;
}
export interface SearchRound {
  roundRef: string;
  domainRef: string;
  state: string;
  detailSlotsReserved: number;
  phase: string;
  searches: { keyword: string }[];
  candidateCount: number;
  candidateLimit: number;
  actualDetailsAcquired: number;
  pendingDetailWorkRefs: string[];
  candidateWorkRefs: string[];
}
export interface SearchProgress {
  domainRef: string;
  authorizations: {
    authorizationRef: string;
    purpose: string;
    maxWorksPerTarget: number;
    expiresAt: string;
  }[];
  rounds: SearchRound[];
}
export interface StructureMember {
  workPublicRef: string;
  role: string;
  rationale: string;
}
export interface StructureDestination {
  displayName: string;
  definitionText: string;
  parentTopicRef: string | null;
  members: StructureMember[];
}
export interface StructurePlan {
  requestRef: string;
  domainRef: string;
  kind: string;
  sources: { topicRef: string; definitionRef: string }[];
  destinations: StructureDestination[];
  rationale: string;
}
export interface StructurePreview {
  previewHash: string;
  impact: { unassignedWorkRefs: string[] };
}
export interface StructureReceipt {
  destinations: { topicRef: string }[];
  requestRef: string;
  domainRef: string;
  kind: string;
  state: string;
  previewHash: string;
  oldReferencesRetained: boolean;
  claimsInherited: boolean;
}
export interface CommandReceipt {
  receiptRef: string;
  subjectRef: string | null;
  revision: number;
  persistedAt: string;
  action: string;
  idempotencyKey?: string;
}
export interface CollectionReceipt {
  requestRef: string;
  domainRef?: string | null;
  state?: string;
  action?: string;
  runRef?: string | null;
  roundRef?: string;
  phase?: string;
  workRefs?: string[];
}
export interface Evidence {
  works: Work[];
}
export type DialogKind =
  | "definition"
  | "materials"
  | "reader"
  | "judge"
  | "replace"
  | "matrix"
  | "compare"
  | "identity"
  | "rules"
  | "saved"
  | "alternative"
  | "save"
  | "research"
  | "product"
  | "changes"
  | "structure"
  | "move"
  | "capture"
  | "method";
export interface DialogState {
  type: DialogKind;
  scroll?: number;
  loading?: boolean;
  error?: string;
  readError?: string;
  snapshot?: Snapshot;
  topicRef?: string;
  refs?: string[];
  unassigned?: boolean;
  query?: string;
  sort?: "latest" | "likes" | "collects" | "comments" | "shares";
  limit?: number;
  label?: string;
  work?: Work;
  domainRef?: string;
  resource?: Resource | null;
  resourceError?: string;
  commentResource?: Comments | null;
  commentError?: string;
  qualificationError?: string;
  works?: Work[];
  tab?: string;
  inlineReader?: Reader;
  sampleRef?: string;
  judgeState?: DialogState;
  rows?: string;
  metric?: string;
  kind?: string;
  manual?: boolean;
  angle?: ResearchItem;
  savedAlternativeRef?: string;
  saveRequestKey?: string;
  alternative?: Alternative;
  alternativeRef?: string;
  moveTopicRef?: string;
  sources?: string[];
  destinationCount?: number;
  plan?: StructurePlan;
  preview?: StructurePreview;
  structureApplied?: boolean;
  collection?: { policy: { enabled: boolean; revision: number }; targets: { targetRef: string; kind: string; displayName: string }[]; collectionRounds: CollectionRound[] };
  search?: SearchProgress;
  collectionError?: string;
  searchError?: string;
  workRef?: string | null;
}
export interface DiscussionInternalGroup {
  key: string;
  topicRef: string;
  label: string;
  canonical: boolean;
  rows: DiscussionRow[];
  seen: Set<string>;
}
export interface SceneGroup {
  label: string;
  works: Map<string, Work | undefined>;
  citations: Set<string>;
  excerpt: string;
  unknownOwner?: boolean;
}
export interface AnnotatedResearchItem extends ResearchItem {
  workRef: string;
  evidenceWorkRefs: string[];
  limitations: string[];
}
export type ResearchOutputs = { work: Work; output: ResearchOutput }[];
export type TextStateKey = {
  [K in keyof ViewState]: ViewState[K] extends string ? K : never;
}[keyof ViewState];
