import { useEffect, useRef, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { hideCachedReading } from './dossierPresent';
import type { Dossier } from './dossierTypes';
import { analysisRequests, latestAnalysis, previewAnalysis, readAnalysis, startAnalysis } from './noteAnalysisApi';
import type { AnalysisRequest, AnalysisScope, AnalysisState } from './noteAnalysisTypes';
import { NoteAnalysisResult } from './NoteAnalysisResult';
import { Section } from './Primitives';
import styles from './NoteAnalysis.module.css';
const STATE: Record<AnalysisState, string> = { pending: '等待分析', running: '正在分析', succeeded: '分析已保存', failed: '本次分析失败', skipped: '本次未执行', cancelled: '本次已取消' };
const activeState = (state: AnalysisState | undefined) => state === 'pending' || state === 'running';
const REASONS: Record<string, string> = { ai_output_invalid: '返回内容未通过检查，已保存的分析不受影响。', no_readable_material: '没有可读材料，请先补齐笔记文字。', input_too_large: '材料超过模型输入额度；可取消包含评论后再试。', task_not_configured: '尚未配置单篇分析任务。', task_config_missing: '尚未配置单篇分析任务。', external_send_not_allowed: '当前配置尚未允许发送这些材料。' };
function reasonText(reason: string) { return REASONS[reason] ?? `当前无法开始分析（${reason}），请检查任务设置。`; }
function loadPending(key: string): { request: AnalysisRequest | null; error: string | null } {
  try { return { request: analysisRequests.get(key), error: null }; } catch { return { request: null, error: '无法读取原请求身份，请恢复浏览器会话存储后再试。' }; }
}
type PanelProps = { domain: string; work: string; active: boolean; dossier?: Dossier };
export function NoteAnalysisPanel(props: PanelProps) {
  return <ScopedAnalysisPanel key={`${props.domain}:${props.work}`} {...props} />;
}
function ScopedAnalysisPanel({ domain, work, active, dossier }: PanelProps) {
  const preferenceKey = `linggan.note-analysis.scope.v1:${domain}:${work}`;
  const [scope, setScope] = useState<AnalysisScope>(() => {
    try { return sessionStorage.getItem(preferenceKey) === 'note_with_comments' ? 'note_with_comments' : 'note'; }
    catch { return 'note'; }
  });
  function selectScope(value: AnalysisScope) {
    setScope(value);
    try { sessionStorage.setItem(preferenceKey, value); } catch { /* Optional view preference; request identity storage is checked at submission. */ }
  }
  return <Section title="单篇分析"><AnalysisSession key={`${domain}:${work}:${scope}`} domain={domain} work={work} scope={scope} active={active} dossier={dossier} onScope={selectScope} /></Section>;
}
function AnalysisSession({ domain, work, scope, active, dossier, onScope }: { domain: string; work: string; scope: AnalysisScope; active: boolean; dossier: Dossier | undefined; onScope: (scope: AnalysisScope) => void }) {
  const key = `${domain}:${work}:${scope}`;
  const [initial] = useState(() => loadPending(key));
  const pending = useRef(initial.request);
  const busy = useRef(false);
  const [sending, setSending] = useState(false);
  const [uncertain, setUncertain] = useState(initial.request !== null);
  const [message, setMessage] = useState<string | null>(initial.error);
  const [requestRef, setRequestRef] = useState<string | null>(null);
  const client = useQueryClient();
  const latest = useQuery({ queryKey: ['note-analysis-latest', domain, work, scope], queryFn: ({ signal }) => latestAnalysis(domain, work, scope, signal), enabled: active, retry: false, gcTime: 0, refetchOnWindowFocus: false });
  const trackedRef = requestRef ?? (activeState(latest.data?.latestRun?.run.state) ? latest.data?.latestRun?.requestRef : null);
  const tracked = useQuery({ queryKey: ['note-analysis-request', domain, trackedRef], queryFn: ({ signal }) => readAnalysis(domain, trackedRef!, signal), enabled: active && Boolean(trackedRef), retry: false, gcTime: 0, refetchOnWindowFocus: false, refetchInterval: query => activeState(query.state.data?.run.state) ? 1500 : false });
  const terminalRef = tracked.data && !activeState(tracked.data.run.state) ? tracked.data.requestRef : null;
  useEffect(() => { if (terminalRef) void client.invalidateQueries({ queryKey: ['note-analysis-latest', domain, work, scope] }); }, [terminalRef, client, domain, work, scope]);
  const denied = hideCachedReading(latest.error) || hideCachedReading(tracked.error);
  const currentRun = tracked.data ?? latest.data?.latestRun;
  const running = Boolean(trackedRef) && (!tracked.data || activeState(tracked.data.run.state));
  const success = denied ? null : tracked.data?.run.state === 'succeeded' ? tracked.data : latest.data?.latestSuccessful;
  async function submit() {
    if (busy.current) return;
    busy.current = true; setSending(true); setMessage(null);
    let posted = false;
    try {
      let request = pending.current;
      if (!request) {
        const preview = await previewAnalysis(domain, work, scope);
        if (!preview.canStart || !preview.inputHash || !preview.taskConfigRef || preview.taskRevision === null) { setMessage(preview.reasons.map(reasonText).join(' ') || '当前材料或任务设置不满足分析条件。'); return; }
        request = { commandRef: crypto.randomUUID(), domainRef: domain, workRef: work, scope, knowledgeCutoff: preview.knowledgeCutoff, inputHash: preview.inputHash, taskConfigRef: preview.taskConfigRef, taskRevision: preview.taskRevision };
        analysisRequests.set(key, request);
        pending.current = request;
      }
      posted = true;
      const receipt = await startAnalysis(request);
      setRequestRef(receipt.requestRef); setUncertain(false); pending.current = null;
      analysisRequests.delete(key);
      setMessage(null);
    } catch (error) {
      if (posted && error instanceof ApiError && error.failure.kind === 'http' && error.failure.status < 500) {
        pending.current = null; analysisRequests.delete(key); setUncertain(false);
        setMessage(error.failure.status === 409 ? '材料或配置已变化。再次点击会重新读取范围并提交。' : reasonText(error.failure.code ?? 'request_rejected'));
      } else if (posted) { setUncertain(true); setMessage('提交结果未确认。核对原请求会沿用同一身份，不会另建分析。'); }
      else { setMessage('分析准备未完成，请检查材料、任务设置或浏览器会话存储后重试。'); }
    } finally { busy.current = false; setSending(false); }
  }
  return <div className={styles.panel}>
    <div className={styles.actions}><label><input type="checkbox" checked={scope === 'note_with_comments'} disabled={sending || uncertain || running || Boolean(initial.error)} onChange={event => onScope(event.target.checked ? 'note_with_comments' : 'note')} /> 包含已有评论</label>
      <ActionButton variant="primary" busy={sending} disabled={running || latest.isPending || Boolean(initial.error) || denied} onClick={() => void submit()}>{uncertain ? '核对原请求' : running ? '分析处理中' : success ? '重新分析' : '分析这篇'}</ActionButton>
    </div>
    {latest.isPending ? <p role="status">正在读取分析记录…</p> : null}
    {latest.isError || tracked.isError ? <p role="status">分析记录暂不可读。<button type="button" onClick={() => void (tracked.isError ? tracked.refetch() : latest.refetch())}>重试读取</button></p> : null}
    {message ? <p role="status">{message} <Link to="/settings/models?view=advanced&area=work&task=note_interpretation">任务设置</Link></p> : null}
    {currentRun && !denied ? <p role="status">{STATE[currentRun.run.state]}{currentRun.run.failureCode ? ` · ${reasonText(currentRun.run.failureCode)}` : ''}</p> : null}
    {!latest.isPending && !latest.isError && !currentRun && !success ? <p className={styles.meta}>尚未分析。点击后按当前任务设置分析所选文字材料。</p> : null}
    {success?.result ? <>{!currentRun || currentRun.requestRef !== success.requestRef ? <p className={styles.meta}>以下保留最近一次成功分析。</p> : null}<NoteAnalysisResult read={success} {...(dossier ? { dossier } : {})} /></> : null}
  </div>;
}
