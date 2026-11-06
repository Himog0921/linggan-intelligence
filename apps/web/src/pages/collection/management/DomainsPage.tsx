import { LoadingState } from '../../../shared/LoadingState';
import { CollectionFrame } from '../CollectionFrame';
import { DomainManagementContent } from './DomainManagementContent';
import { ManagementReturn } from './ManagementReturn';
import { loadDomains } from './managementApi';
import { useManagementRead } from './useManagementRead';

export { DomainManagementContent } from './DomainManagementContent';
export { DomainManagementDialog } from './DomainManagementDialog';

export function DomainsPage() {
  const read = useManagementRead(loadDomains);
  const phase = read.error ? 'stale_error' : read.data ? (read.refreshing ? 'refreshing' : 'ready') : read.loading ? 'initial' : 'ready';
  return <CollectionFrame section="domains">
    {read.data && <ManagementReturn returnTo={read.data.returnTo} />}
    <LoadingState
      phase={phase}
      subject="领域配置"
      error={read.error ? (read.data ? '领域配置和关联关系当前读不到；' : '领域配置和关联关系当前读不到；页面未把未知状态显示为零。') : undefined}
      retained={read.data ? `以下保留上次读取结果（${read.data.asOf ?? '时刻未知'}）。` : undefined}
      action={read.error ? <button className="c-btn-quiet" onClick={() => void read.reload()}>重新读取</button> : undefined}
    >
      {read.data ? <DomainManagementContent read={read.data} reload={() => void read.reload()} /> : undefined}
    </LoadingState>
  </CollectionFrame>;
}
