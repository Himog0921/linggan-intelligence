import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { SelectionBar } from './SelectionBar';

function setup(overrides: Partial<Parameters<typeof SelectionBar>[0]> = {}) {
  const handlers = { onSelectPage: vi.fn(), onDeepCollect: vi.fn(), onOpenBatch: vi.fn(), onResumeBatch: vi.fn() };
  render(<SelectionBar count={3} hasDomain hasPendingBatch={false} {...handlers} {...overrides} />);
  return { ...handlers, user: userEvent.setup() };
}

describe('勾选与批量动作', () => {
  it('显示已勾选篇数；有 1–200 篇时「评论深采」「整理图片文字」可用', async () => {
    const { user, ...handlers } = setup();
    expect(screen.getByRole('status')).toHaveTextContent('已勾选 3 篇作品');
    await user.click(screen.getByRole('button', { name: '勾选本页' }));
    await user.click(screen.getByRole('button', { name: '评论深采' }));
    await user.click(screen.getByRole('button', { name: '整理图片文字' }));
    expect(handlers.onSelectPage).toHaveBeenCalledTimes(1);
    expect(handlers.onDeepCollect).toHaveBeenCalledTimes(1);
    expect(handlers.onOpenBatch).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('button', { name: '继续上次整理' })).toBeNull();
  });

  it('没有勾选、没有领域：两个批量动作禁用', () => {
    setup({ count: 0 });
    expect(screen.getByRole('button', { name: '评论深采' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '整理图片文字' })).toBeDisabled();
  });

  it('没有领域：连「勾选本页」也禁用', () => {
    setup({ hasDomain: false, count: 5 });
    expect(screen.getByRole('button', { name: '勾选本页' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '评论深采' })).toBeDisabled();
  });

  it('超过 200 篇：禁用，并就地说明上限，而不是只禁用', () => {
    setup({ count: 201 });
    expect(screen.getByRole('button', { name: '评论深采' })).toBeDisabled();
    expect(screen.getByText('最多 200 篇，请减少勾选')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('已勾选 201 篇作品');
  });

  it('正好 200 篇仍可用；上次整理未完成时出现「继续上次整理」', async () => {
    const { user, onResumeBatch } = setup({ count: 200, hasPendingBatch: true });
    expect(screen.getByRole('button', { name: '评论深采' })).toBeEnabled();
    await user.click(screen.getByRole('button', { name: '继续上次整理' }));
    expect(onResumeBatch).toHaveBeenCalledTimes(1);
  });
});
