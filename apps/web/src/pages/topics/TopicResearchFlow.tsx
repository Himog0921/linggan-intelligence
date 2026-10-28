import { Link } from 'react-router';
import { useEffect, useRef, useState } from 'react';
import { ApiError, isRecord, requestJson } from '../../shared/api';
import { decode, str } from '../../shared/decode';
import { c } from './styles';

import { parseTopicRuns as runs, type Run } from './topicResearchModel';
const PHASES = ['讨论提炼', '讨论核验', '归属判断', '归属核验'];
const STATES: Record<string, string> = { pending: '等待', running: '执行中', succeeded: '已通过', failed: '失败', skipped: '未执行', cancelled: '已取消' };
function message(error: unknown) {
  if (error instanceof ApiError && !error.answered) return '回执未知，请用同一请求重试。';
  if (error instanceof ApiError && isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)) return '暂不能开始：' + error.responseBody.reasons.filter(v => typeof v === 'string').join('、') + '。请核对 AI 设置及底稿。';
  return '操作未完成，请刷新并检查材料状态。';
}
const REASONS: Record<string,string> = {
  topic_assignment_not_required:'没有待归属的讨论，此阶段无需执行', topic_dependency_failed:'前置阶段未通过',
  topic_publication_withheld:'来源、许可或控制状态已变化',ai_output_invalid:'模型结果未通过校验',ai_provider_failed:'模型调用失败',
  ai_dispatch_unknown:'发送结果未知，请先核对调用记录',ai_input_limit_before_dispatch:'完整输入超过模型额度，未发送',ai_source_unavailable:'来源当前不可读取',
};
export function TopicResearchFlow({ domain, works }: { domain: string; works: { workRef: string; title?: string | null }[] }) {
  const [selected, setSelected] = useState<string[]>([]), [records, setRecords] = useState<Run[]>([]), [notice, setNotice] = useState(''), [busy, setBusy] = useState(false), [uncertain, setUncertain] = useState(false);
  const command = useRef<string | null>(null);
  useEffect(() => { let current = true; void requestJson(`/api/local/topic-research/runs?${new URLSearchParams({ domainRef: domain })}`).then(value => { if (current) setRecords(runs(value)); }).catch(error => { if (current) setNotice(message(error)); }); return () => { current = false; }; }, [domain]);
  async function refresh() { setBusy(true); try { setRecords(runs(await requestJson(`/api/local/topic-research/runs?${new URLSearchParams({ domainRef: domain })}`))); setNotice('已读取阶段回执；研究结果通过后可刷新图谱查看。'); } catch (error) { setNotice(message(error)); } finally { setBusy(false); } }
  async function start() {
    if (!selected.length) return; setBusy(true); command.current ??= crypto.randomUUID();
    try { decode(await requestJson('/api/local/topic-research', { method: 'POST', body: { commandRef: command.current, domainRef: domain, workRefs: selected } }), root => { str(root, 'runRef'); return str(root, 'runRef'); }); command.current = null; setUncertain(false); setNotice('已提交四阶段研究；模型结果尚未完成。'); }
    catch (error) { setNotice(message(error)); if (error instanceof ApiError && error.answered) { command.current = null; setUncertain(false); } else setUncertain(true); } finally { setBusy(false); }
  }
  async function cancel(runRef: string) { setBusy(true); try { await requestJson('/api/local/topic-research/cancel', { method: 'POST', body: { commandRef: crypto.randomUUID(), domainRef: domain, runRef } }); setNotice('已请求取消余下阶段。'); } catch { setNotice('取消回执未知，请刷新阶段回执核对。'); } finally { setBusy(false); } }
  return <section className={c('lgi-tm-section')} aria-label="主题研究流程"><h3>主题研究</h3><p>选择最多 10 篇作品。每篇完整底稿和评论按窗口处理，分别经过提炼、核验、归属和核验；四阶段方法均在 AI 设置编辑和切换。</p>
    <p>开始后会按各阶段设置调用模型并消耗额度。图片缺页和未识读情况保留在输入中。</p><Link to="/settings/models?view=advanced&area=work&task=topic_research">去 AI 设置</Link>
    <fieldset disabled={busy || uncertain}><legend>本次作品</legend>{works.map(work => <label key={work.workRef}><input type="checkbox" checked={selected.includes(work.workRef)} disabled={selected.length >= 10 && !selected.includes(work.workRef)} onChange={event => setSelected(old => event.target.checked ? [...old, work.workRef] : old.filter(ref => ref !== work.workRef))} />{work.title || '未命名作品'}</label>)}</fieldset>
    <button type="button" disabled={busy || selected.length === 0} onClick={() => void start()}>{uncertain ? '重试同一请求' : '开始四阶段研究'}</button><button type="button" disabled={busy} onClick={() => void refresh()}>刷新阶段回执</button>
    {notice ? <p role="status">{notice}</p> : null}{records.map(run => <article key={run.runRef}><h4>{run.pending ? '研究进行中' : run.failed ? '研究存在未完成阶段' : run.stages.some(stage => stage.state === 'cancelled') ? '研究已取消' : '阶段处理完成'} · {run.windows} 个窗口</h4>{run.pending ? <button type="button" disabled={busy} onClick={() => void cancel(run.runRef)}>取消余下阶段</button> : null}<ul>{run.stages.map(stage => <li key={stage.window + stage.phase}>窗口 {Array.from(new Set(run.stages.map(item => item.window))).indexOf(stage.window) + 1} · {PHASES[stage.phase] ?? '阶段未知'}：{STATES[stage.state] ?? stage.state}{stage.failure ? ` · ${REASONS[stage.failure] ?? '请查看 AI 调用记录'}` : stage.wait ? ' · 等待运行条件满足' : ''}</li>)}</ul></article>)}
  </section>;
}
