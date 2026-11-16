import { type ReactNode } from 'react';
import { useQuery } from '@tanstack/react-query';
import { isRecord, requestJson } from '../shared/api';
import { OpportunityStatusContext } from './opportunityStatusContext';

async function readStatus(signal: AbortSignal): Promise<boolean> {
  const result = await requestJson('/api/local/opportunity/status', { signal });
  if (!isRecord(result) || typeof result.enabled !== 'boolean') throw new Error('功能状态回执无法解析');
  return result.enabled;
}

export function OpportunityStatusProvider({ children }: { children: ReactNode }) {
  const query = useQuery({
    queryKey: ['opportunity', 'status'],
    queryFn: ({ signal }) => readStatus(signal),
    retry: false,
    refetchOnWindowFocus: true,
  });
  const state = query.isError ? 'unavailable' : query.isPending ? 'loading' : query.data ? 'enabled' : 'disabled';
  return <OpportunityStatusContext.Provider value={{ state, retry: () => { void query.refetch(); } }}>{children}</OpportunityStatusContext.Provider>;
}
