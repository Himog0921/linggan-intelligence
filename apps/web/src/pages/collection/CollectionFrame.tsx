import { useQuery } from '@tanstack/react-query';
import { lazy, Suspense, useEffect, useState, type ReactNode } from 'react';
import { useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { requestJson } from '../../shared/api';
import collectionSurface from './CollectionSurface.module.css';
import { parseCollectionShell } from './collectionShell';

const DomainManagementDialog = lazy(() => import('./management/DomainManagementDialog').then(m => ({ default: m.DomainManagementDialog })));

function domainSummary(d: {status:string; targetCount:number; sampleCount:number|null}) { return d.status === 'paused' ? '已暂停' : `${d.targetCount} 个目标 · ${d.sampleCount ?? '未知'} 篇作品`; }

type Section = 'targets' | 'tasks' | 'domains' | 'runtime';
const titles: Record<Section, string> = { targets: '观察目标', tasks: '采集任务', domains: '领域管理', runtime: '工位管理' };

export function CollectionReadout({ value, label, scope, valueFirst = false }: { value: string | number | null; label: string; scope: string; valueFirst?: boolean }) {
  return <span className="v7-kpi" title={scope} aria-label={`${label} ${value ?? '未知'}；${scope}`}><em>{valueFirst ? value ?? '未知' : label}</em><b aria-hidden="true">{valueFirst ? label : value ?? '未知'}</b><small>{scope}</small></span>;
}

export function CollectionFrame({ section, toolbar, meta, children }: { section: Section; toolbar?: ReactNode; meta?: ReactNode; children: ReactNode }) {
  const [domainManager, setDomainManager] = useState(false);
  const [params] = useSearchParams();
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
  const management = `/collection/runtime?return_to=${encodeURIComponent(source)}`;
  const scheduler = shell?.scheduler === 'running' ? '调度心跳正常' : shell?.scheduler === 'stale' ? '调度心跳已过期' : '调度状态读不到';
  const patrol = shell?.monitoringTargets === null || !shell ? '巡检状态读不到' : shell.monitoringTargets > 0 ? '自动巡检已开启' : shell.totalTargets === 0 ? '暂无观察目标' : '有观察目标但巡检未开';
  const commonMeta = <><i className="v7-vr" aria-hidden="true" /><span className="v7-query-meta"><span className="v7-status-main">{scheduler}</span><small className="v7-tech-key">SCHEDULER {shell?.scheduler?.toUpperCase() ?? 'UNKNOWN'}</small></span><span><span className="v7-status-main">{patrol}</span><small className="v7-tech-key">PATROL {shell?.monitoringTargets == null ? 'UNKNOWN' : shell.monitoringTargets > 0 ? 'ON' : 'OFF'}</small></span><span><span className="v7-status-main">中国标准时间</span><small className="v7-tech-key">UTC+08</small></span></>;
  useEffect(() => { document.title = `${titles[section]} · 采集 · Linggan Intelligence`; }, [section]);
  const picker = <details className="v7-domain-picker"><summary aria-label="切换当前观察领域"><span>{shell ? selected?.name ?? '全部领域' : '读取领域…'}</span><small>{selected ? domainSummary(selected) : '全部领域'}</small><i aria-hidden="true" /></summary><nav aria-label="可选观察领域"><a href="/collection/targets?domain=all" aria-current={shell && !selected ? 'page' : undefined} aria-label="全部领域，全部领域"><span>全部领域</span><small>全部领域</small></a>{shell?.domains?.map(d => <a key={d.domainRef} href={`/collection/targets?domain=${encodeURIComponent(d.domainRef)}`} aria-current={d.domainRef === selected?.domainRef ? 'page' : undefined} aria-label={`${d.name}，${domainSummary(d)}`}><span>{d.name}</span><small>{domainSummary(d)}</small></a>)}<a className="c-domain-management-link" href={`/collection/domains?return_to=${encodeURIComponent(source)}`} data-collection-management onClick={e => { e.preventDefault(); e.currentTarget.closest('details')?.removeAttribute('open'); setDomainManager(true); }}><span>管理领域</span><small>配置与目标用途</small></a></nav></details>;
  const crumb = <>采集 <span className="v7-slash">/</span> {section === 'targets' ? <>{picker} <span className="v7-slash">/</span> </> : null}<b>{titles[section]}</b></>;
  const collectionState = !shell || shell.totalTargets === null ? '状态未知' : shell.totalTargets > 0 ? '观察中' : '无观察目标';
  return <AppShell active="collection" collectionState={collectionState} boundaryLabel={shell ? '本机不访问平台' : '采集状态未知'} crumb={crumb} meta={<>{meta}{section === 'tasks' || section === 'runtime' ? commonMeta : null}{section !== 'runtime' ? <a className="c-btn-quiet c-management-entry" href={management} data-collection-management aria-label="工位管理，全局执行资源">工位管理</a> : null}</>}>
    <div className="v7-shell"><aside className="v7-side" aria-label="采集导航"><div className="v7-nav-label" data-readout="COLLECTION">采集与观察</div>{(['targets', 'tasks'] as const).map((entry, i) => <a key={entry} className="v7-side-nav" href={`/collection/${entry}${suffix}`} aria-current={section === entry ? 'page' : undefined}><i>{String(i + 1).padStart(2, '0')}</i><span>{titles[entry]}</span></a>)}<div className="v7-side-foot"><span className="v7-side-dot" />{!shell || shell.totalTargets === null ? <>采集状态未知<br /><span className="v7-mono">目标与调度状态当前读不到</span></> : shell.totalTargets === 0 ? <>暂无观察目标<br /><span className="v7-mono">没有可巡检的对象 · {scheduler}</span></> : <>观察目标 {shell.totalTargets}<br /><span className="v7-mono">巡检开着 {shell.monitoringTargets ?? '未知'} · {scheduler}</span></>}</div></aside><main className={collectionSurface['c-page'] ?? 'c-page'} aria-labelledby="page-title"><h1 className="v7-sr-only" id="page-title">{titles[section]}</h1>{toolbar}<div className="c-body">{children}</div>{domainManager && <Suspense fallback={<p role="status">正在读取领域管理…</p>}><DomainManagementDialog onClose={() => setDomainManager(false)} /></Suspense>}</main></div>
  </AppShell>;
}
