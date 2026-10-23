import { useEffect, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { fetchCandidates, reviewReading } from './dossierApi';
import { DossierBlocks, DossierIssues } from './DossierBlocks';
import { DossierBlockEditor } from './DossierBlockEditor';
import { hideCachedReading } from './dossierPresent';
import type { DossierBlock, DossierMaterial, ReadingCandidate, ReadingReviewRequest } from './dossierTypes';
import styles from './Dossier.module.css';

interface Props { unit: DossierMaterial; work: string; domain: string; onApplied: () => void }
const PRODUCER = { platform: '平台原文', ordinary_ocr: '普通识字', local_vision: '本地重新识读', cloud_vision: '云端重新识读', asr: '音频转写', frame_ocr: '画面识字', human_edit: '人工修正' };
function copyBlocks(blocks: DossierBlock[]): DossierBlock[] { return structuredClone(blocks); }
/** Each material session owns its request identity; uncertain writes stay retryable with that identity. */
export function ReadingReviewPanel(props: Props) {
  const [open, setOpen] = useState(false); const [activated, setActivated] = useState(false);
  return <section className={styles.review}><ActionButton variant="quiet" onClick={() => { setActivated(true); setOpen(value => !value); }}>{open ? '收起核对' : '核对与修正'}</ActionButton>
    {activated ? <div hidden={!open}><ReviewContent key={`${props.domain}:${props.work}:${props.unit.ref}`} {...props} /></div> : null}
  </section>;
}
function ReviewContent({ unit, work, domain, onApplied }: Props) {
  const query = useQuery({ queryKey: ['dossier-candidates', domain, work], queryFn: ({ signal }) => fetchCandidates(work, domain, signal), retry: false, gcTime: 0 });
  const [expected, setExpected] = useState(unit.currentRevisionRef);
  const [candidate, setCandidate] = useState<ReadingCandidate | null>(null);
  const [blocks, setBlocks] = useState(() => copyBlocks(unit.blocks));
  const [editing, setEditing] = useState(false); const [verified, setVerified] = useState(unit.reading?.verification === 'verified');
  const [resolved, setResolved] = useState<string[]>([]); const [busy, setBusy] = useState(false);
  const [pending, setPending] = useState<ReadingReviewRequest | null>(null); const [message, setMessage] = useState('');
  const inFlight = useRef(false); const mounted = useRef(true);
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const changed = expected !== unit.currentRevisionRef;
  const candidates = (query.data ?? []).filter(value => value.unitRef === unit.ref);
  const selectedRef = candidate?.candidateRevisionRef ?? expected;
  const sourceBlocks = candidate?.blocks ?? unit.blocks;
  const issues = [...new Map([...(candidate?.issues ?? unit.issues), ...sourceBlocks.flatMap(block => block.issues ?? [])].map(issue => [issue.key, issue])).values()];
  const disabled = busy || pending !== null || changed || selectedRef === null;
  function select(value: string) {
    const next = candidates.find(item => item.candidateRevisionRef === value) ?? null;
    setCandidate(next); setBlocks(copyBlocks(next?.blocks ?? unit.blocks)); setResolved([]); setEditing(false);
    setVerified(next?.verification === 'verified' || (next === null && unit.reading?.verification === 'verified')); setMessage('');
  }
  function reset() { setExpected(unit.currentRevisionRef); select(''); void query.refetch(); }
  async function submit(action: ReadingReviewRequest['action']) {
    if (inFlight.current || hideCachedReading(query.error) || (!pending && (changed || !selectedRef))) return;
    const command = pending ?? { commandRef: crypto.randomUUID(), expectedCurrentRevisionRef: expected, candidateRevisionRef: selectedRef!, action, ...(action === 'edit' ? { editedBlocks: copyBlocks(blocks) } : {}), verified: action === 'keep' && selectedRef !== expected ? unit.reading?.verification === 'verified' : verified, resolvedIssueKeys: action === 'edit' ? resolved : [] };
    inFlight.current = true; setBusy(true); setMessage('');
    try {
      const receipt = await reviewReading(work, domain, unit.ref, command);
      if (!mounted.current) return;
      if (receipt.commandRef !== command.commandRef || receipt.unitRef !== unit.ref || receipt.candidateRevisionRef !== command.candidateRevisionRef || receipt.action !== command.action) throw new ApiError({ kind: 'invalid_response', status: 200 });
      setPending(null); setCandidate(null); setEditing(false); setResolved([]); setExpected(receipt.currentRevisionRef); setVerified(receipt.verification === 'verified');
      setMessage(receipt.action === 'keep' ? '已记录保留当前读本。' : receipt.action === 'edit' ? '完整修正版已保存并采用。' : '候选读本已采用。');
      onApplied(); void query.refetch();
    } catch (error) {
      if (!mounted.current) return;
      if (error instanceof ApiError && error.failure.kind === 'http' && [400, 401, 403, 404, 409, 410, 413, 415, 422].includes(error.failure.status)) {
        setPending(null);
        setMessage(error.failure.kind === 'http' && error.failure.status === 409 ? '当前读本或来源已变化，未覆盖新版本。请刷新底稿后使用最新读本。' : '本次操作未被接纳，当前读本未由此请求改变。请核对来源状态与修正内容。');
      } else { setPending(command); setMessage('保存回执尚未确认。核对同一请求会复用原命令与原内容，不创建第二次修正。'); }
    } finally { inFlight.current = false; if (mounted.current) setBusy(false); }
  }
  if (hideCachedReading(query.error)) return <p role="alert">候选与修正内容当前不可读，来源可能已受限。<button type="button" onClick={() => void query.refetch()}>重新读取权限状态</button></p>;
  return <div className={styles.reviewContent} aria-busy={busy}>
    {query.isPending ? <p role="status">正在读取候选版本…</p> : null}
    {query.isError ? <p role="status">候选版本暂时不可读；当前读本仍可核对。<button type="button" onClick={() => void query.refetch()}>重读候选</button></p> : null}
    {candidates.length > 0 ? <label>核对版本<select disabled={busy || pending !== null || changed} value={candidate?.candidateRevisionRef ?? ''} onChange={event => select(event.target.value)}>
      <option value="">当前采用读本</option>{candidates.map((value, index) => <option key={value.candidateRevisionRef} value={value.candidateRevisionRef}>{`${PRODUCER[value.generatedBy]} · 候选 ${index + 1} · ${value.createdAt}`}</option>)}
    </select></label> : <p className={styles.note}>当前没有其他候选版本，可直接对照来源修正。</p>}
    {changed && pending === null ? <p role="status">底稿已有新版本，未提交的修改仍保留。<button type="button" onClick={reset}>使用最新读本</button></p> : null}
    {editing ? <DossierBlockEditor blocks={blocks} onChange={setBlocks} disabled={disabled} /> : candidate ? <DossierBlocks blocks={candidate.blocks} issues={candidate.issues} /> : null}
    {editing ? <><DossierIssues issues={issues.filter(issue => issue.resolution === 'source_refresh')} />
      {issues.filter(issue => issue.resolution === 'reading_edit').map(issue => <label className={styles.checkLabel} key={issue.key}><input type="checkbox" disabled={disabled} checked={resolved.includes(issue.key)} onChange={event => setResolved(keys => event.target.checked ? [...keys, issue.key] : keys.filter(key => key !== issue.key))} />已对照来源解决：{issue.message}</label>)}
    </> : null}
    <label className={styles.checkLabel}><input type="checkbox" disabled={disabled} checked={verified} onChange={event => setVerified(event.target.checked)} />{selectedRef !== expected ? '已对照来源核验所选候选读本' : '已对照来源核验当前读本'}</label>
    <div className={styles.toolbar}>
      {pending ? <ActionButton busy={busy} onClick={() => void submit(pending.action)}>核对同一保存请求</ActionButton> : <>
        {editing ? <><ActionButton variant="primary" disabled={disabled} onClick={() => void submit('edit')}>保存完整修正版</ActionButton><ActionButton disabled={busy} onClick={() => { setEditing(false); setBlocks(copyBlocks(candidate?.blocks ?? unit.blocks)); setResolved([]); }}>取消修正</ActionButton></> : <>
          {candidate && candidate.candidateRevisionRef !== expected ? <ActionButton variant="primary" disabled={disabled} onClick={() => void submit('adopt')}>采用此候选</ActionButton> : null}
          <ActionButton disabled={disabled} onClick={() => { setBlocks(copyBlocks(candidate?.blocks ?? unit.blocks)); setResolved([]); setEditing(true); }}>逐块修正</ActionButton>
          <ActionButton disabled={disabled || expected === null} onClick={() => void submit('keep')}>保留当前读本</ActionButton>
        </>}
      </>}
    </div>{message ? <p role="status">{message}</p> : null}
  </div>;
}
