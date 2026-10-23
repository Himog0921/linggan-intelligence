import { num } from './format';
import { studyModeLabel } from './labels';
import type { RunDetail } from './runDtos';
import { selectionExclusionLabels } from './labels';
import { c } from './styles';

export function RunSummary({ run }: { run: RunDetail }) {
  const semantic = run.semanticSummary, knowledge = run.knowledgeSummary, cost = run.costSummary, scope = run.scope, exclusions = scope?.exclusionCounts;
  const requested = run.requestedWorkCount ?? scope?.requestedWorkRefs?.length, covered = run.coveredWorkCount ?? run.workCount;
  const candidates = exclusions && semantic?.targetCount != null ? semantic.targetCount + Object.values(exclusions).reduce<number>((total, value) => total + Number(value || 0), 0) : null;
  const exclusionRows = exclusions ? Object.entries(selectionExclusionLabels).filter(([key]) => Number(exclusions[key]) > 0).map(([key, title]) => `${key === 'notSelectedByMode' && scope?.selectionMode === 'new_only' ? '已有研究记录' : title} ${num(exclusions[key])} 条`) : [];
  return <><section className={c('study-run-frozen')} aria-label="本次冻结范围"><h3>本次研究范围</h3><p><strong>选中 {num(requested)} 篇</strong><span aria-hidden="true"> → </span><strong>实际入包 {num(covered)} 篇</strong><span aria-hidden="true"> → </span><strong>目标 {num(semantic?.targetCount)} 条评论</strong></p><p>{candidates == null ? '候选评论数量未记录' : `候选 ${num(candidates)} 条；未入包：${exclusionRows.join('；') || '没有被排除的评论'}`}。</p><p>模式：{studyModeLabel[scope?.selectionMode || ''] || '未记录'} · 评论上限 {num(run.limits?.commentBudget)} 条 · 模型词元上限 {num(run.limits?.tokenLimit)}。</p></section>
    <section className={c('study-run-summary')} aria-label="本次运行摘要"><article><h3>评论提取</h3>{semantic ? <><p>成功 {num(semantic.succeededTargetCount)} · 无信号 {num(semantic.noSignalTargetCount)} · 等待语境 {num(semantic.needsContextTargetCount)} · 失败 {num(semantic.failedTargetCount)}。</p><p>产生 {num(semantic.signalCount)} 条信号；其中当前有效 {num(semantic.currentSignalCount)} 条。</p></> : <p className={c('study-detail-muted')}>提取汇总未记录。</p>}</article>
      <article><h3>问题归并</h3>{knowledge ? <><p>归入问题 {num(knowledge.assignedSignalCount)} 条信号；关联 {num(knowledge.linkedProblemCount)} 个问题。</p><p>待归并 {num(knowledge.pendingResolutionCount)} 条 · 待比较 {num(knowledge.pendingPairCount)} 组。</p><details><summary>查看其他归并状态</summary><p>新表达暂缓 {num(knowledge.deferredNovelCount)} 条，边界未定 {num(knowledge.deferredAmbiguousCount)} 条，等待语境 {num(knowledge.deferredContextCount)} 条；检索未完成 {num(knowledge.retrievalIncompleteCount)} 条，预算中止 {num(knowledge.budgetStoppedCount)} 条，协议拒绝 {num(knowledge.protocolRejectedCount)} 条，归并失败 {num(knowledge.failedResolutionCount)} 条。</p></details></> : <p className={c('study-detail-muted')}>归并汇总未记录。</p>}</article>
      <article><details><summary>模型调用与用量</summary>{!cost ? <p className={c('study-detail-muted')}>调用账本未记录。</p> : cost.recordingState === 'unrecorded' ? <p className={c('study-detail-muted')}>历史调用记录未记录，无法判断请求次数或用量。</p> : <><p>{cost.recordingState !== 'recorded' ? '历史调用记录可能不完整；' : ''}请求 {num(cost.requestCount)} 次，已派发 {num(cost.dispatchedRequestCount)} 次；用量已知 {num(cost.usageKnownRequestCount)} 次、未知 {num(cost.usageUnknownRequestCount)} 次。</p><p>{cost.totalInputTokens != null && cost.totalOutputTokens != null ? `输入 ${num(cost.totalInputTokens)}、输出 ${num(cost.totalOutputTokens)} 词元` : `已知输入 ${num(cost.knownInputTokens)}、已知输出 ${num(cost.knownOutputTokens)} 词元；完整用量未知`}{cost.chargedTokens == null ? '' : `；保守计入 ${num(cost.chargedTokens)} 词元`}。账单金额未记录。</p></>}</details></article>
    </section></>;
}

export function RunMethod({ run }: { run: RunDetail }) {
  const manifest = run.method?.manifest, limits = run.limits, limitText = (value: number | null | undefined) => value == null ? '未记录' : value.toLocaleString('zh-CN');
  const frozen = <p>本次冻结上限：评论 {limitText(limits?.commentBudget)} 条；模型 {limitText(limits?.tokenLimit)} 词元。</p>;
  if (!manifest || !Object.keys(manifest.stages).length) return <div className={c('study-method-detail')}><p>方法版本 {run.policyRef} · 历史方法内容未记录。</p>{frozen}<p>不会用当前默认方法补写这次运行。</p></div>;
  const fixed = Object.fromEntries(Object.entries(manifest).filter(([key]) => key !== 'stages'));
  return <div className={c('study-method-detail')}><p><strong>{run.method?.name || '未命名方法'}</strong> · 版本 {run.policyRef} · hash {run.method?.hash || '未记录'}</p>{frozen}{([['semantic', '评论语义提取'], ['resolution', '问题归并'], ['pair', '独立证据配对']] as const).map(([key, title]) => <section key={key}><h3>{title}</h3>{manifest.stages[key] ? <><pre>{manifest.stages[key].systemInstruction || '说明未记录'}</pre><details><summary>查看严格输出格式定义</summary><pre>{manifest.stages[key].outputSchema == null ? '未记录' : JSON.stringify(manifest.stages[key].outputSchema, null, 2)}</pre></details></> : <p>本阶段说明未记录。</p>}</section>)}<details><summary>固定规则、清洗器与模型配置</summary><pre>{JSON.stringify(fixed, null, 2)}</pre></details></div>;
}
