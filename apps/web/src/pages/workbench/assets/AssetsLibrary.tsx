import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { FactCardLibrary } from './FactCardLibrary';
import { SentenceLibrary } from './SentenceLibrary';
import { ExperienceLibrary } from './ExperienceLibrary';
import { StructuralTemplateLibrary } from './StructuralTemplateLibrary';
import { StyleLibrary } from './StyleLibrary';
import { PersonaFactsLibrary } from './PersonaFactsLibrary';
import * as api from './assetsApi';
import * as stylesApi from './stylesApi';
import * as templatesApi from './structuralTemplatesApi';

const KINDS = [['persona', '人设事实'], ['experience', '经历'], ['factCard', '产品事实卡'], ['sentence', '衔接句'], ['template', '样板库'], ['style', '风格']] as const;
type Kind = typeof KINDS[number][0];
const SCOPES: Partial<Record<Kind, string>> = { persona: 'persona', experience: 'experience', factCard: 'product' };

/** The asset library of the creation module: type tabs across the top, the chosen type below. */
export function AssetsLibrary({ domainRef, onPendingChange = () => {}, onDirtyChange = () => {}, onClose }: { domainRef: string; onPendingChange?: (pending: boolean) => void; onDirtyChange?: (dirty: boolean) => void; onClose?: () => void }) {
  const [kind, setKind] = useState<Kind>('persona');
  const [pending, setPending] = useState(false);
  const [dirty, setDirty] = useState(false);
  const accounts = useQuery({ queryKey: ['creation-assets', 'accounts', domainRef], queryFn: ({ signal }) => api.listAccounts(domainRef, signal), retry: false });
  const sentences = useQuery({ queryKey: ['creation-assets', 'sentences', '', ''], queryFn: ({ signal }) => api.listSentences({}, signal), retry: false });
  const templates = useQuery({ queryKey: ['creation-assets', 'templates', domainRef], queryFn: ({ signal }) => templatesApi.listTemplates(domainRef, signal), retry: false });
  const styleItems = useQuery({ queryKey: ['creation-assets', 'styles', domainRef], queryFn: ({ signal }) => stylesApi.listStyles(domainRef, signal), retry: false });
  const counts: Partial<Record<Kind, number | undefined>> = {
    persona: accounts.data?.length, sentence: sentences.data?.length,
    template: templates.data?.length, style: styleItems.data?.length,
  };
  const pass = { onDirtyChange: (value: boolean) => { setDirty(value); onDirtyChange(value); }, onPendingChange: (value: boolean) => { setPending(value); onPendingChange(value); } };
  return <div className="gm-studio assets-page">
    <h1 className="v7-sr-only">资产库</h1>
    <header className="asset-modal-head">
      <div className="asset-modal-heading">
        <span className="asset-kicker">CREATION RESOURCES</span>
        <h2 id="asset-modal-title">资产库</h2>
        <p>维护账号、经历、产品知识与可复用写法；这些资产会在创作时被调用。</p>
      </div>
      <div className="asset-modal-head-actions"><span className="asset-count">{KINDS.length} 类长期资产</span>{onClose ? <button type="button" className="modal-x" aria-label="关闭资产库" onClick={onClose}>×</button> : null}</div>
    </header>
    <nav className="asset-tabs" aria-label="资产类型">{KINDS.map(([value, label]) => <button type="button" key={value}
      disabled={pending} aria-current={kind === value ? 'page' : undefined} onClick={() => { if (kind === value) return; if (dirty && !window.confirm('当前内容尚未保存，切换会丢弃本页编辑。确定切换吗？')) return; setDirty(false); onDirtyChange(false); setKind(value); }}>{label}{counts[value] === undefined ? null : <i>{counts[value]}</i>}</button>)}</nav>
    <div className={SCOPES[kind] ? `assets ${SCOPES[kind]}` : 'assets'}><div className="as-inner">
      {kind === 'persona' ? <PersonaFactsLibrary domainRef={domainRef} {...pass} />
        : kind === 'experience' ? <ExperienceLibrary domainRef={domainRef} {...pass} />
          : kind === 'factCard' ? <FactCardLibrary />
            : kind === 'sentence' ? <SentenceLibrary />
              : kind === 'template' ? <StructuralTemplateLibrary domainRef={domainRef} {...pass} />
                : <StyleLibrary domainRef={domainRef} {...pass} />}
    </div></div>
  </div>;
}
