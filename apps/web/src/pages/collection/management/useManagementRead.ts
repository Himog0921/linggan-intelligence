import { useQuery } from '@tanstack/react-query';

export function useManagementRead<T>(load: (signal: AbortSignal) => Promise<T>) {
  const query = useQuery({
    queryKey: ['collection-management', load.name, window.location.search],
    queryFn: ({ signal }) => load(signal),
    gcTime: 0,
    retry: false,
  });
  return { data: query.data ?? null, error: query.isError, loading: query.isPending, reload: query.refetch };
}
