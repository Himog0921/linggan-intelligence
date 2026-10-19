import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { AppShell } from './AppShell';

function states(): Record<string, string> {
  const nav = screen.getByRole('navigation', { name: '一级导航' });
  const result: Record<string, string> = {};
  for (const entry of within(nav).getAllByRole('link', { hidden: true }).concat(within(nav).queryAllByRole('button', { hidden: true }))) {
    result[entry.querySelector('.v7-nav-zh')?.textContent ?? ''] = entry.querySelector('.v7-nav-state')?.textContent ?? '';
  }
  return result;
}

describe('AppShell 一级导航状态词', () => {
  it('默认沿用迁移期基线；采集项在读不到事实时保持「状态未知」，不宣布已接通或为空', () => {
    render(<AppShell active="corpus" boundaryLabel="本机服务状态" crumb="语料" meta={null}>x</AppShell>);
    expect(states()).toMatchObject({ 语料: '未知', 采集: '状态未知', 运营: '只读' });
  });

  it('collectionState 只改采集项，不论当前在哪一页（与 shell.rs 的 collection_state 等价）', () => {
    render(<AppShell active="corpus" boundaryLabel="本机服务状态" collectionState="观察中" crumb="语料" meta={null}>x</AppShell>);
    expect(states()).toMatchObject({ 语料: '未知', 采集: '观察中', 主题图谱: '暂定研究' });
  });

  it('无观察目标与状态未知是两个不同的事实，原样显示', () => {
    const { unmount } = render(<AppShell active={null} boundaryLabel="本机服务状态" collectionState="无观察目标" crumb="设置" meta={null}>x</AppShell>);
    expect(states().采集).toBe('无观察目标');
    unmount();
    render(<AppShell active={null} boundaryLabel="本机服务状态" collectionState="状态未知" crumb="设置" meta={null}>x</AppShell>);
    expect(states().采集).toBe('状态未知');
  });

  it('activeState 仍然只作用于当前项（设置页与内容工作台的既有行为不变）', () => {
    render(<AppShell active="ops" activeState="创作" boundaryLabel="本机创作" crumb="运营" meta={null}>x</AppShell>);
    expect(states()).toMatchObject({ 运营: '创作', 采集: '状态未知', 语料: '未知' });
  });

  it('采集页自己给了当前状态时，采集的真实状态词优先（同一项只显示一个词）', () => {
    render(<AppShell active="collection" activeState="巡检中" collectionState="观察中" boundaryLabel="本机服务状态" crumb="采集" meta={null}>x</AppShell>);
    expect(states().采集).toBe('观察中');
  });
});

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

describe('全站唯一React导航', () => {
  it('入口只指向职责根路由；未开放职责保持禁用，个人菜单保留设置入口', () => {
    render(<AppShell active="corpus" boundaryLabel="本机服务状态" crumb="语料" meta={null}>页面</AppShell>);
    const navigation = screen.getByRole('navigation', {name:'一级导航'});
    const links = within(navigation).getAllByRole('link');
    expect(links.map(link => link.getAttribute('href'))).toEqual(['/topics','/corpus','/collection','/ops']);
    expect(within(navigation).getByRole('button',{name:/雷达/})).toBeDisabled();
    expect(within(navigation).getByRole('button',{name:/洞察/})).toBeDisabled();
    expect(screen.getByRole('link',{name:'设置'})).toHaveAttribute('href','/settings');
  });
});
