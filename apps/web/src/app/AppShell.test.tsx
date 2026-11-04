import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppShell } from './AppShell';

function navItems(): HTMLElement[] {
  const nav = screen.getByRole('navigation', { name: '一级导航' });
  return [...nav.querySelectorAll<HTMLElement>(':scope > :is(a,button)')];
}

describe('全站统一深色 Pattern Waves 上下文栏', () => {
  it('普通 AppShell 使用方格材质；保留路径、三级导航和右侧状态的语义节点', () => {
    const { container } = render(
      <AppShell
        active="corpus"
        boundaryLabel="本机服务状态"
        crumb={<><b>评论研究</b> <span className="v7-slash">/</span> ADHD</>}
        contextNav={<nav aria-label="评论研究视图" className="v7-tertiary-nav"><button aria-current="page">概览</button><button>用户评论</button></nav>}
        meta={<span>运行中</span>}
      >内容</AppShell>,
    );
    const bar = container.querySelector('.v7-context-row') as HTMLElement;
    expect(bar).toHaveAttribute('data-context-texture', 'dark');
    expect(bar.querySelector('[data-context-pattern]')).toHaveAttribute('data-context-pattern', 'square');
    expect(bar.querySelector('.v7-context-crumb')).toHaveTextContent('评论研究 / ADHD');
    expect(within(bar).getByRole('navigation', { name: '评论研究视图' })).toBeInTheDocument();
    expect(within(bar).getByText('运行中')).toBeInTheDocument();
  });
});

describe('AppShell 一级导航结构', () => {
  it('只显示中文标题，不再渲染标题下的状态说明行', () => {
    render(<AppShell active="corpus" boundaryLabel="本机服务状态" crumb="语料" meta={null}>x</AppShell>);
    const nav = screen.getByRole('navigation', { name: '一级导航' });
    expect(nav.querySelector('.v7-nav-readout')).toBeNull();
    expect(nav.querySelector('.v7-nav-state')).toBeNull();
    expect(navItems().map(item => item.textContent)).toEqual(['雷达', '主题图谱', '语料', '洞察', '采集', '运营']);
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
  it('只在服务器确认退出后才离开当前工作区；未确认时保留可重试提示', async () => {
    vi.stubGlobal('fetch', vi.fn().mockRejectedValueOnce(new Error('offline')));
    render(<AppShell active="corpus" boundaryLabel="本机服务状态" crumb="语料" meta={null}>页面</AppShell>);
    fireEvent.click(screen.getByText('个人菜单'));
    fireEvent.click(screen.getByRole('button', { name: '退出登录' }));
    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('退出未完成'));
    expect(fetch).toHaveBeenCalledWith('/api/local/auth/logout', expect.objectContaining({ method: 'POST', credentials: 'same-origin' }));
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
