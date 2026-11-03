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
  it('持续悬停 800ms 才出现；快速划过立即取消，不在离开后补弹', async () => {
    const { cell } = setup();
    fireEvent.mouseEnter(cell);
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();
    // 约 40ms 就离开：待打开计时必须取消，800ms 后不应弹出。
    fireEvent.mouseLeave(cell);
    await new Promise(resolve => setTimeout(resolve, 1000));
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();

    fireEvent.mouseEnter(cell);
    // 800ms 之前不出现，避免退回旧的 500ms 计时。
    await new Promise(resolve => setTimeout(resolve, 600));
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ }, { timeout: 2000 });
    fireEvent.mouseLeave(cell);
    fireEvent.mouseEnter(dialog);
    await new Promise(resolve => setTimeout(resolve, 300));
    expect(screen.getByRole('dialog', { name: /^作品预览：/ })).toBeInTheDocument();
    fireEvent.mouseLeave(dialog);
    await waitFor(() => expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull());
  });

  it('键盘聚焦标题给完整标题提示；↓ 展开浮层并移入焦点，Esc 关闭回到标题', async () => {
    const { user } = setup();
    const title = screen.getByRole('link', { name: '合成作品 1' });
    Object.defineProperty(title, 'clientWidth', { configurable: true, value: 60 });
    Object.defineProperty(title, 'scrollWidth', { configurable: true, value: 120 });
    act(() => title.focus());
    const hint = await waitFor(() => {
      const node = document.querySelector('[data-title-tooltip]');
      expect(node).not.toBeNull();
      return node as HTMLElement;
    });
    expect(hint).toHaveTextContent('合成作品 1');
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();

    await user.keyboard('{ArrowDown}');
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ });
    await waitFor(() => expect(within(dialog).getByRole('link', { name: /打开原作品/ })).toHaveFocus());

    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog', { name: /^作品预览：/ })).toBeNull();
    expect(title).toHaveFocus();
    expect(title).toHaveAttribute('aria-expanded', 'false');
  });

  it('未知点赞保持「—」，不写成 0', async () => {
    const { cell } = setup({ likes: null });
    fireEvent.mouseEnter(cell);
    const dialog = await screen.findByRole('dialog', { name: /^作品预览：/ }, { timeout: 2000 });
    expect(dialog).toHaveTextContent('已判相关');
    expect(within(dialog).getByText('—')).toBeInTheDocument();
  });
});
