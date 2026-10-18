/** 全站唯一一级导航登记；WEB-REACT-001 H 后不再维护 Rust 页头副本。 */
export type PrimarySurface = 'corpus' | 'collection' | 'topic' | 'settings' | 'ops';

export interface PrimaryEntry {
  zh: string;
  /** 英文技术旁注；空字符串表示不显示（运营入口按 LANG-05 不带英文代号）。 */
  technicalKey: string;
  /** 辅助行的状态词。读不到真实事实时保持「未知」类表达，不宣布已接通或为空。 */
  state: string;
  surface: PrimarySurface | null;
  /** 没有已提供路由的责任保持禁用按钮，不变成死链。 */
  href: string | null;
}

export const PRIMARY_ENTRIES: readonly PrimaryEntry[] = [
  { zh: '雷达', technicalKey: 'RADAR', state: '尚未接通', surface: null, href: null },
  { zh: '主题图谱', technicalKey: 'TOPIC MAP', state: '暂定研究', surface: 'topic', href: '/topics' },
  { zh: '语料', technicalKey: 'CORPUS', state: '未知', surface: 'corpus', href: '/corpus' },
  { zh: '洞察', technicalKey: 'INSIGHTS', state: '尚未接通', surface: null, href: null },
  { zh: '采集', technicalKey: 'COLLECTION', state: '状态未知', surface: 'collection', href: '/collection' },
  { zh: '运营', technicalKey: '', state: '只读', surface: 'ops', href: '/ops' },
];

export const PERSONAL_MENU_LINKS: readonly { label: string; href: string }[] = [
  { label: '设置', href: '/settings' },
];
