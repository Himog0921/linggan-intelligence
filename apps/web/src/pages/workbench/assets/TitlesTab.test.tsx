import { fireEvent, render as rtlRender, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import type { ReactElement, ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import * as ops from '../../ops/api';
import { newDocument } from '../workbenchModel';
import { TitlesTab, type TitleTools } from './TitlesTab';

vi.mock('../../ops/api', async importOriginal => ({ ...await importOriginal<typeof import('../../ops/api')>(), get: vi.fn() }));
const wrapper = ({ children }: { children: ReactNode }) => <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>{children}</QueryClientProvider>;
const render = (ui: ReactElement) => rtlRender(ui, { wrapper });
beforeEach(() => { vi.mocked(ops.get).mockReset(); });

const tools = (patch: Partial<TitleTools> = {}): TitleTools => ({
  generate: vi.fn(), canGenerate: true, running: false, message: '', cover: null, note: null, choiceLocked: false, choose: vi.fn(), ...patch,
});
type Props = Partial<Parameters<typeof TitlesTab>[0]>;
function show(props: Props = {}) {
  const handlers = { onGoCover: vi.fn(), onCopyTitle: vi.fn() };
  render(<TitlesTab document={newDocument('孩子迟迟不开始写作业')} coverTitle="第一步变小" copyDisabled={false} {...handlers} tools={tools()} {...props} />);
  return handlers;
}

describe('what the draft uses now', () => {
  it('shows the title and the cover text with their lengths against the limits', () => {
    show();
    expect(screen.getByText('孩子迟迟不开始写作业')).toBeVisible();
    expect(screen.getByText('10/20')).toBeInTheDocument();
    expect(screen.getByText('第一步变小')).toBeVisible();
    expect(screen.getByText('5/10')).toBeInTheDocument();
  });
  it('marks an over-long title or cover, and says when nothing is written yet', () => {
    const { container } = render(<TitlesTab document={newDocument('一'.repeat(21))} coverTitle={'二'.repeat(11)} copyDisabled onGoCover={vi.fn()} />);
    expect(container.querySelectorAll('.count.over')).toHaveLength(2);
    render(<TitlesTab document={newDocument('')} coverTitle="" copyDisabled onGoCover={vi.fn()} />);
    expect(screen.getByText('还没有填写标题')).toBeInTheDocument();
    expect(screen.getByText('还没有填写封面大字')).toBeInTheDocument();
  });
  it('goes to the cover, and copies the title only where the page offers it', () => {
    const handlers = show();
    fireEvent.click(screen.getByRole('button', { name: '编辑封面' }));
    expect(handlers.onGoCover).toHaveBeenCalledOnce();
    fireEvent.click(screen.getByRole('button', { name: '复制标题' }));
    expect(handlers.onCopyTitle).toHaveBeenCalledOnce();
  });
  it('hides copy when the page gives no copy action, and disables it while the draft cannot be copied', () => {
    const { unmount } = render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} />);
    expect(screen.queryByRole('button', { name: '复制标题' })).toBeNull();
    unmount();
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onCopyTitle={vi.fn()} onGoCover={vi.fn()} />);
    expect(screen.getByRole('button', { name: '复制标题' })).toBeDisabled();
  });
});

describe('title sources', () => {
  it('offers the title skill and our own hits, and says why the high-likes source is not available yet', () => {
    show();
    expect(screen.getByRole('button', { name: '标题 skill' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: '我方爆款' })).toBeEnabled();
    expect(screen.getByRole('button', { name: '高赞标题' })).toBeDisabled();
    expect(screen.getByText(/高赞标题：要等有跨领域的采集数据/)).toBeVisible();
  });
});

