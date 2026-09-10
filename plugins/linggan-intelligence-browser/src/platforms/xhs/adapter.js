import { hasXhsAppScanVerification } from './accountObservation.js';

const LOGIN_REQUIRED = 'login_required';

function buildCapabilityReport(input = {}) {
  return { ...input, collectedAt: new Date().toISOString(), runtime: 'linggan_local_producer' };
}
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
          blockReasonCode: requiresAppScan ? LOGIN_REQUIRED : '',
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
