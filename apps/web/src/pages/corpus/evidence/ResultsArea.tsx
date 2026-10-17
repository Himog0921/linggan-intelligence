import type { Ref } from 'react';
import { Tech } from './Primitives';
import { ResultsStatus } from './ResultsStatus';
import { WorkList } from './WorkList';
import { errorCode, readErrorText } from './evidenceFeedback';
import type { LayoutKey } from './evidenceQuery';
import { describeStatus, type StatusInput } from './evidenceStatus';
import type { WorkItem, WorkList as WorkListPage } from './evidenceTypes';
import styles from './EvidencePage.module.css';

/** 读取回执条：只在这次读取做了列表本身说不清的事时出现；平时读取时间在页头，已经足够。 */
export function ReadReceipt({ last, failure, count }: { last: WorkListPage | null; failure: unknown; count: number }) {
  const notes: string[] = [];
  if (last?.scanLimited) notes.push('扫描预算已触发，当前结果不是全部匹配；可继续读取');
  if (last?.truncated && !last.cursor) notes.push('本次读取已截断，但来源没有给出继续读取的游标');
  if (last && notes.length > 0) {
    return (
      <div className={styles.receipt}>
        <span>{notes.join(' · ')}</span>
        <span className={styles.receiptMeta}>
          <Tech>{`扫描 ${last.scannedCount ?? '未知'}`}</Tech>
        </span>
      </div>
    );
  }
  if (failure && count === 0) {
    const code = errorCode(failure);
    return (
      <div className={styles.receipt}>
        <span>当前未读取任何材料，不能据此判断库为空或来源不存在。</span>
        <span className={styles.receiptMeta}>{code ? <Tech>{code}</Tech> : null}</span>
      </div>
    );
  }
  return null;
}

export interface ListState {
  /** 首次读取进行中。 */
  pending: boolean;
  error: unknown;
  hasNext: boolean;
  fetchingNext: boolean;
  /** 追加读取失败（已读到的作品仍保留）。 */
  nextFailed: boolean;
  loadMore: () => void;
  retry: () => void;
}

interface ResultsAreaProps {
  shell: StatusInput['shell'];
  domainSelected: boolean;
  problem: StatusInput['scopeProblem'];
  list: ListState;
  items: readonly WorkItem[];
  selectedRef: string | null;
  layout: LayoutKey;
  checked: ReadonlySet<string>;
  listRef: Ref<HTMLDivElement>;
  onSelect: (item: WorkItem, source: 'user' | 'auto') => void;
  onCheck: (ref: string, checked: boolean) => void;
  onRetryShell: () => void;
  onClearScope: () => void;
}

/**
 * 结果区：状态块（读取中、空、失败、地址无效）或作品列表，以及「继续读取作品」。读取失败如实说，不把失败
 * 显示成空列表；追加失败时已读到的作品保留，在列表下方给内联说明，同一个按钮即重试。
 */
export function ResultsArea(props: ResultsAreaProps) {
  const { shell, domainSelected, problem, list, items, selectedRef, layout, checked, listRef, onSelect, onCheck, onRetryShell, onClearScope } = props;
  const status = describeStatus({ shell, domainSelected, scopeProblem: problem, list: { pending: list.pending, error: list.error, count: items.length } });
  return (
    <div className={styles.results}>
      {status ? (
        <ResultsStatus
          status={status}
          onAction={action => {
            if (action === 'retry-shell') onRetryShell();
            else if (action === 'retry-list') list.retry();
            else onClearScope();
          }}
        />
      ) : null}
      {items.length > 0 ? (
        <WorkList ref={listRef} items={items} layout={layout} selectedRef={selectedRef} checked={checked} canCheck={domainSelected} onSelect={onSelect} onCheck={onCheck} />
      ) : null}
      {items.length > 0 && list.hasNext ? (
        <div className={styles.footer}>
          {list.nextFailed ? (
            <p className={styles.footerError} role="status">
              {`继续读取没有成功，已读到的作品仍保留，可以再试一次。${readErrorText(list.error) ?? ''}`}
              {errorCode(list.error) ? <Tech>{errorCode(list.error)}</Tech> : null}
            </p>
          ) : null}
          <button type="button" disabled={list.fetchingNext} onClick={list.loadMore}>
            继续读取作品
          </button>
        </div>
      ) : null}
    </div>
  );
}
