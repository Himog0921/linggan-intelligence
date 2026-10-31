import { humanMoment } from '../../shared/formatting';
import { useEffect } from 'react';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { checkSavedDrafts, type DraftCheckSummary } from './draftChecks';
import type { IdeaBoxSummary, WorkSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

type DraftGroup = 'must' | 'warn' | 'checked' | 'copied' | 'published' | 'unknown';
function draftGroup(work: WorkSummary, check: DraftCheckSummary | undefined): DraftGroup {
  if (work.publicationUnavailable) return 'unknown';
  if (work.publishedRevisionId) return 'published';
  if (!check || check.revisionId !== work.revisionId) return 'unknown';
  if (check.must) return 'must';
  if (work.copiedRevisionId === work.revisionId) return 'copied';
  if (check.warn) return 'warn';
  return 'checked';
}

/** Checks and copy receipts describe saved revisions; neither proves platform publication. */
export function DraftRail({ drafts, ideas = [], status, currentId, onOpen }: {
  drafts: WorkSummary[]; ideas?: IdeaBoxSummary[]; status: 'loading' | 'error' | 'ready'; currentId: string; onOpen: (id: string) => void;
}) {
  const queryClient = useQueryClient();
  const ordered = [...drafts].sort((a, b) => a.workId.localeCompare(b.workId));
  const batches = Array.from({ length: Math.ceil(ordered.length / 25) }, (_, index) => ordered.slice(index * 25, (index + 1) * 25));
  const checks = useQueries({ queries: batches.map(batch => ({
    queryKey: ['creation-assets', 'draft-checks', batch.map(item => [item.domainRef, item.workId, item.revisionId])],
    queryFn: ({ signal }: { signal: AbortSignal }) => checkSavedDrafts(batch, signal),
    enabled: status === 'ready', retry: false, gcTime: 0,
    staleTime: Infinity,
  })) });
  const recheck = () => { void (async () => {
    await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
    await queryClient.invalidateQueries({ queryKey: ['creation-assets', 'draft-checks'] });
  })(); };
  useEffect(() => {
    const onChange = () => { void (async () => {
      await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
      await queryClient.invalidateQueries({ queryKey: ['creation-assets', 'draft-checks'] });
    })(); };
    const channel = typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel('linggan-creation-experience-events');
    if (channel) channel.onmessage = onChange;
    window.addEventListener('focus', onChange);
    return () => { channel?.close(); window.removeEventListener('focus', onChange); };
  }, [queryClient]);
  const results: Record<string, DraftCheckSummary | undefined> = {};
  batches.forEach((_batch, index) => {
    const check = checks[index];
    if (check && !check.isFetching && !check.isError && !check.isStale) Object.assign(results, check.data);
  });
  const groups: { key: DraftGroup; label: string; note: string }[] = [
    { key: 'must', label: '必须先改', note: '保存版本仍有必改' },
    { key: 'warn', label: '有提醒', note: '保存版本有提醒' },
    { key: 'checked', label: '已检查', note: '保存版本没有必改' },
    { key: 'copied', label: '已复制', note: '已复制不代表已发布' },
    { key: 'published', label: '已登记发布', note: '已关联运营发布记录' },
    { key: 'unknown', label: '状态未核实', note: '保存版本需要重新核验' },
  ];
  return <aside className={styles.draftRail} aria-label="稿件列表">
    <div className={styles.draftRailHeading}><strong>我的稿件</strong><small>{status === 'loading' ? '正在读取稿件…' : status === 'error' ? '稿件读取失败' : `${drafts.length} 篇已保存`}</small>{checks.some(check => check.isError) ? <button type="button" onClick={recheck}>重新检查</button> : null}</div>
    {groups.map(group => {
      const items = drafts.filter(item => draftGroup(item, results[item.workId]) === group.key);
      return items.length ? <section key={group.key} aria-label={group.label}>
        <div className={styles.draftGroupHeading}><strong>{group.label}</strong><small>{items.length}</small></div>
        <ul>{items.map(item => {
          const check = results[item.workId];
          return <li key={item.workId}><button type="button" aria-current={item.workId === currentId ? 'page' : undefined} onClick={() => onOpen(item.workId)}>
            <b>{item.title || '未命名稿件'}</b>{item.ideaRef ? <small>选题：{ideas.find(idea => idea.ideaRef === item.ideaRef)?.name ?? '读取中'}</small> : null}<small>{item.publicationUnavailable ? '关联的发布记录当前不可用' : group.note}{item.publishedRevisionId && item.publishedRevisionId !== item.revisionId ? ' · 当前稿件已修改' : ''}{check?.revisionId === item.revisionId ? ` · 必改 ${check.must} · 提醒 ${check.warn}${check.notRun ? ' · 部分未检查' : ''}` : ''}</small><small>更新于 {humanMoment(item.savedAt)}</small>
          </button></li>;
        })}</ul>
      </section> : null;
    })}
    {!drafts.length && status === 'ready' ? <p>还没有其他已保存稿件。</p> : null}
  </aside>;
}
