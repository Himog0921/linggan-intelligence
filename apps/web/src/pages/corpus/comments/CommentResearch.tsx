import { useCallback, useEffect, useRef, useState } from 'react';
import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { bool, decode, list, num, obj, optObj, optStr, str } from '../../../shared/decode';
import { c } from './styles';
import { PHASES, STATES, REASONS } from './researchModel';
import { preview, type Preview } from './commentResearchApi';


function message(error: unknown) {
  if (error instanceof ApiError && error.failure.kind === 'network') return '请求回执未知，可使用同一请求重试。';
  if (error instanceof ApiError && isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)) return error.responseBody.reasons.map(reason => typeof reason === 'string' ? REASONS[reason] ?? '运行条件尚未满足' : '运行条件尚未满足').join('、');
  return '操作未完成，请检查 AI 设置和材料状态。';
}
export function CommentResearch({ domain, work, source, role, initialRequest }: { domain: string; work: string; source: string; role: string; initialRequest?: string | null }) {
  const [prepared, setPrepared] = useState<Preview | null>(null), [request, setRequest] = useState<string | null>(initialRequest ?? null);
  const [notice, setNotice] = useState(''), [signals, setSignals] = useState<{ proposition: string; evidence: string }[]>([]), [busy, setBusy] = useState(false), [uncertain, setUncertain] = useState(false);
  const [stages, setStages] = useState<{ phase: number; state: string; failure: string | null }[]>([]);
  const command = useRef<string | null>(null);
  async function prepare() {
    setBusy(true); setNotice('');
    try { setPrepared(preview(await requestJson(`/api/local/comment-research/preview?${new URLSearchParams({ domainRef: domain, workRef: work, sourceRef: source, observationRole: role })}`))); setRequest(null); setSignals([]); command.current = null; setUncertain(false); }
    catch (error) { setNotice(message(error)); } finally { setBusy(false); }
  }
  async function start() {
    if (!prepared) return; setBusy(true); setNotice(''); command.current ??= crypto.randomUUID();
    try { const value = decode(await requestJson('/api/local/comment-research', { method: 'POST', body: { commandRef: command.current, domainRef: prepared.domainRef, workRef: prepared.workRef, sourceRef: prepared.sourceRef, observationRole: prepared.observationRole, knowledgeCutoff: prepared.knowledgeCutoff, expectedInputHash: prepared.inputHash, taskConfigRef: prepared.taskConfigRef, expectedTaskRevision: prepared.taskRevision } }), row => ({ requestRef: str(row, 'requestRef') })); setRequest(value.requestRef); setNotice('已提交，研究尚未完成。'); command.current = null; setUncertain(false); }
    catch (error) { setNotice(message(error)); if (error instanceof ApiError && error.answered) { command.current = null; setUncertain(false); } else setUncertain(true); } finally { setBusy(false); }
  }
  const applyResult = useCallback((response: unknown) => {
      if (response === null) { setNotice('这条评论尚无新研究记录。'); setSignals([]); return; }
      const value = decode(response, row => { const result = optObj(row, 'result'), target = optObj(row, 'target'); return { requestRef: str(row, 'requestRef'), state: str(row, 'state'), semanticState: str(row, 'semanticState'), stages: list(row, 'stages', s => ({ phase: num(s, 'stageNumber'), state: str(s, 'state'), failure: optStr(s, 'failureCode') })), available: bool(row, 'sourceAvailable'), failure: optStr(row, 'failureCode'), outcome: result ? str(result, 'outcome') : null, reason: result ? optStr(result, 'reason') : null, signals: result ? list(result, 'signals', item => { const span = obj(item, 'evidenceSpan'); const start = num(span, 'start'), end = num(span, 'end'); const chars = Array.from(target ? str(target, 'rawText') : ''); if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || end <= start || end > chars.length) throw new Error('评论依据范围无效'); return { proposition: str(item, 'proposition'), evidence: chars.slice(start, end).join('') }; }) : [] }; });
      setRequest(value.requestRef); setStages(value.stages);
      if (!value.available) { setSignals([]); setNotice('来源当前不可读取，结果不展示。'); }
      else if (value.semanticState === 'succeeded') { setSignals(value.signals); setNotice(value.outcome === 'needs_context' ? `需要补充语境：${value.reason}` : value.outcome === 'no_signal' ? `未发现可研究信号：${value.reason}` : value.state === 'needs_support' ? '已提取原声，问题候选等待另一独立作者的支持。' : value.state === 'succeeded' ? '研究流程已完成，以下判断均须对应本条评论。' : '原声提取已完成，后续归并仍以阶段回执为准。'); }
      else if (['failed', 'skipped', 'cancelled'].includes(value.state)) setNotice(`研究未完成：${value.failure ? REASONS[value.failure] ?? '请检查运行记录' : '任务已停止'}。`);
      else setNotice('研究尚未完成，请稍后刷新。');
  }, []);
  async function readResult(path: string) { setBusy(true); try { applyResult(await requestJson(path)); } catch(error) { setNotice(message(error)); } finally { setBusy(false); } }
  useEffect(() => { if (!initialRequest) return; let current=true; void requestJson(`/api/local/comment-research/${encodeURIComponent(initialRequest)}`).then(response => { if(current) applyResult(response); }).catch(error => { if(current) setNotice(message(error)); }); return () => {current=false;}; }, [initialRequest, applyResult]);
  return <section className={c('study-detail-section')} aria-label="研究这条评论"><h3>研究这条评论</h3>
    <button type="button" disabled={busy || uncertain} onClick={() => void prepare()}>检查评论与研究底稿</button>
    {prepared ? <><p>将使用当前作品研究底稿和这条评论；父评论只提供语境。{prepared.parentState === 'parent_required_missing' ? '父评论暂不可用。' : ''}</p><p>图片总数 {prepared.expectedPages ?? '未知'} · 已取得 {prepared.acquiredPages} · 已识读 {prepared.processedPages}；研究仅基于当前可用材料。</p><p>开始后按 AI 设置调用模型，可能产生费用。</p><button type="button" disabled={busy || request !== null} onClick={() => void start()}>{uncertain ? '重试同一请求' : '开始研究'}</button></> : null}
    {request ? <button type="button" disabled={busy} onClick={() => void readResult(`/api/local/comment-research/${encodeURIComponent(request)}`)}>刷新研究状态</button> : <button type="button" disabled={busy || uncertain} onClick={() => void readResult(`/api/local/comment-research/latest?${new URLSearchParams({ domainRef: domain, workRef: work, sourceRef: source })}`)}>查看最近研究</button>}
    {notice ? <p role="status">{notice}</p> : null}<ol>{stages.map(stage => <li key={stage.phase}>{PHASES[stage.phase]}：{STATES[stage.state] ?? stage.state}{stage.failure ? ` · ${REASONS[stage.failure] ?? '请查看 AI 调用记录'}` : ''}</li>)}</ol>{signals.length ? <ul>{signals.map((signal, index) => <li key={index}><p>{signal.proposition}</p><blockquote>{signal.evidence}</blockquote></li>)}</ul> : null}
  </section>;
}
