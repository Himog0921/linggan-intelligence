import { useId, useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { ApiError } from '../../../shared/api';
import { RequestIdentities } from '../../../shared/requestIdentity';
import * as api from './assetsApi';
import type { Identity, OrgRelation } from './assetsModel';
import { relationLabel, triLabel } from './assetsText';
import styles from './Assets.module.css';

type Answer = 'yes' | 'no' | 'unknown';
const toAnswer = (value: boolean | null): Answer => (value === null ? 'unknown' : value ? 'yes' : 'no');
const fromAnswer = (value: Answer): boolean | null => (value === 'unknown' ? null : value === 'yes');
const RELATIONS: readonly OrgRelation[] = ['parent', 'coach', 'none', 'unknown'];

function Radio({ name, value, label, checked, disabled, onChange }: {
  name: string; value: string; label: string; checked: boolean; disabled: boolean; onChange: (value: string) => void;
}) {
  return <label className="persona-radio"><input className="v7-sr-only" type="radio" name={name} value={value} checked={checked} disabled={disabled} onChange={() => onChange(value)} /><span>{label}</span></label>;
}

function Choice({ label, name, value, options, onChange, disabled }: {
  label: string; name: string; value: string; options: readonly { value: string; label: string }[];
  onChange: (value: string) => void; disabled: boolean;
}) {
  const id = `${name}-label`;
  return <div className="persona-field">
    <span className="persona-label" id={id}>{label}</span>
    <div className="persona-value"><div className="persona-relations" role="radiogroup" aria-labelledby={id}>
      {options.map(option => <Radio key={option.value} name={name} value={option.value} label={option.label}
        checked={value === option.value} disabled={disabled} onChange={onChange} />)}
    </div></div>
  </div>;
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
  const [personChange, setPersonChange] = useState<'' | 'correction' | 'replacement'>('');
  const [experience, setExperience] = useState<Answer>(toAnswer(identity?.canProvideExperience ?? null));
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState('');
  const replacingPerson = person === 'yes' && personChange === 'replacement';
  const related = relation === 'parent' || relation === 'coach';
  const roleText = role.trim();
  const hasKnownPerson = identity?.hasKnownPerson === true || (identity?.hasKnownPerson == null && identity?.hasRealPerson === true);
  const needsPersonChoice = Boolean(hasKnownPerson && person === 'yes' && (identity?.hasRealPerson !== true || roleText !== (identity?.backingPersonRole ?? '')));
  const invalid = (person === 'yes' && roleText.length > 40) || (needsPersonChoice && !personChange);
  const bridgeState = !replacingPerson && related && bio === 'yes' ? '衔接句可用' : '衔接句停用';

  async function save() {
    if (busy || invalid) return;
    setBusy(true);
    setMessage('');
    const body = {
      expectedRevision: identity?.revision ?? 0, orgRelation: replacingPerson ? 'unknown' as const : relation,
      bioDisclosesRelation: !replacingPerson && related ? fromAnswer(bio) : null,
      hasRealPerson: fromAnswer(person), backingPersonRole: person === 'yes' && roleText ? roleText : null,
      canProvideExperience: fromAnswer(experience),
      ...(person === 'yes' && personChange ? { personChange } : {}),
    };
    const requestId = identities.keyFor('identity', JSON.stringify(body));
    try {
      const receipt = await api.putIdentity(accountRef, { ...body, requestId });
      identities.settle('identity');
      if (typeof BroadcastChannel !== 'undefined') {
        const channel = new BroadcastChannel('linggan-creation-experience-events');
        channel.postMessage('identity-changed');
        channel.close();
      }
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

  return <form className="persona-fields" aria-labelledby={`${uid}-title`} onSubmit={event => { event.preventDefault(); void save(); }}>
    <h3 id={`${uid}-title`} className="v7-sr-only">{accountName} · 账号身份</h3>
    <Choice label="和机构的关系" name={`${uid}-relation`} value={replacingPerson ? 'unknown' : relation} disabled={busy || replacingPerson}
      options={RELATIONS.map(value => ({ value, label: relationLabel[value] }))} onChange={value => {
        if (value !== relation) setBio('unknown');
        setRelation(value as OrgRelation);
      }} />
    {!replacingPerson && related ? <Choice label="账号简介里写明这层关系了吗" name={`${uid}-bio`} value={bio} disabled={busy}
      options={[{ value: 'yes', label: '已写明' }, { value: 'no', label: '没写明' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setBio(value as Answer)} /> : null}
    <div className="persona-field"><span className="persona-label">衔接句</span><div className="persona-value"><span className="persona-state">{bridgeState}</span></div></div>
    <Choice label="背后有没有真人" name={`${uid}-person`} value={person} disabled={busy}
      options={[{ value: 'yes', label: '有' }, { value: 'no', label: '没有' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setPerson(value as Answer)} />
    {person === 'yes' ? <div className="persona-field">
      <label className="persona-label" htmlFor={`${uid}-role`}>真人的角色（如“妈妈”“老师”）</label>
      <div className="persona-value"><input id={`${uid}-role`} className="input" value={role} maxLength={40} disabled={busy} onChange={event => setRole(event.target.value)} /></div>
    </div> : null}
    {person === 'yes' && hasKnownPerson ? <>
      <Choice label="这次是更正原真人的信息，还是更换了真人？" name={`${uid}-person-change`} value={personChange} disabled={busy}
        options={[{ value: 'correction', label: '同一位真人，更正信息' }, { value: 'replacement', label: '更换了真人' }]}
        onChange={value => setPersonChange(value as 'correction' | 'replacement')} />
      {replacingPerson ? <div className="persona-field"><span className="persona-label" /><div className="persona-value"><span className="aux">保存换人后，机构关系和简介状态都将设为未确认，请根据新真人和当前简介重新核实。</span></div></div> : null}
    </> : null}
    <Choice label="能不能每周提供经历" name={`${uid}-experience`} value={experience} disabled={busy}
      options={[{ value: 'yes', label: '能' }, { value: 'no', label: '不能' }, { value: 'unknown', label: '不清楚' }]} onChange={value => setExperience(value as Answer)} />
    <p className={styles.hint}>只写角色，不写真名、孩子姓名、学校；不清楚就选“不清楚”，系统不会把它当成“没有”。{needsPersonChoice ? ' 涉及更正或换人时，请先选择上面一项。' : ''}</p>
    {message ? <p role="alert" className={styles.problem}>{message}</p> : null}
    <div className="persona-value">
      <ActionButton type="submit" variant="ink" busy={busy} disabled={invalid}>{busy ? '保存中…' : '保存'}</ActionButton>
      {onCancel ? <ActionButton variant="quiet" disabled={busy} onClick={onCancel}>取消</ActionButton> : null}
      <span className="aux">身份只用于判断能不能放衔接句；{triLabel(identity?.bioDisclosesRelation ?? null, '简介已写明', '简介没写明', '简介未核对')}。</span>
    </div>
  </form>;
}
