import { useQuery } from '@tanstack/react-query';
import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from 'react';
import { useLocation, useNavigate, useSearchParams } from 'react-router';
import { ActionButton } from '../../shared/ActionButton';
import { IntelligenceIcon } from '../../shared/IntelligenceIcon';
import { Sidebar, type SidebarNavigationItem } from '../../shared/Sidebar';
import { AppShell } from '../../app/AppShell';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { DomainScopePicker } from '../../shared/DomainScopePicker';
import { requestJson } from '../../shared/api';
import collectionSurface from './CollectionSurface.module.css';
import styles from './CollectionFrame.module.css';
import { parseCollectionShell } from './collectionShell';

const RuntimeManagementDialog = lazy(() => import('./management/RuntimeManagementDialog').then(m => ({ default: m.RuntimeManagementDialog })));
const DomainManagementDialog = lazy(() => import('./management/DomainManagementDialog').then(m => ({ default: m.DomainManagementDialog })));

function domainSummary(d: {status:string; targetCount:number; sampleCount:number|null}) { return d.status === 'paused' ? '已暂停' : `${d.targetCount} 个目标 · ${d.sampleCount ?? '未知'} 篇作品`; }

type Section = 'targets' | 'tasks' | 'domains' | 'runtime';
const titles: Record<Section, string> = { targets: '观察目标', tasks: '采集任务', domains: '领域管理', runtime: '工位管理' };

export function CollectionReadout({ value, label, scope, valueFirst = false }: { value: string | number | null; label: string; scope: string; valueFirst?: boolean }) {
  return <span className="v7-kpi" title={scope} aria-label={`${label} ${value ?? '未知'}；${scope}`}><em>{valueFirst ? value ?? '未知' : label}</em><b aria-hidden="true">{valueFirst ? label : value ?? '未知'}</b><small>{scope}</small></span>;
}

