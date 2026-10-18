import { useInfiniteQuery } from '@tanstack/react-query';
import { parseRuns } from './runDtos';
import { studyKey, studyRead } from './studyApi';

export function useRunDirectory(domain: string, enabled = true) {
  return useInfiniteQuery({ queryKey: studyKey(domain, 'runs-directory'), initialPageParam: null as string | null,
    queryFn: ({ signal, pageParam }) => { const params = new URLSearchParams({ limit: '50' }); if (pageParam) params.set('cursor', pageParam); return studyRead(domain, `runs?${params}`, parseRuns, signal); },
    getNextPageParam: page => page.page.nextCursor || undefined, enabled, retry: false, gcTime: Infinity, refetchOnWindowFocus: false,
  });
}
