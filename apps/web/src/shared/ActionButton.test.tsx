import { fireEvent, render, screen } from '@testing-library/react';
import { createRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { ActionButton } from './ActionButton';

describe('V8 ActionButton request and focus boundaries', () => {
  it.each(['primary','ink','outline','soft','danger','quiet'] as const)('%s preserves its action label and native keyboard focus', variant => {
    const click=vi.fn(),ref=createRef<HTMLButtonElement>();
    render(<ActionButton ref={ref} variant={variant} onClick={click}>保存作品</ActionButton>);
    const button=screen.getByRole('button',{name:'保存作品'});
    expect(button).toHaveAttribute('type','button');expect(button).toHaveAttribute('data-variant',variant);
    ref.current?.focus();expect(button).toHaveFocus();fireEvent.click(button);expect(click).toHaveBeenCalledTimes(1);
  });
  it('keeps disabled actions inert and unable to steal keyboard focus', () => {
    const click=vi.fn();render(<><input aria-label="正文"/><ActionButton variant="primary" disabled onClick={click}>生成图文包</ActionButton></>);
    const input=screen.getByRole('textbox',{name:'正文'}),button=screen.getByRole('button',{name:'生成图文包'});
    input.focus();button.focus();fireEvent.click(button);expect(button).toBeDisabled();expect(input).toHaveFocus();expect(click).not.toHaveBeenCalled();
  });
  it('announces one busy action, hides the four decorative cells, and prevents repeated activation', () => {
    const click=vi.fn();const view=render(<ActionButton busy onClick={click}>保存作品</ActionButton>);
    const button=screen.getByRole('button',{name:'保存作品'});expect(button).toBeDisabled();expect(button).toHaveAttribute('aria-busy','true');expect(button).toHaveAttribute('data-loading','');
    expect(button.querySelector('[aria-hidden="true"]')?.children.length).toBe(4);fireEvent.click(button);expect(click).not.toHaveBeenCalled();
    view.rerender(<ActionButton busy={false} onClick={click}>保存作品</ActionButton>);expect(button).toBeEnabled();fireEvent.click(button);expect(click).toHaveBeenCalledTimes(1);
  });
  it('supports the explicit loading state and form submission only when requested', () => {
    const view=render(<ActionButton variant="loading">处理中</ActionButton>);expect(screen.getByRole('button',{name:'处理中'})).toBeDisabled();
    view.rerender(<ActionButton type="submit" aria-label="提交作品">保存</ActionButton>);expect(screen.getByRole('button',{name:'提交作品'})).toHaveAttribute('type','submit');
  });
});
