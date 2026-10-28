import { useState } from 'react';
import { FactCardLibrary } from './FactCardLibrary';
import { IdentityLibrary } from './IdentityLibrary';
import { SentenceLibrary } from './SentenceLibrary';
import styles from './Assets.module.css';

const KINDS = [['identity', '账号身份'], ['factCard', '产品事实卡'], ['sentence', '衔接句']] as const;
type Kind = typeof KINDS[number][0];

/** The asset library of the creation module: a type index on the left, the chosen type on the right. */
export function AssetsLibrary({ domainRef }: { domainRef: string }) {
  const [kind, setKind] = useState<Kind>('identity');
  return <div className={styles.library}>
    <h1 className="v7-sr-only">资产库</h1>
    <nav className={styles.index} aria-label="资产类型">{KINDS.map(([value, label]) => <button type="button" key={value}
      aria-current={kind === value ? 'page' : undefined} onClick={() => setKind(value)}>{label}</button>)}</nav>
    {kind === 'identity' ? <IdentityLibrary domainRef={domainRef} /> : kind === 'factCard' ? <FactCardLibrary /> : <SentenceLibrary />}
  </div>;
}
