import { screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CommentDeepCollect } from './CommentDeepCollect';
import { forgetCommentDeepRequests } from './commentDeepRequests';
import { parseWorkDetail } from './evidenceDecode';
import { DOMAIN, detailJson, workRef, type DetailOptions } from './evidenceFixtures';
import { ObservationHistory } from './ObservationHistory';
import { ObservationRecordSection } from './ObservationRecordSection';
import type { Reobservation as Channel } from './evidenceTypes';
import type { Reobservation } from './useReobservation';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';

const RUNS = '/api/collection/runs';
const RUN = '30000000-0000-4000-8000-000000000001';
const NOW = Date.parse('2026-10-23T10:00:00Z');

function detail(options: DetailOptions = {}) {
  return parseWorkDetail(detailJson(1, options));
}

function timelinePoint(observedAt: string, lane: string, like: number | null, comment: number | null = null) {
  return {
    observedAt, sourceLane: lane,
    likeCount: like, likeCountState: like === null ? 'UNKNOWN' : 'KNOWN',
    commentCount: comment, commentCountState: comment === null ? 'UNKNOWN' : 'KNOWN',
    collectCount: null, collectCountState: 'UNKNOWN', shareCount: null, shareCountState: 'UNKNOWN',
  };
}

function runJson(overrides: Record<string, unknown> = {}) {
  return {
    run_ref: RUN, domain_ref: DOMAIN, name: '已选 1 篇作品', domain_name: 'ADHD', source_kind: 'selected_works', legacy: false, ended: false,
    control_intent: 'running', phase: 'materials', comment_mode: 'all_public', created_at: '2026-10-23T10:00:00Z', updated_at: '2026-10-23T10:00:00Z',
    revision: 1, work_limit: 1, live_leases: 0, received_comments: 0, new_comments: 0, ...overrides,
  };
}

let kit: FetchKit;
beforeEach(() => {
  kit = createFetchMock();
  kit.install();
});
afterEach(() => {
  forgetCommentDeepRequests();
  vi.unstubAllGlobals();
});

