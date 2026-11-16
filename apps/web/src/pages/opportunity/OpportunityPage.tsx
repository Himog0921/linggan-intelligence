import { Link, useSearchParams } from 'react-router';
import { AppShell } from '../../app/AppShell';
import { useOpportunityStatus } from '../../app/opportunityStatusContext';
import { NotFoundPage } from '../../app/NotFoundPage';
import { useCorpusShell } from '../../corpus/useCorpusShell';
import { DomainPicker } from '../../corpus/DomainPicker';
import { ContextBreadcrumb } from '../../shared/ContextBar';
import { Sidebar } from '../../shared/Sidebar';
import { OpportunityHome } from './OpportunityHome';
import { OpportunityDetail } from './OpportunityDetail';
import styles from './Opportunity.module.css';

export function OpportunityPage() {
  const status = useOpportunityStatus();
  if (status.state === 'disabled') return <NotFoundPage />;
  if (status.state === 'loading') return <p role="status">正在确认机会卡状态…</p>;
  if (status.state === 'unavailable') return <main className={styles.standalone}><p role="alert">无法确认机会卡功能状态。</p><button type="button" onClick={status.retry}>重试</button></main>;
  return <Workspace />;
}

function Workspace() {
  const [params] = useSearchParams();
  const requestedDomain = params.get('domain');
  const problem = params.get('problem');
  const shell = useCorpusShell(requestedDomain);
  const domain = shell.data?.selectedDomain ?? null;
  const selected = shell.data?.domains.find(item => item.domainRef === domain);
  const picker = shell.data ? <DomainPicker domains={shell.data.domains} selected={domain} action="/opportunity" /> : null;
  const home = domain ? `/opportunity?${new URLSearchParams({ domain })}` : '/opportunity';
  const crumb = <ContextBreadcrumb items={[<Link key="home" to={home}>洞察</Link>, ...(picker ? [picker] : []), problem ? '机会卡' : '机会首页']} />;
  return <AppShell active="opportunity" boundaryLabel="本机服务状态" contextBar={{ breadcrumb: crumb, meta: selected?.name ?? '请选择领域' }}>
    <title>{problem ? '机会卡' : '机会首页'} · Linggan Intelligence</title>
    <div className="v7-shell">
      <Sidebar aria-label="洞察导航" title="洞察工作区" items={[{ id: 'home', label: '机会首页', index: '01', icon: 'overview', href: home }]} activeId="home" />
      <main className={styles.main}>
        {shell.isPending ? <p role="status">正在读取领域…</p> : shell.isError ? <div role="alert"><p>领域信息读取失败。</p><button type="button" onClick={() => void shell.refetch()}>重试</button></div> : !domain ? <><h1>机会首页</h1><p>请先在页头选择观察领域。</p></> : problem ? <OpportunityDetail key={`${domain}:${problem}`} domain={domain} problem={problem} home={home} /> : <OpportunityHome key={domain} domain={domain} />}
      </main>
    </div>
  </AppShell>;
}
