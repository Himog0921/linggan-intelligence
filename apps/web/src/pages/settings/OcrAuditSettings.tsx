import { useState, type FormEvent } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Link } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { fetchAuditRuns, fetchAuditSettings, saveAuditSettings, startAuditRun, type AuditSettings } from './ocrAuditApi';
import styles from './AiSettings.module.css';
import experience from './AiSettingsExperience.module.css';
import audit from './OcrAudit.module.css';
const POOL: Record<string, string> = { random: '随机样本', risk: '风险样本', regression: '固定回归', probe: '合成视觉探测' };
const METRICS = { titleFalseAdoption: '标题错采', titleMissing: '标题漏采', bodyMissing: '重要正文遗漏', hallucinatedText: '无依据文字', disputedRecovery: '争议恢复' };
function ConfigForm({ value, revision, onSaved }: { value: AuditSettings; revision: number; onSaved: () => Promise<unknown> }) {
  const [busy, setBusy] = useState(false); const [note, setNote] = useState('');
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); const f = new FormData(event.currentTarget); const settings = { ...value, automatic: f.has('automatic'), domainRef: String(f.get('domainRef') ?? '').trim() || null,
      regressionRefs: String(f.get('regressionRefs') ?? '').split(/[\s,]+/).filter(Boolean) };
    for (const key of ['randomCount', 'riskCount', 'regressionCount', 'maxRequests', 'maxTokens', 'maxCostMicroUsd', 'concurrency', 'timeoutSeconds'] as const) settings[key] = Number(f.get(key));
    setBusy(true); setNote('');
    try { await saveAuditSettings({ commandRef: crypto.randomUUID(), expectedRevision: revision, settings }); await onSaved(); setNote('配置已保存，未发起抽检。'); }
    catch { setNote('保存未确认，请刷新核对配置版本后再处理。'); } finally { setBusy(false); }
  }
  return <form className={audit.config} onSubmit={event => void save(event)}>
    <p>默认关闭。每周一 09:00（Asia/Shanghai）由现有 AI 执行器检查；任务、全局和执行器开关以及材料许可仍分别生效。首版不自动修正。</p>
    <label><input name="automatic" type="checkbox" defaultChecked={value.automatic} /> 开启每周抽样</label>
    <label>研究领域标识<input name="domainRef" defaultValue={value.domainRef ?? ''} placeholder="选择抽检范围对应的领域标识" /></label>
    <div className={styles.grid}>{([['randomCount','随机样本数'],['riskCount','风险样本数'],['regressionCount','回归样本数'],['maxRequests','每轮最多请求数'],['maxTokens','每轮最多 token'],['maxCostMicroUsd','每轮估算费用上限（百万分之一美元）'],['concurrency','最多同时调用数'],['timeoutSeconds','单次超时（秒）']] as const).map(([key,label]) => <label key={key}>{label}<input name={key} type="number" min={key.endsWith('Count') ? 0 : 1} step={1} required defaultValue={value[key]} /></label>)}</div>
    <label>固定回归的采用记录标识（空格或逗号分隔）<textarea name="regressionRefs" defaultValue={value.regressionRefs.join('\n')} /></label>
    <ActionButton type="submit" busy={busy}>保存抽检配置</ActionButton><p role="status">{note}</p>
  </form>;
}
export function OcrAuditSettings() {
  const query = useQuery({ queryKey: ['ocr-audit-settings'], queryFn: ({ signal }) => fetchAuditSettings(signal), retry: false });
  const reports = useQuery({ queryKey: ['ocr-audit-runs'], queryFn: ({ signal }) => fetchAuditRuns(signal), retry: false });
  const [busy, setBusy] = useState(false); const [note, setNote] = useState('');
  const [retry, setRetry] = useState<{ probe: boolean; id: string } | null>(null);
  async function start(probe: boolean) { setBusy(true); setNote(''); try { const id = retry?.probe === probe ? retry.id : crypto.randomUUID(); setRetry({ probe, id }); const run = await startAuditRun(probe, id); setRetry(null); setNote(`已冻结并排队，运行标识：${run}。排队不代表执行成功。`); await reports.refetch(); } catch { setNote('未能确认排队；重试同一动作会核对原请求。请检查工作配置、价格、视觉能力与材料许可。'); } finally { setBusy(false); } }
  return <section className={`${experience.advancedContent} ${audit.audit}`} aria-label="图片文字质量抽检">
    <h2>图片文字质量抽检</h2><p>模型只提供第二意见。质量指标来自人工对照原图确认的点，三类样本分开统计；未审核不显示为错误率。</p>
    <div className={styles.actions}><Link className={experience.subtleLink} to="/settings/models?view=advanced&area=work&task=ocr_quality_audit">模型、工作开关与额度</Link><Link className={experience.subtleLink} to="/settings/models?view=advanced&area=permissions">图片材料许可</Link></div>
    {query.isPending ? <p>正在读取配置…</p> : query.error ? <p role="alert">抽检配置尚不可读，请核对本机是否已安装此版本。</p> : query.data ? <details><summary>抽样与运行边界</summary><ConfigForm key={query.data.revision} value={query.data.settings} revision={query.data.revision} onSaved={() => query.refetch()} /></details> : null}
    <div className={styles.grid}><ActionButton busy={busy} disabled={!query.data || retry?.probe === false} onClick={() => void start(true)}>验证合成图视觉能力</ActionButton><ActionButton busy={busy} disabled={!query.data || retry?.probe === true} onClick={() => void start(false)}>手动抽检一次</ActionButton><ActionButton onClick={() => void reports.refetch()}>刷新周报</ActionButton></div><p role="status">{note}</p>
    {reports.error ? <p role="alert">抽检报告读取失败，未用空报告代替。</p> : null}
    {reports.data?.runs.length === 0 ? <p>尚无抽检记录；没有人工审核分母。</p> : null}
    {reports.data?.runs.some(run => run.kind === 'weekly') ? <details><summary>最近四周 · 各池人工核验趋势</summary><div className={experience.tableScroll}><table><thead><tr><th>周</th><th>样本池</th><th>标题错采</th><th>标题漏采</th><th>正文遗漏</th><th>无依据文字</th><th>争议恢复</th></tr></thead><tbody>{reports.data.runs.filter(run => run.kind === 'weekly').slice(0, 4).flatMap(run => run.pools.filter(p => p.pool !== 'probe').map(p => <tr key={`${run.runRef}:${p.pool}`}><td>{run.runKey.replace('weekly:', '')}</td><td>{POOL[p.pool]}</td>{(Object.keys(METRICS) as (keyof typeof METRICS)[]).map(key => <td key={key}>{p.metrics[key].numerator}/{p.metrics[key].denominator}{p.metrics[key].rate === null ? ' · 未核验' : ` · ${p.metrics[key].rate}%`}</td>)}</tr>))}</tbody></table></div></details> : null}
    {reports.data?.runs.map(run => <details key={run.runRef} data-audit-run={run.runRef}><summary>{run.kind === 'visual_probe' ? '合成视觉探测' : run.kind === 'weekly' ? '每周抽检' : '手动抽检'} · {run.modelId} · {run.kind === 'weekly' ? run.runKey.replace('weekly:', '') : run.createdAt.slice(0, 16)} · {run.runRef.slice(0, 8)}</summary>
      <p>估算费用依据冻结价格，不是供应商账单。样本不足不跨池补齐。</p>
      {run.pools.filter(p => p.selected > 0).map(p => <section key={p.pool}><h3>{POOL[p.pool] ?? p.pool}</h3><p>选中 {p.selected} · 人工核验 {p.humanReviewedSamples}（一致案例 {p.humanReviewedAgreements} / 分歧案例 {p.humanReviewedDisagreements}） · 待确认差异 {p.pendingDifferences} · 受限隐藏 {p.withheld} · 实际调用 {p.modelCalls} · 失败 {p.failures}</p><p>输入 token：{p.inputTokens ?? '未知'} · 输出 token：{p.outputTokens ?? '未知'} · 预留 token：{p.reservedTokens} · 估算美元：{p.estimatedCostPicoUsd === null ? '未知' : (Number(p.estimatedCostPicoUsd) / 1e12).toFixed(4)}</p>
        <table><thead><tr><th>人工确认指标</th><th>确认点 / 核验点</th><th>比例</th></tr></thead><tbody>{(Object.keys(METRICS) as (keyof typeof METRICS)[]).map(key => <tr key={key}><td>{METRICS[key]}</td><td>{p.metrics[key].numerator} / {p.metrics[key].denominator}</td><td>{p.metrics[key].rate === null ? '尚未核验' : `${p.metrics[key].rate}%`}</td></tr>)}</tbody></table>
      </section>)}<p>本地规则、提示词、冻结名单和人工版本可沿每张图回查；最近四周的各次每周报告在本列表分别保留。</p>
    </details>)}
  </section>;
}
