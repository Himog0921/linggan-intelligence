/** 语料三页的唯一 React 导航登记表。共享样式仍直接消费 shell.css（WEB-REACT-001 R4）。 */
export type CorpusPageKey = 'evidence' | 'comments' | 'creators';

export interface CorpusEntry {
  ordinal: string;
  label: string;
  /** 没有已提供路由的项保持禁用，不变成死链。 */
  href: string | null;
  page: CorpusPageKey | null;
}

export const CORPUS_ENTRIES: readonly CorpusEntry[] = [
  { ordinal: '01', label: '证据库', href: '/corpus/evidence', page: 'evidence' },
  { ordinal: '02', label: '评论研究', href: '/corpus/comments', page: 'comments' },
  { ordinal: '03', label: '创作者', href: '/corpus/creators', page: 'creators' },
];
