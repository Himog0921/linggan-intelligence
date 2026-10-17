import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { DiscussionBlock } from './DiscussionBlock';
import { parseWorkDetail } from './evidenceDecode';
import { DOMAIN, commentsJson, detailJson, workRef } from './evidenceFixtures';
import { createFetchMock, deferred, fail, ok } from './testKit';
import { useCommentChannel } from './useCommentChannel';
import type { DetailOptions } from './evidenceFixtures';

afterEach(() => {
  sessionStorage.clear();
  localStorage.clear();
});

const PATH = `/api/local/work-resources/${workRef(1)}/comments`;

function Harness({ options = {} }: { options?: DetailOptions }) {
  const detail = parseWorkDetail(detailJson(1, { comments: 321, ...options }));
  const channelUrl = detail.channels.commentsUrl;
  const channel = useCommentChannel(channelUrl);
  return <DiscussionBlock item={detail.item} inspector={detail.inspector} channelUrl={channelUrl} channel={channel} />;
}

function setup(options: DetailOptions = {}) {
  const kit = createFetchMock();
  kit.install();
  const view = render(<Harness options={options} />);
  return { kit, ...view, user: userEvent.setup() };
}

describe('评论与回复', () => {
  it('点击之前不读评论原文：只有覆盖读数、两种事实并列的一行，和授权说明', () => {
    const { kit } = setup();
    expect(kit.calls).toHaveLength(0);
    expect(screen.getByText('平台显示 321 条，本地留存 45 条')).toBeInTheDocument();
    expect(screen.getByText('原文只在本机授权详情中按页读取')).toBeInTheDocument();
    expect(screen.getByText(/作者只显示匿名上下文；页面不会渲染平台用户标识/)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '读取评论原文' })).toBeEnabled();
    expect(screen.queryByText(/匿名评论/)).toBeNull();
  });

  it('平台数与本地留存数是两种事实：平台数未知、本地数未知各自写明，不互相推算', () => {
    setup({ comments: null, commentsCoverage: { count: null, countState: 'UNKNOWN' } });
    expect(screen.getByText('平台评论数当前未知，本地留存当前未知')).toBeInTheDocument();
  });

  it('读取后按页显示匿名评论与回复；回执一行；继续读取追加第二页，游标只加一个参数', async () => {
    const { kit, user } = setup();
    kit.route('GET', PATH, ok(commentsJson(20, { cursor: 'c2', total: 48 })), ok(commentsJson(20, { offset: 20, total: 48 })));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    await screen.findByText('总数 48 · 当前 20 · 本次 20；本页已截断，可继续读取');
    expect(screen.getAllByText('匿名评论').length).toBeGreaterThan(0);
    expect(screen.getAllByText('匿名回复').length).toBeGreaterThan(0);
    expect(screen.getAllByText('本条正文未在通道内截断')).toHaveLength(20);
    expect(kit.of('GET', PATH)[0]?.search.has('cursor')).toBe(false);
    await user.click(screen.getByRole('button', { name: '继续读取评论' }));
    await screen.findByText(/当前 40/);
    expect(kit.of('GET', PATH)[1]?.search.get('cursor')).toBe('c2');
    expect(kit.of('GET', PATH)[1]?.search.get('domain')).toBe(DOMAIN);
    expect(screen.queryByRole('button', { name: '继续读取评论' })).toBeNull();
  });

  it('不渲染平台用户标识；评论文本不进地址和浏览器存储', async () => {
    const { kit, user, container } = setup();
    kit.route('GET', PATH, ok(commentsJson(2, { total: 2 })));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    await screen.findAllByText(/合成评论/);
    expect(container.textContent).not.toMatch(/WITHHELD|platformUserId|anonymousAuthorContext/);
    expect(sessionStorage.length).toBe(0);
    expect(localStorage.length).toBe(0);
  });

  it('正文未知写「评论正文当前未知」；被读取边界截断要说明', async () => {
    const { kit, user } = setup();
    const page = commentsJson(2, { total: 2 }) as unknown as { items: Record<string, unknown>[] };
    page.items[0]!.body = null;
    page.items[0]!.bodyState = 'UNKNOWN';
    page.items[1]!.bodyTruncated = true;
    kit.route('GET', PATH, ok(page));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    expect(await screen.findByText('评论正文当前未知')).toBeInTheDocument();
    expect(screen.getByText('本条正文已在读取边界截断')).toBeInTheDocument();
  });

  it('读取失败：中文说明＋错误码旁注；可再点读取', async () => {
    const { kit, user } = setup();
    kit.route('GET', PATH, fail(503, 'material_comment_channel_unavailable'), ok(commentsJson(1)));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    const failure = (await screen.findByText('评论研究通道读取失败')).closest('div') as HTMLElement;
    expect(within(failure).getByText('评论通道当前不可读。')).toBeInTheDocument();
    expect(within(failure).getByText('material_comment_channel_unavailable')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    await screen.findByText('匿名评论');
    expect(screen.queryByText('评论研究通道读取失败')).toBeNull();
  });

  it('通道返回空：只描述本次授权读取，不表示平台评论为 0', async () => {
    const { kit, user } = setup();
    kit.route('GET', PATH, ok(commentsJson(0, { total: 0 })));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    expect(await screen.findByText('当前通道没有返回评论材料')).toBeInTheDocument();
    expect(screen.getByText('这只描述本次授权读取，不表示平台评论为 0。')).toBeInTheDocument();
  });

  it('续读失败时已读到的评论仍保留', async () => {
    const { kit, user } = setup();
    kit.route('GET', PATH, ok(commentsJson(2, { cursor: 'c2', total: 5 })), fail(503, 'material_comment_channel_unavailable'));
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    await screen.findAllByText(/合成评论/);
    await user.click(screen.getByRole('button', { name: '继续读取评论' }));
    await screen.findByText('继续读取评论失败');
    expect(screen.getAllByText(/合成评论/)).toHaveLength(2);
  });

  it('详情没给评论通道地址：按钮禁用并写明来源信息不完整，不猜路由', () => {
    const { kit } = setup({ commentsUrl: null });
    expect(screen.getByRole('button', { name: '读取评论原文' })).toBeDisabled();
    expect(screen.getByText('来源信息不完整：详情没有给出评论通道地址。')).toBeInTheDocument();
    expect(kit.calls).toHaveLength(0);
  });

  it('卸载时取消在途请求，迟到的响应不再写入', async () => {
    const { kit, user, unmount } = setup();
    const gate = deferred<{ status: number; body: unknown }>();
    let signal: AbortSignal | null | undefined;
    kit.fetchMock.mockImplementationOnce(async (_input, init) => {
      signal = init?.signal;
      const reply = await gate.promise;
      return new Response(JSON.stringify(reply.body), { status: reply.status, headers: { 'Content-Type': 'application/json' } });
    });
    await user.click(screen.getByRole('button', { name: '读取评论原文' }));
    await waitFor(() => expect(signal).toBeTruthy());
    unmount();
    expect(signal?.aborted).toBe(true);
    gate.resolve({ status: 200, body: commentsJson(1) });
  });
});
