import type { Status } from './evidenceStatus';
import { Tech } from './Primitives';
import page from './EvidencePage.module.css';

const ACTIONS = { 'retry-shell': '重试读取', 'retry-list': '重试读取', 'clear-scope': '清除这个筛选条件' } as const;

/** 结果区的状态块（读取中、空、失败、地址无效）。`role=status`，读屏会播报；失败带「重试读取」。 */
export function ResultsStatus({ status, onAction }: { status: Status; onAction: (action: 'retry-shell' | 'retry-list' | 'clear-scope') => void }) {
  return (
    <div className={page.feedback} role="status" aria-live="polite" data-kind={status.kind} aria-busy={status.kind === 'loading'}>
      <h2>{status.title}</h2>
      <p>{status.detail}</p>
      {status.reason ? <p>{status.reason}</p> : null}
      <div className={page.feedbackActions}>
        {status.code ? <Tech>{status.code}</Tech> : null}
        {status.action === 'manage-domains' ? <a href="/collection/domains" data-domain-manage="">前往领域管理 →</a> : null}
        {status.action && status.action !== 'manage-domains' ? (
          <button type="button" className={page.textButton} onClick={() => onAction(status.action as keyof typeof ACTIONS)}>
            {ACTIONS[status.action]}
          </button>
        ) : null}
      </div>
    </div>
  );
}
