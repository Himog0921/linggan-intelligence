import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { ApiError } from '../../../shared/api';
import { ActionButton } from '../../../shared/ActionButton';
import type { RequestIdentities } from '../../../shared/requestIdentity';
import { reason } from '../api';
import { Dialog, Status } from '../ui';
import { adjudicate, getSpotCheck, labels, reviewDimensions, type ReviewCommand, type ReviewDimension } from './api';
import css from './Lens.module.css';

type Sample = Awaited<ReturnType<typeof getSpotCheck>>;
type Decisions = Partial<Record<ReviewDimension, { value: string; snapshotRef: string }>>;
type ReviewProps = {
  decisions?: Decisions;
  item: Sample['items'][number];
  vocabulary: Sample['vocabulary'];
  busy: boolean;
  stale: boolean;
  dimension: ReviewDimension;
  selectDimension: (dimension: ReviewDimension) => void;
  submit: (dimension: ReviewDimension, value: string, reason?: string) => void;
};
const confidenceNames: Record<string, string> = { high: '高', medium: '中', low: '低' };

function TitleReview({ item, decisions, vocabulary, busy, stale, dimension, selectDimension, submit }: ReviewProps) {
  const effective = (dim: ReviewDimension) => decisions?.[dim]?.value ?? item.effective[dim];
  const [choice, setChoice] = useState(effective(dimension));
  const [explanation, setExplanation] = useState('');
  const reasonTooLong = Array.from(explanation.trim()).length > 300;
  const name = (dim: ReviewDimension, key: string) => vocabulary[dim].find(v => v.key === key)?.name ?? '未知分类';
  function changeDimension(dim: ReviewDimension) {
    selectDimension(dim);
    setChoice(effective(dim));
    setExplanation('');
  }
  return <>
    <h3>{item.title ?? '标题未取得'}</h3>
    <p>机器标注把握程度：{confidenceNames[item.confidence] ?? '未知'}</p>
    <table className={css.effects}>
      <thead><tr><th>维度</th><th>冻结机器值</th><th>生效值</th><th>本轮裁定</th></tr></thead>
      <tbody>{reviewDimensions.map(dim => <tr key={dim}>
        <th scope="row">{labels[dim]}</th><td>{name(dim, item.machine[dim])}</td><td>{name(dim, effective(dim))}</td><td>{decisions?.[dim] ? '已记录' : '待裁定'}</td>
      </tr>)}</tbody>
    </table>
    <form className={css.reviewControls} onSubmit={event => {
      event.preventDefault();
      if (!busy && !stale && !reasonTooLong) submit(dimension, choice, explanation.trim() || undefined);
    }}>
      <label>裁定维度 <select value={dimension} disabled={busy} onChange={e => changeDimension(e.target.value as ReviewDimension)}>
        {reviewDimensions.map(dim => <option key={dim} value={dim}>{labels[dim]}</option>)}
      </select></label>
      <label>裁定分类 <select value={choice} disabled={busy} onChange={e => setChoice(e.target.value)}>
        {vocabulary[dimension].map(v => <option key={v.key} value={v.key}>{v.name}</option>)}
      </select></label>
      <label>理由（选填） <input value={explanation} disabled={busy} onChange={e => setExplanation(e.target.value)} /></label>
      {reasonTooLong && <span role="alert">理由最多可填 300 字。</span>}
      <ActionButton type="submit" disabled={busy || stale || reasonTooLong || !vocabulary[dimension].some(v => v.key === choice)}>
        {busy ? '正在提交…' : choice === effective(dimension) ? '确认当前分类' : '保存改正'}
      </ActionButton>
    </form>
  </>;
}

export function SpotList({ snapshotRef, domain, ids, close }: {
  snapshotRef: string; domain: string; ids: RequestIdentities; close: () => void;
}) {
  const client = useQueryClient();
  const [dimension, setDimension] = useState<ReviewDimension>('moment');
  const [index, setIndex] = useState(0);
  const [decisions, setDecisions] = useState<Record<string, Decisions>>({});
  const query = useQuery({ queryKey: ['ops', 'content-lens', 'spot-check', snapshotRef], retry: false, staleTime: Infinity,
    queryFn: ({ signal }) => getSpotCheck(snapshotRef, signal) });
  const mutation = useMutation({
    mutationFn: async (command: ReviewCommand) => {
      const fingerprint = JSON.stringify(command);
      const action = 'adjudication:' + fingerprint;
      const receipt = await adjudicate(command, ids.keyFor(action, fingerprint));
      ids.settle(action);
      return receipt;
    },
    onSuccess: async (receipt, command) => {
      setDecisions(previous => ({
        ...previous,
        [command.titleKey]: {
          ...previous[command.titleKey],
          [command.dimension]: { value: command.value, snapshotRef: receipt.snapshotRef },
        },
      }));
      await client.invalidateQueries({ queryKey: ['ops', 'content-lens', 'snapshot', domain], exact: true });
    },
  });
  const items = query.data?.items ?? [];
  const item = items[index];
  const busy = mutation.isPending;
  const failure = mutation.error instanceof ApiError ? mutation.error.failure : null;
  const conflict = failure?.kind === 'http' && failure.code === 'ops_lens_adjudication_conflict';

  const dismiss = () => { if (!busy) close(); };
  function navigate(nextIndex: number) {
    setIndex(nextIndex);
    mutation.reset();
  }
  return <Dialog label="人工抽看样本" onClose={dismiss} className={css.dialog}>
    <header className={css.heading}><h2>人工抽看样本</h2><ActionButton disabled={busy} onClick={dismiss}>关闭</ActionButton></header>
    {conflict ? <p role="alert">裁定发生冲突。请关闭抽看，点击“生成当前范围快照”，再重新打开抽看样本。</p>
      : mutation.isError && <p role="alert">{failure?.kind === 'http' ? '裁定未保存' : '裁定未确认'}（{reason(mutation.error)}）。可再次提交。</p>}
    {mutation.isSuccess && <p role="status">裁定已记录。</p>}
    {query.isError ? <Status error retry={() => void query.refetch()}>抽看样本读取失败（{reason(query.error)}）。</Status>
      : query.isPending ? <Status>正在读取样本…</Status>
      : !item ? <p>暂无抽看样本。</p> : <>
        <nav className={css.controls} aria-label="抽看样本翻页">
          <ActionButton disabled={busy || conflict || index === 0} onClick={() => navigate(index - 1)}>上一条</ActionButton>
          <span>第 {index + 1} 条，共 {items.length} 条</span>
          <ActionButton disabled={busy || conflict || index + 1 >= items.length} onClick={() => navigate(index + 1)}>下一条</ActionButton>
        </nav>
        <TitleReview key={item.key + ':' + (decisions[item.key]?.[dimension]?.snapshotRef ?? snapshotRef)} decisions={decisions[item.key]} dimension={dimension} selectDimension={setDimension} item={item} vocabulary={query.data.vocabulary} busy={busy} stale={conflict}
          submit={(dimension, value, reason) => {
            const source = decisions[item.key]?.[dimension]?.snapshotRef ?? snapshotRef;
            mutation.mutate({ snapshotRef: source, titleKey: item.key, dimension, value, reason });
          }} />
      </>}
  </Dialog>;
}
