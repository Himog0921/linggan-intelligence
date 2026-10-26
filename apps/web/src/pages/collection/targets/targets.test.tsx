import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { decodeDrawer, decodeList } from './api';
import { bucketPoints, compact, exact, percentile, smoothPath } from './chartMath';
import { useCommands, listHref, pendingTargetFields } from './useWorkspace';
const target = {
  targetRef: '00000000-0000-4000-8000-000000000001',
  kind: 'creator',
  platform: 'xhs',
  identity: 'synthetic-profile',
  handle: 'synthetic-profile',
  name: '合成创作者',
  group: '未分组',
  domainName: '合成领域',
  lifecycle: 'monitoring',
  monitoring: true,
  avatar: { state: 'not_observed', path: null },
  archiveLabel: '档案已建立',
  archiveTone: 'ok',
  patrolLabel: '等待巡查',
  patrolTone: 'neutral',
  works: '1 篇',
  details: '1/1',
  recentNew: null,
  recentHits: null,
  last: null,
  next: null,
  action: { kind: 'view', label: '查看档案', tab: 'works', fragment: 'target-works' },
};
const count = (value: number | null) => (value === null ? { state: 'unknown' } : { state: 'known', value });
const drawer = {
  target,
  contentGapAction: null,
  decision: { title: '当前无需处理', note: '真实回执到达后更新', action: null },
  inspector: {
    archive: { state: 'complete' },
    execution: { state: 'idle' },
    patrol: {
      state: 'waiting',
      lastSucceededAt: null,
      nextRunAt: null,
      latestHits: count(null),
      latestNew: count(0),
    },
    coverage: {
      directoryWorks: count(1),
      capturedDetails: count(1),
      missingDetails: count(0),
      quarantinedRecords: count(0),
      blockedDetails: count(null),
    },
  },
  catalog: [],
  rules: [],
  blocked: [],
  lifecycle: { state: 'not_read' },
};
const json = (value: unknown) =>
  new Response(JSON.stringify(value), { headers: { 'Content-Type': 'application/json' } });
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
describe('采集目标数据与动作边界', () => {
  it('Rust 近90天 wire 枚举归一到原查询口径，切指标不生成无效条件', () => {
    const result = decodeDrawer({ ...drawer, lifecycle: { state: 'ready', projection: {
      status: 'AVAILABLE', asOf: '2026-01-01', window: 'recent90_days', metric: 'comments', points: [],
      summary: { linkedWorkCount: 0, linkedWorkCountLowerBound: 0, confirmedAuthorWorkCount: 0, eligiblePointCount: 0 },
      exclusions: { authorNotVerified: 0, authorMismatch: 0, publishedAtNotQualified: 0, outsideWindow: 0, metricUnknown: 0, scanTruncated: false },
      receipt: { scanLimit: 2000, probedCount: 0, scannedCount: 0, returnedCount: 0, truncated: false },
    } } });
    expect(result.lifecycle.projection?.window).toBe('recent_90_days');
    const href = listHref(new URLSearchParams(), { life_window: result.lifecycle.projection?.window ?? null, life_metric: 'likes' });
    expect(new URL(href, 'http://localhost').searchParams.get('life_window')).toBe('recent_90_days');
  });
  it('未知计数、已知零、已知空目录与读取失败分开解码', () => {
    const decoded = decodeDrawer(drawer);
    expect(decoded.inspector?.hits).toBeNull();
    expect(decoded.inspector?.latestNew).toBe(0);
    expect(decoded.inspector?.blocked).toBeNull();
    expect(decoded.catalog).toEqual([]);
    expect(decodeDrawer({ ...drawer, catalog: null }).catalog).toBeNull();
    expect(() =>
      decodeDrawer({
        ...drawer,
        inspector: {
          ...drawer.inspector,
          patrol: { ...drawer.inspector.patrol, latestNew: { state: 'known' } },
        },
      }),
    ).toThrow();
    expect(() =>
      decodeList({ rows: [target], counts: null, currentDomain: null, domains: [], limit: '200' }),
    ).toThrow();
  });
  it('POST失联后保留原编号，只GET核实且不误报成功', async () => {
    const applied = vi.fn(),
      calls: { url: string; method: string }[] = [];
    let reference = '';
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input),
        method = init?.method ?? 'GET';
      calls.push({ url, method });
      if (url.endsWith('/context')) return json({ epoch: 'synthetic-epoch' });
      if (method === 'POST') {
        const body: unknown = JSON.parse(typeof init?.body === 'string' ? init.body : '{}');
        if (
          typeof body === 'object' &&
          body !== null &&
          'requestRef' in body &&
          typeof body.requestRef === 'string'
        )
          reference = body.requestRef;
        throw new TypeError('offline');
      }
      return json({
        requestRef: reference,
        action: 'archive',
        outcome: 'unconfirmed',
        code: null,
        location: null,
      });
    });
    const { result } = renderHook(() => useCommands(applied));
    await act(async () => {
      await result.current.run('archive', { row_target_ref: target.targetRef });
    });
    expect(result.current.feedback?.unknown).toBe(true);
    expect(result.current.feedback?.requestRef).toBe(reference);
    await act(async () => {
      await result.current.verify();
    });
    await act(async () => {
      await result.current.run('archive', { row_target_ref: target.targetRef });
    });
    expect(calls.filter((c) => c.method === 'POST')).toHaveLength(1);
    expect(calls.some((c) => c.url.includes(`/receipts/${reference}`))).toBe(true);
    expect(applied).not.toHaveBeenCalled();
    expect(result.current.feedback?.unknown).toBe(true);
  });
});
describe('目标导航上下文', () => {
  it('换目标或关闭抽屉不携带上一目标的表现/筛选条件，列表领域与排序保留', () => {
    const params = new URLSearchParams('domain=all&filter=creator&sort=last&drawer=old&dtab=works&wview=performance&life_metric=comments&catalog_query=old&catalog_page=3&inspect_right_round=old&inspect_works=old');
    const changed = new URL(listHref(params, { drawer: target.targetRef }), 'http://localhost');
    expect(changed.searchParams.get('drawer')).toBe(target.targetRef);
    expect(changed.searchParams.has('dtab')).toBe(false);
    expect(changed.searchParams.has('life_metric')).toBe(false);
    expect(changed.searchParams.has('catalog_query')).toBe(false);
    expect(changed.searchParams.has('catalog_page')).toBe(false);
    expect(changed.searchParams.has('inspect_right_round')).toBe(false);
    expect(changed.searchParams.has('inspect_works')).toBe(false);
    expect(changed.searchParams.get('domain')).toBe('all');
    expect(changed.searchParams.get('sort')).toBe('last');
    expect(new URL(listHref(params, { drawer: null }), 'http://localhost').searchParams.has('wview')).toBe(false);
  });
});
describe('合格作品图表呈现', () => {
  it('保留真实零与小数中位数，紧凑数仍保留精确数', () => {
    expect(percentile([0, 1], 0.5)).toBe(0.5);
    expect(exact(1234.5)).toBe('1,234.5');
    expect(compact(1234.5)).toBe('1,234.5');
    expect(compact(12345)).toBe('1.2万');
    expect(percentile([], 0.5)).toBeNull();
  });
  it('按星期一分桶并正确跨年；曲线控制点遵循旧图形', () => {
    const base = {
      publicRef: 'synthetic',
      title: null,
      published: '',
      epochMs: 0,
      value: 0,
      discussionRate: null,
      association: 'DIRECTORY_LINKED',
      new: false,
    };
    expect(
      bucketPoints(
        [
          { ...base, date: '2025-12-31' },
          { ...base, date: '2026-01-02' },
          { ...base, date: '2026-01-05' },
        ],
        true,
      ).map((b) => [b.label, b.points.length]),
    ).toEqual([
      ['2025/12/29', 2],
      ['2026/01/05', 1],
    ]);
    expect(bucketPoints([{ ...base, date: '2026-01-05' }], false)[0]?.label).toBe('2026/01');
    expect(
      smoothPath([
        [62, 416],
        [952, 28],
      ]),
    ).toBe('M 62.0,416.0 C 210.3,351.3 803.7,92.7 952.0,28.0');
  });
});

