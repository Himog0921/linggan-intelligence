import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NoteAnalysisPanel } from './NoteAnalysisPanel';
import type { AnalysisLatest, AnalysisPreview, AnalysisRead, AnalysisRequest } from './noteAnalysisTypes';
const coverage = { materialColumns: ['body'], includedCommentCount: 0, authorCommentCount: 0, platformCommentCount: null, issues: [] };
const empty: AnalysisLatest = { contract: 'note-analysis.latest.v1', latestRun: null, latestSuccessful: null };
const preview: AnalysisPreview = { contract: 'note-analysis.preview.v1', domainRef: 'domain', workRef: 'work', scope: 'note', knowledgeCutoff: '2026-11-04T00:00:00Z', inputHash: 'hash', taskConfigRef: 'config', taskRevision: 1, methodVersion: 'note-analysis.method.v0.3', coverage, canStart: true, reasons: [] };
const success: AnalysisRead = { contract: 'note-analysis.read.v1', requestRef: 'old', workRef: 'work', scope: 'note', run: { state: 'succeeded', batchRef: 'batch', itemRef: 'item', invocationRef: 'call', failureCode: null }, result: { contract: 'note-analysis.result.v1', summary: '三个动作回应作业启动问题。', summaryBasisIds: ['discussion'], understanding: { discussion: { id: 'discussion', text: '孩子坐下后没有开始作业。', kind: 'observation', evidenceRefs: ['e1'] }, answer: { id: 'answer', text: '作者提出三个启动动作。', kind: 'author_claim', evidenceRefs: ['e1'] }, audience: null, situation: null, offeredValue: null }, features: [], explanations: [], feedback: [], tags: [{ label: '作业启动', dimension: 'discussion', judgmentIds: ['discussion'], retrievalUse: '查找作业开始困难' }], methodCandidates: [], limitations: [], evidence: [{ id: 'e1', kind: 'material', sourceRef: 'source', sourceVersion: 'v1', citationId: 'citation', start: 0, end: 2, quote: '坐下' }] }, coverage, freshness: 'source_changed', versions: { methodVersion: 'v0.3', taskConfigRef: 'config', skillVersionRef: null, promptVersionRef: null, modelConfigRef: null, createdAt: '2026-11-04T00:00:00Z' } };
function json(value: unknown, status = 200) { return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } }); }
function mount(work = 'work') {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false }, mutations: { retry: false } } });
  return render(<QueryClientProvider client={client}><MemoryRouter><NoteAnalysisPanel domain="domain" work={work} active /></MemoryRouter></QueryClientProvider>);
}
beforeEach(() => { sessionStorage.clear(); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
describe('单篇分析手动请求与结果', () => {
  it('进入只读 latest，不自动预览或发起；失败保留最近成功并标明材料更新', async () => {
    const fetch = vi.fn(async () => json({ ...empty, latestRun: { ...success, requestRef: 'failed', run: { ...success.run, state: 'failed', failureCode: 'ai_output_invalid' }, result: null }, latestSuccessful: success }));
    vi.stubGlobal('fetch', fetch); mount();
    expect(await screen.findByText('三个动作回应作业启动问题。')).toBeInTheDocument();
    expect(screen.getByText(/以下保留最近一次成功/)).toBeInTheDocument();
    expect(screen.getByText(/返回内容未通过检查/)).toBeInTheDocument();
    expect(screen.getByText(/材料已更新/)).toBeInTheDocument();
    expect(screen.getByText('作者主张 ·')).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: '作业启动' })).toBeNull();
    expect(fetch.mock.calls).toHaveLength(1);
  });
  it('网络未知后刷新仍用原 commandRef 和冻结参数重试，不重新 preview', async () => {
    const posts: AnalysisRequest[] = []; let previews = 0;
    vi.stubGlobal('fetch', vi.fn(async (path: string, init: RequestInit) => {
      if (path.includes('/latest?')) return json(empty);
      if (path.includes('/preview?')) { previews++; return json(preview); }
      if (init.method === 'POST') { posts.push(JSON.parse(String(init.body))); if (posts.length === 1) throw new Error('network'); return json({ requestRef: 'new', batchRef: 'batch', itemRef: 'item' }, 202); }
      return json({ ...success, requestRef: 'new' });
    }));
    const first = mount();
    const start = await screen.findByRole('button', { name: '分析这篇' }); await waitFor(() => expect(start).toBeEnabled()); fireEvent.click(start);
    await screen.findByRole('button', { name: '核对原请求' });
    first.unmount(); mount();
    const retry = await screen.findByRole('button', { name: '核对原请求' });
    await waitFor(() => expect(retry).toBeEnabled()); fireEvent.click(retry);
    await screen.findByText('三个动作回应作业启动问题。');
    expect(posts).toHaveLength(2); expect(posts[1]).toEqual(posts[0]); expect(previews).toBe(1);
  });
  it('明确 409 后下一次点击重新 preview 并使用新 commandRef', async () => {
    const posts: AnalysisRequest[] = []; let previews = 0;
    vi.stubGlobal('fetch', vi.fn(async (path: string, init: RequestInit) => {
      if (path.includes('/latest?')) return json(empty);
      if (path.includes('/preview?')) { previews++; return json({ ...preview, inputHash: `hash-${previews}` }); }
      if (init.method === 'POST') { posts.push(JSON.parse(String(init.body))); return json({ error: 'input_changed' }, 409); }
      throw new Error('unexpected');
    })); mount();
    const button = await screen.findByRole('button', { name: '分析这篇' }); await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
    await screen.findByText(/材料或配置已变化/); fireEvent.click(button);
    await waitFor(() => expect(posts).toHaveLength(2));
    expect(previews).toBe(2); expect(posts[1]?.commandRef).not.toBe(posts[0]?.commandRef); expect(posts[1]?.inputHash).toBe('hash-2');
  });
  it('无可读材料不提交；保留可行动原因和设置入口', async () => {
    const fetch = vi.fn(async (path: string) => path.includes('/latest?') ? json(empty) : json({ ...preview, canStart: false, inputHash: null, reasons: ['no_readable_material'] }));
    vi.stubGlobal('fetch', fetch); mount();
    const button = await screen.findByRole('button', { name: '分析这篇' }); await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
    expect(await screen.findByText(/没有可读材料/)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: '任务设置' })).toHaveAttribute('href', '/settings/models?view=advanced&area=work&task=note_interpretation');
    expect(fetch).toHaveBeenCalledTimes(2);
  });
  it('scope 显式切换后按该范围读取和提交', async () => {
    const posts: AnalysisRequest[] = []; const paths: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string, init: RequestInit) => {
      paths.push(path);
      if (path.includes('/latest?')) return json(empty);
      if (path.includes('/preview?')) return json({ ...preview, scope: 'note_with_comments' });
      if (init.method === 'POST') { posts.push(JSON.parse(String(init.body))); return json({ error: 'input_changed' }, 409); }
      throw new Error('unexpected');
    })); mount();
    fireEvent.click(screen.getByRole('checkbox', { name: '包含已有评论' }));
    const button = screen.getByRole('button', { name: '分析这篇' }); await waitFor(() => expect(button).toBeEnabled()); fireEvent.click(button);
    await waitFor(() => expect(posts).toHaveLength(1)); expect(posts[0]?.scope).toBe('note_with_comments');
    expect(paths.some(path => path.includes('/preview?') && path.includes('scope=note_with_comments'))).toBe(true);
  });
  it('已接受任务按 requestRef 轮询，终态刷新 latest，不发起新调用', async () => {
    let reads = 0; let latestReads = 0;
    const pending = { ...success, requestRef: 'new', result: null, run: { ...success.run, state: 'running', invocationRef: null } };
    const completed = { ...success, requestRef: 'new', result: { ...success.result!, summary: '新的成功分析。' } };
    const fetch = vi.fn(async (path: string, init: RequestInit) => {
      expect(init.method).toBe('GET');
      if (path.includes('/latest?')) { latestReads++; return json({ ...empty, latestRun: reads >= 2 ? completed : pending, latestSuccessful: reads >= 2 ? completed : success }); }
      expect(path).toBe('/api/local/note-analysis/new?domainRef=domain');
      reads++; return json(reads >= 2 ? completed : pending);
    }); vi.stubGlobal('fetch', fetch); mount();
    expect(await screen.findByText('三个动作回应作业启动问题。')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText('新的成功分析。')).toBeInTheDocument(), { timeout: 3000 });
    await waitFor(() => expect(latestReads).toBeGreaterThan(1));
    expect(screen.getByRole('button', { name: '重新分析' })).toBeEnabled();
  });
  it('任务读取明确拒绝来源访问时收起旧成功正文', async () => {
    vi.stubGlobal('fetch', vi.fn(async (path: string) => path.includes('/latest?')
      ? json({ ...empty, latestRun: { ...success, requestRef: 'new', result: null, run: { ...success.run, state: 'running' } }, latestSuccessful: success })
      : json({ error: 'source_restricted' }, 403)));
    mount(); await screen.findByText(/分析记录暂不可读/);
    expect(screen.queryByText('三个动作回应作业启动问题。')).toBeNull();
    expect(screen.getByRole('button', { name: '分析处理中' })).toBeDisabled();
  });

  it('最新请求不可读但成功记录仍可读时保留结果，不宣称尚未分析', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => json({ ...empty, latestSuccessful: success })));
    mount();
    expect(await screen.findByText('三个动作回应作业启动问题。')).toBeInTheDocument();
    expect(screen.getByText('以下保留最近一次成功分析。')).toBeInTheDocument();
    expect(screen.queryByText(/尚未分析/)).toBeNull();
    expect(screen.getByRole('button', { name: '重新分析' })).toBeEnabled();
  });

  it('刷新恢复本篇已选范围，切换作品不会串用该偏好', async () => {
    const paths: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string) => {
      paths.push(path);
      return json(path.includes('scope=note_with_comments') ? { ...empty, latestRun: success, latestSuccessful: success } : empty);
    }));
    const first = mount();
    await screen.findByText(/尚未分析/);
    fireEvent.click(screen.getByRole('checkbox', { name: '包含已有评论' }));
    await screen.findByText('三个动作回应作业启动问题。');
    first.unmount();
    const reloaded = mount();
    expect(screen.getByRole('checkbox', { name: '包含已有评论' })).toBeChecked();
    await screen.findByText('三个动作回应作业启动问题。');
    expect(paths.at(-1)).toContain('scope=note_with_comments');
    reloaded.unmount(); mount('other-work');
    expect(screen.getByRole('checkbox', { name: '包含已有评论' })).not.toBeChecked();
    await screen.findByText(/尚未分析/);
  });

});
