import type { RunTarget } from './runDtos';
import type { StudyNavigation } from './studyApi';
import { contextSourceLabel, contextStateLabel, dependencyStateLabel, label, observationRoleLabel, resolutionLabel, sourceStateLabel, targetStateLabel } from './labels';
import { c } from './styles';

function recoveryText(target: RunTarget) {
  const parent = target.parentContext, attempt = target.latestAttempt, failureCode = attempt?.providerFailureCode || attempt?.rejectionCode || target.terminalReason;
  if (target.state === 'needs_context') {
    if (parent?.state === 'none' && target.dependencyState === 'parent_required_missing') return '这条是根评论，没有父评论关系；旧规则把短文本错误标成缺父评论。可从原研究批次显式补跑。';
    if (parent?.state === 'not_included') return '这次研究漏带了父评论；修正输入组装后可从原研究批次显式补跑。';
    if (parent?.state === 'missing') return '当前没有可用父评论原文；可先补齐来源输入再显式补跑。相同输入不会反复重试。';
    if (!target.modelReason) return '目标在进入模型前就被判定缺语境；先核对冻结的作品语境和依赖判定。';
    return '模型拿到了当前冻结文本但仍无法确认指代；只有补充了有效语境后才值得重跑。';
  }
  if (['response_too_large', 'output_limit'].includes(failureCode ?? '')) return attempt?.retryStrategy === 'split_single_target' ? '大批次已改为单目标处理；单条仍超过响应上限时会停止，不会原样重复请求。' : '响应超过适配层上限；系统会避免原样重试，按单目标恢复或给出终态原因。';
  if (['authentication_failed', 'provider_authentication_failed', 'model_secret_unavailable'].includes(failureCode ?? '')) return '先修复模型连接凭证；固定配置错误不会在同一批研究内重复尝试。';
  return target.state === 'failed' ? '根据下方故障码先处理输入、连接或响应问题，再创建续做运行。' : '查看本次冻结输入和研究结果。';
}
function TargetDiagnostics({ target }: { target: RunTarget }) {
  const attempt = target.latestAttempt;
  if (!attempt) return <p className={c('study-detail-muted')}>没有模型调用记录（尝试 {Number(target.attemptCount || 0)} 次）。</p>;
  const usage = attempt.usageKnown ? `已知用量：输入 ${Number(attempt.inputTokens)} · 输出 ${Number(attempt.outputTokens)} · 计费 ${Number(attempt.chargedTokens)} 词元` : attempt.reservedTokens != null ? `用量未知；按预留额度 ${attempt.reservedTokens} 词元保守记账（计入 ${Number(attempt.chargedTokens || 0)} 词元）` : '模型用量记录不可用';
  const limits: Record<string, string> = { sse_stream_262144: 'SSE 响应流超过 262,144 bytes', final_text_65536: '最终文本超过 65,536 bytes', output_tokens: '模型输出词元上限' };
  const responseLimit = limits[attempt.responseLimit ?? ''];
  return <details className={c('study-target-diagnostics')}><summary>技术诊断</summary><p>尝试 {Number(target.attemptCount || 0)} 次 · {attempt.state || '状态未知'}{attempt.rejectionCode ? ` · 拒绝码 ${attempt.rejectionCode}` : ''}{attempt.providerFailureCode ? ` · 调用错误 ${attempt.providerFailureCode}` : ''}{attempt.retryStrategy ? ` · 后续策略 ${attempt.retryStrategy}` : ''}</p><p>阶段 {attempt.stage || '未记录'} · HTTP {attempt.httpStatus ?? '未记录'} · 接收 {attempt.receivedBytes ?? '未记录'} bytes · 收到结束事件 {attempt.terminalReceived == null ? '未知' : attempt.terminalReceived ? '是' : '否'}{responseLimit ? ` · ${responseLimit}` : ''}</p><p>{usage}{target.terminalReason ? ` · 终态 ${target.terminalReason}` : ''}</p></details>;
}
function TargetRow({ target, runRef, focused, nav }: { target: RunTarget; runRef: string; focused: boolean; nav: StudyNavigation }) {
  const sourceReadable = target.sourceState === 'known' && target.parentContext?.state !== 'restricted', parent = target.parentContext, sources = target.workContext?.sources ?? [];
  const parentLabels: Record<string, string> = { none: '根评论，没有父评论关系', not_included: '本条是回复，但本次运行没有冻结父评论', missing: '父评论关系存在，但冻结时未读取到可用原文', restricted: '父评论来源当前受限，原文已隐藏' };
  return <tr data-target-ref={target.targetRef} data-focus={focused}><td><span className={c('study-badge')}>{observationRoleLabel[target.observationRole ?? ''] || '来源角色未知'}</span>{sourceReadable && target.commentText ? <blockquote>{target.commentText}</blockquote> : <p className={c('study-restricted')}>{target.parentContext?.state === 'restricted' ? '父评论语境受限，本条及衍生内容已隐藏。' : sourceStateLabel[target.sourceState ?? ''] || '原文当前不可读取。'}</p>}</td>
    <td>{label(targetStateLabel, target.state) ?? target.state}{target.exclusionReason ? <p>{target.exclusionReason}</p> : null}{target.terminalReason ? <p>{target.terminalReason}</p> : null}{target.commentKey ? <p><button type="button" className={c('study-link')} data-target-comment-work={target.commentKey.workRef} data-target-comment-id={target.commentKey.commentExternalId} onClick={() => nav.comment(target.commentKey!)}>查看评论详情</button></p> : null}
      <details className={c('study-target-detail')}><summary>查看本次输入、原因与处理建议</summary><div className={c('study-target-detail-body')}><section><h4>本条原声与清洗文本</h4>{sourceReadable ? <p>{target.researchText || target.commentText || '没有可展示的清洗文本'}</p> : <p className={c('study-restricted')}>来源已受限，原声与清洗文本不再显示。</p>}</section><section><h4>本次研究使用的作品内容</h4>{!sourceReadable ? <p className={c('study-restricted')}>来源当前受限，冻结语境不再显示。</p> : sources.length ? <ul className={c('study-context-sources')}>{sources.map((source, i) => <li key={i}><span>{contextSourceLabel[source.kind ?? ''] || source.kind || '作品语境'}</span><blockquote>{source.text || ''}</blockquote></li>)}</ul> : <p className={c('study-detail-muted')}>{label(contextStateLabel, target.contextState) ?? target.contextState}；本次运行没有保存可展示的语境片段。</p>}</section><section><h4>本次研究使用的父评论</h4>{sourceReadable && parent?.state === 'available' ? <><blockquote>{parent.researchText || ''}</blockquote><p className={c('study-detail-muted')}>父评论只作为解释语境，不作为本条评论的研究信号依据。</p></> : <p className={c('study-detail-muted')}>{parentLabels[parent?.state || 'none'] || '父评论语境状态未知'}</p>}</section>{sourceReadable && target.modelReason ? <section><h4>模型给出的原因</h4><p>{target.modelReason}</p></section> : null}<section><h4>下一步</h4><p>{recoveryText(target)}</p></section><TargetDiagnostics target={target} /></div></details>
    </td><td>{label(contextStateLabel, target.contextState) ?? target.contextState}<p>{dependencyStateLabel[target.dependencyState ?? ''] || target.dependencyState || '评论依赖未记录'}</p></td><td>{Number(target.signalCount)} 条{target.resolutionState ? <p>{label(resolutionLabel, target.resolutionState) ?? target.resolutionState}</p> : null}{Number(target.signalCount) > 0 ? <p><button type="button" className={c('study-link')} data-target-signals={target.targetRef} onClick={() => nav.run(runRef, 'signals', target.targetRef)}>查看相关信号</button></p> : null}</td>
  </tr>;
}
export function RunTargetTable({ targets, runRef, focused, nav }: { targets: RunTarget[]; runRef: string; focused: string | null; nav: StudyNavigation }) {
  if (!targets.length) return <p className={c('study-empty')}>这次运行没有冻结任何评论目标。</p>;
  return <div className={c('study-review-table-wrap')}><table className={c('study-review-table')}><thead><tr>{['评论原声', '处理状态与输入说明', '语境与依赖', '研究信号'].map(title => <th scope="col" key={title}>{title}</th>)}</tr></thead><tbody>{targets.map(target => <TargetRow target={target} runRef={runRef} focused={focused === target.targetRef} nav={nav} key={target.targetRef} />)}</tbody></table></div>;
}
