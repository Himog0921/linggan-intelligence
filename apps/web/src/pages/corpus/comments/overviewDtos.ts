import { decode, list, obj, optObj, optNum, optStr, str, type Json } from '../../../shared/decode';
import type { CommentKey, StudyPage } from './studyApi';

export interface VoicePreview {
  sourceState: string | null;
  commentText: string | null;
  proposition: string | null;
  workTitle: string | null;
  commentKey: CommentKey | null;
  runRef: string | null;
  signalRef: string | null;
}
export interface ObservationPoint {
  date: string;
  newObservedCommentCount: number | null;
  studiedCommentCount: number | null;
  acceptedSignalCount: number | null;
  coveredWorkCount: number | null;
  observationCoverage: string | null;
}
export interface ObservationSeries { timezone: string | null; points: ObservationPoint[] }
export interface CorpusSummary { displayableCommentCount: number | null; eligibleCommentCount: number | null }
export interface ResearchSummary {
  studiedCommentCount: number | null;
  succeededCommentCount: number | null;
  noSignalCommentCount: number | null;
  currentSignalCount: number | null;
}
export interface ProblemPreview {
  problemRef: string;
  title: string | null;
  addedSupportCommentCount: number | null;
  supportCommentCount: number | null;
  voice: VoicePreview | null;
}
const knowledgeCountKeys = ['problemCount', 'deferredNovelCount', 'assignedSignalCount', 'activeProblemCount', 'supportInsufficientProblemCount', 'solutionSignalCount', 'experienceSignalCount', 'solutionWorkCount', 'experienceWorkCount', 'supportWindowDays', 'deferredAmbiguousCount', 'deferredContextCount', 'retrievalIncompleteCount', 'pendingResolutionCount', 'budgetStoppedCount', 'protocolRejectedCount', 'failedResolutionCount'] as const;
export type KnowledgeSummary = Record<typeof knowledgeCountKeys[number], number | null> & { problemSupportPreview: ProblemPreview[]; voicePreview: VoicePreview[]; solutionPreview: VoicePreview[]; experiencePreview: VoicePreview[] };
export interface StudyOverview {
  asOf: string | null;
  corpusSummaryState: string | null;
  corpusSummary: CorpusSummary | null;
  researchSummary: ResearchSummary | null;
  knowledgeSummary: KnowledgeSummary | null;
  indexCoverage: { indexedCount: number | null; pendingCount: number | null } | null;
  observationSeries: ObservationSeries | null;
  latestRun: { runRef: string; state: string | null; createdAt: string | null } | null;
  yesterdayNewProblemCount: number | null;
}

export function parseVoice(row: Json): VoicePreview {
  const key = optObj(row, 'commentKey');
  return { sourceState: optStr(row, 'sourceState'), commentText: optStr(row, 'commentText'), proposition: optStr(row, 'proposition'), workTitle: optStr(row, 'workTitle'), commentKey: key ? { workRef: str(key, 'workRef'), commentExternalId: str(key, 'commentExternalId') } : null, runRef: optStr(row, 'runRef'), signalRef: optStr(row, 'signalRef') };
}

export function parseOverview(value: Json): StudyOverview {
  return decode(value, root => {
    const corpus = optObj(root, 'corpusSummary'), research = optObj(root, 'researchSummary'), knowledge = optObj(root, 'knowledgeSummary');
    const coverage = optObj(root, 'indexCoverage'), series = optObj(root, 'observationSeries'), run = optObj(root, 'latestRun');
    return {
      asOf: optStr(root, 'asOf'), corpusSummaryState: optStr(root, 'corpusSummaryState'),
      corpusSummary: corpus ? { displayableCommentCount: optNum(corpus, 'displayableCommentCount'), eligibleCommentCount: optNum(corpus, 'eligibleCommentCount') } : null,
      researchSummary: research ? { studiedCommentCount: optNum(research, 'studiedCommentCount'), succeededCommentCount: optNum(research, 'succeededCommentCount'), noSignalCommentCount: optNum(research, 'noSignalCommentCount'), currentSignalCount: optNum(research, 'currentSignalCount') } : null,
      knowledgeSummary: knowledge ? {
        ...Object.fromEntries(knowledgeCountKeys.map(key => [key, optNum(knowledge, key)])) as Record<typeof knowledgeCountKeys[number], number | null>,
        problemSupportPreview: list(knowledge, 'problemSupportPreview', row => ({ problemRef: str(row, 'problemRef'), title: optStr(row, 'title'), addedSupportCommentCount: optNum(row, 'addedSupportCommentCount'), supportCommentCount: optNum(row, 'supportCommentCount'), voice: optObj(row, 'voice') ? parseVoice(obj(row, 'voice')) : null })),
        voicePreview: list(knowledge, 'voicePreview', parseVoice), solutionPreview: list(knowledge, 'solutionPreview', parseVoice), experiencePreview: list(knowledge, 'experiencePreview', parseVoice),
      } : null,
      indexCoverage: coverage ? { indexedCount: optNum(coverage, 'indexedCount'), pendingCount: optNum(coverage, 'pendingCount') } : null,
      observationSeries: series ? { timezone: optStr(series, 'timezone'), points: list(series, 'points', row => ({ date: str(row, 'date'), newObservedCommentCount: optNum(row, 'newObservedCommentCount'), studiedCommentCount: optNum(row, 'studiedCommentCount'), acceptedSignalCount: optNum(row, 'acceptedSignalCount'), coveredWorkCount: optNum(row, 'coveredWorkCount'), observationCoverage: optStr(row, 'observationCoverage') })) } : null,
      latestRun: run ? { runRef: str(run, 'runRef'), state: optStr(run, 'state'), createdAt: optStr(run, 'createdAt') } : null,
      yesterdayNewProblemCount: optNum(root, 'yesterdayNewProblemCount'),
    };
  });
}

export function parseCurrentSignals(value: Json): { signals: VoicePreview[]; page: StudyPage } {
  return decode(value, root => { const page = obj(root, 'page'); return { signals: list(root, 'signals', parseVoice), page: { nextCursor: optStr(page, 'nextCursor'), totalCount: optNum(page, 'totalCount') } }; });
}
