import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createRef } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SearchField, type SearchFieldHandle } from './SearchField';

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  vi.useRealTimers();
});

function setup(value = '', mode: 'author' | 'work' = 'author') {
  const onCommit = vi.fn();
  const onModeChange = vi.fn();
  const handle = createRef<SearchFieldHandle>();
  const view = render(<SearchField mode={mode} value={value} onCommit={onCommit} onModeChange={onModeChange} handle={handle} />);
  return { onCommit, onModeChange, handle, ...view };
}
const input = () => screen.getByRole('searchbox', { name: '搜索创作者或作品' }) as HTMLInputElement;

describe('搜索框', () => {
  it('停止输入 250ms 才提交，连续输入只提交最后一次', () => {
    const { onCommit } = setup();
    fireEvent.change(input(), { target: { value: '开' } });
    act(() => vi.advanceTimersByTime(200));
    fireEvent.change(input(), { target: { value: '开始' } });
    act(() => vi.advanceTimersByTime(200));
    expect(onCommit).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(60));
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('开始');
  });

  it('中文输入法组词期间不发送查询，组词结束才提交', () => {
    const { onCommit } = setup();
    fireEvent.compositionStart(input());
    fireEvent.change(input(), { target: { value: 'kai' } });
    fireEvent.change(input(), { target: { value: 'kaishi' } });
    act(() => vi.advanceTimersByTime(1000));
    expect(onCommit).not.toHaveBeenCalled();
    fireEvent.change(input(), { target: { value: '开始' } });
    fireEvent.compositionEnd(input(), { target: { value: '开始' } });
    act(() => vi.advanceTimersByTime(300));
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(onCommit).toHaveBeenCalledWith('开始');
  });

  it('组词期间按回车不提交', () => {
    const { onCommit } = setup();
    fireEvent.compositionStart(input());
    fireEvent.change(input(), { target: { value: 'kai' } });
    fireEvent.submit(input().closest('form') as HTMLFormElement);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('回车立即提交（去掉首尾空白）；没变化就不重复提交', () => {
    const { onCommit } = setup('旧词');
    fireEvent.change(input(), { target: { value: '  新词 ' } });
    fireEvent.submit(input().closest('form') as HTMLFormElement);
    expect(onCommit).toHaveBeenCalledWith('新词');
    onCommit.mockClear();
    fireEvent.change(input(), { target: { value: '旧词' } });
    fireEvent.submit(input().closest('form') as HTMLFormElement);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it('打开筛选之前先提交待发的搜索词（flush）', () => {
    const { onCommit, handle } = setup();
    fireEvent.change(input(), { target: { value: '待发' } });
    act(() => handle.current?.flush());
    expect(onCommit).toHaveBeenCalledWith('待发');
    act(() => vi.advanceTimersByTime(500));
    expect(onCommit).toHaveBeenCalledTimes(1);
  });

  it('地址里的搜索词变了（后退、移除条件）输入框跟着变', () => {
    const { rerender, onCommit, onModeChange, handle } = setup('旧词');
    expect(input().value).toBe('旧词');
    rerender(<SearchField mode="author" value="" onCommit={onCommit} onModeChange={onModeChange} handle={handle} />);
    expect(input().value).toBe('');
  });

  it('有词时显示「清空」，无词时显示快捷键提示；清空立即提交空词', () => {
    const { onCommit } = setup('有词');
    expect(screen.queryByText('/')).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '清空搜索' }));
    expect(onCommit).toHaveBeenCalledWith('');
  });

  it('清空取消尚未到期的查询，不会在250ms后把旧词写回来', () => {
    const { onCommit } = setup();
    fireEvent.change(input(), { target: { value: '尚未提交' } });
    fireEvent.click(screen.getByRole('button', { name: '清空搜索' }));
    act(() => vi.advanceTimersByTime(300));
    expect(onCommit.mock.calls).toEqual([['']]);
    expect(input()).toHaveValue('');
    expect(input()).toHaveFocus();
  });

  it('后退或外部查询改变时取消旧输入定时器，不覆盖已经恢复的URL条件', () => {
    const { onCommit, onModeChange, rerender } = setup('旧词');
    fireEvent.change(input(), { target: { value: '尚未发出' } });
    rerender(<SearchField mode="author" value="返回的查询" onCommit={onCommit} onModeChange={onModeChange} />);
    act(() => vi.advanceTimersByTime(300));
    expect(input()).toHaveValue('返回的查询'); expect(onCommit).not.toHaveBeenCalled();
  });

  it('无词时显示「/」提示', () => {
    setup('');
    expect(screen.getByText('/')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: '清空搜索' })).toBeNull();
  });

  it('搜索范围切换时把当前输入一起交出去；占位符随之变化', () => {
    const { onModeChange, rerender, onCommit, handle } = setup();
    expect(input()).toHaveAttribute('placeholder', '昵称、平台账号、公开简介');
    fireEvent.change(input(), { target: { value: ' 词 ' } });
    fireEvent.click(screen.getByRole('combobox', { name: '搜索范围' }));
    fireEvent.click(screen.getByRole('option', { name: '作品内容' }));
    expect(onModeChange).toHaveBeenCalledWith('work', '词');
    rerender(<SearchField mode="work" value="" onCommit={onCommit} onModeChange={onModeChange} handle={handle} />);
    expect(input()).toHaveAttribute('placeholder', '标题、正文、图文识别或内容方向');
  });
});

describe('搜索框（真实计时器）', () => {
  it('卸载后不再提交', async () => {
    vi.useRealTimers();
    const onCommit = vi.fn();
    const user = userEvent.setup();
    const { unmount } = render(<SearchField mode="author" value="" onCommit={onCommit} onModeChange={() => undefined} />);
    await user.type(input(), '甲');
    unmount();
    await new Promise(resolve => setTimeout(resolve, 320));
    expect(onCommit).not.toHaveBeenCalled();
  });
});
