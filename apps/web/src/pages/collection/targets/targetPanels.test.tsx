import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MemoryRouter } from 'react-router';
import { AssignDomain } from './TargetAssignment';
import { RuleErrors, RuleReceipt } from './RuleFeedback';
import { Chart } from './LifecycleChart';
import { LifecyclePanel } from './TargetLifecycle';
import { Review } from './LifecycleReview';
import { bucketPoints } from './chartMath';
import type { Lifecycle } from './api';
import { Toolbar } from './TargetToolbar';
import { Modal } from './TargetModals';
import { RuleModal } from './TargetModals';
import { Overview } from './TargetOverview';
import { decodeDrawer } from './api';
import type { TargetList } from './api';
import type { Commands } from './useWorkspace';
const targetRef = '00000000-0000-4000-8000-000000000001';
const domainRef = '00000000-0000-4000-8000-000000000002';
const list: TargetList = { rows: [], currentDomain: null, limit: 200, counts: null,
  domains: [{ domainRef, name: '合成领域', status: 'active' }] };
const commands = (): Commands => ({ run: vi.fn(async () => false), verify: vi.fn(async () => undefined), busy: false, feedback: null });
const answer = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
afterEach(() => { cleanup(); vi.restoreAllMocks(); });
describe('领域分配的独立目标与资格', () => {
  it('列表筛选外目标仍读取真实名称，并以 primary 说明和绘制 select 提交明确领域', async () => {
    const fetch = vi.spyOn(globalThis, 'fetch').mockResolvedValue(answer({ targetRef, name: '过滤外合成目标', assignable: true }));
    const actions = commands();
    render(<MemoryRouter><AssignDomain targetRef={targetRef} list={list} params={new URLSearchParams('filter=keyword')} commands={actions} /></MemoryRouter>);
    expect(await screen.findByRole('heading', { name: '为「过滤外合成目标」分配领域' })).toBeInTheDocument();
    expect(screen.getByText(/分配后先按 primary 角色关联/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '领域' })).toHaveClass('c-tg-drawn-toggle');
    fireEvent.click(screen.getByRole('button', { name: '确认分配' }));
    expect(actions.run).toHaveBeenCalledWith('domain', expect.objectContaining({ row_target_ref: targetRef, domain_ref: domainRef, return_filter: 'keyword' }));
    expect(fetch.mock.calls[0]?.[0]).toBe(`/api/collection/workspace/target/${targetRef}/assignment`);
  });
  it.each([true, false])('无可用领域或已有归属不提供写入口：assignable=%s', async (assignable) => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(answer({ targetRef, name: '合成目标', assignable }));
    const actions = commands();
    render(<MemoryRouter><AssignDomain targetRef={targetRef} list={{ ...list, domains: [] }} params={new URLSearchParams()} commands={actions} /></MemoryRouter>);
    await screen.findByRole('heading', { name: '为「合成目标」分配领域' });
    expect(screen.getByText(assignable ? '当前读不到任何可用领域，没有修改目标。领域读取恢复后再分配。' : '这个目标已经有领域，没有覆盖现有领域。')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '确认分配' })).not.toBeInTheDocument();
    expect(actions.run).not.toHaveBeenCalled();
  });
});
describe('监控规则原回执与失败语义', () => {
  it('展示中文结果、已应用版本与原回执编号', () => {
    render(<RuleReceipt receipt={{ receiptRef: targetRef, outcome: 'applied', reason: 'rule_saved', expectedRevision: 0,
      recordedAt: '2026-01-01 09:00', appliedRuleRevisionRef: domainRef }} />);
    expect(screen.getByText('命令已应用')).toBeInTheDocument();
    expect(screen.getByText('规则已保存。没有据此声称采集已开始或完成。')).toBeInTheDocument();
    expect(screen.getByText('规则 00000000')).toBeInTheDocument();
    expect(screen.getByText(targetRef)).toBeInTheDocument();
    expect(screen.getByText('命令已应用').closest('section')).toHaveClass('c-rule-feedback-ok');
  });
  it('过期不误报成功并明确保留草稿和当前版本', () => {
    render(<><RuleReceipt receipt={{ receiptRef: targetRef, outcome: 'stale_revision', reason: 'stale_revision', expectedRevision: 1,
      recordedAt: '2026-01-01 09:00', appliedRuleRevisionRef: null }} /><RuleErrors code="stale_revision" /></>);
    expect(screen.getByText('规则版本已经变化').closest('section')).toHaveClass('c-rule-feedback-warn');
    expect(screen.getByText('未产生规则版本')).toBeInTheDocument();
    expect(screen.getByRole('alert')).toHaveTextContent('下面保留了你提交的值，没有覆盖当前版本');
  });
  it('规则读取失败沿用紧凑失败面板，未知不提供保存', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(answer({ code: 'target_not_found' }, 404));
    render(<MemoryRouter><RuleModal targetRef={targetRef} params={new URLSearchParams()} commands={commands()} revision={0} /></MemoryRouter>);
    expect(await screen.findByRole('heading', { name: '没有找到这个观察目标' })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole('heading', { name: '没有找到这个观察目标' })).toHaveFocus());
    expect(screen.getByText('这个本机标识没有对应目标；它不证明平台上的对象不存在，也不证明目标曾被删除。')).toBeInTheDocument();
    expect(screen.getByRole('dialog')).toHaveClass('c-rule-modal-compact');
    expect(screen.queryByRole('button', { name: '保存新版本' })).not.toBeInTheDocument();
  });
});
describe('概览的原目标种类边界', () => {
  it.each(['creator', 'keyword'])('材料失效判断只属于 creator 概览：%s', kind => {
    const known = { state: 'known', value: 1 };
    const data = decodeDrawer({ target: {
      targetRef, kind, platform: 'xhs', identity: 'synthetic-target', handle: 'synthetic-target', name: '合成目标',
      group: '未分组', domainName: '合成领域', lifecycle: 'paused', monitoring: false,
      avatar: { state: 'not_observed', path: null }, archiveLabel: '尚未建立', archiveTone: 'neutral',
      patrolLabel: '未开启', patrolTone: 'neutral', works: '0', details: '0/0', recentNew: null, recentHits: null,
      last: null, next: null, action: { kind: 'archive', label: '建立档案', tab: 'overview', fragment: '' },
    }, inspector: { archive: { state: 'not_started' }, execution: { state: 'idle' },
      patrol: { state: 'disabled', lastSucceededAt: null, nextRunAt: null, latestHits: known, latestNew: known },
      coverage: { directoryWorks: known, capturedDetails: known, missingDetails: known, quarantinedRecords: known, blockedDetails: known } },
      contentGapAction: null, decision: { title: '需要建立档案', note: '合成说明', action: null },
      blocked: [{ publicRef: domainRef, externalId: 'synthetic-content', title: '合成作品' }], catalog: [], rules: [], lifecycle: { state: 'not_read' },
    });
    const { container } = render(<MemoryRouter><Overview data={data} params={new URLSearchParams()} commands={commands()} /></MemoryRouter>);
    expect(Boolean(screen.queryByRole('button', { name: '确认这些作品已失效' }))).toBe(kind === 'creator');
    expect(container.querySelector(kind === 'creator' ? '#archive-problems' : '#required-action')).not.toBeNull();
  });
});

