import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, expect, it } from 'vitest';
import { OperationsCoverage, OperationsWorkBadge } from './OperationsCoverage';
import { decodeSnapshot } from './topicDtos';
import { topicSnapshot } from './topicTestFixture';
import type { OperationsCoverage as Coverage } from './types';
afterEach(cleanup);
const coverage: Coverage = { publishedWorkCount: 1, publicationCount: 2, hitWorkCount: 1, unknownViewsWorkCount: 1, hitRule: { platform: 'xhs', metric: 'views', threshold: 5000 }, works: [{ workRef: 'w', publications: [
  { publicationRef: 'p1', accountRef: null, accountName: null, platform: 'xhs', publishedOn: null, views: 6000, hit: true },
  { publicationRef: 'p2', accountRef: null, accountName: null, platform: 'xhs', publishedOn: null, views: null, hit: false },
] }] };
it('keeps publication count, unique works, unknown views and manual marks separate', () => {
  const snapshot = topicSnapshot();
  const parsed = decodeSnapshot({ ...snapshot, statistics: { ...snapshot.statistics, operations: coverage }, scope: { ...snapshot.scope, operations: coverage } });
  expect(parsed.scope.operations).toEqual(coverage);
  expect(parsed.statistics.ownBreakoutCount).toBe(snapshot.statistics.ownBreakoutCount);
  expect(parsed.scope.operations?.works[0]?.publications[1]?.views).toBeNull();
  render(<><OperationsCoverage coverage={parsed.statistics.operations} /><OperationsWorkBadge coverage={coverage} workRef="w" /></>);
  const section = screen.getByRole('region', { name: '运营发布覆盖' });
  for (const text of ['运营发布 1 篇', '发布记录 2 条', '运营爆款 1 篇', '阅读未知 1 篇']) expect(section).toHaveTextContent(text);
  expect(screen.getByTitle('小红书阅读 ≥ 5,000')).toBeVisible();
  expect(screen.getAllByText('运营爆款')).toHaveLength(1);
});
it('only labels exact publication mappings and does not infer a hit from missing views', () => {
  const plain = { ...coverage, works: [{ workRef: 'w', publications: [coverage.works[0]!.publications[1]!] }] };
  const view = render(<OperationsWorkBadge coverage={plain} workRef="w" />);
  expect(screen.getByText('运营已发布')).toBeVisible();
  expect(screen.queryByText('运营爆款')).not.toBeInTheDocument();
  view.rerender(<OperationsWorkBadge coverage={plain} workRef="unmapped" />);
  expect(screen.queryByText('运营已发布')).not.toBeInTheDocument();
});
