import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { ActionButton } from '../../shared/ActionButton';
import { fetchHistory } from './aiSettingsApi';
import { BATCH_STATE, count, taskLabel, type AiSettings } from './aiSettingsModel';
import { Moment, Reference } from './AiSettingsFields';
import styles from './AiSettings.module.css';

const USAGE: Record<string, string> = { measured: '已实测', unknown_reserved: '用量未知，仍占额度', not_dispatched: '未派发' };
export function AiInvocationHistory({ data, taskKey }: { data: AiSettings; taskKey: string }) {
  const [cursors, setCursors] = useState(['']); const before = cursors.at(-1) ?? '';
  const query = useQuery({ queryKey: ['ai-invocation-history', taskKey, before], queryFn: ({ signal }) => fetchHistory(taskKey, before, signal), retry: false });
  return <section className={styles.group}><h3>调用账与历史回执</h3><p>每页最多 50 条，包含迁移保留的旧调用。未知用量、额度占用与迁移前记账值分别显示；不展示发送正文。</p>
    {query.isError ? <p role="alert">调用账读取失败，不能以空记录替代。</p> : query.isPending ? <p>正在读取调用账…</p> : query.data.items.length ? <div className={styles.scroll} tabIndex={0} role="region" aria-label="调用账表格，可横向滚动"><table className={styles.table}><thead><tr><th>时间 / 工作</th><th>模型 / 状态</th><th>实测 token</th><th>额度占用 token</th><th>用量状态 / 回执</th></tr></thead><tbody>{query.data.items.map(item => <tr key={item.invocationRef}>
      <td><Moment value={item.createdAt} /><br />{taskLabel(data, item.taskKey)}</td><td>{item.connectionName} · {item.modelId ?? '未指定模型'}<br />{BATCH_STATE[item.state] ?? item.state}</td>
      <td>{count(item.measuredTokens)}<small> · 输入 {count(item.inputTokens)} / 输出 {count(item.outputTokens)}</small></td><td>{count(item.quotaHeldTokens)}</td>
      <td>{USAGE[item.usageState] ?? '用量状态未识别'}{item.failureCode ? ` · ${item.failureCode}` : ''}<details><summary>调用与版本详情</summary><p><Reference value={item.invocationRef} /></p><p>配置 <Reference value={item.taskConfigRef} /></p><p>模型配置 <Reference value={item.modelConfigRef} /></p><p>密钥版本 <Reference value={item.connectionVersionRef} /></p><p>缓存读取 {count(item.cachedInputTokens)} / 缓存写入 {count(item.cacheWriteTokens)} / 思考 {count(item.reasoningTokens)}</p><p>报价 pico-USD：{count(item.quotePicoUsd)}；迁移前记账值：{count(item.legacyChargedTokens)}</p><p>{item.operation} · {item.dispatchState} · 预算日 {item.budgetDay}</p><Moment value={item.finishedAt} /></details></td>
    </tr>)}</tbody></table></div> : <p>本页没有调用记录，不据此推断其他工作或历史没有消耗。</p>}
    <div className={styles.actions}><ActionButton disabled={cursors.length <= 1 || query.isFetching} onClick={() => setCursors(c => c.slice(0, -1))}>较新一页</ActionButton><ActionButton disabled={query.isFetching} onClick={() => void query.refetch()}>刷新调用账</ActionButton><ActionButton disabled={!query.data?.hasMore || query.isFetching} onClick={() => { const next = query.data?.nextCursor; if (next) setCursors(c => [...c, next]); }}>更早一页</ActionButton></div>
  </section>;
}
