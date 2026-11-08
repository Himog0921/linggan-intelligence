import { humanMoment } from '../../shared/formatting';
import { useEffect } from 'react';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { checkSavedDrafts, type DraftCheckSummary } from './draftChecks';
import type { IdeaBoxSummary, WorkSummary } from './workbenchModel';

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
const GROUP_DOT: Record<DraftGroup, string> = { must: 'bad', warn: 'warn', checked: 'ok', copied: 'ok', published: 'ok', unknown: 'unk' };
function stateText(work: WorkSummary, check: DraftCheckSummary | undefined, group: DraftGroup) {
  if (work.publicationUnavailable) return '关联的发布记录当前不可用';
  if (group === 'published') return `已登记发布${work.publishedRevisionId && work.publishedRevisionId !== work.revisionId ? ' · 当前稿件已修改' : ''}`;
  if (!check || check.revisionId !== work.revisionId) return '状态未核实';
  if (check.must) return `必改 ${check.must} · 提醒 ${check.warn}${check.notRun ? ' · 部分未检查' : ''}`;
  if (group === 'copied') return `已复制不代表已发布${check.warn ? ` · 提醒 ${check.warn}` : ''}`;
  if (check.warn) return `提醒 ${check.warn} · 可以复制`;
  return '可以复制了';
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
  const groups: { key: DraftGroup; label: string }[] = [
    { key: 'must', label: '必须先改' },
    { key: 'warn', label: '有提醒' },
    { key: 'checked', label: '已检查' },
    { key: 'copied', label: '已复制' },
    { key: 'published', label: '已登记发布' },
    { key: 'unknown', label: '状态未核实' },
  ];
  const today = new Date();
  return <aside className="queue" aria-label="稿件列表">
    <div className="q-head">
      <div className="q-head-top">
        <h2>今日 · {today.getMonth() + 1}月{today.getDate()}日 周{'日一二三四五六'[today.getDay()]}</h2>
        <small className="aux">{status === 'loading' ? '正在读取稿件…' : status === 'error' ? '稿件读取失败' : `${drafts.length} 篇已保存`}</small>
      </div>
      {checks.some(check => check.isError) ? <button type="button" className="btn sm" onClick={recheck}>重新检查</button> : null}
    </div>
    <div className="q-list">
      {groups.map(group => {
        const items = drafts.filter(item => draftGroup(item, results[item.workId]) === group.key);
        return items.length ? <section key={group.key} aria-label={group.label}>
          <div className="q-sec">{group.label} <span className="data">{items.length}</span></div>
          {items.map(item => {
            const check = results[item.workId];
            return <button type="button" key={item.workId} className="q-item" aria-current={item.workId === currentId ? 'page' : undefined} onClick={() => onOpen(item.workId)}>
              <span className="q-title">{item.title || '未命名稿件'}</span>
              <span className="q-meta"><span>{item.ideaRef ? ideas.find(idea => idea.ideaRef === item.ideaRef)?.name ?? '选题读取中' : '无选题框'}</span><span>{humanMoment(item.savedAt)}</span></span>
              <span className="q-state"><span className={`dot ${GROUP_DOT[group.key]}`} />{stateText(item, check, group.key)}</span>
            </button>;
          })}
        </section> : null;
      })}
      {!drafts.length && status === 'ready' ? <p className="q-foot">还没有其他已保存稿件。</p> : null}
    </div>
  </aside>;
}
