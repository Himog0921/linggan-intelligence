import { useQuery } from '@tanstack/react-query';
import { useLocation } from 'react-router';

export function useManagementRead<T>(load: (signal: AbortSignal, search: string) => Promise<T>) {
  const {search}=useLocation();
  const query = useQuery({
    queryKey: ['collection-management', load.name, search],
    queryFn: ({ signal }) => load(signal,search),
    gcTime: 0,
    retry: false,
  });
  return { data: query.data ?? null, error: query.isError, loading: query.isPending, reload: query.refetch };
}