describe('规则拒绝后的表单生命周期', () => {
  it('独立核对原回执不卸载草稿，也不以新服务器值覆盖已填字段', async () => {
    const panel = {
      targetRef, name: '合成规则目标', kind: 'creator', lifecycle: 'paused', slot: 'primary', rankings: [],
      form: { expected_revision: 1, idempotency_key: domainRef, automatic_enabled: false, creator_follow_details: false,
        fixed_interval_seconds: '86400', surface_key: 'creator_profile', ranking_key: '', scroll_rounds: '', top_by_likes: '',
        published_within_days: '', task_contract_version: 'v2' }, receipt: null,
    };
    vi.spyOn(globalThis, 'fetch').mockImplementation(async input => String(input).includes('rule_receipt=')
      ? answer({ ...panel, form: { ...panel.form, fixed_interval_seconds: '604800', expected_revision: 2 }, receipt: {
        receiptRef: domainRef, outcome: 'stale_revision', reason: 'stale_revision', expectedRevision: 1,
        recordedAt: '2026-01-01 09:00', appliedRuleRevisionRef: null,
      } }) : answer(panel));
    const actions = commands(), params = new URLSearchParams(`rule=${targetRef}`);
    const { rerender } = render(<MemoryRouter><RuleModal targetRef={targetRef} params={params} commands={actions} revision={0} /></MemoryRouter>);
    const input = await screen.findByLabelText('观察间隔（仅可选择下列 5 档）');
    fireEvent.change(input, { target: { value: '21600' } });
    const rejected = { ...actions, feedback: { requestRef: domainRef, action: 'rules', code: 'monitor_rule_command_stale',
      text: '拒绝', applied: false, unknown: false,
      location: `/collection/targets?rule=${targetRef}&rule_receipt=${domainRef}&error=monitor_rule_command_stale` } };
    rerender(<MemoryRouter><RuleModal targetRef={targetRef} params={params} commands={rejected} revision={0} /></MemoryRouter>);
    await screen.findByText('规则版本已经变化');
    expect(screen.getByLabelText('观察间隔（仅可选择下列 5 档）')).toHaveValue('21600');
    expect(screen.getByRole('alert')).toHaveTextContent('下面保留了你提交的值');
    expect(actions.run).not.toHaveBeenCalled();
  });
});
describe('既有 SVG 绘图精度', () => {
  it('分布点以原固定一位小数坐标绘制，并保留高讨论率的可访问说明', () => {
    const point = { publicRef: targetRef, title: '合成作品', published: '2026-01-01', date: '2026-01-01', epochMs: 0,
      value: 1, discussionRate: 0.25, association: 'AUTHOR_CONFIRMED', new: false };
    const lifecycle: Lifecycle = { status: 'AVAILABLE', asOf: '', window: 'all', metric: 'likes', linked: 3, lowerBound: 3,
      confirmed: 3, eligible: 3, exclusions: { authorNotVerified: 0, authorMismatch: 0, publishedAtNotQualified: 0,
        outsideWindow: 0, metricUnknown: 0, scanTruncated: false }, receipt: { scanLimit: 2000, probedCount: 3,
        scannedCount: 3, returnedCount: 3, truncated: false }, points: [point, { ...point, publicRef: domainRef, epochMs: 1 },
        { ...point, publicRef: '00000000-0000-4000-8000-000000000003', epochMs: 3, value: 3 }] };
    const { container } = render(<MemoryRouter><Chart p={lifecycle} params={new URLSearchParams()} distribution week={false} buckets={[]} /></MemoryRouter>);
    const circles = container.querySelectorAll('.life-point-visible');
    expect(circles[1]).toHaveAttribute('cx', '358.7');
    expect(circles[1]).toHaveAttribute('cy', '286.7');
    expect(container.querySelector('.life-point')).toHaveAttribute('aria-label', expect.stringContaining('评论／点赞 25%（高讨论率复核信号）'));
  });
});

