import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { expect, it, vi } from 'vitest';
import { SelectField } from '../../shared/SelectField';
import { FieldLabel } from './FieldLabel';

const options = [{ value: 'a', label: '选项甲' }, { value: 'b', label: '选项乙' }] as const;

function Wrapped({ onLabelClick }: { onLabelClick?: () => void }) {
  const [value, setValue] = useState<'a' | 'b'>('a');
  return <FieldLabel onClick={onLabelClick}>所属账号<SelectField label="所属账号" value={value} options={options} onChange={setValue} /></FieldLabel>;
}

it('点选项时取消 label 的默认动作，否则浏览器会把点击转给触发按钮、菜单被重新打开', () => {
  render(<Wrapped />);
  fireEvent.click(screen.getByRole('combobox', { name: '所属账号' }));
  const option = screen.getByRole('option', { name: /选项乙/ });
  // fireEvent returns false when the click's default action was cancelled.
  expect(fireEvent.click(option)).toBe(false);
  expect(screen.queryByRole('listbox')).toBeNull();
  expect(screen.getByRole('combobox', { name: '所属账号' })).toHaveTextContent('选项乙');
});

it('点标题文字仍是正常的 label 点击，调用方自己的点击处理照常执行', () => {
  const onLabelClick = vi.fn();
  render(<Wrapped onLabelClick={onLabelClick} />);
  expect(fireEvent.click(screen.getByText('所属账号', { selector: 'label' }))).toBe(true);
  // The label forwards the click to the trigger, whose click bubbles back to the label: the handler may run more than once.
  expect(onLabelClick).toHaveBeenCalled();
  expect(screen.getByRole('listbox')).toBeInTheDocument();
});
