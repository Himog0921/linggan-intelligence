export type AnalysisScope = 'note' | 'note_with_comments';
export type AnalysisState = 'pending' | 'running' | 'succeeded' | 'failed' | 'skipped' | 'cancelled';
export interface AnalysisStatement { id: string; text: string; kind: 'observation' | 'author_claim' | 'interpretation'; evidenceRefs: string[] }
export interface AnalysisFeature extends AnalysisStatement { label: string; category: 'answer_form' | 'expression' | 'participation' }
export interface AnalysisExplanation extends AnalysisStatement { kind: 'interpretation'; featureIds: string[]; feedbackIds: string[]; support: string; uncertainty: string | null }
export interface AnalysisEvidence { id: string; kind: 'material' | 'comment'; sourceRef: string; sourceVersion: string; citationId: string; start: number; end: number; quote: string }
export interface AnalysisResult {
  contract: 'note-analysis.result.v1'; summary: string; summaryBasisIds: string[];
  understanding: { discussion: AnalysisStatement; answer: AnalysisStatement; audience: AnalysisStatement | null; situation: AnalysisStatement | null; offeredValue: AnalysisStatement | null };
  features: AnalysisFeature[]; feedback: AnalysisStatement[]; explanations: AnalysisExplanation[];
  tags: { label: string; dimension: 'discussion' | 'audience' | 'situation' | 'offered_value' | 'answer_form' | 'expression' | 'participation' | 'response'; judgmentIds: string[]; retrievalUse: string }[];
  methodCandidates: { id: string; text: string; kind: 'reuse_idea'; basisIds: string[]; conditions: string; openQuestion: string | null }[];
  limitations: string[]; evidence: AnalysisEvidence[];
}
export interface AnalysisCoverage { materialColumns: string[]; includedCommentCount: number; authorCommentCount: number; platformCommentCount: number | null; issues: string[] }
export interface AnalysisPreview {
  contract: 'note-analysis.preview.v1'; domainRef: string; workRef: string; scope: AnalysisScope;
  knowledgeCutoff: string; inputHash: string | null; taskConfigRef: string | null; taskRevision: number | null; methodVersion: string;
  coverage: AnalysisCoverage; canStart: boolean; reasons: string[];
}
export interface AnalysisRequest { commandRef: string; domainRef: string; workRef: string; scope: AnalysisScope; knowledgeCutoff: string; inputHash: string; taskConfigRef: string; taskRevision: number }
export interface AnalysisReceipt { requestRef: string; batchRef: string; itemRef: string }
export interface AnalysisRead {
  contract: 'note-analysis.read.v1'; requestRef: string; workRef: string; scope: AnalysisScope;
  run: { state: AnalysisState; batchRef: string; itemRef: string; invocationRef: string | null; failureCode: string | null };
  result: AnalysisResult | null; coverage: AnalysisCoverage; freshness: 'current' | 'source_changed' | 'not_checked';
  versions: { methodVersion: string; taskConfigRef: string; skillVersionRef: string | null; promptVersionRef: string | null; modelConfigRef: string | null; createdAt: string };
}
export interface AnalysisLatest { contract: 'note-analysis.latest.v1'; latestRun: AnalysisRead | null; latestSuccessful: AnalysisRead | null }
