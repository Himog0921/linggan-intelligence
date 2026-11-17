import { isRecord, requestJson } from '../../../shared/api';

export const DEEP_MODE_LABELS: Record<string, string> = {
  content_design: '内容设计', content_value: '内容价值', creation_method: '创作方法',
};

export const REASON_COPY: Record<string, string> = {
  egress_not_approved: '尚未批准把这类材料发送给模型。',
  content_understanding_requires_high: '深入研究需要高思考档位，当前模型配置不满足。',
  input_too_large: '材料超出当前模型的输入上限。',
  ai_prior_dispatch_unresolved: '这篇之前有一次处理结果尚未确认，暂时不能重新发起。',
  task_disabled: '深入研究任务当前未开启。',
};

export type Mode = { researchMode: string; defaultSubQuestions: string[]; defaultMaterialKinds: string[] };
export type Setup = {
  modes: Mode[]; taskConfigRef: string; taskRevision: number;
  unavailable: string | null; imageCount: number; imageLimit: number;
};

function readModes(value: unknown): { modes: Mode[]; imageLimit: number } {
  if (!isRecord(value) || value.contract !== 'content-understanding.deep-modes.v1' || !Array.isArray(value.modes)) throw new Error('modes');
  const modes = value.modes.map((entry): Mode => {
    if (!isRecord(entry) || typeof entry.researchMode !== 'string' || !Array.isArray(entry.defaultSubQuestions) || !Array.isArray(entry.defaultMaterialKinds)) throw new Error('modes');
    const questions = entry.defaultSubQuestions.map(item => {
      if (!isRecord(item) || typeof item.text !== 'string') throw new Error('modes');
      return item.text;
    });
    const kinds = entry.defaultMaterialKinds.filter((kind): kind is string => typeof kind === 'string');
    return { researchMode: entry.researchMode, defaultSubQuestions: questions, defaultMaterialKinds: kinds };
  });
  return { modes, imageLimit: typeof value.imageLimit === 'number' ? value.imageLimit : 9 };
}
function readTask(value: unknown): { taskConfigRef: string; taskRevision: number; unavailable: string | null } {
  if (!isRecord(value) || !Array.isArray(value.tasks)) throw new Error('settings');
  const task = value.tasks.find(item => isRecord(item) && item.taskKey === 'content_understanding_deep');
  if (!isRecord(task) || typeof task.revision !== 'number' || !isRecord(task.configuration) || typeof task.configuration.taskConfigRef !== 'string') throw new Error('settings');
  const configured = task.configuration.model !== null && typeof task.activeSkillVersionRef === 'string';
  const controls = Array.isArray(value.controls) ? value.controls : [];
  const control = controls.find(item => isRecord(item) && item.scope === 'task:content_understanding_deep');
  const enabled = isRecord(control) && control.enabled === true;
  return {
    taskConfigRef: task.configuration.taskConfigRef, taskRevision: task.revision,
    unavailable: !configured ? '深入研究尚未完成模型与方法配置。' : !enabled ? REASON_COPY.task_disabled! : null,
  };
}
function readImageCount(value: unknown): number {
  return isRecord(value) && typeof value.imageCount === 'number' ? value.imageCount : 0;
}
export async function loadSetup(domain: string, work: string): Promise<Setup> {
  const query = new URLSearchParams({ domainRef: domain, workRef: work });
  const [modes, settings, preview] = await Promise.all([
    requestJson('/api/local/content-understanding/deep/modes'),
    requestJson('/api/local/ai-settings'),
    requestJson(`/api/local/content-understanding/preview?${query}`),
  ]);
  const { modes: list, imageLimit } = readModes(modes);
  return { modes: list, ...readTask(settings), imageCount: readImageCount(preview), imageLimit };
}
export interface DeepRequest {
  commandRef: string; domainRef: string; workRef: string; coreRef: string; researchMode: string; baseMode: null;
  subQuestions: { id: string; text: string }[]; taskConfigRef: string; taskRevision: number;
}
export async function submitDeepRequest(request: DeepRequest): Promise<void> {
  await requestJson('/api/local/content-understanding/deep/generate', { method: 'POST', body: request });
}
