/** 全站唯一一级导航登记；WEB-REACT-001 H 后不再维护 Rust 页头副本。 */
export type PrimarySurface = 'corpus' | 'collection' | 'topic' | 'settings' | 'ops';

export interface PrimaryEntry {
  zh: string;
  surface: PrimarySurface | null;
  /** 没有已提供路由的责任保持禁用按钮，不变成死链。 */
  href: string | null;
}

export const PRIMARY_ENTRIES: readonly PrimaryEntry[] = [
  { zh: '雷达', surface: null, href: null },
  { zh: '主题图谱', surface: 'topic', href: '/topics' },
  { zh: '语料', surface: 'corpus', href: '/corpus' },
  { zh: '洞察', surface: null, href: null },
  { zh: '采集', surface: 'collection', href: '/collection' },
  { zh: '运营', surface: 'ops', href: '/ops' },
];

export const PERSONAL_MENU_LINKS: readonly { label: string; href: string }[] = [
  { label: '设置', href: '/settings' },
  { label: '账户安全', href: '/settings/account' },
  { label: '团队成员', href: '/settings/team' },
];
