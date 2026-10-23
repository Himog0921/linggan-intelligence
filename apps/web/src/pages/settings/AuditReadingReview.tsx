import { useEffect, useRef, useState, type FormEvent } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../shared/ActionButton';
import { ApiError, isRecord } from '../../shared/api';
import { mediaPath } from '../corpus/evidence/evidencePaths';
import { fetchAuditDetail, reviewAudit, type AuditReviewRequest } from './ocrAuditApi';
import styles from '../corpus/evidence/Dossier.module.css';
const METRICS = { titleFalseAdoption: '标题错采', titleMissing: '标题漏采', bodyMissing: '重要正文遗漏', hallucinatedText: '无依据文字', disputedRecovery: '争议恢复' };
const DECISION = { confirmed: '确认意见', dismissed: '不采纳意见', pending: '保持待核验' };
export function AuditReadingReview({ readingRef }: { readingRef: string }) {
  const [open, setOpen] = useState(false); const [activated, setActivated] = useState(false);
  return <section><ActionButton variant="quiet" onClick={() => { setActivated(true); setOpen(value => !value); }}>{open ? '收起抽检意见' : '审阅抽检意见'}</ActionButton>{activated ? <div hidden={!open}><ReviewContent key={readingRef} readingRef={readingRef} /></div> : null}</section>;
}
function ReviewContent({ readingRef }: { readingRef: string }) {
  const client = useQueryClient(); const mounted = useRef(true); const sending = useRef(false);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const query = useQuery({ queryKey: ['ocr-audit-detail', readingRef], queryFn: ({ signal }) => fetchAuditDetail(readingRef, signal), retry: false });
  const [busy, setBusy] = useState(false); const [message, setMessage] = useState(''); const [pending, setPending] = useState<AuditReviewRequest | null>(null);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); if (!query.data?.sampleRef || sending.current) return;
    const f = new FormData(event.currentTarget);
    const command: AuditReviewRequest = pending ?? { commandRef: crypto.randomUUID(), sampleRef: query.data.sampleRef, issueRef: String(f.get('issueRef') ?? '') || null, expectedRevision: query.data.revision, decision: String(f.get('decision')) as AuditReviewRequest['decision'], materialReviewCommandRef: String(f.get('materialReviewCommandRef') ?? '') || null, actor: String(f.get('actor')), reason: String(f.get('reason')), metrics: Object.fromEntries(Object.keys(METRICS).map(key => [key, { numerator: Number(f.get(`${key}:n`) ?? 0), denominator: Number(f.get(`${key}:d`) ?? 0) }])) };
    sending.current = true; setBusy(true); setMessage('');
    try { const receipt = await reviewAudit(command); if (!mounted.current) return; if (receipt.readingRef !== readingRef || receipt.decision !== command.decision || receipt.materialReviewCommandRef !== command.materialReviewCommandRef) throw new ApiError({ kind: 'invalid_response', status: 200 }); setPending(null); setMessage(`抽检审阅已保存，第 ${receipt.revision} 版。`); void query.refetch(); void client.invalidateQueries({ queryKey: ['ocr-audit-runs'] }); }
    catch (error) { if (!mounted.current) return; if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) { setPending(null); setMessage('审阅未被接纳，请刷新核对版本和核验点数。'); } else { setPending(command); setMessage('审阅回执未确认，请核对同一请求。'); } }
    finally { sending.current = false; if (mounted.current) setBusy(false); }
  }
  if (query.isPending) return <p>正在读取抽检意见…</p>;
  if (query.isError) return <p role="alert">抽检意见不可读或来源已受限。</p>;
  const data = query.data; if (!data?.sampleRef) return <p>此读本尚未被抽检。</p>;
  const opinion = isRecord(data.modelOpinion) ? data.modelOpinion : null;
  const regions = Array.isArray(opinion?.regions) ? opinion.regions.filter(isRecord) : [];
  const image = mediaPath(data.originalAssetUrl);
  return <div className={styles.audit}><h4>模型第二意见</h4><p className={styles.note}>这里记录对抽检意见的判断。底稿修正在对应笔记的“核对与修正”中保存。</p>
    {image ? <div className={styles.visual}><img src={image} alt="抽检来源图片" loading="lazy" /></div> : null}
    {regions.map((row, index) => <p className={styles.text} key={index}>{typeof row.text === 'string' ? row.text : '此区域尚未读清'}</p>)}
    {data.reviews.length ? <details><summary>人工审阅历史</summary>{data.reviews.map(review => <p key={review.revision}>{`第 ${review.revision} 版 · ${DECISION[review.decision]} · ${review.actor} · ${review.reason}`}</p>)}</details> : null}
    <form className={styles.reviewForm} onSubmit={event => void submit(event)}><fieldset disabled={busy || pending !== null}><legend>对照来源后的意见</legend>
      <label>对应差异<select name="issueRef"><option value="">整张图</option>{data.issues.map((issue, index) => <option key={issue.issueRef} value={issue.issueRef}>差异 {index + 1}</option>)}</select></label>
      <label>审阅决定<select name="decision"><option value="pending">保持待核验</option><option value="confirmed">确认意见</option><option value="dismissed">不采纳意见</option></select></label>
      <label>审核人<input name="actor" required maxLength={120} /></label><label>核对依据<input name="reason" required maxLength={300} /></label>
      <details><summary>关联底稿修正与累计核验点</summary><label>已保存的底稿修正命令（可选）<input name="materialReviewCommandRef" pattern="[0-9a-fA-F-]{36}" /></label>{(Object.keys(METRICS) as (keyof typeof METRICS)[]).map(key => <div key={key}><span>{METRICS[key]}</span><label>确认点数<input name={`${key}:n`} type="number" min={0} defaultValue={data.humanCounts[key]?.numerator ?? 0} /></label><label>核验点数<input name={`${key}:d`} type="number" min={0} defaultValue={data.humanCounts[key]?.denominator ?? 0} /></label></div>)}</details>
    </fieldset><ActionButton type="submit" busy={busy}>{pending ? '核对同一审阅请求' : '保存审阅意见'}</ActionButton>{message ? <p role="status">{message}</p> : null}</form>
  </div>;
}
