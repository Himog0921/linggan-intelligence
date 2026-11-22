import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { ContentLensPage } from './ContentLensPage';

const rate = { numerator:1, denominator:1, value:null, low:null, high:null, reason:'below_min_n' };
const group = { key:'method', name:'操作方法', publications:2, share:1 };
const row = { key:'method', medianViews:1200, highCount:0, midRate:rate, activeCommentRate:rate,
  commentsPerThousandViews:{value:0,publications:1}, withinAccountResidual:{value:null,publications:0} };
const body = { schemaVersion:'ops-content-lens.snapshot.v1', snapshotRef:'snapshot-1',
  quality:{ latestPublishedDate:'2026-01-01',staleDays:0,isStale:false,publications:2,pendingTitles:0,withViews:1,titled:2,withViewsShare:0.5,
    taxonomyVersion:1,metricVersion:1,adjudicatedTitles:0,agreement:[] },
  structure:{ offer:[group],moment:[group],situation:[group] },
  coverage:{ cells:[{ row:'method',column:'method',publications:2,state:'sparse' }],top10Share:1,emptyCells:0,sparseCells:1 },
  effect:{ grade:'hypothesis',byDimension:{offer:{available:true,rows:[row]},moment:{available:false,rows:null}}, varianceShare:[] },
};
function show(answer:(url:URL)=>Response) {
  const requests:URL[] = [];
  vi.stubGlobal('fetch',vi.fn(async (path:string,init?:RequestInit) => {
    expect(init?.method ?? 'GET').toBe('GET');
    const url = new URL(path,'http://localhost'); requests.push(url); return answer(url);
  }));
  const client = new QueryClient({defaultOptions:{queries:{retry:false}}});
  render(<QueryClientProvider client={client}><ContentLensPage domain="d-1" /></QueryClientProvider>);
  return requests;
}
afterEach(() => vi.unstubAllGlobals());
it('uses the frozen ref, dimension, metric and subset for both sides of a suppressed rate', async () => {
  const requests = show(url => {
    if (url.pathname.endsWith('status')) return Response.json({enabled:true,run:null});
    if (url.pathname.endsWith('snapshot')) return Response.json({snapshot:body,freshness:{staleDays:24,isStale:true}});
    return Response.json({snapshotRef:'snapshot-1',offset:0,limit:25,total:1,items:[{
      publicationRef:'pub-1', title:'同标题', accountName:'账号甲', businessDate:null, views:1200, comments:null,
    }]});
  });
  await screen.findByText(/数据陈旧.*24 天/);
  expect(requests.find(u => u.pathname.endsWith('snapshot'))?.searchParams.get('domainRef')).toBe('d-1');
  for (const [word, subset] of [['分子','numerator'],['分母','denominator']]) {
    fireEvent.click(screen.getByRole('button',{name:'操作方法过千率'+word+' 1'}));
    const dialog = await screen.findByRole('dialog');
    await within(dialog).findByText('同标题');
    expect(within(dialog).getByText('未填')).toBeVisible();
    expect(within(dialog).getByText('1200')).toBeVisible();
    const latest = requests.filter(u => u.pathname.endsWith('publications')).at(-1)!;
    expect(Object.fromEntries(latest.searchParams)).toMatchObject({snapshotRef:'snapshot-1',offer:'method',metric:'overThousand',subset});
    fireEvent.click(within(dialog).getByRole('button',{name:'关闭'}));
  }
  fireEvent.click(within(screen.getByRole('navigation',{name:'效果维度'})).getByRole('button',{name:'时刻'}));
  expect(await screen.findByText(/复标一致率不足/)).toBeVisible();
});
it('keeps disabled and in-progress routes from requesting a snapshot', async () => {
  const requests = show(() => Response.json({ enabled:false,run:null }));
  expect(await screen.findByText('内容回溯未启用。')).toBeVisible();
  expect(requests.every(u => u.pathname.endsWith('status'))).toBe(true);
});
it('shows the server progress and never presents effects while labelling is in progress', async () => {
  const requests = show(() => Response.json({ enabled:true,run:{inProgress:true,progress:0.25,primary:{state:'running',succeededShards:1,totalShards:4},audit:null} }));
  expect(await screen.findByText('25%')).toBeVisible();
  expect(screen.queryByText('发布效果')).not.toBeInTheDocument();
  expect(requests.every(u => u.pathname.endsWith('status'))).toBe(true);
});
it('reports a read failure and allows retry to an empty scope', async () => {
  let fail = true;
  show(url => {
    if (url.pathname.endsWith('status')) return Response.json({enabled:true,run:null});
    if (fail) { fail = false; return Response.json({code:'unavailable'},{status:503}); }
    return Response.json({snapshot:null,freshness:null});
  });
  await screen.findByText(/快照读取失败/);
  fireEvent.click(screen.getByRole('button',{name:'重试'}));
  await waitFor(() => expect(screen.getByText(/暂无标注数据/)).toBeVisible());
});
