import {
  REMOTE_ERROR_CODE,
  REMOTE_TARGET_PAGE_TYPE,
  REMOTE_TASK_TYPE,
  WORKBENCH_MESSAGE_TYPE,
  WORKBENCH_PROTOCOL_VERSION,
} from '../protocol/schema.js';

function inferCanRunTaskTypes(pageContext = {}) {
  if (pageContext.platformBlocked) return [];

  const platform = String(pageContext.platform || '').trim();
  const capabilities = pageContext.capabilities || {};
  const taskTypes = [];

  if (platform === 'xhs') {
    if (capabilities.canBatchNotes || (String(pageContext.mode || '').trim() === 'detail' && capabilities.canCollectPrimary)) {
      taskTypes.push(REMOTE_TASK_TYPE.XHS_BATCH_NOTES);
    }
    if (capabilities.canBatchComments || (String(pageContext.mode || '').trim() === 'detail' && capabilities.canCollectComments)) {
      taskTypes.push(REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS);
    }
    if (capabilities.canCollectAuthor) taskTypes.push(REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR);
    if (
      String(pageContext.pageType || pageContext.mode || '').trim() === REMOTE_TARGET_PAGE_TYPE.PROFILE
      && capabilities.canBatchNotes
    ) {
      taskTypes.push(REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS);
    }
    return taskTypes;
  }

  if (platform === 'douyin') {
    if (capabilities.canBatchNotes) taskTypes.push(REMOTE_TASK_TYPE.DOUYIN_BATCH_NOTES);
    if (capabilities.canBatchComments) taskTypes.push(REMOTE_TASK_TYPE.DOUYIN_BATCH_COMMENTS);
    if (capabilities.canCollectAuthor) taskTypes.push(REMOTE_TASK_TYPE.DOUYIN_COLLECT_AUTHOR);
    if (capabilities.canCollectComments) taskTypes.push(REMOTE_TASK_TYPE.DOUYIN_SINGLE_COMMENTS);
    if (capabilities.canDownloadCommentImages) taskTypes.push(REMOTE_TASK_TYPE.DOUYIN_COMMENT_IMAGE_DOWNLOAD);
  }

  return taskTypes;
}

function inferRecommendedNextAction(pageContext = {}) {
  if (pageContext.platformBlocked) {
    return 'resolve_platform_security_challenge';
  }
  const pageType = String(pageContext.pageType || '').trim();
  if (pageType === REMOTE_TARGET_PAGE_TYPE.SEARCH && !pageContext.isStableSearchList) {
    return 'wait_for_search_results_stable';
  }
  if (pageType === REMOTE_TARGET_PAGE_TYPE.UNKNOWN || !pageType) {
    return 'switch_to_supported_page';
  }
  if (pageType === REMOTE_TARGET_PAGE_TYPE.DETAIL && pageContext.platform === 'douyin' && !pageContext.isDyStrictDetailPage) {
    return 'open_strict_detail_page';
  }
  return 'ready';
}

function inferReadiness(pageContext = {}, canRunTaskTypes = []) {
  if (pageContext.platformBlocked) {
    return {
      ready: false,
      reasonCode: String(pageContext.blockReasonCode || REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE).trim(),
      reasonMessage: String(pageContext.blockReasonMessage || '平台触发了安全验证，请先完成验证后继续').trim(),
    };
  }

  if (canRunTaskTypes.length > 0) {
    return {
      ready: true,
      reasonCode: '',
      reasonMessage: '',
    };
  }

  const pageType = String(pageContext.pageType || '').trim();
  if (pageType === REMOTE_TARGET_PAGE_TYPE.SEARCH && !pageContext.isStableSearchList) {
    return {
      ready: false,
      reasonCode: REMOTE_ERROR_CODE.SEARCH_LIST_UNSTABLE,
      reasonMessage: '搜索结果列表尚未形成稳定可遍历状态',
    };
  }

  return {
    ready: false,
    reasonCode: REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE,
    reasonMessage: '当前页面未形成可执行上下文',
  };
}

export function buildCapabilityReport(pageContext = {}) {
  const canRunTaskTypes = inferCanRunTaskTypes(pageContext);
  const readiness = inferReadiness(pageContext, canRunTaskTypes);

  return {
    type: WORKBENCH_MESSAGE_TYPE.CAPABILITY_REPORT,
    protocolVersion: WORKBENCH_PROTOCOL_VERSION,
    contextVersion: WORKBENCH_PROTOCOL_VERSION,
    supportedProtocolVersions: [WORKBENCH_PROTOCOL_VERSION],
    platform: String(pageContext.platform || '').trim(),
    mode: String(pageContext.mode || '').trim(),
    pageType: String(pageContext.pageType || '').trim(),
    url: String(pageContext.url || '').trim(),
    title: String(pageContext.title || '').trim(),
    isStableSearchList: Boolean(pageContext.isStableSearchList),
    isDyVideoPage: Boolean(pageContext.isDyVideoPage),
    isDyStrictDetailPage: Boolean(pageContext.isDyStrictDetailPage),
    capabilities: {
      ...(pageContext.capabilities || {}),
      canRunTaskTypes,
    },
    readiness,
    recommendedNextAction: inferRecommendedNextAction(pageContext),
    contextSnapshot: {
      isStableSearchList: Boolean(pageContext.isStableSearchList),
      isDyVideoPage: Boolean(pageContext.isDyVideoPage),
      isDyStrictDetailPage: Boolean(pageContext.isDyStrictDetailPage),
      platformBlocked: Boolean(pageContext.platformBlocked),
    },
  };
}
