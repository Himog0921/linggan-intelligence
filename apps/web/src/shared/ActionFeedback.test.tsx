import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { ActionFeedback } from './ActionFeedback';

it('结果未确认：写明阶段、请求编号与技术代码，读屏用 status', () => {
  render(
    <ActionFeedback state="unconfirmed" requestRef="req-7f21" code="network_error">
      提交结果尚未确认，请使用原请求核对，不重复派单。
    </ActionFeedback>,
  );
  const receipt = screen.getByText('未确认').closest('[data-lids-component]');
  expect(receipt).toHaveAttribute('data-lids-component', 'C44');
  expect(receipt).toHaveAttribute('data-receipt-state', 'unconfirmed');
  expect(receipt).toHaveAttribute('role', 'status');
  expect(receipt).toHaveTextContent('请求编号 req-7f21');
  expect(receipt).toHaveTextContent('network_error');
});

it('接纳与完成分开说：服务器收下请求不等于结果已经生效', () => {
  const { unmount } = render(<ActionFeedback state="accepted">请求已由服务器接纳；详情取得数在实际回执入库后更新。</ActionFeedback>);
  expect(screen.getByText('已接纳')).toBeInTheDocument();
  expect(screen.queryByText('已完成')).toBeNull();
  unmount();
  render(<ActionFeedback state="applied">观察关系已保存。</ActionFeedback>);
  expect(screen.getByText('已完成')).toBeInTheDocument();
});

it('未完成用 danger 阶段标注，并给出下一步', () => {
  render(
    <ActionFeedback state="rejected" action={<button type="button">重新核对</button>}>
      没有完成：服务器拒绝本次写入。
    </ActionFeedback>,
  );
  const receipt = screen.getByText('未完成').closest('[data-lids-component]');
  expect(receipt).toHaveAttribute('data-receipt-state', 'rejected');
  expect(screen.getByRole('button', { name: '重新核对' })).toBeInTheDocument();
});

it('进行中：读屏标为忙碌，结果还没出来就只说在途', () => {
  render(<ActionFeedback state="pending">正在提交详情缺口补采，结果以服务器回执为准。</ActionFeedback>);
  const receipt = screen.getByText('进行中').closest('[data-lids-component]');
  expect(receipt).toHaveAttribute('data-receipt-state', 'pending');
  expect(receipt).toHaveAttribute('aria-busy', 'true');
});

it('还没有回执时保留空的播报区，不显示任何阶段文字', () => {
  render(<ActionFeedback />);
  const region = screen.getByRole('status');
  expect(region).toHaveAttribute('aria-live', 'polite');
  expect(region).toBeEmptyDOMElement();
});
