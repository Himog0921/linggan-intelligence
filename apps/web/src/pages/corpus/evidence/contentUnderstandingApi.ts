import { useQuery } from '@tanstack/react-query';
import { isRecord, requestJson } from '../../../shared/api';
import { parseMaterialReadiness } from './evidenceDecode';
import type { MaterialReadiness } from './evidenceTypes';

type Unit = { id: string; unitType: string; statement: string };
type DeepSupport = { excerpt?: string; observation?: string };
type DeepFinding = { id: string; statement: string; evidenceRefs?: string[]; support?: DeepSupport[] };
type DeepMaterial = { id: string; materialKind: string };
type DeepAnalysis = {
  analysisRef: string; researchMode: string | null; baseMode: string | null;
  coreBasis: null | { kind?: string }; question: string; methodVersion: string;
  findings: DeepFinding[]; materials: DeepMaterial[];
};
export type Understanding = {
  materialReadiness: MaterialReadiness;
  currentRequest?: { state: string; failureCode: string | null };
  deepAnalyses?: DeepAnalysis[];
  deepEvidence?: Record<string, string[]>;
  analysis: null | { state: string; sourceChanged?: boolean; failureCode: string | null; result: null | {
    card: { analysisRef?: string; core: { summary: { text: string }; knowledgeUnits: Unit[] } };
  } };
};
function readDeep(entries: unknown): DeepAnalysis[] | undefined {
  if (!Array.isArray(entries)) return;
  const analyses: DeepAnalysis[] = [];
  for (const entry of entries) {
    if (!isRecord(entry) || typeof entry.analysisRef !== 'string' || typeof entry.question !== 'string'
        || typeof entry.methodVersion !== 'string' || !Array.isArray(entry.findings)) return;
    const findings: DeepFinding[] = [];
    for (const finding of entry.findings) {
      if (!isRecord(finding) || typeof finding.id !== 'string' || typeof finding.statement !== 'string') return;
      const refs = finding.evidenceRefs;
      const evidenceRefs = Array.isArray(refs) && refs.every((ref): ref is string => typeof ref === 'string')
        ? refs : undefined;
      const items = finding.support;
      const support = Array.isArray(items)
        ? items.filter((item): item is DeepSupport => isRecord(item)
          && (typeof item.excerpt === 'string' || typeof item.observation === 'string'))
        : undefined;
      if (!evidenceRefs && !support) return;
      findings.push({ id: finding.id, statement: finding.statement, ...(evidenceRefs ? { evidenceRefs } : {}), ...(support ? { support } : {}) });
    }
    const materials = Array.isArray(entry.materials)
      ? entry.materials.filter((material): material is DeepMaterial => isRecord(material)
        && typeof material.id === 'string' && typeof material.materialKind === 'string')
      : [];
    const basis = entry.coreBasis;
    const coreBasis = isRecord(basis) && typeof basis.kind === 'string' ? { kind: basis.kind } : null;
    analyses.push({
      analysisRef: entry.analysisRef, question: entry.question, methodVersion: entry.methodVersion,
      researchMode: typeof entry.researchMode === 'string' ? entry.researchMode : null,
      baseMode: typeof entry.baseMode === 'string' ? entry.baseMode : null,
      coreBasis, findings, materials,
    });
  }
  return analyses;
}
export async function readContentUnderstanding(domain: string, work: string, asOf: string, signal: AbortSignal): Promise<Understanding> {
  const value = await requestJson(`/api/local/content-understanding/latest?${new URLSearchParams({ domainRef: domain, workRef: work, asOf })}`, { signal });
  if (!isRecord(value) || value.contract !== 'content-understanding.read.v1' || value.workRef !== work) throw new Error('理解结果无法读取');
  const materialReadiness = parseMaterialReadiness(value.materialReadiness);
  const currentRequest = isRecord(value.currentRequest) && typeof value.currentRequest.state === 'string' ? { state: value.currentRequest.state, failureCode: typeof value.currentRequest.failureCode === 'string' ? value.currentRequest.failureCode : null } : undefined;
  const analysis = value.analysis;
  if (analysis === null) return { analysis: null, materialReadiness, currentRequest };
  if (!isRecord(analysis) || !['pending', 'running', 'succeeded', 'failed', 'skipped', 'cancelled'].includes(String(analysis.state))) throw new Error('理解状态无法读取');
  let deepAnalyses: DeepAnalysis[] | undefined;
  let deepEvidence: Record<string, string[]> | undefined;
  if (analysis.result !== null) {
    const result = analysis.result;
    if (analysis.state !== 'succeeded' || !isRecord(result) || !isRecord(result.card) || result.card.workRef !== work || result.card.schemaVersion !== 'content-card.v0.2' || !isRecord(result.card.core) || !isRecord(result.card.core.summary) || typeof result.card.core.summary.text !== 'string' || !Array.isArray(result.card.core.knowledgeUnits) || !result.card.core.knowledgeUnits.every(u => isRecord(u) && typeof u.id === 'string' && typeof u.unitType === 'string' && typeof u.statement === 'string')) throw new Error('理解内容无法读取');
    deepAnalyses = readDeep(analysis.deepAnalyses);
    const evidenceItems = Array.isArray(result.card.evidence) ? result.card.evidence : [];
    deepEvidence = Object.fromEntries(evidenceItems.flatMap(item => {
      if (!isRecord(item) || typeof item.id !== 'string') return [];
      const excerpts = [...new Set([item.quote, item.modelQuote, item.observation]
        .filter((text): text is string => typeof text === 'string' && text.trim() !== ''))];
      return excerpts.length ? [[item.id, excerpts]] : [];
    }));
  } else if (analysis.state === 'succeeded') throw new Error('理解内容缺失');
  return { analysis: analysis as Understanding['analysis'], materialReadiness, currentRequest, deepAnalyses, deepEvidence };
}

export type DeepResearchWatch = { count: number; submittedAt: number };
export const contentUnderstandingQueryKey = (domain: string, work: string, revision: string, asOf: string) =>
  ['content-understanding', domain, work, revision, asOf] as const;
export const deepResearchWatchKey = (domain: string, work: string) =>
  ['content-understanding-deep-watch', domain, work] as const;

export function useContentUnderstanding(domain: string, work: string, revision: string, asOf: string, active: boolean, deepWatchCount: number | null = null) {
  return useQuery({
    queryKey: contentUnderstandingQueryKey(domain, work, revision, asOf),
    queryFn: ({ signal }) => readContentUnderstanding(domain, work, asOf, signal),
    enabled: active,
    retry: false,
    gcTime: 0,
    staleTime: 10_000,
    refetchOnWindowFocus: false,
    refetchInterval: query => {
      if (['pending', 'running'].includes(query.state.data?.currentRequest?.state ?? query.state.data?.analysis?.state ?? '')) return 1500;
      if (deepWatchCount !== null) return 4000;
      return false;
    },
  });
}
