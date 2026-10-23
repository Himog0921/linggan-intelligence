import { useInfiniteQuery } from '@tanstack/react-query';
import { fetchDossierComments } from './dossierApi';
import { hideCachedReading, nestComments } from './dossierPresent';
import type { DossierComment } from './dossierTypes';
import { formatNumber } from './evidencePresent';
import { dossierCommentsKey } from './evidenceKeys';
import styles from './Dossier.module.css';

function likes(item: DossierComment): string {
  return item.likeCount === null ? '赞 —' : `赞 ${formatNumber(item.likeCount)}`;
}

function CommentItem({ item, replies, orphan }: { item: DossierComment; replies: DossierComment[]; orphan: boolean }) {
  return (
    <article className={styles.comment} data-relation={item.relation}>
      <p className={styles.commentMeta}>{item.relation === 'REPLY' ? `回复 · ${likes(item)}` : likes(item)}</p>
      <p className={styles.commentText}>{item.body || '这条评论没有取得文字'}</p>
      {orphan ? <p className={styles.note}>它回复的那条评论还没读到。</p> : null}
      {replies.length > 0 ? (
        <div className={styles.replies}>
          {replies.map(reply => (
            <CommentItem key={reply.sourceRef} item={reply} replies={[]} orphan={false} />
          ))}
        </div>
      ) : null}
    </article>
  );
}

/** 跨页去重：同一条评论只出现一次（续读不会重复）。 */
function uniqueItems(pages: readonly { items: readonly DossierComment[] }[]): DossierComment[] {
  const seen = new Set<string>();
  return pages.flatMap(page => page.items).filter(item => (seen.has(item.sourceRef) ? false : (seen.add(item.sourceRef), true)));
}

/**
 * 「评论与回复」里的评论原文：「作品」标签一打开就读第一页（每页 30 条），主评论下嵌回复，直接显示全文；
 * 还有更多时给「加载更多评论」。与正文同一规则：标签没激活就不读。读不出来只在这里提示，正文与图组照常。
 */
export function DossierComments({ work, domain, active }: { work: string; domain: string; active: boolean }) {
  const query = useInfiniteQuery({
    queryKey: dossierCommentsKey(domain, work),
    initialPageParam: null as string | null,
    queryFn: ({ pageParam, signal }) => fetchDossierComments(work, domain, pageParam, signal),
    getNextPageParam: page => page.nextCursor ?? undefined,
    enabled: active,
    retry: false,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
  // 读取被明确拒绝（无权限、来源撤回或不存在）时，已经显示的评论一并收起，不继续展示旧读数。
  const denied = query.isError && hideCachedReading(query.error);
  const items = query.data && !denied ? uniqueItems(query.data.pages) : [];
  const nodes = nestComments(items);
  const failed = query.isError && !query.isFetching;
  return (
    <div className={styles.comments} aria-busy={query.isFetching}>
      {nodes.map(node => (
        <CommentItem key={node.item.sourceRef} item={node.item} replies={node.replies} orphan={node.orphan} />
      ))}
      {query.isPending && !failed ? <p className={styles.note} role="status">正在读取评论…</p> : null}
      {query.isSuccess && items.length === 0 ? <p className={styles.note}>还没有采到评论。</p> : null}
      {failed ? (
        <p className={styles.note} role="status">
          评论暂时读不出来，正文和图片不受影响。
          <button type="button" className={styles.inlineAction} onClick={() => void (query.isFetchNextPageError ? query.fetchNextPage() : query.refetch())}>
            重新读取
          </button>
        </p>
      ) : null}
      {query.hasNextPage && !failed ? (
        <button type="button" disabled={query.isFetchingNextPage} onClick={() => void query.fetchNextPage()}>
          {query.isFetchingNextPage ? '正在读取…' : '加载更多评论'}
        </button>
      ) : null}
    </div>
  );
}
