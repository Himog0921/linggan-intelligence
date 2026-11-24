import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import * as ops from '../api';
import { PublicationDetail } from '../PublicationDetail';
import { c } from '../styles';
import { Dialog, Status } from '../ui';
import { getPublications, type Drill } from './api';
import css from './Lens.module.css';

type Publication = Awaited<ReturnType<typeof getPublications>>['items'][number];

export function PublicationList({ snapshotRef, domain, drill, close }: {
  snapshotRef: string; domain: string; drill: Drill; close: () => void;
}) {
  const [offset,setOffset] = useState(0);
  const [selected, setSelected] = useState<Publication | null>(null);
  const query = useQuery({ queryKey:['ops','content-lens','publications',snapshotRef,drill,offset], retry:false,
    queryFn: ({signal}) => getPublications(snapshotRef,drill,offset,signal) });
  return <>
    <Dialog label="快照笔记列表" outsideClose onClose={close} className={css.dialog}>
    <header className={css.heading}><div><h2>{drill.label}</h2>{query.data && <p>共 {query.data.total} 条发布记录</p>}</div><ActionButton onClick={close}>关闭</ActionButton></header>
    <div className={css.dialogBody}>
    {query.isError ? <Status error retry={() => void query.refetch()}>笔记读取失败（{ops.reason(query.error)}）。</Status>
    : query.isPending ? <Status>正在读取笔记…</Status> : <>
      {query.data.items.length > 0 ? <table className={css.records}><thead><tr><th>标题</th><th>账号</th><th>发布日期</th><th>阅读量</th><th>评论数</th></tr></thead><tbody>
        {query.data.items.map(p => <tr key={p.ref} data-publication-ref={p.ref}><th scope="row"><ActionButton variant="quiet" aria-label={`查看发布记录：${p.title ?? '标题未取得'}`}
          onClick={() => setSelected(p)}>{p.title ?? '标题未取得'}</ActionButton></th><td>{p.account ?? '未知'}</td><td>{p.date ?? '未知'}</td><td>{p.views ?? '未填'}</td><td>{p.comments ?? '未填'}</td></tr>)}
      </tbody></table> : <p>该样本没有发布记录。</p>}
      {(offset > 0 || query.data.total > query.data.limit) && <nav aria-label="笔记分页" className={css.controls}>
        <ActionButton disabled={offset === 0} onClick={() => setOffset(Math.max(0,offset-query.data.limit))}>上一页</ActionButton>
        <span>{offset+1}–{offset+query.data.items.length} / {query.data.total}</span>
        <ActionButton disabled={offset+query.data.items.length >= query.data.total} onClick={() => setOffset(offset+query.data.limit)}>下一页</ActionButton>
      </nav>}
    </>}
    </div>
    </Dialog>
    {selected ? <SelectedPublication key={selected.ref} publication={selected} domain={domain} close={() => setSelected(null)} /> : null}
  </>;
}

function SelectedPublication({ publication, domain, close }: {
  publication: Publication;
  domain: string;
  close: () => void;
}) {
  const query = useQuery({
    queryKey: ['ops', 'publication-detail', domain, publication.ref],
    queryFn: ({ signal }) => ops.get(ops.url('publications', { domain, publication_ref: publication.ref }), ops.publications, signal),
    retry: false,
  });
  const detail = query.data?.items.find(item => item.ref === publication.ref);
  if (query.isSuccess && detail) {
    return <PublicationDetail detail={detail} isAdmin snapshotLinks={publication.creationLinks} close={close} />;
  }
  return <Dialog label="发布记录详情" outsideClose onClose={close}>
    <header className={c('ops-detail-head')}>
      <h2>{publication.title || '发布记录详情'}</h2>
      <ActionButton onClick={close}>关闭详情</ActionButton>
    </header>
    <div className={c('ops-dialog-body')}>
      {query.isError ? <Status error retry={() => void query.refetch()}>发布记录读取失败（{ops.reason(query.error)}）。</Status>
        : query.isPending ? <Status>正在读取发布记录…</Status>
        : <Status>此快照中的发布记录在当前台账已不可用。</Status>}
    </div>
  </Dialog>;
}
