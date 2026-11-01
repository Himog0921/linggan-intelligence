import { useQuery } from '@tanstack/react-query';
import { fetchObservationRecord } from './creatorsApi';
import type { ObservationRecord } from './creatorsTypes';

export interface ObservationRecordState {
  record: ObservationRecord | null;
  /** 还在读（尚未返回，也没有失败）。 */
  pending: boolean;
  failed: boolean;
}

/** 只在该作者已加入本领域观察时读取；每次打开都读现状，不把别的领域或旧缓存当记录。 */
export function useObservationRecord(domain: string | null, creatorKey: string, enabled: boolean): ObservationRecordState {
  const query = useQuery({
    queryKey: ['creator-observation-record', domain, creatorKey],
    queryFn: ({ signal }) => fetchObservationRecord(domain ?? '', creatorKey, signal),
    enabled: enabled && domain !== null,
    gcTime: 0,
  });
  return {
    record: query.data ?? null,
    pending: query.isPending && query.fetchStatus !== 'idle',
    failed: query.isError,
  };
}
