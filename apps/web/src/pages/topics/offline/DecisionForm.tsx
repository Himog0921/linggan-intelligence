import { useState } from 'react';
import { ActionButton } from '../../../shared/ActionButton';
import { relevanceLabels, type Judgment, type Note, type Preview, type Relevance } from './api';
import styles from './OfflineReview.module.css';

export function DecisionForm({ note, catalogue, busy, apply, reset }: { note: Note; catalogue: Preview['catalogue']; busy: boolean; apply: (judgment: Judgment) => void; reset: () => void }) {
  const source = note.result.effective;
  const [relevance, setRelevance] = useState<Relevance>(source.relevance);
  const [primary, setPrimary] = useState(source.primaryDirection ?? '');
  const [secondary, setSecondary] = useState(source.alsoRelated ?? '');
  const [reason, setReason] = useState(source.reason ?? '');
  const length = Array.from(reason).length;
  return <form className={styles.form} onSubmit={event => {
    event.preventDefault();
    apply({ origin: 'offline_mock', identity: note.input.identity, relevance, primaryDirection: relevance === 'related' ? primary || null : null, alsoRelated: relevance === 'related' ? secondary || null : null, reason: reason === '' ? null : reason });
  }}>
    <h3>本次会话裁定</h3><p>仅用于预演，刷新页面即丢失，不写正式业务记录。</p>
    <fieldset disabled={busy}>
      <label htmlFor="offline-relevance">相关性</label><select id="offline-relevance" value={relevance} onChange={event => {
        const value = event.target.value;
        if (value === 'related' || value === 'not_related' || value === 'unknown') setRelevance(value);
      }}>{Object.entries(relevanceLabels).map(([key, label]) => <option value={key} key={key}>{label}</option>)}</select>
      <label htmlFor="offline-primary">主方向</label><select id="offline-primary" value={primary} disabled={relevance !== 'related'} onChange={event => setPrimary(event.target.value)}><option value="">暂无合适方向</option>{catalogue.directions.map(d => <option key={d.directionRef} value={d.directionRef}>{d.name}</option>)}</select>
      <label htmlFor="offline-secondary">也相关</label><select id="offline-secondary" value={secondary} disabled={relevance !== 'related'} onChange={event => setSecondary(event.target.value)}><option value="">不设置</option>{catalogue.directions.map(d => <option key={d.directionRef} value={d.directionRef}>{d.name}</option>)}</select>
      <label htmlFor="offline-reason">裁定理由</label><textarea id="offline-reason" value={reason} onChange={event => setReason(event.target.value)} aria-describedby="offline-reason-length" />
      <p id="offline-reason-length">{length} / 200 字，可清空。{length > 200 ? '请缩短理由后再应用。' : ''}</p>
      {primary !== '' && primary === secondary && relevance === 'related' ? <p role="alert">主方向与也相关不能相同。</p> : null}
      <ActionButton type="submit" variant="primary" disabled={length > 200 || (primary !== '' && primary === secondary && relevance === 'related')}>应用到本次预演</ActionButton>
      <ActionButton onClick={reset}>恢复此篇示例</ActionButton>
    </fieldset>
  </form>;
}
