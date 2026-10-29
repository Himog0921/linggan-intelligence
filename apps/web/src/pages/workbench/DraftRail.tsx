import { humanMoment } from '../../shared/formatting';
import type { IdeaBoxSummary, WorkSummary } from './workbenchModel';
import styles from './WorkbenchPage.module.css';

type CopyGroup = 'pending' | 'copied' | 'unknown';
function copyGroup(work: WorkSummary): CopyGroup {
  if (work.copiedRevisionId === undefined) return 'unknown';
  return work.copiedRevisionId === work.revisionId ? 'copied' : 'pending';
}

/** Status describes clipboard receipts for the saved revision, never platform publication. */
export function DraftRail({ drafts, ideas = [], status, currentId, onOpen }: {
  drafts: WorkSummary[]; ideas?: IdeaBoxSummary[]; status: 'loading' | 'error' | 'ready'; currentId: string; onOpen: (id: string) => void;
}) {
  const groups: { key: CopyGroup; label: string; note: string }[] = [
    { key: 'pending', label: '待处理', note: '当前保存版本尚未确认复制' },
    { key: 'copied', label: '已复制', note: '已复制不代表已发布' },
    { key: 'unknown', label: '状态未核实', note: '复制记录未返回' },
  ];
  return <aside className={styles.draftRail} aria-label="稿件列表">
    <div className={styles.draftRailHeading}><strong>我的稿件</strong><small>{status === 'loading' ? '正在读取稿件…' : status === 'error' ? '稿件读取失败' : `${drafts.length} 篇已保存`}</small></div>
    {groups.map(group => {
      const items = drafts.filter(item => copyGroup(item) === group.key);
      return items.length ? <section key={group.key} aria-label={group.label}>
        <div className={styles.draftGroupHeading}><strong>{group.label}</strong><small>{items.length}</small></div>
        <ul>{items.map(item => <li key={item.workId}><button type="button" aria-current={item.workId === currentId ? 'page' : undefined} onClick={() => onOpen(item.workId)}>
          <b>{item.title || '未命名稿件'}</b>{item.ideaRef ? <small>选题：{ideas.find(idea => idea.ideaRef === item.ideaRef)?.name ?? '读取中'}</small> : null}<small>{group.note}</small><small>更新于 {humanMoment(item.savedAt)}</small>
        </button></li>)}</ul>
      </section> : null;
    })}
    {!drafts.length && status === 'ready' ? <p>还没有其他已保存稿件。</p> : null}
  </aside>;
}
