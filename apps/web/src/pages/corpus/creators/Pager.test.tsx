import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { Pager } from './Pager';

function setup(page: number, total: number | null, pageSize = 25) {
  const onPage = vi.fn();
  const onPageSize = vi.fn();
  const view = render(<Pager page={page} pageSize={pageSize} total={total} onPage={onPage} onPageSize={onPageSize} />);
  return { onPage, onPageSize, ...view };
}
const jumpInput = () => screen.getByRole('spinbutton', { name: '跳转页码' }) as HTMLInputElement;

describe('分页', () => {
  it('显示「x / y 页」，首页不能上一页，末页不能下一页', () => {
    const { rerender, onPage, onPageSize } = setup(0, 60);
    expect(screen.getByText('1 / 3 页')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '上一页' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '下一页' })).toBeEnabled();
    rerender(<Pager page={2} pageSize={25} total={60} onPage={onPage} onPageSize={onPageSize} />);
    expect(screen.getByText('3 / 3 页')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled();
  });

  it('零命中是「0 页」，不是「1 页」；所有翻页控件不可用', () => {
    setup(0, 0);
    expect(screen.getByText('0 页')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '上一页' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled();
    expect(jumpInput()).toBeDisabled();
    expect(screen.getByRole('button', { name: '跳转' })).toBeDisabled();
  });

  it('还没读到总数（加载中、失败）：整套控件不可用，不产生旧结果跳转', () => {
    setup(0, null);
    expect(screen.getByRole('button', { name: '下一页' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '上一页' })).toBeDisabled();
    expect(jumpInput()).toBeDisabled();
  });

  it('上一页 / 下一页交出从 0 起的页码', () => {
    const { onPage } = setup(1, 100);
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    expect(onPage.mock.calls).toEqual([[2], [0]]);
  });

  it('跳转：输入 1 起始的页码；超界就地提示，并保持当前页', () => {
    const { onPage } = setup(0, 100);
    fireEvent.change(jumpInput(), { target: { value: '9' } });
    fireEvent.click(screen.getByRole('button', { name: '跳转' }));
    expect(onPage).not.toHaveBeenCalled();
    expect(screen.getByText('请输入 1 至 4 页。')).toBeInTheDocument();
    expect(jumpInput()).toHaveAttribute('aria-invalid', 'true');
    fireEvent.change(jumpInput(), { target: { value: '3' } });
    fireEvent.click(screen.getByRole('button', { name: '跳转' }));
    expect(onPage).toHaveBeenCalledWith(2);
    expect(screen.queryByText('请输入 1 至 4 页。')).toBeNull();
  });

  it('0、负数、小数、空都不是有效页码', () => {
    const { onPage } = setup(0, 100);
    for (const value of ['0', '-1', '1.5', '']) {
      fireEvent.change(jumpInput(), { target: { value } });
      fireEvent.click(screen.getByRole('button', { name: '跳转' }));
    }
    // 空输入时输入框回显当前页（1），属于有效页码，所以只允许最后那次真正跳转
    expect(onPage.mock.calls.every(call => call[0] === 0)).toBe(true);
  });

  it('页码变了（翻页、后退），旧的越界提示作废', () => {
    const { rerender, onPage, onPageSize } = setup(0, 100);
    fireEvent.change(jumpInput(), { target: { value: '9' } });
    fireEvent.click(screen.getByRole('button', { name: '跳转' }));
    expect(screen.getByText('请输入 1 至 4 页。')).toBeInTheDocument();
    rerender(<Pager page={1} pageSize={25} total={100} onPage={onPage} onPageSize={onPageSize} />);
    expect(screen.queryByText('请输入 1 至 4 页。')).toBeNull();
    expect(jumpInput().value).toBe('2');
  });

  it('每页数量只有 25 / 50 / 100', () => {
    const { onPageSize } = setup(0, 100, 50);
    const select = screen.getByRole('combobox');
    expect(select).toHaveTextContent('50');
    fireEvent.click(select);
    expect(screen.getAllByRole('option').map(option => option.textContent?.replace('✓', ''))).toEqual(['25', '50', '100']);
    fireEvent.click(screen.getByRole('option', { name: '100' }));
    expect(onPageSize).toHaveBeenCalledWith(100);
  });
});