describe('未确认目标命令跨卸载恢复', () => {
  it('同 action/object 重挂只 GET 原编号，保留原草稿而不重发改过的输入', async () => {
    const object = '00000000-0000-4000-8000-000000000011', calls: { method: string; url: string }[] = [];
    let reference = '';
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input), method = init?.method ?? 'GET'; calls.push({ url, method });
      if (url.endsWith('/context')) return json({ epoch: 'remount' });
      if (method === 'POST') {
        const body = JSON.parse(String(init?.body)); reference = String(body.requestRef); throw new TypeError('lost reply');
      }
      return json({ requestRef: reference, action: 'archive', outcome: 'applied', code: 'archive_requested', location: null });
    });
    const onApplied = vi.fn();
    const original = renderHook(() => useCommands(onApplied, 'synthetic-archive-scope'));
    await act(async () => { await original.result.current.run('archive', { row_target_ref: object, archive_action: 'gaps', domain_ref: 'original-domain' }); });
    expect(original.result.current.feedback?.requestRef).toBe(reference);
    original.unmount();
    const restored = renderHook(() => useCommands(onApplied, 'synthetic-archive-scope'));
    expect(restored.result.current.feedback?.unknown).toBe(true);
    expect(restored.result.current.feedback?.requestRef).toBe(reference);
    expect(pendingTargetFields('archive', { row_target_ref: object })?.domain_ref).toBe('original-domain');
    await act(async () => { await restored.result.current.run('archive', { row_target_ref: object, domain_ref: 'changed-domain' }); });
    expect(calls.filter((c) => c.method === 'POST')).toHaveLength(1);
    expect(calls.some((c) => c.url.includes(`/receipts/${reference}`))).toBe(true);
    expect(onApplied).toHaveBeenCalledOnce();
    expect(restored.result.current.feedback?.unknown).toBe(false);
  });
  it('另一对象不继承未知编号，原对象的恢复仍只读自己的回执', async () => {
    const a = '00000000-0000-4000-8000-000000000021', b = '00000000-0000-4000-8000-000000000022';
    const posts: { requestRef: string; fields: { row_target_ref: string } }[] = [], gets: string[] = [];
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const url = String(input);
      if (url.endsWith('/context')) return json({ epoch: 'two-objects' });
      if (init?.method === 'POST') {
        const body = JSON.parse(String(init.body)); posts.push(body);
        if (body.fields.row_target_ref === a) throw new TypeError('lost A');
        return json({ requestRef: body.requestRef, action: 'archive', outcome: 'applied', code: null, location: null });
      }
      gets.push(url);
      return json({ requestRef: posts[0]?.requestRef, action: 'archive', outcome: 'rejected', code: 'archive_not_requestable', location: null });
    });
    const first = renderHook(() => useCommands(vi.fn(), 'object-a'));
    await act(async () => { await first.result.current.run('archive', { row_target_ref: a }); }); first.unmount();
    const other = renderHook(() => useCommands(vi.fn(), 'object-b'));
    expect(other.result.current.feedback).toBeNull();
    await act(async () => { await other.result.current.run('archive', { row_target_ref: b }); }); other.unmount();
    expect(posts).toHaveLength(2); expect(posts[0]?.requestRef).not.toBe(posts[1]?.requestRef);
    const resumed = renderHook(() => useCommands(vi.fn(), 'object-a'));
    await act(async () => { await resumed.result.current.verify(); });
    expect(posts).toHaveLength(2);
    expect(gets[0]).toContain(`/receipts/${posts[0]?.requestRef}`);
    expect(resumed.result.current.feedback?.code).toBe('archive_not_requestable');
    expect(resumed.result.current.feedback?.unknown).toBe(false);
  });
  it('旧挂载的迟到成功不会丢掉新挂载核实入口，也不导航到已卸载页面', async () => {
    const object = '00000000-0000-4000-8000-000000000031';
    let reference = '', posts = 0, resolvePost: ((response: Response) => void) | undefined;
    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      if (String(input).endsWith('/context')) return json({ epoch: 'late-ack' });
      if (init?.method === 'POST') {
        posts++; reference = String(JSON.parse(String(init.body)).requestRef);
        return new Promise<Response>((resolve) => { resolvePost = resolve; });
      }
      return json({ requestRef: reference, action: 'archive', outcome: 'applied', code: 'archive_requested', location: '/collection/targets?domain=all' });
    });
    const written = vi.fn(), first = renderHook(() => useCommands(written, 'late-object'));
    let submission: Promise<boolean> | undefined;
    await act(async () => { submission = first.result.current.run('archive', { row_target_ref: object }); });
    await waitFor(() => expect(posts).toBe(1)); first.unmount();
    const resumed = renderHook(() => useCommands(written, 'late-object'));
    expect(resumed.result.current.feedback?.requestRef).toBe(reference);
    await act(async () => {
      resolvePost?.(json({ requestRef: reference, action: 'archive', outcome: 'applied', code: 'archive_requested', location: '/collection/targets?domain=all' }));
      await submission;
    });
    expect(written).not.toHaveBeenCalled();
    await act(async () => { await resumed.result.current.verify(); });
    expect(posts).toBe(1); expect(written).toHaveBeenCalledOnce();
    expect(resumed.result.current.feedback?.unknown).toBe(false);
  });

});
