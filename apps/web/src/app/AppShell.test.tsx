import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
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

describe('共享页头高度测量（固定抽屉的起点）', () => {
  const root = document.documentElement;
  const variable = () => root.style.getPropertyValue('--v7-header-height');
  let headerBottom = 56;
  let nextFrame = 1;
  const frames = new Map<number, FrameRequestCallback>();
  let fontsReady: () => void = () => {};

  const flushFrames = () => {
    const pending = [...frames.values()];
    frames.clear();
    pending.forEach(callback => callback(0));
  };
  const settle = async () => {
    await Promise.resolve();
    await Promise.resolve();
  };

  beforeEach(() => {
    headerBottom = 56;
    nextFrame = 1;
    frames.clear();
    root.style.removeProperty('--v7-header-height');
    // 真实浏览器里已脱离文档的元素量出来全是 0；页头还在文档里时量出它当前的下沿。
    vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (this: Element) {
      const bottom = this.isConnected && this.hasAttribute('data-v7-shell-header') ? headerBottom : 0;
      return { x: 0, y: 0, width: 0, height: 0, top: 0, left: 0, right: 0, bottom, toJSON: () => ({}) } as DOMRect;
    });
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback: FrameRequestCallback) => {
      const id = nextFrame++;
      frames.set(id, callback);
      return id;
    });
    vi.spyOn(window, 'cancelAnimationFrame').mockImplementation((id: number) => {
      frames.delete(id);
    });
    const ready = new Promise<void>(resolve => { fontsReady = resolve; });
    Object.defineProperty(document, 'fonts', { configurable: true, value: { ready } });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    Reflect.deleteProperty(document, 'fonts');
    root.style.removeProperty('--v7-header-height');
  });

  it('挂载时同步写入页头实际下沿，卸载时原样还原', () => {
    root.style.setProperty('--v7-header-height', '77px');
    const { unmount } = render(<AppShell {...props}>x</AppShell>);
    expect(variable()).toBe('56px');
    unmount();
    expect(variable()).toBe('77px');
  });

  it('原先没有这个变量时，卸载后移除它而不是留下 0px', () => {
    const { unmount } = render(<AppShell {...props}>x</AppShell>);
    expect(variable()).toBe('56px');
    unmount();
    expect(variable()).toBe('');
  });

  it('窄屏文档滚动使页头移出视口：变量夹到 0，并随可见下沿变化', () => {
    render(<AppShell {...props}>x</AppShell>);
    headerBottom = -40;
    window.dispatchEvent(new Event('scroll'));
    flushFrames();
    expect(variable()).toBe('0px');
    headerBottom = 20;
    window.dispatchEvent(new Event('scroll'));
    flushFrames();
    expect(variable()).toBe('20px');
  });

  it('页头尺寸变化与窗口缩放都会重新量，卸载时断开观察', () => {
    let notify: () => void = () => {};
    const disconnect = vi.fn();
    const observe = vi.fn();
    class FakeResizeObserver {
      constructor(callback: () => void) { notify = callback; }
      observe = observe;
      disconnect = disconnect;
    }
    vi.stubGlobal('ResizeObserver', FakeResizeObserver);
    const { unmount } = render(<AppShell {...props}>x</AppShell>);
    // The measured value is the header's border-box bottom, so the border box is what must be observed.
    expect(observe).toHaveBeenCalledWith(document.querySelector('[data-v7-shell-header]'), { box: 'border-box' });
    headerBottom = 120;
    notify();
    flushFrames();
    expect(variable()).toBe('120px');
    headerBottom = 64;
    window.dispatchEvent(new Event('resize'));
    flushFrames();
    expect(variable()).toBe('64px');
    unmount();
    expect(disconnect).toHaveBeenCalled();
  });

  it('字体就绪后页头换行高度可能变了，要再量一次', async () => {
    render(<AppShell {...props}>x</AppShell>);
    headerBottom = 64;
    fontsReady();
    await settle();
    flushFrames();
    expect(variable()).toBe('64px');
  });

  it('页面卸载后才就绪的字体回调，不会把已还原的变量覆写成 0px', async () => {
    root.style.setProperty('--v7-header-height', '77px');
    const { unmount } = render(<AppShell {...props}>x</AppShell>);
    unmount();
    fontsReady();
    await settle();
    flushFrames();
    expect(variable()).toBe('77px');
  });

  it('切入工作台模式后，旧页面等待中的字体回调同样不能改写变量', async () => {
    root.style.setProperty('--v7-header-height', '77px');
    const { rerender } = render(<AppShell {...props}>x</AppShell>);
    expect(variable()).toBe('56px');
    rerender(<AppShell {...props} workbenchMode="compact">x</AppShell>);
    expect(variable()).toBe('77px');
    fontsReady();
    await settle();
    flushFrames();
    window.dispatchEvent(new Event('scroll'));
    flushFrames();
    expect(variable()).toBe('77px');
  });

  it('工作台模式不测量也不改写这个变量', () => {
    root.style.setProperty('--v7-header-height', '77px');
    render(<AppShell {...props} workbenchMode="compact">x</AppShell>);
    window.dispatchEvent(new Event('scroll'));
    flushFrames();
    expect(variable()).toBe('77px');
  });
});
