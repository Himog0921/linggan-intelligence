import { useEffect, useId, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import { ModalDialog } from '../../../corpus/ModalDialog';
import type { Revision, WorkDocument } from '../workbenchModel';
import { readWork } from '../workbenchApi';
import * as api from './assetsApi';
import { blockedFindings, type Finding, type Fix, type Location } from './assetsModel';
import { applyRange, bridgeFromSentence, countByLevel, identityCommand, levelHeading, levelLabel, nextSentence, withBridge, withoutTag } from './assetsText';
import { BridgeSlot } from './BridgeSlot';
import { IdentityForm } from './IdentityForm';
import styles from './Assets.module.css';

type SaveState = 'saved' | 'dirty' | 'saving' | 'failed' | 'unknown' | 'conflict';
const LEVELS = ['must', 'warn', 'hint'] as const;
const CHECK_DELAY_MS = 450;

async function writeClipboard(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** Holds the last value that has been still for a moment, and never while a Chinese input is open. */
function useSettled(document: WorkDocument, hold: boolean) {
  const key = JSON.stringify(api.checkDocument(document));
  const [state, setState] = useState({ key, document });
  useEffect(() => {
    if (hold || state.key === key) return;
    const timer = window.setTimeout(() => setState({ key, document }), CHECK_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [key, document, hold, state.key]);
  return { settled: state.document, settledKey: state.key, currentKey: key };
}

/**
 * Checks and copy for one draft (CREATION-ASSETS-001 S1a). The rules live on the server: this panel
 * shows what they found, applies the fixes the server offered through the normal save, and only
 * copies the saved revision once nothing is left in 必改.
 */
export function CheckPanel({ workId, domainRef, document, revision, saveState, composing, locked, active, pendingConfirmation, setPendingConfirmation, confirmationIdentities, tagsDraft, onTagsChange, edit, saveCurrent, onGoCover, copyContainer, sensitiveVisible = true, withdrawalInvalidated = false }: {
  workId: string; domainRef: string; document: WorkDocument; revision: Revision; saveState: SaveState; composing: boolean; locked: boolean; active: boolean;
  pendingConfirmation: string | null; setPendingConfirmation: (receipt: string | null) => void; confirmationIdentities: RequestIdentities;
  tagsDraft: string; onTagsChange: (value: string) => void; edit: (next: WorkDocument) => void; saveCurrent: () => Promise<Revision>; onGoCover: () => void;
  copyContainer?: HTMLElement | null;
  sensitiveVisible?: boolean; withdrawalInvalidated?: boolean;
}) {
  const uid = useId();
  const queryClient = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [notice, setNotice] = useState('');
  const [copying, setCopying] = useState(false);
  const [manual, setManual] = useState<{ text: string; receiptRef: string } | null>(null);
  const access = useRef({ permitted: active && sensitiveVisible && !withdrawalInvalidated, generation: 0 });
  const permitted = active && sensitiveVisible && !withdrawalInvalidated;
  if (access.current.permitted !== permitted) {
    access.current = { permitted, generation: access.current.generation + 1 };
  }
  useEffect(() => {
    access.current = { permitted, generation: access.current.generation };
    return () => { access.current = { permitted: false, generation: access.current.generation + 1 }; };
  }, []);
  const [identityOpen, setIdentityOpen] = useState(false);
  const [bioConfirm, setBioConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const accountSelect = useRef<HTMLSelectElement>(null);
  const bridgeText = useRef<HTMLTextAreaElement>(null);
  const slotTop = useRef<HTMLDivElement>(null);
  const skipChosen = useRef(false);
  useEffect(() => { if (withdrawalInvalidated) setManual(null); }, [withdrawalInvalidated]);
  const checkInput = useMemo(() => ({ ...document, tags: tagsDraft.split(/\s+/u).filter(Boolean) }), [document, tagsDraft]);
  const { settled, settledKey, currentKey } = useSettled(checkInput, composing);
  const fresh = settledKey === currentKey && !composing;

  const accounts = useQuery({ queryKey: ['creation-assets', 'accounts', domainRef], queryFn: ({ signal }) => api.listAccounts(domainRef, signal), gcTime: 0 });
  const report = useQuery({
    queryKey: ['creation-assets', 'checks', workId, settledKey], queryFn: ({ signal }) => api.checkDraft(settled, signal, workId),
    retry: false, staleTime: 0, gcTime: 0, placeholderData: keepPreviousData,
  });
  const wasActive = useRef(active);
  useEffect(() => {
    if (active && !wasActive.current) void report.refetch();
    wasActive.current = active;
  }, [active, report.refetch]);
  const state = report.data?.bridgeState ?? null;
  const usable = useQuery({
    queryKey: ['creation-assets', 'candidates', document.accountRef], enabled: state === 'ready' && !!document.accountRef,
    queryFn: ({ signal }) => api.candidates(document.accountRef ?? '', signal), gcTime: 0,
  });
  const copied = useQuery({ queryKey: ['creation-assets', 'copy-status', workId], queryFn: ({ signal }) => api.copyStatus(workId, signal), gcTime: 0 });

  const findings = report.data?.findings ?? [];
  const counts = countByLevel(findings);
  const account = accounts.data?.find(item => item.accountRef === document.accountRef) ?? null;
  const reportOk = report.isSuccess && fresh && !report.isFetching;
  const wasCopied = copied.data?.copied?.revisionId === revision.id && saveState === 'saved';
  const saveBlocked = saveState === 'failed' || saveState === 'unknown' || saveState === 'conflict';
  const canCopy = reportOk && counts.must === 0 && !saveBlocked && !copying && !locked && !composing && !pendingConfirmation;
  const copyWhy = composing ? '请先完成中文输入' : saveBlocked ? '稿件还没有保存成功' : !reportOk ? '正在检查…' : counts.must > 0 ? '必改清零后才能复制' : '';
  const fixable = reportOk && !locked && !busy;

  useEffect(() => {
    if (!active || !reportOk || state !== 'ready' || !report.data?.findings.some(f => f.rule === 'R-M4') || locked || busy || copying || skipChosen.current || document.bridge?.skip || document.bridge?.sentence || !usable.data?.items.length) return;
    const first = usable.data.items[0];
    if (first) edit(withBridge(document, { skip: false, sentence: bridgeFromSentence(first) }));
  }, [active, reportOk, report.data, state, locked, busy, copying, document, usable.data, edit]);

  function reload() {
    void queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
  }
  function select(element: HTMLInputElement | HTMLTextAreaElement | null, start: number, end: number) {
    if (!element) return;
    element.focus();
    element.setSelectionRange(Math.min(start, element.value.length), Math.min(end, element.value.length));
  }
  function locate(location: Location) {
    const root = window.document;
    switch (location.kind) {
      case 'paragraph': return select(root.querySelector<HTMLTextAreaElement>(`[data-paragraph="${location.id}"]`), location.start, location.end);
      case 'title': return select(root.querySelector<HTMLInputElement>('input[aria-label="正文标题"]'), location.start, location.end);
      case 'bridge': return select(bridgeText.current, location.start, location.end);
      case 'tag': return root.querySelector<HTMLInputElement>('input[aria-label="确认发布标签"]')?.focus();
      case 'cover': return onGoCover();
      case 'slot': return slotTop.current?.scrollIntoView?.({ block: 'nearest' });
      case 'text': return undefined;
    }
  }
  /** Rewrites the located text through the ordinary edit, so undo, autosave and the checks all follow. */
  function rewrite(location: Location, replacement: string | null) {
    if (location.kind === 'paragraph') {
      const target = document.paragraphs.find(paragraph => paragraph.id === location.id);
      if (!target) return;
      const text = applyRange(target.text, location.start, location.end, replacement);
      edit({ ...document, paragraphs: document.paragraphs.map(paragraph => paragraph.id === location.id ? { ...paragraph, text } : paragraph) });
    } else if (location.kind === 'title') {
      edit({ ...document, title: applyRange(document.title, location.start, location.end, replacement) });
    } else if (location.kind === 'bridge' && document.bridge?.sentence) {
      const sentence = document.bridge.sentence;
      edit(withBridge(document, { sentence: { ...sentence, text: applyRange(sentence.text, location.start, location.end, replacement), edited: true } }));
    }
  }
  async function pick(swap: boolean) {
    const items = usable.data?.items ?? (document.accountRef ? (await api.candidates(document.accountRef)).items : []);
    const next = nextSentence(items, swap ? document.bridge?.sentence?.sentenceCode ?? null : null);
    if (next) edit(withBridge(document, { skip: false, sentence: bridgeFromSentence(next) }));
    else setNotice('这个身份还没有审核通过的衔接句，先去资产库的“衔接句”里添加并审核。');
  }
  async function fix(finding: Finding, action: Fix) {
    if (!fixable) return;
    setNotice('');
    try {
      switch (action.action) {
        case 'skipBridge': skipChosen.current = true; return edit(withBridge(document, { skip: true }));
        case 'autoPickBridge': return await pick(false);
        case 'swapBridge': return await pick(true);
        case 'chooseAccount': return accountSelect.current?.focus();
        case 'openIdentity': return setIdentityOpen(true);
        case 'confirmBioWritten': return setBioConfirm(true);
        case 'editByHand': return locate(finding.location);
        case 'deleteMatch': return rewrite(finding.location, null);
        case 'replaceText': return rewrite(finding.location, action.replacement);
        case 'removeTag': return finding.location.kind === 'tag' ? onTagsChange(withoutTag(tagsDraft, finding.location.index)) : undefined;
        case 'copyBioSuggestion':
          setNotice((await writeClipboard(action.text)) ? '已复制建议简介：去账号简介里粘贴，改好后回来点“我已改好简介”。' : `没能自动复制，请手动复制：${action.text}`);
          return;
      }
    } catch (error) {
      setNotice(api.assetsErrorText(error));
    }
  }
  async function confirmBio() {
    if (!account || busy) return;
    setBusy(true);
    try {
      const requestId = identities.keyFor('bio', JSON.stringify({ revision: account.identity?.revision ?? 0, bio: true }));
      await api.putIdentity(account.accountRef, identityCommand(account.identity, { bioDisclosesRelation: true }, requestId));
      identities.settle('bio');
      setBioConfirm(false);
      reload();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle('bio');
      setNotice(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function saveBack(sentence: { group: 'parent' | 'coach'; text: string }) {
    setBusy(true);
    try {
      const body = { group: sentence.group, text: sentence.text, origin: 'saved_back' as const };
      const requestId = identities.keyFor('save-back', JSON.stringify(body));
      const receipt = await api.createSentence({ ...body, requestId });
      identities.settle('save-back');
      setNotice(`已存回衔接句库（${receipt.sentence.code}），待审核后才会被别的稿子用到。`);
      reload();
    } catch (error) {
      const refused = blockedFindings(error);
      if (error instanceof ApiError && error.answered) identities.settle('save-back');
      setNotice(refused ? `没有存回：${refused[0]?.message ?? '含有不能入库的说法'}` : api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function confirmClipboard(receiptRef: string) {
    const requestId = confirmationIdentities.keyFor('confirm', receiptRef);
    await api.confirmCopy(workId, receiptRef, { requestId });
    confirmationIdentities.settle('confirm');
    await queryClient.invalidateQueries({ queryKey: ['creation-assets', 'copy-status', workId] });
  }
  async function recordClipboard(receiptRef: string, warnCount = 0) {
    setPendingConfirmation(receiptRef);
    setCopying(true);
    try {
      await confirmClipboard(receiptRef);
      setPendingConfirmation(null);
      setNotice(warnCount ? `已复制到剪贴板（还有 ${warnCount} 条提醒没处理）。这不等于已发布。` : '已复制到剪贴板，已记录为已复制。这不等于已发布。');
    } catch (error) {
      setNotice(`复制已完成，记录结果未确认：${api.assetsErrorText(error)}`);
    } finally {
      setCopying(false);
    }
  }
  async function copy() {
    if (!canCopy || !access.current.permitted) return;
    const generation = access.current.generation;
    const stillCurrent = () => access.current.permitted && access.current.generation === generation;
    setCopying(true);
    setNotice('');
    let saved: Revision;
    try {
      saved = await saveCurrent();
    } catch {
      setNotice('保存没有完成，所以没有复制，当前输入仍保留。');
      setCopying(false);
      return;
    }
    if (!stillCurrent()) { setCopying(false); setNotice('稿件状态已变化，没有复制。'); return; }
    try {
      const requestId = identities.keyFor('copy', saved.id);
      const ready = await api.prepareCopy(workId, { requestId, expectedRevision: saved.id });
      identities.settle('copy');
      if (!stillCurrent()) { setNotice('稿件状态已变化，没有复制。'); return; }
      const written = await writeClipboard(ready.text);
      if (!written && !stillCurrent()) { setNotice('稿件状态已变化，没有复制。'); return; }
      if (!written) {
        setManual({ text: ready.text, receiptRef: ready.receiptRef });
        return;
      }
      await recordClipboard(ready.receiptRef, ready.warnCount);
    } catch (error) {
      const refused = blockedFindings(error);
      if (error instanceof ApiError && error.answered) identities.settle('copy');
      setNotice(refused ? '还有必须先改的问题，没有复制。' : api.assetsErrorText(error));
      if (refused) reload();
    } finally {
      setCopying(false);
    }
  }
  async function copyTitle() {
    if (!access.current.permitted) return;
    const generation = access.current.generation;
    const stillCurrent = () => access.current.permitted && access.current.generation === generation;
    setCopying(true);
    try {
      const saved = await saveCurrent();
      if (!stillCurrent()) { setNotice('稿件状态已变化，没有复制标题。'); return; }
      const latest = await readWork(workId);
      if (!stillCurrent()) { setNotice('稿件状态已变化，没有复制标题。'); return; }
      if (latest.experienceResetRequired || latest.revision.id !== saved.id) {
        setNotice('稿件已发生变化，请先重新读取并处理撤回的经历，再复制标题。');
        return;
      }
      if (latest.revision.document.title !== saved.document.title ||
          (latest.revision.document.title.includes('[来源当前不可使用]') && latest.sourceStatuses.some(source => source.state === 'restricted' || source.state === 'unavailable'))) {
        setNotice('标题中的来源当前不可使用，请先处理来源，再复制标题。');
        return;
      }
      setNotice((await writeClipboard(latest.revision.document.title.trim())) ? '标题已复制。' : '没能自动复制，请手动选中标题复制。');
    } catch {
      setNotice('保存或核验没有完成，所以没有复制标题。');
    } finally {
      setCopying(false);
    }
  }

  const grouped = LEVELS.map(level => ({ level, items: findings.filter(finding => finding.level === level) })).filter(group => group.items.length > 0);
  const copyPanel = <section aria-label="复制" className={`${styles.copy} ${copyContainer ? styles.footerCopy : ''}`}>
    <p>{wasCopied ? '已复制（还没有发布）' : '草稿中'}</p>
    <div className={styles.actions}>
      <ActionButton variant="primary" busy={copying} disabled={!canCopy} onClick={() => void copy()}>{copying ? '复制中…' : '复制到小红书'}</ActionButton>
      <ActionButton variant="outline" disabled={!document.title.trim() || !canCopy} onClick={() => void copyTitle()}>复制标题</ActionButton>
    </div>
    {!canCopy && copyWhy ? <p className={styles.hint}>{copyWhy}</p> : null}
    {pendingConfirmation ? <p role="status">复制记录结果未确认。<ActionButton variant="outline" disabled={copying} onClick={() => void recordClipboard(pendingConfirmation)}>重新确认记录</ActionButton></p> : null}
    {notice ? <p role="status">{notice}</p> : null}
  </section>;
  return <aside className={styles.panel} aria-label="检查与复制">
    <h2 className={styles.panelHeading}>检查与复制{reportOk && findings.length > 0 ? <span aria-label={`${findings.length} 条检查结果`}>{findings.length}</span> : null}</h2>
    <section aria-label="检查结果" aria-live="polite">
      <h3>检查结果</h3>
      {report.isPending ? <p>正在检查…</p> : report.isError ? <p role="alert" className={styles.problem}>检查读取失败。{api.assetsErrorText(report.error)}<ActionButton variant="quiet" onClick={() => void report.refetch()}>重试</ActionButton></p> : <>
        <p className={styles.counts}>必改 {counts.must} · 提醒 {counts.warn} · 提示 {counts.hint}{!reportOk ? '（更新中）' : ''}</p>
        {grouped.length === 0 && reportOk ? <p>没有发现问题。</p> : null}
        {grouped.map(group => <section key={group.level} aria-label={levelHeading[group.level]}>
          <h4>{levelHeading[group.level]}</h4>
          <ul className={styles.findings}>{group.items.map((finding, index) => <li key={`${finding.rule}-${index}`} data-level={finding.level}>
            <span className={styles.chip}>{levelLabel[finding.level]}</span>
            <span>{finding.message}</span>
            <div className={styles.fixes}>
              {finding.location.kind !== 'slot' && finding.location.kind !== 'text' && !finding.fixes.some(f => f.action === 'editByHand') ? <ActionButton variant="quiet" disabled={!fixable} onClick={() => locate(finding.location)}>找到位置</ActionButton> : null}
              {finding.fixes.map(item => <ActionButton key={item.action + item.label} variant="outline" disabled={!fixable} onClick={() => void fix(finding, item)}>{item.label}</ActionButton>)}
            </div>
          </li>)}</ul>
        </section>)}
        {report.data?.notRun.map(item => <p key={item.rule} className={styles.hint}>未检查：{item.reason}。</p>)}
      </>}
    </section>
    <div ref={slotTop}>
      <label>所属账号
        <select ref={accountSelect} aria-label="所属账号" disabled={locked || accounts.isPending}
          value={document.accountRef ?? ''} onChange={event => edit({ ...document, accountRef: event.target.value || null })}>
          <option value="">还没选</option>
          {document.accountRef && !account && !accounts.isPending ? <option value={document.accountRef}>当前账号（不在可写稿的小红书账号里）</option> : null}
          {(accounts.data ?? []).map(item => <option key={item.accountRef} value={item.accountRef}>{item.accountName}</option>)}
        </select>
      </label>
      {accounts.isError ? <p role="alert" className={styles.problem}>账号读取失败，可以<ActionButton variant="quiet" onClick={() => void accounts.refetch()}>重试</ActionButton></p> : null}
      {account ? <p className={styles.hint}>{account.identity ? '' : '这个号的身份还没有填写。'}<ActionButton variant="quiet" disabled={locked} onClick={() => setIdentityOpen(true)}>编辑账号身份</ActionButton></p> : null}
    </div>
    <BridgeSlot state={report.data ? state : null} document={document} usable={usable.data?.items ?? null} disabled={!fixable} textRef={bridgeText} onChange={next => { skipChosen.current = next.bridge?.skip === true; edit(next); }} onSaveBack={sentence => void saveBack(sentence)} />
    {state === 'ready' && usable.isPending ? <p>正在读取可用衔接句…</p> : null}
    {state === 'ready' && usable.isError ? <p role="alert" className={styles.problem}>衔接句读取失败。<ActionButton variant="quiet" onClick={() => void usable.refetch()}>重试</ActionButton></p> : null}
    {copyContainer ? createPortal(copyPanel, copyContainer) : copyPanel}
    {identityOpen && document.accountRef && account ? <ModalDialog labelledBy={`${uid}-identity`} className={styles.dialog} onClose={() => setIdentityOpen(false)}>
      <IdentityForm accountRef={account.accountRef} accountName={account.accountName} identity={account.identity}
        onSaved={() => { setIdentityOpen(false); reload(); }} onStale={reload} onCancel={() => setIdentityOpen(false)} />
    </ModalDialog> : null}
    {bioConfirm && account ? <ModalDialog labelledBy={`${uid}-bio`} className={styles.dialog} onClose={() => setBioConfirm(false)}>
      <h3 id={`${uid}-bio`}>确认简介已写明</h3>
      <p>你已经在“{account.accountName}”的小红书简介里写明了和机构的关系吗？确认后，这个号才会出现衔接句。</p>
      <div className={styles.actions}>
        <ActionButton variant="primary" busy={busy} onClick={() => void confirmBio()}>确认已写明</ActionButton>
        <ActionButton variant="outline" disabled={busy} onClick={() => setBioConfirm(false)}>取消</ActionButton>
      </div>
    </ModalDialog> : null}
    {manual && sensitiveVisible ? <ModalDialog labelledBy={`${uid}-manual`} className={styles.dialog} onClose={() => setManual(null)}>
      <h3 id={`${uid}-manual`}>请手动复制</h3>
      <p>浏览器没有允许自动复制。请选中下面的文字复制，再点“我已复制”。</p>
      <textarea readOnly aria-label="待复制的文字" rows={10} value={manual.text} data-initial-focus onFocus={event => event.currentTarget.select()} />
      <div className={styles.actions}>
        <ActionButton variant="primary" onClick={() => {
          const receiptRef = manual.receiptRef;
          setManual(null);
          void recordClipboard(receiptRef);
        }}>我已复制</ActionButton>
        <ActionButton variant="outline" onClick={() => setManual(null)}>放弃</ActionButton>
      </div>
    </ModalDialog> : null}
  </aside>;
}
