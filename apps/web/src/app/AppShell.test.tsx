import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AppShell } from './AppShell';

const props = { active: 'ops' as const, boundaryLabel: '本机创作', crumb: '运营 / 研读取材', meta: '个人空间' };

describe('V3 局部创作空间', () => {
  it('紧凑页头保留导航且 Esc 返回入口，不堆叠全局上下文栏', () => {
    render(<AppShell {...props} workbenchMode="compact"><main>阅读区</main></AppShell>);
    expect(screen.queryByText('领域情报工作台')).not.toBeInTheDocument();
    const summary = screen.getByText('intelligence');
    fireEvent.click(summary);
    expect(screen.getByRole('link', { name: '语料' })).toHaveAttribute('href', '/corpus');
    fireEvent.keyDown(summary, { key: 'Escape' });
    expect(summary.closest('details')).not.toHaveAttribute('open');
    expect(summary).toHaveFocus();
  });
  it('沉浸模式只给当前工作区空间', () => {
    render(<AppShell {...props} workbenchMode="immersive"><main><button>退出沉浸</button></main></AppShell>);
    expect(screen.queryByRole('banner')).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '退出沉浸' })).toBeVisible();
  });
  it('设置等其他页面继续使用原有完整页头', () => {
    render(<AppShell {...props}><main>设置</main></AppShell>);
    expect(screen.getByText('领域情报工作台')).toBeVisible();
    expect(screen.getByText('个人空间')).toBeVisible();
  });
});
