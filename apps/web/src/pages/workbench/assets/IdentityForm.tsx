import { useId, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { Identity, OrgRelation } from './assetsModel';
import { relationLabel } from './assetsText';
import styles from './Assets.module.css';

type Answer = 'yes' | 'no' | 'unknown';
const toAnswer = (value: boolean | null): Answer => (value === null ? 'unknown' : value ? 'yes' : 'no');
const fromAnswer = (value: Answer): boolean | null => (value === 'unknown' ? null : value === 'yes');
const RELATIONS: readonly OrgRelation[] = ['parent', 'coach', 'none', 'unknown'];

function Choice({ legend, name, value, options, onChange, disabled }: {
  legend: string; name: string; value: string; options: readonly { value: string; label: string }[];
  onChange: (value: string) => void; disabled: boolean;
}) {
  return <fieldset className={styles.choice} disabled={disabled}>
    <legend>{legend}</legend>
    <div>{options.map(option => <label key={option.value} className={styles.radio}>
      <input type="radio" name={name} value={option.value} checked={value === option.value} onChange={() => onChange(option.value)} />
      <span>{option.label}</span>
    </label>)}</div>
  </fieldset>;
}

/**
 * What the operators know about one account. "不清楚" is a real answer and is stored as unknown,
 * never as "no"; the bridge sentence only unlocks when the relation is confirmed and the bio says so.
 */
export function IdentityForm({ accountRef, accountName, identity, onSaved, onStale, onCancel }: {
  accountRef: string; accountName: string; identity: Identity | null; onSaved: (identity: Identity) => void;
  onStale?: () => void; onCancel?: () => void;
}) {
  const uid = useId();
  const [identities] = useState(() => new RequestIdentities());
  const [relation, setRelation] = useState<OrgRelation>(identity?.orgRelation ?? 'unknown');
  const [bio, setBio] = useState<Answer>(toAnswer(identity?.bioDisclosesRelation ?? null));
  const [person, setPerson] = useState<Answer>(toAnswer(identity?.hasRealPerson ?? null));
  const [role, setRole] = useState(identity?.backingPersonRole ?? '');
  const [experience, setExperience] = useState<Answer>(toAnswer(identity?.canProvideExperience ?? null));
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const related = relation === 'parent' || relation === 'coach';
  const roleText = role.trim();
  const invalid = person === 'yes' && roleText.length > 40;

  async function save() {
    if (busy || invalid) return;
    setBusy(true);
    setMessage('');
    const body = {
      expectedRevision: identity?.revision ?? 0, orgRelation: relation,
      bioDisclosesRelation: related ? fromAnswer(bio) : null,
      hasRealPerson: fromAnswer(person), backingPersonRole: person === 'yes' && roleText ? roleText : null,
      canProvideExperience: fromAnswer(experience),
    };
    const requestId = identities.keyFor('identity', JSON.stringify(body));
    try {
      const receipt = await api.putIdentity(accountRef, { ...body, requestId });
      identities.settle('identity');
      onSaved(receipt.identity);
    } catch (error) {
      if (error instanceof ApiError && error.answered) {
        identities.settle('identity');
        if (error.failure.kind === 'http' && error.failure.code === 'creation_assets_stale') onStale?.();
      }
      setMessage(api.assetsErrorText(error));
    } finally {
      setBusy(false);
    }
  }

  return <form className={styles.form} aria-labelledby={`${uid}-title`} onSubmit={event => { event.preventDefault(); void save(); }}>
    <h3 id={`${uid}-title`}>{accountName} · 账号身份</h3>
    <p className={styles.hint}>只写角色，不写真名、孩子姓名、学校；不清楚就选“不清楚”，系统不会把它当成“没有”。</p>
    <Choice legend="和机构的关系" name={`${uid}-relation`} value={relation} disabled={busy}
      options={RELATIONS.map(value => ({ value, label: relationLabel[value] }))} onChange={value => setRelation(value as OrgRelation)} />
    {related ? <Choice legend="账号简介里写明这层关系了吗" name={`${uid}-bio`} value={bio} disabled={busy}
      options={[{ value: 'yes', label: '已写明' }, { value: 'no', label: '没写明' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setBio(value as Answer)} /> : null}
    <Choice legend="背后有没有真人" name={`${uid}-person`} value={person} disabled={busy}
      options={[{ value: 'yes', label: '有' }, { value: 'no', label: '没有' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setPerson(value as Answer)} />
    {person === 'yes' ? <label>真人的角色（如“妈妈”“老师”）
      <input value={role} maxLength={40} disabled={busy} onChange={event => setRole(event.target.value)} />
    </label> : null}
    <Choice legend="能不能每周提供经历" name={`${uid}-experience`} value={experience} disabled={busy}
      options={[{ value: 'yes', label: '能' }, { value: 'no', label: '不能' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setExperience(value as Answer)} />
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className={styles.actions}>
      <ActionButton type="submit" variant="primary" busy={busy} disabled={invalid}>{busy ? '保存中…' : '保存'}</ActionButton>
      {onCancel ? <ActionButton variant="outline" disabled={busy} onClick={onCancel}>取消</ActionButton> : null}
    </div>
  </form>;
}
