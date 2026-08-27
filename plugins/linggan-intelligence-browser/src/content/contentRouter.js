import { CONTENT_PLATFORM, resolveContentPlatform } from './contentPlatformRegistry.js';

export { CONTENT_PLATFORM };

export function createContentRouter({
  getHostname = () => '',
  initByPlatform = {},
  defaultPlatform = CONTENT_PLATFORM.XHS,
} = {}) {
  function resolvePlatform(hostname = getHostname()) {
    return resolveContentPlatform(hostname);
  }

  async function init(hostname = getHostname()) {
    const platform = resolvePlatform(hostname);
    const initHandler = initByPlatform[platform] || initByPlatform[defaultPlatform];

    if (typeof initHandler !== 'function') {
      throw new Error(`未配置平台初始化器：${platform}`);
    }

    await initHandler({
      platform,
      hostname,
    });

    return platform;
  }

  return {
    init,
    resolvePlatform,
  };
}
