import type { ReactNode } from 'react';
import { Sidebar, type SidebarNavigationItem } from '../shared/Sidebar';
import type { CorpusCommentsView, CorpusPageKey } from './corpusPages';

/** 证据库页填写的侧栏计数：本次读取的作品数，及悬停说明。 */
export interface EvidenceCount {
  count: number;
  title: string;
}

interface CorpusSideNavProps {
  active: CorpusPageKey;
  /** 当前已确认的领域。没有选定领域时链接保持裸路径，不传入虚假的读取范围。 */
  domain: string | null;
  /** 评论研究当前的三级页面。 */
  view?: CorpusCommentsView;
  /** 评论研究的视图切换由页面保留原有筛选与历史语义。 */
  onViewSelect?: (view: CorpusCommentsView) => void;
  foot?: ReactNode;
  evidenceCount?: EvidenceCount | null;
}

const COMMENT_VIEWS: readonly { key: CorpusCommentsView; label: string; code: string }[] = [
  { key: 'overview', label: '概览', code: 'OV' },
  { key: 'comments', label: '用户评论', code: 'UC' },
  { key: 'runs', label: '研究记录', code: 'RUN' },
  { key: 'problems', label: '用户问题', code: 'Q' },
];

/** 语料导航：二级页面为主项；评论研究的四个真实视图作为三级子项。 */
export function CorpusSideNav({ active, domain, view = 'overview', onViewSelect, foot, evidenceCount = null }: CorpusSideNavProps) {
  const scope = domain ? `?domain=${encodeURIComponent(domain)}` : '';
  const commentHref = (next: CorpusCommentsView) => `/corpus/comments${scope ? `${scope}&view=${next}` : `?view=${next}`}`;
  const items: SidebarNavigationItem[] = [
    {
      id: 'evidence',
      label: '证据库',
      index: '01',
      icon: 'evidence',
      href: `/corpus/evidence${scope}`,
      badge: evidenceCount && evidenceCount.count > 0 ? evidenceCount.count : undefined,
      badgeTitle: evidenceCount?.title,
    },
    {
      id: 'comments',
      label: '评论研究',
      index: '02',
      icon: 'comments',
      href: commentHref('overview'),
      children: COMMENT_VIEWS.map(item => ({
        id: `comments-${item.key}`,
        label: item.label,
        code: item.code,
        href: commentHref(item.key),
        onSelect: onViewSelect ? () => onViewSelect(item.key) : undefined,
      })),
    },
    {
      id: 'creators',
      label: '创作者',
      index: '03',
      icon: 'creators',
      href: `/corpus/creators${scope}`,
    },
  ];
  const activeId = active === 'comments' ? `comments-${view}` : active;

  return (
    <Sidebar
      aria-label="语料导航"
      eyebrow="CORPUS / NAV"
      title="语料工作区"
      items={items}
      activeId={activeId}
      activeParentId={active === 'comments' ? 'comments' : undefined}
      foot={foot}
    />
  );
}
