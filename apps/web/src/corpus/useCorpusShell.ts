import { useQuery } from '@tanstack/react-query';
import { fetchCorpusShell } from './corpusShellApi';

/**
 * 读取语料外壳（领域列表、解析后的当前领域、采集状态词）。
 * 缓存不当作新事实：不保留已卸载页面的结果，切换领域时不拿上一个领域的数据占位。
 */
export function useCorpusShell(domainParam: string | null) {
  return useQuery({
    queryKey: ['corpus-shell', domainParam ?? ''],
    queryFn: ({ signal }) => fetchCorpusShell(domainParam, signal),
    gcTime: 0,
  });
}
