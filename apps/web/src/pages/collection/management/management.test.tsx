import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandForm } from './CommandForm';
import { DomainManagementContent } from './DomainsPage';
import { decodeDomains, decodeRuntime } from './managementApi';
import { RuntimeContent } from './RuntimePage';

function domains(overrides: Record<string, unknown> = {}) {
  return {
    asOf: '2026-10-16 09:00+08', returnTo: '/collection/tasks?domain=all', readable: true, targetsReadable: true, relationsReadable: true,
    domains: [{ domainRef: 'd', name: '合成领域', description: null, researchGoal: null, status: 'active', targetCount: 0, creatorTargetCount: 0, keywordTargetCount: 0, primaryTargetCount: 0, referenceTargetCount: 0, sampleCount: null, commentCount: null, lastReadable: false, lastObservation: null, lanes: [{ lane: 'detail', label: '详情', summary: '未知', readable: false }] }], targets: [], relations: [], ...overrides,
  };
}
function runtime(overrides: Record<string, unknown> = {}) {
  return {
    asOf: '2026-10-16 09:00+08', returnTo: '/collection/targets', readable: true,
    projection: {
      meta: { vacantStations: 0, unclaimedInstallations: 0 },
      board: { state: '运行状态读不到', tone: 'unknown', badge: '读不到', headNote: '通道数未知', metrics: [] }, lanes: null,
      stations: [{ stationRef: 's', name: '合成工位', stateLabel: '失联', stateTone: 'c-tg-warn', pluginVersion: 'synthetic', lastSeen: null, dailyUsed: 0, dailyQuota: 200, quotaExhausted: false, litSegments: 0, claimWindowOpen: false, dispatchLabel: '从未问过活', dispatchHint: '先确认插件安装', dispatchAt: null, capSummary: '读不到', capTone: 'c-tg-neutral', capabilities: null, account: null, selectorHealth: [] }],
      unclaimed: [], accountObservationAvailable: false, staffedCount: 1, running: [], schedule: [],
      drawer: { coverage: '0 / 1 台工位有读数', readCount: 0, picked: 0, capabilityFailures: 0, executionFailures: 0, oldest: null, pauses: null, dispatched: '读不到', succeeded: '读不到' }, ...overrides,
    },
  };
}
afterEach(() => vi.unstubAllGlobals());
describe('collection management data boundaries', () => {
  it('keeps modal cards and create forms outside the full-page typography scope', () => {
    const read = decodeDomains(domains()), reload = vi.fn();
    const { container, rerender } = render(<DomainManagementContent read={read} reload={reload} onlyDomain="d" />);
    expect(container.firstElementChild).toHaveClass('domain-card');
    expect(container.querySelector('.domain-management')).toBeNull();
    rerender(<DomainManagementContent read={read} reload={reload} onlyDomain="new" />);
    expect(container.firstElementChild).toHaveClass('domain-create');
    expect(container.querySelector('.domain-management')).toBeNull();
    rerender(<DomainManagementContent read={read} reload={reload} />);
    expect(container.firstElementChild).toHaveClass('domain-management');
  });
  it('keeps unavailable material counts separate from observed zero and never invents a recent receipt', () => {
    render(<DomainManagementContent read={decodeDomains(domains({ targetsReadable: false }))} reload={vi.fn()} />);
    expect(screen.getByText('未知 篇作品 · 未知 条评论')).toBeInTheDocument();
    expect(screen.getByText('最近接纳：未知')).toBeInTheDocument();
    expect(screen.getByText('0 个观察目标：博主 0 · 关键词 0')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '关联目标' })).toBeDisabled();
  });
  it('rejects malformed counts and preserves unreadable roster instead of turning it into no stations', () => {
    expect(() => decodeDomains(domains({ domains: [{ ...domains().domains[0], commentCount: -1 }] }))).toThrow();
    const read = decodeRuntime(runtime({ stations: null }));
    if (!read.projection) throw new Error('missing synthetic projection');
    render(<RuntimeContent data={read.projection} reload={vi.fn()} />);
    expect(screen.getByText('工位列表当前读不到。这不表示没有登记工位，也不表示它们离线——两者的处置不同。')).toBeInTheDocument();
    expect(screen.queryByText(/还没有登记任何工位/)).not.toBeInTheDocument();
  });
  it('retains the station management actions and explicit unknown capability/account states', async () => {
    const read = decodeRuntime(runtime());
    if (!read.projection) throw new Error('missing synthetic projection');
    render(<RuntimeContent data={read.projection} reload={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: '管理 合成工位 工位' }));
    const modal = screen.getByRole('dialog', { name: '管理 合成工位' });
    expect(within(modal).getByText(/能力矩阵当前读不到/)).toBeInTheDocument();
    expect(within(modal).getByText(/接活开关与账号绑定当前读不到/)).toBeInTheDocument();
    expect(within(modal).getByRole('button', { name: '开窗口' })).toBeInTheDocument();
    expect(within(modal).getByRole('button', { name: '停用这台工位' })).toBeInTheDocument();
  });
  it('keeps native field constraints and leaves draft input after an explicit server rejection', async () => {
    const calls: { path: string; body: unknown }[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string, options?: RequestInit) => {
      const body: unknown = options?.body ? JSON.parse(String(options.body)) : null; calls.push({ path, body });
      if (path.endsWith('/context')) return Response.json({ epoch: 'test' });
      const requestRef = typeof body === 'object' && body !== null && 'requestRef' in body ? body.requestRef : null;
      return Response.json({ requestRef, action: 'domain-save', outcome: 'rejected', code: 'name_taken', location: null }, { status: 409 });
    }));
    const written = vi.fn();
    render(<CommandForm action="domain-save" onWritten={written}><input name="name" aria-label="名称" required defaultValue="未保存的领域" /><button type="submit">保存</button></CommandForm>);
    await userEvent.click(screen.getByRole('button', { name: '保存' }));
    await screen.findByText(/name_taken/);
    expect(screen.getByRole('textbox', { name: '名称' })).toHaveValue('未保存的领域');
    expect(written).not.toHaveBeenCalled();
    expect(calls.filter(call => call.path.endsWith('/domain-save'))).toHaveLength(1);
  });
  it('does not resend an unconfirmed local command; verification reads the same request identity', async () => {
    const calls: { path: string; method: string; body: unknown }[] = [];
    let identity: unknown;
    vi.stubGlobal('fetch', vi.fn(async (path: string, options?: RequestInit) => {
      const body: unknown = options?.body ? JSON.parse(String(options.body)) : null;
      calls.push({ path, method: options?.method ?? 'GET', body });
      if (path.endsWith('/context')) return Response.json({ epoch: 'synthetic' });
      if (options?.method === 'POST') {
        if (typeof body === 'object' && body !== null && 'requestRef' in body) identity = body.requestRef;
        throw new TypeError('lost reply');
      }
      return Response.json({ requestRef: identity, action: 'runtime-register', outcome: 'applied', code: null, location: '/collection/runtime' });
    }));
    const written = vi.fn();
    render(<CommandForm action="runtime-register" onWritten={written}><input name="display_name" aria-label="工位名称" defaultValue="合成工位" /><button type="submit">登记</button></CommandForm>);
    await userEvent.click(screen.getByRole('button', { name: '登记' }));
    await screen.findByText(/结果未确认/);
    expect(screen.getByRole('button', { name: '登记' })).toBeDisabled();
    expect(written).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: '核实同一请求' }));
    await waitFor(() => expect(written).toHaveBeenCalledOnce());
    expect(calls.filter(call => call.method === 'POST')).toHaveLength(1);
    expect(calls.some(call => call.path.includes(`/receipts/${String(identity)}`))).toBe(true);
  });
  it('restores an unconfirmed create draft after closing and remounting and can only read its receipt', async () => {
    let identity: unknown; const methods: string[] = [];
    vi.stubGlobal('fetch', vi.fn(async (path: string, options?: RequestInit) => {
      methods.push(options?.method ?? 'GET');
      if (path.endsWith('/context')) return Response.json({ epoch: 'remount' });
      if (options?.method === 'POST') {
        const body: unknown = JSON.parse(String(options.body));
        if (typeof body === 'object' && body !== null && 'requestRef' in body) identity = body.requestRef;
        throw new TypeError('reply lost after local write');
      }
      expect(path).toContain(`/receipts/${String(identity)}`);
      return Response.json({ requestRef: identity, action: 'runtime-register', outcome: 'applied', code: null, location: '/collection/runtime' });
    }));
    const written = vi.fn();
    const form = () => <CommandForm action="runtime-register" onWritten={written}><input name="display_name" aria-label="工位名称" defaultValue="本机 Chrome" /><button type="submit">登记</button></CommandForm>;
    const initial = render(form());
    await userEvent.clear(screen.getByRole('textbox', { name: '工位名称' }));
    await userEvent.type(screen.getByRole('textbox', { name: '工位名称' }), '需要核实的工位草稿');
    await userEvent.click(screen.getByRole('button', { name: '登记' })); await screen.findByText(/结果未确认/);
    initial.unmount(); render(form());
    expect(screen.getByRole('textbox', { name: '工位名称' })).toHaveValue('需要核实的工位草稿');
    expect(screen.getByRole('button', { name: '登记' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '核实同一请求' }));
    await waitFor(() => expect(written).toHaveBeenCalledOnce());
    expect(methods.filter(method => method === 'POST')).toHaveLength(1);
  });
  it('keeps unsettled intents isolated by stable station identity', async () => {
    let firstIdentity: unknown; let posts = 0;
    vi.stubGlobal('fetch', vi.fn(async (path: string, options?: RequestInit) => {
      if (path.endsWith('/context')) return Response.json({ epoch: 'scope' });
      if (options?.method === 'POST') {
        posts++; const body: unknown = JSON.parse(String(options.body));
        const requestRef = typeof body === 'object' && body !== null && 'requestRef' in body ? body.requestRef : null;
        if (posts === 1) { firstIdentity = requestRef; throw new TypeError('reply lost'); }
        return Response.json({ requestRef, action: 'runtime-rename', outcome: 'applied', code: null, location: '/collection/runtime' });
      }
      return Response.json({ requestRef: firstIdentity, action: 'runtime-rename', outcome: 'applied', code: null, location: '/collection/runtime' });
    }));
    const form = (station: string) => <CommandForm action="runtime-rename" fields={{ station_ref: station }} onWritten={vi.fn()}><input name="display_name" aria-label="工位名称" defaultValue={station} /><button type="submit">保存</button></CommandForm>;
    const first = render(form('station-a'));
    await userEvent.click(screen.getByRole('button', { name: '保存' })); await screen.findByText(/结果未确认/); first.unmount();
    const other = render(form('station-b'));
    expect(screen.queryByText(/结果未确认/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '保存' })).not.toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '保存' })); await screen.findByText('已收到服务器回执。'); other.unmount();
    render(form('station-a'));
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: '核实同一请求' })); await screen.findByText('已收到服务器回执。');
    expect(posts).toBe(2);
  });

});
