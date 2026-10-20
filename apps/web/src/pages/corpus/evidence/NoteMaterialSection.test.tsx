import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { NoteMaterialSection } from './NoteMaterialSection';
import { DOMAIN, noteMaterialJson, OTHER_DOMAIN, workRef } from './evidenceFixtures';

afterEach(() => vi.unstubAllGlobals());
const response = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
function mount() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const view = (work = workRef(1), domain = DOMAIN) => <QueryClientProvider client={client}><NoteMaterialSection key={`${domain}:${work}`} work={work} domain={domain} /></QueryClientProvider>;
  return { ...render(view()), view, user: userEvent.setup() };
}
async function open() {
  await userEvent.click(screen.getByText('展开每篇解读材料'));
  await screen.findByRole('button', { name: '刷新每篇材料' });
}

it('展开才读；五列与转写上下文分开，原文/Unicode引用与读取身份可核对', async () => {
  const mock = vi.fn(async () => response(noteMaterialJson())); vi.stubGlobal('fetch', mock);
  mount(); expect(mock).not.toHaveBeenCalled(); await open();
  expect(await screen.findByText('底稿准备状态：材料仍有待处理项')).toBeInTheDocument();
  expect(screen.getByText('转写处理未结束')).toBeInTheDocument();
  expect(screen.getByText('转写上下文（未纳入默认每篇输入）')).toBeInTheDocument();
  expect(screen.getByText('合成🙂文')).toBeInTheDocument();
  expect(screen.getByText('4–8')).toBeInTheDocument();
  expect(screen.getByText(/没有保存快照/)).toBeInTheDocument();
  expect(mock).toHaveBeenCalledOnce();
});

it('刷新期间立即隐藏缓存原文；读取失败、未知状态和随后受限响应都不恢复旧正文', async () => {
  let release: (value: Response) => void = () => { throw new Error('not started'); };
  const mock = vi.fn<(...args: unknown[]) => Promise<Response>>()
    .mockResolvedValueOnce(response(noteMaterialJson()))
    .mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
  vi.stubGlobal('fetch', mock); const { user } = mount(); await open(); await screen.findByText('合成🙂文');
  await user.click(screen.getByRole('button', { name: '刷新每篇材料' }));
  expect(screen.queryByText('合成🙂文')).not.toBeInTheDocument();
  await act(async () => release(response({ error: 'unavailable' }, 503)));
  await screen.findByText(/读取未成功/); expect(screen.queryByText('合成🙂文')).not.toBeInTheDocument();
  const unknown = noteMaterialJson(); unknown.columns[0]!.state = 'new_state'; mock.mockResolvedValueOnce(response(unknown));
  await user.click(screen.getByRole('button', { name: '刷新每篇材料' })); await screen.findByText(/读取合同待核对/);
  expect(screen.getByText('state: new_state')).toBeInTheDocument();
  expect(screen.queryByText('合成🙂文')).not.toBeInTheDocument();
  const restricted = noteMaterialJson(); restricted.columns.find(column => column.field === 'body')!.state = 'restricted';
  mock.mockResolvedValueOnce(response(restricted)); await user.click(screen.getByRole('button', { name: '刷新每篇材料' }));
  await screen.findByText('本栏来源受限，文字不在此显示。');
  expect(screen.queryByText('合成🙂文')).not.toBeInTheDocument();
});

it.each([[workRef(2), DOMAIN], [workRef(1), OTHER_DOMAIN]])('单独换作品或领域时旧请求被取消，失序响应不可覆盖；404不解释为材料不存在 (%s/%s)', async (nextWork, nextDomain) => {
  let release: (value: Response) => void = () => { throw new Error('not started'); };
  let oldSignal: AbortSignal | undefined;
  const mock = vi.fn().mockImplementationOnce((_path: string, options: RequestInit) => {
    oldSignal = options.signal ?? undefined;
    return new Promise<Response>(resolve => { release = resolve; });
  }).mockResolvedValueOnce(response({ error: 'material_not_found' }, 404));
  vi.stubGlobal('fetch', mock); const { rerender, view } = mount(); await open();
  await waitFor(() => expect(mock).toHaveBeenCalledOnce());
  rerender(view(nextWork, nextDomain)); expect(oldSignal?.aborted).toBe(true);
  await open(); await screen.findByText(/当前领域或作品范围无法读取/);
  await act(async () => release(response(noteMaterialJson())));
  expect(screen.queryByText('合成🙂文')).not.toBeInTheDocument();
  expect(mock).toHaveBeenCalledTimes(2);
});
