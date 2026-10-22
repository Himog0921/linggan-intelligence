import { useState, useId, type FormEvent } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from 'react-router';
import { isRecord } from '../../../shared/api';
import { ActionButton } from '../../../shared/ActionButton';
import { fetchAuditDetail, reviewAudit } from '../../settings/ocrAuditApi';
import { DOSSIER_KEY } from './evidenceKeys';
import styles from './Dossier.module.css';
const KINDS: Record<string, string> = { title_false_adoption: '疑似标题错采', title_missing: '疑似标题漏采', body_missing: '疑似正文遗漏', wrong_character: '字词读法分歧', hallucinated_text: '疑似无依据文字', source_role_mismatch: '文字归属分歧', reading_order: '阅读顺序分歧', uncertain: '尚不能确认' };
const METRICS = { titleFalseAdoption: '标题错采', titleMissing: '标题漏采', bodyMissing: '重要正文遗漏', hallucinatedText: '无依据文字', disputedRecovery: '争议恢复' };
export function OcrAuditReview({ adoptionRef, body }: { adoptionRef: string; body: string }) {
  const [open, setOpen] = useState(false);
  return <section><ActionButton onClick={() => setOpen(value => !value)}>{open ? '收起质量抽检' : '查看质量抽检'}</ActionButton>{open ? <ReviewContent adoptionRef={adoptionRef} body={body} /> : null}</section>;
}
function ReviewContent({ adoptionRef, body }: { adoptionRef: string; body: string }) {
  const client = useQueryClient();
  const id = useId();
  const query = useQuery({ queryKey: ['ocr-audit-detail', adoptionRef], queryFn: ({ signal }) => fetchAuditDetail(adoptionRef, signal), retry: false });
  const [busy, setBusy] = useState(false); const [note, setNote] = useState(''); const [retry, setRetry] = useState<object | null>(null);
  const [decision, setDecision] = useState('pending');
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!query.data?.sampleRef) return;
    const f = new FormData(event.currentTarget);
    const metrics = Object.fromEntries(Object.keys(METRICS).map(key => [key, { numerator: Number(f.get(`${key}:n`) ?? 0), denominator: Number(f.get(`${key}:d`) ?? 0) }]));
    const payload = retry ?? { commandRef: crypto.randomUUID(), sampleRef: query.data.sampleRef, issueRef: String(f.get('issueRef') ?? '') || null, expectedRevision: query.data.revision,
      decision, field: String(f.get('field')), text: decision === 'adopt' ? String(f.get('text') ?? '') : null, actor: String(f.get('actor')), reason: String(f.get('reason')), metrics };
    setBusy(true); setNote('');
    try { const receipt = await reviewAudit(payload); setRetry(null); setNote(`审阅已保存，第 ${receipt.revision} 版。${receipt.sourceKind === 'manual_transcription' ? '文字来源标为人工转写。' : ''}`); await Promise.all([query.refetch(), client.invalidateQueries({ queryKey: [DOSSIER_KEY] }), client.invalidateQueries({ queryKey: ['ocr-audit-runs'] })]); }
    catch { setRetry(payload); setNote('保存结果未确认。再次提交会核对同一请求，不创建第二次修正；也可刷新查看已保存版本。'); } finally { setBusy(false); }
  }
  if (query.isPending) return <p>正在读取本图抽检…</p>;
  if (query.error) return <p role="alert">本图抽检不可读或来源已受限；未显示旧意见。</p>;
  const data = query.data;
  if (!data?.sampleRef) return <p>本图尚未被抽检。<Link to="/settings/models?view=runs">查看抽检配置与周报</Link></p>;
  const opinion = isRecord(data.modelOpinion) ? data.modelOpinion : null;
  const regions = Array.isArray(opinion?.regions) ? opinion.regions.filter(isRecord) : [];
  return <div className={styles.audit}><h3>模型第二意见 · {data.modelId ?? '模型未知'}</h3><p>模型独立看图，不是人工真值。请对照上方原图确认；截图中的不同表达者不能直接归为作品作者。</p>
    <div className={styles.compare}><section><h4>当前采用文字</h4><pre className={styles.pre}>{body || '当前没有已采用正文，原始文字及待核验片段仍保留。'}</pre></section><section><h4>模型盲读文字</h4>{opinion ? regions.map((r, n) => <pre className={styles.pre} key={n}>{typeof r.text === 'string' ? r.text : '此区文字未确认'}</pre>) : <p>尚无合格的完整返回；请查看运行回执。</p>}</section></div>
    <ul>{data.issues.map(issue => <li key={issue.issueRef}>{isRecord(issue.suggestion) && typeof issue.suggestion.kind === 'string' ? KINDS[issue.suggestion.kind] ?? '待确认差异' : '待确认差异'}</li>)}</ul>
    {data.reviews.length > 0 ? <details className={styles.disclosure}><summary>人工审阅历史（{data.reviews.length} 版）</summary>{data.reviews.map(r => <section key={r.revision}><h4>第 {r.revision} 版 · {r.decision === 'adopt' ? '采用修正' : r.decision === 'reject' ? '拒绝建议' : '待核验'}</h4><p>{r.actor} · {r.reason}{r.sourceKind === 'manual_transcription' ? ' · 人工转写' : ''}</p><details><summary>修正前后文字</summary><pre className={styles.pre}>{r.beforeText}</pre><pre className={styles.pre}>{r.afterText ?? '未改变采用文字'}</pre></details></section>)}</details> : null}
    <form className={styles.reviewForm} onSubmit={event => void submit(event)}>
      <fieldset disabled={busy || retry !== null}><legend>对照原图后的审阅</legend>
        <label htmlFor={`${id}-issue`}>处理的差异</label><select id={`${id}-issue`} name="issueRef"><option value="">核验整张图（也适用于一致案例）</option>{data.issues.map(issue => <option key={issue.issueRef} value={issue.issueRef}>{isRecord(issue.suggestion) && typeof issue.suggestion.kind === 'string' ? KINDS[issue.suggestion.kind] ?? '待确认差异' : '待确认差异'}</option>)}</select>
        <label htmlFor={`${id}-field`}>采用对象</label><select id={`${id}-field`} name="field"><option value="adopted_body">本图采用正文</option><option value="cover_headline">封面标题候选</option></select>
        <label htmlFor={`${id}-decision`}>审阅决定</label><select id={`${id}-decision`} value={decision} onChange={event => setDecision(event.target.value)}><option value="pending">保持待核验</option><option value="reject">拒绝本条建议</option><option value="adopt">采用修正</option></select>
        {decision === 'adopt' ? <div><label htmlFor={`${id}-text`}>修正后本图完整采用文字</label><textarea id={`${id}-text`} name="text" defaultValue={body} maxLength={16000} /><p>此栏是所选对象的完整新版本；原 OCR 不覆盖。无法对应原区块的文字会登记为人工转写。</p></div> : null}
        <label>审核人<input name="actor" required maxLength={120} autoComplete="off" /></label><label>对照依据与原因<input name="reason" required maxLength={300} /></label>
        {decision !== 'pending' ? <details><summary>本轮冻结版本累计核验点（本图计一次，未核验为 0）</summary>{(Object.keys(METRICS) as (keyof typeof METRICS)[]).map(key => <div key={key}><span>{METRICS[key]}</span><label>确认点数<input name={`${key}:n`} type="number" min={0} max={10000} defaultValue={data.humanCounts[key]?.numerator ?? 0} /></label><label>核验点数<input name={`${key}:d`} type="number" min={0} max={10000} defaultValue={data.humanCounts[key]?.denominator ?? 0} /></label></div>)}</details> : null}
      </fieldset>
      <ActionButton type="submit" busy={busy}>{retry ? '核对同一审阅请求' : '保存审阅决定'}</ActionButton><p role="status">{note}</p>
    </form>
  </div>;
}
