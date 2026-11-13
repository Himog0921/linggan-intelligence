import { decode, list, num, obj, optStr, str, ShapeError } from '../../shared/decode';
export type Run = { executionMode: 'single_pass' | 'legacy_four_stage'; runRef: string; pending: number; failed: number; windows: number; stages: { window: string; phase: number; state: string; failure: string | null; wait: string | null }[] };
export type TopicProgress = { summary: { pendingStageCount: number; succeededStageCount: number; failedStageCount: number }; runs: Run[] };
export function parseTopicProgress(value: unknown): TopicProgress { return decode(value, root => { const summary = obj(root, 'summary'); return { summary: { pendingStageCount: num(summary, 'pendingStageCount'), succeededStageCount: num(summary, 'succeededStageCount'), failedStageCount: num(summary, 'failedStageCount') }, runs: parseTopicRuns(root) }; }); }
export function parseTopicRuns(value: unknown): Run[] { return decode(value, root => list(root, 'runs', row => ({ executionMode: executionMode(row), runRef: str(row, 'runRef'), pending: num(row, 'pendingStageCount'), failed: num(row, 'failedStageCount'), windows: num(row, 'windowCount'), stages: list(row, 'stages', item => ({ window: str(item, 'windowRef'), phase: num(item, 'stageNumber'), state: str(item, 'state'), failure: optStr(item, 'failureCode'), wait: optStr(item, 'waitReason') })) }))); }

function executionMode(row: Parameters<typeof str>[0]): Run['executionMode'] {
  const mode = str(row, 'executionMode');
  if (mode !== 'single_pass' && mode !== 'legacy_four_stage') throw new ShapeError('executionMode');
  return mode;
}
