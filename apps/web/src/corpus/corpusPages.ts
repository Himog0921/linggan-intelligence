/**
 * 语料二级导航登记表。迁移期间评论研究仍由旧服务提供，它的侧栏由
 * `apps/api/src/local_web/shell.rs` 的 `corpus_side_nav` 输出，两边必须一致（WEB-REACT-001 规则 R4）：
 * `corpusPages.test.ts` 逐项对照 shell.rs 的 `CORPUS_ENTRIES`。
 * 三页全部迁完、shell.rs 删除之后，这里成为唯一来源。
 */
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
