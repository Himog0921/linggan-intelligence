import { describe, expect, it } from 'vitest';
import { topicRows } from './topicRows';
import type { WorkSummary } from './workbenchModel';

const base: WorkSummary = { workId: 'anchor', domainRef: 'domain', title: '笔记标题甲', revisionId: 'r1', savedAt: '2026-10-29T00:00:00Z', ideaRef: 'idea' };
describe('today idea cards', () => {
  it('uses the anchor and idea name once for drafts linked to the same idea', () => {
    const works = [{ ...base, workId: 'second', title: '笔记标题乙' }, base, { ...base, workId: 'legacy', ideaRef: null, title: '旧手写稿' }];
    const ideas = [{ ideaRef: 'idea', anchorWorkId: 'anchor', anchorRevisionId: 'anchor-revision', savedAt: '2026-10-28T00:00:00Z', name: '真正的选题', intent: '意图' }];
    const rows = topicRows(works, ideas);
    expect(rows.map(row => [row.workId, row.title])).toEqual([['anchor', '真正的选题'], ['legacy', '旧手写稿']]);
    expect(topicRows(works, [])[0]?.title).toBe('选题信息暂不可用');
    expect(topicRows([works[0]!], ideas)[0]).toMatchObject({ workId: 'anchor', revisionId: 'anchor-revision', savedAt: '2026-10-28T00:00:00Z' });
  });
});
