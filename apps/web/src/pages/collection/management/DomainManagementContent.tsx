import { CommandForm } from './CommandForm';
import type { DomainEntry, DomainsRead } from './managementApi';

function DomainForm({ domain, returnTo, reload, onApplied }: { domain?: DomainEntry; returnTo: string; reload: () => void; onApplied?: () => void }) {
  return <CommandForm action="domain-save" resetOnApplied={!domain} className={domain ? 'domain-fields' : 'domain-create'} returnTo={returnTo} onWritten={reload} onApplied={onApplied} fields={domain ? { domain_ref: domain.domainRef } : {}}>
    {!domain && <h2>创建领域</h2>}
    <label>名称<input name="name" required maxLength={120} defaultValue={domain?.name ?? ''} /></label>
    <label>说明<input name="description" maxLength={1000} defaultValue={domain ? domain.description ?? '暂无领域说明' : ''} /></label>
    <label>研究目标<input name="research_goal" maxLength={1000} defaultValue={domain?.researchGoal ?? ''} /></label>
    {domain ? <label>状态<select name="status" defaultValue={domain.status}><option value="active">运行中</option><option value="paused">已暂停</option></select></label> : <input type="hidden" name="status" value="active" />}
    <button className={domain ? 'c-btn-secondary' : 'c-btn-primary'} type="submit">{domain ? '保存领域设置' : '创建领域'}</button>
  </CommandForm>;
}
function DomainCard({ domain, read, reload, compact = false, onSaved }: { domain: DomainEntry; read: DomainsRead; reload: () => void; compact?: boolean; onSaved?: () => void }) {
  const assigned = read.relations.filter(row => row.domainRef === domain.domainRef);
  const statistics = <>
    <div className="domain-counts">
      <span>{`${domain.targetCount} 个观察目标：博主 ${domain.creatorTargetCount ?? "未知"} · 关键词 ${domain.keywordTargetCount ?? "未知"}`}</span>
      <span>{`主研究 ${domain.primaryTargetCount} · 参照 ${domain.referenceTargetCount}`}</span>
      <span>{`${domain.sampleCount ?? "未知"} 篇作品 · ${domain.commentCount ?? "未知"} 条评论`}</span>
      <span>{`最近接纳：${domain.lastReadable ? domain.lastObservation ?? "暂无接纳回执" : "未知"}`}</span>
    </div>
    <div className="domain-lanes"><ul aria-label="各材料通道状态">{domain.lanes.map(row => <li key={row.lane}><span>{row.label}</span><span>{row.summary}</span></li>)}</ul><small>一般通道按每篇作品最新的已接纳通道回执统计；媒体字节依据本地物化与远程候选事实统计。</small></div>
  </>;
  return <article className="domain-card" data-domain-ref={domain.domainRef}>
    <header><div><h2>{domain.name}</h2><span>{domain.status === 'paused' ? '已暂停' : '运行中'}</span></div><p>{domain.description ?? '暂无领域说明'}</p></header>
    <DomainForm domain={domain} returnTo={read.returnTo} reload={reload} onApplied={onSaved} />
    {compact ? <details><summary>材料统计与通道</summary>{statistics}</details> : statistics}
    <table><thead><tr><th>观察目标</th><th>研究角色</th><th>关联</th></tr></thead><tbody>
      {assigned.map(row => <tr key={row.targetRef}><td>{row.name}<small>{row.kindLabel} · {row.lifecycleLabel}</small></td><td><CommandForm action="domain-target" returnTo={read.returnTo} onWritten={reload} fields={{ domain_ref: domain.domainRef, target_ref: row.targetRef, action: 'role' }}>
        <select name="role" defaultValue={row.role}><option value="primary">主研究</option><option value="reference">参照</option></select><button className="c-btn-secondary" type="submit">保存</button>
      </CommandForm></td><td><CommandForm action="domain-target" returnTo={read.returnTo} onWritten={reload} fields={{ domain_ref: domain.domainRef, target_ref: row.targetRef, action: 'remove' }}><button className="c-btn-quiet" type="submit">解除关联</button></CommandForm></td></tr>)}
      {!read.relationsReadable && <tr><td colSpan={3}>关联关系未知</td></tr>}
      {read.relationsReadable && assigned.length === 0 && <tr><td colSpan={3}>当前没有关联观察目标。</td></tr>}
    </tbody></table>
    <CommandForm action="domain-target" className="domain-target-add" returnTo={read.returnTo} onWritten={reload} fields={{ domain_ref: domain.domainRef, action: 'assign' }}>
      <label>关联现有目标<select name="target_ref" required>{read.targets.map(row => <option key={row.targetRef} value={row.targetRef}>{row.name} · {row.kindLabel}</option>)}</select></label>
      <label>角色<select name="role" defaultValue="primary"><option value="primary">主研究</option><option value="reference">参照</option></select></label>
      <button className="c-btn-secondary" type="submit" disabled={domain.status === 'paused' || !read.targetsReadable || !read.relationsReadable}>关联目标</button>
    </CommandForm>
    <footer><a href={`/collection/targets?domain=${encodeURIComponent(domain.domainRef)}`}>查看该领域目标</a> · <a href={`/corpus/evidence?domain=${encodeURIComponent(domain.domainRef)}`}>查看该领域材料</a></footer>
  </article>;
}
export function DomainManagementContent({ read, reload, onlyDomain, onSaved }: { read: DomainsRead; reload: () => void; onlyDomain?: string; onSaved?: () => void }) {
  if (!read.readable) return <p>领域配置和关联关系当前读不到；页面未把未知状态显示为零。</p>;
  const domains = onlyDomain ? read.domains.filter(row => row.domainRef === onlyDomain) : read.domains;
  const content = <>
    {(!onlyDomain || onlyDomain === 'new') && <DomainForm returnTo={read.returnTo} reload={reload} onApplied={onSaved} />}
    {domains.map(domain => <DomainCard key={domain.domainRef} domain={domain} read={read} reload={reload} compact={Boolean(onlyDomain)} onSaved={onSaved} />)}
    {!onlyDomain && domains.length === 0 && <p>暂无领域。创建领域只保存研究边界，不会触发采集。</p>}
    {onlyDomain && onlyDomain !== 'new' && domains.length === 0 && <p role="alert">该领域当前读不到。</p>}
  </>;
  // The legacy modal inserts its card/form directly. The full-page wrapper owns stronger
  // input/label rules, so carrying it into the modal changes typography and every row height.
  return onlyDomain ? content : <section className="domain-management">{content}</section>;
}
