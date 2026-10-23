import { screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { DossierComments } from './DossierComments';
import { DOMAIN, dossierCommentsJson, workRef } from './evidenceFixtures';
import { createFetchMock, fail, ok, renderWithProviders, type FetchKit } from './testKit';

const PATH = `/api/local/work-resources/${workRef(1)}/research-dossier/comments`;

function setup(...responders: Parameters<FetchKit['route']>[2][]) {
  const kit = createFetchMock();
  kit.install();
  kit.route('GET', PATH, ...responders);
  const view = renderWithProviders(<DossierComments work={workRef(1)} domain={DOMAIN} />);
  return { kit, ...view };
}

function withReply() {
  const page = dossierCommentsJson(2);
  const [root, second] = page.items;
  return { ...page, items: [root, { ...second!, relation: 'REPLY', parentSourceRef: root!.sourceRef, body: '合成回复', likeCount: null }] };
}

afterEach(() => {
  sessionStorage.clear();
  localStorage.clear();
});

describe('评论与回复：直接展示（Mog 2026-10-23）', () => {
  it('打开就读第一页并直接显示全文，不需要点「读取」；没有数量解释语', async () => {
    const { kit } = setup(ok(dossierCommentsJson(3, { total: 3 })));
    expect(await screen.findByText('合成评论 0')).toBeInTheDocument();
    expect(screen.getByText('合成评论 2')).toBeInTheDocument();
    expect(kit.of('GET', PATH)).toHaveLength(1);
    expect(kit.of('GET', PATH)[0]?.search.get('domain')).toBe(DOMAIN);
    expect(screen.queryByRole('button', { name: /读取前/ })).toBeNull();
    expect(screen.queryByText(/可读取评论|数量相同|不代表平台展示顺序/)).toBeNull();
    expect(screen.queryByRole('button', { name: '加载更多评论' })).toBeNull();
    expect(screen.getAllByText('赞 5')).toHaveLength(3);
  });

  it('回复缩进在它回复的主评论下；点赞未知写「—」，不写 0', async () => {
    setup(ok(withReply()));
    const reply = await screen.findByText('合成回复');
    const replyItem = reply.closest('article')!;
    expect(replyItem).toHaveAttribute('data-relation', 'REPLY');
    expect(replyItem.parentElement?.closest('article')).toHaveTextContent('合成评论 0');
    expect(screen.getByText('回复 · 赞 —')).toBeInTheDocument();
  });

  it('还有更多时「加载更多评论」带游标续读，跨页去重', async () => {
    const { kit, user } = setup(ok(dossierCommentsJson(30, { cursor: 'next-1', total: 32 })), ok(dossierCommentsJson(3, { offset: 29, total: 32 })));
    await screen.findByText('合成评论 29');
    await user.click(screen.getByRole('button', { name: '加载更多评论' }));
    await screen.findByText('合成评论 31');
    expect(kit.of('GET', PATH)[1]?.search.get('cursor')).toBe('next-1');
    expect(screen.getAllByText('合成评论 29')).toHaveLength(1);
    expect(screen.queryByRole('button', { name: '加载更多评论' })).toBeNull();
  });

  it('一条都没采到时如实说；读失败只在这里提示，可以重读', async () => {
    const empty = setup(ok(dossierCommentsJson(0, { total: 0 })));
    expect(await screen.findByText('还没有采到评论。')).toBeInTheDocument();
    empty.unmount();
    const { kit, user } = setup(fail(503, 'material_detail_unavailable'), ok(dossierCommentsJson(1)));
    expect(await screen.findByText(/评论暂时读不出来，正文和图片不受影响/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '重新读取' }));
    expect(await screen.findByText('合成评论 0')).toBeInTheDocument();
    await waitFor(() => expect(kit.of('GET', PATH)).toHaveLength(2));
  });
});
