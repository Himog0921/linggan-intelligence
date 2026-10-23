import { ActionButton } from '../../../shared/ActionButton';
import { useState } from 'react';
import { OperationBlock } from './ReobservationOperation';
import { FeedbackLine, InlineState, Section, SourceNote } from './Primitives';
import { ReobserveConfirmDialog } from './ReobserveConfirmDialog';
import { errorCode } from './evidenceFeedback';
import { reobservationAccess } from './reobservationAccess';
import type { Reobservation as Channel, WorkItem } from './evidenceTypes';
import type { Reobservation } from './useReobservation';
import page from './EvidencePage.module.css';
import styles from './Reobservation.module.css';

interface ReobservationSectionProps {
  item: Pick<WorkItem, 'platform'>;
  channel: Channel;
  domain: string;
  state: Reobservation;
  /** 重新读取作品详情；来源轨迹里出现工单引用即说明已入队。 */
  onRecheck: () => void;
}

/**
 * 「复观测」：要不要现在再看一次这篇笔记。范围用人话常驻（不放 tooltip）。点击先弹确认，确认之后才发请求；
 * 回执分四种结局显示。未确认之后不自动重发，只提供「重新核对」和「查看采集任务」；确需再提交时会再次确认，并写明后果。
 * 不可用时只说原因；原因码只留在技术信息里（Mog 2026-10-23：普通层不放机器语言）。
 */
export function ReobservationSection({ item, channel, domain, state, onRecheck }: ReobservationSectionProps) {
  const [confirming, setConfirming] = useState(false);
  const access = reobservationAccess(item, channel);
  if (access.actionPath === null) {
    return (
      <Section title="复观测">
        <SourceNote>{access.reason}</SourceNote>
      </Section>
    );
  }
  const unknown = state.feedback?.tone === 'unknown';
  return (
    <Section title="复观测">
      <p className={styles.scope}>重新打开这篇笔记，更新点赞、评论、收藏、分享，顺带采最多 30 条评论和回复；不重新下载图片视频，也不重新识字。</p>
      <div className={styles.actions}>
        <ActionButton variant="primary" disabled={state.queued || state.sending} onClick={() => setConfirming(true)}>
          {state.queued ? '复观测已入队' : state.sending ? '提交中…' : '立即复观测'}
        </ActionButton>
      </div>
      <FeedbackLine feedback={state.feedback}>
        {unknown ? (
          <>
            <button type="button" className={page.textButton} onClick={onRecheck}>
              重新核对
            </button>
            <a href={`/collection/tasks?domain=${encodeURIComponent(domain)}`}>查看采集任务</a>
          </>
        ) : null}
      </FeedbackLine>
      {state.readError ? (
        <InlineState tone="danger" title="复观测状态已更新，但作品当前事实未重新读取" code={errorCode(state.readError) ?? String(state.readError)}>
          保留已知的任务、尝试、包与回执；没有把检查器的旧快照写成新事实。
        </InlineState>
      ) : null}
      {state.pollError ? (
        <InlineState tone="danger" title="复观测状态读取失败" code={errorCode(state.pollError) ?? 'REOBSERVATION_STATUS_UNAVAILABLE'}>
          已保留上一次已知状态；页面没有把未知状态写成完成。
        </InlineState>
      ) : null}
      {state.operation ? <OperationBlock operation={state.operation} /> : null}
      {confirming ? (
        <ReobserveConfirmDialog
          afterUnknown={unknown}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            state.submit();
          }}
        />
      ) : null}
    </Section>
  );
}
