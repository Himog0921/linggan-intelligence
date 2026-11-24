import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';
import { newDocument } from '../../workbench/workbenchModel';
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
function show(answer:(url:URL, init?:RequestInit)=>Response, allowWrite = false,
  client = new QueryClient({defaultOptions:{queries:{retry:false}}})) {
  const requests:URL[] = [];
  vi.stubGlobal('fetch',vi.fn(async (path:string,init?:RequestInit) => {
    if (!allowWrite) expect(init?.method ?? 'GET').toBe('GET');
    const url = new URL(path,'http://localhost'); requests.push(url); return answer(url,init);
  }));
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
  fireEvent.click(within(screen.getByRole('navigation',{name:'效果维度'})).getByRole('button',{name:'选题内容'}));
  expect(await screen.findByText(/复标一致率不足/)).toBeVisible();
});
it.each([
  { state: '登记时关联一致', currentLink: 'frozen-link', currentRef: 'pub-2', visible: '登记时保存的正文' },
  { state: '解除后重新关联同一版本', currentLink: 'new-link', currentRef: 'pub-2', visible: '快照中的稿件关联在当前台账已不可用。' },
  { state: '台账只有同名其他记录', currentLink: 'frozen-link', currentRef: 'pub-1', visible: '此快照中的发布记录在当前台账已不可用。' },
])('按发布与关联身份下钻：$state', async ({ currentLink, currentRef, visible }) => {
  const workId = '00000000-0000-4000-8000-000000000001';
  const revisionId = '00000000-0000-4000-8000-000000000002';
  const frozenLink = { linkRef: 'frozen-link', workId, revisionId };
  const revisionPath = `/ops/api/creation/works/${workId}/revisions/${revisionId}`;
  const requests = show(url => {
    if (url.pathname.endsWith('/status')) return Response.json({enabled:true,run:null});
    if (url.pathname.endsWith('/snapshot')) return Response.json({snapshot:body,freshness:{staleDays:0,isStale:false}});
    if (url.pathname.endsWith('/content-lens/publications')) {
      return Response.json({snapshotRef:'snapshot-1',offset:0,limit:25,total:2,items:['pub-1','pub-2'].map(publicationRef => ({
        publicationRef, title:'同名发布', accountName:'账号甲', businessDate:'2026-01-01', views:1200, comments:null,
        creationLinks: publicationRef === 'pub-2' ? [frozenLink] : [],
      }))});
    }
    if (url.pathname === '/api/local/ops/publications') {
      expect(Object.fromEntries(url.searchParams)).toEqual({ domain: 'd-1', publication_ref: 'pub-2' });
      return Response.json({asOf:null,page:1,pageSize:50,total:1,totalPages:1,operators:[],items:[{
        ref: currentRef, source:'native', businessDate:'2026-01-01', dateState:'as_recorded', platform:'xhs',
        title:'同名发布', body:'台账正文', url:'https://example.com/post', account:null, publisher:null,
        creation: { ...frozenLink, linkRef: currentLink, domainRef: 'd-1', registeredWithDraft: true },
        views:{state:'value',value:1500,overdue:false,hit:false}, likes:null, collects:null, comments:null, coverClickRate:null,
        systemCheck:'not_checked', sync:{requestRef:null,decision:null,state:'not_requested'},
        media:{state:'not_requested',observed:0,available:0,failed:0,unavailable:0},
        fieldAdoption:{title:'adopted',body:'adopted',likes:'not_provided',collects:'not_provided',comments:'not_provided',publishedAt:'adopted'},
        duplicates:[], cover:{available:false},
      }]});
    }
    expect(url.pathname).toBe(revisionPath);
    return Response.json({id:revisionId,number:1,savedAt:'2026-01-01T00:00:00Z',document:{
      ...newDocument('登记时标题'),
      paragraphs:[{id:'00000000-0000-4000-8000-000000000003',text:'登记时保存的正文'}],
    }});
  });
  fireEvent.click(await screen.findByRole('button', {name:'操作方法过千率分母 1'}));
  const list = within(await screen.findByRole('dialog', {name:'快照笔记列表'}));
  fireEvent.click((await list.findAllByRole('button', {name:'查看发布记录：同名发布'}))[1]!);
  const detail = () => within(screen.getByRole('dialog', {name:'发布记录详情'}));
  if (currentRef === 'pub-2' && currentLink === 'frozen-link') {
    await screen.findByText('当前台账详情 · 看板指标取自快照');
    fireEvent.click(detail().getByRole('button', {name:'查看登记时稿件'}));
    await detail().findByText(visible);
    expect(detail().getByRole('heading', {name:'登记时稿件'})).toBeVisible();
    expect(detail().queryByText('台账正文')).not.toBeInTheDocument();
    expect(requests.some(url => url.pathname === revisionPath)).toBe(true);
    fireEvent.click(detail().getByRole('button', {name:'返回发布记录详情'}));
    expect(detail().getByText('台账正文')).toBeVisible();
  } else {
    await screen.findByText(visible);
    expect(detail().queryByRole('button', {name:'查看登记时稿件'})).not.toBeInTheDocument();
    expect(requests.some(url => url.pathname.includes('/revisions/'))).toBe(false);
  }
  fireEvent.click(detail().getByRole('button', {name:'关闭详情'}));
  expect(list.getAllByRole('button', {name:'查看发布记录：同名发布'})).toHaveLength(2);
});

