import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ToastProvider } from './Toast';
import { useToast, type ToastNotice } from './toastContext';

function Harness() {
  const toast = useToast();
  const fire = (notice: ToastNotice) => toast.notify(notice);
  return (
    <div>
      <button type="button" onClick={() => fire({ key: 'work:reobserve', tone: 'success', text: '复观测已提交，等待执行', context: '合成作品 1' })}>复观测回执</button>
      <button type="button" onClick={() => fire({ key: 'work:comments', tone: 'success', text: '评论采集已提交', context: '合成作品 1' })}>评论回执</button>
      <button type="button" onClick={() => fire({ key: 'dossier:export', tone: 'success', text: '已复制完整底稿', context: '合成底稿' })}>导出回执</button>
    </div>
  );
}

function setup() {
  return render(<ToastProvider><Harness /></ToastProvider>);
}

afterEach(() => {
  vi.useRealTimers();
});

describe('轻通知', () => {
  it('同时最多两条，同一动作的新回执替换上一条而不是叠加', () => {
    setup();
    fireEvent.click(screen.getByRole('button', { name: '复观测回执' }));
    fireEvent.click(screen.getByRole('button', { name: '评论回执' }));
    fireEvent.click(screen.getByRole('button', { name: '导出回执' }));
    const region = screen.getByRole('region', { name: '操作通知' });
    expect(within(region).getAllByRole('status')).toHaveLength(2);
    expect(within(region).queryByText('复观测已提交，等待执行')).toBeNull();
    expect(within(region).getByText('评论采集已提交')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '复观测回执' }));
    expect(within(region).getAllByRole('status')).toHaveLength(2);
    expect(within(region).getAllByText('复观测已提交，等待执行')).toHaveLength(1);
  });

  it('约五秒后自动关闭；悬停或键盘停留暂停计时', () => {
    vi.useFakeTimers();
    setup();
    fireEvent.click(screen.getByRole('button', { name: '复观测回执' }));
    const toast = screen.getByText('复观测已提交，等待执行').closest('[role="status"]') as HTMLElement;

    act(() => { vi.advanceTimersByTime(3000); });
    fireEvent.pointerEnter(toast);
    act(() => { vi.advanceTimersByTime(10000); });
    expect(screen.getByText('复观测已提交，等待执行')).toBeInTheDocument();

    fireEvent.pointerLeave(toast);
    act(() => { vi.advanceTimersByTime(1999); });
    expect(screen.getByText('复观测已提交，等待执行')).toBeInTheDocument();
    act(() => { vi.advanceTimersByTime(1); });
    expect(screen.queryByText('复观测已提交，等待执行')).toBeNull();

    fireEvent.click(screen.getByRole('button', { name: '评论回执' }));
    const second = screen.getByText('评论采集已提交').closest('[role="status"]') as HTMLElement;
    const close = within(second).getByRole('button', { name: '关闭通知' });
    act(() => { close.focus(); });
    expect(close).toHaveFocus();
    act(() => { vi.advanceTimersByTime(9000); });
    expect(screen.getByText('评论采集已提交')).toBeInTheDocument();
  });

  it('关闭按钮立即移除；每条回执带作品上下文，不抢焦点', () => {
    setup();
    const trigger = screen.getByRole('button', { name: '复观测回执' });
    trigger.focus();
    fireEvent.click(trigger);
    expect(trigger).toHaveFocus();
    const toast = screen.getByText('复观测已提交，等待执行').closest('[role="status"]') as HTMLElement;
    expect(within(toast).getByText('合成作品 1')).toBeInTheDocument();
    fireEvent.click(within(toast).getByRole('button', { name: '关闭通知' }));
    expect(screen.queryByText('复观测已提交，等待执行')).toBeNull();
  });

  it('原生 dialog 打开时，通知宿主进入顶层对话框，不被遮罩挡住', async () => {
    render(
      <ToastProvider>
        <dialog open aria-label="合成确认框">
          <Harness />
        </dialog>
      </ToastProvider>,
    );
    const dialog = screen.getByRole('dialog', { name: '合成确认框' });
    fireEvent.click(screen.getByRole('button', { name: '评论回执' }));
    await waitFor(() => expect(within(dialog).getByRole('region', { name: '操作通知' })).toBeInTheDocument());
    expect(document.body.querySelector('body > div > [role="region"]')).toBeNull();
  });
});
