import { useEffect, useRef, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { reviewReading } from './dossierApi';
import { materialIdentity } from './dossierPresent';
import { DossierIssues } from './DossierBlocks';
import { DossierBlockEditor } from './DossierBlockEditor';
import type { DossierBlock, DossierMaterial, ReadingReviewRequest, ReadingReviewReceipt } from './dossierTypes';
import styles from './Dossier.module.css';

interface Props {
  unit: DossierMaterial; work: string; domain: string; onApplied: (receipt: ReadingReviewReceipt) => void;
  initiallyOpen?: boolean; originalReadable?: boolean;
}
function copyBlocks(blocks: DossierBlock[]): DossierBlock[] { return structuredClone(blocks); }

/** A correction edits the canonical reading. An uncertain save keeps its original identity and draft. */
export function ReadingReviewPanel(props: Props) {
  const [open, setOpen] = useState(props.initiallyOpen ?? false);
  const [activated, setActivated] = useState(props.initiallyOpen ?? false);
  return <section className={styles.review}><ActionButton variant="quiet" onClick={() => { setActivated(true); setOpen(value => !value); }}>{open ? '收起修正' : '修正文字'}</ActionButton>
    {activated ? <div hidden={!open}><ReviewContent key={`${props.domain}:${props.work}:${materialIdentity(props.unit)}`} {...props} onCancel={() => setOpen(false)} /></div> : null}
  </section>;
}
function ReviewContent({ unit, work, domain, onApplied, originalReadable, onCancel }: Props & { onCancel: () => void }) {
  const [expected, setExpected] = useState(unit.currentRevisionRef);
  const [sourceVersion, setSourceVersion] = useState(unit.sourceVersion);
  const [blocks, setBlocks] = useState(() => copyBlocks(unit.blocks));
  const [verified, setVerified] = useState(unit.reading?.verification === 'verified');
  const [actor, setActor] = useState(''); const [reason, setReason] = useState('');
  const [resolved, setResolved] = useState<string[]>([]); const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<ReadingReviewRequest | null>(null); const [message, setMessage] = useState('');
  const [confirmedRevision, setConfirmedRevision] = useState<string | null>(null);
  const inFlight = useRef(false); const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  if (confirmedRevision !== null && unit.currentRevisionRef === confirmedRevision) {
    setBlocks(copyBlocks(unit.blocks)); setExpected(unit.currentRevisionRef); setSourceVersion(unit.sourceVersion);
    setVerified(unit.reading?.verification === 'verified'); setConfirmedRevision(null);
  }
  const changed = expected !== unit.currentRevisionRef || sourceVersion !== unit.sourceVersion;
  const originalUnavailable = unit.sourceState !== 'present' || (unit.kind === 'image' && (unit.originalUrl === null || originalReadable === false));
  const metadataValid = actor.trim().length > 0 && Array.from(actor.trim()).length <= 120 && reason.trim().length > 0 && Array.from(reason.trim()).length <= 300;
  const disabled = busy || pending !== null || confirmedRevision !== null || changed || originalUnavailable;
  const issues = [...new Map([...unit.issues, ...unit.blocks.flatMap(block => block.issues ?? [])].map(issue => [issue.key, issue])).values()];
  function reset() {
    setExpected(unit.currentRevisionRef); setSourceVersion(unit.sourceVersion); setBlocks(copyBlocks(unit.blocks));
    setVerified(unit.reading?.verification === 'verified'); setResolved([]); setConfirmedRevision(null); setMessage('');
  }
  async function submit() {
    if (inFlight.current || (!pending && (confirmedRevision !== null || changed || originalUnavailable || !metadataValid))) return;
    const command: ReadingReviewRequest = pending ?? { commandRef: crypto.randomUUID(), unitRef: unit.ref, sourceVersion, expectedCurrentRevisionRef: expected, action: 'edit', editedBlocks: copyBlocks(blocks), verified, resolvedIssueKeys: resolved, actor: actor.trim(), reason: reason.trim() };
    inFlight.current = true; setBusy(true); setMessage('');
    try {
      const receipt = await reviewReading(work, domain, command.unitRef, command);
      if (!mounted.current) return;
      if (receipt.commandRef !== command.commandRef || receipt.unitRef !== command.unitRef || receipt.action !== 'edit' || receipt.currentRevisionRef === null || receipt.adoption !== 'human_edited') throw new ApiError({ kind: 'invalid_response', status: 200 });
      setPending(null); setResolved([]); setExpected(receipt.currentRevisionRef); setConfirmedRevision(receipt.currentRevisionRef); setVerified(receipt.verification === 'verified');
      setMessage('修正已保存。'); onApplied(receipt);
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) {
        setPending(null);
        setMessage(error.failure.code === 'reading_original_unavailable' ? '原件当前不可读，修正未保存。恢复原件后可重试。' : error.failure.status === 409 ? '读本或来源已变化，未覆盖新版本。请刷新底稿后使用最新读本。' : '修正未被接纳，请核对来源状态与修正内容。');
      } else { setPending(command); setMessage('保存回执尚未确认。核对同一请求会复用原命令与原内容。'); }
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  if (unit.sourceState === 'restricted') return <p role="alert">来源当前受限，文字修正暂不可用。</p>;
  return <div className={styles.reviewContent} aria-busy={busy}>
    {changed && pending === null && confirmedRevision === null ? <p role="status">底稿已有新版本，未提交的修改仍保留。<button type="button" onClick={reset}>使用最新读本</button></p> : null}
    {originalUnavailable ? <p role="status">原件当前不可读，恢复原件后可保存修正。</p> : null}
    <DossierBlockEditor blocks={blocks} onChange={setBlocks} disabled={disabled} />
    <DossierIssues issues={issues.filter(issue => issue.resolution === 'source_refresh')} />
    {issues.filter(issue => issue.resolution === 'reading_edit').map(issue => <label className={styles.checkLabel} key={issue.key}><input type="checkbox" disabled={disabled} checked={resolved.includes(issue.key)} onChange={event => setResolved(keys => event.target.checked ? [...keys, issue.key] : keys.filter(key => key !== issue.key))} />已对照原件解决：{issue.message}</label>)}
    <label className={styles.checkLabel}><input type="checkbox" disabled={disabled} checked={verified} onChange={event => setVerified(event.target.checked)} />已对照原件核验文字</label>
    <label>修正人<input value={actor} maxLength={120} disabled={disabled} autoComplete="off" onChange={event => setActor(event.target.value)} /></label>
    <label>修正说明<textarea value={reason} maxLength={300} disabled={disabled} onChange={event => setReason(event.target.value)} /></label>
    <div className={styles.toolbar}>{pending ? <ActionButton busy={busy} onClick={() => void submit()}>核对同一保存请求</ActionButton> : <>
      <ActionButton variant="primary" disabled={disabled || !metadataValid} onClick={() => void submit()}>保存修正</ActionButton>
      <ActionButton disabled={busy} onClick={() => { reset(); onCancel(); }}>取消</ActionButton>
    </>}</div>
    {message ? <p role="status">{message}</p> : null}
  </div>;
}
