import { useQuery } from '@tanstack/react-query';
import { useId, useState } from 'react';
import { Link } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { createRun, definiteRejection, type RunRequest } from '../../collection/tasks/runRequests';
import { errorText, readRun, runStatus } from '../../collection/tasks/tasksApi';
import { CommentDeepConfirmDialog } from './CommentDeepConfirmDialog';
import {
  MAX_COMMENT_TARGET,
  MIN_COMMENT_TARGET,
  commentDeepIntent,
  commentDeepRequests as requests,
  parseCommentTarget,
  type CommentDeepIntent,
} from './commentDeepRequests';
import { feedback, type Feedback } from './evidenceFeedback';
import { formatNumber, metricReading, momentText } from './evidencePresent';
import type { Inspector, WorkItem } from './evidenceTypes';
import { commentCollectionRows, type CommentCollectionRow } from './observationRecords';
import { FeedbackLine, Section } from './Primitives';
import page from './EvidencePage.module.css';
import styles from './Records.module.css';

/** 评论采集历史在这里最多列几次；更早的折起来，展开仍是本地已有记录。 */
const HISTORY_SHOWN = 5;
/** 「采指定条数」默认填的条数。 */
const DEFAULT_TARGET = '100';

function notFound(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && error.failure.status === 404;
}

function count(value: number | null): string {
  return value === null ? '未知' : formatNumber(value);
}

function intentTarget(intent: CommentDeepIntent): number | null {
  return 'commentTarget' in intent ? intent.commentTarget : null;
}

function HistoryList({ rows, label }: { rows: readonly CommentCollectionRow[]; label: string }) {
  return (
    <ol className={styles.history} aria-label={label}>
      {rows.map((row, index) => (
        <li key={`${row.at ?? 'unknown'}:${index}`} className={styles.historyItem}>
          <time className={styles.historyDate} dateTime={row.at ?? undefined}>{momentText(row.at, '时间未知')}</time>
          <span className={styles.historyScope}>{row.scope}</span>
          <span className={styles.historyCount}>采到 {count(row.collected)} 条</span>
          {row.shown !== null ? <span className={styles.historyShown}>页面显示 {formatNumber(row.shown)}</span> : null}
          {row.result ? (
            <span className={styles.historyResult} data-result={row.complete === true ? 'complete' : row.complete === false ? 'partial' : 'unknown'}>
              {row.result}
            </span>
          ) : null}
        </li>
      ))}
    </ol>
  );
}

/**
 * 「评论采集」（Mog 2026-10-23、2026-11-02）：先说平台显示多少、累计已采多少；再选「全部可见」或「指定条数」。
 * 动作走采集页同一条单次采集合同（`POST /api/collection/runs`，语料已选作品；尽量采全或采到 N 条就停），
 * 确认后才发；回执分已提交／没有建立／未确认，进度读同一次采集的汇总，不在本页伪造完成。
 * 历史只重排接口已给的记录；空历史只代表暂无记录，不推断没有采过。
 */
