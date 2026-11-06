import { fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, expect, it, vi } from 'vitest';
import { SelectField } from './SelectField';

afterEach(() => { vi.restoreAllMocks(); Reflect.deleteProperty(HTMLElement.prototype, 'scrollIntoView'); });

it('长菜单定位只滚动菜单自身，末项点击仍提交，外部滚动仍关闭', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  const ancestorScroll = vi.fn();
  Object.defineProperty(HTMLElement.prototype, 'scrollIntoView', { configurable: true, value: ancestorScroll });
  const options = Array.from({ length: 13 }, (_, index) => ({ value: String(index), label: `状态 ${index}` }));
  render(<SelectField label="通道状态" value="0" options={options} onChange={change} />);
  const trigger = screen.getByRole('combobox');
  await user.click(trigger);
  const popup = screen.getByRole('listbox');
  vi.spyOn(popup, 'getBoundingClientRect').mockReturnValue({ top: 100, bottom: 300 } as DOMRect);
  Object.defineProperty(popup, 'clientHeight', { configurable: true, value: 200 });
  const last = screen.getByRole('option', { name: '状态 12' });
  vi.spyOn(last, 'getBoundingClientRect').mockReturnValue({ top: 580, bottom: 620 } as DOMRect);
  await user.keyboard('{End}');
  expect(popup.scrollTop).toBe(320);
  expect(ancestorScroll).not.toHaveBeenCalled();
  fireEvent.scroll(popup);
  expect(screen.getByRole('listbox')).toBe(popup);
  const preview = document.createElement('div');
  document.body.append(preview);
  fireEvent.scroll(preview);
  expect(screen.getByRole('listbox')).toBe(popup);
  preview.remove();
  await user.click(last);
  expect(change).toHaveBeenCalledExactlyOnceWith('12');
  expect(trigger).toHaveFocus();
  await user.click(trigger);
  fireEvent.scroll(trigger.parentElement!);
  expect(screen.queryByRole('listbox')).toBeNull();
  await user.click(trigger);
  fireEvent.scroll(document);
  expect(screen.queryByRole('listbox')).toBeNull();
});


it('首次打开时在菜单高度收紧后仍能看见当前末项', async () => {
  const user = userEvent.setup();
  vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(function (this: Element) {
    return this instanceof HTMLElement && this.getAttribute('role') === 'listbox' ? Number.parseFloat(this.style.maxHeight) || 520 : 0;
  });
  vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
    if (this.getAttribute('role') === 'combobox') return new DOMRect(100, 300, 200, 40);
    if (this.getAttribute('role') === 'listbox') return new DOMRect(100, 340, 200, this.clientHeight);
    if (this.getAttribute('role') === 'option' && this.parentElement) {
      const index = [...this.parentElement.children].indexOf(this);
      return new DOMRect(100, 340 + index * 40 - this.parentElement.scrollTop, 200, 40);
    }
    return new DOMRect();
  });
  const options = Array.from({ length: 13 }, (_, index) => ({ value: String(index), label: `状态 ${index}` }));
  render(<SelectField label="通道状态" value="12" options={options} onChange={() => undefined} />);
  await user.click(screen.getByRole('combobox'));
  const popup = screen.getByRole('listbox');
  const selected = screen.getByRole('option', { name: '状态 12' });
  expect(popup.clientHeight).toBeLessThan(520);
  expect(popup.scrollTop).toBeGreaterThan(0);
  expect(selected.getBoundingClientRect().bottom).toBeLessThanOrEqual(popup.getBoundingClientRect().bottom);
});

it('键盘可开合与选择，跳过不可用选项，Escape 收起但保留当前值', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  const options = [
    { value: 'recent', label: '最近收录' },
    { value: 'blocked', label: '暂不可选', disabled: true },
    { value: 'relevant', label: '相关作品数' },
  ];
  render(<SelectField label="当前排序" value="recent" options={options} onChange={change} />);
  const trigger = screen.getByRole('combobox');
  expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await user.tab();
  expect(trigger).toHaveFocus();
  await user.keyboard('{ArrowDown}');
  expect(trigger).toHaveAttribute('aria-expanded', 'true');
  expect(screen.getByRole('listbox')).toBeInTheDocument();
  await user.keyboard('{ArrowDown}');
  const active = trigger.getAttribute('aria-activedescendant');
  expect(screen.getByRole('option', { name: '相关作品数' })).toHaveAttribute('id', active ?? '');
  await user.keyboard('{Enter}');
  expect(change).toHaveBeenCalledExactlyOnceWith('relevant');
  expect(trigger).toHaveAttribute('aria-expanded', 'false');
  expect(trigger).toHaveFocus();
});

it('Escape 收起菜单且不提交，焦点回到触发器', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(<SelectField label="当前排序" value="recent" options={[{ value: 'recent', label: '最近收录' }, { value: 'relevant', label: '相关作品数' }]} onChange={change} />);
  const trigger = screen.getByRole('combobox');
  await user.click(trigger);
  expect(screen.getByRole('listbox')).toBeInTheDocument();
  await user.keyboard('{Escape}');
  expect(screen.queryByRole('listbox')).toBeNull();
  expect(change).not.toHaveBeenCalled();
  expect(trigger).toHaveFocus();
});

it('不可用字段不打开菜单', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(<SelectField disabled label="当前排序" value="recent" options={[{ value: 'recent', label: '最近收录' }]} onChange={change} />);
  const trigger = screen.getByRole('combobox');
  expect(trigger).toBeDisabled();
  await user.click(trigger);
  await user.keyboard('{ArrowDown}');
  expect(screen.queryByRole('listbox')).toBeNull();
  expect(change).not.toHaveBeenCalled();
});

it('紧凑形态只改尺寸，仍以 combobox 暴露当前值', () => {
  render(<SelectField compact label="演示范围" value="author" options={[{ value: 'author', label: '创作者资料' }, { value: 'work', label: '作品内容' }]} onChange={() => undefined} />);
  const trigger = screen.getByRole('combobox', { name: '演示范围' });
  expect(trigger).toHaveTextContent('创作者资料');
  expect(trigger.parentElement).toHaveAttribute('data-compact');
});
