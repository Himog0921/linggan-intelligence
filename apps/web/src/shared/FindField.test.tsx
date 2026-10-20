import { act, fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FindField, type FindFieldHandle } from './FindField';

afterEach(() => vi.useRealTimers());
describe('C16 shared query contract', () => {
  it('manual submission keeps draft local, repeats explicit reads, and blocks IME Enter', () => {
    const commit=vi.fn();render(<FindField label="材料检索" value="" trigger="submit" onCommit={commit} />);
    const input=screen.getByRole('searchbox');fireEvent.change(input,{target:{value:'新词'}});expect(commit).not.toHaveBeenCalled();
    fireEvent.compositionStart(input);fireEvent.submit(input.closest('form')!);expect(commit).not.toHaveBeenCalled();
    fireEvent.compositionEnd(input);fireEvent.submit(input.closest('form')!);fireEvent.submit(input.closest('form')!);
    expect(commit.mock.calls).toEqual([['新词'],['新词']]);
  });
  it('external query/scope changes invalidate pending draft and unmount cancels work', () => {
    vi.useFakeTimers();const commit=vi.fn();const view=render(<FindField label="检索" value="旧词" trigger="debounce_250" onCommit={commit}/>);
    fireEvent.change(screen.getByRole('searchbox'),{target:{value:'过期词'}});
    view.rerender(<FindField label="检索" value="恢复词" contextKey="other" trigger="debounce_250" onCommit={commit}/>);
    act(()=>vi.advanceTimersByTime(300));expect(commit).not.toHaveBeenCalled();expect(screen.getByRole('searchbox')).toHaveValue('恢复词');
    fireEvent.change(screen.getByRole('searchbox'),{target:{value:'卸载词'}});view.unmount();act(()=>vi.advanceTimersByTime(300));expect(commit).not.toHaveBeenCalled();
  });
  it('busy cannot commit or clear; flush submits once and returns input focus', () => {
    vi.useFakeTimers();const commit=vi.fn(),handle=createRef<FindFieldHandle>();const view=render(<FindField label="检索" value="" trigger="debounce_250" onCommit={commit} handle={handle}/>);
    fireEvent.change(screen.getByRole('searchbox'),{target:{value:' 待发 '}});act(()=>handle.current?.flush());act(()=>vi.advanceTimersByTime(300));expect(commit.mock.calls).toEqual([['待发']]);
    view.rerender(<FindField label="检索" value="待发" trigger="debounce_250" onCommit={commit} busy/>);fireEvent.submit(screen.getByRole('searchbox').closest('form')!);expect(commit).toHaveBeenCalledTimes(1);expect(screen.getByRole('button',{name:'清空搜索'})).toBeDisabled();
  });
  it('commits exactly at 250ms, preserves untrimmed drafts, and clears immediately during debounce', () => {
    vi.useFakeTimers(); const commit = vi.fn();
    render(<FindField label="创作检索" value="" trigger="debounce_250" trim={false} onCommit={commit}/>);
    const input = screen.getByRole('searchbox');
    fireEvent.compositionStart(input); fireEvent.change(input, { target: { value: ' 中文 ' } });
    act(() => vi.advanceTimersByTime(500)); expect(commit).not.toHaveBeenCalled();
    fireEvent.compositionEnd(input); act(() => vi.advanceTimersByTime(249)); expect(commit).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1)); expect(commit.mock.calls).toEqual([[' 中文 ']]);
    fireEvent.change(input, { target: { value: '后续词' } });
    fireEvent.click(screen.getByRole('button', { name: '清空搜索' }));
    act(() => vi.advanceTimersByTime(300)); expect(commit.mock.calls).toEqual([[' 中文 '], ['']]); expect(input).toHaveFocus();
  });
  it('supports a submit button associated from the page toolbar', () => {
    const commit=vi.fn();render(<><FindField label="检索" formId="shared-find-proof" value="同一词" trigger="submit" onCommit={commit}/><button type="submit" form="shared-find-proof">读取材料</button></>);
    fireEvent.click(screen.getByRole('button',{name:'读取材料'}));expect(commit).toHaveBeenCalledWith('同一词');
  });
  it('slash focuses only from non-editable background', () => {
    render(<><FindField label="检索" value="" trigger="submit" onCommit={()=>undefined} shortcut/><input aria-label="其他输入"/></>);
    const other=screen.getByRole('textbox',{name:'其他输入'});other.focus();fireEvent.keyDown(other,{key:'/'});expect(other).toHaveFocus();
    fireEvent.keyDown(document.body,{key:'/'});expect(screen.getByRole('searchbox')).toHaveFocus();
  });
});
