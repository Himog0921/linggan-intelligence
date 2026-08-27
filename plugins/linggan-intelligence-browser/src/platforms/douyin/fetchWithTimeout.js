const DEFAULT_DOUYIN_FETCH_TIMEOUT_MS = 8000;

export async function fetchDouyinWithTimeout(url, options = {}, {
  timeoutMs = DEFAULT_DOUYIN_FETCH_TIMEOUT_MS,
  fetchImpl = globalThis.fetch,
  AbortControllerImpl = globalThis.AbortController,
  setTimeoutFn = globalThis.setTimeout,
  clearTimeoutFn = globalThis.clearTimeout,
} = {}) {
  if (typeof fetchImpl !== 'function') {
    throw new Error('当前环境不支持网络请求');
  }

  const timeout = Math.max(0, Number(timeoutMs || 0) || 0);
  if (!timeout || typeof AbortControllerImpl !== 'function') {
    return fetchImpl(url, options);
  }

  const controller = new AbortControllerImpl();
  let timedOut = false;
  const timer = setTimeoutFn(() => {
    timedOut = true;
    controller.abort();
  }, timeout);

  try {
    return await fetchImpl(url, {
      ...options,
      signal: controller.signal,
    });
  } catch (error) {
    if (timedOut) {
      const timeoutError = new Error(`抖音接口请求超时（${timeout}ms）`);
      timeoutError.code = 'douyin_fetch_timeout';
      timeoutError.cause = error;
      throw timeoutError;
    }
    throw error;
  } finally {
    clearTimeoutFn(timer);
  }
}
