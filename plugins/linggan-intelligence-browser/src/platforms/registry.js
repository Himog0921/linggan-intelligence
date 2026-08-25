import { createDouyinPlatformAdapter } from './douyin/adapter.js';
import { createXhsPlatformAdapter } from './xhs/adapter.js';

export const PLATFORM_ID = Object.freeze({
  XHS: 'xhs',
  DOUYIN: 'douyin',
});

/**
 * @param {{xhs?: Record<string, any>, douyin?: Record<string, any>}} [options]
 */
export function createPlatformAdapterRegistry({
  xhs = {},
  douyin = {},
} = {}) {
  /** @type {Map<string, any>} */
  const adapters = new Map([
    [PLATFORM_ID.XHS, createXhsPlatformAdapter(xhs)],
    [PLATFORM_ID.DOUYIN, createDouyinPlatformAdapter(douyin)],
  ]);

  return {
    get(platform = '') {
      return adapters.get(String(platform || '').trim()) || null;
    },

    require(platform = '') {
      const adapter = this.get(platform);
      if (!adapter) {
        throw new Error(`未注册平台适配器：${platform}`);
      }
      return adapter;
    },

    list() {
      return [...adapters.values()];
    },
  };
}
