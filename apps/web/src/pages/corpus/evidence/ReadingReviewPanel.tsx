import { useEffect, useId, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { fetchCandidates, reviewReading } from './dossierApi';
import { DossierBlocks, DossierIssues } from './DossierBlocks';
import { DossierBlockEditor } from './DossierBlockEditor';
import { hideCachedReading } from './dossierPresent';
import type { DossierBlock, DossierMaterial, ReadingCandidate, ReadingReviewRequest, ReadingReviewReceipt } from './dossierTypes';
import styles from './Dossier.module.css';

interface Props {
  unit: DossierMaterial; work: string; domain: string; onApplied: (receipt: ReadingReviewReceipt) => void;
  initialCandidate?: ReadingCandidate; initiallyOpen?: boolean; originalReadable?: boolean; showCurrentReading?: boolean;
}
const PRODUCER = { platform: '平台原文', ordinary_ocr: '普通识字', local_vision: '本地重新识读', cloud_vision: '云端重新识读', asr: '音频转写', frame_ocr: '画面识字', human_edit: '人工修正' };
const ACTION_LABEL = { adopt: '采用候选', edit: '人工修正', keep: '保留当前读本', pending: '待核验', reject: '拒绝候选' };
function copyBlocks(blocks: DossierBlock[]): DossierBlock[] { return structuredClone(blocks); }
/** Each material session owns its request identity; uncertain writes stay retryable with that identity. */
export function ReadingReviewPanel(props: Props) {
  const [open, setOpen] = useState(props.initiallyOpen ?? false); const [activated, setActivated] = useState(props.initiallyOpen ?? false);
  return <section className={styles.review}><ActionButton variant="quiet" onClick={() => { setActivated(true); setOpen(value => !value); }}>{open ? '收起核对' : '核对与修正'}</ActionButton>
    {activated ? <div hidden={!open}><ReviewContent key={`${props.domain}:${props.work}:${props.unit.ref}`} {...props} /></div> : null}
  </section>;
}
function ReviewContent({ unit, work, domain, onApplied, initialCandidate, originalReadable, showCurrentReading }: Props) {
  const versionLabel = useId();
  const query = useQuery({ queryKey: ['dossier-candidates', domain, work], queryFn: ({ signal }) => fetchCandidates(work, domain, signal), retry: false, gcTime: 0 });
  const [expected, setExpected] = useState(unit.currentRevisionRef);
  const [candidate, setCandidate] = useState<ReadingCandidate | null>(initialCandidate ?? null);
  const [blocks, setBlocks] = useState(() => copyBlocks(initialCandidate?.blocks ?? unit.blocks));
  const [editing, setEditing] = useState(false); const [verified, setVerified] = useState((initialCandidate?.verification ?? unit.reading?.verification) === 'verified');
  const [actor, setActor] = useState(''); const [reason, setReason] = useState('');
  const [resolved, setResolved] = useState<string[]>([]); const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<ReadingReviewRequest | null>(null); const [message, setMessage] = useState('');
  const inFlight = useRef(false); const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const changed = expected !== unit.currentRevisionRef;
  const candidates = (query.data ?? []).filter(value => value.unitRef === unit.ref);
  // Refresh review provenance without replacing the selected blocks, draft, or uncertain command.
  const candidateReview = candidate === null ? null : candidates.find(value => value.candidateRevisionRef === candidate.candidateRevisionRef)?.review
    ?? (initialCandidate?.candidateRevisionRef === candidate.candidateRevisionRef ? initialCandidate.review : null) ?? candidate.review;
  const options = candidate && !candidates.some(value => value.candidateRevisionRef === candidate.candidateRevisionRef) ? [...candidates, candidate] : candidates;
  const selectedRef = candidate?.candidateRevisionRef ?? expected;
  const sourceBlocks = candidate?.blocks ?? unit.blocks;
  const issues = [...new Map([...(candidate?.issues ?? unit.issues), ...sourceBlocks.flatMap(block => block.issues ?? [])].map(issue => [issue.key, issue])).values()];
  const originalUnavailable = unit.kind === 'image' && (unit.originalUrl === null || originalReadable === false);
  const metadataValid = actor.trim().length > 0 && Array.from(actor.trim()).length <= 120 && reason.trim().length > 0 && Array.from(reason.trim()).length <= 300;
  const disabled = busy || pending !== null || changed || selectedRef === null || originalUnavailable;
  function select(value: string) {
    const next = options.find(item => item.candidateRevisionRef === value) ?? null;
    setCandidate(next); setBlocks(copyBlocks(next?.blocks ?? unit.blocks)); setResolved([]); setEditing(false);
    setVerified(next?.verification === 'verified' || (next === null && unit.reading?.verification === 'verified')); setMessage('');
  }
  function reset() { setExpected(unit.currentRevisionRef); select(''); void query.refetch(); }
  async function submit(action: ReadingReviewRequest['action']) {
    if (inFlight.current || hideCachedReading(query.error) || (!pending && (changed || !selectedRef || originalUnavailable || !metadataValid))) return;
    const opinionOnly = action === 'pending' || action === 'reject';
    const command = pending ?? { commandRef: crypto.randomUUID(), expectedCurrentRevisionRef: expected, candidateRevisionRef: selectedRef!, action, ...(action === 'edit' ? { editedBlocks: copyBlocks(blocks) } : {}), verified: opinionOnly ? false : action === 'keep' && selectedRef !== expected ? unit.reading?.verification === 'verified' : verified, resolvedIssueKeys: action === 'edit' ? resolved : [], actor: actor.trim(), reason: reason.trim() };
    inFlight.current = true; setBusy(true); setMessage('');
    try {
      const receipt = await reviewReading(work, domain, unit.ref, command);
      if (!mounted.current) return;
      if (receipt.commandRef !== command.commandRef || receipt.unitRef !== unit.ref || receipt.candidateRevisionRef !== command.candidateRevisionRef || receipt.action !== command.action) throw new ApiError({ kind: 'invalid_response', status: 200 });
      if ((command.action === 'pending' || command.action === 'reject') && receipt.currentRevisionRef !== command.expectedCurrentRevisionRef) throw new ApiError({ kind: 'invalid_response', status: 200 });
      setPending(null); if (receipt.action !== 'pending') setCandidate(null); setEditing(false); setResolved([]); setExpected(receipt.currentRevisionRef); setVerified(receipt.action === 'pending' ? candidate?.verification === 'verified' : receipt.verification === 'verified');
      setMessage(receipt.action === 'keep' ? '已记录保留当前读本。' : receipt.action === 'edit' ? '完整修正版已保存并采用。' : receipt.action === 'pending' ? '已记录待核验意见，当前读本保持原样。' : receipt.action === 'reject' ? '已拒绝此候选，当前读本保持原样。' : '候选读本已采用。');
      onApplied(receipt); void query.refetch();
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) {
        setPending(null);
        setMessage(error.failure.code === 'reading_original_unavailable' ? '原始媒体当前不可读，本次人工决定未保存。请刷新底稿并恢复原始媒体后再核对。' : error.failure.status === 409 ? '当前读本或来源已变化，未覆盖新版本。请刷新底稿后使用最新读本。' : '本次操作未被接纳，当前读本未由此请求改变。请核对来源状态与修正内容。');
      } else { setPending(command); setMessage('保存回执尚未确认。核对同一请求会复用原命令与原内容，不创建第二次修正。'); }
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  if (hideCachedReading(query.error)) return <p role="alert">候选与修正内容当前不可读，来源可能已受限。<button type="button" onClick={() => void query.refetch()}>重新读取权限状态</button></p>;
  return <div className={styles.reviewContent} aria-busy={busy}>
    {showCurrentReading ? <section aria-label="当前采用读本"><h4>当前采用读本</h4>{unit.currentRevisionRef === null ? <p>尚未采用读本。</p> : <DossierBlocks blocks={unit.blocks} issues={unit.issues} />}</section> : null}
    {unit.reading?.review ? <p className={styles.note}>当前读本审核：{unit.reading.review.actor} · {unit.reading.review.reviewedAt}<br />核对依据：{unit.reading.review.reason}</p> : null}
    {candidateReview ? <p className={styles.note}>此候选最近意见：{ACTION_LABEL[candidateReview.action]} · {candidateReview.actor} · {candidateReview.reviewedAt}<br />核对依据：{candidateReview.reason}</p> : null}
    {query.isPending ? <p role="status">正在读取候选版本…</p> : null}
    {query.isError ? <p role="status">{expected === null ? '候选版本暂时不可读。' : '候选版本暂时不可读；当前读本仍可核对。'}<button type="button" onClick={() => void query.refetch()}>重读候选</button></p> : null}
    {options.length > 0 ? <label><span id={versionLabel}>核对版本</span><select aria-labelledby={versionLabel} disabled={busy || pending !== null || changed} value={candidate?.candidateRevisionRef ?? ''} onChange={event => select(event.target.value)}>
      <option value="">{expected === null ? '尚未采用读本' : '当前采用读本'}</option>{options.map((value, index) => <option key={value.candidateRevisionRef} value={value.candidateRevisionRef}>{`${PRODUCER[value.generatedBy]} · 候选 ${index + 1} · ${value.createdAt}`}</option>)}
    </select></label> : !query.isPending && !query.isError ? <p className={styles.note}>{expected === null ? '尚无可核对的读本，请先完成图片识别。' : '当前没有其他候选版本。'}</p> : null}
    {changed && pending === null ? <p role="status">底稿已有新版本，未提交的修改仍保留。<button type="button" onClick={reset}>使用最新读本</button></p> : null}
    {selectedRef !== null || pending !== null ? <>
    {originalUnavailable ? <p role="status">原始媒体当前不可读，已有文字仍可阅读。恢复原始媒体后才能提交新的人工决定。</p> : null}
    {editing ? <DossierBlockEditor blocks={blocks} onChange={setBlocks} disabled={disabled} /> : candidate ? <DossierBlocks blocks={candidate.blocks} issues={candidate.issues} /> : null}
    {editing ? <><DossierIssues issues={issues.filter(issue => issue.resolution === 'source_refresh')} />
      {issues.filter(issue => issue.resolution === 'reading_edit').map(issue => <label className={styles.checkLabel} key={issue.key}><input type="checkbox" disabled={disabled} checked={resolved.includes(issue.key)} onChange={event => setResolved(keys => event.target.checked ? [...keys, issue.key] : keys.filter(key => key !== issue.key))} />已对照来源解决：{issue.message}</label>)}
    </> : null}
    <label className={styles.checkLabel}><input type="checkbox" disabled={disabled} checked={verified} onChange={event => setVerified(event.target.checked)} />{selectedRef !== expected ? '已对照来源核验所选候选读本' : '已对照来源核验当前读本'}</label>
    <label>审核人<input value={actor} maxLength={120} disabled={busy || pending !== null} autoComplete="off" onChange={event => setActor(event.target.value)} /></label>
    <label>核对依据<textarea value={reason} maxLength={300} disabled={busy || pending !== null} onChange={event => setReason(event.target.value)} /></label>
    {candidate ? <p className={styles.note}>采用后替换当前读本；待核验与拒绝仅记录意见。</p> : null}
    <div className={styles.toolbar}>
      {pending ? <ActionButton busy={busy} onClick={() => void submit(pending.action)}>核对同一保存请求</ActionButton> : <>
        {editing ? <><ActionButton variant="primary" disabled={disabled || !metadataValid} onClick={() => void submit('edit')}>保存完整修正版</ActionButton><ActionButton disabled={busy} onClick={() => { setEditing(false); setBlocks(copyBlocks(candidate?.blocks ?? unit.blocks)); setResolved([]); }}>取消修正</ActionButton></> : <>
          {candidate && candidate.candidateRevisionRef !== expected ? <ActionButton variant="primary" disabled={disabled || !metadataValid} onClick={() => void submit('adopt')}>采用此候选</ActionButton> : null}
          <ActionButton disabled={disabled} onClick={() => { setBlocks(copyBlocks(candidate?.blocks ?? unit.blocks)); setResolved([]); setEditing(true); }}>逐块修正</ActionButton>
          <ActionButton disabled={disabled || expected === null || !metadataValid} onClick={() => void submit('keep')}>保留当前读本</ActionButton>
          {candidate ? <><ActionButton disabled={disabled || !metadataValid} onClick={() => void submit('pending')}>保持待核验</ActionButton><ActionButton disabled={disabled || !metadataValid} onClick={() => void submit('reject')}>拒绝此候选</ActionButton></> : null}
        </>}
      </>}
    </div></> : options.length > 0 ? <p className={styles.note}>选择候选后可核对与修正，选中不会自动采用。</p> : null}
    {message ? <p role="status">{message}</p> : null}
  </div>;
}
