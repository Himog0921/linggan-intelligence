import type { ReactNode } from 'react';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { ContextTabs } from '../../shared/ContextTabs';
import { NavIcon, Sidebar } from '../../shared/Sidebar';

/** Ops owns two secondary destinations; page sections and scope belong to Z02. */
export function OpsFrame({ section, domain, scope, tabs, meta, foot, immersive, preview = false, previewSidebar, restricted = false, children }: {
  section: 'data' | 'creation'; domain: string; scope: ReactNode; tabs: ReactNode;
  meta?: ReactNode; foot?: ReactNode; immersive?: boolean; preview?: boolean; previewSidebar?: ReactNode; restricted?: boolean; children: ReactNode;
}) {
  const selectedDomain = domain && domain !== 'all' ? `?domain=${encodeURIComponent(domain)}` : '';
  return <AppShell active="ops" restrictedOps={restricted} previewNavigation={preview}
    boundaryLabel={preview ? "Demo · 示例数据" : "本机服务状态"} className={[immersive ? 'ops-immersive' : '', preview ? 'creation-preview' : ''].filter(Boolean).join(' ') || undefined}
    contextBar={{
      tone: preview ? 'light' : undefined,
      breadcrumb: <ContextBreadcrumb items={preview
        ? [<span key="creator-preview" className="creation-preview-crumb-brand">A娃家长矩阵</span>, '运营', section === 'creation' ? '创作' : '数据']
        : ['运营', scope, section === 'creation' ? '创作' : '数据']} />,
      navigation: preview ? undefined : <ContextTabs label={section === 'data' ? '数据页面' : '创作页面'}>{tabs}</ContextTabs>,
      meta,
    }}>
    <div className="v7-shell" data-ops-section={section}>
      <Sidebar aria-label="运营导航" toggleLabels={preview ? { expanded: "收起导航", collapsed: "展开导航" } : undefined}>
        {previewSidebar ?? <>
        {/* Full navigation preserves the editor's existing beforeunload protection. */}
        <a className="v7-side-nav" aria-label="数据" title="数据" href={`/ops/${restricted ? 'publications' : 'overview'}${selectedDomain}`} aria-current={section === 'data' ? 'page' : undefined}>
          <NavIcon name="overview"/><span>数据</span>
        </a>
        {!restricted ? <a className="v7-side-nav" aria-label="创作" title="创作" href={`/ops/creation${selectedDomain}`} aria-current={section === 'creation' ? 'page' : undefined}>
          <NavIcon name="publications"/><span>创作</span>
        </a> : null}
        {foot ? <div className="v7-side-foot">{foot}</div> : null}
        </>}
      </Sidebar>
      {children}
    </div>
  </AppShell>;
}