describe('观察记录（Mog 2026-10-23、2026-11-02）', () => {
  it('最近一次用轻元数据；表格从新到旧，变化与上一次比，没读到写「—」', () => {
    const { item, inspector } = detail({
      timeline: [timelinePoint('2026-10-12T08:36:46Z', 'discovery', 293), timelinePoint('2026-10-14T21:59:48Z', 'detail', 305, 37)],
    });
    renderWithProviders(<ObservationHistory item={item} inspector={inspector} now={NOW} />);
    expect(screen.getAllByText('2026-10-15 05:59').length).toBeGreaterThan(0);
    expect(screen.getByText('8 天前')).toBeInTheDocument();
    const table = screen.getByRole('table', { name: '每次观察读到的数字，最近一次在最上面' });
    const [newest, oldest] = within(table).getAllByRole('row').slice(1);
    expect(within(newest!).getByRole('rowheader')).toHaveTextContent('2026-10-15 05:59打开详情页');
    expect(newest).toHaveTextContent('305+12');
    expect(newest).toHaveTextContent('37');
    expect(within(oldest!).getByRole('rowheader')).toHaveTextContent('2026-10-12 16:36列表中看到');
    expect(within(oldest!).getAllByText('—')).toHaveLength(3);
    expect(screen.queryByText(/[0-9a-f]{8}-[0-9a-f]{4}-/)).toBeNull();
  });

  it('没有互动数字时用记录级空态，并给出最近一次观察时间', () => {
    const { item, inspector } = detail({ timeline: [], observedAt: '2026-10-20 09:00' });
    renderWithProviders(<ObservationHistory item={item} inspector={inspector} now={NOW} />);
    expect(screen.getByText('暂无互动观察记录')).toBeInTheDocument();
    expect(screen.getByText('2026-10-20 09:00')).toBeInTheDocument();
    expect(screen.getByText('3 天前')).toBeInTheDocument();
    expect(screen.queryByRole('table')).toBeNull();
  });

  it('标题次数取原 timeline 窗口数，不是合并后的行数；满窗口写 100+ 并说明只列最近 100 次', () => {
    const channel: Channel = { supported: true, eligible: true, reason: 'OK', url: `/api/local/work-resources/${workRef(1)}/reobserve?domain=${DOMAIN}`, mediaPolicy: 'EXISTING_ASSETS_REUSED' };
    const state: Reobservation = { sending: false, operation: null, feedback: null, readError: null, pollError: null, queued: false, submit: () => undefined };
    const merged = detail({
      timeline: [timelinePoint('2026-10-12T08:36:46Z', 'detail', 293), timelinePoint('2026-10-14T21:59:48Z', 'detail', 293)],
    });
    const first = renderWithProviders(<ObservationRecordSection item={merged.item} inspector={merged.inspector} channel={channel} domain={DOMAIN} state={state} onRecheck={() => undefined} now={NOW} />);
    expect(screen.getByText('2')).toBeInTheDocument();
    expect(screen.getByText('2 次观察记录')).toBeInTheDocument();
    expect(screen.getByRole('table')).toBeInTheDocument();
    expect(within(screen.getByRole('table')).getAllByRole('row')).toHaveLength(2);
    first.unmount();
    const full = detail({
      timeline: Array.from({ length: 100 }, (_, index) => timelinePoint(`2026-10-${String((index % 20) + 1).padStart(2, '0')}T08:36:46Z`, 'detail', index)),
    });
    renderWithProviders(<ObservationRecordSection item={full.item} inspector={full.inspector} channel={channel} domain={DOMAIN} state={state} onRecheck={() => undefined} now={NOW} />);
    expect(screen.getByText('100+')).toBeInTheDocument();
    expect(screen.getByText('至少 100 次，仅列最近 100 次')).toBeInTheDocument();
    expect(screen.getByText('至少 100 次，仅列最近 100 次。')).toBeInTheDocument();
  });

  it('零记录仍显示最近观察时间；复观测不可用时只留紧凑原因，没有假按钮', () => {
    const channel: Channel = { supported: true, eligible: false, reason: 'REOBSERVATION_ELIGIBILITY_UNKNOWN', url: null, mediaPolicy: 'EXISTING_ASSETS_REUSED' };
    const state: Reobservation = { sending: false, operation: null, feedback: null, readError: null, pollError: null, queued: false, submit: () => undefined };
    const { item, inspector } = detail({ timeline: [], observedAt: '2026-10-20 09:00' });
    renderWithProviders(<ObservationRecordSection item={item} inspector={inspector} channel={channel} domain={DOMAIN} state={state} onRecheck={() => undefined} now={NOW} />);
    expect(screen.getByText('0')).toBeInTheDocument();
    expect(screen.getByText('暂无互动观察记录')).toBeInTheDocument();
    expect(screen.getByText('2026-10-20 09:00')).toBeInTheDocument();
    expect(screen.getByText(/资格待确认。/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '复观测' })).toBeNull();
  });
});

