import { useQuery } from '@tanstack/react-query';
import { fetchWorkDetail } from './evidenceApi';
import { detailKey } from './evidenceKeys';
import { detailPath, directDetailPath } from './evidencePaths';
import type { WorkItem } from './evidenceTypes';
import type { DetailState } from './Inspector';

interface Input {
  domain: string | null;
  selected: string | null;
  /** 选中的作品在已读到的列表里时的那一行；不在（地址直达）时为 null。 */
  listItem: WorkItem | null;
  collectionRunRef: string | null;
}

/**
 * 选中作品的详情。列表读到之后才有「选中」：在此之前不知道地址里的作品是在页内还是要直读。
 * 详情地址只有两处来源，且都经过同源与形状校验：列表行给出的 `detailUrl`；地址里的 `work` 不在已读页内时
 * 直读（保持所选领域与 `collectionRunRef`，不回退到首项）。列表没给出可用地址就是「来源信息不完整」，不猜路由。
 */
export function useWorkDetail({ domain, selected, listItem, collectionRunRef }: Input) {
  let path: string | null = null;
  if (domain !== null && selected !== null) {
    path = listItem ? detailPath(listItem.detailUrl) : directDetailPath(selected, domain, collectionRunRef);
  }
  const read = useQuery({
    queryKey: detailKey(domain, path),
    queryFn: ({ signal }) => fetchWorkDetail(path ?? '', signal),
    enabled: path !== null,
    gcTime: 0,
  });

  let status: DetailState['status'] = 'idle';
  if (selected !== null) {
    if (listItem && path === null) status = 'incomplete';
    else if (read.data) status = 'ready';
    else if (read.isError) status = 'error';
    else status = 'loading';
  }
  const detail: DetailState = { status, data: read.data ?? null, error: read.error, direct: selected !== null && listItem === null };

  /** 重读；返回是否成功（复观测终态后用它对账，失败要如实说）。 */
  async function refetch(): Promise<boolean> {
    return (await read.refetch()).isSuccess;
  }

  return { detail, refetch };
}
