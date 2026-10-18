import { useQuery } from '@tanstack/react-query';
import { parsePolicyPage } from './methodDtos';
import { studyKey, studyRead } from './studyApi';

export function usePolicies(domain: string, enabled = true) {
  return useQuery({ queryKey: studyKey(domain, 'all-policies'), queryFn: async ({ signal }) => {
    let page = await studyRead(domain, 'policies?limit=100', parsePolicyPage, signal);
    const policies = [...page.items], seen = new Set<string>();
    while (page.page.hasMore) {
      const cursor = page.page.nextCursor;
      if (!cursor || seen.has(cursor)) throw new Error('方法目录分页回执不完整');
      seen.add(cursor);
      page = await studyRead(domain, `policies?limit=100&cursor=${encodeURIComponent(cursor)}`, parsePolicyPage, signal);
      policies.push(...page.items);
    }
    return policies;
  }, enabled, retry: false, gcTime: 0, refetchOnWindowFocus: false });
}