describe('原目标弹窗的几何与初始选择', () => {
  it.each([['all', domainRef], [targetRef, targetRef], ['paused-domain', domainRef]])(
    '新建领域选择仅默认服务器 active 集合，当前参数 %s 优先合法项', async (current, expected) => {
      const read = { ...list, domains: [...list.domains, { domainRef: targetRef, name: '另一合成领域', status: 'active' }, { domainRef: 'paused-domain', name: '已暂停合成领域', status: 'paused' }] };
      const { rerender } = render(<MemoryRouter><Toolbar list={null} params={new URLSearchParams({ domain: current })} selected={[]} commands={commands()} openBatch={vi.fn()} /></MemoryRouter>);
      rerender(<MemoryRouter><Toolbar list={read} params={new URLSearchParams({ domain: current })} selected={[]} commands={commands()} openBatch={vi.fn()} /></MemoryRouter>);
      expect(screen.queryByRole('textbox', { name: '创作者主页链接、ID 或关键词' })).not.toBeInTheDocument();
      fireEvent.click(screen.getByRole('button', { name: '＋ 新建目标' }));
      const dialog = screen.getByRole('dialog', { name: '新建观察目标' });
      expect(screen.getByRole('textbox', { name: '创作者主页链接、ID 或关键词' })).toHaveFocus();
      fireEvent.change(screen.getByRole('textbox', { name: '创作者主页链接、ID 或关键词' }), { target: { value: 'ad0000000000000000000992' } });
      expect(dialog.id).toBe('target-domain-modal');
      expect(screen.getByRole('combobox', { name: '领域' })).toHaveValue(expected);

      expect(screen.queryByRole('option', { name: '已暂停合成领域' })).not.toBeInTheDocument();
    },
  );
  it('收起新建目标必须明确保留输入，重新打开保留草稿且没有发送命令', () => {
    const actions=commands();
    render(<MemoryRouter><Toolbar list={list} params={new URLSearchParams()} selected={[]} commands={actions} openBatch={vi.fn()}/></MemoryRouter>);
    fireEvent.click(screen.getByRole('button',{name:'＋ 新建目标'}));
    fireEvent.change(screen.getByRole('textbox',{name:'创作者主页链接、ID 或关键词'}),{target:{value:'合成未提交目标'}});
    fireEvent.click(screen.getByRole('button',{name:'取消'}));
    expect(screen.getByRole('region',{name:'保留目标输入'})).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button',{name:'收起并保留输入'}));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button',{name:'＋ 新建目标'}));
    expect(screen.getByRole('textbox',{name:'创作者主页链接、ID 或关键词'})).toHaveValue('合成未提交目标');
    expect(actions.run).not.toHaveBeenCalled();
  });
  it('批量打开的初始焦点在原 group_name 输入', () => {
    render(<Modal id="batch-title" initialFocus='input[name="group_name"]' className="c-tg-batch-dialog"><h2 id="batch-title">批量</h2><input name="group_name" aria-label="分组" /></Modal>);
    expect(screen.getByRole('textbox', { name: '分组' })).toHaveFocus();
  });
  it('删除预览不增加旧页没有的标题焦点框，首次 Tab 进入弹窗', () => {
    render(<Modal id="delete-title" initialFocus={null} className="c-tg-delete"><h2 id="delete-title">删除预览</h2><button type="button">取消</button></Modal>);
    expect(screen.getByRole('heading', { name: '删除预览' })).not.toHaveFocus();
    const cancel = screen.getByRole('button', { name: '取消' });
    Object.defineProperty(cancel, 'offsetParent', { configurable: true, value: document.body });
    fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Tab' });
    expect(cancel).toHaveFocus();
  });
});

