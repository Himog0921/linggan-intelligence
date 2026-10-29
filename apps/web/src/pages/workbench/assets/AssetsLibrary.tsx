import { useState } from 'react';
import { FactCardLibrary } from './FactCardLibrary';
import { IdentityLibrary } from './IdentityLibrary';
import { SentenceLibrary } from './SentenceLibrary';
import { ExperienceLibrary } from './ExperienceLibrary';
import styles from './Assets.module.css';

const KINDS = [['identity', '账号身份'], ['factCard', '产品事实卡'], ['sentence', '衔接句'], ['experience', '经历库']] as const;
type Kind = typeof KINDS[number][0];

/** The asset library of the creation module: a type index on the left, the chosen type on the right. */
export function AssetsLibrary({ domainRef, onPendingChange = () => {}, onDirtyChange = () => {} }: { domainRef: string; onPendingChange?: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const [kind, setKind] = useState<Kind>('identity');
  const [pending, setPending] = useState(false);
  const [dirty, setDirty] = useState(false);
  return <div className={styles.library}>
    <h1 className="v7-sr-only">资产库</h1>
    <nav className={styles.index} aria-label="资产类型">{KINDS.map(([value, label]) => <button type="button" key={value}
      disabled={pending} aria-current={kind === value ? 'page' : undefined} onClick={() => { if (kind === value) return; if (dirty && !window.confirm('这段经历有未保存内容，切换会丢弃本页编辑。确定切换吗？')) return; setDirty(false); onDirtyChange(false); setKind(value); }}>{label}</button>)}</nav>
    {kind === 'identity' ? <IdentityLibrary domainRef={domainRef} /> : kind === 'factCard' ? <FactCardLibrary /> : kind === 'sentence' ? <SentenceLibrary /> : <ExperienceLibrary domainRef={domainRef} onDirtyChange={value => { setDirty(value); onDirtyChange(value); }} onPendingChange={value => { setPending(value); onPendingChange(value); }} />}
  </div>;
}
