import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { EmptyState } from './EmptyState';

it('空态说清缺什么、为什么空和可用的下一步', () => {
  render(
    <EmptyState kind="no_material" title="当前范围没有可读材料" action={<button type="button">切换范围</button>}>
      当前筛选下没有文章；这不表示平台上没有内容。
    </EmptyState>,
  );
  const region = screen.getByText('当前范围没有可读材料').closest('section');
  expect(region).toHaveAttribute('data-lids-component', 'C42');
  expect(region).toHaveAttribute('data-empty-kind', 'no_material');
  expect(screen.getByText('当前筛选下没有文章；这不表示平台上没有内容。')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '切换范围' })).toBeInTheDocument();
});

it('没有下一步时不渲染动作区', () => {
  render(<EmptyState title="当前条件没有结果" />);
  const region = screen.getByText('当前条件没有结果').closest('section');
  expect(region).toHaveAttribute('data-empty-kind', 'no_match');
  expect(region?.querySelector('button')).toBeNull();
  expect(region?.querySelectorAll('p').length).toBe(0);
});