export function CommentDeepCollect({ item, inspector, domain }: { item: WorkItem; inspector: Inspector; domain: string }) {
  const key = `${domain}:${item.ref}`;
  const remembered = requests.get(key);
  const modeName = useId();
  const [confirming, setConfirming] = useState(false);
  const [sending, setSending] = useState(false);
  const [mode, setMode] = useState<'all' | 'count'>(() => (remembered && intentTarget(remembered.intent) !== null ? 'count' : 'all'));
  const [targetText, setTargetText] = useState(() => {
    const target = remembered ? intentTarget(remembered.intent) : null;
    return target === null ? DEFAULT_TARGET : String(target);
  });
  const [runRef, setRunRef] = useState<string | null>(remembered?.runRef ?? null);
  const [uncertain, setUncertain] = useState(Boolean(remembered && remembered.runRef === null && remembered.request.sent));
  const [result, setResult] = useState<Feedback | null>(null);
  const run = useQuery({
    queryKey: ['evidence-comment-deep-run', domain, runRef],
    queryFn: ({ signal }) => readRun(runRef!, domain, signal),
    enabled: runRef !== null,
    retry: false,
    gcTime: 0,
    refetchOnWindowFocus: false,
  });
  const history = commentCollectionRows(inspector.commentsHistory);
  const cumulative = inspector.commentsReceipt?.total ?? null;
  const target = mode === 'count' ? parseCommentTarget(targetText) : null;
  const invalidTarget = mode === 'count' && target === null;
  // 这一次已经建好且还没结束：不再重复建；结束了可以再采一次（新请求、新身份）。
  const running = runRef !== null && run.data?.ended !== true;
  const locked = sending || uncertain || running;
  const recentHistory = history.slice(0, HISTORY_SHOWN);
  const olderHistory = history.slice(HISTORY_SHOWN);

  async function submit() {
    if (sending) return;
    const stored = requests.get(key);
    // 未确认的原请求：沿用它的身份与内容去核对回执，不按现在的选择另建一个。
    const retry = stored && stored.runRef === null ? stored : null;
    const intent = retry?.intent ?? commentDeepIntent(domain, item.ref, target);
    const request: RunRequest = retry?.request ?? { identity: crypto.randomUUID(), fingerprint: JSON.stringify(intent), sent: false };
    try {
      requests.set(key, { request, intent, runRef: null });
    } catch {
      setResult(feedback('error', '无法保存请求身份，尚未提交。请允许浏览器会话存储后重试。'));
      return;
    }
    const wasSent = request.sent;
    setSending(true);
    setResult(null);
    try {
      const receipt = await createRun(request, intent);
      requests.set(key, { request, intent, runRef: receipt.runRef });
      setRunRef(receipt.runRef);
      setUncertain(false);
      setResult(feedback('success', '评论采集任务已建立，要等工位执行；这还不是已经采到。', null, '已提交'));
    } catch (error) {
      if ((!wasSent && definiteRejection(error)) || (wasSent && notFound(error))) {
        requests.delete(key);
        setUncertain(false);
        setResult(feedback('error', wasSent ? '原请求没有在服务器留下记录，没有建立。可以重新提交。' : `没有建立：${errorText(error)}。`));
      } else {
        setUncertain(true);
        setResult(feedback('unknown', '没有得到确认的答复，可能已经建立。点“核对原请求”会拿原请求去核对，不会重复建立。'));
      }
    } finally {
      setSending(false);
    }
  }

  const status = run.data
    ? `${runStatus(run.data)} · 本轮采回 ${count(run.data.received_comments)} 条 · 首次新增 ${count(run.data.new_comments)} 条`
    : run.isError ? '进度暂时读不出来' : '正在读取进度…';
  return (
    <Section title="评论采集">
      <p className={styles.lead}>
        <span>平台显示 {metricReading(item.display.engagement.comment)}</span>
        <span className="v7-sr-only"> 条评论</span>
        <span className={styles.separator} aria-hidden="true">·</span>
        <span>累计已采 {count(cumulative)}</span>
        <span className="v7-sr-only"> 条</span>
      </p>
      {item.platform !== 'xhs' ? (
        <p className={styles.unsupported}>评论采集目前只支持小红书笔记。</p>
      ) : (
        <>
          <fieldset className={styles.segments} disabled={locked}>
            <legend className="v7-sr-only">评论采集范围</legend>
            <label className={styles.segment}>
              <input type="radio" name={modeName} checked={mode === 'all'} onChange={() => setMode('all')} />
              <span>全部可见</span>
            </label>
            <label className={styles.segment}>
              <input type="radio" name={modeName} checked={mode === 'count'} onChange={() => setMode('count')} />
              <span>指定条数</span>
            </label>
          </fieldset>
          {mode === 'count' ? (
            <div className={styles.target}>
              <label htmlFor={`${modeName}-target`}>条数</label>
              <input
                id={`${modeName}-target`}
                type="number"
                inputMode="numeric"
                min={MIN_COMMENT_TARGET}
                max={MAX_COMMENT_TARGET}
                step={1}
                value={targetText}
                aria-invalid={invalidTarget}
                aria-describedby={`${modeName}-hint`}
                disabled={locked}
                onChange={event => setTargetText(event.target.value)}
              />
              <span className={styles.targetUnit}>条</span>
              <small id={`${modeName}-hint`} className={styles.targetHint}>
                {invalidTarget ? `请填 ${MIN_COMMENT_TARGET}–${MAX_COMMENT_TARGET} 之间的整数` : `${MIN_COMMENT_TARGET}–${MAX_COMMENT_TARGET} 的整数`}
              </small>
            </div>
          ) : null}
          <p className={styles.scope}>含回复；到达评论区末尾时结束</p>
          <div className={styles.actions}>
            {uncertain ? (
              <ActionButton variant="ink" busy={sending} onClick={() => void submit()}>
                核对原请求
              </ActionButton>
            ) : (
              <ActionButton variant="ink" busy={sending} disabled={running || invalidTarget} onClick={() => setConfirming(true)}>
                {running ? '评论采集已提交' : '采集评论'}
              </ActionButton>
            )}
          </div>
          <FeedbackLine feedback={result} />
          {runRef !== null ? (
            <div className={styles.runProgress} role="status">
              <span>
                评论采集任务：<b>{status}</b>
              </span>
              <span className={styles.runActions}>
                <button type="button" className={page.textButton} disabled={run.isFetching} onClick={() => void run.refetch()}>
                  刷新进度
                </button>
                <Link to={`/collection/tasks?domain=${encodeURIComponent(domain)}&run=${encodeURIComponent(runRef)}`}>在采集任务里查看</Link>
              </span>
            </div>
          ) : null}
        </>
      )}
      {history.length === 0 ? (
        <p className={styles.historyEmpty}>暂无采集记录</p>
      ) : (
        <>
          <HistoryList rows={recentHistory} label="评论采集记录" />
          {olderHistory.length > 0 ? (
            <details className={styles.older}>
              <summary>{`更早 ${olderHistory.length} 次`}</summary>
              <HistoryList rows={olderHistory} label="更早的评论采集记录" />
            </details>
          ) : null}
        </>
      )}
      {confirming ? (
        <CommentDeepConfirmDialog
          target={target}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            if (runRef !== null) {
              // 上一次已经结束，再采一次是新请求。
              requests.delete(key);
            }
            void submit();
          }}
        />
      ) : null}
    </Section>
  );
}
