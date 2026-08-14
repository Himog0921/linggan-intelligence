import { REMOTE_ERROR_CODE, REMOTE_TASK_TYPE } from '../protocol/schema.js';
import {
  extractContentIdentityFromUrl,
  extractProfileIdentityFromUrl,
} from '../../shared/targetIdentity.js';
import { looksLikeDeadPageTitle } from '../../shared/deadPageSignals.js';

function normalizeCapabilityPageMode(report = {}) {
  const mode = String(report?.mode || '').trim();
  if (mode) return mode;

  const pageType = String(report?.pageType || '').trim();
  if (pageType === 'noteDetail' || pageType === 'videoDetail' || pageType === 'detail') {
    return 'detail';
  }
  if (pageType === 'profile') return 'profile';
  if (pageType === 'search') return 'search';
  return '';
}

function buildReadinessRejection(report = {}) {
  if (report?.readiness?.ready !== false) return null;
  const reasonCode = String(report?.readiness?.reasonCode || '').trim();
  const reasonMessage = String(report?.readiness?.reasonMessage || '').trim();
  if (!reasonCode && !reasonMessage) return null;
  return {
    accepted: false,
    reasonCode: reasonCode || REMOTE_ERROR_CODE.PAGE_CONTEXT_UNAVAILABLE,
    reasonMessage: reasonMessage || '当前页面未形成可执行上下文',
  };
}

function shouldPrioritizeReadinessRejection(reasonCode = '') {
  return new Set([
    REMOTE_ERROR_CODE.CONTENT_NOT_FOUND,
    REMOTE_ERROR_CODE.ERROR_PAGE,
    REMOTE_ERROR_CODE.PAGE_PERMISSION_DENIED,
  ]).has(String(reasonCode || '').trim());
}

function equivalentTaskTypes(taskType = '') {
  const normalized = String(taskType || '').trim();
  const aliases = {
    [REMOTE_TASK_TYPE.XHS_LIST_SCAN]: REMOTE_TASK_TYPE.XHS_BATCH_NOTES,
    [REMOTE_TASK_TYPE.XHS_NOTE_FULL]: REMOTE_TASK_TYPE.XHS_BATCH_NOTES,
    [REMOTE_TASK_TYPE.XHS_COMMENT_SCAN]: REMOTE_TASK_TYPE.XHS_BATCH_COMMENTS,
    [REMOTE_TASK_TYPE.XHS_AUTHOR_PROFILE]: REMOTE_TASK_TYPE.XHS_COLLECT_AUTHOR,
    [REMOTE_TASK_TYPE.XHS_AUTHOR_LINKS]: REMOTE_TASK_TYPE.XHS_AUTHOR_NOTE_LINKS,
  };
  return [normalized, aliases[normalized]].filter(Boolean);
}

export function canDispatchTaskFromCapabilityReport(report = {}, taskType = '', target = {}) {
  const supportedTaskTypes = Array.isArray(report?.capabilities?.canRunTaskTypes)
    ? report.capabilities.canRunTaskTypes
    : [];
  const readinessReasonCode = String(report?.readiness?.reasonCode || '').trim();
  const platformBlocked = Boolean(report?.contextSnapshot?.platformBlocked)
    || readinessReasonCode === REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE;

  if (platformBlocked && !report?.readiness?.ready) {
    return {
      accepted: false,
      reasonCode: readinessReasonCode || REMOTE_ERROR_CODE.PLATFORM_SECURITY_CHALLENGE,
      reasonMessage: String(report?.readiness?.reasonMessage || '平台触发了安全验证，请先完成验证后继续').trim(),
    };
  }

  const readinessRejection = buildReadinessRejection(report);
  if (readinessRejection && shouldPrioritizeReadinessRejection(readinessRejection.reasonCode)) {
    return readinessRejection;
  }

  // 失效检测：任务目标是某条具体笔记/视频（detail），但当前页面是失效页。两条信号任一命中即判
  // CONTENT_NOT_FOUND（终态，停止重试），先于"任务类型不支持"，避免失效内容被误报成模糊的
  // "能力不支持"在服务端反复重试刷屏（topic-dashboard PR#41 根因）：
  //   ① URL 被跳走：失效内容被平台重定向到 /404/首页，report.url 提取不出目标 noteId
  //   ② 页面 title 是失效页：复用已有 tab 时 URL 可能停在原地址（没跳走），但 title 已变成
  //      "页面不见了/暂时无法浏览/已删除"等——只看 URL 会漏，必须再看 title（2.0.76 盲区修复）
  if (String(target?.pageType || '').trim() === 'detail') {
    const targetContentId = extractContentIdentityFromUrl(target?.url);
    const currentContentId = extractContentIdentityFromUrl(report?.url);
    const pageTitle = String(report?.title || '').trim();
    const looksLikeDeadPage = looksLikeDeadPageTitle(pageTitle);
    if (targetContentId && (!currentContentId || looksLikeDeadPage)) {
      return {
        accepted: false,
        reasonCode: REMOTE_ERROR_CODE.CONTENT_NOT_FOUND,
        reasonMessage: `目标内容 ${targetContentId} 无法访问（可能已删除/设私密/失效）`,
      };
    }
  }

  // Non-terminal readiness failures describe the current page more accurately
  // than a missing task type. For example, an XHS login/scan page has no
  // collection capabilities yet, but the station can still run the task once
  // the page becomes ready.
  if (readinessRejection) return readinessRejection;

  if (!equivalentTaskTypes(taskType).some((type) => supportedTaskTypes.includes(type))) {
    return {
      accepted: false,
      reasonCode: REMOTE_ERROR_CODE.UNSUPPORTED_TASK_TYPE,
      reasonMessage: '当前页面能力报告未声明支持该任务类型',
    };
  }

  const expectedPageType = String(target?.pageType || '').trim();
  const actualPageType = normalizeCapabilityPageMode(report);
  if (expectedPageType && expectedPageType !== 'unknown' && actualPageType && actualPageType !== expectedPageType) {
    return {
      accepted: false,
      reasonCode: REMOTE_ERROR_CODE.PAGE_TYPE_MISMATCH,
      reasonMessage: `当前页面类型不对：任务需要 ${expectedPageType}，实际是 ${actualPageType}`,
    };
  }

  if (expectedPageType === 'profile') {
    const targetProfileId = extractProfileIdentityFromUrl(target?.url);
    const currentProfileId = extractProfileIdentityFromUrl(report?.url);
    if (targetProfileId && currentProfileId && targetProfileId !== currentProfileId) {
      return {
        accepted: false,
        reasonCode: REMOTE_ERROR_CODE.PAGE_TARGET_MISMATCH,
        reasonMessage: `当前页面不是任务目标博主：任务要 ${targetProfileId}，实际是 ${currentProfileId}`,
      };
    }
  }

  if (expectedPageType === 'detail') {
    const targetContentId = extractContentIdentityFromUrl(target?.url);
    const currentContentId = extractContentIdentityFromUrl(report?.url);
    if (targetContentId && targetContentId !== currentContentId) {
      return {
        accepted: false,
        reasonCode: REMOTE_ERROR_CODE.PAGE_TARGET_MISMATCH,
        reasonMessage: currentContentId
          ? `当前页面不是任务目标内容：任务要 ${targetContentId}，实际是 ${currentContentId}`
          : `当前页面不是任务目标内容：任务要 ${targetContentId}`,
      };
    }
  }

  return {
    accepted: true,
    reasonCode: '',
    reasonMessage: '',
  };
}
