import type { ReactNode } from 'react';
import { AppShell } from '../../app/AppShell';
import { NavIcon, Sidebar } from '../../shared/Sidebar';

/** Ops owns two secondary destinations; page sections and scope belong to Z02. */
export function OpsFrame({ section, domain, scope, tabs, meta, foot, immersive, restricted = false, children }: {
  section: 'data' | 'creation'; domain: string; scope: ReactNode; tabs: ReactNode;
  meta?: ReactNode; foot?: ReactNode; immersive?: boolean; restricted?: boolean; children: ReactNode;
}) {
  const selectedDomain = domain && domain !== 'all' ? `?domain=${encodeURIComponent(domain)}` : '';
  return <AppShell active="ops" activeState={section === 'creation' ? '创作' : '数据'} restrictedOps={restricted}
    boundaryLabel="本机服务状态" className={immersive ? 'ops-immersive' : undefined}
    crumb={<>运营 <span aria-hidden="true">/</span> {scope} <span aria-hidden="true">/</span> <b>{section === 'data' ? '数据' : '创作'}</b></>}
    contextNav={<nav className="v7-tertiary-nav" aria-label={section === 'data' ? '数据页面' : '创作页面'}>{tabs}</nav>}
    meta={meta}>
    <div className="v7-shell" data-ops-section={section}>
      <Sidebar aria-label="运营导航">
        {/* Full navigation preserves the editor's existing beforeunload protection. */}
        <a className="v7-side-nav" aria-label="数据" title="数据" href={`/ops/${restricted ? 'publications' : 'overview'}${selectedDomain}`} aria-current={section === 'data' ? 'page' : undefined}>
          <NavIcon name="overview"/><span>数据</span>
        </a>
        {!restricted ? <a className="v7-side-nav" aria-label="创作" title="创作" href={`/ops/creation${selectedDomain}`} aria-current={section === 'creation' ? 'page' : undefined}>
          <NavIcon name="publications"/><span>创作</span>
        </a> : null}
        {foot ? <div className="v7-side-foot">{foot}</div> : null}
      </Sidebar>
      {children}
    </div>
  </AppShell>;
}
