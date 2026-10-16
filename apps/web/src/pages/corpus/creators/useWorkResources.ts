import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { fetchWorkResources } from './creatorsApi';
import type { WorkResource } from './creatorsTypes';

export interface WorkResources {
  byRef: ReadonlyMap<string, WorkResource>;
  /** 资源还在读取（尚未返回，也没有失败）。 */
  pending: boolean;
  /** 读取失败：文字和链接仍可用，媒体暂不可用。 */
  failed: boolean;
}

/** 批量读取作品的标题、头像与封面。每次都是新读取，不拿别的范围的缓存当现状。 */
export function useWorkResources(domain: string | null, refs: readonly string[]): WorkResources {
  const unique = useMemo(() => [...new Set(refs)].sort(), [refs]);
  const query = useQuery({
    queryKey: ['work-resources', domain, unique.join(',')],
    queryFn: ({ signal }) => fetchWorkResources(domain ?? '', unique, signal),
    enabled: domain !== null && unique.length > 0,
    gcTime: 0,
  });
  const byRef = useMemo(() => new Map((query.data ?? []).map(resource => [resource.publicRef, resource] as const)), [query.data]);
  return { byRef, pending: query.isPending && query.fetchStatus !== 'idle', failed: query.isError };
}

/** 合并列表与抽屉各自读到的资源：同一作品以较新的（抽屉）为准。 */
export function mergeResources(list: WorkResources, extra: WorkResources): WorkResources {
  if (extra.byRef.size === 0) return { byRef: list.byRef, pending: list.pending || extra.pending, failed: list.failed || extra.failed };
  return {
    byRef: new Map([...list.byRef, ...extra.byRef]),
    pending: list.pending || extra.pending,
    failed: list.failed || extra.failed,
  };
}
