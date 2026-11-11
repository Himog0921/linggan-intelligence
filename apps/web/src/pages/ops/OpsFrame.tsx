import type { ReactNode } from 'react';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { Sidebar, type SidebarNavigationItem } from '../../shared/Sidebar';

const DATA_PAGES = [
  { key: 'overview', label: '概览', code: 'OV', icon: 'overview' },
  { key: 'publications', label: '发布记录', code: 'PUB', icon: 'publications' },
  { key: 'accounts', label: '账号管理', code: 'ACC', icon: 'accounts' },
  { key: 'daily', label: '每日数据', code: 'DAY', icon: 'daily' },
] as const;

/** 运营导航：数据页的三级入口进入侧栏；业务范围与当前页面仍由 Z02 面包屑表达。 */
export function OpsFrame({
  section,
  domain,
  scope,
  meta,
  foot,
  immersive,
  restricted = false,
  page,
  dataPageKey = 'overview',
  creationPageKey = 'today',
  children,
}: {
  section: 'data' | 'creation';
  domain: string;
  scope: ReactNode;
  meta?: ReactNode;
  foot?: ReactNode;
  immersive?: boolean;
  restricted?: boolean;
  page?: string;
  dataPageKey?: string;
  creationPageKey?: 'today' | 'works';
  children: ReactNode;
}) {
  const selectedDomain = domain && domain !== 'all' ? '?domain=' + encodeURIComponent(domain) : '';
  const dataPages = (restricted ? DATA_PAGES.slice(1) : DATA_PAGES).map(item => ({
    id: 'data-' + item.key,
    label: item.label,
    code: item.code,
    icon: item.icon,
    href: '/ops/' + item.key + selectedDomain,
  }));
  const items: SidebarNavigationItem[] = [
    {
      id: 'data',
      label: '数据',
      index: '01',
      icon: 'overview',
      href: '/ops/' + (restricted ? 'publications' : 'overview') + selectedDomain,
      children: dataPages,
    },
    ...(!restricted ? [{
      id: 'creation',
      label: '创作',
      index: '02',
      icon: 'creation',
      href: '/ops/creation' + selectedDomain,
      children: [
        {
          id: 'creation-today',
          label: '今日选题',
          code: 'TODAY',
          href: '/ops/creation' + selectedDomain + (selectedDomain ? '&' : '?') + 'section=today',
        },
        {
          id: 'creation-works',
          label: '我的创作',
          code: 'WORKS',
          href: '/ops/creation' + selectedDomain + (selectedDomain ? '&' : '?') + 'section=works',
        },
      ],
    }] : []),
  ];
  const activeId = section === 'data' ? 'data-' + dataPageKey : 'creation-' + creationPageKey;
  const activeParentId = section === 'data' ? 'data' : 'creation';
  const creationPageLabel = creationPageKey === 'works' ? '我的创作' : '今日选题';
  const creationTrail = page
    ? ['运营', scope, '创作', creationPageLabel, page]
    : ['运营', scope, '创作', creationPageLabel];
  const crumb = <ContextBreadcrumb items={section === 'creation' ? creationTrail : ['运营', scope, '数据', page ?? '概览']} />;

  return (
    <AppShell
      active="ops"
      boundaryLabel="本机服务状态"
      className={immersive ? 'ops-immersive' : undefined}
      restrictedOps={restricted}
      contextBar={{ breadcrumb: crumb, meta }}
    >
      <div className="v7-shell" data-ops-section={section}>
        <Sidebar
          aria-label="运营导航"
          eyebrow="OPERATIONS / NAV"
          title="运营工作区"
          items={items}
          activeId={activeId}
          activeParentId={activeParentId}
          foot={foot}
        />
        {children}
      </div>
    </AppShell>
  );
}
