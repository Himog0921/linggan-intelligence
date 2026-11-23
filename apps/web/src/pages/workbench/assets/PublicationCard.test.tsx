import { fireEvent, render as rtlRender, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as ops from '../../ops/api';
import * as api from './assetsApi';
import { PublicationCard } from './PublicationCard';

vi.mock('./assetsApi', async importOriginal => ({ ...await importOriginal<typeof import('./assetsApi')>(), publicationLinks: vi.fn() }));
vi.mock('../../ops/api', async importOriginal => ({ ...await importOriginal<typeof import('../../ops/api')>(), get: vi.fn() }));
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{children}</QueryClientProvider>;
const render = (ui: ReactElement) => rtlRender(ui, { wrapper });

const WORK = '00000000-0000-4000-8000-000000000010', DOMAIN = '00000000-0000-4000-8000-000000000011', ACCOUNT = '00000000-0000-4000-8000-000000000012', PUB = '00000000-0000-4000-8000-000000000013';
const link = (patch: Record<string, unknown> = {}) => ({ linkRef: PUB, revisionId: PUB, publicationRef: PUB, title: '为什么越催越慢', publishedOn: '2026-11-10', available: true, ...patch });
const item = (views: Record<string, unknown>, patch: Record<string, unknown> = {}) => ({ ref: PUB, platform: 'xhs', likes: 312, views, ...patch });
const show = () => render(<PublicationCard workId={WORK} domainRef={DOMAIN} accountRef={ACCOUNT} />);

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.publicationLinks).mockResolvedValue([link()]);
});

describe('recovery card of a published draft', () => {
  it('still finds the record when the link has no publication day', async () => {
    vi.mocked(api.publicationLinks).mockResolvedValue([link({ publishedOn: null })]);
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'value', value: 6116, overdue: false, hit: true })] } as never);
    show();
    expect(await screen.findByText('6,116')).toBeVisible();
    expect(String(vi.mocked(ops.get).mock.calls[0]?.[0])).toContain(`publication_ref=${PUB}`);
  });
  it('shows the publication day, the reads and likes from the ledger, and calls a record over the line a hit', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'value', value: 6116, overdue: false, hit: true })] } as never);
    show();
    expect(await screen.findByText('6,116')).toBeVisible();
    expect(screen.getByText('312')).toBeVisible();
    expect(screen.getByText('爆款')).toBeVisible();
    expect(screen.getByText('已有阅读数据')).toBeVisible();
    expect(screen.getByText(/2026-11-10 · 为什么越催越慢/)).toBeVisible();
  });
  it('asks the ledger for exactly that record, whatever its day or account, and links to where the numbers are filled in', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'value', value: 6116, overdue: false, hit: true })] } as never);
    show();
    await screen.findByText('6,116');
    const requested = String(vi.mocked(ops.get).mock.calls[0]?.[0]);
    expect(requested).toContain(`domain=${DOMAIN}`);
    expect(requested).toContain(`publication_ref=${PUB}`);
    // No page of someone's busy account or a missing publication day can hide the record.
    expect(requested).not.toMatch(/account_ref=|from=|to=/);
    expect(screen.getByRole('link', { name: '去运营发布记录 ↗' })).toHaveAttribute('href', `/ops/publications?domain=${DOMAIN}&account_ref=${ACCOUNT}`);
  });
  it('says a record with reads below the line is not a hit', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'value', value: 3150, overdue: false, hit: false })] } as never);
    show();
    expect(await screen.findByText('未到爆款线')).toBeVisible();
    expect(screen.queryByText('爆款')).toBeNull();
  });
  it('says the reads are still being collected, and when a week has passed without them', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'pending', value: null, overdue: true, hit: false })] } as never);
    show();
    expect(await screen.findByText('已满 7 天，阅读还没填')).toBeVisible();
    expect(screen.getByText('待回填')).toBeVisible();
    expect(screen.getByText('回收中')).toBeVisible();
  });
  it('does not judge a non-Xiaohongshu record', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [item({ state: 'optional', value: null, overdue: false, hit: false }, { platform: 'douyin' })] } as never);
    show();
    expect(await screen.findByText('选填，不判爆款')).toBeVisible();
  });
  it('explains when the linked record is not available, without asking the ledger', async () => {
    vi.mocked(api.publicationLinks).mockResolvedValue([link({ available: false })]);
    show();
    expect(await screen.findByText(/关联的发布记录当前不可用/)).toBeVisible();
    expect(ops.get).not.toHaveBeenCalled();
  });
  it('says when the ledger has no data for the record, and reports a failed read with a retry', async () => {
    vi.mocked(ops.get).mockResolvedValueOnce({ items: [] } as never);
    const first = show();
    expect(await screen.findByText('运营台账里还没有这条记录的数据。')).toBeVisible();
    first.unmount();
    vi.mocked(ops.get).mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce({ items: [item({ state: 'value', value: 6116, overdue: false, hit: true })] } as never);
    show();
    expect(await screen.findByText(/运营台账暂时读不到/)).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    expect(await screen.findByText('6,116')).toBeVisible();
  });
  it('reports a failed read of the link itself', async () => {
    vi.mocked(api.publicationLinks).mockRejectedValue(new Error('boom'));
    show();
    expect(await screen.findByText(/关联的发布记录读取失败/)).toBeVisible();
  });
});
