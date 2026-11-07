import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ContextBar, ContextBreadcrumb } from './ContextBar';

describe('共享面包屑渲染', () => {
  it('每段之间用 / 分隔，最后一段是当前页：加粗、手绘线、aria-current', () => {
    const { container } = render(
      <ContextBar breadcrumb={<ContextBreadcrumb items={['语料', '创作者']} />} meta={null} />,
    );
    const crumb = container.querySelector('.v7-context-crumb') as HTMLElement;
    expect(crumb.querySelectorAll('.v7-slash')).toHaveLength(1);

    const current = crumb.querySelector('b') as HTMLElement;
    expect(current).toHaveTextContent('创作者');
    expect(current).toHaveClass('v7-annotated-current');
    expect(current).toHaveAttribute('aria-current', 'page');
    expect(current.querySelector('svg path')).not.toBeNull();
  });

  it('中间段可以是任意节点（领域选择器），不会被额外包裹挡住交互', () => {
    render(
      <ContextBar
        breadcrumb={
          <ContextBreadcrumb
            items={['运营', <button key="scope" type="button">全部领域</button>, '数据']}
          />
        }
        meta={null}
      />,
    );
    expect(screen.getByRole('button', { name: '全部领域' })).toBeVisible();
    expect(screen.getByRole('navigation', { name: '当前位置' })).toHaveTextContent('运营/全部领域/数据');
  });

  it('只有一段时没有分隔符，这一段就是当前页', () => {
    const { container } = render(<ContextBreadcrumb items={['页面不存在']} />);
    expect(container.querySelector('.v7-slash')).toBeNull();
    expect(container.querySelector('b')).toHaveAttribute('aria-current', 'page');
  });
  it('supports a scoped light creator preview while keeping default dark texture', () => {
    const { container } = render(<ContextBar breadcrumb={<span>预览</span>} tone="light" />);
    expect(container.querySelector('[data-context-texture="light"]')).not.toBeNull();
    expect(container.querySelector('[data-context-pattern]')).toBeNull();
    const defaultView = render(<ContextBar breadcrumb={<span>正式页面</span>} />);
    expect(defaultView.container.querySelector('[data-context-texture="dark"]')).not.toBeNull();
  });

});
