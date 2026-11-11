import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { TopicProgress } from './TopicProgress';
import { decodeSnapshot } from './topicDtos';
import { topicSnapshot } from './topicTestFixture';
afterEach(cleanup);
const progress = { totalWorkCount: 12, material: { ready: 6, preparing: 3, blocked: 2, unknown: 1 }, understanding: { available: 4, missing: 5, stale: 2, unknown: 1 }, graph: { assigned: 3, unassigned: 9 } };
it('preserves separate server scope stages rather than inferring counts from loaded works', () => {
  const snapshot = topicSnapshot();
  const result = decodeSnapshot({ ...snapshot, scope: { ...snapshot.scope, progress } });
  expect(result.scope.progress).toEqual(progress);
  render(<TopicProgress progress={result.scope.progress} />);
  const region = screen.getByRole('region', { name: '当前范围处理进度' });
  for (const text of ['就绪 6', '准备中 3', '受阻 2', '可用 4', '未理解 5', '待更新 2', '已归主题 3', '待归主题 9']) expect(region).toHaveTextContent(text);
});
it('keeps unavailable historical progress distinct from a real zero', () => {
  expect(decodeSnapshot(topicSnapshot()).scope.progress).toBeUndefined();
  const view = render(<TopicProgress />);
  expect(screen.getByText('待归主题')).toHaveTextContent('—');
  view.rerender(<TopicProgress progress={{ ...progress, graph: { assigned: 12, unassigned: 0 } }} />);
  expect(screen.getByText('待归主题')).toHaveTextContent('0');
});
it('rejects partial progress rather than silently inventing missing counts', () => {
  const snapshot = topicSnapshot();
  expect(() => decodeSnapshot({ ...snapshot, scope: { ...snapshot.scope, progress: { ...progress, understanding: { available: 4 } } } })).toThrow();
});
