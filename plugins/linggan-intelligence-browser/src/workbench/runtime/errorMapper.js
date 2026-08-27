import { REMOTE_ERROR_CATEGORY, REMOTE_ERROR_CODE } from '../protocol/schema.js';

const CATEGORY_USER_MESSAGE = {
  [REMOTE_ERROR_CATEGORY.CONTEXT]: '当前页面不匹配任务要求',
  [REMOTE_ERROR_CATEGORY.AUTH]: '需要先登录对应平台账号',
  [REMOTE_ERROR_CATEGORY.NETWORK]: '网络连接异常，请检查网络',
  [REMOTE_ERROR_CATEGORY.PLATFORM_BLOCK]: '平台触发了安全验证，请稍后重试',
  [REMOTE_ERROR_CATEGORY.RATE_LIMIT]: '操作过于频繁，请稍后重试',
  [REMOTE_ERROR_CATEGORY.STORAGE]: '本地存储空间不足',
  [REMOTE_ERROR_CATEGORY.DOWNLOAD]: '媒体文件下载失败',
  [REMOTE_ERROR_CATEGORY.USER_CANCEL]: '用户取消了任务',
  [REMOTE_ERROR_CATEGORY.INTERNAL]: '系统内部错误',
};

function normalizeMessage(error = '') {
  return String(error?.message || error || '').trim();
}

function inferErrorCode(message = '') {
  const text = normalizeMessage(message);
  if (/页面权限|缺少权限|host.?permission|permission.?denied/i.test(text)) {
    return REMOTE_ERROR_CODE.PAGE_PERMISSION_DENIED;
  }
  if (/不存在|暂时无法浏览|无法浏览|已删除|已私密|设为私密|仅.{0,3}自己可见|链接.{0,3}失效|不可见|不可访问|已隐藏|404|not.?found|removed|deleted|private/i.test(text)) {
    return REMOTE_ERROR_CODE.CONTENT_NOT_FOUND;
  }
  if (/错误页|页面错误|error.?page/i.test(text)) {
    return REMOTE_ERROR_CODE.ERROR_PAGE;
  }
  if (/登录已失效|login.?expired|登录态失效/i.test(text)) {
    return REMOTE_ERROR_CODE.LOGIN_EXPIRED;
  }
  if (/安全验证|验证码|滑块|请完成验证|security.?challenge|captcha|verify/i.test(text)) {
    return REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE;
  }
  if (/平台拦截|风控|platform.?blocked/i.test(text)) {
    return REMOTE_ERROR_CODE.PLATFORM_BLOCKED;
  }
  if (/只有心跳|没有新数据|heartbeat.?only/i.test(text)) {
    return REMOTE_ERROR_CODE.HEARTBEAT_ONLY_STALL;
  }
  if (/搜索结果列表|稳定搜索列表/.test(text)) return REMOTE_ERROR_CODE.SEARCH_LIST_UNSTABLE;
  if (/当前页面|可执行上下文|页面未形成/.test(text)) return REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE;
  if (/登录|login/i.test(text)) return REMOTE_ERROR_CODE.LOGIN_REQUIRED;
  if (/限流|频率|rate.?limit|too.?many/i.test(text)) return 'rate_limited';
  if (/下载/.test(text)) return REMOTE_ERROR_CODE.DOWNLOAD_FAILED;
  if (/写入|数据库|storage|indexeddb/i.test(text)) return REMOTE_ERROR_CODE.STORAGE_WRITE_FAILED;
  if (/停止|取消/.test(text)) return REMOTE_ERROR_CODE.TASK_STOPPED_BY_USER;
  return REMOTE_ERROR_CODE.UNEXPECTED_INTERNAL_ERROR;
}

function inferErrorCategory(code = '', message = '') {
  if (code === 'rate_limited') return REMOTE_ERROR_CATEGORY.RATE_LIMIT;
  if (
    code === REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE ||
    code === REMOTE_ERROR_CODE.PLATFORM_BLOCKED
  ) {
    return REMOTE_ERROR_CATEGORY.PLATFORM_BLOCK;
  }
  if (
    code === REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE ||
    code === REMOTE_ERROR_CODE.SEARCH_LIST_UNSTABLE ||
    code === REMOTE_ERROR_CODE.ERROR_PAGE
  ) {
    return REMOTE_ERROR_CATEGORY.CONTEXT;
  }
  if (
    code === REMOTE_ERROR_CODE.LOGIN_REQUIRED ||
    code === REMOTE_ERROR_CODE.LOGIN_EXPIRED
  ) {
    return REMOTE_ERROR_CATEGORY.AUTH;
  }
  if (code === REMOTE_ERROR_CODE.PAGE_PERMISSION_DENIED) return REMOTE_ERROR_CATEGORY.AUTH;
  if (code === REMOTE_ERROR_CODE.CONTENT_NOT_FOUND) return REMOTE_ERROR_CATEGORY.CONTEXT;
  if (code === REMOTE_ERROR_CODE.DOWNLOAD_FAILED) {
    return REMOTE_ERROR_CATEGORY.DOWNLOAD;
  }
  if (code === REMOTE_ERROR_CODE.STORAGE_WRITE_FAILED) {
    return REMOTE_ERROR_CATEGORY.STORAGE;
  }
  if (code === REMOTE_ERROR_CODE.TASK_STOPPED_BY_USER || /停止|取消/.test(message)) {
    return REMOTE_ERROR_CATEGORY.USER_CANCEL;
  }
  return REMOTE_ERROR_CATEGORY.INTERNAL;
}

function inferRetryable(code = '', category = '') {
  if (category === REMOTE_ERROR_CATEGORY.USER_CANCEL) return false;
  if (
    code === REMOTE_ERROR_CODE.PAGE_PERMISSION_DENIED ||
    code === REMOTE_ERROR_CODE.LOGIN_EXPIRED ||
    code === REMOTE_ERROR_CODE.CONTENT_NOT_FOUND
  ) {
    return false;
  }
  if (category === REMOTE_ERROR_CATEGORY.RATE_LIMIT) return true;
  if (code === REMOTE_ERROR_CODE.TASK_STOPPED_BY_USER) return false;
  return true;
}

export function mapErrorToProtocolError(error, overrides = {}) {
  const message = normalizeMessage(error) || '未知错误';
  const code = String(overrides.code || '').trim() || inferErrorCode(message);
  const category = String(overrides.category || '').trim() || inferErrorCategory(code, message);
  const retryable = typeof overrides.retryable === 'boolean'
    ? overrides.retryable
    : inferRetryable(code, category);

  return {
    code,
    message,
    retryable,
    category,
    userMessage: CATEGORY_USER_MESSAGE[category] || '系统内部错误',
  };
}