const coverageFixture:Lifecycle={status:'AVAILABLE',asOf:'2026-01-03T00:00:00Z',window:'all',metric:'likes',linked:220,lowerBound:220,confirmed:220,eligible:220,
  exclusions:{authorNotVerified:0,authorMismatch:0,publishedAtNotQualified:0,outsideWindow:0,metricUnknown:0,scanTruncated:false},
  receipt:{scanLimit:2000,probedCount:220,scannedCount:220,returnedCount:220,truncated:false},points:[]};
describe('检查器紧凑披露保留数据含义',()=>{
  it('已知相等的220只披露一次，作者与分析资格分别保留',()=>{
    render(<MemoryRouter><LifecyclePanel lifecycle={{state:'ready',projection:coverageFixture}} params={new URLSearchParams()}/></MemoryRouter>);
    const coverage=screen.getByLabelText('作品覆盖摘要');expect(coverage.textContent?.match(/220/g)).toHaveLength(1);
    expect(coverage).toHaveTextContent('作者均已确认');expect(coverage).toHaveTextContent('当前均可分析');
  });
  it.each([{linked:220,lowerBound:220,confirmed:200,eligible:180},{linked:null,lowerBound:220,confirmed:220,eligible:220},{linked:0,lowerBound:0,confirmed:0,eligible:0}])('差异、目录下限和已知零不被合并成全部：%j',counts=>{
    render(<MemoryRouter><LifecyclePanel lifecycle={{state:'ready',projection:{...coverageFixture,...counts}}} params={new URLSearchParams()}/></MemoryRouter>);
    const coverage=screen.getByLabelText('作品覆盖摘要');expect(coverage).toHaveTextContent(`作者已确认 ${counts.confirmed} 篇`);
    expect(coverage).toHaveTextContent(`当前可分析 ${counts.eligible} 篇`);expect(coverage).not.toHaveTextContent('均');
    if(counts.linked===null)expect(coverage).toHaveTextContent('作品目录下限 ≥220');
  });
  it('复核读数仍直接可见，解释可按需展开，密度不重复作品总数',()=>{
    const point={publicRef:targetRef,title:'合成作品',published:'2026-01-01',date:'2026-01-01',epochMs:Date.parse('2026-01-01T00:00:00Z'),value:0,discussionRate:null,association:'AUTHOR_CONFIRMED',new:true};
    const p={...coverageFixture,eligible:1,points:[point]};const {container}=render(<Review p={p} week={false} distribution={false} buckets={bucketPoints(p.points,false)}/>);
    expect(screen.getByText('1.0 / 活跃月')).toBeVisible();expect(screen.getByText('1 篇')).toBeVisible();
    const details=container.querySelector('.life-review-item details');expect(details).not.toHaveAttribute('open');
    expect(screen.getByText(/只按当前可分析作品计算/)).not.toBeVisible();
    fireEvent.click(container.querySelector('.life-review-item summary')!);
    expect(screen.getByText(/只按当前可分析作品计算/)).toBeVisible();
  });
});

it('紧凑图表只疏显示日期刻度，保留全部20个作品时间桶且末端标签不相邻堆叠',()=>{
  const points=Array.from({length:20},(_,index)=>{const date=new Date(Date.UTC(2025,2+index,1)).toISOString().slice(0,10);return {publicRef:`00000000-0000-4000-8000-${String(index+10).padStart(12,'0')}`,title:'合成作品',published:date,date,epochMs:Date.parse(date),value:index,discussionRate:null,association:'AUTHOR_CONFIRMED',new:false};});
  const p={...coverageFixture,linked:20,lowerBound:20,confirmed:20,eligible:20,points};
  const {container}=render(<MemoryRouter><Chart p={p} params={new URLSearchParams()} distribution={false} week={false} buckets={bucketPoints(points,false)}/></MemoryRouter>);
  const dates=Array.from(container.querySelectorAll('text[y="446"]'));
  expect(dates.length).toBeLessThanOrEqual(7);expect(dates[0]).toHaveTextContent('2025/03');expect(dates.at(-1)).toHaveTextContent('2026/10');
  expect(Number(dates.at(-1)?.getAttribute('x'))-Number(dates.at(-2)?.getAttribute('x'))).toBeGreaterThanOrEqual(100);
  expect(container.querySelectorAll('.life-trend-bucket-hit')).toHaveLength(20);
});