describe('our own hits', () => {
  const pub = (ref: string, title: string, value: number | null, hit: boolean, platform = 'xhs') => ({ ref, platform, title, businessDate: '2026-11-03', account: { name: '小满老师' }, views: { state: value === null ? 'pending' : 'value', value, hit, overdue: false } });
  const open = () => { fireEvent.click(screen.getByRole('button', { name: '我方爆款' })); };
  it('asks the ledger for hits only, lists those that have a title, highest reads first, and reads only once the source is chosen', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [pub('a', '三千阅读', 3000, false), pub('b', '八千阅读的标题', 8420, true), pub('c', '一万二的标题', 12090, true), pub('d', '抖音爆款', 99999, true, 'douyin'), pub('e', '待回填', null, false)] } as never);
    const onUse = vi.fn();
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} tools={tools()} domainRef="d1" enabled onUseOurTitle={onUse} />);
    expect(ops.get).not.toHaveBeenCalled();
    open();
    expect(await screen.findByText('一万二的标题')).toBeVisible();
    expect(ops.get).toHaveBeenCalledWith('/api/local/ops/publications?domain=d1&views=hit', expect.any(Function), expect.anything());
    expect([...document.querySelectorAll('.t-adapt > span:first-child')].map(node => node.textContent)).toEqual(['一万二的标题', '八千阅读的标题']);
    expect(screen.getByText('12,090')).toBeVisible();
    expect(screen.getByText(/用我方爆款的原标题就算“复用”/)).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '用作封面：八千阅读的标题' }));
    expect(onUse).toHaveBeenLastCalledWith('cover', '八千阅读的标题');
    fireEvent.click(screen.getByRole('button', { name: '用作标题：一万二的标题' }));
    expect(onUse).toHaveBeenLastCalledWith('note', '一万二的标题');
  });
  it('does not read the ledger while the sidebar tab is hidden', () => {
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} domainRef="d1" enabled={false} />);
    open();
    expect(ops.get).not.toHaveBeenCalled();
  });
  it('says so when no recent publication is a hit', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [pub('a', '三千阅读', 3000, false)] } as never);
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} domainRef="d1" enabled />);
    open();
    expect(await screen.findByText('运营台账里还没有爆款记录。')).toBeVisible();
  });
  it('reports a failed read and retries on request', async () => {
    vi.mocked(ops.get).mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce({ items: [pub('b', '八千阅读的标题', 8420, true)] } as never);
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} domainRef="d1" enabled />);
    open();
    expect(await screen.findByText(/运营台账暂时读不到/)).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: '重试' }));
    expect(await screen.findByText('八千阅读的标题')).toBeVisible();
  });
  it('does not let a title be put in while the draft is locked', async () => {
    vi.mocked(ops.get).mockResolvedValue({ items: [pub('b', '八千阅读的标题', 8420, true)] } as never);
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} domainRef="d1" enabled usingLocked onUseOurTitle={vi.fn()} />);
    open();
    expect(await screen.findByRole('button', { name: '用作封面：八千阅读的标题' })).toBeDisabled();
  });
});

describe('generating and choosing', () => {
  it('starts generation through the page, and is disabled when the page says it cannot', () => {
    const t = tools();
    const { unmount } = render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} tools={t} />);
    fireEvent.click(screen.getByRole('button', { name: '生成标题候选' }));
    expect(t.generate).toHaveBeenCalledOnce();
    expect(screen.getByText(/还没有候选/)).toBeVisible();
    unmount();
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} tools={tools({ canGenerate: false })} />);
    expect(screen.getByRole('button', { name: '生成标题候选' })).toBeDisabled();
  });
  it('lists cover and note candidates with their lengths, flags a cure promise, and marks the one in use', () => {
    const t = tools({
      cover: [{ text: '先做一步', risk: false, chosen: true }, { text: '治好拖延', risk: true, chosen: false }],
      note: [{ text: '把第一步变小，孩子反而启动得更快', risk: false, chosen: false }],
    });
    const { container } = render(<TitlesTab document={newDocument('t')} coverTitle="先做一步" copyDisabled onGoCover={vi.fn()} tools={t} />);
    expect(screen.getByRole('button', { name: '换一批' })).toBeEnabled();
    expect(screen.getByText('封面大字候选')).toBeVisible();
    expect(screen.getByText('笔记标题候选')).toBeVisible();
    expect(screen.getByText('在用')).toBeVisible();
    expect(screen.queryByRole('button', { name: '用作封面：先做一步' })).toBeNull();
    expect(screen.getByText('有疗效承诺，建议换一个')).toBeVisible();
    expect(container.querySelectorAll('.t-adapt')).toHaveLength(3);
    fireEvent.click(screen.getByRole('button', { name: '用作封面：治好拖延' }));
    expect(t.choose).toHaveBeenLastCalledWith('cover', 1);
    fireEvent.click(screen.getByRole('button', { name: /用作标题：把第一步变小/ }));
    expect(t.choose).toHaveBeenLastCalledWith('note', 0);
  });
  it('does not let a candidate be chosen while the page says a choice is unsafe', () => {
    const t = tools({ choiceLocked: true, cover: [{ text: '先做一步', risk: false, chosen: false }] });
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} tools={t} />);
    expect(screen.getByRole('button', { name: '用作封面：先做一步' })).toBeDisabled();
  });
  it('shows a running request and any failure message', () => {
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} tools={tools({ running: true, message: '请先在设置中配置标题任务的模型和外发权限。' })} />);
    expect(screen.getByRole('status')).toHaveTextContent('正在生成标题…');
    expect(screen.getByRole('alert')).toHaveTextContent('请先在设置中配置标题任务');
    expect(screen.queryByText(/还没有候选/)).toBeNull();
  });
  it('has no generate action without the page tools, and links to the skill settings', () => {
    render(<TitlesTab document={newDocument('t')} coverTitle="" copyDisabled onGoCover={vi.fn()} />);
    expect(screen.queryByRole('button', { name: /生成标题候选|换一批/ })).toBeNull();
    expect(screen.getByRole('link', { name: '管理标题 skill ↗' })).toHaveAttribute('href', '/settings/models?section=skills&task=creation_title');
  });
});
