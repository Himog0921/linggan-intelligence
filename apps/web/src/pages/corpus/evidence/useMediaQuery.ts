import { useSyncExternalStore } from 'react';

/** 抽屉模式：检查器在 ≤1180px 变成覆盖在列表上的抽屉，而不是一栏。 */
export const DRAWER_QUERY = '(max-width: 1180px)';
/** 支持悬停的设备：封面卡悬停即翻；触屏改成点击翻转。 */
export const NO_HOVER_QUERY = '(hover: none)';

export function matches(query: string): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia(query).matches;
}

function subscribeTo(query: string, onChange: () => void): () => void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return () => undefined;
  const list = window.matchMedia(query);
  list.addEventListener('change', onChange);
  return () => list.removeEventListener('change', onChange);
}

/** 订阅媒体查询。没有 matchMedia（例如测试环境）时恒为 false。 */
export function useMediaQuery(query: string): boolean {
  return useSyncExternalStore(
    onChange => subscribeTo(query, onChange),
    () => matches(query),
    () => false,
  );
}
