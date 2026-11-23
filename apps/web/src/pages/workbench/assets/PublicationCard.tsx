import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import * as ops from '../../ops/api';
import * as api from './assetsApi';

const num = (value: number | null) => value === null ? '—' : value.toLocaleString('en-US');

/**
 * What came back after a draft was published. The numbers belong to the operations ledger, so this only reads the
 * linked publication there and links to where they are filled in; nothing is typed or stored here.
 */
export function PublicationCard({ workId, domainRef, accountRef }: { workId: string; domainRef: string; accountRef: string | null }) {
  const links = useQuery({ queryKey: ['creation-assets', 'publication-links', workId], queryFn: ({ signal }) => api.publicationLinks(workId, signal), retry: false, gcTime: 0 });
  const link = links.data?.find(item => item.available) ?? null;
  const record = useQuery({
    queryKey: ['creation-assets', 'publication-recovery', domainRef, accountRef, link?.publicationRef, link?.publishedOn],
    enabled: !!link && !!accountRef,
    retry: false, gcTime: 0,
    queryFn: async ({ signal }) => {
      // The ledger lists a business's publications by date; one account on the publication day narrows it to the record.
      const day: Record<string, string> = link?.publishedOn ? { from: link.publishedOn, to: link.publishedOn } : {};
      const list = await ops.get(ops.url('publications', { domain: domainRef, account_ref: accountRef ?? '', ...day }), ops.publications, signal);
      return list.items.find(item => item.ref === link?.publicationRef) ?? null;
    },
  });
  const ledger = `/ops/publications?domain=${encodeURIComponent(domainRef)}${accountRef ? `&account_ref=${encodeURIComponent(accountRef)}` : ''}`;

  const published = link ? <span className="aux">{link.publishedOn ?? '日期未填'}{link.title ? ` · ${link.title}` : ''}</span> : null;
  const views = record.data?.views;
  const verdict = !record.data ? null : record.data.platform !== 'xhs' ? <span className="chip unk">选填，不判爆款</span>
    : views?.hit ? <span className="chip sig">爆款</span>
      : views?.state === 'value' ? <span className="chip">未到爆款线</span>
        : views?.overdue ? <span className="chip warn">已满 7 天，阅读还没填</span> : <span className="chip unk">阅读还没填</span>;
  const status = !record.data ? null : views?.state === 'value' ? <span className="chip ok">已有阅读数据</span> : <span className="chip info">回收中</span>;

  return <section className="pub-card" aria-label="发布回收">
    <header>
      <span className="dot ok" /><b>已发布</b>{published}
      <span className="grow" style={{ flex: 1 }} />
      {status}
    </header>
    {links.isPending ? <p className="aux" role="status">正在读取关联的发布记录…</p>
      : links.isError ? <p className="aux" role="alert">关联的发布记录读取失败。<ActionButton variant="quiet" onClick={() => void links.refetch()}>重试</ActionButton></p>
        : !link ? <p className="aux">这篇稿子关联的发布记录当前不可用，所以读不到回收数据。</p>
          : record.isPending ? <p className="aux" role="status">正在读取回收数据…</p>
            : record.isError ? <p className="aux" role="alert">运营台账暂时读不到这条记录。<ActionButton variant="quiet" onClick={() => void record.refetch()}>重试</ActionButton></p>
              : !record.data ? <p className="aux">运营台账里还没有这条记录的数据。</p>
                : <div className="pub-grid">
                  <div><span className="lbl">阅读</span><b className="data">{views?.state === 'value' ? num(views.value) : '待回填'}</b></div>
                  <div><span className="lbl">点赞</span><b className="data">{num(record.data.likes)}</b></div>
                  <div><span className="lbl">判定</span><div>{verdict}</div></div>
                </div>}
    <div className="pub-steps aux">
      阅读 ≥ 5,000 记为爆款，达到后可以用底栏的“存为样板”保存这篇的写法结构。数据在运营发布记录里填写和回填，这里只读。
      <a href={ledger}>去运营发布记录 ↗</a>
    </div>
  </section>;
}
