import { InlineState, Section, StateLine } from './Primitives';
import { errorCode, readErrorText } from './evidenceFeedback';
import { formatNumber, laneSummary } from './evidencePresent';
import type { Inspector, WorkItem } from './evidenceTypes';
import type { CommentChannel } from './useCommentChannel';
import styles from './Discussion.module.css';

interface DiscussionBlockProps {
  item: WorkItem;
  inspector: Inspector;
  /** 评论通道地址（已校验同源）；没有时「读取评论原文」禁用并写明来源信息不完整。 */
  channelUrl: string | null;
  channel: CommentChannel;
}

function count(value: number | null, state: string): string {
  return state === 'KNOWN' && value !== null ? `${formatNumber(value)} 条` : '当前未知';
}

/**
 * 评论与回复：状态与数量、「平台显示 X 条，本地留存 Y 条」（两种事实不合并，也不互相推算）、
 * 「本机授权评论研究」读取评论原文。评论原文仍要用户点击才读取，作者只显示匿名上下文，
 * 页面不渲染平台用户标识，也不把评论原文放回普通列表（I07）。
 */
export function DiscussionBlock({ item, inspector, channelUrl, channel }: DiscussionBlockProps) {
  const comments = inspector.commentsCoverage;
  const replies = inspector.repliesCoverage;
  const platform = item.display.engagement.comment;
  const local = comments?.countState === 'KNOWN' && comments.count !== null ? `本地留存 ${formatNumber(comments.count)} 条` : '本地留存当前未知';
  const published = platform.state === 'KNOWN' && platform.value !== null ? `平台显示 ${formatNumber(platform.value)} 条` : '平台评论数当前未知';
  return (
    <Section title="评论与回复">
      <dl className={styles.coverage}>
        <div>
          <dt>评论</dt>
          <dd>
            <StateLine state={laneSummary(item.lanes, 'comments')?.state} />
            <span>{count(comments?.count ?? null, comments?.countState ?? 'UNKNOWN')}</span>
          </dd>
        </div>
        <div>
          <dt>回复</dt>
          <dd>
            <StateLine state={laneSummary(item.lanes, 'replies')?.state} />
            <span>{count(replies?.count ?? null, replies?.countState ?? 'UNKNOWN')}</span>
          </dd>
        </div>
      </dl>
      <p className={styles.counts}>{`${published}，${local}`}</p>
      <InlineState tone="restricted" title="原文只在本机授权详情中按页读取">
        作者只显示匿名上下文；页面不会渲染平台用户标识，也不会把评论原文放回普通列表。
      </InlineState>
      <div className={styles.actions}>
        <button type="button" disabled={channelUrl === null || channel.phase === 'loading'} onClick={() => channel.load(false)}>
          {channel.phase === 'loading' && channel.items.length === 0 ? '正在读取评论' : '读取评论原文'}
        </button>
        {channelUrl === null ? <span className={styles.note}>来源信息不完整：详情没有给出评论通道地址。</span> : null}
      </div>
      <CommentList channel={channel} />
    </Section>
  );
}

function CommentList({ channel }: { channel: CommentChannel }) {
  if (channel.phase === 'error' && channel.items.length === 0) {
    return (
      <InlineState tone="danger" title="评论研究通道读取失败" code={errorCode(channel.error) ?? 'COMMENT_CHANNEL_UNAVAILABLE'}>
        {readErrorText(channel.error) ?? '请求没有完成；已读到的内容不受影响。'}
      </InlineState>
    );
  }
  if (channel.items.length === 0) {
    return channel.phase === 'loaded' ? (
      <InlineState title="当前通道没有返回评论材料">这只描述本次授权读取，不表示平台评论为 0。</InlineState>
    ) : null;
  }
  const more = channel.truncated ? '；本页已截断，可继续读取' : channel.cursor ? '；可继续读取' : '';
  return (
    <div className={styles.list}>
      {channel.items.map(comment => (
        <article key={comment.sourceRef} className={styles.comment} data-relation={comment.relation}>
          <strong title={`来源引用 ${comment.sourceRef}`}>{comment.relation === 'REPLY' ? '匿名回复' : '匿名评论'}</strong>
          <p>{comment.bodyState === 'KNOWN' && comment.body !== null ? comment.body : '评论正文当前未知'}</p>
          <span className={styles.meta}>{comment.bodyTruncated ? '本条正文已在读取边界截断' : '本条正文未在通道内截断'}</span>
        </article>
      ))}
      <p className={styles.receipt}>{`总数 ${channel.total === null ? '未知' : channel.total} · 当前 ${channel.items.length} · 本次 ${channel.returned === null ? '未知' : channel.returned}${more}`}</p>
      {channel.phase === 'error' ? (
        <InlineState tone="danger" title="继续读取评论失败" code={errorCode(channel.error) ?? 'COMMENT_CHANNEL_UNAVAILABLE'}>
          已读到的评论仍保留。
        </InlineState>
      ) : null}
      {channel.cursor ? (
        <button type="button" disabled={channel.phase === 'loading'} onClick={() => channel.load(true)}>
          继续读取评论
        </button>
      ) : null}
    </div>
  );
}
