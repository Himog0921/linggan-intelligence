import { useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ObservationHistory } from './ObservationHistory';
import { ReobservationStatus } from './ReobservationStatus';
import { ReobserveConfirmDialog } from './ReobserveConfirmDialog';
import { Section } from './Primitives';
import { TIMELINE_WINDOW } from './observationRecords';
import { reobservationAccess } from './reobservationAccess';
import type { Inspector, Reobservation as Channel, WorkItem } from './evidenceTypes';
import type { Reobservation } from './useReobservation';
import styles from './Records.module.css';

interface ObservationRecordSectionProps {
  item: WorkItem;
  inspector: Inspector;
  channel: Channel;
  domain: string;
  state: Reobservation;
  /** 重新读取作品详情；来源轨迹里出现工单引用即说明已入队。 */
  onRecheck: () => void;
  now?: number;
}

/**
 * 「观察记录」：同一行标题答「看过几次」并给出复观测入口。次数是原 timeline 窗口的记录数，
 * 不是合并后的表格行数；满窗口时显示「100+」并说明只列最近 100 次。复观测不再另起大标题，
 * 不可用时只留紧凑原因，可用时保留原确认弹窗与执行状态。
 */
export function ObservationRecordSection({ item, inspector, channel, domain, state, onRecheck, now }: ObservationRecordSectionProps) {
  const [confirming, setConfirming] = useState(false);
  const access = reobservationAccess(item, channel);
  const total = inspector.timeline.length;
  const countText = total >= TIMELINE_WINDOW ? `${TIMELINE_WINDOW}+` : String(total);
  const countLabel = total >= TIMELINE_WINDOW ? `至少 ${TIMELINE_WINDOW} 次，仅列最近 ${TIMELINE_WINDOW} 次` : `${total} 次观察记录`;
  const action = access.actionPath !== null ? (
    <ActionButton
      className={styles.reobserveButton}
      disabled={state.queued}
      busy={state.sending}
      onClick={() => setConfirming(true)}
    >
      {state.queued ? '复观测已入队' : state.sending ? '提交中…' : '复观测'}
    </ActionButton>
  ) : undefined;
  return (
    <Section
      title="观察记录"
      badge={(
        <>
          <span className={styles.count} aria-hidden="true">{countText}</span>
          <span className="v7-sr-only">{countLabel}</span>
        </>
      )}
      actions={action}
    >
      <ReobservationStatus domain={domain} state={state} onRecheck={onRecheck} />
      <ObservationHistory item={item} inspector={inspector} now={now} />
      {access.actionPath === null ? (
        <p className={styles.reobserveReason}><b>复观测</b>：{access.reason}</p>
      ) : null}
      {confirming && access.actionPath !== null ? (
        <ReobserveConfirmDialog
          afterUnknown={state.feedback?.tone === 'unknown'}
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
