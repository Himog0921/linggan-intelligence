import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { SelectionBar } from './SelectionBar';

function setup(overrides: Partial<Parameters<typeof SelectionBar>[0]> = {}) {
  const handlers = { onClear: vi.fn(), onDeepCollect: vi.fn() };
  render(<SelectionBar count={3} visibleSelected={2} visibleCount={3} hasDomain
    {...handlers} {...overrides} />);
  return { ...handlers, user: userEvent.setup() };
}
describe('explicit selection toolbar', () => {
  it('selection count and real batch actions use the same handoff', async () => {
    const { user, onClear, onDeepCollect } = setup();
    const bar = screen.getByRole('group', { name: '批量操作' });
    expect(within(bar).getByRole('status')).toHaveTextContent('已选 3 篇');
    expect(within(bar).getByText(/当前筛选 2\/3/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '评论深采' }));
    await user.click(screen.getByRole('button', { name: '清空已选' }));
    expect(onDeepCollect).toHaveBeenCalledTimes(1);
    expect(onClear).toHaveBeenCalledTimes(1);
  });
  it('no selection disables batch actions', () => {
    setup({ count:0 });
    expect(screen.getByRole('button', { name:'评论深采' })).toBeDisabled();
    expect(screen.getByRole('button', { name:'清空已选' })).toBeDisabled();
  });
  it('unscoped domain never submits a batch', () => {
    setup({ hasDomain:false });
    expect(screen.getByRole('button', { name:'评论深采' })).toBeDisabled();
  });
  it('over 200 selections leaves records intact but disables destructive actions', () => {
    setup({ count:201 });
    expect(screen.getByText('最多 200 篇，请减少勾选')).toBeInTheDocument();
    expect(screen.getByRole('button', { name:'评论深采' })).toBeDisabled();
  });
  it('200 works remains available for comment collection without any obsolete OCR batch entry', () => {
    setup({ count:200 });
    expect(screen.getByRole('button', { name:'评论深采' })).toBeEnabled();
    expect(screen.queryByRole('button', { name:'整理图片文字' })).not.toBeInTheDocument();
  });
});