it('keeps the disabled route from requesting a snapshot', async () => {
  const requests = show(() => Response.json({ enabled:false,run:null }));
  expect(await screen.findByText('内容回溯未启用。')).toBeVisible();
  expect(requests.every(u => u.pathname.endsWith('status'))).toBe(true);
});
const running = {inProgress:true,progress:0.25,primary:{state:'running',succeededShards:1,totalShards:4},audit:null};

it('shows the server progress while no snapshot has been built', async () => {
  const requests = show(url => url.pathname.endsWith('/status')
    ? Response.json({enabled:true,run:running,latestSnapshotRef:null})
    : Response.json({snapshot:null,freshness:null}));
  expect(await screen.findByText('25%')).toBeVisible();
  await waitFor(() => expect(requests.some(u => u.pathname.endsWith('/snapshot'))).toBe(true));
  expect(screen.queryByText('发布效果')).not.toBeInTheDocument();
});

it('keeps the current board readable during a run and refreshes only when the scope receipt changes', async () => {
  const client = new QueryClient({defaultOptions:{queries:{retry:false}}});
  let ref = 'snapshot-1';
  let run: typeof running | null = running;
  const requests = show(url => {
    expect(url.searchParams.get('domainRef')).toBe('d-1');
    if (url.pathname.endsWith('/status')) return Response.json({enabled:true,run,latestSnapshotRef:ref});
    return Response.json({snapshot:{...body,snapshotRef:ref,quality:{...body.quality,titled:ref === 'snapshot-1' ? 2 : 3}},
      freshness:{staleDays:0,isStale:false}});
  }, false, client);
  expect(await screen.findByText('25%')).toBeVisible();
  expect(await screen.findByText('发布效果')).toBeVisible();
  const count = () => requests.filter(u => u.pathname.endsWith('/snapshot')).length;
  const statusKey = ['ops', 'content-lens', 'status', 'd-1'];
  expect(count()).toBe(1);
  run = null;
  await client.invalidateQueries({queryKey:statusKey});
  await waitFor(() => expect(screen.queryByText('25%')).not.toBeInTheDocument());
  expect(screen.getByText('发布效果')).toBeVisible();
  expect(count()).toBe(1);
  ref = 'snapshot-2';
  await client.invalidateQueries({queryKey:statusKey});
  await waitFor(() => expect(count()).toBe(2));
  await waitFor(() => expect(screen.getByRole('button',{name:'查看有标题的发布记录'})).toHaveTextContent('3'));
  await client.invalidateQueries({queryKey:statusKey});
  expect(count()).toBe(2);
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
it('confirms or corrects one dimension, retries the original command, and reads back the scoped snapshot', async () => {
  const titles = ['标题甲', '标题乙'];
  const current = { moment:'t1', situation:'s1', offer:'o1', audience:'a1' };
  const vocabulary = {
    moment:[{key:'t1',name:'放学'}], situation:[{key:'s1',name:'等待'}], offer:[{key:'o1',name:'建议'}],
    audience:[{key:'a1',name:'家长'},{key:'a2',name:'教师'}],
  };
  let version = 1;
  const writes: Array<Record<string,string>> = [];
  const requests = show((url, init) => {
    const ref = 'snapshot-' + version;
    if (url.pathname.endsWith('/status')) return Response.json({enabled:true,run:null});
    if (url.pathname.endsWith('/snapshot')) return Response.json({snapshot:{...body,snapshotRef:ref,quality:{...body.quality,...(version > 1 && {adjudicatedTitles:1,correctedTitles:1})}},freshness:{staleDays:0,isStale:false}});
    if (url.pathname.endsWith('/spot-check')) {
      expect(url.searchParams.get('snapshotRef')).toBe(ref);
      return Response.json({snapshotRef:ref,vocabulary,items:titles.map((title, i) => ({
        titleKey:'key-' + i,title,confidence:'low',...current,
        effective:{...current,audience:version > 1 && i === 1 ? 'a2' : 'a1'},
      }))});
    }
    expect(url.pathname).toMatch(/\/adjudications$/);
    expect(init?.method).toBe('POST');
    writes.push(JSON.parse(String(init?.body)));
    if (writes.length === 1) throw new Error('Connection lost');
    version += 1;
    return Response.json({snapshotRef:'snapshot-' + version, reused:writes.length === 2});
  }, true);
  fireEvent.click(await screen.findByRole('button',{name:'查看人工抽看样本'}));
  expect(screen.queryByText(/已改正.*个标题/)).not.toBeInTheDocument();
  const dialog = within(await screen.findByRole('dialog'));
  await dialog.findByText(titles[0]!);
  fireEvent.click(dialog.getByRole('button',{name:'下一条'}));
  expect(dialog.getByText(titles[1]!)).toBeVisible();
  expect(dialog.getByText('机器标注把握程度：低')).toBeVisible();
  fireEvent.change(dialog.getByLabelText('裁定维度'),{target:{value:'audience'}});
  fireEvent.change(dialog.getByLabelText('裁定分类'),{target:{value:'a2'}});
  fireEvent.change(dialog.getByLabelText('理由（选填）'),{target:{value:'面向教师'}});
  fireEvent.click(dialog.getByRole('button',{name:'保存改正'}));
  await dialog.findByRole('alert');
  fireEvent.click(dialog.getByRole('button',{name:'保存改正'}));
  await dialog.findByText('裁定已记录。');
  const audience = within(await dialog.findByRole('row',{name:/面向谁 家长 教师/}));
  expect(audience.getByText('家长')).toBeVisible();
  expect(writes[0]).toEqual({snapshotRef:'snapshot-1',titleKey:'key-1',dimension:'audience',value:'a2',reason:'面向教师',commandRef:expect.any(String)});
  expect(writes[1]).toEqual(writes[0]);
  fireEvent.click(dialog.getByRole('button',{name:'确认当前分类'}));
  await waitFor(() => expect(writes).toHaveLength(3));
  expect(writes[2]).toEqual({snapshotRef:'snapshot-2',titleKey:'key-1',dimension:'audience',value:'a2',commandRef:expect.any(String)});
  expect(writes[2]!.commandRef).not.toBe(writes[1]!.commandRef);
  await dialog.findByText('裁定已记录。');
  expect(await screen.findByText('已改正 1 个标题')).toBeVisible();
  fireEvent.click(dialog.getByRole('button',{name:'上一条'}));
  expect(dialog.getByText(titles[0]!)).toBeVisible();
  fireEvent.click(dialog.getByRole('button',{name:'下一条'}));
  expect(dialog.getByRole('row',{name:/面向谁 家长 教师 已记录/})).toBeVisible();
  fireEvent.change(dialog.getByLabelText('裁定维度'), {target:{value:'offer'}});
  fireEvent.click(dialog.getByRole('button',{name:'确认当前分类'}));
  await dialog.findByText('裁定已记录。');
  expect(writes[3]).toMatchObject({snapshotRef:'snapshot-1',titleKey:'key-1',dimension:'offer',value:'o1'});
  expect(requests.filter(url => url.pathname.endsWith('/spot-check')).map(url => url.searchParams.get('snapshotRef'))).toEqual(['snapshot-1']);
  expect(requests.filter(url => url.pathname.endsWith('/snapshot')).map(url => url.searchParams.get('domainRef'))).toEqual(['d-1','d-1','d-1','d-1']);
});

it('keeps a conflicting session frozen until the user closes it and regenerates the scope', async () => {
  let current = 'snapshot-1';
  const writes: string[] = [];
  show((url, init) => {
    if (url.pathname.endsWith('/status')) return Response.json({enabled:true,run:null});
    if (url.pathname.endsWith('/snapshot')) return Response.json({snapshot:{...body,snapshotRef:current},freshness:{staleDays:0,isStale:false}});
    if (url.pathname.endsWith('/spot-check')) {
      const vocabulary = Object.fromEntries(['moment','situation','offer','audience'].map(dim => [dim,[{key:'a',name:'类别甲'}]]));
      const values = {moment:'a',situation:'a',offer:'a',audience:'a'};
      return Response.json({snapshotRef:current,vocabulary,items:[{
        titleKey:current, title:current === 'snapshot-1' ? '旧样本' : '新样本', confidence:'medium',...values,effective:values,
      }]});
    }
    if (url.pathname.endsWith('/snapshots')) {
      expect(JSON.parse(String(init?.body))).toEqual({domainRef:'d-1'});
      current = 'snapshot-2';
      return Response.json({snapshotRef:current});
    }
    writes.push(JSON.parse(String(init?.body)).snapshotRef);
    return Response.json({error:'ops_lens_adjudication_conflict'},{status:409});
  }, true);
  fireEvent.click(await screen.findByRole('button',{name:'查看人工抽看样本'}));
  const dialog = within(await screen.findByRole('dialog'));
  fireEvent.click(await dialog.findByRole('button',{name:'确认当前分类'}));
  expect(await dialog.findByRole('alert')).toHaveTextContent('裁定发生冲突');
  expect(dialog.getByRole('button',{name:'确认当前分类'})).toBeDisabled();
  expect(dialog.getByRole('alert')).toHaveTextContent('生成当前范围快照');
  fireEvent.click(dialog.getByRole('button',{name:'关闭'}));
  fireEvent.click(screen.getByRole('button',{name:'生成当前范围快照'}));
  await screen.findByText('快照已生成');
  fireEvent.click(screen.getByRole('button',{name:'查看人工抽看样本'}));
  const reopened = within(await screen.findByRole('dialog'));
  expect(await reopened.findByText('新样本')).toBeVisible();
  expect(reopened.getByRole('button',{name:'确认当前分类'})).toBeEnabled();
  expect(writes).toEqual(['snapshot-1']);
});
