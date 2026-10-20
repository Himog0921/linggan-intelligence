import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { Link, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { command, fetchBatch } from './aiSettingsApi';
import { BATCH_STATE, BRAKE_REASON, count, taskLabel, type AiSettings } from './aiSettingsModel';
import { Field, Impact, Moment, Reference } from './AiSettingsFields';
import type { AiAction } from './useAiAction';
import styles from './AiSettings.module.css';
import { AiInvocationHistory } from './AiInvocationHistory';

const SERVICE: Record<string, string> = { not_started: '尚未启动', running: '正在运行', draining: '正在排空', drained: '已排空', stale: '心跳或持有权已过期' };
function picoUsd(value: string | null) {
  if (value === null) return '—';
  const n = BigInt(value); const fraction = (n % 1_000_000_000_000n).toString().padStart(12, '0').replace(/0+$/, '');
  return `$${(n / 1_000_000_000_000n).toLocaleString('zh-CN')}${fraction ? `.${fraction}` : ''}`;
}
function BatchRead({ reference, action }: { reference: string; action: AiAction }) {
  const [fromPosition, setFromPosition] = useState(0); const [reason, setReason] = useState('');
  const query = useQuery({ queryKey: ['ai-batch', reference, fromPosition], queryFn: ({ signal }) => fetchBatch(reference, fromPosition, signal), retry: false });
  if (query.isPending) return <p>正在读取批次…</p>;
  if (query.isError) return <p role="alert">批次读取失败。</p>;
  const { batch, items, rounds, pageSize } = query.data;
  async function cancel() {
    const payload = { batchRef: reference, expectedRevision: batch.revision, reason };
    const receipt = await action.run(`cancel:${reference}`, payload, commandRef => command('batches/cancel', { ...payload, commandRef }), '取消请求已记录；已发出的调用保留用量回执，终态结果不被改写。');
    if (receipt) void query.refetch();
  }
  return <section className={styles.group}><h3>批次详情</h3><p>{BATCH_STATE[batch.state] ?? batch.state} · {batch.stopReason ?? '尚无终止原因'} · 第 {batch.lastRoundNo} 轮</p><details><summary>批次固定版本</summary><Reference value={batch.batchRef} /><p>配置 <Reference value={batch.taskConfigRef} /></p></details>
    <div className={styles.scroll}><table className={styles.table}><caption>成员回执（每页最多 {pageSize} 项）</caption><thead><tr><th>序号</th><th>状态</th><th>尝试</th><th>产出</th><th>失败或等待原因</th></tr></thead><tbody>{items.map(item => <tr key={item.itemRef}><td>{item.position + 1}</td><td>{BATCH_STATE[item.state] ?? item.state}</td><td>{item.attemptCount}</td><td>{item.usable === null ? '未知' : item.usable ? '可用' : '无可用产出'}</td><td>{item.failureCode ?? item.waitReason ?? '—'}<small><Moment value={item.nextAttemptAt} /></small></td></tr>)}</tbody></table></div>
    <div className={styles.actions}><ActionButton disabled={fromPosition === 0} onClick={() => setFromPosition(p => Math.max(0, p - pageSize))}>上一页</ActionButton><ActionButton onClick={() => void query.refetch()}>刷新批次</ActionButton><ActionButton disabled={fromPosition + pageSize >= batch.itemCount} onClick={() => setFromPosition(p => p + pageSize)}>下一页</ActionButton></div>
    {rounds.length ? <div className={styles.scroll}><table className={styles.table}><caption>轮次与自动暂停依据</caption><thead><tr><th>轮次</th><th>实际尝试</th><th>失败</th><th>可用产出</th><th>连续无产出轮</th><th>自动暂停依据</th></tr></thead><tbody>{rounds.map(round => <tr key={round.roundRef}><td>{round.roundNo}</td><td>{round.attempted}</td><td>{round.failed}</td><td>{round.usable}</td><td>{round.consecutiveWithoutUsable}</td><td>{round.brakeReason ? BRAKE_REASON[round.brakeReason] ?? round.brakeReason : '未触发'}</td></tr>)}</tbody></table></div> : <p>尚无已结算轮次。</p>}
    {batch.state === 'open' ? <Impact title="取消此批次" label="请求取消批次" action={() => void cancel()} disabled={action.busy !== null || batch.cancelRequested || !reason.trim()}><p>阻止待领取成员继续派发，已发出的调用保留用量回执；重新启用开关不会复活本批次。</p><Field label="取消原因"><input maxLength={300} value={reason} onChange={event => setReason(event.target.value)} /></Field></Impact> : null}
  </section>;
}
export function AiHealthSection({ data, action }: { data: AiSettings; action: AiAction }) {
  const [query] = useSearchParams(); const reference = query.get('batch');
  const { service, dailyUsage, failures, lateUsage } = data.execution;
  const [taskFilter, setTaskFilter] = useState('');
  return <section className={styles.section} aria-labelledby="health-title"><h2 id="health-title">运行状况</h2><section className={styles.group}><h3>独立 AI 执行器 · {SERVICE[service.observedState] ?? '状态未识别'}</h3>
    {'instanceRef' in service ? <dl className={styles.facts}><dt>最后心跳</dt><dd><Moment value={service.heartbeatAt} /></dd><dt>在途成员</dt><dd>{count(service.inflight)}</dd><dt>并发</dt><dd>{service.concurrency}</dd><dt>源码版本</dt><dd><Reference value={service.sourceRevision} /></dd><dt>实例 / 代次</dt><dd><Reference value={service.instanceRef} /> / {service.generation}</dd><dt>排空请求</dt><dd><Moment value={service.drainRequestedAt} /></dd><dt>排空完成</dt><dd><Moment value={service.drainedAt} /></dd></dl> : <p>数据库尚无执行器实例回执。页面可访问不代表执行器正在运行。</p>}</section>
    <section className={styles.group}><h3>最近七日用量（上海时区）</h3><Field label="按工作筛选"><select value={taskFilter} onChange={event => setTaskFilter(event.target.value)}><option value="">全部工作</option>{data.tasks.map(t => <option key={t.taskKey} value={t.taskKey}>{t.label}</option>)}</select></Field>
      <p>实测、当前额度占用和未知用量分开记录；成本仅合计已知报价，有未定价调用时不代表总成本。</p><div className={styles.scroll}><table className={styles.table}><thead><tr><th>工作 / 日期</th><th>调用数</th><th>实测 token</th><th>额度占用 token</th><th>未知用量调用 / 占用</th><th>已知报价 / 未定价调用</th></tr></thead><tbody>{dailyUsage.filter(row => !taskFilter || row.taskKey === taskFilter).map(row => <tr key={`${row.taskKey}:${row.budgetDay}`}><td>{taskLabel(data, row.taskKey)}<br />{row.budgetDay}</td><td>{count(row.calls)}<small> · 执行中 {count(row.runningCalls)} / 失败 {count(row.failedCalls)} / 未派发 {count(row.notDispatchedCalls)}</small></td><td>{count(row.measuredTokens)}</td><td>{count(row.quotaHeldTokens)}</td><td>{count(row.unknownCalls)} / {count(row.unknownHeldTokens)}</td><td>{picoUsd(row.knownQuotePicoUsd)} / {count(row.unpricedMeasuredCalls)}<small> · 缓存读取未知 {count(row.cacheReadUnknownCalls)} / 思考用量未知 {count(row.reasoningUnknownCalls)}</small></td></tr>)}</tbody></table></div>{!dailyUsage.length ? <p>最近七日没有调用账记录，不据此推断历史没有消耗。</p> : null}
      <p><small>另有 {count(lateUsage.receiptCount)} 份迟到用量回执，已知输入 {count(lateUsage.knownInputTokens)} / 输出 {count(lateUsage.knownOutputTokens)} token；{lateUsage.quotaReconciled ? '已核对额度' : '尚未据此改写原额度账'}。</small></p>
    </section>
    <AiInvocationHistory key={taskFilter} data={data} taskKey={taskFilter} />
    <section className={styles.group}><h3>最近批次（最多 50 批）</h3>{data.batches.length ? <nav className={styles.selector} aria-label="最近批次">{data.batches.map(batch => <Link key={batch.batchRef} to={`?section=health&batch=${batch.batchRef}`} aria-current={reference === batch.batchRef ? 'page' : undefined}><span>{taskLabel(data, batch.taskKey)} · {BATCH_STATE[batch.state] ?? batch.state}</span><small>{batch.itemCount} 项 / 待领取 {batch.pending} / 执行中 {batch.running} · <Moment value={batch.createdAt} /></small></Link>)}</nav> : <p>尚无批次。</p>}{reference ? <BatchRead key={reference} reference={reference} action={action} /> : null}</section>
    <section className={styles.group}><h3>最近失败（最多 100 条）</h3>{failures.filter(f => !taskFilter || f.taskKey === taskFilter).map(failure => <details key={failure.invocationRef}><summary>{taskLabel(data, failure.taskKey)} · {failure.description ?? '原因尚未归类'} · <Moment value={failure.createdAt} /></summary><p>{failure.failureCode ?? '无失败码'} · {failure.usageState} · {failure.dispatchState}</p><p>重试属性：{failure.retryable === null ? '未知' : failure.retryable ? '允许按合同重试' : '不可重试'}；计入失败：{failure.countsAsFailure === null ? '未知' : failure.countsAsFailure ? '是' : '否'}</p><dl className={styles.facts}><dt>调用</dt><dd><Reference value={failure.invocationRef} /></dd><dt>配置</dt><dd><Reference value={failure.taskConfigRef} /></dd><dt>密钥版本</dt><dd><Reference value={failure.connectionVersionRef} /></dd></dl></details>)}{!failures.length ? <p>没有已记录的失败。</p> : null}</section>
  </section>;
}
