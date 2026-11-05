import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { expect, it, vi } from 'vitest';
import { ViewSwitch } from './ViewSwitch';

const options = [
  { key: 'table', label: '表格' },
  { key: 'covers', label: '封面' },
] as const;

it('展示同一集合的视图，选中项用 aria-pressed 表达且带读数', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(<ViewSwitch label="结果排版" value="table" options={[{ ...options[0], count: 128 }, options[1]]} onChange={change} />);
  const group = screen.getByRole('group', { name: '结果排版' });
  expect(group).toHaveAttribute('data-lids-component', 'C26');
  const selected = screen.getByRole('button', { name: /表格/ });
  expect(selected).toHaveAttribute('aria-pressed', 'true');
  expect(selected).toHaveTextContent('128');
  expect(screen.getByRole('button', { name: '封面' })).toHaveAttribute('aria-pressed', 'false');
  await user.click(screen.getByRole('button', { name: '封面' }));
  expect(change).toHaveBeenCalledExactlyOnceWith('covers');
});

it('点击当前项不产生重复切换', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(<ViewSwitch label="结果排版" value="table" options={options} onChange={change} />);
  await user.click(screen.getByRole('button', { name: '表格' }));
  expect(change).not.toHaveBeenCalled();
});

it('不可用视图不可点击并给出原因', async () => {
  const user = userEvent.setup();
  const change = vi.fn();
  render(<ViewSwitch label="结果排版" value="table" options={[options[0], { ...options[1], disabled: true, reason: '当前领域没有封面' }]} onChange={change} />);
  const covers = screen.getByRole('button', { name: '封面' });
  expect(covers).toBeDisabled();
  expect(covers).toHaveAttribute('title', '当前领域没有封面');
  await user.click(covers);
  expect(change).not.toHaveBeenCalled();
});
