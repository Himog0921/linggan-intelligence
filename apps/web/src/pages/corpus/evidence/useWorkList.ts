import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { fetchWorkList } from './evidenceApi';
import { listKey } from './evidenceKeys';
import { toApiParams, type EvidenceQuery } from './evidenceQuery';
import type { WorkItem, WorkList } from './evidenceTypes';

/** 追加的页里可能有已经读到过的作品（两次读取之间库里有了变化）：按稳定引用去重，保留先到的。 */
export function mergePages(pages: readonly WorkList[]): WorkItem[] {
  const seen = new Set<string>();
  const items: WorkItem[] = [];
  for (const page of pages) {
    for (const item of page.items) {
      if (seen.has(item.ref)) continue;
      seen.add(item.ref);
      items.push(item);
    }
  }
  return items;
}

/**
 * 作品列表（每页 50 条，游标续读）。键里带领域和完整的接口参数，范围一变就是另一个查询：晚到的响应不会覆盖
 * 新范围，也不拿上一个范围的数据占位。`blocked` 为真（地址里的筛选条件无效）或没有领域时一律不发请求。
 */
export function useWorkList(domain: string | null, query: EvidenceQuery, blocked: boolean) {
  const apiKey = domain === null ? '' : toApiParams(domain, query).toString();
  const list = useInfiniteQuery({
    queryKey: listKey(domain, apiKey),
    queryFn: ({ pageParam, signal }) => fetchWorkList(domain ?? '', query, pageParam, signal),
    initialPageParam: null as string | null,
    getNextPageParam: page => page.cursor,
    enabled: domain !== null && !blocked,
    gcTime: 0,
  });
  const pages = list.data?.pages;
  const items = useMemo(() => mergePages(pages ?? []), [pages]);
  return { list, items, last: pages?.at(-1) ?? null, apiKey };
}
