import { useState } from 'react';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { humanMoment } from '../../../shared/formatting';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { FactCard, FactCardView } from './assetsModel';
import styles from './Assets.module.css';

const splitNames = (text: string) => text.split(/[、,，\n]/u).map(part => part.trim()).filter(Boolean);

function cardState(view: FactCardView): string {
  const { latest, confirmed } = view;
  if (!confirmed) return '未确认';
  return confirmed.revision === latest.revision ? `已确认 第 ${latest.revision} 版` : `生效第 ${confirmed.revision} 版 · 第 ${latest.revision} 版待确认`;
}

type FieldKey = 'name' | 'allowed' | 'banned' | 'what' | 'who' | 'canSay' | 'cannotSay' | 'mustNote';
const FIELD_LABEL: Record<FieldKey, string> = {
  name: '规范名称', allowed: '可用叫法', banned: '不用的叫法', what: '是什么', who: '适合谁',
  canSay: '能讲的', cannotSay: '不能讲的', mustNote: '必带提示',
};
const GROUPS: readonly { label: string; fields: readonly FieldKey[] }[] = [
  { label: '产品叫法', fields: ['name', 'allowed', 'banned'] },
  { label: '产品与对象', fields: ['what', 'who'] },
  { label: '表达边界', fields: ['canSay', 'cannotSay', 'mustNote'] },
];
type Draft = Record<FieldKey, string>;
const draftOf = (card: FactCard | null): Draft => ({
  name: card?.name ?? '', allowed: (card?.allowedNames ?? []).join('、'), banned: (card?.bannedNames ?? []).join('、'),
  what: card?.what ?? '', who: card?.who ?? '', canSay: card?.canSay ?? '', cannotSay: card?.cannotSay ?? '', mustNote: card?.mustNote ?? '',
});

/**
 * One product fact card: the organisation's own wording. Saving never changes what is in force;
 * only confirming a revision does, and the older confirmed one stays in force until then.
 */
