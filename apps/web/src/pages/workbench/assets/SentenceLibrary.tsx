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
const STATUSES: readonly SentenceStatus[] = ['review', 'ok', 'fix', 'off'];

function FindingList({ findings }: { findings: readonly Finding[] }) {
  if (findings.length === 0) return null;
  return <ul className={styles.findings} aria-label="入库检查">{findings.map((finding, index) => <li key={`${finding.rule}-${index}`} data-level={finding.level}>
    <span className={styles.chip}>{levelLabel[finding.level]}</span><span>{finding.message}</span>
  </li>)}</ul>;
}

/** A sentence being added or rewritten, checked as it is typed; a cure promise cannot be added. */
function SentenceEditor({ sentence, onSaved }: { sentence: Sentence | null; onSaved: (code: string) => void }) {
  const [identities] = useState(() => new RequestIdentities());
  const [group, setGroup] = useState<BridgeGroup>(sentence?.group ?? 'parent');
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
    {check.data?.notRun.map(item => <p key={item.rule} className={styles.hint}>未检查（{item.rule}）：{item.reason}。</p>)}
    {blocked ? <p role="alert" className={styles.problem}>含有疗效承诺，不能加入句库。</p> : null}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className={styles.actions}>
      <ActionButton type="submit" variant="primary" busy={busy} disabled={!text.trim() || blocked || !fresh || unchanged}>
        {sentence ? '保存为新的一版（回到待审）' : '加入句库（待审）'}
      </ActionButton>
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
  const [group, setGroup] = useState<BridgeGroup | ''>('');
  const [status, setStatus] = useState<SentenceStatus | ''>('');
  const [selected, setSelected] = useState<string | 'new' | null>(null);
  const sentences = useQuery({
    queryKey: ['creation-assets', 'sentences', group, status],
    queryFn: ({ signal }) => api.listSentences({ ...(group ? { group } : {}), ...(status ? { status } : {}) }, signal), gcTime: 0,
  });
  const items = sentences.data ?? [];
  const current = items.find(item => item.code === selected) ?? null;
  const reload = () => void queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
  return <section className={styles.assetBody} aria-label="衔接句">
    <div>
      <div className={styles.toolbar}>
        <h2>衔接句</h2>
        <ActionButton variant="outline" onClick={() => setSelected('new')}>＋ 新增一句</ActionButton>
      </div>
      <p className={styles.hint}>只有“可用”的句子会在写稿时被选用；新增和改写的句子都先待审。</p>
      <div className={styles.actions}>
        <label>身份
          <select aria-label="按身份筛选" value={group} onChange={event => setGroup(event.target.value as BridgeGroup | '')}>
            <option value="">全部</option>{GROUPS.map(value => <option key={value} value={value}>{groupLabel[value]}</option>)}
          </select>
        </label>
        <label>状态
          <select aria-label="按状态筛选" value={status} onChange={event => setStatus(event.target.value as SentenceStatus | '')}>
            <option value="">全部</option>{STATUSES.map(value => <option key={value} value={value}>{sentenceStatusLabel[value]}</option>)}
          </select>
        </label>
      </div>
      {sentences.isPending ? <p role="status">正在读取…</p> : null}
      {sentences.isError ? <p role="alert" className={styles.problem}>读取失败，可以<ActionButton variant="quiet" onClick={() => void sentences.refetch()}>重试</ActionButton></p> : null}
      {sentences.isSuccess && items.length === 0 ? <p>{group || status ? '没有符合筛选的句子。' : '句库还是空的，先新增一句。'}</p> : null}
      <ul className={styles.list}>{items.map(item => <li key={item.code}>
        <button type="button" aria-current={item.code === selected ? 'true' : undefined} onClick={() => setSelected(item.code)}>
          <span>{item.text}</span>
          <small className={styles.meta}><span>{item.code}</span><span>{groupLabel[item.group]}</span>
            <span className={styles.statusChip}>{sentenceStatusLabel[item.status]}</span><span>用过 {item.usedCount} 篇</span></small>
        </button>
      </li>)}</ul>
    </div>
    <div className={styles.detail}>
      {selected === 'new' ? <SentenceEditor key="new" sentence={null} onSaved={code => { setSelected(code); reload(); }} />
        : current ? <>
          <SentenceEditor key={`${current.code}:${current.revision}`} sentence={current} onSaved={reload} />
          <FindingList findings={current.entryFindings} />
          <ReviewBar key={`${current.code}:${current.revision}:${current.status}`} sentence={current} onDone={reload} />
        </> : <p className={styles.hint}>在左边选一句，或新增一句。</p>}
    </div>
  </section>;
}
