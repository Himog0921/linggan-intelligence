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
import { FeedbackLine, Section, SourceNote } from './Primitives';
import page from './EvidencePage.module.css';
import styles from './Records.module.css';

/** 评论采集历史在这里最多列几次；更早的折起来。 */
const HISTORY_SHOWN = 5;
/** 「采指定条数」默认填的条数。 */
const DEFAULT_TARGET = '100';

function notFound(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && error.failure.status === 404;
}

function count(value: number | null): string {
  return value === null ? '未知' : formatNumber(value);
}

function historyCopy(row: CommentCollectionRow): string {
  const shown = row.shown === null ? '' : `，当时页面显示 ${formatNumber(row.shown)} 条`;
  // 「尽量采全」走到了评论区尽头就算采全，页面上的总数可能含已删除或看不到的评论，所以不说「已采全 N 条」。
  const complete = row.complete === true ? ' · 能看到的都采了' : row.complete === false ? ' · 没有采全' : '';
  return `${momentText(row.at)} · ${row.scope} · 采到 ${count(row.collected)} 条${shown}${complete}`;
}

function intentTarget(intent: CommentDeepIntent): number | null {
  return 'commentTarget' in intent ? intent.commentTarget : null;
}

/**
 * 「评论深采」（Mog 2026-10-23）：先说这篇评论现在采到多少、以前怎么采的；再选「采全部评论」或「采指定条数」。
 * 动作走采集页同一条单次采集合同（`POST /api/collection/runs`，语料已选作品；尽量采全或采到 N 条就停），
 * 确认后才发；回执分已提交／没有建立／未确认，进度读同一次采集的汇总，不在本页伪造完成。
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
  const roots = inspector.commentsCoverage?.retained ?? null;
  const replies = inspector.repliesCoverage?.retained ?? null;
  const target = mode === 'count' ? parseCommentTarget(targetText) : null;
  const invalidTarget = mode === 'count' && target === null;
  // 这一次已经建好且还没结束：不再重复建；结束了可以再采一次（新请求、新身份）。
  const running = runRef !== null && run.data?.ended !== true;
  const locked = sending || uncertain || running;

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
      setResult(feedback('success', '评论深采任务已建立，要等工位执行；这还不是已经采到。', null, '已提交'));
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
    <Section title="评论深采">
      <p className={styles.lead}>
        {`平台显示 ${metricReading(item.display.engagement.comment)} 条评论 · 最近记录：评论 ${count(roots)} 条、回复 ${count(replies)} 条`}
      </p>
      {history.length === 0 ? (
        <SourceNote>还没有采过这篇的评论。</SourceNote>
      ) : (
        <ol className={styles.history} aria-label="评论采集记录">
          {history.slice(0, HISTORY_SHOWN).map((row, index) => (
            <li key={`${row.at ?? 'unknown'}:${index}`}>{historyCopy(row)}</li>
          ))}
          {history.length > HISTORY_SHOWN ? <li className={styles.more}>{`还有 ${history.length - HISTORY_SHOWN} 次更早的评论采集`}</li> : null}
        </ol>
      )}
      {item.platform !== 'xhs' ? (
        <SourceNote>评论深采目前只支持小红书笔记。</SourceNote>
      ) : (
        <>
          <fieldset className={styles.modes} disabled={locked}>
            <legend className="v7-sr-only">评论深采范围</legend>
            <label className={styles.mode}>
              <input type="radio" name={modeName} checked={mode === 'all'} onChange={() => setMode('all')} />
              <span>采全部评论</span>
              <small>把能看到的评论和回复尽量采全</small>
            </label>
            <label className={styles.mode}>
              <input type="radio" name={modeName} checked={mode === 'count'} onChange={() => setMode('count')} />
              <span>采指定条数</span>
              <small>采到这么多条就停（含回复），评论区先到底也会停</small>
            </label>
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
                  onChange={event => setTargetText(event.target.value)}
                />
                <small id={`${modeName}-hint`}>
                  {invalidTarget ? `请填 ${MIN_COMMENT_TARGET}–${MAX_COMMENT_TARGET} 之间的整数` : `${MIN_COMMENT_TARGET}–${MAX_COMMENT_TARGET} 条，含回复`}
                </small>
              </div>
            ) : null}
          </fieldset>
          <p className={styles.scope}>让插件重新打开这篇笔记去采；评论多的笔记会分几段采，每段最多 30 条。</p>
          <div className={styles.actions}>
            {uncertain ? (
              <ActionButton variant="primary" busy={sending} onClick={() => void submit()}>
                核对原请求
              </ActionButton>
            ) : (
              <ActionButton variant="primary" busy={sending} disabled={running || invalidTarget} onClick={() => setConfirming(true)}>
                {running ? '评论深采已提交' : mode === 'count' ? `采 ${target ?? '指定'} 条评论` : '采全部评论'}
              </ActionButton>
            )}
          </div>
          <FeedbackLine feedback={result} />
          {runRef !== null ? (
            <div className={styles.runProgress} role="status">
              <span>
                评论深采任务：<b>{status}</b>
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
