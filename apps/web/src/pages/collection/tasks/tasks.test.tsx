import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '../../../shared/api';
import { RunCreationDialog } from './RunCreationDialog';
import { controlRun, createRun } from './runRequests';
import { commentModeLabel, parseRun, parseWorks, readRun, sourceLabel } from './tasksApi';

const domain = 'a0000000-0000-4000-8000-000000000001';
const run = 'a0000000-0000-4000-8000-000000000002';
const request = 'a0000000-0000-4000-8000-000000000003';
const legacy = { run_ref: run, domain_ref: domain, name: '合成历史采集', domain_name: '合成领域', source_kind: 'patrol', legacy: true, ended: true, control_intent: null, phase: 'ended', comment_mode: null, created_at: '2026-10-14T00:00:00Z', updated_at: '2026-10-14T00:00:00Z', revision: null, work_limit: 20, selected_works: 20, available_details: 0, delivered_comments: 0, reused_details: null, new_comments: null };
function json(value: unknown, status = 200) { return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } }); }
afterEach(() => { cleanup(); vi.unstubAllGlobals(); sessionStorage.clear(); });

describe('采集任务数据与失联恢复', () => {
  it('关闭在途创建弹窗后，迟到成功保留原身份但不会重新打开任务', async () => {
    let finish!: (response: Response) => void;
    const pending = new Promise<Response>(resolve => { finish = resolve; });
    const fetch = vi.fn(async (_path: string, options: RequestInit) => options.method === 'POST' ? pending : json({readable:true,domains:[{domainRef:domain,name:'合成领域',status:'active'}]}));
    vi.stubGlobal('fetch', fetch);
    const created = vi.fn();
    const view = render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RunCreationDialog params={new URLSearchParams({domain})} onClose={()=>{}} onCreated={created}/></QueryClientProvider>);
    await screen.findByRole('option',{name:'合成领域'});
    const user=userEvent.setup();
    await user.type(screen.getByRole('textbox',{name:'这次想找什么 *'}),'合成在途问题');
    await user.click(screen.getByRole('button',{name:'▷ 发起采集'}));
    await waitFor(()=>expect(fetch.mock.calls.some(c=>c[1].method==='POST')).toBe(true));
    const saved=sessionStorage.getItem(`collection-run-draft:${domain}`);
    view.unmount();
    await act(async()=>{finish(json({runRef:run,domainRef:domain}));await pending;});
    expect(created).not.toHaveBeenCalled();
    expect(sessionStorage.getItem(`collection-run-draft:${domain}`)).toBe(saved);
  });
  it('评论范围的写法：尽量采全、指定条数（Mog 2026-10-23）、30 条样本；条数缺失不编造', () => {
    const current = { ...legacy, legacy: false, control_intent: 'running', revision: 1 };
    expect(commentModeLabel(parseRun({ ...current, comment_mode: 'all_public' }), '样本')).toBe('尽量采全，含回复');
    expect(commentModeLabel(parseRun({ ...current, comment_mode: 'up_to', comment_target: 40 }), '样本')).toBe('每篇采到 40 条就停，含回复');
    expect(commentModeLabel(parseRun({ ...current, comment_mode: 'up_to' }), '样本')).toBe('每篇采到指定条数，含回复');
    expect(commentModeLabel(parseRun({ ...current, comment_mode: 'sample_30' }), '每篇最多30条总样本')).toBe('每篇最多30条总样本');
    expect(() => parseRun({ ...current, comment_mode: 'up_to', comment_target: -1 })).toThrow(ApiError);
  });
  it('主页浅看与精选研究：文字版写成「只取笔记文字」，来源有自己的名字，文字版以详情完成为收尾', () => {
    const current = { ...legacy, legacy: false, control_intent: 'running', revision: 1 };
    expect(commentModeLabel(parseRun({ ...current, comment_mode: 'none' }), '每篇最多30条总样本')).toBe('只取笔记文字，不采评论与媒体');
    expect(sourceLabel('creator_preview')).toBe('作者主页浅看');
    expect(sourceLabel('creator_selected_works')).toBe('主页精选研究');
    expect(sourceLabel('something_new')).toBe('既有采集');
    const works = (checkpoint: unknown) => parseWorks({
      runRef: run, domainRef: domain, total: 1,
      items: [{ publicRef: run, ordinal: 1, title: '作品', creator: '作者', gapReason: null, checkpoint }],
    }).items[0]!.commentsComplete;
    expect(works({ detailComplete: true, commentsNotRequested: true })).toBe(true);
    expect(works({ commentsComplete: true })).toBe(true);
    expect(works({})).toBeNull();
    expect(works(null)).toBeNull();
  });
  it('历史记录允许不存在的控制版本，保留未知与真实零；新版记录缺版本拒绝展示', () => {
    const r = parseRun(legacy); expect(r.revision).toBeNull(); expect(r.new_comments).toBeNull(); expect(r.available_details).toBe(0);
    expect(() => parseRun({ ...legacy, legacy: false })).toThrow(ApiError);
    expect(() => parseRun({ ...legacy, available_details: -1 })).toThrow(ApiError);
    expect(parseWorks({ runRef: run, domainRef: domain, total: 0, items: [] }).items).toEqual([]);
  });
  it('读取到其他领域或任务的结果会拒绝，不会把旧数据塞入新抽屉', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(json({ ...legacy, domain_ref: run })));
    await expect(readRun(run, domain)).rejects.toBeInstanceOf(ApiError);
  });
  it('创建响应丢失后只读原身份回执，404仍保留未知，再读成功不重复POST', async () => {
    const fetch = vi.fn().mockRejectedValueOnce(new TypeError('offline')).mockResolvedValueOnce(json({ code: 'collection_run_not_found' }, 404)).mockResolvedValueOnce(json({ runRef: run, domainRef: domain }));
    vi.stubGlobal('fetch', fetch);
    const intent = { domainRef: domain, source: { kind: 'temporary_keyword', query: '合成问题' } };
    const attempt = { identity: request, fingerprint: JSON.stringify(intent), sent: false };
    await expect(createRun(attempt, intent)).rejects.toBeInstanceOf(ApiError);
    await expect(createRun(attempt, intent)).rejects.toBeInstanceOf(ApiError);
    await expect(createRun(attempt, intent)).resolves.toEqual({ runRef: run, domainRef: domain });
    expect(fetch.mock.calls.map(c => c[1].method)).toEqual(['POST', 'GET', 'GET']);
    expect(fetch.mock.calls[1]?.[0]).toBe(`/api/collection/runs/requests/${request}`);
  });
  it('控制回执必须匹配原任务、动作与版本，响应丢失后不再提交', async () => {
    const fetch = vi.fn().mockRejectedValueOnce(new TypeError('offline')).mockResolvedValueOnce(json({ runRef: run, commandRef: request, controlIntent: 'stopped', revision: 2 }));
    vi.stubGlobal('fetch', fetch);
    const intent = { run, domainRef: domain, expectedRevision: 1, action: 'pause' as const };
    const attempt = { identity: request, fingerprint: JSON.stringify(intent), sent: false };
    await expect(controlRun(attempt, intent)).rejects.toBeInstanceOf(ApiError);
    await expect(controlRun(attempt, intent)).rejects.toBeInstanceOf(ApiError);
    expect(fetch.mock.calls.map(c => c[1].method)).toEqual(['POST', 'GET']);
  });
  it('原观察目标已不可选时，未确认请求仍核对原意图，不按新读取的来源重算', async () => {
    const target='a0000000-0000-4000-8000-000000000004';
    const intent={domainRef:domain,source:{kind:'observation_target',targetRef:target},ranking:null,workLimit:50,commentMode:'sample_30'};
    sessionStorage.setItem(`collection-run-draft:${domain}`,JSON.stringify({draft:{domain,source:'observation_target',query:'',target,ranking:'comprehensive',workLimit:'50',commentMode:'sample_30'},request:{identity:request,fingerprint:JSON.stringify(intent),sent:true}}));
    const calls:string[]=[];
    vi.stubGlobal('fetch',vi.fn(async (path:string)=>{calls.push(path);if(path.includes('/requests/'))return json({runRef:run,domainRef:domain});if(path.includes('/runs/targets'))return json({total:0,items:[]});return json({readable:true,domains:[{domainRef:domain,name:'合成领域',status:'active'}]});}));
    const created=vi.fn();
    render(<QueryClientProvider client={new QueryClient({defaultOptions:{queries:{retry:false}}})}><RunCreationDialog params={new URLSearchParams({domain})} onClose={()=>{}} onCreated={created}/></QueryClientProvider>);
    await userEvent.setup().click(await screen.findByRole('button',{name:'核实原请求回执'}));
    await waitFor(()=>expect(created).toHaveBeenCalledWith({runRef:run,domainRef:domain}));
    expect(calls.filter(p=>p.includes('/requests/'))).toHaveLength(1);
  });
  it('新建弹窗打开不写；失联保留草稿与身份，重开后也只查询原请求', async () => {
    vi.spyOn(HTMLDialogElement.prototype, 'showModal').mockImplementation(function (this: HTMLDialogElement) { this.setAttribute('open', ''); });
    vi.spyOn(HTMLDialogElement.prototype, 'close').mockImplementation(function (this: HTMLDialogElement) { this.removeAttribute('open'); });
    const calls: { path: string; method: string }[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string, options: RequestInit) => {
      calls.push({ path, method: options.method ?? 'GET' });
      if (path === '/api/collection/workspace/domains') return json({ readable:true, domains: [{ domainRef: domain, name: '合成领域', status: 'active' }] });
      if (options.method === 'POST') throw new TypeError('offline');
      return json({ code: 'collection_run_not_found' }, 404);
    }));
    const show = () => render(<QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}><RunCreationDialog params={new URLSearchParams({ domain })} onClose={() => {}} onCreated={() => {}} /></QueryClientProvider>);
    const first = show();
    await screen.findByRole('option', { name: '合成领域' }); expect(calls.every(c => c.method === 'GET')).toBe(true);
    const user = userEvent.setup(); await user.type(screen.getByRole('textbox', { name: '这次想找什么 *' }), '合成问题'); await user.click(screen.getByRole('button', { name: '▷ 发起采集' }));
    await screen.findByRole('button', { name: '核实原请求回执' });
    first.unmount(); show(); await user.click(screen.getByRole('button', { name: '核实原请求回执' }));
    await waitFor(() => expect(calls.filter(c => c.path.includes('/requests/'))).toHaveLength(1));
    expect(calls.filter(c => c.method === 'POST')).toHaveLength(1);
    expect(screen.getByRole('textbox', { name: '这次想找什么 *' })).toHaveValue('合成问题');
  });
});