export function FactCardLibrary() {
  const queryClient = useQueryClient();
  const [identities] = useState(() => new RequestIdentities());
  const [selected, setSelected] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const cards = useQuery({ queryKey: ['creation-assets', 'fact-cards'], queryFn: ({ signal }) => api.listFactCards(signal), gcTime: 0 });
  const items = cards.data ?? [];
  const current = items.find(item => item.code === (selected ?? items[0]?.code)) ?? null;
  const card = current?.latest ?? null;
  const dirty = editing && draft !== null && JSON.stringify(draft) !== JSON.stringify(draftOf(card));
  const changed = (code: string) => {
    setSelected(code); setEditing(false); setDraft(null); setMessage('');
    void (async () => {
      await queryClient.cancelQueries({ queryKey: ['creation-assets', 'draft-checks'] });
      await queryClient.invalidateQueries({ queryKey: ['creation-assets'] });
    })();
  };
  function startEdit() { setEditing(true); setDraft(draftOf(card)); setMessage(''); }
  async function save() {
    if (!draft || busy) return;
    setBusy(true); setMessage('');
    const body = { expectedRevision: card?.revision ?? 0, name: draft.name.trim(), allowedNames: splitNames(draft.allowed), bannedNames: splitNames(draft.banned),
      what: draft.what, who: draft.who, canSay: draft.canSay, cannotSay: draft.cannotSay, mustNote: draft.mustNote };
    const requestId = identities.keyFor('save', JSON.stringify(body));
    try {
      const receipt = await api.saveFactCard(card?.code ?? null, { ...body, requestId });
      identities.settle('save');
      changed(receipt.card.code);
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle('save');
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function confirm() {
    if (!card || busy) return;
    setBusy(true); setMessage('');
    const body = { expectedRevision: card.revision };
    const requestId = identities.keyFor('confirm', JSON.stringify(body));
    try {
      const receipt = await api.confirmFactCard(card.code, { ...body, requestId });
      identities.settle('confirm');
      changed(receipt.card.code);
    } catch (error) {
      if (error instanceof ApiError && error.answered) identities.settle('confirm');
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }
  const set = (key: FieldKey, value: string) => setDraft(current => current ? { ...current, [key]: value } : current);
  const viewValue = (key: FieldKey) => key === 'allowed' ? (card?.allowedNames ?? []).join('、')
    : key === 'banned' ? (card?.bannedNames ?? []).join('、') : card?.[key] ?? '';
  const head = <div className={`product-head${editing ? ' editing' : ''}`}>
    <div className="product-heading">
      <h1 className="product-title">{editing && draft ? draft.name.trim() || '未填写产品名称' : card?.name ?? '未填写产品名称'}</h1>
      <span className="chip warn">{!card ? '新建' : current ? cardState(current) : '未填写'}</span>
    </div>
    <div className="product-actions">
      {editing ? <>
        <span className="product-edit-state" role="status">{dirty ? '未保存' : '编辑中'}</span>
        <ActionButton variant="quiet" disabled={busy} onClick={() => { setEditing(false); setDraft(null); setMessage(''); }}>取消</ActionButton>
        <ActionButton type="submit" variant="primary" busy={busy} disabled={!draft?.name.trim()}>保存</ActionButton>
      </> : <>
        {card && !card.confirmedAt ? <ActionButton variant="outline" disabled={busy} onClick={() => void confirm()}>确认这一版</ActionButton> : null}
        <ActionButton variant="outline" disabled={busy} onClick={startEdit}>{card ? '编辑' : '新建'}</ActionButton>
      </>}
    </div>
  </div>;
  const body = <>
    {head}
    {GROUPS.map(group => <section key={group.label} className="product-group" aria-label={group.label}>
      <dl>{group.fields.filter(key => editing || key !== 'name').map(key => {
        const id = `prod-${key}`;
        const value = editing ? draft?.[key] ?? '' : viewValue(key);
        const lined = key === 'name' || key === 'allowed' || key === 'banned';
        return <div key={key} className="product-field">
          <dt className="product-label">{editing ? <label htmlFor={id}>{FIELD_LABEL[key]}</label> : FIELD_LABEL[key]}</dt>
          <dd className="product-value">{editing
            ? lined
              ? <input id={id} className="input" value={value} required={key === 'name'} disabled={busy} maxLength={2000}
                placeholder={key === 'name' ? '例如：3V1 家庭干预陪跑' : '多个叫法用顿号分隔'} onChange={event => set(key, event.target.value)} />
              : <textarea id={id} className="input" rows={2} value={value} disabled={busy} maxLength={2000} onChange={event => set(key, event.target.value)} />
            : <p className={`product-text${value.trim() ? '' : ' empty'}`}>{value.trim() || '未填写'}</p>}</dd>
        </div>;
      })}</dl>
    </section>)}
  </>;
  return <section aria-label="产品事实卡">
    {items.length > 1 ? <div className="lib-tools" role="group" aria-label="产品卡">
      <div className="seg">{items.map(item => <button type="button" key={item.code} aria-pressed={item.code === current?.code} disabled={busy || editing}
        onClick={() => changed(item.code)}>{item.latest.name}</button>)}</div>
    </div> : null}
    {cards.isPending && !items.length ? <p role="status">正在读取…</p> : null}
    {cards.isError ? <p role="alert" className={styles.problem}>读取失败，可以<ActionButton variant="quiet" onClick={() => void cards.refetch()}>重试</ActionButton></p> : null}
    {!items.length && !editing ? <p className="lib-empty">还没有产品事实卡。没有已确认的卡时，叫法检查不会运行。</p> : null}
    {editing
      ? <form className="product" aria-label={card ? `产品事实卡 ${card.code}` : '新建产品事实卡'} onSubmit={event => { event.preventDefault(); void save(); }}>{body}</form>
      : <div className="product">{body}</div>}
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    {card ? <p className={styles.hint}>{card.confirmedAt ? `这一版已确认（${humanMoment(card.confirmedAt)}），检查按它执行。` : '这一版还没有确认：检查仍按上一个已确认的版本执行，没有已确认的版本时叫法检查不运行。'}</p> : null}
  </section>;
}
