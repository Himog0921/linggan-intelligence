import { render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { InlineNotice } from './InlineNotice';

function noticeOf(text: string) {
  return screen.getByText(text).closest('div');
}

it('默认是说明：读屏用 status，点明原因与影响', () => {
  render(<InlineNotice>当前可选轮次中，未找到本轮之前的同规则观察。</InlineNotice>);
  const notice = noticeOf('当前可选轮次中，未找到本轮之前的同规则观察。');
  expect(notice).toHaveAttribute('data-lids-component', 'C43');
  expect(notice).toHaveAttribute('data-tone', 'info');
  expect(notice).toHaveAttribute('role', 'status');
  expect(notice?.querySelector('button')).toBeNull();
});

it('限制类提示仍用 status，但标明是注意而不是失败', () => {
  render(
    <InlineNotice tone="warning" action={<button type="button">返回作品</button>}>
      当前只检索了有界范围，未找到候选不能解释为作者从未讲过。
    </InlineNotice>,
  );
  const notice = noticeOf('当前只检索了有界范围，未找到候选不能解释为作者从未讲过。');
  expect(notice).toHaveAttribute('data-tone', 'warning');
  expect(notice).toHaveAttribute('role', 'status');
  expect(screen.getByRole('button', { name: '返回作品' })).toBeInTheDocument();
});

it('错误用 alert 播报，并给出下一步', () => {
  render(
    <InlineNotice tone="error" action={<button type="button">重新核对材料资格</button>}>
      此来源引用当前不可解析。
    </InlineNotice>,
  );
  const notice = noticeOf('此来源引用当前不可解析。');
  expect(notice).toHaveAttribute('data-tone', 'error');
  expect(notice).toHaveAttribute('role', 'alert');
  expect(screen.getByRole('button', { name: '重新核对材料资格' })).toBeInTheDocument();
});
