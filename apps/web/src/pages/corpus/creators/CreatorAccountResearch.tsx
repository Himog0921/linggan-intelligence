import { useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { bool, decode, num, optStr, str, strList } from '../../../shared/decode';
import styles from './CreatorDrawer.module.css';

type Preview = { domainRef: string; platform: string; authorExternalId: string; workRefs: string[]; knowledgeCutoff: string; inputFingerprint: string; taskConfigRef: string; taskRevision: number };
function previewValue(value: unknown): Preview {
  return decode(value, root => ({ domainRef: str(root,'domainRef'), platform: str(root,'platform'), authorExternalId: str(root,'authorExternalId'), workRefs: strList(root,'workRefs'), knowledgeCutoff: str(root,'knowledgeCutoff'), inputFingerprint: str(root,'inputFingerprint'), taskConfigRef: str(root,'taskConfigRef'), taskRevision: num(root,'taskRevision') }));
}
function receipt(value: unknown) { return decode(value, root => ({ requestRef: str(root, 'requestRef'), batchRef: str(root, 'batchRef') })); }
function result(value: unknown) { return decode(value, root => ({ state: str(root, 'state'), sourceAvailable: bool(root, 'sourceAvailable'),
  failureCode: optStr(root, 'failureCode') })); }
const REASONS: Record<string, string> = {
  global_off: 'AI 总开关未开启', executor_off: 'AI 执行器未开启', task_creator_research_off: '创作者研究未开启',
  daily_budget_unconfigured: '尚未设置额度', model_configuration_unqualified: '模型配置尚未确认',
  current_model_not_probed: '当前模型尚未通过测试', connection_disabled: '模型连接已关闭',
  egress_not_approved: '尚未允许向当前模型发送研究底稿', author_bio_egress_not_approved: '尚未允许发送公开简介', ai_source_unavailable: '研究底稿当前不可用',
  creator_publication_withheld: '材料、许可或任务状态已变化', ai_output_invalid: '模型结果未通过结构或依据检查',
  ai_provider_failed: '模型调用失败', ai_dispatch_unknown: '模型调用结果未知，请检查运行记录',
};
function message(error: unknown): string {
  if (error instanceof ApiError && error.failure.kind === 'network') return '请求回执未知，可使用同一请求重试。';
  if (error instanceof ApiError && error.failure.kind === 'http' && isRecord(error.responseBody) && Array.isArray(error.responseBody.reasons)) {
    const reasons = error.responseBody.reasons.filter((value): value is string => typeof value === 'string');
    if (reasons.length) return `暂不能开始：${reasons.map(reason => REASONS[reason] ?? '运行条件尚未满足').join('、')}。请检查 AI 设置。`;
  }
  return '操作未完成，请检查 AI 设置和材料状态后重试。';
}
export function CreatorAccountResearch({ domain, platform, author, disabled }: { domain: string; platform: string; author: string; disabled: boolean }) {
  const client = useQueryClient();
  const [prepared, setPrepared] = useState<Preview | null>(null);
  const [request, setRequest] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(false);
  const command = useRef<string | null>(null);
  async function prepare() {
    setBusy(true); setNotice('');
    try { setPrepared(previewValue(await requestJson(`/api/local/creator-account-research/preview?${new URLSearchParams({ domainRef: domain, platform, authorExternalId: author })}`))); setRequest(null); command.current = null; setUncertain(false); }
    catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  }
  async function start() {
    if (!prepared) return;
    setBusy(true); setNotice(''); command.current ??= crypto.randomUUID();
    try {
      const value = receipt(await requestJson('/api/local/creator-account-research', { method: 'POST', body: {
        commandRef: command.current, domainRef: prepared.domainRef, platform: prepared.platform, authorExternalId: prepared.authorExternalId, workRefs: prepared.workRefs,
        knowledgeCutoff: prepared.knowledgeCutoff, expectedFingerprint: prepared.inputFingerprint,
        taskConfigRef: prepared.taskConfigRef, expectedTaskRevision: prepared.taskRevision,
      } }));
      setRequest(value.requestRef); setNotice('已提交研究，结果尚未完成。'); command.current = null; setUncertain(false);
    } catch (error) { setNotice(message(error)); if (error instanceof ApiError && error.answered) { command.current = null; setUncertain(false); } else setUncertain(true); }
    finally { setBusy(false); }
  }
  async function refresh() {
    if (!request) return;
    setBusy(true);
    try {
      const value = result(await requestJson(`/api/local/creator-account-research/${encodeURIComponent(request)}`));
      if (!value.sourceAvailable) setNotice('来源当前不可读取，结果不展示。');
      else if (value.state === 'succeeded') { setNotice('账号研究已完成，判断和依据已更新。'); await client.invalidateQueries({ queryKey: ['creators'] }); }
      else if (value.state === 'failed' || value.state === 'skipped' || value.state === 'cancelled') setNotice(`研究未完成：${value.failureCode ? (REASONS[value.failureCode] ?? '请检查运行记录') : '任务已停止'}。`);
      else setNotice('研究尚未完成，请稍后刷新。');
    } catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  }
  return <section aria-label="账号样本研究"><h4>账号样本研究</h4>
    <ActionButton onClick={() => void prepare()} disabled={disabled || busy || uncertain}>检查账号样本</ActionButton>
    {prepared ? <><p className={styles.help}>本次固定 {prepared.workRefs.length} 篇作品及可用公开简介，最多 30 篇，不代表账号全部作品。</p>
      <p className={styles.help}>开始后会按 AI 设置调用模型，可能产生费用。</p>
      <ActionButton onClick={() => void start()} disabled={disabled || busy || request !== null}>{uncertain ? '重试同一请求' : '开始账号研究'}</ActionButton></> : null}
    {request ? <ActionButton onClick={() => void refresh()} disabled={busy}>刷新研究状态</ActionButton> : null}
    {notice ? <p role="status" className={styles.help}>{notice}</p> : null}
  </section>;
}
