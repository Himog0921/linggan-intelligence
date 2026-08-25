import { buildCapabilityReport } from '../../workbench/runtime/capabilityReportBuilder.js';
import { REMOTE_ERROR_CODE } from '../../workbench/protocol/schema.js';
import { detectPageType } from './pageDetector.js';

function getDefaultWindow() {
  return typeof window !== 'undefined' ? window : { location: { href: '' } };
}

function normalizeText(value = '') {
  return String(value || '').trim();
}

function resolveXhsMode(page = {}) {
  if (page.type === 'noteDetail') return 'detail';
  if (page.type === 'profile') return 'profile';
  if (page.type === 'search') return 'search';
  return 'unknown';
}

function hasXhsAppScanVerification(win = {}) {
  const title = String(win?.document?.title || '').trim();
  const bodyText = String(win?.document?.body?.innerText || '').trim().slice(0, 3000);
  const text = `${title}\n${bodyText}`;
  return /使用已登录.*小红书.*扫码验证身份|小红书\s*APP.*扫码验证身份|扫码验证身份/.test(text);
}

/**
 * @param {Record<string, any>} [options]
 */
export function createXhsPlatformAdapter(options = {}) {
  const {
    detectPage = detectPageType,
    getWindow = getDefaultWindow,
  } = options;
  return {
    platform: 'xhs',
    id: 'xhs',
    platformName: '小红书',
    hostPattern: /xiaohongshu\.com/,

    detectPage(ctx = {}) {
      const page = typeof detectPage === 'function' ? detectPage(ctx) : {};
      const win = ctx.win || getWindow();
      return {
        ...page,
        url: normalizeText(page?.url || win?.location?.href),
      };
    },

    normalizeTarget(task = {}) {
      const target = task?.target && typeof task.target === 'object' && !Array.isArray(task.target)
        ? task.target
        : {};
      return {
        platform: 'xhs',
        taskType: normalizeText(task.taskType),
        pageType: normalizeText(target.pageType),
        url: normalizeText(target.url || task.targetUrl),
      };
    },

    async checkCapability(task = {}, ctx = {}) {
      const page = this.detectPage(ctx);
      const mode = resolveXhsMode(page);
      const target = this.normalizeTarget(task);
      const win = ctx.win || getWindow();
      const title = String(win?.document?.title || '').trim();
      const requiresAppScan = hasXhsAppScanVerification(win);
      return {
        ...buildCapabilityReport({
          platform: 'xhs',
          mode,
          pageType: page.type,
          url: page.url,
          title,
          isStableSearchList: mode === 'search',
          platformBlocked: requiresAppScan,
          blockReasonCode: requiresAppScan ? REMOTE_ERROR_CODE.LOGIN_REQUIRED : '',
          blockReasonMessage: requiresAppScan
            ? '小红书要求使用已登录账号的 APP 扫码验证身份'
            : '',
          capabilities: {
            canCollectPrimary: mode === 'detail',
            canCollectSecondary: mode === 'detail' || mode === 'profile',
            canCollectAuthor: mode === 'profile',
            canCollectComments: mode === 'detail',
            canDownloadCommentImages: false,
            canBatchNotes: mode === 'search' || mode === 'profile',
            canBatchComments: mode === 'search' || mode === 'profile',
            secondaryAction: mode === 'profile' ? 'author' : (mode === 'detail' ? 'comment' : 'none'),
          },
        }),
        target,
      };
    },

    async prepare() {
      return { success: true };
    },

    async collect() {
      throw new Error('xhs adapter collect is platform-controller owned');
    },

    pause() {},
    resume() {},
    stop() {},
    cleanup() {},
  };
}

export default createXhsPlatformAdapter();
