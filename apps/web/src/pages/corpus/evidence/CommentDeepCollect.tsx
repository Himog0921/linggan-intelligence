import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Link } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { createRun, definiteRejection, type RunRequest } from '../../collection/tasks/runRequests';
import { errorText, readRun, runStatus } from '../../collection/tasks/tasksApi';
import { CommentDeepConfirmDialog } from './CommentDeepConfirmDialog';
import { commentDeepIntent, commentDeepRequests as requests } from './commentDeepRequests';
import { feedback, type Feedback } from './evidenceFeedback';
import { formatNumber, metricReading, momentText } from './evidencePresent';
import type { Inspector, WorkItem } from './evidenceTypes';
import { commentCollectionRows, type CommentCollectionRow } from './observationRecords';
import { FeedbackLine, Section, SourceNote } from './Primitives';
import page from './EvidencePage.module.css';
import styles from './Records.module.css';

/** 评论采集历史在这里最多列几次；更早的折起来。 */
const HISTORY_SHOWN = 5;

function notFound(error: unknown): boolean {
  return error instanceof ApiError && error.failure.kind === 'http' && error.failure.status === 404;
}

function sum(left: number | null, right: number | null): number | null {
  return left === null || right === null ? null : left + right;
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

function RunProgress({ runRef, domain }: { runRef: string; domain: string }) {
  const run = useQuery({ queryKey: ['evidence-comment-deep-run', domain, runRef], queryFn: ({ signal }) => readRun(runRef, domain, signal), retry: false, gcTime: 0, refetchOnWindowFocus: false });
  const status = run.data ? `${runStatus(run.data)} · 这次新采到 ${count(run.data.new_comments)} 条` : run.isError ? '进度暂时读不出来' : '正在读取进度…';
  return (
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
  );
}

/**
 * 「评论深采」（Mog 2026-10-23）：先说这篇评论现在采到多少、以前采过几次；再给「采全部评论」。
 * 动作走采集页同一条单次采集合同（`POST /api/collection/runs`，语料已选作品、尽量采全），确认后才发；
 * 回执分已提交／没有建立／未确认，进度读同一次采集的汇总，不在本页伪造完成。
 */
export function CommentDeepCollect({ item, inspector, domain }: { item: WorkItem; inspector: Inspector; domain: string }) {
  const key = `${domain}:${item.ref}`;
  const remembered = requests.get(key);
  const [confirming, setConfirming] = useState(false);
  const [sending, setSending] = useState(false);
  const [runRef, setRunRef] = useState<string | null>(remembered?.runRef ?? null);
  const [uncertain, setUncertain] = useState(Boolean(remembered && remembered.runRef === null && remembered.request.sent));
  const [result, setResult] = useState<Feedback | null>(null);
  const history = commentCollectionRows(inspector.commentsHistory);
  const roots = inspector.commentsCoverage?.retained ?? null;
  const replies = inspector.repliesCoverage?.retained ?? null;

  async function submit() {
    if (sending) return;
    const intent = commentDeepIntent(domain, item.ref);
    const stored = requests.get(key);
    const request: RunRequest = stored && stored.runRef === null ? stored.request : { identity: crypto.randomUUID(), fingerprint: JSON.stringify(intent), sent: false };
    requests.set(key, { request, runRef: null });
    const wasSent = request.sent;
    setSending(true);
    setResult(null);
    try {
      const receipt = await createRun(request, intent);
      requests.set(key, { request, runRef: receipt.runRef });
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

  return (
    <Section title="评论深采">
      <p className={styles.lead}>
        {`平台显示 ${metricReading(item.display.engagement.comment)} 条评论 · 已采到 ${count(sum(roots, replies))} 条`}
        {roots !== null && replies !== null ? `（评论 ${formatNumber(roots)}、回复 ${formatNumber(replies)}）` : ''}
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
          <p className={styles.scope}>让插件重新打开这篇笔记，把能看到的评论和回复尽量采全；评论多的笔记会分几段采。</p>
          <div className={styles.actions}>
            {uncertain ? (
              <ActionButton variant="primary" busy={sending} onClick={() => void submit()}>
                核对原请求
              </ActionButton>
            ) : (
              <ActionButton variant="primary" busy={sending} disabled={runRef !== null} onClick={() => setConfirming(true)}>
                {runRef !== null ? '评论深采已提交' : '采全部评论'}
              </ActionButton>
            )}
            {uncertain ? (
              <button type="button" className={page.textButton} disabled={sending} onClick={() => setConfirming(true)}>
                仍要重新提交
              </button>
            ) : null}
          </div>
          <FeedbackLine feedback={result} />
          {runRef !== null ? <RunProgress runRef={runRef} domain={domain} /> : null}
        </>
      )}
      {confirming ? (
        <CommentDeepConfirmDialog
          afterUnknown={uncertain}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            if (uncertain) {
              // 用户看过「可能已经建立」的提示仍要再提交：放弃原请求身份，按新请求提交。
              requests.delete(key);
              setUncertain(false);
            }
            void submit();
          }}
        />
      ) : null}
    </Section>
  );
}
