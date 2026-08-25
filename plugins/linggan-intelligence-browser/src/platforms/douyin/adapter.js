import { buildCapabilityReport } from '../../workbench/runtime/capabilityReportBuilder.js';
import { REMOTE_ERROR_CODE } from '../../workbench/protocol/schema.js';

function getDefaultWindow() {
  return typeof window !== 'undefined' ? window : { location: { href: '' } };
}

function getDefaultDocument() {
  return typeof document !== 'undefined' ? document : null;
}

function normalizeText(value = '') {
  return String(value || '').trim();
}

function resolveDouyinMode(page = {}) {
  if (page.type === 'profile') return 'profile';
  if (page.type === 'search') return 'search';
  if (page.type === 'videoDetail' || page.type === 'noteDetail') return 'detail';
  return 'unknown';
}

/**
 * @param {Record<string, any>} [options]
 */
export function createDouyinPlatformAdapter(options = {}) {
  const {
    detectDouyinPageType,
    detectDouyinSearchBatchContext,
    detectDouyinSecurityChallenge,
    isStrictDouyinDetailPage,
    getWindow = getDefaultWindow,
    getDocument = getDefaultDocument,
  } = options;
  return {
    platform: 'douyin',
    id: 'douyin',
    platformName: '抖音',
    hostPattern: /douyin\.com/,

    detectPage(ctx = {}) {
      const win = ctx.win || getWindow();
      if (typeof detectDouyinPageType !== 'function') {
        return {
          type: 'unknown',
          url: normalizeText(win?.location?.href),
        };
      }
      const page = detectDouyinPageType(ctx);
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
        platform: 'douyin',
        taskType: normalizeText(task.taskType),
        pageType: normalizeText(target.pageType),
        url: normalizeText(target.url || task.targetUrl),
      };
    },

    async checkCapability(task = {}, ctx = {}) {
      const win = ctx.win || getWindow();
      const root = ctx.root || getDocument();
      const href = normalizeText(win?.location?.href);
      const platformBlocked = typeof detectDouyinSecurityChallenge === 'function'
        ? detectDouyinSecurityChallenge({ root, href })
        : false;
      const target = this.normalizeTarget(task);

      if (platformBlocked) {
        return {
          ...buildCapabilityReport({
            platform: 'douyin',
            mode: 'unknown',
            pageType: 'unknown',
            url: href,
            platformBlocked: true,
            blockReasonCode: REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
            blockReasonMessage: '检测到抖音安全验证，请先完成验证后继续操作',
            capabilities: {
              canCollectPrimary: false,
              canCollectSecondary: false,
              canCollectAuthor: false,
              canCollectComments: false,
              canDownloadCommentImages: false,
              canBatchNotes: false,
              canBatchComments: false,
              secondaryAction: 'none',
            },
          }),
          platformBlocked: true,
          blockReasonCode: REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
          target,
        };
      }

      const page = this.detectPage({ ...ctx, win });
      const searchContext = typeof detectDouyinSearchBatchContext === 'function'
        ? detectDouyinSearchBatchContext(win)
        : {};
      const isDetailPage = page.type === 'videoDetail' || page.type === 'noteDetail';
      const isStrictDetail = typeof isStrictDouyinDetailPage === 'function'
        ? isStrictDouyinDetailPage(href)
        : false;
      const mode = resolveDouyinMode(page);

      return {
        ...buildCapabilityReport({
          platform: 'douyin',
          mode,
          pageType: page.type,
          url: page.url,
          title: String(win?.document?.title || '').trim(),
          isDyVideoPage: isDetailPage,
          isDyStrictDetailPage: isStrictDetail,
          isStableSearchList: Boolean(searchContext.stableSearchList),
          capabilities: {
            canCollectPrimary: isDetailPage,
            canCollectSecondary: isDetailPage || page.type === 'profile',
            canCollectAuthor: page.type === 'profile',
            canCollectComments: isDetailPage,
            canDownloadCommentImages: isStrictDetail,
            canBatchNotes: page.type === 'profile' || (page.type === 'search' && Boolean(searchContext.stableSearchList)),
            canBatchComments: page.type === 'profile' || (page.type === 'search' && Boolean(searchContext.stableSearchList)),
            secondaryAction: isDetailPage ? 'comment' : (page.type === 'profile' ? 'author' : 'none'),
          },
        }),
        searchKeyword: searchContext.keyword || '',
        target,
      };
    },

    async prepare() {
      return { success: true };
    },

    async collect() {
      throw new Error('douyin adapter collect is platform-controller owned');
    },

    pause() {},
    resume() {},
    stop() {},
    cleanup() {},
  };
}
