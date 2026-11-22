import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { reason } from '../api';
import { Dialog, Status } from '../ui';
import { getPublications, getSpotCheck, type Drill, type Snapshot } from './api';
import css from './Lens.module.css';

export function PublicationList({ snapshotRef, drill, close }: { snapshotRef: string; drill: Drill; close: () => void }) {
  const [offset,setOffset] = useState(0);
  const query = useQuery({ queryKey:['ops','content-lens','publications',snapshotRef,drill,offset], retry:false,
    queryFn: ({signal}) => getPublications(snapshotRef,drill,offset,signal) });
  return <Dialog label="快照笔记列表" outsideClose onClose={close} className={css.dialog}>
    <header className={css.heading}><div><h2>{drill.label}</h2><small>快照 {snapshotRef.slice(0,8)}</small></div><ActionButton onClick={close}>关闭</ActionButton></header>
    {query.isError ? <Status error retry={() => void query.refetch()}>笔记读取失败（{reason(query.error)}）。</Status>
    : query.isPending ? <Status>正在读取笔记…</Status> : <>
      <p>共 {query.data.total} 条发布记录</p>
      {query.data.items.length > 0 ? <table className={css.effects}><thead><tr><th>标题</th><th>账号</th><th>发布日期</th><th>阅读量</th><th>评论数</th></tr></thead><tbody>
        {query.data.items.map(p => <tr key={p.ref} data-publication-ref={p.ref}><th scope="row">{p.title ?? '标题未取得'}</th><td>{p.account ?? '未知'}</td><td>{p.date ?? '未知'}</td><td>{p.views ?? '未填'}</td><td>{p.comments ?? '未填'}</td></tr>)}
      </tbody></table> : <p>该样本没有发布记录。</p>}
      {(offset > 0 || query.data.total > query.data.limit) && <nav aria-label="笔记分页" className={css.controls}>
        <ActionButton disabled={offset === 0} onClick={() => setOffset(Math.max(0,offset-query.data.limit))}>上一页</ActionButton>
        <span>{offset+1}–{offset+query.data.items.length} / {query.data.total}</span>
        <ActionButton disabled={offset+query.data.items.length >= query.data.total} onClick={() => setOffset(offset+query.data.limit)}>下一页</ActionButton>
      </nav>}
    </>}
  </Dialog>;
}
export function SpotList({ snapshot,close }: { snapshot:Snapshot; close:()=>void }) {
  const query = useQuery({ queryKey:['ops','content-lens','spot-check',snapshot.ref],retry:false,
    queryFn: ({signal}) => getSpotCheck(snapshot.ref,signal) });
  return <Dialog label="人工抽看样本" onClose={close} outsideClose className={css.dialog}>
    <header className={css.heading}><h2>人工抽看样本</h2><ActionButton onClick={close}>关闭</ActionButton></header>
    {query.isError ? <Status error retry={() => void query.refetch()}>抽看样本读取失败（{reason(query.error)}）。</Status>
      : query.isPending ? <Status>正在读取样本…</Status> : query.data.length === 0 ? <p>暂无抽看样本。</p> :
      <table className={css.effects}><thead><tr><th>标题</th><th>给什么</th><th>时刻</th><th>处境</th></tr></thead><tbody>{query.data.map(p => <tr key={p.key}>
        <th scope="row">{p.title ?? '标题未取得'}</th>{(['offer','moment','situation'] as const).map(dim => <td key={dim}>{snapshot.structure[dim].find(row => row.key === p[dim])?.name ?? p[dim]}</td>)}
      </tr>)}</tbody></table>}
  </Dialog>;
}
