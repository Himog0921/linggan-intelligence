import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { DeepResearchLauncher } from './DeepResearchLauncher';

const modes = {
  contract: 'content-understanding.deep-modes.v1', imageLimit: 9, methodVersion: 'content-understanding.l2.model.v2',
  modes: [
    { researchMode: 'content_design', defaultMaterialKinds: ['core_unit', 'title', 'body', 'cover'],
      defaultSubQuestions: [{ id: 'Q1', text: '标题和开头提出了什么？' }, { id: 'Q2', text: '正文按什么顺序展开？' }] },
    { researchMode: 'creation_method', defaultMaterialKinds: ['core_unit', 'title', 'body'],
      defaultSubQuestions: [{ id: 'Q1', text: '有哪些做法值得迁移？' }] },
  ],
};
function settings(enabled = true, configured = true) {
  return {
    tasks: [{ taskKey: 'content_understanding_deep', revision: 3, activeSkillVersionRef: configured ? 'skill-v' : null,
      configuration: { taskConfigRef: 'config-1', model: configured ? { modelId: 'm' } : null } }],
    controls: [{ scope: 'task:content_understanding_deep', enabled }],
  };
}
function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
}
function route(options: { enabled?: boolean; configured?: boolean; generate?: () => Response | Promise<Response> } = {}) {
  const calls: { path: string; body?: Record<string, unknown> }[] = [];
  const fetch = vi.fn(async (path: string, init?: RequestInit) => {
    calls.push({ path, body: init?.body ? JSON.parse(String(init.body)) as Record<string, unknown> : undefined });
    if (path.includes('/deep/modes')) return json(modes);
    if (path.includes('/ai-settings')) return json(settings(options.enabled, options.configured));
    if (path.includes('/preview')) return json({ imageCount: 4 });
    if (path.includes('/deep/generate')) return options.generate ? options.generate() : json({ requestRef: 'r' }, 202);
    return json({}, 404);
  });
  vi.stubGlobal('fetch', fetch);
  return calls;
}
const onSubmitted = vi.fn();
function mount() {
  return render(<DeepResearchLauncher domain="domain" work="work" coreRef="core" onSubmitted={onSubmitted}/>);
}
async function open() {
  fireEvent.click(screen.getByRole('button', { name: '深入研究' }));
  await screen.findByLabelText('子问题 1');
}
afterEach(() => { cleanup(); vi.unstubAllGlobals(); onSubmitted.mockReset(); });

it('展开之前不读取任何设置', () => {
  const calls = route();
  mount();
  expect(calls).toHaveLength(0);
});
it('展开后显示默认子问题与材料清单，图片数取预览且不超过上限', async () => {
  route();
  mount();
  await open();
  expect(screen.getByLabelText('子问题 1')).toHaveValue('标题和开头提出了什么？');
  expect(screen.getByLabelText('子问题 2')).toHaveValue('正文按什么顺序展开？');
  expect(screen.getByText(/标题、正文、封面与图片（图片 4 张，最多 9 张）/)).toBeVisible();
});
it('提交时按顺序重编子问题编号，并带上当前任务配置与核心理解', async () => {
  const calls = route();
  mount();
  await open();
  fireEvent.click(screen.getAllByRole('button', { name: '删除' })[0]!);
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
  await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
  const body = calls.find(call => call.path.includes('/deep/generate'))!.body!;
  expect(body).toMatchObject({
    domainRef: 'domain', workRef: 'work', coreRef: 'core', researchMode: 'content_design', baseMode: null,
    subQuestions: [{ id: 'Q1', text: '正文按什么顺序展开？' }], taskConfigRef: 'config-1', taskRevision: 3,
  });
  expect(typeof body.commandRef).toBe('string');
  expect(screen.queryByRole('dialog', { name: '深入研究' })).not.toBeInTheDocument();
});
it('空子问题与超过四个时不能提交或添加', async () => {
  route();
  mount();
  await open();
  fireEvent.change(screen.getByLabelText('子问题 1'), { target: { value: '   ' } });
  expect(screen.getByRole('button', { name: '提交' })).toBeDisabled();
  fireEvent.change(screen.getByLabelText('子问题 1'), { target: { value: '问题一' } });
  fireEvent.click(screen.getByRole('button', { name: '添加子问题' }));
  fireEvent.click(screen.getByRole('button', { name: '添加子问题' }));
  expect(screen.getByRole('button', { name: '添加子问题' })).toBeDisabled();
  expect(screen.getAllByLabelText(/^子问题 \d$/)).toHaveLength(4);
});
it('任务未开启或未配置时给出原因并禁止提交', async () => {
  route({ enabled: false });
  mount();
  await open();
  expect(screen.getByText('深入研究任务当前未开启。')).toBeVisible();
  expect(screen.getByRole('button', { name: '提交' })).toBeDisabled();
  cleanup();
  route({ configured: false });
  mount();
  await open();
  expect(screen.getByText('深入研究尚未完成模型与方法配置。')).toBeVisible();
});
it('服务器明确拒绝后显示原因，并为下一次提交换新编号', async () => {
  const calls = route({ generate: () => json({ error: 'ai_admission_denied', reasons: ['egress_not_approved'] }, 409) });
  mount();
  await open();
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
  expect(await screen.findByText('尚未批准把这类材料发送给模型。')).toBeVisible();
  expect(onSubmitted).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
  await waitFor(() => expect(calls.filter(call => call.path.includes('/deep/generate'))).toHaveLength(2));
  const [first, second] = calls.filter(call => call.path.includes('/deep/generate'));
  expect(second!.body!.commandRef).not.toBe(first!.body!.commandRef);
});
it('没有得到答复时沿用同一请求编号重试', async () => {
  let attempt = 0;
  const calls = route({ generate: () => { attempt += 1; if (attempt === 1) throw new TypeError('offline'); return json({ requestRef: 'r' }, 202); } });
  mount();
  await open();
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
  expect(await screen.findByText(/尚未确认/)).toBeVisible();
  fireEvent.click(screen.getByRole('button', { name: '提交' }));
  await waitFor(() => expect(onSubmitted).toHaveBeenCalledTimes(1));
  const [first, second] = calls.filter(call => call.path.includes('/deep/generate'));
  expect(second!.body!.commandRef).toBe(first!.body!.commandRef);
});
