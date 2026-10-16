import { CollectionFrame } from '../CollectionFrame';
import { DomainManagementContent } from './DomainManagementContent';
import { ManagementReturn } from './ManagementReturn';
import { loadDomains } from './managementApi';
import { useManagementRead } from './useManagementRead';

export { DomainManagementContent } from './DomainManagementContent';
export { DomainManagementDialog } from './DomainManagementDialog';

export function DomainsPage() {
  const read = useManagementRead(loadDomains);
  return <CollectionFrame section="domains">
    {read.data && <ManagementReturn returnTo={read.data.returnTo} />}
    {read.error && <p role="alert">领域配置和关联关系当前读不到；{read.data ? `以下保留上次读取结果（${read.data.asOf ?? "时刻未知"}）。` : "页面未把未知状态显示为零。"}<button className="c-btn-quiet" onClick={() => void read.reload()}>重新读取</button></p>}{read.data ? <DomainManagementContent read={read.data} reload={() => void read.reload()} /> : read.loading ? <p role="status">正在读取领域配置…</p> : null}
  </CollectionFrame>;
}
