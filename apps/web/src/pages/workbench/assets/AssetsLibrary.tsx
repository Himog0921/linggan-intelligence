import { useState } from 'react';
import { FactCardLibrary } from './FactCardLibrary';
import { IdentityLibrary } from './IdentityLibrary';
import { SentenceLibrary } from './SentenceLibrary';
import { ExperienceLibrary } from './ExperienceLibrary';
import { StructuralTemplateLibrary } from './StructuralTemplateLibrary';
import { PersonaFactsLibrary } from './PersonaFactsLibrary';

const KINDS = [['identity', '账号身份'], ['persona', '人设事实'], ['factCard', '产品事实卡'], ['sentence', '衔接句'], ['experience', '经历库'], ['template', '样板库']] as const;
type Kind = typeof KINDS[number][0];

/** The asset library of the creation module: type tabs across the top, the chosen type below. */
export function AssetsLibrary({ domainRef, onPendingChange = () => {}, onDirtyChange = () => {} }: { domainRef: string; onPendingChange?: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void }) {
  const [kind, setKind] = useState<Kind>('identity');
  const [pending, setPending] = useState(false);
  const [dirty, setDirty] = useState(false);
  return <div className="gm-studio assets-page">
    <h1 className="v7-sr-only">资产库</h1>
    <header className="asset-modal-head">
      <div className="asset-modal-heading">
        <span className="asset-kicker">CREATION RESOURCES</span>
        <h2>资产库</h2>
        <p>维护账号、经历、产品知识与可复用写法；这些资产会在创作时被调用。</p>
      </div>
      <div className="asset-modal-head-actions"><span className="asset-count">{KINDS.length} 类长期资产</span></div>
    </header>
    <nav className="asset-tabs" aria-label="资产类型">{KINDS.map(([value, label]) => <button type="button" key={value}
      disabled={pending} aria-current={kind === value ? 'page' : undefined} onClick={() => { if (kind === value) return; if (dirty && !window.confirm('当前内容尚未保存，切换会丢弃本页编辑。确定切换吗？')) return; setDirty(false); onDirtyChange(false); setKind(value); }}>{label}</button>)}</nav>
    <div className="assets"><div className="as-inner">
      {kind === 'identity' ? <IdentityLibrary domainRef={domainRef} /> : kind === 'factCard' ? <FactCardLibrary /> : kind === 'sentence' ? <SentenceLibrary /> : kind === 'persona' ? <PersonaFactsLibrary domainRef={domainRef} onDirtyChange={value => { setDirty(value); onDirtyChange(value); }} onPendingChange={value => { setPending(value); onPendingChange(value); }} /> : kind === 'template' ? <StructuralTemplateLibrary domainRef={domainRef} onDirtyChange={value => { setDirty(value); onDirtyChange(value); }} onPendingChange={value => { setPending(value); onPendingChange(value); }} /> : <ExperienceLibrary domainRef={domainRef} onDirtyChange={value => { setDirty(value); onDirtyChange(value); }} onPendingChange={value => { setPending(value); onPendingChange(value); }} />}
    </div></div>
  </div>;
}
