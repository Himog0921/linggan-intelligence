import type { ReactNode } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { CreatorWorkbenchPreviewPage } from './CreatorWorkbenchPreviewPage';

vi.mock('../ops/OpsFrame', () => ({
  OpsFrame: ({ children, tabs, meta, immersive }: { children: ReactNode; tabs: ReactNode; meta: ReactNode; immersive?: boolean }) => (
    <div data-testid="ops-frame" data-immersive={immersive ? 'true' : 'false'}>
      <nav aria-label="预览上下文导航">{tabs}</nav>
      <div>{meta}</div>
      {children}
    </div>
  ),
}));

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function showModal() { this.setAttribute('open', ''); };
  HTMLDialogElement.prototype.close = function close() { this.removeAttribute('open'); this.dispatchEvent(new Event('close')); };
});

describe('CreatorWorkbenchPreviewPage', () => {
  it('renders the three-column writing workspace with synthetic-only notice', () => {
    render(<CreatorWorkbenchPreviewPage />);
    expect(screen.getByRole('main', { name: '创作工作台' })).toBeTruthy();
    expect(screen.getByRole('complementary', { name: '稿件队列' })).toBeTruthy();
    expect(screen.getByRole('complementary', { name: '稿件检查器' })).toBeTruthy();
    expect(screen.getByRole('textbox', { name: '正文第 1 段' })).toBeTruthy();
    expect(screen.getByText('DEMO · 合成数据')).toBeTruthy();
  });

  it('opens the six-type asset dialog and returns focus to its trigger', async () => {
    const user = userEvent.setup();
    render(<CreatorWorkbenchPreviewPage />);
    const trigger = screen.getByRole('button', { name: '资产库' });
    await user.click(trigger);
    const dialog = screen.getByRole('dialog', { name: '创作资产库' });
    expect(dialog).toBeTruthy();
    await user.click(screen.getByRole('button', { name: /经历库/ }));
    expect(screen.getByRole('heading', { name: '一次临时调整计划的经历' })).toBeTruthy();
    await user.click(screen.getByRole('button', { name: '关闭资产库' }));
    expect(screen.queryByRole('dialog', { name: '创作资产库' })).toBeNull();
    expect(document.activeElement).toBe(trigger);
  });

  it('switches to the immersive material desk and returns without replacing the draft', async () => {
    const user = userEvent.setup();
    render(<CreatorWorkbenchPreviewPage />);
    const paragraph = screen.getByRole('textbox', { name: '正文第 1 段' }) as HTMLTextAreaElement;
    await user.clear(paragraph);
    await user.type(paragraph, '本地编辑内容保留');
    await user.click(screen.getByRole('button', { name: '材料桌' }));
    expect(screen.getByRole('region', { name: '沉浸材料桌' })).toBeTruthy();
    expect(screen.getByTestId('ops-frame').getAttribute('data-immersive')).toBe('true');
    await user.click(screen.getByRole('button', { name: '返回写稿' }));
    expect(screen.getByRole('textbox', { name: '正文第 1 段' })).toHaveValue('本地编辑内容保留');
    expect(screen.getByTestId('ops-frame').getAttribute('data-immersive')).toBe('false');
  });

  it('adds a local note on canvas double click without calling a production endpoint', () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch');
    render(<CreatorWorkbenchPreviewPage />);
    fireEvent.click(screen.getByRole('button', { name: '材料桌' }));
    const canvas = screen.getByTestId('canvas-viewport');
    const before = canvas.querySelectorAll('[data-canvas-item]').length;
    const existingCard = canvas.querySelector<HTMLElement>('[data-canvas-item]');
    expect(existingCard).not.toBeNull();
    fireEvent.doubleClick(existingCard!);
    expect(canvas.querySelectorAll('[data-canvas-item]')).toHaveLength(before);
    fireEvent.doubleClick(canvas);
    expect(canvas.querySelectorAll('[data-canvas-item]')).toHaveLength(before + 1);
    expect(fetchSpy).not.toHaveBeenCalled();
    fetchSpy.mockRestore();
  });
  it('does not steal initial focus and supports keyboard tab navigation', async () => {
    const user = userEvent.setup();
    render(<CreatorWorkbenchPreviewPage />);
    expect(document.activeElement).not.toBe(screen.getByRole('button', { name: '资产库' }));
    const first = screen.getByRole('tab', { name: /检查/ });
    first.focus();
    await user.keyboard('{ArrowRight}');
    const materials = screen.getByRole('tab', { name: '素材' });
    expect(materials).toHaveAttribute('aria-selected', 'true');
    expect(document.activeElement).toBe(materials);
    await user.keyboard('{End}');
    expect(screen.getByRole('tab', { name: '预览' })).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{Home}');
    expect(first).toHaveAttribute('aria-selected', 'true');
  });

  it('keeps locally added canvas item IDs unique after delete and re-add', async () => {
    const user = userEvent.setup();
    render(<CreatorWorkbenchPreviewPage />);
    await user.click(screen.getByRole('button', { name: '材料桌' }));
    const canvas = screen.getByTestId('canvas-viewport');
    fireEvent.doubleClick(canvas);
    await user.click(screen.getByRole('button', { name: '移出画布' }));
    fireEvent.doubleClick(canvas);
    fireEvent.doubleClick(canvas);
    const ids = [...canvas.querySelectorAll<HTMLElement>('[data-canvas-item]')].map(item => item.dataset.canvasItem);
    expect(new Set(ids).size).toBe(ids.length);
  });

});
