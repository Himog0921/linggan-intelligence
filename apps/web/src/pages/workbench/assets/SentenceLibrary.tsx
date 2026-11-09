import { useEffect, useState } from 'react';
import { keepPreviousData, useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import type { BridgeGroup } from '../workbenchModel';
import * as api from './assetsApi';
import { blockedFindings, type Finding, type ReviewAction, type Sentence, type SentenceStatus } from './assetsModel';
import { groupLabel, levelLabel, sentenceStatusLabel } from './assetsText';
import styles from './Assets.module.css';

const GROUPS: readonly BridgeGroup[] = ['parent', 'coach'];
const STATUS_TONE: Record<SentenceStatus, string> = { review: 'info', ok: 'ok', fix: 'warn', off: 'unk' };
const GROUP_HINT: Record<BridgeGroup, string> = { parent: '简介写明“陪跑营学员”后可用', coach: '简介写明“陪跑老师”后可用' };
/** Column headings follow the reference layout, which names the reader rather than the group key. */
const COLUMN_TITLE: Record<BridgeGroup, string> = { parent: '参加过陪跑营的家长', coach: '机构陪跑老师' };

function FindingList({ findings }: { findings: readonly Finding[] }) {
  if (findings.length === 0) return null;
  return <ul className={styles.findings} aria-label="入库检查">{findings.map((finding, index) => <li key={`${finding.rule}-${index}`} data-level={finding.level}>
    <span className={styles.chip}>{levelLabel[finding.level]}</span><span>{finding.message}</span>
  </li>)}</ul>;
}

/** A sentence being added or rewritten, checked as it is typed; a cure promise cannot be added. */
function SentenceEditor({ sentence, initialGroup = 'parent', onSaved, onClose }: {
  sentence: Sentence | null; initialGroup?: BridgeGroup; onSaved: (code: string) => void; onClose?: () => void;
}) {
  const [identities] = useState(() => new RequestIdentities());
  const [group, setGroup] = useState<BridgeGroup>(sentence?.group ?? initialGroup);
  const [text, setText] = useState(sentence?.text ?? '');
  const [settled, setSettled] = useState(text);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const [refused, setRefused] = useState<Finding[]>([]);
  useEffect(() => {
    const timer = window.setTimeout(() => setSettled(text), 400);
    return () => window.clearTimeout(timer);
  }, [text]);
  const check = useQuery({
    queryKey: ['creation-assets', 'sentence-check', settled.trim()], enabled: settled.trim().length > 0,
    queryFn: ({ signal }) => api.checkSentence(settled.trim(), signal), retry: false, gcTime: 0, placeholderData: keepPreviousData,
  });
  const fresh = settled === text && check.isSuccess && !check.isFetching;
  const blocked = check.data?.blocked === true;
  const unchanged = sentence !== null && sentence.text === text.trim() && sentence.group === group;

  async function save() {
    if (busy || !text.trim()) return;
    setBusy(true);
    setMessage('');
    setRefused([]);
    try {
      if (sentence) {
        const body = { expectedRevision: sentence.revision, group, text: text.trim() };
        const requestId = identities.keyFor('edit', JSON.stringify(body));
        const receipt = await api.editSentence(sentence.code, { ...body, requestId });
        identities.settle('edit');
        onSaved(receipt.sentence.code);
      } else {
        const body = { group, text: text.trim(), origin: 'manual' as const };
        const requestId = identities.keyFor('create', JSON.stringify(body));
        const receipt = await api.createSentence({ ...body, requestId });
        identities.settle('create');
        onSaved(receipt.sentence.code);
      }
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle(sentence ? 'edit' : 'create');
      const found = blockedFindings(error);
      if (found) setRefused(found);
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  return <form className={styles.form} aria-label={sentence ? `改写 ${sentence.code}` : '新增衔接句'} onSubmit={event => { event.preventDefault(); void save(); }}>
    <h3>{sentence ? `${sentence.code} · 第 ${sentence.revision} 版` : '新增一句衔接句'}</h3>
    <label>适合哪种身份
      <select value={group} disabled={busy} onChange={event => setGroup(event.target.value as BridgeGroup)}>
        {GROUPS.map(value => <option key={value} value={value}>{groupLabel[value]}</option>)}
      </select>
    </label>
    <label>这句话
      <textarea rows={3} value={text} maxLength={500} disabled={busy} onChange={event => setText(event.target.value)} />
      <small>一行，不换行；如实说明自己和机构的关系，不写效果承诺。</small>
    </label>
    <FindingList findings={refused.length ? refused : check.data?.findings ?? []} />
    {check.data?.notRun.map(item => <p key={item.rule} className={styles.hint}>未检查：{item.reason}。</p>)}
    {blocked ? <p role="alert" className={styles.problem}>含有疗效承诺，不能加入句库。</p> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className={styles.actions}>
      <ActionButton type="submit" variant="primary" busy={busy} disabled={!text.trim() || blocked || !fresh || unchanged}>
        {sentence ? '保存为新的一版（回到待审）' : '加入句库（待审）'}
      </ActionButton>
      {onClose ? <ActionButton variant="quiet" disabled={busy} onClick={onClose}>收起</ActionButton> : null}
    </div>
  </form>;
}

function ReviewBar({ sentence, onDone }: { sentence: Sentence; onDone: () => void }) {
  const [identities] = useState(() => new RequestIdentities());
  const [note, setNote] = useState('');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  async function act(action: ReviewAction) {
    if (busy) return;
    setBusy(true);
    setMessage('');
    try {
      const body = { expectedRevision: sentence.revision, action, note: note.trim() || null };
      const requestId = identities.keyFor('review', JSON.stringify(body));
      await api.reviewSentence(sentence.code, { ...body, requestId });
      identities.settle('review');
      setNote('');
      onDone();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle('review');
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  const status = sentence.status;
  return <section className={styles.form} aria-label="审核">
    <h3>审核</h3>
    <p>现在是“{sentenceStatusLabel[status]}”。{status === 'ok' ? '写稿时会被选用。' : '审核通过前不会出现在写稿里。'}{sentence.statusNote ? ` 说明：${sentence.statusNote}` : ''}</p>
    {status !== 'off' ? <label>退回待改或停用时的说明
      <input value={note} maxLength={500} disabled={busy} onChange={event => setNote(event.target.value)} />
    </label> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className={styles.actions}>
      {status === 'review' || status === 'fix' ? <ActionButton variant="primary" busy={busy} onClick={() => void act('approve')}>审核通过</ActionButton> : null}
      {status === 'review' || status === 'ok' ? <ActionButton variant="outline" disabled={busy || !note.trim()} onClick={() => void act('request_fix')}>退回待改</ActionButton> : null}
      {status !== 'off' ? <ActionButton variant="outline" disabled={busy} onClick={() => void act('disable')}>停用</ActionButton> : null}
      {status === 'off' ? <ActionButton variant="outline" busy={busy} onClick={() => void act('enable')}>重新启用（回到待审）</ActionButton> : null}
    </div>
  </section>;
}

export function SentenceLibrary() {
  const queryClient = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [selected, setSelected] = useState<string | 'new' | null>(null);
  const [newGroup, setNewGroup] = useState<BridgeGroup>('parent');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const sentences = useQuery({
    queryKey: ['creation-assets', 'sentences', '', ''],
    queryFn: ({ signal }) => api.listSentences({}, signal), gcTime: 0,
  });
  const items = sentences.data ?? [];
  const current = items.find(item => item.code === selected) ?? null;
  const reload = () => { void (async () => {
    await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
    await queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
  })(); };
  async function quick(sentence: Sentence, action: ReviewAction) {
    if (busy) return;
    setBusy(true); setMessage('');
    try {
      const body = { expectedRevision: sentence.revision, action, note: null };
      const requestId = identities.keyFor(`quick:${action}`, JSON.stringify([sentence.code, body]));
      await api.reviewSentence(sentence.code, { ...body, requestId });
      identities.settle(`quick:${action}`);
      reload();
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle(`quick:${action}`);
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  return <section aria-label="衔接句">
    <header className="as-head">
      <div className="grow">
        <h1 className="h-obj">衔接句</h1>
        <p>按账号和机构的真实关系分组。账号简介写明这层关系后，写稿时只出现对应那一组；新增的句子先过自动检查，再等审核。</p>
      </div>
    </header>
    {sentences.isPending ? <p role="status">正在读取…</p> : null}
    {sentences.isError ? <p role="alert" className={styles.problem}>读取失败，可以<ActionButton variant="quiet" onClick={() => void sentences.refetch()}>重试</ActionButton></p> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    {sentences.isSuccess ? <div className="bcols">
      {GROUPS.map(group => {
        const groupItems = (sentences.data ?? []).filter(item => item.group === group);
        return <section className="bcol" key={group}>
          <div className="bcol-head"><h3>{COLUMN_TITLE[group]}</h3><p>{GROUP_HINT[group]} · {groupItems.length} 句</p></div>
          {groupItems.map(item => <div key={item.code} className={`bitem${item.status === 'fix' ? ' fix' : item.status === 'off' ? ' off' : ''}`}>
            <div className="aux data">{item.code}</div>
            <p>{item.text}</p>
            {item.entryFindings.length ? <div className="live-checks">{item.entryFindings.map((finding, index) => <div key={`${finding.rule}-${index}`}><span className={`dot ${finding.level === 'must' ? 'bad' : 'warn'}`} />{finding.message}</div>)}</div> : null}
            <div className="row">
              <span className={`chip ${STATUS_TONE[item.status]}`}>{sentenceStatusLabel[item.status]}</span>
              {item.origin === 'saved_back' ? <span className="chip demo">原稿</span> : null}
              <span className="aux">用过 <span className="data">{item.usedCount}</span> 次</span>
              <span style={{ flex: 1 }} />
              {item.status === 'review' || item.status === 'fix' ? <button type="button" className="btn quiet sub" disabled={busy} onClick={() => void quick(item, 'approve')}>审核通过</button> : null}
              <button type="button" className="btn quiet sub" disabled={busy} onClick={() => setSelected(item.code)}>改写</button>
              {item.status !== 'off' ? <button type="button" className="btn quiet sub" disabled={busy} onClick={() => void quick(item, 'disable')}>停用</button>
                : <button type="button" className="btn quiet sub" disabled={busy} onClick={() => void quick(item, 'enable')}>启用</button>}
            </div>
          </div>)}
          <div className="badd">
            <label className="lbl">新增一句</label>
            <button type="button" className="btn sm" disabled={busy} onClick={() => { setNewGroup(group); setSelected('new'); }}>加入（待审）</button>
          </div>
        </section>;
      })}
      <section className="bcol">
        <div className="bcol-head"><h3>和机构无关</h3><p>不放衔接句</p></div>
        <div className="bitem">
          <p>账号和机构没有关系时，正文不提产品。想让读者找到我们，走公开身份的账号，或者请博主合作并标明合作。</p>
          <div className="row"><span className="chip unk">不植入</span></div>
        </div>
      </section>
    </div> : null}
    {selected === 'new' ? <div className="lib-insp"><SentenceEditor key={`new:${newGroup}`} sentence={null} initialGroup={newGroup} onSaved={code => { setSelected(code); reload(); }} onClose={() => setSelected(null)} /></div> : null}
    {current ? <>
      <div className="lib-insp"><SentenceEditor key={`${current.code}:${current.revision}`} sentence={current} onSaved={() => reload()} onClose={() => setSelected(null)} /></div>
      <FindingList findings={current.entryFindings} />
      <div className="lib-insp"><ReviewBar key={`${current.code}:${current.revision}:${current.status}`} sentence={current} onDone={reload} /></div>
    </> : null}
  </section>;
}
