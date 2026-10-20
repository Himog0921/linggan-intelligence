import { useState } from 'react';
import { errorText } from './format';
import { parsePolicyDetail } from './methodDtos';
import { PolicySnapshot } from './PolicySnapshot';
import { useStudyRead } from './studyApi';
import { usePolicies } from './usePolicies';
import { c } from './styles';

/** Immutable historical methods remain readable; creation, editing and activation are retired. */
export function PolicyHistory({ domain }: { domain: string }) {
  const [open, setOpen] = useState(false), [selected, setSelected] = useState(''), [compare, setCompare] = useState('');
  const policies = usePolicies(domain, open);
  const currentRef = selected || policies.data?.[0]?.policyRef || '';
  const current = useStudyRead(domain, `policies/${encodeURIComponent(currentRef)}`, parsePolicyDetail, open && Boolean(currentRef));
  const other = useStudyRead(domain, `policies/${encodeURIComponent(compare)}`, parsePolicyDetail, open && Boolean(compare));
  return <details className={c('study-catalog-context')} onToggle={event => setOpen(event.currentTarget.open)}><summary>查看历史方法版本</summary>
    <p>以下是旧研究保存的不可变方法；执行链已停用，不能在此编辑或启用。</p>
    {policies.isError ? <p role="status">方法目录读取失败：{errorText(policies.error)}</p> : policies.isPending ? <p>正在读取历史方法…</p> : !policies.data?.length ? <p>没有已记录的历史方法版本。</p> : <>
      <div className={c('study-policy-current')}><label>历史方法版本<select value={currentRef} onChange={event => { setSelected(event.target.value); setCompare(''); }}>{policies.data.map(policy => <option key={policy.policyRef} value={policy.policyRef}>{policy.methodName || '未命名方法'} · {policy.policyRef.slice(0, 8)}{policy.recordingState !== 'recorded' ? '（版本未完整记录）' : ''}</option>)}</select></label>
        <label>并排对比其他历史版本<select value={compare} onChange={event => setCompare(event.target.value)}><option value="">不对比</option>{policies.data.filter(policy => policy.policyRef !== currentRef).map(policy => <option key={policy.policyRef} value={policy.policyRef}>{policy.methodName || '未命名方法'} · {policy.policyRef.slice(0, 8)}</option>)}</select></label></div>
      {current.isError ? <p role="status">方法详情读取失败：{errorText(current.error)}</p> : current.isPending ? <p>正在读取固定版本…</p> : <div className={c('study-policy-compare')}><PolicySnapshot policy={current.data} />{compare && other.data ? <PolicySnapshot policy={other.data} /> : null}</div>}
      {compare && other.isError ? <p role="status">对比版本读取失败：{errorText(other.error)}</p> : null}
    </>}
  </details>;
}
