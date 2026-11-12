import type { ReactNode } from 'react';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { ContextTabs } from '../../shared/ContextTabs';
import { IntelligenceIcon } from '../../shared/IntelligenceIcon';
import { NavIcon, Sidebar } from '../../shared/Sidebar';

/** Ops keeps its secondary destinations in the sidebar and page tabs in the context bar. */
export function OpsFrame({ section, domain, scope, tabs, meta, foot, immersive, restricted = false, manageTeam = false, page, children }: {
  section: 'data' | 'creation'; domain: string; scope: ReactNode; tabs?: ReactNode;
  meta?: ReactNode; foot?: ReactNode; immersive?: boolean; restricted?: boolean; manageTeam?: boolean; page?: string; children: ReactNode;
}) {
  const selectedDomain = domain && domain !== 'all' ? `?domain=${encodeURIComponent(domain)}` : '';
  const creationTrail = page ? [scope, '运营', '创作', page] : [scope, '运营', '创作'];
  return <AppShell active="ops" restrictedOps={restricted}
    boundaryLabel="本机服务状态" className={immersive ? 'ops-immersive' : undefined}
    contextBar={{
      breadcrumb: <ContextBreadcrumb items={section === 'creation' ? creationTrail : ['运营', scope, '数据']} />,
      navigation: tabs ? <ContextTabs label={section === 'data' ? '数据页面' : '创作页面'}>{tabs}</ContextTabs> : undefined,
      meta,
    }}>
    <div className="v7-shell" data-ops-section={section}>
      <Sidebar aria-label="运营导航">
        {/* Full navigation preserves the editor's existing beforeunload protection. */}
        <a className="v7-side-nav" aria-label="数据" title="数据" href={`/ops/${restricted ? 'publications' : 'overview'}${selectedDomain}`} aria-current={section === 'data' ? 'page' : undefined}>
          <NavIcon name="overview"/><span>数据</span>
        </a>
        {!restricted ? <a className="v7-side-nav" aria-label="创作" title="创作" href={`/ops/creation${selectedDomain}`} aria-current={section === 'creation' ? 'page' : undefined}>
          <NavIcon name="creation"/><span>创作</span>
        </a> : null}
        {manageTeam ? <a className="v7-side-nav" aria-label="团队与权限" title="团队与权限" href="/settings/team">
          <IntelligenceIcon name="security" className="v7-side-icon"/><span>团队与权限</span>
        </a> : null}
        {foot ? <div className="v7-side-foot">{foot}</div> : null}
      </Sidebar>
      {children}
    </div>
  </AppShell>;
}
