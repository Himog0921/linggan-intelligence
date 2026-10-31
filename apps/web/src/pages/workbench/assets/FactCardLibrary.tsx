import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { humanMoment } from '../../../shared/formatting';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { FactCard, FactCardView } from './assetsModel';
import styles from './Assets.module.css';

const lines = (text: string) => text.split(/\n/u).map(line => line.trim()).filter(Boolean);

function cardState(view: FactCardView): string {
  const { latest, confirmed } = view;
  if (!confirmed) return '未确认';
  return confirmed.revision === latest.revision ? `已确认 第 ${latest.revision} 版` : `生效第 ${confirmed.revision} 版 · 第 ${latest.revision} 版待确认`;
}

/**
 * One product fact card: the organisation's own wording. Saving never changes what is in force;
 * only confirming a revision does, and the older confirmed one stays in force until then.
 */
function CardForm({ card, onChanged }: { card: FactCard | null; onChanged: (code: string) => void }) {
  const [identities] = useState(() => new RequestIdentities());
  const [name, setName] = useState(card?.name ?? '');
  const [allowed, setAllowed] = useState((card?.allowedNames ?? []).join('\n'));
  const [banned, setBanned] = useState((card?.bannedNames ?? []).join('\n'));
  const [what, setWhat] = useState(card?.what ?? '');
  const [who, setWho] = useState(card?.who ?? '');
  const [canSay, setCanSay] = useState(card?.canSay ?? '');
  const [cannotSay, setCannotSay] = useState(card?.cannotSay ?? '');
  const [mustNote, setMustNote] = useState(card?.mustNote ?? '');
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const confirmed = card?.confirmedAt != null;

  async function run(action: 'save' | 'confirm') {
    if (busy) return;
    setBusy(true);
    setMessage('');
    try {
      if (action === 'save') {
        const body = { expectedRevision: card?.revision ?? 0, name: name.trim(), allowedNames: lines(allowed), bannedNames: lines(banned),
          what, who, canSay, cannotSay, mustNote };
        const requestId = identities.keyFor('save', JSON.stringify(body));
        const receipt = await api.saveFactCard(card?.code ?? null, { ...body, requestId });
        identities.settle('save');
        onChanged(receipt.card.code);
      } else if (card) {
        const body = { expectedRevision: card.revision };
        const requestId = identities.keyFor('confirm', JSON.stringify(body));
        const receipt = await api.confirmFactCard(card.code, { ...body, requestId });
        identities.settle('confirm');
        onChanged(receipt.card.code);
      }
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle(action);
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  const field = (label: string, value: string, set: (value: string) => void, rows = 3, hint?: string) => <label>{label}
    <textarea rows={rows} value={value} disabled={busy} maxLength={2000} onChange={event => set(event.target.value)} />{hint ? <small>{hint}</small> : null}</label>;
  return <form className={styles.form} aria-label={card ? `产品事实卡 ${card.code}` : '新建产品事实卡'} onSubmit={event => { event.preventDefault(); void run('save'); }}>
    <h3>{card ? `${card.code} · 第 ${card.revision} 版` : '新建产品事实卡'}</h3>
    <label>规范名称
      <input value={name} maxLength={80} disabled={busy} onChange={event => setName(event.target.value)} />
    </label>
    {field('可用叫法（一行一个）', allowed, setAllowed, 3, '规范名称本身不用再写一遍。')}
    {field('不要用的叫法（一行一个）', banned, setBanned, 3, '稿子里出现这些叫法会提醒；不能和可用叫法重复。')}
    {field('是什么', what, setWhat)}
    {field('适合谁', who, setWho)}
    {field('能讲的', canSay, setCanSay)}
    {field('不能讲的', cannotSay, setCannotSay)}
    {field('必带的提示', mustNote, setMustNote)}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className={styles.actions}>
      <ActionButton type="submit" variant="primary" busy={busy} disabled={!name.trim()}>{card ? '保存为新的一版' : '保存'}</ActionButton>
      {card && !confirmed ? <ActionButton variant="outline" disabled={busy} onClick={() => void run('confirm')}>确认这一版</ActionButton> : null}
    </div>
    {card ? <p className={styles.hint}>{confirmed ? `这一版已确认（${humanMoment(card.confirmedAt)}），检查按它执行。` : '这一版还没有确认：检查仍按上一个已确认的版本执行，没有已确认的版本时叫法检查不运行。'}</p> : null}
  </form>;
}

export function FactCardLibrary() {
  const queryClient = useQueryClient();
  const [selected, setSelected] = useState<string | 'new' | null>(null);
  const cards = useQuery({ queryKey: ['creation-assets', 'fact-cards'], queryFn: ({ signal }) => api.listFactCards(signal), gcTime: 0 });
  const items = cards.data ?? [];
  const current = items.find(item => item.code === selected) ?? null;
  const changed = (code: string) => {
    setSelected(code);
    void (async () => {
      await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
      await queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
    })();
  };
  return <section className={styles.assetBody} aria-label="产品事实卡">
    <div>
      <div className={styles.toolbar}>
        <h2>产品事实卡</h2>
        <ActionButton variant="outline" onClick={() => setSelected('new')}>＋ 新建</ActionButton>
      </div>
      <p className={styles.hint}>写明产品的正式叫法、能讲和不能讲的话。稿子的检查读取“已确认”的那一版。</p>
      {cards.isPending ? <p role="status">正在读取…</p> : null}
      {cards.isError ? <p role="alert" className={styles.problem}>读取失败，可以<ActionButton variant="quiet" onClick={() => void cards.refetch()}>重试</ActionButton></p> : null}
      {cards.isSuccess && items.length === 0 ? <p>还没有产品事实卡。没有已确认的卡时，叫法检查不会运行。</p> : null}
      <ul className={styles.list}>{items.map(item => <li key={item.code}>
        <button type="button" aria-current={item.code === selected ? 'true' : undefined} onClick={() => setSelected(item.code)}>
          <b>{item.latest.name}</b><small>{item.code} · {cardState(item)}</small>
        </button>
      </li>)}</ul>
    </div>
    <div className={styles.detail}>
      {selected === 'new' ? <CardForm key="new" card={null} onChanged={changed} />
        : current ? <CardForm key={`${current.code}:${current.latest.revision}`} card={current.latest} onChanged={changed} />
          : <p className={styles.hint}>在左边选一张卡，或新建一张。</p>}
    </div>
  </section>;
}
