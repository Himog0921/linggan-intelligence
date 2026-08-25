function normalizeString(value = '') {
  return String(value || '').trim();
}

function toFiniteNumber(value, fallback = 0) {
  const num = Number(value);
  return Number.isFinite(num) ? num : fallback;
}

function createManualTaskId({ action = '', tabId = '', now = Date.now, random = Math.random } = {}) {
  const timestamp = typeof now === 'function' ? now() : Date.now();
  const ratio = typeof random === 'function' ? random() : Math.random();
  const suffix = Math.floor(Math.max(0, Math.min(1, Number(ratio) || 0)) * 1_000_000)
    .toString(36)
    .padStart(4, '0');
  return `manual:${normalizeString(action) || 'task'}:${normalizeString(tabId) || 'tab'}:${timestamp}:${suffix}`;
}

function inferPlatformFromUrl(url = '') {
  const normalized = normalizeString(url).toLowerCase();
  if (/xiaohongshu\.com|xhslink\.com/.test(normalized)) return 'xhs';
  if (/douyin\.com/.test(normalized)) return 'douyin';
  return '';
}

function inferPlatformFromTaskType(taskType = '') {
  const normalized = normalizeString(taskType).toLowerCase();
  if (normalized.startsWith('xhs.')) return 'xhs';
  if (normalized.startsWith('douyin.')) return 'douyin';
  return '';
}

export function inferManualExecutionPlatform({ msg = {}, tabUrl = '' } = {}) {
  return normalizeString(msg.platform)
    || inferPlatformFromTaskType(msg.externalTaskMeta?.externalTaskType)
    || inferPlatformFromUrl(tabUrl);
}

function isRemoteWorkbenchDispatch(msg = {}) {
  return Boolean(normalizeString(msg.externalTaskMeta?.externalTaskId));
}

function isUsableAccount(account = {}, platform = '', now = Date.now()) {
  if (normalizeString(account.platform) !== platform) return false;
  if (normalizeString(account.status || 'available') !== 'available') return false;
  if (!normalizeString(account.accountId)) return false;
  if (!normalizeString(account.cookieJson)) return false;
  if (toFiniteNumber(account.cooldownUntil, 0) > now) return false;
  const dailyQuotaLimit = toFiniteNumber(account.dailyQuotaLimit, 100);
  return toFiniteNumber(account.dailyQuotaUsed, 0) < dailyQuotaLimit;
}

async function getCandidateAccounts(accountStore, platform = '', now = Date.now()) {
  if (!accountStore?.getAll) return [];
  const accounts = await accountStore.getAll();
  return (Array.isArray(accounts) ? accounts : [])
    .filter((account) => isUsableAccount(account, platform, now))
    .sort((a, b) => toFiniteNumber(a.lastUsedAt, 0) - toFiniteNumber(b.lastUsedAt, 0));
}

function clearManualLockMessage(platform = '') {
  const label = platform === 'douyin' ? '抖音' : platform === 'xhs' ? '小红书' : '当前平台';
  return `没有可用的${label}账号，无法启动手动采集。请先在插件里添加或恢复可用账号。`;
}

function accountBusyMessage() {
  return '同一账号正在执行另一个采集任务，请等当前任务结束后再启动手动采集。';
}

function isManualTaskId(taskId = '') {
  return /^manual:/i.test(normalizeString(taskId));
}

export function createManualExecutionLockCoordinator({
  accountStore = null,
  lockManager = null,
  injectCookiesForAccount = async () => ({ success: true }),
  shouldReleaseStaleWorkbenchLock = async () => false,
  now = Date.now,
  random = Math.random,
} = {}) {
  function getNow() {
    return typeof now === 'function' ? now() : Date.now();
  }

  async function release(lock = {}) {
    if (!lockManager?.release || !lock) return { released: false };
    return lockManager.release(lock);
  }

  async function prepare({
    action = '',
    msg = {},
    tabId = '',
    tabUrl = '',
  } = {}) {
    if (isRemoteWorkbenchDispatch(msg)) {
      return { locked: false, message: msg, lock: null };
    }

    const platform = inferManualExecutionPlatform({ msg, tabUrl });
    if (platform !== 'xhs' && platform !== 'douyin') {
      return { locked: false, message: msg, lock: null };
    }

    const candidates = await getCandidateAccounts(accountStore, platform, getNow());
    if (!candidates.length) {
      throw new Error(clearManualLockMessage(platform));
    }

    let lastBusy = null;
    for (const account of candidates) {
      const lock = {
        platform,
        accountId: normalizeString(account.accountId),
        taskId: createManualTaskId({ action, tabId, now, random }),
      };
      let lockResult = lockManager?.acquire
        ? await lockManager.acquire(lock)
        : { acquired: true };
      const existingTaskId = normalizeString(lockResult?.existingTaskId);
      if (!lockResult?.acquired && existingTaskId && !isManualTaskId(existingTaskId)) {
        let shouldRelease = false;
        try {
          shouldRelease = await shouldReleaseStaleWorkbenchLock({
            platform,
            accountId: lock.accountId,
            existingTaskId,
            lockResult,
          });
        } catch {
          shouldRelease = false;
        }
        if (shouldRelease && lockManager?.release) {
          await lockManager.release({
            platform,
            accountId: lock.accountId,
            taskId: existingTaskId,
          });
          lockResult = lockManager?.acquire
            ? await lockManager.acquire(lock)
            : { acquired: true };
        }
      }
      if (!lockResult?.acquired) {
        lastBusy = lockResult;
        continue;
      }

      const injectResult = await injectCookiesForAccount(account.cookieJson, platform);
      if (!injectResult?.success) {
        await release(lock);
        throw new Error(`账号登录状态不可用：${normalizeString(injectResult?.error) || '请重新授权账号'}`);
      }

      return {
        locked: true,
        lock,
        account,
        message: {
          ...msg,
          accountId: normalizeString(account.accountId),
          executionLock: lock,
        },
      };
    }

    const message = normalizeString(lastBusy?.reasonMessage) || accountBusyMessage();
    throw new Error(message);
  }

  return {
    prepare,
    release,
  };
}
