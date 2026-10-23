import { useEffect, useRef, useState } from 'react';
import { fetchDossierComments } from './dossierApi';
import { hideCachedReading, nestComments } from './dossierPresent';
import type { DossierComment } from './dossierTypes';
import { Section } from './Primitives';
import { metricReading } from './evidencePresent';
import type { Metric } from './evidenceTypes';
import styles from './Dossier.module.css';

function Row({ item, replies, orphan }: { item: DossierComment; replies: DossierComment[]; orphan: boolean }) {
  const summary = `${item.relation === 'REPLY' ? '回复' : '主评论'} · 赞 ${item.likeCount ?? '未知'} · ${item.body?.slice(0, 80) || '原文未知'}${replies.length > 0 ? ` · ${replies.length} 条已采回复` : ''}`;
  return (
    <details className={styles.commentRow}>
      <summary>{summary}</summary>
      <p className={styles.text}>{item.body || '尚未取得评论文字'}</p>
      {orphan ? <p className={styles.note}>父评论尚未读到，暂不拼接其内容。</p> : null}
      {replies.length > 0 ? (
        <div className={styles.replies}>
          {replies.map(reply => (
            <Row key={reply.sourceRef} item={reply} replies={[]} orphan={false} />
          ))}
        </div>
      ) : null}
    </details>
  );
}

type State = { phase: 'idle' | 'loading' | 'error'; items: readonly DossierComment[]; cursor: string | null; total: number | null; loadedOnce: boolean };

/**
 * 底稿里的「已采评论」：经授权评论通道按页读取（每页 30 条），主评论下嵌回复；
 * 数量相同不代表已采全——平台曾显示的条数另行写明，不推算、不合并。
 */
export function DossierComments({ work, domain, platformComments }: { work: string; domain: string; platformComments: Metric }) {
  const [state, setState] = useState<State>({ phase: 'idle', items: [], cursor: null, total: null, loadedOnce: false });
  const controller = useRef<AbortController | null>(null);
  useEffect(() => () => controller.current?.abort(), []);

  function load() {
    controller.current?.abort();
    const abort = new AbortController();
    controller.current = abort;
    setState(previous => ({ ...previous, phase: 'loading' }));
    fetchDossierComments(work, domain, state.cursor, abort.signal).then(
      page => {
        if (abort.signal.aborted) return;
        setState(previous => ({ phase: 'idle', items: [...previous.items, ...page.items.filter(item => !previous.items.some(known => known.sourceRef === item.sourceRef))], cursor: page.nextCursor, total: page.total, loadedOnce: true }));
      },
      error => {
        if (!abort.signal.aborted) setState(previous => hideCachedReading(error) ? { phase: 'error', items: [], cursor: null, total: null, loadedOnce: false } : { ...previous, phase: 'error' });
      },
    );
  }

  const nodes = nestComments(state.items);
  const status = state.loadedOnce
    ? `已展示 ${state.items.length} / ${state.total ?? '未知'} 条可读取评论；平台曾显示 ${metricReading(platformComments)} 条。数量相同不代表已采全。`
    : '按已取得的来源稳定顺序读取，不代表平台展示顺序。';
  return (
    <Section title="已采评论">
      <p className={styles.note}>{status}</p>
      <div className={styles.panel}>
        {nodes.map(node => (
          <Row key={node.item.sourceRef} item={node.item} replies={node.replies} orphan={node.orphan} />
        ))}
      </div>
      {state.phase === 'error' ? <p className={styles.note}>评论读取暂不可用，底稿其他部分仍可使用。</p> : null}
      {state.loadedOnce && state.cursor === null ? null : (
        <button type="button" disabled={state.phase === 'loading'} onClick={load}>
          {state.loadedOnce ? '继续读取 30 条' : '读取前 30 条'}
        </button>
      )}
    </Section>
  );
}
