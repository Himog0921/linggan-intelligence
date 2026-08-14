import { COMMENT_DEPTH_MODE } from './constants.js';

export const FEEDBACK_META = {
  info: {
    tone: 'info',
    title: '处理中',
    icon: 'infoCircle',
  },
  success: {
    tone: 'success',
    title: '已完成',
    icon: 'check',
  },
  warning: {
    tone: 'warning',
    title: '请注意',
    icon: 'alertTriangle',
  },
  error: {
    tone: 'error',
    title: '操作失败',
    icon: 'xCircle',
  },
};

export function getFeedbackMeta(type = 'info') {
  return FEEDBACK_META[type] || FEEDBACK_META.info;
}

export function getAccountStatusMeta(status = '', cooldownUntil = '') {
  const normalized = String(status || '').trim();
  if (normalized === 'available') {
    return {
      tone: 'success',
      label: '可用',
      detail: '账号可直接参与执行与监控。',
    };
  }
  if (normalized === 'cooldown') {
    const resumeAt = cooldownUntil
      ? `预计 ${new Date(cooldownUntil).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })} 恢复`
      : '冷却中';
    return {
      tone: 'warning',
      label: '冷却中',
      detail: resumeAt,
    };
  }
  if (normalized === 'disabled') {
    return {
      tone: 'neutral',
      label: '已禁用',
      detail: '当前账号不会参与自动执行。',
    };
  }
  return {
    tone: 'neutral',
    label: '待确认',
    detail: '状态尚未同步完成。',
  };
}

export function getMediaStatusMeta(status = '') {
  const normalized = String(status || '待下载').trim();
  const map = {
    下载中: { tone: 'info', label: '下载中' },
    已完成: { tone: 'success', label: '已完成' },
    部分失败: { tone: 'warning', label: '部分失败' },
    失败: { tone: 'error', label: '失败' },
    无媒体: { tone: 'neutral', label: '无媒体' },
    待下载: { tone: 'neutral', label: '待下载' },
  };
  return map[normalized] || map.待下载;
}

export function inferPopupBatchDefaults({
  type = 'notes',
  platform = 'xhs',
  mode = 'batch',
  isSingleComment = false,
} = {}) {
  if (isSingleComment) {
    return {
      count: 10,
      topByLikes: false,
      commentLimit: '30',
      commentDepthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    };
  }

  if (type === 'notes') {
    return {
      count: mode === 'profile' ? 20 : 10,
      topByLikes: mode === 'search',
      commentLimit: '',
      commentDepthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
    };
  }

  return {
    count: mode === 'profile' ? 10 : 10,
    topByLikes: mode === 'search',
    commentLimit: '30',
    commentDepthMode: COMMENT_DEPTH_MODE.TWO_LEVEL,
  };
}

export function summarizeBatchPlan({
  platform = 'xhs',
  type = 'notes',
  mode = 'batch',
  count = 10,
  topByLikes = false,
  commentLimit = '',
  commentDepthMode = COMMENT_DEPTH_MODE.TWO_LEVEL,
  isSingleComment = false,
} = {}) {
  const platformLabel = platform === 'douyin' ? '抖音' : '小红书';
  const surfaceLabel = mode === 'profile' ? '博主页' : mode === 'search' ? '搜索结果页' : '当前详情页';
  const targetLabel = type === 'comments' ? '评论' : (platform === 'douyin' ? '视频' : '笔记');
  const selectionLabel = topByLikes ? '按高互动优先' : (mode === 'profile' ? '按当前主页顺位' : '按当前页面顺位');
  const depthLabel = commentDepthMode === COMMENT_DEPTH_MODE.ALL_REPLIES ? '尽量展开全部回复' : '仅采一级 + 二级';
  const limitLabel = Number(commentLimit || 0) > 0 ? `每条最多 ${Number(commentLimit)} 条` : '每条尽量全部';

  if (isSingleComment) {
    return {
      title: `将采集当前${platform === 'douyin' ? '视频' : '笔记'}评论`,
      detail: `${platformLabel} · ${surfaceLabel} · ${limitLabel} · ${depthLabel}`,
    };
  }

  return {
    title: `将批量采集 ${count} 条${targetLabel}`,
    detail: `${platformLabel} · ${surfaceLabel} · ${selectionLabel}${type === 'comments' ? ` · ${limitLabel} · ${depthLabel}` : ''}`,
  };
}

export function getPopupBatchSettingsStorageKey({
  type = 'notes',
  platform = 'xhs',
  mode = 'batch',
  isSingleComment = false,
} = {}) {
  return `lgbbb.popup.batch.${platform}.${mode}.${type}.${isSingleComment ? 'single' : 'batch'}`;
}

export function readPopupBatchSettings(key = '') {
  if (!key) return null;
  try {
    const raw = window.localStorage.getItem(key);
    return raw ? JSON.parse(raw) : null;
  } catch {
    return null;
  }
}

export function writePopupBatchSettings(key = '', value = {}) {
  if (!key) return;
  try {
    window.localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // ignore UX-only storage failure
  }
}
