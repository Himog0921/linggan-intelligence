/** 语料模块的二级页面及评论研究的三级视图。 */
export type CorpusPageKey = 'evidence' | 'comments' | 'creators';
export type CorpusCommentsView = 'overview' | 'comments' | 'runs' | 'problems';

export interface CorpusEntry {
  ordinal: string;
  label: string;
  href: string | null;
  page: CorpusPageKey | null;
}

export const CORPUS_ENTRIES: readonly CorpusEntry[] = [
  { ordinal: '01', label: '证据库', href: '/corpus/evidence', page: 'evidence' },
  { ordinal: '02', label: '评论研究', href: '/corpus/comments', page: 'comments' },
  { ordinal: '03', label: '创作者', href: '/corpus/creators', page: 'creators' },
];