export function CollectionFrame({ section, toolbar, meta, children }: { section: Section; toolbar?: ReactNode; meta?: ReactNode; children: ReactNode }) {
  const [domainManager, setDomainManager] = useState(false);
  const [params, setParams] = useSearchParams();
  const routerLocation = useLocation();
  const navigate = useNavigate();
  const openedFrom = useRef<string | null>(null);
  const returnPath = params.get('return_to');
  const returnDomain = returnPath && /^\/collection\/(targets|tasks)(?:[?#]|$)/.test(returnPath) && !/[\\\r\n]/.test(returnPath) ? new URL(returnPath, location.origin).searchParams.get('domain') : null;
  const domain = params.get('domain') ?? (section === 'domains' || section === 'runtime' ? returnDomain : null);
  const query = useQuery({ queryKey: ['collection-shell', domain], queryFn: ({ signal }) => requestJson(`/api/collection/workspace/shell${domain ? `?domain=${encodeURIComponent(domain)}` : ''}`, { signal }).then(parseCollectionShell), gcTime: 0 });
  const refreshShell = query.refetch;
  useEffect(() => { const refresh = () => { void refreshShell(); }; window.addEventListener('collection:changed', refresh); return () => window.removeEventListener('collection:changed', refresh); }, [refreshShell]);
  const shell = query.data;
  const selected = shell?.domains?.find(d => d.domainRef === shell.selectedDomain);
  const navDomain = section === 'targets' ? shell ? selected?.domainRef ?? 'all' : domain : domain;
  const suffix = navDomain ? `?domain=${encodeURIComponent(navDomain)}` : '';
  const source = `/collection/${section === 'tasks' ? 'tasks' : 'targets'}${suffix}`;
  const runtimeOpen = params.get('runtime') === '1';
  function toggleRuntime(open: boolean) {
    const next = new URLSearchParams(params);
    if (open) {
      openedFrom.current = routerLocation.key;
      next.set('runtime','1');
      setParams(next, { state: { ...routerLocation.state, collectionRuntimeOrigin: routerLocation.key } });
    } else if (openedFrom.current !== null && routerLocation.state?.collectionRuntimeOrigin === openedFrom.current) navigate(-1);
    else { next.delete('runtime'); next.delete('station'); setParams(next, { replace: true, state: routerLocation.state }); }
  }
  const scheduler = shell?.scheduler === 'running' ? '调度心跳正常' : shell?.scheduler === 'stale' ? '调度心跳已过期' : '调度状态读不到';
  useEffect(() => { document.title = `${titles[section]} · 采集 · Linggan Intelligence`; }, [section]);
  const pickerSelected = section === 'targets'
    ? selected
    : domain && domain !== 'all'
      ? shell?.domains?.find(d => d.domainRef === domain)
      : undefined;
  const pickerAction = section === 'tasks' ? '/collection/tasks' : '/collection/targets';
  const picker = section === 'targets' || section === 'tasks' ? (
    <DomainScopePicker
      label="切换当前观察领域"
      summary={
        <>
          <span>{shell ? pickerSelected?.name ?? '全部领域' : '读取领域…'}</span>
          <small>{pickerSelected ? domainSummary(pickerSelected) : '全部领域'}</small>
          <i aria-hidden="true" />
        </>
      }
    >
      <nav aria-label="可选观察领域">
        <a href={pickerAction + '?domain=all'} aria-current={shell && !pickerSelected ? 'page' : undefined} aria-label="全部领域，全部领域"><span>全部领域</span><small>全部领域</small></a>
        {shell?.domains?.map(d => <a key={d.domainRef} href={pickerAction + '?domain=' + encodeURIComponent(d.domainRef)} aria-current={d.domainRef === pickerSelected?.domainRef ? 'page' : undefined} aria-label={d.name + '，' + domainSummary(d)}><span>{d.name}</span><small>{domainSummary(d)}</small></a>)}
        <a className="c-domain-management-link" href={'/collection/domains?return_to=' + encodeURIComponent(source)} data-collection-management onClick={e => { e.preventDefault(); e.currentTarget.closest('details')?.removeAttribute('open'); setDomainManager(true); }}><span>管理领域</span><small>配置与目标用途</small></a>
      </nav>
    </DomainScopePicker>
  ) : null;
  const crumb = <ContextBreadcrumb items={section === 'domains' ? ['采集', '观察目标', titles[section]] : section === 'targets' || section === 'tasks' ? ['采集', picker, titles[section]] : ['采集', titles[section]]} />;
  const sidebarItems: SidebarNavigationItem[] = [
    {
      id: 'targets',
      label: '观察目标',
      index: '01',
      icon: 'targets',
      href: '/collection/targets' + suffix,
      children: [{
        id: 'domains',
        label: '领域管理',
        code: 'DOM',
        href: '/collection/domains?return_to=' + encodeURIComponent(source),
      }],
    },
    {
      id: 'tasks',
      label: '采集任务',
      index: '02',
      icon: 'tasks',
      href: '/collection/tasks' + suffix,
    },
  ];
  const activeId = section === 'domains' ? 'domains' : section === 'tasks' ? 'tasks' : 'targets';
  const activeParentId = section === 'targets' || section === 'domains' ? 'targets' : undefined;
  const sidebarFoot = !shell || shell.totalTargets === null
    ? <>采集状态未知<br /><span className="v7-mono">目标与调度状态当前读不到</span></>
    : shell.totalTargets === 0
      ? <>暂无观察目标<br /><span className="v7-mono">没有可巡检的对象 · {scheduler}</span></>
      : <>观察目标 {shell.totalTargets}<br /><span className="v7-mono">巡检开着 {shell.monitoringTargets ?? '未知'} · {scheduler}</span></>;
  return <AppShell active="collection" boundaryLabel={shell ? '本机不访问平台' : '采集状态未知'} contextBar={{ breadcrumb: crumb, meta: <>{meta}{section !== 'runtime' ? <ActionButton variant="quiet" className="c-management-entry" data-collection-management aria-label="工位管理，全局执行资源" aria-haspopup="dialog" aria-expanded={runtimeOpen} onClick={()=>toggleRuntime(true)}><IntelligenceIcon name="diagnostics" size={16}/>工位管理</ActionButton> : null}</> }}>
    <div className="v7-shell"><Sidebar aria-label="采集导航" eyebrow="COLLECTION / NAV" title="采集工作区" items={sidebarItems} activeId={activeId} activeParentId={activeParentId} foot={sidebarFoot}/><main className={`${collectionSurface['c-page'] ?? 'c-page'} ${section === 'domains' ? '' : styles.frame}`} data-collection-page={section} aria-labelledby="page-title"><h1 className="v7-sr-only" id="page-title">{titles[section]}</h1>{toolbar}<div className={`c-body ${section === 'domains' ? '' : styles.body}`}>{children}</div>{domainManager && <Suspense fallback={<p role="status">正在读取领域管理…</p>}><DomainManagementDialog onClose={() => setDomainManager(false)} /></Suspense>}{runtimeOpen && <Suspense fallback={<p role="status">正在打开工位管理…</p>}><RuntimeManagementDialog onClose={()=>toggleRuntime(false)}/></Suspense>}</main></div>
  </AppShell>;
}
