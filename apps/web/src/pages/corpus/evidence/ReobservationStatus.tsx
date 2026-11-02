import { OperationBlock } from './ReobservationOperation';
import { FeedbackLine, InlineState } from './Primitives';
import { errorCode } from './evidenceFeedback';
import type { Reobservation } from './useReobservation';
import page from './EvidencePage.module.css';

interface ReobservationStatusProps {
  domain: string;
  state: Reobservation;
  /** 重新读取作品详情；来源轨迹里出现工单引用即说明已入队。 */
  onRecheck: () => void;
}

/**
 * 复观测的回执、读取失败与本次执行状态。放在「观察记录」正文之后，不再另起一节标题：
 * 普通层只说人话，原因码只留在技术信息里；未确认之后不自动重发，只提供「重新核对」和「查看采集任务」。
 */
export function ReobservationStatus({ domain, state, onRecheck }: ReobservationStatusProps) {
  const unknown = state.feedback?.tone === 'unknown';
  if (!state.feedback && !state.readError && !state.pollError && !state.operation) return null;
  return (
    <>
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
    </>
  );
}
