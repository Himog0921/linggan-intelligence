import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { parseQuery } from './creatorsQuery';
import type { Work } from './creatorsTypes';
import { DOMAIN, creatorKeyFor, workJson } from './testKit';
import { WorkPreviewCell } from './WorkPreviewTip';

const creatorKey = creatorKeyFor('author-1');
const link = { domain: DOMAIN, query: parseQuery(new URLSearchParams()), returnTo: '/corpus/creators' };

function setup(overrides: Record<string, unknown> = {}) {
  const work = workJson(1, overrides) as unknown as Work;
  const resources = {
    byRef: new Map([[work.workRef, { publicRef: work.workRef, title: work.title, avatarUrl: null, coverUrl: '/api/local/media/cover-01.svg' }]]),
    pending: false,
    failed: false,
  };
  const user = userEvent.setup();
  const view = render(
    <WorkPreviewCell work={work} title={work.title ?? ''} creatorKey={creatorKey} link={link} resources={resources} />,
  );
  return { ...view, user, work, cell: view.container.querySelector('[data-work-preview]') as HTMLElement };
}

afterEach(() => {
  vi.useRealTimers();
});

describe('作品预览浮层', () => {
  it('聚焦即打开，Enter 把焦点移进浮层，Esc 关闭并把焦点交回触发点；再按 Enter 仍可打开', async () => {
    const { user } = setup();
    const trigger = screen.getByRole('button', { name: /^查看完整预览/ });
    act(() => trigger.focus());
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ });
    expect(trigger).toHaveAttribute('aria-expanded', 'true');
    expect(within(dialog).getByText('合成作品 1')).toBeInTheDocument();

    await user.keyboard('{Enter}');
    await waitFor(() => expect(within(dialog).getByRole('link', { name: /打开原作品/ })).toHaveFocus());

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();
    expect(trigger).toHaveFocus();
    expect(trigger).toHaveAttribute('aria-expanded', 'false');

    // Esc 交回焦点的那一次聚焦不应把浮层立即重开；用户再按 Enter 时仍可打开。
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog', { name: /^作品预览：/ })).toBeInTheDocument();
  });

  it('悬停约 250ms 才出现；指针移进浮层不会关闭，移出后收起', async () => {
    const { cell } = setup();
    fireEvent.mouseEnter(cell);
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ }, { timeout: 2000 });
    fireEvent.mouseLeave(cell);
    fireEvent.mouseEnter(dialog);
    await new Promise(resolve => setTimeout(resolve, 300));
    expect(screen.getByRole('dialog', { name: /^作品预览：/ })).toBeInTheDocument();
    fireEvent.mouseLeave(dialog);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull());
  });

  it('未知点赞保持「—」，不写成 0', async () => {
    const { user } = setup({ likes: null });
    await user.click(screen.getByRole('button', { name: /^查看完整预览/ }));
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ });
    expect(dialog).toHaveTextContent('已判相关');
    expect(within(dialog).getByText('—')).toBeInTheDocument();
  });
});
