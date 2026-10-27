import { useRef, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError, isRecord, requestJson } from '../../../shared/api';
import { bool, decode, num, obj, optNum, optStr, str } from '../../../shared/decode';
import styles from './CreatorDrawer.module.css';

type Preview = { domainRef: string; workRef: string; knowledgeCutoff: string; inputFingerprint: string;
  taskConfigRef: string; taskRevision: number; expectedPages: number | null; acquiredPages: number; processedPages: number };
function previewValue(value: unknown): Preview {
  return decode(value, root => {
    const inventory = obj(obj(root, 'coverage'), 'imageInventory');
    return { domainRef: str(root, 'domainRef'), workRef: str(root, 'workRef'), knowledgeCutoff: str(root, 'knowledgeCutoff'),
      inputFingerprint: str(root, 'inputFingerprint'), taskConfigRef: str(root, 'taskConfigRef'), taskRevision: num(root, 'taskRevision'),
      expectedPages: optNum(inventory, 'expectedPages'), acquiredPages: num(inventory, 'acquiredPages'), processedPages: num(inventory, 'processedPages') };
  });
}
function receipt(value: unknown) { return decode(value, root => ({ requestRef: str(root, 'requestRef'), batchRef: str(root, 'batchRef') })); }
function result(value: unknown) { return decode(value, root => ({ state: str(root, 'state'), sourceAvailable: bool(root, 'sourceAvailable'),
  failureCode: optStr(root, 'failureCode') })); }
const REASONS: Record<string, string> = {
  global_off: 'AI 总开关未开启', executor_off: 'AI 执行器未开启', task_creator_research_off: '创作者研究未开启',
  daily_budget_unconfigured: '尚未设置额度', model_configuration_unqualified: '模型配置尚未确认',
  current_model_not_probed: '当前模型尚未通过测试', connection_disabled: '模型连接已关闭',
  egress_not_approved: '尚未允许向当前模型发送研究底稿', ai_source_unavailable: '研究底稿当前不可用',
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
export function CreatorWorkResearch({ domain, work, disabled }: { domain: string; work: string; disabled: boolean }) {
  const client = useQueryClient();
  const [prepared, setPrepared] = useState<Preview | null>(null);
  const [request, setRequest] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(false);
  const command = useRef<string | null>(null);
  async function prepare() {
    setBusy(true); setNotice('');
    try { setPrepared(previewValue(await requestJson(`/api/local/creator-research/preview?${new URLSearchParams({ domainRef: domain, workRef: work })}`))); setRequest(null); command.current = null; setUncertain(false); }
    catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  }
  async function start() {
    if (!prepared) return;
    setBusy(true); setNotice(''); command.current ??= crypto.randomUUID();
    try {
      const value = receipt(await requestJson('/api/local/creator-research', { method: 'POST', body: {
        commandRef: command.current, domainRef: prepared.domainRef, workRef: prepared.workRef,
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
      const value = result(await requestJson(`/api/local/creator-research/${encodeURIComponent(request)}`));
      if (!value.sourceAvailable) setNotice('来源当前不可读取，结果不展示。');
      else if (value.state === 'succeeded') { setNotice('单篇研究已完成，判断和依据已更新。'); await client.invalidateQueries({ queryKey: ['creators'] }); }
      else if (value.state === 'failed' || value.state === 'skipped' || value.state === 'cancelled') setNotice(`研究未完成：${value.failureCode ? (REASONS[value.failureCode] ?? '请检查运行记录') : '任务已停止'}。`);
      else setNotice('研究尚未完成，请稍后刷新。');
    } catch (error) { setNotice(message(error)); }
    finally { setBusy(false); }
  }
  return <section aria-label="单篇作品研究"><h4>单篇作品研究</h4>
    <ActionButton onClick={() => void prepare()} disabled={disabled || busy || uncertain}>检查研究底稿</ActionButton>
    {prepared ? <><p className={styles.help}>图片总数 {prepared.expectedPages ?? '未知'} · 已取得 {prepared.acquiredPages} · 已识读 {prepared.processedPages}。结果仅基于当前可用材料。</p>
      <p className={styles.help}>开始后会按 AI 设置调用模型，可能产生费用。</p>
      <ActionButton onClick={() => void start()} disabled={disabled || busy || request !== null}>{uncertain ? '重试同一请求' : '开始单篇研究'}</ActionButton></> : null}
    {request ? <ActionButton onClick={() => void refresh()} disabled={busy}>刷新研究状态</ActionButton> : null}
    {notice ? <p role="status" className={styles.help}>{notice}</p> : null}
  </section>;
}
