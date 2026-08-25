export const LINGGAN_LOCAL_ORIGIN = 'http://localhost:3000';

export const LINGGAN_PENDING_MESSAGE = [
  '该采集能力已保留在 Linggan 插件界面中，但 Linggan 的对应接收合同尚未接通。',
  '本次点击没有访问平台、没有下载媒体，也没有写入 Linggan。',
].join(' ');

export function createLingganPendingResult(capability = '') {
  return {
    success: false,
    code: 'linggan_adapter_pending',
    capability: String(capability || '').trim(),
    message: LINGGAN_PENDING_MESSAGE,
    error: LINGGAN_PENDING_MESSAGE,
  };
}

export async function assertLingganCapability(capability = '') {
  const result = createLingganPendingResult(capability);
  throw new Error(result.message);
}

export function formatLingganIdleNotice() {
  return 'Linggan 尚未开放自动任务、工位或远程调度；页面保留原控制位置，但当前不会接单。';
}

export async function readLingganLocalReadiness(fetchImpl = globalThis.fetch) {
  if (typeof fetchImpl !== 'function') {
    return { connected: false, message: '浏览器当前无法检查 Linggan 本机服务。' };
  }
  try {
    const response = await fetchImpl(`${LINGGAN_LOCAL_ORIGIN}/health`, {
      method: 'GET',
      credentials: 'omit',
    });
    if (!response.ok) {
      return { connected: false, message: `Linggan 本机服务返回 ${response.status}。` };
    }
    return { connected: true, message: 'Linggan 本机服务可访问；采集 adapter 仍待逐项接通。' };
  } catch {
    return { connected: false, message: 'Linggan 本机服务当前不可访问。' };
  }
}
