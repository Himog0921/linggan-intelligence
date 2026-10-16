import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { requestJson } from '../../../shared/api';
import { errorText, number, object, text } from './tasksApi';

export function ExecutionDiagnostics({ domain, run }: { domain?: string | null; run?: string }) {
  const [page, setPage] = useState(1);
  const query = useQuery({ queryKey: ['collection-executions', domain, run, page], queryFn: async ({ signal }) => {
    const params = new URLSearchParams({ page: String(page) }); if (domain) params.set('domainRef', domain); if (run) params.set('runRef', run);
    const data = object(await requestJson(`/api/collection/executions?${params}`, { signal }));
    if (!Array.isArray(data.items)) throw new Error('执行记录格式无法核对');
    return { total: number(data.total), items: data.items.map(value => { const v = object(value); return { workOrderRef: v.workOrderRef, target: v.target, frozenScope: v.frozenScope, lease: v.lease, attempts: v.attempts, delivery: v.delivery, taskId: text(v.taskId), capabilities: Array.isArray(v.capabilities) ? v.capabilities.map(text) : null, stageState: v.stageState == null ? null : text(v.stageState) }; }) };
  }, gcTime: 0 });
  if (!query.data) return <p>{query.error ? `诊断记录读取失败：${errorText(query.error)}` : '正在读取执行记录…'}</p>;
  return <section data-full-executions><p>完整执行记录：共 {query.data.total} 条 · 第 {page} 页</p>{query.data.items.map(t => <details key={t.taskId}><summary>{t.capabilities?.join(' / ')} · {t.stageState ?? '未认领'} · {t.taskId}</summary><dl><dt>冻结范围与资源</dt><dd><pre>{JSON.stringify({ work: t.workOrderRef, target: t.target, scope: t.frozenScope, lease: t.lease }, null, 2)}</pre></dd><dt>Attempt / Package / Receipt</dt><dd><pre>{JSON.stringify(t.attempts, null, 2)}</pre></dd><dt>交付对账</dt><dd><pre>{JSON.stringify(t.delivery, null, 2)}</pre></dd></dl></details>)}<button type="button" className="c-btn-quiet" disabled={page <= 1 || query.isFetching} onClick={() => setPage(page - 1)}>上一页</button><button type="button" className="c-btn-quiet" disabled={page * 25 >= query.data.total || query.isFetching} onClick={() => setPage(page + 1)}>下一页</button></section>;
}
