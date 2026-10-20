import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it, vi } from 'vitest';
import { CustomDialog } from './OverviewPage';
import { useOpsLeave } from './useOpsLeave';

it('distinguishes missing dates from reversed or excessive ranges and clears stale feedback', () => {
  const save = vi.fn();
  render(<CustomDialog from="" to="" close={vi.fn()} save={save}/>);
  const start = screen.getByLabelText('开始日期'), end = screen.getByLabelText('结束日期');
  const confirm = screen.getByRole('button', { name: '确认' });
  fireEvent.change(start, { target: { value: '2026-10-02' } }); fireEvent.click(confirm);
  expect(screen.getByRole('alert')).toHaveTextContent('请选择开始和结束日期');
  expect(save).not.toHaveBeenCalled();
  fireEvent.change(end, { target: { value: '2026-10-01' } });
  expect(screen.getByRole('alert')).toBeEmptyDOMElement();
  fireEvent.click(confirm); expect(screen.getByRole('alert')).toHaveTextContent('开始日期不能晚于结束日期');
  fireEvent.change(end, { target: { value: '2028-10-02' } }); fireEvent.click(confirm);
  expect(screen.getByRole('alert')).toHaveTextContent('最长 366 天'); expect(save).not.toHaveBeenCalled();
  fireEvent.change(end, { target: { value: '2026-10-03' } }); fireEvent.click(confirm);
  expect(save).toHaveBeenCalledWith('2026-10-02', '2026-10-03');
});

it('labels an uncertain period switch accurately and invokes only the selected continuation', () => {
  const change = vi.fn(), close = vi.fn();
  function Harness() {
    const guard = useOpsLeave({ dirty: true, pending: true, onClose: close });
    return <><button onClick={() => guard.requestLeave(change, 'switch_period')}>切换周期</button>{guard.confirmation}</>;
  }
  render(<Harness/>); fireEvent.click(screen.getByRole('button', { name: '切换周期' }));
  expect(screen.getByText(/切换周期不会停止原请求/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: '继续编辑' })); expect(change).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '切换周期' }));
  fireEvent.click(screen.getByRole('button', { name: '切换并继续跟踪' }));
  expect(change).toHaveBeenCalledOnce(); expect(close).not.toHaveBeenCalled();
});