describe('评论采集（Mog 2026-10-23、2026-11-02）', () => {
  const history = [
    { observedAt: '2026-10-14T21:59:48Z', state: 'PARTIAL', collectionScope: 'detail_window', requestedLimit: 30, pageCommentCount: 37, expectedCount: 30, uniqueCollectedCount: 21, collectionState: 'partial', producerAcquired: 18, retained: 18, failed: 0 },
  ];

  function setup(options: DetailOptions = {}, cumulative: number | null = 48) {
    const { item, inspector } = detail({ comments: 37, commentsHistory: history, commentsCoverage: { count: 18, countState: 'KNOWN', retained: 18 }, ...options });
    inspector.commentsReceipt = { total: cumulative, returned: 0, truncated: true, nextCursor: null };
    return renderWithProviders(<CommentDeepCollect item={item} inspector={inspector} domain={DOMAIN} />);
  }

  it('先说平台与累计数、历史记录；点「采集评论」先确认，取消不发请求', async () => {
    const view = setup();
    expect(screen.getByText('平台显示 37')).toBeInTheDocument();
    expect(screen.getByText('累计已采 48')).toBeInTheDocument();
    const row = within(screen.getByRole('list', { name: '评论采集记录' })).getAllByRole('listitem')[0]!;
    expect(row).toHaveTextContent('2026-10-15 05:59');
    expect(row).toHaveTextContent('随详情（最多 30 条）');
    expect(row).toHaveTextContent('采到 21 条');
    expect(row).toHaveTextContent('页面显示 37');
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    const dialog = screen.getByRole('dialog', { name: '确认评论采集' });
    expect(within(dialog).getByText(/让合格的工位真实访问平台/)).toBeInTheDocument();
    expect(within(dialog).getAllByRole('button').map(button => button.textContent)).toEqual(['取消', '确认提交']);
    await view.user.click(within(dialog).getByRole('button', { name: '取消' }));
    expect(kit.calls).toHaveLength(0);
  });

  it('范围、条数与动作收在标题行；正常范围只作可访问说明，错误才就地出现', async () => {
    const view = setup();
    const heading = screen.getByRole('heading', { name: '评论采集' });
    const header = heading.parentElement!;
    expect(header).toContainElement(screen.getByRole('group', { name: '评论采集范围' }));
    expect(header).toContainElement(screen.getByRole('button', { name: '采集评论' }));
    expect(header).not.toContainElement(screen.getByText('平台显示 37'));

    await view.user.click(screen.getByRole('radio', { name: /指定条数/ }));
    const input = screen.getByRole('spinbutton', { name: '条数' });
    const description = document.getElementById(input.getAttribute('aria-describedby')!);
    expect(description).toHaveTextContent('1–1000 的整数');
    expect(description).toHaveClass('v7-sr-only');

    await view.user.clear(input);
    await view.user.type(input, '0');
    const error = screen.getByText('请填 1–1000 之间的整数');
    expect(error).toBeInTheDocument();
    expect(input).toHaveAttribute('aria-describedby', error.id);
    expect(input).toHaveAttribute('aria-errormessage', error.id);
  });

  it('确认后按采集页同一合同建一次「只含这一篇、尽量采全」的单次采集；进度读同一次采集', async () => {
    kit.route('POST', RUNS, ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson()));
    const view = setup();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText('已提交')).toBeInTheDocument();
    const [post] = kit.of('POST', RUNS);
    expect(post?.body).toMatchObject({ domainRef: DOMAIN, source: { kind: 'selected_works', workRefs: [workRef(1)] }, ranking: null, workLimit: 1, commentMode: 'all_public' });
    expect(typeof (post?.body as { idempotencyKey?: unknown }).idempotencyKey).toBe('string');
    expect(await screen.findByText('等待派发 · 本轮采回 0 条 · 首次新增 0 条')).toBeInTheDocument();
    expect(kit.of('GET', `${RUNS}/${RUN}`)[0]?.search.get('domainRef')).toBe(DOMAIN);
    expect(screen.getByRole('link', { name: '在采集任务里查看' })).toHaveAttribute('href', `/collection/tasks?domain=${encodeURIComponent(DOMAIN)}&run=${RUN}`);
    expect(screen.getByRole('button', { name: '评论采集已提交' })).toBeDisabled();
  });

  it.each([37, null])('累计总数与本轮采回%s条、首次新增0条分别展示', async received => {
    kit.route('POST', RUNS, ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson({ ended: true, received_comments: received, new_comments: 0 })));
    const view = setup({ commentsCoverage: { count: 8, countState: 'KNOWN', retained: 8 } }, 37);
    expect(screen.getByText('平台显示 37')).toBeInTheDocument();
    expect(screen.getByText('累计已采 37')).toBeInTheDocument();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText(`本轮已结束 · 本轮采回 ${received === null ? '未知' : received} 条 · 首次新增 0 条`)).toBeInTheDocument();
    expect(kit.of('POST', RUNS)).toHaveLength(1);
    expect(kit.of('GET', `${RUNS}/${RUN}`)).toHaveLength(1);
  });

  it.each([0, null])('累计数为%s时保留零/未知，不拿平台数、最近记录或历史合计补值', total => {
    setup({}, total);
    expect(screen.getByText('平台显示 37')).toBeInTheDocument();
    expect(screen.getByText(`累计已采 ${total === null ? '未知' : total}`)).toBeInTheDocument();
  });

  it('顶部读跨历史去重累计37，最近8和更早29/30保留在下面分段记录，不相加', () => {
    setup({
      commentsCoverage: { count: 8, countState: 'KNOWN', retained: 8 },
      commentsHistory: [8, 29, 30].map((collected, index) => ({ ...history[0], observedAt: `2026-10-${14 - index}T10:01:00Z`, uniqueCollectedCount: collected })),
    }, 37);
    expect(screen.getByText('平台显示 37')).toBeInTheDocument();
    expect(screen.getByText('累计已采 37')).toBeInTheDocument();
    const rows = within(screen.getByRole('list', { name: '评论采集记录' })).getAllByRole('listitem');
    expect(rows).toHaveLength(3);
    for (const [index, collected] of [8, 29, 30].entries()) expect(rows[index]).toHaveTextContent(`采到 ${collected} 条`);
    expect(kit.calls).toHaveLength(0);
  });

  it('没有累计回执时写未知，即使最近记录和平台数都已知', () => {
    const { item, inspector } = detail({ comments: 37 });
    renderWithProviders(<CommentDeepCollect item={item} inspector={{ ...inspector, commentsReceipt: null }} domain={DOMAIN} />);
    expect(screen.getByText('平台显示 37')).toBeInTheDocument();
    expect(screen.getByText('累计已采 未知')).toBeInTheDocument();
  });

  it('累计48但历史为空：只写暂无采集记录，不推断没采过', () => {
    setup({ commentsHistory: [] }, 48);
    expect(screen.getByText('累计已采 48')).toBeInTheDocument();
    expect(screen.getByText('暂无采集记录')).toBeInTheDocument();
    expect(screen.queryByText(/还没有采过/)).toBeNull();
  });

  it('超过5次历史先列最近5次，更早条目在本地展开收起', async () => {
    const view = setup({
      commentsHistory: Array.from({ length: 7 }, (_, index) => ({
        ...history[0],
        observedAt: `2026-10-${String(14 - index).padStart(2, '0')}T10:01:00Z`,
        uniqueCollectedCount: index + 1,
      })),
    });
    expect(within(screen.getByRole('list', { name: '评论采集记录' })).getAllByRole('listitem')).toHaveLength(5);
    const older = screen.getByText('更早 2 次').closest('details')!;
    expect(older).not.toHaveAttribute('open');
    await view.user.click(screen.getByText('更早 2 次'));
    expect(older).toHaveAttribute('open');
    expect(within(screen.getByRole('list', { name: '更早的评论采集记录' })).getAllByRole('listitem')).toHaveLength(2);
  });

  it('服务器明确拒绝：说「没有建立」和原因，按钮恢复，下一次是新请求', async () => {
    kit.route('POST', RUNS, fail(422, 'domain_not_active'), ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson()));
    const view = setup();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText(/没有建立：本次领域尚未运行，请先恢复领域/)).toBeInTheDocument();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    await screen.findByText('已提交');
    const [first, second] = kit.of('POST', RUNS).map(call => (call.body as { idempotencyKey: string }).idempotencyKey);
    expect(first).not.toBe(second);
  });

  it('没得到确认：只给「核对原请求」，用原请求身份读回执，不重复建立', async () => {
    kit.route('POST', RUNS, () => 'network');
    const view = setup();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText(/可能已经建立/)).toBeInTheDocument();
    const identity = (kit.of('POST', RUNS)[0]?.body as { idempotencyKey: string }).idempotencyKey;
    expect(screen.queryByRole('button', { name: '仍要重新提交' })).toBeNull();
    expect(screen.queryByRole('button', { name: '采集评论' })).toBeNull();
    kit.route('GET', `${RUNS}/requests/${identity}`, fail(500, 'unavailable'), ok({ runRef: RUN, domainRef: DOMAIN, replayed: true }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson()));
    await view.user.click(screen.getByRole('button', { name: '核对原请求' }));
    expect(await screen.findByText(/可能已经建立/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '仍要重新提交' })).toBeNull();
    await view.user.click(screen.getByRole('button', { name: '核对原请求' }));
    expect(await screen.findByText('已提交')).toBeInTheDocument();
    expect(kit.of('POST', RUNS)).toHaveLength(1);
    expect(kit.of('GET', `${RUNS}/requests/${identity}`)).toHaveLength(2);
  });

  it('会话存储不可写时阻止提交，避免刷新后无法核对身份', async () => {
    const storage = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('storage unavailable'); });
    try {
      const view = setup();
      await view.user.click(screen.getByRole('button', { name: '采集评论' }));
      await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
      expect(await screen.findByText(/无法保存请求身份，尚未提交/)).toBeInTheDocument();
      expect(kit.of('POST', RUNS)).toHaveLength(0);
    } finally { storage.mockRestore(); }
  });

  it('指定40条的未知请求刷新后恢复原条数和身份，核对成功前不会再 POST', async () => {
    kit.route('POST', RUNS, () => 'network');
    const first = setup();
    await first.user.click(screen.getByRole('radio', { name: /指定条数/ }));
    await first.user.clear(screen.getByRole('spinbutton', { name: '条数' }));
    await first.user.type(screen.getByRole('spinbutton', { name: '条数' }), '40');
    await first.user.click(screen.getByRole('button', { name: '采集评论' }));
    await first.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    await screen.findByText(/可能已经建立/);
    const identity = (kit.of('POST', RUNS)[0]?.body as { idempotencyKey: string }).idempotencyKey;
    expect(Object.values(sessionStorage).some(value => value.includes(identity))).toBe(true);
    first.unmount();
    kit.route('GET', `${RUNS}/requests/${identity}`, ok({ runRef: RUN, domainRef: DOMAIN, replayed: true }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson()));
    const restored = setup();
    expect(screen.getByRole('spinbutton', { name: '条数' })).toHaveValue(40);
    expect(screen.getByRole('spinbutton', { name: '条数' })).toBeDisabled();
    expect(screen.getByRole('radio', { name: /全部可见/ })).toBeDisabled();
    expect(screen.queryByRole('button', { name: '采集评论' })).toBeNull();
    await restored.user.click(screen.getByRole('button', { name: '核对原请求' }));
    expect(await screen.findByText('已提交')).toBeInTheDocument();
    expect(kit.of('POST', RUNS)).toHaveLength(1);
  });

  it('核对时服务器说原请求没有记录：说没有建立，可以重新提交', async () => {
    kit.route('POST', RUNS, () => 'network');
    const view = setup();
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    await view.user.click(within(screen.getByRole('dialog')).getByRole('button', { name: '确认提交' }));
    await screen.findByText(/可能已经建立/);
    const identity = (kit.of('POST', RUNS)[0]?.body as { idempotencyKey: string }).idempotencyKey;
    kit.route('GET', `${RUNS}/requests/${identity}`, fail(404, 'collection_run_not_found'));
    await view.user.click(screen.getByRole('button', { name: '核对原请求' }));
    expect(await screen.findByText(/原请求没有在服务器留下记录，没有建立/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '采集评论' })).toBeEnabled();
  });

  it('「尽量采全」走到尽头说「能看到的都采了」（页面总数可能含看不到的），没走到尽头说「没有采全」', () => {
    setup({
      commentsHistory: [
        { ...history[0], observedAt: '2026-10-14T10:01:00Z', collectionScope: 'all_public_comments', collectionState: 'complete', uniqueCollectedCount: 2, pageCommentCount: 3 },
        { ...history[0], observedAt: '2026-10-13T10:01:00Z', collectionScope: 'all_public_comments', collectionState: 'partial', uniqueCollectedCount: 1, pageCommentCount: 3 },
        { ...history[0], observedAt: '2026-10-12T10:01:00Z', collectionScope: 'all_public_comments', collectionState: null, uniqueCollectedCount: 1, pageCommentCount: 2 },
      ],
    });
    const rows = within(screen.getByRole('list', { name: '评论采集记录' })).getAllByRole('listitem');
    expect(rows[0]).toHaveTextContent('2026-10-14 18:01');
    expect(rows[0]).toHaveTextContent('尽量采全');
    expect(rows[0]).toHaveTextContent('采到 2 条');
    expect(rows[0]).toHaveTextContent('页面显示 3');
    expect(rows[0]).toHaveTextContent('能看到的都采了');
    expect(rows[1]).toHaveTextContent('没有采全');
    expect(rows[2]).toHaveTextContent('结果未知');
  });

  it('采指定条数：默认 100，可改；确认框写清采到多少条就停；请求带 up_to 与条数', async () => {
    kit.route('POST', RUNS, ok({ runRef: RUN, domainRef: DOMAIN, replayed: false }));
    kit.route('GET', `${RUNS}/${RUN}`, ok(runJson({ comment_mode: 'up_to', comment_target: 40 })));
    const view = setup();
    await view.user.click(screen.getByRole('radio', { name: /指定条数/ }));
    const input = screen.getByRole('spinbutton', { name: '条数' });
    expect(input).toHaveValue(100);
    await view.user.clear(input);
    await view.user.type(input, '40');
    await view.user.click(screen.getByRole('button', { name: '采集评论' }));
    const dialog = screen.getByRole('dialog', { name: '确认评论采集' });
    expect(within(dialog).getByText(/把评论和回复采到 40 条就停（评论区先到底也会停）/)).toBeInTheDocument();
    await view.user.click(within(dialog).getByRole('button', { name: '确认提交' }));
    expect(await screen.findByText('已提交')).toBeInTheDocument();
    expect(kit.of('POST', RUNS)[0]?.body).toMatchObject({ source: { kind: 'selected_works', workRefs: [workRef(1)] }, workLimit: 1, commentMode: 'up_to', commentTarget: 40 });
    expect(screen.getByRole('radio', { name: /全部可见/ })).toBeDisabled();
  });

  it('条数不是 1–1000 的整数：按钮不可用并就地说明，不发请求', async () => {
    const view = setup();
    await view.user.click(screen.getByRole('radio', { name: /指定条数/ }));
    const input = screen.getByRole('spinbutton', { name: '条数' });
    for (const bad of ['0', '1001', '2.5']) {
      await view.user.clear(input);
      await view.user.type(input, bad);
      expect(screen.getByText('请填 1–1000 之间的整数')).toBeInTheDocument();
      expect(screen.getByRole('button', { name: '采集评论' })).toBeDisabled();
    }
    await view.user.clear(input);
    expect(screen.getByRole('button', { name: '采集评论' })).toBeDisabled();
    expect(kit.calls).toHaveLength(0);
  });

  it('不是小红书：只说暂不支持，不给按钮；历史为空只写暂无采集记录', () => {
    setup({ platform: 'douyin', commentsHistory: [] });
    expect(screen.getByText('评论采集目前只支持小红书笔记。')).toBeInTheDocument();
    expect(screen.getByText('暂无采集记录')).toBeInTheDocument();
    expect(screen.queryByText(/还没有采过/)).toBeNull();
    expect(screen.queryByRole('button', { name: '采集评论' })).toBeNull();
  });
});
