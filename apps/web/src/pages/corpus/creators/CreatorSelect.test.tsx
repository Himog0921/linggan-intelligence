import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { CreatorSelect } from './CreatorSelect';

const options = [{ value: 'all', label: '全部' }, { value: 'blocked', label: '不可用', disabled: true }, { value: 'active', label: '待处理' }];
function Example({ change = () => undefined }: { change?: (value: string) => void }) {
  const [value, setValue] = useState('all');
  return <><CreatorSelect label="测试范围" value={value} options={options} onChange={next => { setValue(next); change(next); }} /><button>外部按钮</button></>;
}

describe('创作者选择适配器 · 真实选择与键盘语义', () => {
  it('选项只在展开后可见，选择值改变才通知调用者，回到原触发器', async () => {
    const user = userEvent.setup(); const change = vi.fn();
    render(<Example change={change} />);
    const trigger = screen.getByRole('combobox', { name: '测试范围' });
    expect(screen.queryByRole('listbox')).toBeNull();
    await user.click(trigger);
    expect(screen.getByRole('option', { name: '全部' })).toHaveAttribute('aria-selected', 'true');
    await user.click(screen.getByRole('option', { name: '待处理' }));
    expect(change).toHaveBeenCalledExactlyOnceWith('active');
    expect(trigger).toHaveTextContent('待处理'); expect(trigger).toHaveFocus();
    await user.click(trigger); await user.click(screen.getByRole('option', { name: '待处理' }));
    expect(change).toHaveBeenCalledTimes(1);
  });

  it('方向键跳过不可用项，Home/End定位，Enter才提交', async () => {
    const user = userEvent.setup(); const change = vi.fn(); render(<Example change={change} />);
    const trigger = screen.getByRole('combobox');
    await user.tab(); await user.keyboard('{ArrowDown}{ArrowDown}');
    expect(trigger).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: '待处理' }).id);
    expect(change).not.toHaveBeenCalled();
    await user.keyboard('{Home}');
    expect(trigger).toHaveAttribute('aria-activedescendant', screen.getByRole('option', { name: '全部' }).id);
    await user.keyboard('{End}{Enter}');
    expect(change).toHaveBeenCalledExactlyOnceWith('active'); expect(trigger).toHaveFocus();
  });

  it('Escape只收起菜单，保留原值；Tab正常移向下一控件', async () => {
    const user = userEvent.setup(); render(<Example />);
    const trigger = screen.getByRole('combobox');
    await user.click(trigger); await user.keyboard('{ArrowDown}{Escape}');
    expect(trigger).toHaveAttribute('aria-expanded', 'false'); expect(trigger).toHaveTextContent('全部');
    await user.click(trigger); await user.tab();
    expect(screen.getByRole('button', { name: '外部按钮' })).toHaveFocus();
    expect(screen.queryByRole('listbox')).toBeNull();
  });

  it('外部点击收起而不把焦点抢回；不可用项不提交', async () => {
    const user = userEvent.setup(); const change = vi.fn(); render(<Example change={change} />);
    await user.click(screen.getByRole('combobox'));
    await user.click(screen.getByRole('option', { name: '不可用' }));
    expect(change).not.toHaveBeenCalled(); expect(screen.getByRole('listbox')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: '外部按钮' }));
    expect(screen.queryByRole('listbox')).toBeNull(); expect(screen.getByRole('button')).toHaveFocus();
  });

  it('输入前缀能定位，滚动菜单内部不关闭，外部滚动收起', async () => {
    const user = userEvent.setup(); const change = vi.fn(); render(<Example change={change} />);
    await user.tab(); await user.keyboard('待');
    const menu = screen.getByRole('listbox'); fireEvent.scroll(menu);
    expect(menu).toBeInTheDocument();
    await user.keyboard('{Enter}'); expect(change).toHaveBeenCalledWith('active');
    await user.click(screen.getByRole('combobox')); fireEvent.scroll(document);
    expect(screen.queryByRole('listbox')).toBeNull();
  });

  it('整个控件不可用时没有选择入口', async () => {
    const change = vi.fn(); render(<CreatorSelect label="不可用范围" value="all" options={options} onChange={change} disabled />);
    const trigger = screen.getByRole('combobox'); expect(trigger).toBeDisabled();
    await userEvent.setup().click(trigger); expect(screen.queryByRole('listbox')).toBeNull(); expect(change).not.toHaveBeenCalled();
  });
});
