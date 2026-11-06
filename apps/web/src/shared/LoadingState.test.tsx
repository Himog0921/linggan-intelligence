import { fireEvent, render, screen } from '@testing-library/react';
import { expect, it } from 'vitest';
import { LoadingState } from './LoadingState';

it('首次读取只报读取中，不渲染旧内容', () => {
  render(
    <LoadingState phase="initial" subject="领域配置">
      <p>上次读取结果</p>
    </LoadingState>,
  );
  const line = screen.getByRole('status');
  expect(line).toHaveTextContent('正在读取领域配置…');
  expect(line).toHaveAttribute('aria-busy', 'true');
  expect(screen.queryByText('上次读取结果')).not.toBeInTheDocument();
});

it('刷新时说明正在刷新，旧结果仍然可读', () => {
  render(
    <LoadingState phase="refreshing" subject="巡检记录">
      <p>上次读取结果</p>
    </LoadingState>,
  );
  expect(screen.getByRole('status')).toHaveTextContent('正在刷新巡检记录，以下是上次读取结果。');
  expect(screen.getByText('上次读取结果')).toBeInTheDocument();
});

it('读取失败时说明原因、保留旧结果并给出重试', () => {
  render(
    <LoadingState
      phase="stale_error"
      subject="工位信息"
      error="工位信息读取失败。"
      retained="以下保留上次读取结果（2026-11-06 09:20）。"
      action={<button type="button">重新读取</button>}
    >
      <p>上次读取结果</p>
    </LoadingState>,
  );
  const alert = screen.getByRole('alert');
  expect(alert).toHaveTextContent('工位信息读取失败。以下保留上次读取结果（2026-11-06 09:20）。');
  expect(screen.getByText('上次读取结果')).toBeInTheDocument();
  expect(screen.getByRole('button', { name: '重新读取' })).toBeInTheDocument();
});

it('没有旧结果时不声称保留', () => {
  render(<LoadingState phase="stale_error" subject="领域配置" />);
  const alert = screen.getByRole('alert');
  expect(alert).toHaveTextContent('领域配置当前读不到。');
  expect(alert).not.toHaveTextContent('保留上次读取结果');
});

it('刷新与读取失败不重建旧内容：未提交的草稿和展开状态留在原地', () => {
  const content = <input aria-label="领域名称" defaultValue="" />;
  const { rerender } = render(<LoadingState phase="ready" subject="领域配置">{content}</LoadingState>);
  fireEvent.change(screen.getByRole('textbox', { name: '领域名称' }), { target: { value: '草稿领域' } });
  rerender(<LoadingState phase="refreshing" subject="领域配置">{content}</LoadingState>);
  rerender(<LoadingState phase="ready" subject="领域配置">{content}</LoadingState>);
  rerender(<LoadingState phase="stale_error" subject="领域配置">{content}</LoadingState>);
  expect(screen.getByRole('textbox', { name: '领域名称' })).toHaveValue('草稿领域');
});

it('旧结果由页面自己渲染时，仍可声明保留', () => {
  render(<LoadingState phase="stale_error" subject="巡检记录" retainsOld />);
  expect(screen.getByRole('alert')).toHaveTextContent('以下保留上次读取结果。');
});
