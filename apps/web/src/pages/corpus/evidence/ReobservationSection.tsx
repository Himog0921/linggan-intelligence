import { useState } from 'react';
import { OperationBlock } from './ReobservationOperation';
import { FeedbackLine, InlineState, Section, SourceNote, Tech } from './Primitives';
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

const MEDIA_POLICY: Readonly<Record<string, string>> = { EXISTING_ASSETS_REUSED: '媒体：复用既有副本，不新取' };

/**
 * 「立即复观测」。范围说明常驻（不放 tooltip）。点击先弹确认，确认之后才发请求；回执分四种结局显示。
 * 未确认之后不自动重发，只提供「重新核对」和「查看采集任务」；确需再提交时会再次确认，并写明后果。
 */
export function ReobservationSection({ item, channel, domain, state, onRecheck }: ReobservationSectionProps) {
  const [confirming, setConfirming] = useState(false);
  const access = reobservationAccess(item, channel);
  if (access.actionPath === null) {
    return (
      <Section title="立即复观测">
        <SourceNote>{access.reason}</SourceNote>
        <Tech>{channel.reason}</Tech>
      </Section>
    );
  }
  const unknown = state.feedback?.tone === 'unknown';
  return (
    <Section title="立即复观测">
      <p className={styles.scope}>范围：详情、评论（最多 30 条）与回复；不下载媒体、不跑 OCR / 转录。</p>
      <div className={styles.actions}>
        <button type="button" className={page.primary} disabled={state.queued || state.sending} onClick={() => setConfirming(true)}>
          {state.queued ? '复观测已入队' : state.sending ? '提交中…' : '立即复观测'}
        </button>
        <span className={styles.policy}>{(channel.mediaPolicy ? MEDIA_POLICY[channel.mediaPolicy] : undefined) ?? (channel.mediaPolicy ? <Tech>{channel.mediaPolicy}</Tech> : '媒体策略未知')}</span>
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
