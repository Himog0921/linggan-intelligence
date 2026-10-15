import type { DrawerTab } from './CreatorTable';

/**
 * 去证据库看原作品之前，记下本页的位置（表格滚动、作者、抽屉分区与展开项）；浏览器返回时恢复。
 * 这里只有位置信息，不含任何作品或原声内容；存取失败（隐私模式等）时返回照常可用，只是不恢复位置。
 */
const KEY = 'linggan.creatorDiscovery.return';

export interface ReturnState {
  /** 离开时的页面地址（路径 + 查询串）；只有回到同一个地址才恢复。 */
  url: string;
  scrollY: number;
  scrollX: number;
  creatorKey?: string;
  drawer?: boolean;
  tab?: DrawerTab;
  drawerScroll?: number;
  expanded?: string[];
}

export function saveReturn(state: ReturnState): void {
  try {
    sessionStorage.setItem(KEY, JSON.stringify(state));
  } catch {
    /* 存储不可用时，导航照常，只是不恢复位置。 */
  }
}

/** 读取但不删除（严格模式下初始化函数会执行两次）；用完后调用 `clearReturn`。 */
export function peekReturn(url: string): ReturnState | null {
  try {
    const value: unknown = JSON.parse(sessionStorage.getItem(KEY) ?? 'null');
    if (typeof value !== 'object' || value === null) return null;
    const state = value as Partial<ReturnState>;
    if (state.url !== url || !Number.isFinite(state.scrollY) || !Number.isFinite(state.scrollX)) return null;
    return state as ReturnState;
  } catch {
    return null;
  }
}

export function clearReturn(): void {
  try {
    sessionStorage.removeItem(KEY);
  } catch {
    /* 同上。 */
  }
}
