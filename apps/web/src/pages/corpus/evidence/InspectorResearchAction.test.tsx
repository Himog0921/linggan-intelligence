import { QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { InspectorResearchAction } from './InspectorResearchAction';
import { deepResearchWatchKey } from './DossierUnderstanding';
import { newClient } from './testKit';

const ready = { state: 'ready', kind: 'image_text', gaps: [], scopeNote: null };
const modes = {
  contract: 'content-understanding.deep-modes.v1',
  imageLimit: 9,
  methodVersion: 'content-understanding.l2.model.v2',
  modes: [{ researchMode: 'content_design', defaultMaterialKinds: ['core_unit', 'title', 'body'],
    defaultSubQuestions: [{ id: 'Q1', text: '标题和开头提出了什么？' }] }],
};
const settings = {
  tasks: [{ taskKey: 'content_understanding_deep', revision: 3, activeSkillVersionRef: 'skill-v',
    configuration: { taskConfigRef: 'config-1', model: { modelId: 'm' } } }],
  controls: [{ scope: 'task:content_understanding_deep', enabled: true }],
};
function understanding(sourceChanged = false) {
  return {
    contract: 'content-understanding.read.v1',
    workRef: 'work',
    materialReadiness: ready,
    analysis: {
      state: 'succeeded',
      sourceChanged,
      failureCode: null,
      result: { card: {
        analysisRef: 'core-1',
        workRef: 'work',
        schemaVersion: 'content-card.v0.2',
        core: { summary: { text: 'L1 核心理解' }, knowledgeUnits: [] },
        evidence: [],
      } },
    },
  };
}
function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
}
function mount(analysis: unknown) {
  const client = newClient();
  const calls: string[] = [];
  const fetch = vi.fn(async (path: string, init?: RequestInit) => {
    calls.push(path);
    if (path.includes('/content-understanding/latest')) return json({ ...understanding(), analysis });
    if (path.includes('/deep/modes')) return json(modes);
    if (path.includes('/ai-settings')) return json(settings);
    if (path.includes('/preview')) return json({ imageCount: 2 });
    if (path.includes('/deep/generate')) return json({ requestRef: 'request-1' }, 202);
    return json({}, 404);
  });
  vi.stubGlobal('fetch', fetch);
  const submitted = vi.fn();
  render(<QueryClientProvider client={client}>
    <InspectorResearchAction domain="domain" work="work" revision="r1" asOf="2026-11-18T00:00:00Z" onSubmitted={submitted}/>
  </QueryClientProvider>);
  return { client, calls, submitted };
}
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });

it('没有有效 L1 Core 时禁用 L2 入口', async () => {
  mount(null);
  const button = await screen.findByRole('button', { name: '深入研究' });
  expect(button).toBeDisabled();
  expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
});

it('L1 材料已变化时不允许用旧 Core 发起 L2', async () => {
  mount(understanding(true).analysis);
  expect(await screen.findByRole('button', { name: '深入研究' })).toBeDisabled();
});

it('复用当前 L1 Core 打开 L2 弹窗，提交后写入跨页签轮询信号', async () => {
  const { client, submitted } = mount(understanding().analysis);
  const button = await screen.findByRole('button', { name: '深入研究' });
  expect(button).toBeEnabled();
  fireEvent.click(button);
  expect(await screen.findByRole('dialog', { name: '深入研究' })).toBeVisible();
  fireEvent.click(await screen.findByRole('button', { name: '提交' }));
  await waitFor(() => expect(submitted).toHaveBeenCalledTimes(1));
  expect(screen.queryByRole('dialog', { name: '深入研究' })).not.toBeInTheDocument();
  expect(client.getQueryData(deepResearchWatchKey('domain', 'work'))).toMatchObject({ count: 0 });
});
