import { useId, useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ModalDialog } from '../../../corpus/ModalDialog';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import type { Revision } from '../workbenchModel';
import * as api from './assetsApi';
import styles from './Assets.module.css';

type Command = { requestId: string; expectedRevision: string; publicationRef: string };
export type PublicationPending = { kind: 'link'; command: Command } | { kind: 'unlink'; linkRef: string; requestId: string };
/** Associates an operator-selected record; it does not send anything to the platform. */
export function PublicationLinker({ workId, domainRef, saveCurrent, disabled, pending, onPending, blocked = false, onResetBlock = () => {} }: {
  workId: string; domainRef: string; saveCurrent: () => Promise<Revision>; disabled: boolean;
  pending: PublicationPending | null; onPending: (pending: PublicationPending | null) => void; blocked?: boolean; onResetBlock?: () => void;
}) {
  const uid = useId();
  const client = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const pendingLink = pending?.kind === 'link' ? pending.command : null;
  const pendingUnlink = pending?.kind === 'unlink' ? pending : null;
  const [open, setOpen] = useState(false);
  const [revision, setRevision] = useState<string | null>(null);
  const [selected, setSelected] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');
  const candidates = useQuery({ queryKey: ['creation-assets', 'publication-candidates', workId, revision],
    queryFn: ({ signal }) => api.publicationCandidates(workId, signal), enabled: open && !!revision && !pendingLink, retry: false, gcTime: 0 });
  const links = useQuery({ queryKey: ['creation-assets', 'publication-links', workId],
    queryFn: ({ signal }) => api.publicationLinks(workId, signal), enabled: open, retry: false, gcTime: 0 });
  async function start() {
    setOpen(true); setNotice('');
    if (pendingLink) { setRevision(pendingLink.expectedRevision); setSelected(pendingLink.publicationRef); return; }
    setBusy(true); setRevision(null); setSelected('');
    try { setRevision((await saveCurrent()).id); }
    catch { setNotice('保存未完成，暂不能关联发布记录。'); }
    finally { setBusy(false); }
  }
  async function link() {
    if (!revision || !selected || busy || pendingUnlink) return;
    const current = pendingLink ?? { expectedRevision: revision, publicationRef: selected,
      requestId: identities.keyFor('publication-link', JSON.stringify([revision, selected])) };
    onPending({ kind: 'link', command: current });
    setBusy(true); setNotice('');
    try {
      const receipt = await api.linkPublication(workId, current);
      if (receipt.revisionId !== current.expectedRevision || receipt.publicationRef !== current.publicationRef) throw new Error('invalid receipt');
      identities.settle('publication-link'); onPending(null);
      await client.invalidateQueries({ queryKey: ['creation-works', domainRef] });
      await client.invalidateQueries({ queryKey: ['creation-assets', 'publication-links', workId] });
      setNotice('已关联运营发布记录。');
      setSelected(''); setRevision(null);
    } catch (error) {
      if (error instanceof ApiError && error.answered) { identities.settle('publication-link'); onPending(null); setNotice(api.assetsErrorText(error)); }
      else setNotice('关联结果尚未确认。请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  async function unlink(linkRef: string) {
    if (busy || pendingLink || (pendingUnlink && pendingUnlink.linkRef !== linkRef)) return;
    if (!pendingUnlink && !window.confirm('撤销该稿件与发布记录的关联？运营发布记录仍会保留。')) return;
    const current = pendingUnlink ?? { linkRef, requestId: identities.keyFor('publication-unlink', linkRef) };
    onPending({ kind: 'unlink', linkRef: current.linkRef, requestId: current.requestId }); setBusy(true); setNotice('');
    try {
      const receipt = await api.unlinkPublication(workId, current.linkRef, current.requestId);
      if (receipt.linkRef !== current.linkRef) throw new Error('invalid receipt');
      onPending(null); identities.settle('publication-unlink');
      await client.invalidateQueries({ queryKey: ['creation-works', domainRef] });
      await client.invalidateQueries({ queryKey: ['creation-assets', 'publication-links', workId] });
      await client.invalidateQueries({ queryKey: ['creation-assets', 'publication-candidates', workId] });
      setNotice('已撤销关联，运营发布记录仍保留。');
    } catch (error) {
      if (error instanceof ApiError && error.answered) { identities.settle('publication-unlink'); onPending(null); setNotice(api.assetsErrorText(error)); }
      else setNotice('撤销结果尚未确认，请沿用原请求重试。');
    } finally { setBusy(false); }
  }
  return <><ActionButton variant="quiet" disabled={disabled && !pending} onClick={() => void start()}>关联发布记录</ActionButton>
    {open ? <ModalDialog labelledBy={`${uid}-title`} className={styles.dialog} onClose={() => { if (!busy) setOpen(false); }}>
      <h2 id={`${uid}-title`}>关联运营发布记录</h2>{blocked ? <p role="alert">关联操作结果未确认，请先留在此稿并重试。 <button type="button" onClick={onResetBlock}>返回稿件</button></p> : null}
      <p>选择此账号已经登记的发布记录，并关联到当前保存版本。这个操作不会发布笔记。</p>
      {links.isError ? <p role="alert">已关联的发布记录读取失败。<ActionButton variant="quiet" onClick={() => void links.refetch()}>重试</ActionButton></p> : null}
      {links.data?.length ? <section aria-label="已关联发布记录"><h3>已关联</h3>{links.data.map(item => <div key={item.linkRef}><p>{item.publishedOn ?? '日期未填'} · {item.title ?? '标题未填'}{!item.available ? ' · 记录当前不可用' : ''}{item.revisionId !== revision ? ' · 对应其他保存版本' : ''}</p><ActionButton variant="quiet" disabled={busy || !!pendingLink || !!pendingUnlink} onClick={() => void unlink(item.linkRef)}>撤销关联</ActionButton></div>)}</section> : null}
      {pendingUnlink ? <p>上一笔撤销结果未确认。<ActionButton variant="quiet" disabled={busy} onClick={() => void unlink(pendingUnlink!.linkRef)}>沿用原请求重试撤销</ActionButton></p> : null}
      {busy ? <p>正在处理…</p> : null}
      {candidates.isError && !pendingLink ? <p role="alert">发布记录读取失败。<ActionButton variant="quiet" onClick={() => void candidates.refetch()}>重试</ActionButton></p> : null}
      {revision && !pendingLink && candidates.isSuccess ? <><label>发布记录<select value={selected} onChange={event => setSelected(event.target.value)}><option value="">请选择</option>{candidates.data.map(item => <option key={item.publicationRef} value={item.publicationRef}>{item.publishedOn ?? '日期未填'} · {item.title ?? '标题未填'} · {item.url ?? '链接未填'}</option>)}</select></label>{!candidates.data.length ? <p>该账号没有可关联的发布记录。</p> : null}</> : null}
      {pendingLink ? <p>上一请求结果未确认，重试会沿用原来的记录和请求编号。</p> : null}
      {notice ? <p role="status">{notice}</p> : null}
      <div className={styles.actions}><ActionButton variant="outline" disabled={busy} onClick={() => setOpen(false)}>关闭</ActionButton><ActionButton variant="primary" disabled={busy || !!pendingUnlink || !revision || !selected} onClick={() => void link()}>{pendingLink ? '重试关联' : '确认关联'}</ActionButton></div>
    </ModalDialog> : null}</>;
}
