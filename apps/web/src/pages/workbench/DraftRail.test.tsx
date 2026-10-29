import { fireEvent, render, screen, within } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { DraftRail } from './DraftRail';
import type { WorkSummary } from './workbenchModel';

const base: WorkSummary = { workId: 'a', domainRef: 'domain', title: '第一篇', revisionId: 'r1', savedAt: '2026-10-29T00:00:00Z' };
describe('draft rail', () => {
  it('groups by copy confirmation for the current saved revision without claiming publication', () => {
    const onOpen = vi.fn();
    render(<DraftRail currentId="a" status="ready" onOpen={onOpen} drafts={[
      { ...base, copiedRevisionId: null },
      { ...base, workId: 'b', title: '第二篇', copiedRevisionId: 'r1' },
      { ...base, workId: 'c', title: '第三篇', revisionId: 'r2', copiedRevisionId: 'r1' },
      { ...base, workId: 'd', title: '第四篇' },
    ]} />);
    expect(within(screen.getByRole('region', { name: '待处理' })).getByText('第一篇')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '待处理' })).getByText('第三篇')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '已复制' })).getByText('已复制不代表已发布')).toBeVisible();
    expect(within(screen.getByRole('region', { name: '状态未核实' })).getByText('第四篇')).toBeVisible();
    fireEvent.click(screen.getByRole('button', { name: /第二篇/ }));
    expect(onOpen).toHaveBeenCalledWith('b');
  });
});
