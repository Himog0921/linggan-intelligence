import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { calls, DOMAIN, installFetch, json, renderPage, WORK } from './testKit';
afterEach(() => vi.unstubAllGlobals());
describe('D3 review regression boundaries', () => {
 it.each(['not-a-domain', '00000000-0000-4000-8000-000000000099'])('does not propagate the server-rejected domain %s into corpus navigation', async rejected => {
  installFetch(url => url.pathname === '/api/local/corpus/shell' ? json({ domains: [{ domainRef: DOMAIN, name: '合成领域一', status: 'active', targetCount: 2, sampleCount: 3 }], selectedDomain: null, collectionState: '观察中' }) : undefined);
  renderPage(`domain=${rejected}`); await screen.findByText('请先在页头选择观察领域，再查看评论研究。');
  expect(document.querySelector('.v7-side a[href^="/corpus/evidence"]')).toHaveAttribute('href', '/corpus/evidence');
  expect(document.querySelector('.v7-side a[href^="/corpus/creators"]')).toHaveAttribute('href', '/corpus/creators');
  expect(calls.filter(c => c.url.pathname !== '/api/local/ai-settings').map(c => c.url.pathname)).toEqual(['/api/local/corpus/shell']);
  expect(calls.some(c => c.url.pathname === '/api/local/ai-settings')).toBe(false);
 });
 it('a late work lookup cannot navigate back after leaving comments', async () => {
  let finish: (value: Response) => void = () => {}; installFetch(url => url.pathname.endsWith('/works') ? new Promise<Response>(resolve => { finish = resolve; }) : undefined);
  const { router } = renderPage(`domain=${DOMAIN}&view=comments`); await screen.findByRole('table'); await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'})); await userEvent.click(screen.getByRole('option',{name:'作品标题'})); await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'延迟'); await userEvent.click(screen.getByRole('button',{name:'筛选'})); await userEvent.click(screen.getByRole('button',{name:'完成'})); await userEvent.click(screen.getByRole('button', { name: '研究记录' }));
  await act(async () => { finish(json({ items: [], page: { nextCursor: null } })); await Promise.resolve(); }); expect(new URLSearchParams(router.state.location.search).get('view')).toBe('runs');
 });
 it('an older work search cannot overtake a newer pending search within the same route',async()=>{
  const finish=new Map<string,(value:Response)=>void>();installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish.set(url.searchParams.get('q')||'',resolve);}):undefined);renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');
  await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'作品标题'}));
  const input=screen.getByRole('searchbox',{name:'字面搜索'});await userEvent.type(input,'older');await userEvent.click(screen.getByRole('button',{name:'筛选'}));await userEvent.click(screen.getByRole('button',{name:'完成'}));await userEvent.clear(input);await userEvent.type(input,'newer');await userEvent.keyboard('{Enter}');
  await act(async()=>{finish.get('older')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 旧搜索'}],page:{nextCursor:null}}));await Promise.resolve();});
  await act(async()=>{finish.get('newer')!(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 新搜索'}],page:{nextCursor:null}}));});
  expect(await screen.findByRole('option',{name:'SYNTHETIC 新搜索'})).toBeInTheDocument();
  expect(screen.queryByRole('option',{name:'SYNTHETIC 旧搜索'})).not.toBeInTheDocument();
 });
 it('a work lookup cannot restore an old filter after the comment route changes',async()=>{
  let finish:(value:Response)=>void=()=>{};installFetch(url=>url.pathname.endsWith('/works')?new Promise<Response>(resolve=>{finish=resolve;}):undefined);const {router}=renderPage(`domain=${DOMAIN}&view=comments`);await screen.findByRole('table');await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'作品标题'}));await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'过期');await userEvent.click(screen.getByRole('button',{name:'筛选'}));await userEvent.click(screen.getByRole('button',{name:'完成'}));await userEvent.click(screen.getByRole('combobox',{name:'搜索范围'}));await userEvent.click(screen.getByRole('option',{name:'评论文本'}));await userEvent.clear(screen.getByRole('searchbox',{name:'字面搜索'}));await userEvent.type(screen.getByRole('searchbox',{name:'字面搜索'}),'new-filter');await userEvent.keyboard('{Enter}');await waitFor(()=>expect(router.state.location.search).toContain('q=new-filter'));
  await act(async()=>{finish(json({items:[{workRef:WORK,displayTitle:'SYNTHETIC 过期搜索'}],page:{nextCursor:null}}));await Promise.resolve();});expect(router.state.location.search).toContain('q=new-filter');expect(screen.queryByRole('option',{name:'SYNTHETIC 过期搜索'})).not.toBeInTheDocument();
 });

 it('a failed background shell refresh preserves the current comment filter and selection without a write', async () => {
  let shellReads = 0;
  installFetch(url => url.pathname === '/api/local/corpus/shell' && ++shellReads > 1 ? json({ error: 'unavailable' }, 503) : undefined);
  const { queryClient } = renderPage(`domain=${DOMAIN}&view=comments&q=kept`);
  await screen.findByRole('table'); await userEvent.click(screen.getByRole('button', { name: '选择评论' })); await userEvent.click(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]!);
  await act(async () => { await queryClient.refetchQueries({ queryKey: ['corpus-shell', DOMAIN] }); });
  await screen.findByText(/领域信息刷新失败/);
  expect(screen.getByRole('searchbox', { name: '字面搜索' })).toHaveValue('kept');
  expect(screen.getAllByRole('checkbox', { name: '选择这条评论' })[0]).toBeChecked();
  expect(calls.every(c => c.method === 'GET')).toBe(true);
 });

});
